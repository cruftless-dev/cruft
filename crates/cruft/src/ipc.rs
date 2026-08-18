
use crate::net::{install_emitter, install_emitter_methods_own, net_emit};
use crate::register::{new_object, register_method};
use rusty_js_runtime::send_ir::{lower_to_send_ir, rematerialize_send_ir, LowerCtx, SendIr};
use rusty_js_runtime::value::{JsString, Object, ObjectRef};
use rusty_js_runtime::{AgentId, HostEnqueuePhase, HostHook, Runtime, RuntimeError, Value};
use std::cell::RefCell;
use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::process::{Child, Command, Stdio};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

struct IpcChannel {
    agent_id: AgentId,
    writer: Arc<Mutex<Option<TcpStream>>>,
    pending_writes: Arc<Mutex<VecDeque<Vec<u8>>>>,
    inbox: Arc<Mutex<Vec<String>>>,
    target: ObjectRef,
    realm: usize,
    alive: Arc<AtomicBool>,
    child: Option<Child>,
    exit_done: bool,
}

thread_local! {
    static CHANNELS: RefCell<Vec<Option<IpcChannel>>> = const { RefCell::new(Vec::new()) };
}

fn notify_agent_wake(wake: &Arc<(Mutex<u64>, Condvar)>) {
    let (lock, cv) = &**wake;
    let mut generation = lock.lock().unwrap();
    *generation = generation.wrapping_add(1);
    cv.notify_all();
}

fn data_clone_error(rt: &mut Runtime, message: &str) -> RuntimeError {
    let mut o = Object::new_ordinary();
    o.set_own(
        "name".into(),
        Value::String(Rc::new(JsString::from("DataCloneError"))),
    );
    o.set_own(
        "message".into(),
        Value::String(Rc::new(JsString::from(message.to_string()))),
    );
    o.set_own("code".into(), Value::Number(25.0));
    RuntimeError::Thrown(Value::Object(rt.alloc_object(o)))
}

struct InProcessWorker {
    agent_id: AgentId,
    worker_agent_id: AgentId,
    to_worker: Sender<SendIr>,
    from_worker: Arc<Mutex<Receiver<SendIr>>>,
    target: ObjectRef,
    realm: usize,
    alive: Arc<AtomicBool>,
    exit_done: bool,
}

thread_local! {
    static INPROCESS_WORKERS: RefCell<Vec<Option<InProcessWorker>>> = const { RefCell::new(Vec::new()) };
}

static NEXT_INPROCESS_WORKER_AGENT_ID: AtomicU64 = AtomicU64::new(1);

fn next_inprocess_worker_agent_id() -> AgentId {
    AgentId::from_raw(NEXT_INPROCESS_WORKER_AGENT_ID.fetch_add(1, Ordering::SeqCst))
}

fn json_stringify(rt: &mut Runtime, val: &Value) -> String {
    if let Value::Object(j) = rt.global_get("JSON") {
        let f = rt.object_get(j, "stringify");
        if rt.is_callable(&f) {
            if let Ok(Value::String(s)) = rt.call_function(f, Value::Object(j), vec![val.clone()]) {
                return s.as_str().to_string();
            }
        }
    }
    "null".to_string()
}

fn json_parse(rt: &mut Runtime, line: &str) -> Value {
    if let Value::Object(j) = rt.global_get("JSON") {
        let f = rt.object_get(j, "parse");
        if rt.is_callable(&f) {
            let arg = Value::String(Rc::new(JsString::from(line.to_string())));
            if let Ok(v) = rt.call_function(f, Value::Object(j), vec![arg]) {
                return v;
            }
        }
    }
    Value::Undefined
}

pub fn attach(rt: &Runtime, stream: TcpStream, target: ObjectRef, child: Option<Child>) -> usize {
    let inbox = Arc::new(Mutex::new(Vec::new()));
    let alive = Arc::new(AtomicBool::new(true));
    let wake = rt.agent_wake_handle();
    spawn_reader(
        stream.try_clone().expect("ipc: clone stream"),
        inbox.clone(),
        alive.clone(),
        wake,
    );
    CHANNELS.with(|v| {
        let mut v = v.borrow_mut();
        v.push(Some(IpcChannel {
            agent_id: rt.agent_id(),
            writer: Arc::new(Mutex::new(Some(stream))),
            pending_writes: Arc::new(Mutex::new(VecDeque::new())),
            inbox,
            target,
            realm: rt.current_realm,
            alive,
            child,
            exit_done: false,
        }));
        v.len() - 1
    })
}

fn spawn_reader(
    reader: TcpStream,
    inbox: Arc<Mutex<Vec<String>>>,
    alive: Arc<AtomicBool>,
    wake: Arc<(Mutex<u64>, Condvar)>,
) {
    std::thread::spawn(move || {
        let mut br = BufReader::new(reader);
        loop {
            let mut line = String::new();
            match br.read_line(&mut line) {
                Ok(0) | Err(_) => {
                    alive.store(false, Ordering::SeqCst);
                    notify_agent_wake(&wake);
                    break;
                }
                Ok(_) => {
                    let t = line.trim_end_matches(['\n', '\r']).to_string();
                    if !t.is_empty() {
                        inbox.lock().unwrap().push(t);
                        notify_agent_wake(&wake);
                    }
                }
            }
        }
    });
}

fn attach_pending(rt: &Runtime, listener: TcpListener, target: ObjectRef, child: Child) -> usize {
    let inbox = Arc::new(Mutex::new(Vec::new()));
    let alive = Arc::new(AtomicBool::new(true));
    let writer = Arc::new(Mutex::new(None));
    let pending_writes = Arc::new(Mutex::new(VecDeque::<Vec<u8>>::new()));
    let inbox_for_accept = inbox.clone();
    let alive_for_accept = alive.clone();
    let writer_for_accept = writer.clone();
    let pending_for_accept = pending_writes.clone();
    let wake = rt.agent_wake_handle();
    std::thread::spawn(move || match listener.accept() {
        Ok((stream, _)) => {
            stream.set_nodelay(true).ok();
            match stream.try_clone() {
                Ok(reader) => {
                    {
                        let mut writer_guard = writer_for_accept.lock().unwrap();
                        *writer_guard = Some(stream);
                        if let Some(writer) = writer_guard.as_mut() {
                            let pending: Vec<Vec<u8>> =
                                pending_for_accept.lock().unwrap().drain(..).collect();
                            for data in pending {
                                if writer.write_all(&data).is_err() {
                                    alive_for_accept.store(false, Ordering::SeqCst);
                                    notify_agent_wake(&wake);
                                    return;
                                }
                            }
                        }
                    }
                    notify_agent_wake(&wake);
                    spawn_reader(reader, inbox_for_accept, alive_for_accept, wake);
                }
                Err(_) => {
                    alive_for_accept.store(false, Ordering::SeqCst);
                    notify_agent_wake(&wake);
                }
            }
        }
        Err(_) => {
            alive_for_accept.store(false, Ordering::SeqCst);
            notify_agent_wake(&wake);
        }
    });
    CHANNELS.with(|v| {
        let mut v = v.borrow_mut();
        v.push(Some(IpcChannel {
            agent_id: rt.agent_id(),
            writer,
            pending_writes,
            inbox,
            target,
            realm: rt.current_realm,
            alive,
            child: Some(child),
            exit_done: false,
        }));
        v.len() - 1
    })
}

fn chan_of(rt: &Runtime, obj: ObjectRef) -> Option<usize> {
    match rt.object_get(obj, "__ipc_chan") {
        Value::Number(n) => Some(n as usize),
        _ => None,
    }
}

pub fn send_value(rt: &mut Runtime, id: usize, msg: &Value) -> bool {
    let line = json_stringify(rt, msg);
    let agent_id = rt.agent_id();
    CHANNELS.with(|v| {
        if let Some(Some(ch)) = v.borrow_mut().get_mut(id) {
            if ch.agent_id != agent_id {
                return false;
            }
            let mut data = line.into_bytes();
            data.push(b'\n');
            if let Some(writer) = ch.writer.lock().unwrap().as_mut() {
                writer.write_all(&data).is_ok()
            } else {
                ch.pending_writes.lock().unwrap().push_back(data);
                true
            }
        } else {
            false
        }
    })
}

pub fn close(rt: &Runtime, id: usize) {
    let agent_id = rt.agent_id();
    CHANNELS.with(|v| {
        if let Some(Some(ch)) = v.borrow_mut().get_mut(id) {
            if ch.agent_id != agent_id {
                return;
            }
            ch.alive.store(false, Ordering::SeqCst);
            if let Some(writer) = ch.writer.lock().unwrap().as_mut() {
                let _ = writer.shutdown(std::net::Shutdown::Both);
            }
        }
    });
}

fn send_ir_from_value(rt: &Runtime, value: &Value) -> Result<SendIr, RuntimeError> {
    let mut ctx = LowerCtx::new(None);
    lower_to_send_ir(rt, value, &mut ctx)
}

fn value_from_send_ir(rt: &mut Runtime, ir: &SendIr) -> Result<Value, RuntimeError> {
    let mut table = std::collections::HashMap::new();
    value_from_send_ir_with_table(rt, ir, &mut table)
}

fn value_from_send_ir_with_table(
    rt: &mut Runtime,
    ir: &SendIr,
    table: &mut std::collections::HashMap<u32, ObjectRef>,
) -> Result<Value, RuntimeError> {
    if let SendIr::Composite { props, .. } = ir {
        if let Some((_, buffer_ir)) = props.iter().find(|(key, _)| key == "__wasm_memory_buffer") {
            if let Value::Object(ab) = value_from_send_ir_with_table(rt, buffer_ir, table)? {
                let mem = crate::wasm::make_memory_object_from_buffer(rt, ab);
                for key in [
                    "__wasm_memory_maximum",
                    "__wasm_memory_address64",
                    "__wasm_memory_shared",
                ] {
                    if let Some((_, value_ir)) = props.iter().find(|(prop, _)| prop == key) {
                        let value = value_from_send_ir_with_table(rt, value_ir, table)?;
                        rt.object_set(mem, key.to_string(), value);
                    }
                }
                return Ok(Value::Object(mem));
            }
        }
    }
    match ir {
        SendIr::Composite {
            ref_id,
            is_array,
            proto_null,
            props,
        } => {
            let obj = if *is_array {
                rt.alloc_object(Object::new_array())
            } else {
                new_object(rt)
            };
            if *proto_null {
                rt.obj_mut(obj).proto = None;
            }
            table.insert(*ref_id, obj);
            for (key, value_ir) in props {
                let value = value_from_send_ir_with_table(rt, value_ir, table)?;
                rt.object_set(obj, key.clone().into(), value);
            }
            Ok(Value::Object(obj))
        }
        SendIr::Ref(ref_id) => table
            .get(ref_id)
            .copied()
            .map(Value::Object)
            .ok_or_else(|| RuntimeError::TypeError("send IR: dangling reference".into())),
        other => rematerialize_send_ir(rt, other, None, table),
    }
}

fn install_endpoint(rt: &mut Runtime, obj: ObjectRef, chan_id: usize) {
    install_endpoint_named(rt, obj, chan_id, "send");
}

fn install_endpoint_named(rt: &mut Runtime, obj: ObjectRef, chan_id: usize, send_name: &str) {
    rt.set_engine_sentinel(obj, "__ipc_chan", Value::Number(chan_id as f64));
    rt.object_set(obj, "connected".into(), Value::Boolean(true));
    register_method(rt, obj, send_name, |rt, args| {
        let this = match rt.current_this() {
            Value::Object(t) => t,
            _ => return Ok(Value::Boolean(false)),
        };
        let chan = match chan_of(rt, this) {
            Some(c) => c,
            None => return Ok(Value::Boolean(false)),
        };
        let msg = args.first().cloned().unwrap_or(Value::Undefined);
        let ok = send_value(rt, chan, &msg);
        if let Some(cb) = args.iter().find(|v| rt.is_callable(v)).cloned() {
            let _ = rt.call_function(cb, Value::Undefined, vec![Value::Null]);
        }
        Ok(Value::Boolean(ok))
    });
    register_method(rt, obj, "disconnect", |rt, _a| {
        let this = match rt.current_this() {
            Value::Object(t) => t,
            _ => return Ok(Value::Undefined),
        };
        if let Some(chan) = chan_of(rt, this) {
            close(rt, chan);
        }
        rt.object_set(this, "connected".into(), Value::Boolean(false));
        net_emit(rt, this, "disconnect", Vec::new());
        Ok(Value::Undefined)
    });
    register_method(rt, obj, "close", |rt, _a| {
        let this = match rt.current_this() {
            Value::Object(t) => t,
            _ => return Ok(Value::Undefined),
        };
        if let Some(chan) = chan_of(rt, this) {
            close(rt, chan);
        }
        Ok(Value::Undefined)
    });
}

pub fn spawn_worker(
    rt: &mut Runtime,
    filename: &str,
    worker_data: &Value,
) -> Result<ObjectRef, RuntimeError> {
    let wd_json = json_stringify(rt, worker_data);
    let listener = TcpListener::bind("127.0.0.1:0")
        .map_err(|e| RuntimeError::TypeError(format!("Worker: bind: {e}")))?;
    let addr = listener
        .local_addr()
        .map_err(|e| RuntimeError::TypeError(format!("Worker: addr: {e}")))?
        .to_string();
    let exe = std::env::current_exe()
        .map_err(|e| RuntimeError::TypeError(format!("Worker: exe: {e}")))?;
    let mut cmd = Command::new(exe);
    cmd.arg("run").arg(filename);
    cmd.env("CRUFT_CHANNEL_ADDR", &addr);
    cmd.env("CRUFT_WORKER", "1");
    cmd.env("CRUFT_WORKER_DATA", &wd_json);
    cmd.stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    let child = cmd
        .spawn()
        .map_err(|e| RuntimeError::TypeError(format!("Worker: spawn: {e}")))?;
    let (stream, _) = listener
        .accept()
        .map_err(|e| RuntimeError::TypeError(format!("Worker: accept: {e}")))?;
    stream.set_nodelay(true).ok();
    let worker = new_object(rt);
    install_emitter_methods_own(rt, worker);
    rt.object_set(
        worker,
        "__worker_threads_worker__".into(),
        Value::Boolean(true),
    );
    rt.object_set(worker, "threadId".into(), Value::Number(1.0));
    let chan_id = attach(rt, stream, worker, Some(child));
    install_endpoint_named(rt, worker, chan_id, "postMessage");
    for noop in ["ref", "unref"] {
        register_method(rt, worker, noop, |rt, _a| Ok(rt.current_this()));
    }
    register_method(rt, worker, "terminate", |rt, _a| {
        let this = match rt.current_this() {
            Value::Object(t) => t,
            _ => return Ok(Value::Undefined),
        };
        if let Some(chan) = chan_of(rt, this) {
            CHANNELS.with(|v| {
                if let Some(Some(ch)) = v.borrow_mut().get_mut(chan) {
                    if let Some(c) = ch.child.as_mut() {
                        let _ = c.kill();
                    }
                }
            });
        }

        rt.object_set(this, "__worker_terminated".into(), Value::Boolean(true));
        let p = rusty_js_runtime::promise::new_promise(rt);
        rusty_js_runtime::promise::resolve_promise(rt, p, Value::Number(1.0));
        Ok(Value::Object(p))
    });

    rt.enqueue_host_phase_rooted(
        HostEnqueuePhase::HostCompletionMacrotask,
        "worker.online",
        vec![worker],
        move |rt| {
            net_emit(rt, worker, "online", Vec::new());
            Ok(())
        },
    );
    Ok(worker)
}

pub fn spawn_inprocess_worker(
    rt: &mut Runtime,
    filename: &str,
    worker_data: &Value,
) -> Result<ObjectRef, RuntimeError> {
    let worker_data_ir = send_ir_from_value(rt, worker_data)?;
    let (to_worker_tx, to_worker_rx) = std::sync::mpsc::channel::<SendIr>();
    let (to_main_tx, to_main_rx) = std::sync::mpsc::channel::<SendIr>();
    let alive = Arc::new(AtomicBool::new(true));
    let worker_alive = alive.clone();
    let file = filename.to_string();
    let parent_wake = rt.agent_wake_handle();
    let worker_agent_id = next_inprocess_worker_agent_id();
    std::thread::Builder::new()
        .name("cruft-worker".to_string())
        .stack_size(16 * 1024 * 1024)
        .spawn(move || {
            run_inprocess_worker_thread(
                worker_agent_id,
                file,
                worker_data_ir,
                to_worker_rx,
                to_main_tx,
                worker_alive,
                parent_wake,
            );
        })
        .map_err(|e| RuntimeError::TypeError(format!("Worker: thread spawn: {e}")))?;

    let worker = new_object(rt);
    install_emitter_methods_own(rt, worker);
    rt.object_set(
        worker,
        "__worker_threads_worker__".into(),
        Value::Boolean(true),
    );
    rt.object_set(worker, "threadId".into(), Value::Number(1.0));
    rt.object_set(worker, "connected".into(), Value::Boolean(true));
    let id = INPROCESS_WORKERS.with(|v| {
        let mut v = v.borrow_mut();
        v.push(Some(InProcessWorker {
            agent_id: rt.agent_id(),
            worker_agent_id,
            to_worker: to_worker_tx,
            from_worker: Arc::new(Mutex::new(to_main_rx)),
            target: worker,
            realm: rt.current_realm,
            alive,
            exit_done: false,
        }));
        v.len() - 1
    });
    rt.set_engine_sentinel(worker, "__inprocess_worker", Value::Number(id as f64));
    rt.set_engine_sentinel(
        worker,
        "__inprocess_worker_agent_id",
        Value::Number(worker_agent_id.raw() as f64),
    );
    rt.enqueue_host_phase_rooted(
        HostEnqueuePhase::HostCompletionMacrotask,
        "worker.online",
        vec![worker],
        move |rt| {
            net_emit(rt, worker, "online", Vec::new());
            Ok(())
        },
    );
    register_method(rt, worker, "postMessage", |rt, args| {
        let this = match rt.current_this() {
            Value::Object(t) => t,
            _ => return Ok(Value::Boolean(false)),
        };
        let id = match rt.object_get(this, "__inprocess_worker") {
            Value::Number(n) => n as usize,
            _ => return Ok(Value::Boolean(false)),
        };
        let msg = args.first().cloned().unwrap_or(Value::Undefined);
        let cloned = post_message_clone_with_optional_transfer(rt, &msg, args.get(1).cloned())?;
        let ir = send_ir_from_value(rt, &cloned)?;
        let ok = INPROCESS_WORKERS.with(|v| {
            v.borrow()
                .get(id)
                .and_then(|slot| slot.as_ref())
                .map(|w| w.agent_id == rt.agent_id() && w.to_worker.send(ir).is_ok())
                .unwrap_or(false)
        });
        if let Some(cb) = args.iter().find(|v| rt.is_callable(v)).cloned() {
            let _ = rt.call_function(cb, Value::Undefined, vec![Value::Null]);
        }
        Ok(Value::Boolean(ok))
    });
    for noop in ["ref", "unref"] {
        register_method(rt, worker, noop, |rt, _a| Ok(rt.current_this()));
    }
    register_method(rt, worker, "terminate", |rt, _a| {
        let this = match rt.current_this() {
            Value::Object(t) => t,
            _ => return Ok(Value::Undefined),
        };
        if let Value::Number(n) = rt.object_get(this, "__inprocess_worker") {
            INPROCESS_WORKERS.with(|v| {
                if let Some(Some(w)) = v.borrow_mut().get_mut(n as usize) {
                    if w.agent_id == rt.agent_id() {
                        w.alive.store(false, Ordering::SeqCst);
                    }
                }
            });
        }
        rt.object_set(this, "connected".into(), Value::Boolean(false));

        rt.object_set(this, "__worker_terminated".into(), Value::Boolean(true));
        let p = rusty_js_runtime::promise::new_promise(rt);
        rusty_js_runtime::promise::resolve_promise(rt, p, Value::Number(1.0));
        Ok(Value::Object(p))
    });
    Ok(worker)
}

fn run_inprocess_worker_thread(
    worker_agent_id: AgentId,
    filename: String,
    worker_data: SendIr,
    from_main: Receiver<SendIr>,
    to_main: Sender<SendIr>,
    alive: Arc<AtomicBool>,
    parent_wake: Arc<(Mutex<u64>, Condvar)>,
) {
    let mut rt = Runtime::new_with_agent_id(worker_agent_id);

    rusty_js_runtime::agent_scheduler::AgentScheduler::global().register(rt.agent_handle());
    rt.install_intrinsics();
    let argv = vec!["cruft".to_string(), filename.clone(), filename.clone()];
    crate::install_cruft_host(&mut rt, argv);

    let port = new_object(&mut rt);
    install_emitter_methods_own(&mut rt, port);
    let tx_for_post = to_main.clone();
    let wake_for_post = parent_wake.clone();
    register_method(&mut rt, port, "postMessage", move |rt, args| {
        let msg = args.first().cloned().unwrap_or(Value::Undefined);
        let ir = send_ir_from_value(rt, &msg)?;
        let ok = tx_for_post.send(ir).is_ok();
        if ok {
            notify_agent_wake(&wake_for_post);
        }
        Ok(Value::Boolean(ok))
    });
    for noop in ["ref", "unref", "start"] {
        register_method(&mut rt, port, noop, |rt, _a| Ok(rt.current_this()));
    }

    let alive_for_close = alive.clone();
    register_method(&mut rt, port, "close", move |rt, _a| {
        alive_for_close.store(false, Ordering::SeqCst);
        Ok(rt.current_this())
    });
    let worker_data_value = match value_from_send_ir(&mut rt, &worker_data) {
        Ok(v) => v,
        Err(_) => Value::Undefined,
    };
    if !matches!(rt.global_get("worker_threads"), Value::Object(_)) {
        crate::node_stubs::install_worker_threads(&mut rt);
    }
    if let Value::Object(wt) = rt.global_get("worker_threads") {
        rt.object_set(wt, "parentPort".into(), Value::Object(port));
        rt.object_set(wt, "isMainThread".into(), Value::Boolean(false));
        rt.object_set(wt, "threadId".into(), Value::Number(1.0));
        rt.object_set(wt, "workerData".into(), worker_data_value);
    }

    let prior_poll = rt.host_hooks.poll_io.take();
    let rx = Arc::new(Mutex::new(from_main));
    let alive_for_poll = alive.clone();
    rt.install_host_hook(HostHook::PollIo(Box::new(move |rt| {
        if let Some(poll) = prior_poll.as_ref() {
            if poll(rt)? {
                return Ok(true);
            }
        }
        let mut fired = false;
        loop {
            let next = rx.lock().expect("worker inbox").try_recv();
            match next {
                Ok(ir) => {
                    let msg = value_from_send_ir(rt, &ir)?;
                    net_emit(rt, port, "message", vec![msg]);
                    fired = true;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    alive_for_poll.store(false, Ordering::SeqCst);
                    break;
                }
            }
        }
        if fired {
            return Ok(true);
        }
        if alive_for_poll.load(Ordering::SeqCst) {
            let next = rx
                .lock()
                .expect("worker inbox")
                .recv_timeout(Duration::from_millis(10));
            match next {
                Ok(ir) => {
                    let msg = value_from_send_ir(rt, &ir)?;
                    net_emit(rt, port, "message", vec![msg]);
                    return Ok(true);
                }
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    rt.io_wait_tick = true;
                    return Ok(true);
                }
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                    alive_for_poll.store(false, Ordering::SeqCst);
                }
            }
        }
        Ok(false)
    })));

    let path = std::fs::canonicalize(&filename)
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or(filename);
    match std::fs::read_to_string(&path) {
        Ok(src) => {
            let url = format!("file://{}", path);

            let goal_is_module = std::env::var_os("CRUFT_FORCE_SCRIPT").is_none()
                && (path.ends_with(".mjs")
                    || path.ends_with(".mts")
                    || std::env::var_os("CRUFT_FORCE_MODULE").is_some()
                    || matches!(
                        rusty_js_runtime::detect_module_kind(&url),
                        rusty_js_runtime::ModuleKind::ESM
                    ));
            let run_result = if goal_is_module {
                rt.pending_parse_goal = Some(true);
                rt.evaluate_module(&src, &url).map(|_| ())
            } else {
                rt.run_script(&src, &url).map(|_| ())
            };
            if let Err(e) = run_result {
                if to_main
                    .send(SendIr::Composite {
                        ref_id: 0,
                        is_array: false,
                        proto_null: false,
                        props: vec![
                            (
                                "status".to_string(),
                                SendIr::Str(rusty_js_runtime::send_ir::SendStr::Owned(
                                    "throw".to_string(),
                                )),
                            ),
                            (
                                "value".to_string(),
                                SendIr::Str(rusty_js_runtime::send_ir::SendStr::Owned(format!(
                                    "{:?}",
                                    e
                                ))),
                            ),
                        ],
                    })
                    .is_ok()
                {
                    notify_agent_wake(&parent_wake);
                }
            }

            {
                use rusty_js_runtime::agent_reactor::{
                    agent_control_source, host_completion_inbox_source, host_poll_io_source,
                    js_job_queue_source, AgentReactor,
                };
                let mut reactor = AgentReactor::new();
                reactor.register(js_job_queue_source());
                reactor.register(host_poll_io_source());
                reactor.register(host_completion_inbox_source());
                reactor.register(agent_control_source());
                let alive_for_loop = alive.clone();
                loop {
                    let progressed = match reactor.turn(&mut rt, Duration::from_millis(10)) {
                        Ok(p) => p,
                        Err(_) => break,
                    };
                    if rt.agent_terminate_requested() {
                        break;
                    }
                    if !alive_for_loop.load(Ordering::SeqCst) && !progressed {
                        break;
                    }
                }
            }
        }
        Err(e) => {
            if to_main
                .send(SendIr::Composite {
                    ref_id: 0,
                    is_array: false,
                    proto_null: false,
                    props: vec![
                        (
                            "status".to_string(),
                            SendIr::Str(rusty_js_runtime::send_ir::SendStr::Owned(
                                "throw".to_string(),
                            )),
                        ),
                        (
                            "value".to_string(),
                            SendIr::Str(rusty_js_runtime::send_ir::SendStr::Owned(format!(
                                "worker read: {e}"
                            ))),
                        ),
                    ],
                })
                .is_ok()
            {
                notify_agent_wake(&parent_wake);
            }
        }
    }
    alive.store(false, Ordering::SeqCst);

    rusty_js_runtime::agent_scheduler::AgentScheduler::global().deregister(worker_agent_id);
    notify_agent_wake(&parent_wake);
}

pub fn fork_child(
    rt: &mut Runtime,
    module: &str,
    args: &[String],
    cwd: Option<&str>,
    env: &[(String, String)],
) -> Result<ObjectRef, RuntimeError> {
    let listener = TcpListener::bind("127.0.0.1:0")
        .map_err(|e| RuntimeError::TypeError(format!("fork: bind: {e}")))?;
    let addr = listener
        .local_addr()
        .map_err(|e| RuntimeError::TypeError(format!("fork: addr: {e}")))?
        .to_string();
    let exe =
        std::env::current_exe().map_err(|e| RuntimeError::TypeError(format!("fork: exe: {e}")))?;
    let mut cmd = Command::new(exe);
    cmd.arg("run").arg(module);
    for a in args {
        cmd.arg(a);
    }
    if let Some(d) = cwd {
        cmd.current_dir(d);
    }
    for (key, value) in env {
        cmd.env(key, value);
    }
    cmd.env("CRUFT_CHANNEL_ADDR", &addr);
    cmd.env("CRUFT_NODE_FORK", "1");
    cmd.stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    let child = cmd
        .spawn()
        .map_err(|e| RuntimeError::TypeError(format!("fork: spawn: {e}")))?;
    let pid = child.id();

    let child_obj = new_object(rt);
    install_emitter(rt, child_obj);
    rt.object_set(child_obj, "pid".into(), Value::Number(pid as f64));
    let chan_id = attach_pending(rt, listener, child_obj, child);
    install_endpoint(rt, child_obj, chan_id);
    register_method(rt, child_obj, "kill", |rt, _a| {
        let this = match rt.current_this() {
            Value::Object(t) => t,
            _ => return Ok(Value::Boolean(false)),
        };
        if let Some(chan) = chan_of(rt, this) {
            CHANNELS.with(|v| {
                if let Some(Some(ch)) = v.borrow_mut().get_mut(chan) {
                    if let Some(c) = ch.child.as_mut() {
                        let _ = c.kill();
                    }
                }
            });
        }
        Ok(Value::Boolean(true))
    });
    Ok(child_obj)
}

pub fn report_worker_error(rt: &mut Runtime, message: &str) {
    if std::env::var("CRUFT_WORKER").is_err() {
        return;
    }
    let mut obj = Object::new_ordinary();
    obj.set_own(
        "__cruft_worker_error".into(),
        Value::String(Rc::new(JsString::from(message.to_string()))),
    );
    let val = Value::Object(rt.alloc_object(obj));
    let ids: Vec<usize> = CHANNELS.with(|v| (0..v.borrow().len()).collect());
    for id in ids {
        let _ = send_value(rt, id, &val);
    }

    std::thread::sleep(Duration::from_millis(30));
}

pub fn install_child_bootstrap(rt: &mut Runtime, process: ObjectRef) {
    let addr = match std::env::var("CRUFT_CHANNEL_ADDR") {
        Ok(a) => a,
        Err(_) => return,
    };
    let stream = match TcpStream::connect(&addr) {
        Ok(s) => s,
        Err(_) => return,
    };
    stream.set_nodelay(true).ok();
    if std::env::var("CRUFT_WORKER").is_ok() {

        let port = new_object(rt);
        install_emitter_methods_own(rt, port);
        let chan_id = attach(rt, stream, port, None);
        install_endpoint_named(rt, port, chan_id, "postMessage");
        let wd = std::env::var("CRUFT_WORKER_DATA").unwrap_or_else(|_| "null".to_string());
        let wd_val = json_parse(rt, &wd);

        rt.define_global_property("worker_threads", Value::Undefined);
        crate::node_stubs::install_worker_threads(rt);
        if let Value::Object(wt) = rt.global_get("worker_threads") {
            rt.object_set(wt, "parentPort".into(), Value::Object(port));
            rt.object_set(wt, "isMainThread".into(), Value::Boolean(false));
            rt.object_set(wt, "threadId".into(), Value::Number(1.0));
            rt.object_set(wt, "workerData".into(), wd_val);
        }
        return;
    }
    install_emitter_methods_own(rt, process);
    let chan_id = attach(rt, stream, process, None);
    install_endpoint(rt, process, chan_id);
}

pub fn harvest(rt: &mut Runtime) -> Result<bool, RuntimeError> {
    let agent_id = rt.agent_id();
    let work: Vec<(ObjectRef, usize, Vec<String>)> = CHANNELS.with(|v| {
        v.borrow()
            .iter()
            .filter_map(|s| {
                s.as_ref().and_then(|ch| {
                    if ch.agent_id != agent_id {
                        return None;
                    }
                    let lines = std::mem::take(&mut *ch.inbox.lock().unwrap());
                    Some((ch.target, ch.realm, lines))
                })
            })
            .collect()
    });
    let mut fired = false;
    for (target, realm, lines) in work {
        for line in lines {
            let prior = rt.enter_realm(realm);
            let msg = json_parse(rt, &line);

            let worker_err = match &msg {
                Value::Object(mid) => match rt.object_get(*mid, "__cruft_worker_error") {
                    Value::String(s) => Some(s.as_str().to_string()),
                    _ => None,
                },
                _ => None,
            };
            if let Some(emsg) = worker_err {
                let err = rt
                    .construct(
                        rt.global_get("Error"),
                        vec![Value::String(Rc::new(JsString::from(emsg)))],
                    )
                    .unwrap_or(Value::Undefined);

                let handled = net_emit(rt, target, "error", vec![err.clone()]);
                if !handled {
                    rt.exit_realm(prior);
                    return Err(RuntimeError::Thrown(err));
                }
            } else {
                net_emit(rt, target, "message", vec![msg]);
            }
            rt.exit_realm(prior);
            fired = true;
        }
    }
    if fired {
        return Ok(true);
    }
    let inproc_work: Vec<(ObjectRef, usize, Vec<SendIr>)> = INPROCESS_WORKERS.with(|v| {
        v.borrow()
            .iter()
            .filter_map(|slot| {
                let w = slot.as_ref()?;
                if w.agent_id != agent_id {
                    return None;
                }
                let mut items = Vec::new();
                loop {
                    let next = w
                        .from_worker
                        .lock()
                        .expect("inprocess worker inbox")
                        .try_recv();
                    match next {
                        Ok(ir) => items.push(ir),
                        Err(std::sync::mpsc::TryRecvError::Empty) => break,
                        Err(std::sync::mpsc::TryRecvError::Disconnected) => break,
                    }
                }
                Some((w.target, w.realm, items))
            })
            .collect()
    });
    let mut inproc_fired = false;
    for (target, realm, items) in inproc_work {
        for ir in items {
            let prior = rt.enter_realm(realm);
            let msg = value_from_send_ir(rt, &ir)?;
            net_emit(rt, target, "message", vec![msg]);
            rt.exit_realm(prior);
            inproc_fired = true;
        }
    }
    if inproc_fired {
        return Ok(true);
    }

    let exits: Vec<(ObjectRef, usize, i32)> = CHANNELS.with(|v| {
        let mut out = Vec::new();
        for ch in v.borrow_mut().iter_mut().flatten() {
            if ch.agent_id != agent_id {
                continue;
            }
            if ch.exit_done {
                continue;
            }
            if let Some(c) = ch.child.as_mut() {
                if let Ok(Some(status)) = c.try_wait() {
                    ch.exit_done = true;
                    ch.alive.store(false, Ordering::SeqCst);
                    out.push((ch.target, ch.realm, status.code().unwrap_or(0)));
                }
            }
        }
        out
    });
    let had_exits = !exits.is_empty();
    for (target, realm, code) in exits {
        let prior = rt.enter_realm(realm);
        rt.object_set(target, "connected".into(), Value::Boolean(false));

        let code = if matches!(
            rt.object_get(target, "__worker_terminated"),
            Value::Boolean(true)
        ) {
            1
        } else {
            code
        };
        net_emit(
            rt,
            target,
            "exit",
            vec![Value::Number(code as f64), Value::Null],
        );
        net_emit(
            rt,
            target,
            "close",
            vec![Value::Number(code as f64), Value::Null],
        );
        rt.exit_realm(prior);
    }
    let inproc_exits: Vec<(ObjectRef, usize)> = INPROCESS_WORKERS.with(|v| {
        let mut out = Vec::new();
        for w in v.borrow_mut().iter_mut().flatten() {
            if w.agent_id != agent_id {
                continue;
            }
            if !w.exit_done && !w.alive.load(Ordering::SeqCst) {
                w.exit_done = true;
                out.push((w.target, w.realm));
            }
        }
        out
    });
    let had_inproc_exits = !inproc_exits.is_empty();
    for (target, realm) in inproc_exits {
        let prior = rt.enter_realm(realm);
        rt.object_set(target, "connected".into(), Value::Boolean(false));

        let code = if matches!(
            rt.object_get(target, "__worker_terminated"),
            Value::Boolean(true)
        ) {
            1.0
        } else {
            0.0
        };
        net_emit(rt, target, "exit", vec![Value::Number(code), Value::Null]);
        net_emit(rt, target, "close", vec![Value::Number(code), Value::Null]);
        rt.exit_realm(prior);
    }
    if had_exits || had_inproc_exits {
        return Ok(true);
    }
    Ok(false)
}

pub fn has_open(rt: &Runtime) -> bool {
    has_open_process_channels(rt) || has_open_inprocess_workers(rt)
}

fn has_open_process_channels(rt: &Runtime) -> bool {
    let agent_id = rt.agent_id();
    CHANNELS.with(|v| {
        v.borrow().iter().any(|s| {
            s.as_ref().is_some_and(|ch| {
                if ch.agent_id != agent_id {
                    return false;
                }

                ch.alive.load(Ordering::SeqCst)
                    || (!ch.exit_done && ch.child.is_some())
                    || ch.inbox.lock().map(|q| !q.is_empty()).unwrap_or(false)
            })
        })
    })
}

pub fn has_open_inprocess_workers(rt: &Runtime) -> bool {
    let agent_id = rt.agent_id();
    INPROCESS_WORKERS.with(|v| {
        v.borrow().iter().any(|s| {
            s.as_ref().is_some_and(|w| {
                debug_assert_ne!(w.worker_agent_id, AgentId::DEFAULT);

                w.agent_id == agent_id && (w.alive.load(Ordering::SeqCst) || !w.exit_done)
            })
        })
    })
}

pub fn poll_io(rt: &mut Runtime) -> Result<bool, RuntimeError> {
    if has_open_process_channels(rt) || has_open_inprocess_workers(rt) {
        let observed = rt.agent_wake_generation();
        if harvest(rt)? {
            return Ok(true);
        }
        let _ = rt.wait_agent_wake_timeout(observed, Duration::from_secs(1));
        return Ok(true);
    }
    Ok(false)
}

fn port_post_message_transfer_type_error(rt: &mut Runtime) -> RuntimeError {
    let msg = "Optional transferList argument must be an iterable";
    match rusty_js_runtime::intrinsics::make_error_instance(rt, "TypeError", msg) {
        Some(id) => {
            rt.object_set(
                id,
                "code".into(),
                Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                    "ERR_INVALID_ARG_TYPE",
                ))),
            );
            RuntimeError::Thrown(Value::Object(id))
        }
        None => RuntimeError::TypeError(msg.to_string()),
    }
}

fn post_message_clone_with_optional_transfer(
    rt: &mut Runtime,
    msg: &Value,
    transfer_list: Option<Value>,
) -> Result<Value, RuntimeError> {

    let forward_transfer = match &transfer_list {
        None | Some(Value::Undefined) | Some(Value::Null) => None,
        Some(Value::Object(id)) => {
            let iterable = matches!(
                rt.lookup_well_known_method(*id, "@@iterator"),
                Ok(ref m) if rt.is_callable(m)
            );
            iterable.then(|| transfer_list.clone().unwrap())
        }
        Some(_) => {
            return Err(port_post_message_transfer_type_error(rt));
        }
    };
    let sc = rt.global_get("structuredClone");
    if rt.is_callable(&sc) {
        let mut sc_args = vec![msg.clone()];
        if let Some(list) = forward_transfer {
            let opts = new_object(rt);
            rt.object_set(opts, "transfer".into(), list);
            sc_args.push(Value::Object(opts));
        }
        rt.call_function(sc, Value::Undefined, sc_args)
    } else {
        let json = json_stringify(rt, msg);
        Ok(json_parse(rt, &json))
    }
}

fn make_port(rt: &mut Runtime) -> ObjectRef {
    let port = new_object(rt);
    rt.object_set(port, "__worker_message_port__".into(), Value::Boolean(true));
    install_emitter(rt, port);
    register_method(rt, port, "postMessage", |rt, args| {
        if let Some(Value::Object(list_id)) = args.get(1) {
            for key in rt.ordinary_own_enumerable_string_keys(*list_id) {
                let item = rt.object_get(*list_id, &key);
                if let Value::Object(item_id) = item {
                    if rt
                        .array_buffers
                        .get(&item_id)
                        .map(|buf| buf.untransferable)
                        .unwrap_or(false)
                    {
                        return Err(data_clone_error(
                            rt,
                            "MessagePort.postMessage: ArrayBuffer is marked as untransferable",
                        ));
                    }
                }
            }
        }
        let this = match rt.current_this() {
            Value::Object(t) => t,
            _ => return Ok(Value::Undefined),
        };
        let peer = match rt.object_get(this, "__peer") {
            Value::Object(p) => p,
            _ => return Ok(Value::Undefined),
        };

        let msg = args.first().cloned().unwrap_or(Value::Undefined);
        let cloned = match &msg {
            Value::Object(id) if rt.obj(*id).has_own_str("__block_list") => msg.clone(),

            _ => post_message_clone_with_optional_transfer(rt, &msg, args.get(1).cloned())?,
        };

        let queue = match rt.object_get(peer, "__msg_queue") {
            Value::Object(q) => q,
            _ => {
                let q = new_object(rt);
                rt.object_set(q, "length".into(), Value::Number(0.0));
                rt.object_set(peer, "__msg_queue".into(), Value::Object(q));
                q
            }
        };
        let qlen = rt.array_length(queue);
        rt.object_set(queue, qlen.to_string(), cloned);
        rt.object_set(queue, "length".into(), Value::Number((qlen + 1) as f64));
        rt.enqueue_host_phase_rooted(
            HostEnqueuePhase::MessageDeliveryMacrotask,
            "MessagePort message",
            vec![peer],
            move |rt| {

                let Value::Object(q) = rt.object_get(peer, "__msg_queue") else {
                    return Ok(());
                };
                let len = rt.array_length(q);
                if len == 0 {
                    return Ok(());
                }
                let msg = rt.object_get(q, "0");
                for i in 1..len {
                    let v = rt.object_get(q, &i.to_string());
                    rt.object_set(q, (i - 1).to_string(), v);
                }
                rt.object_set(q, "length".into(), Value::Number((len - 1) as f64));
                net_emit(rt, peer, "message", vec![msg]);
                Ok(())
            },
        );
        Ok(Value::Undefined)
    });
    for noop in ["start", "close", "ref", "unref"] {
        register_method(rt, port, noop, |rt, _a| Ok(rt.current_this()));
    }
    port
}

pub fn make_message_channel(rt: &mut Runtime) -> ObjectRef {
    let chan = new_object(rt);
    rt.object_set(
        chan,
        "__worker_message_channel__".into(),
        Value::Boolean(true),
    );
    let port1 = make_port(rt);
    let port2 = make_port(rt);
    rt.set_engine_sentinel(port1, "__peer", Value::Object(port2));
    rt.set_engine_sentinel(port2, "__peer", Value::Object(port1));
    rt.object_set(chan, "port1".into(), Value::Object(port1));
    rt.object_set(chan, "port2".into(), Value::Object(port2));
    chan
}

#[cfg(test)]
mod tests {
    use super::{
        harvest, notify_agent_wake, poll_io, send_ir_from_value, spawn_reader, value_from_send_ir,
        InProcessWorker, IpcChannel, CHANNELS, INPROCESS_WORKERS,
    };
    use rusty_js_runtime::{send_ir::SendIr, AgentId, Object, Runtime, Value};
    use std::collections::VecDeque;
    use std::io::Write;
    use std::net::{TcpListener, TcpStream};
    use std::sync::atomic::AtomicBool;
    use std::sync::{mpsc, Arc, Mutex};
    use std::time::{Duration, Instant};

    static HEAVY_WORKER_TEST_LOCK: Mutex<()> = Mutex::new(());
    fn heavy_worker_test_guard() -> std::sync::MutexGuard<'static, ()> {
        HEAVY_WORKER_TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn pump_until<F: Fn(&mut Runtime) -> bool>(
        rt: &mut Runtime,
        timeout: Duration,
        done: F,
    ) -> bool {
        let deadline = Instant::now() + timeout;
        loop {
            let observed = rt.agent_wake_generation();
            let _ = super::poll_io(rt).expect("poll in-process worker");
            if done(rt) {
                return true;
            }
            if Instant::now() >= deadline {
                return false;
            }
            let _ = rt.wait_agent_wake_timeout(observed, Duration::from_millis(5));
        }
    }

    #[test]
    fn cross_agent_structured_clone_transfers_between_two_live_agents() {
        use rusty_js_runtime::agent_scheduler::AgentScheduler;
        use rusty_js_runtime::send_ir::SendIrDisposition;
        use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
        let _serialize = heavy_worker_test_guard();

        let sched = AgentScheduler::global();
        let id_b = AgentId::from_raw(561);
        let got_x = Arc::new(AtomicI64::new(-1));
        let got_len = Arc::new(AtomicI64::new(-1));
        let got_first = Arc::new(AtomicI64::new(-1));
        let got_str_ok = Arc::new(AtomicBool::new(false));
        let done = Arc::new(AtomicBool::new(false));

        let done_b = done.clone();
        let tb = std::thread::spawn(move || {
            let mut rt_b = Runtime::new_with_agent_id(id_b);
            rt_b.install_intrinsics();
            assert!(AgentScheduler::global().register(rt_b.agent_handle()));
            rt_b.run_host_completions_until(
                |_rt| done_b.load(Ordering::SeqCst),
                Duration::from_secs(30),
            );
            AgentScheduler::global().deregister(id_b);
        });

        let deadline = Instant::now() + Duration::from_secs(5);
        while !sched.is_registered(id_b) && Instant::now() < deadline {
            std::thread::yield_now();
        }
        assert!(sched.is_registered(id_b), "agent B must come up");

        let mut rt_a = Runtime::new_with_agent_id(AgentId::from_raw(560));
        rt_a.install_intrinsics();
        rt_a.run_script(
            "globalThis.v = { x: 42, arr: [10, 20, 30], s: \"hi\" };",
            "file://cross-agent-clone-a",
        )
        .expect("build value on A");
        let ga = rt_a.global_object.unwrap();
        let v = rt_a.object_get(ga, "v");
        let ir = send_ir_from_value(&rt_a, &v).expect("lower A value to SendIr");
        assert_eq!(
            ir.disposition(),
            SendIrDisposition::Clone,
            "a plain object is a clone-disposition transfer"
        );

        let (gx, gl, gf, gs, dn) = (
            got_x.clone(),
            got_len.clone(),
            got_first.clone(),
            got_str_ok.clone(),
            done.clone(),
        );
        assert!(sched.post_completion(
            id_b,
            Box::new(move |rt_b: &mut Runtime| {
                let val = value_from_send_ir(rt_b, &ir).expect("rematerialize on B");
                if let Value::Object(o) = val {
                    if let Value::Number(n) = rt_b.object_get(o, "x") {
                        gx.store(n as i64, Ordering::SeqCst);
                    }
                    if let Value::Object(arr) = rt_b.object_get(o, "arr") {
                        if let Value::Number(n) = rt_b.object_get(arr, "length") {
                            gl.store(n as i64, Ordering::SeqCst);
                        }
                        if let Value::Number(n) = rt_b.object_get(arr, "0") {
                            gf.store(n as i64, Ordering::SeqCst);
                        }
                    }
                    if let Value::String(s) = rt_b.object_get(o, "s") {
                        gs.store(s.as_str() == "hi", Ordering::SeqCst);
                    }
                }
                dn.store(true, Ordering::SeqCst);
            }),
        ));

        tb.join().unwrap();

        assert_eq!(got_x.load(Ordering::SeqCst), 42, "B must see x=42");
        assert_eq!(got_len.load(Ordering::SeqCst), 3, "B must see arr.length=3");
        assert_eq!(got_first.load(Ordering::SeqCst), 10, "B must see arr[0]=10");
        assert!(got_str_ok.load(Ordering::SeqCst), "B must see s=\"hi\"");

        assert!(matches!(rt_a.object_get(ga, "v"), Value::Object(_)));
        assert!(matches!(
            {
                let ga = rt_a.global_object.unwrap();
                if let Value::Object(o) = rt_a.object_get(ga, "v") {
                    rt_a.object_get(o, "x")
                } else {
                    Value::Undefined
                }
            },
            Value::Number(n) if n == 42.0
        ));
    }

    #[test]
    fn cross_agent_transfer_arraybuffer_between_two_live_agents() {
        use rusty_js_runtime::agent_scheduler::AgentScheduler;
        use rusty_js_runtime::send_ir::SendIrDisposition;
        use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
        let _serialize = heavy_worker_test_guard();

        let sched = AgentScheduler::global();
        let id_b = AgentId::from_raw(563);
        let got_len = Arc::new(AtomicI64::new(-1));
        let got_first = Arc::new(AtomicI64::new(-1));
        let done = Arc::new(AtomicBool::new(false));

        let done_b = done.clone();
        let tb = std::thread::spawn(move || {
            let mut rt_b = Runtime::new_with_agent_id(id_b);
            rt_b.install_intrinsics();
            assert!(AgentScheduler::global().register(rt_b.agent_handle()));
            rt_b.run_host_completions_until(
                |_rt| done_b.load(Ordering::SeqCst),
                Duration::from_secs(30),
            );
            AgentScheduler::global().deregister(id_b);
        });

        let deadline = Instant::now() + Duration::from_secs(5);
        while !sched.is_registered(id_b) && Instant::now() < deadline {
            std::thread::yield_now();
        }
        assert!(sched.is_registered(id_b), "agent B must come up");

        let mut rt_a = Runtime::new_with_agent_id(AgentId::from_raw(562));
        rt_a.install_intrinsics();
        rt_a.run_script(
            "globalThis.ab = new ArrayBuffer(4); new Uint8Array(globalThis.ab)[0] = 42;",
            "file://cross-agent-transfer-a",
        )
        .expect("build ArrayBuffer on A");
        let ga = rt_a.global_object.unwrap();
        let ab = rt_a.object_get(ga, "ab");
        let ir = send_ir_from_value(&rt_a, &ab).expect("lower A ArrayBuffer to SendIr");
        assert_eq!(
            ir.disposition(),
            SendIrDisposition::Transfer,
            "an ArrayBuffer is a transfer-disposition value"
        );

        let (gl, gf, dn) = (got_len.clone(), got_first.clone(), done.clone());
        assert!(sched.post_completion(
            id_b,
            Box::new(move |rt_b: &mut Runtime| {
                let val = value_from_send_ir(rt_b, &ir).expect("rematerialize AB on B");
                let gb = rt_b.global_object.unwrap();
                rt_b.object_set(gb, "__ab".into(), val);

                let _ = rt_b.run_script(
                    "globalThis.__len = globalThis.__ab.byteLength; \
                     globalThis.__first = new Uint8Array(globalThis.__ab)[0];",
                    "file://cross-agent-transfer-b",
                );
                if let Value::Number(n) = rt_b.object_get(gb, "__len") {
                    gl.store(n as i64, Ordering::SeqCst);
                }
                if let Value::Number(n) = rt_b.object_get(gb, "__first") {
                    gf.store(n as i64, Ordering::SeqCst);
                }
                dn.store(true, Ordering::SeqCst);
            }),
        ));

        tb.join().unwrap();
        assert_eq!(got_len.load(Ordering::SeqCst), 4, "B must see byteLength=4");
        assert_eq!(
            got_first.load(Ordering::SeqCst),
            42,
            "B must see byte[0]=42"
        );
    }

    #[test]
    fn cross_agent_shared_backing_visible_across_two_live_agents() {
        use rusty_js_runtime::agent_scheduler::AgentScheduler;
        use rusty_js_runtime::send_ir::SendIrDisposition;
        use std::sync::atomic::{AtomicBool, Ordering};
        let _serialize = heavy_worker_test_guard();

        let sched = AgentScheduler::global();
        let id_b = AgentId::from_raw(565);
        let done = Arc::new(AtomicBool::new(false));

        let done_b = done.clone();
        let tb = std::thread::spawn(move || {
            let mut rt_b = Runtime::new_with_agent_id(id_b);
            rt_b.install_intrinsics();
            assert!(AgentScheduler::global().register(rt_b.agent_handle()));
            rt_b.run_host_completions_until(
                |_rt| done_b.load(Ordering::SeqCst),
                Duration::from_secs(30),
            );
            AgentScheduler::global().deregister(id_b);
        });

        let deadline = Instant::now() + Duration::from_secs(5);
        while !sched.is_registered(id_b) && Instant::now() < deadline {
            std::thread::yield_now();
        }
        assert!(sched.is_registered(id_b), "agent B must come up");

        let mut rt_a = Runtime::new_with_agent_id(AgentId::from_raw(564));
        rt_a.install_intrinsics();
        rt_a.run_script(
            "globalThis.sab = new SharedArrayBuffer(8); \
             globalThis.av = new Uint8Array(globalThis.sab); globalThis.av[0] = 0;",
            "file://cross-agent-shared-a",
        )
        .expect("build SAB on A");
        let ga = rt_a.global_object.unwrap();
        let sab = rt_a.object_get(ga, "sab");
        let ir = send_ir_from_value(&rt_a, &sab).expect("lower A SAB to SendIr");
        assert_eq!(
            ir.disposition(),
            SendIrDisposition::SharedBacking,
            "a SharedArrayBuffer is a shared-backing value"
        );

        let dn = done.clone();
        assert!(sched.post_completion(
            id_b,
            Box::new(move |rt_b: &mut Runtime| {
                let val = value_from_send_ir(rt_b, &ir).expect("rematerialize SAB on B");
                let gb = rt_b.global_object.unwrap();
                rt_b.object_set(gb, "__sab".into(), val);

                let _ = rt_b.run_script(
                    "new Uint8Array(globalThis.__sab)[0] = 99;",
                    "file://cross-agent-shared-b",
                );
                dn.store(true, Ordering::SeqCst);
            }),
        ));

        tb.join().unwrap();

        rt_a.run_script(
            "globalThis.observed = globalThis.av[0];",
            "file://cross-agent-shared-a-read",
        )
        .expect("read SAB on A");
        assert!(
            matches!(rt_a.object_get(ga, "observed"), Value::Number(n) if n == 99.0),
            "agent A must observe the byte agent B wrote through the shared backing"
        );
    }

    #[test]
    fn cross_agent_shared_backing_survives_source_gc_when_target_retains_it() {
        std::thread::Builder::new()
            .name("cross-agent-sab-retention-test".into())
            .stack_size(16 * 1024 * 1024)
            .spawn(|| {
                use rusty_js_runtime::agent_scheduler::AgentScheduler;
                use rusty_js_runtime::send_ir::{SendIr, SendIrDisposition};
                use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
                let _serialize = heavy_worker_test_guard();

                let sched = AgentScheduler::global();
                let id_b = AgentId::from_raw(566);
                let rematerialized = Arc::new(AtomicBool::new(false));
                let release_b = Arc::new(AtomicBool::new(false));
                let same_backing = Arc::new(AtomicBool::new(false));
                let b_observed = Arc::new(AtomicI64::new(-1));

                let release_for_b = release_b.clone();
                let tb = std::thread::Builder::new()
                    .name("cross-agent-sab-retention-b".into())
                    .stack_size(16 * 1024 * 1024)
                    .spawn(move || {
                        let mut rt_b = Runtime::new_with_agent_id(id_b);
                        rt_b.install_intrinsics();
                        assert!(AgentScheduler::global().register(rt_b.agent_handle()));
                        rt_b.run_host_completions_until(
                            |_rt| release_for_b.load(Ordering::SeqCst),
                            Duration::from_secs(30),
                        );
                        AgentScheduler::global().deregister(id_b);
                    })
                    .expect("spawn B SAB-retention agent");

                let deadline = Instant::now() + Duration::from_secs(5);
                while !sched.is_registered(id_b) && Instant::now() < deadline {
                    std::thread::yield_now();
                }
                assert!(sched.is_registered(id_b), "agent B must come up");

                let mut rt_a = Runtime::new_with_agent_id(AgentId::from_raw(567));
                rt_a.install_intrinsics();
                rt_a.run_script(
                    "globalThis.sab = new SharedArrayBuffer(4); \
             globalThis.av = new Uint8Array(globalThis.sab); globalThis.av[0] = 17;",
                    "file://cross-agent-sab-retention-a-init",
                )
                .expect("build source SAB on A");
                let ga = rt_a.global_object.unwrap();
                let sab = rt_a.object_get(ga, "sab");
                let ir = send_ir_from_value(&rt_a, &sab).expect("lower source SAB");
                assert_eq!(
                    ir.disposition(),
                    SendIrDisposition::SharedBacking,
                    "a SharedArrayBuffer is a shared-backing value"
                );
                let arc_a = match &ir {
                    SendIr::SharedArrayBuffer { shared, .. } => shared.clone(),
                    _ => panic!("expected a SharedArrayBuffer SendIr"),
                };
                let weak = Arc::downgrade(&arc_a);

                let (remat_b, same_b, ir_b, arc_a_for_b) = (
                    rematerialized.clone(),
                    same_backing.clone(),
                    ir,
                    arc_a.clone(),
                );
                assert!(sched.post_completion(
                    id_b,
                    Box::new(move |rt_b: &mut Runtime| {
                        let val = value_from_send_ir(rt_b, &ir_b).expect("rematerialize SAB on B");
                        let ir_reb = send_ir_from_value(rt_b, &val).expect("re-lower SAB on B");
                        let arc_b = match &ir_reb {
                            SendIr::SharedArrayBuffer { shared, .. } => shared.clone(),
                            _ => panic!("expected a SharedArrayBuffer SendIr"),
                        };
                        same_b.store(Arc::ptr_eq(&arc_b, &arc_a_for_b), Ordering::SeqCst);
                        let gb = rt_b.global_object.unwrap();
                        rt_b.object_set(gb, "__sab".into(), val);
                        remat_b.store(true, Ordering::SeqCst);
                    }),
                ));

                let remat_deadline = Instant::now() + Duration::from_secs(5);
                while !rematerialized.load(Ordering::SeqCst) && Instant::now() < remat_deadline {
                    std::thread::yield_now();
                }
                assert!(
                    rematerialized.load(Ordering::SeqCst),
                    "agent B must rematerialize and root the SAB"
                );
                assert!(
                    same_backing.load(Ordering::SeqCst),
                    "B's rematerialized SAB must share A's exact backing"
                );

                drop(arc_a);
                rt_a.run_script(
                    "globalThis.sab = null; globalThis.av = null;",
                    "file://cross-agent-sab-retention-a-drop-roots",
                )
                .expect("drop source SAB roots");
                let _ = rt_a.collect();
                drop(rt_a);

                let retained = weak
                    .upgrade()
                    .expect("B's rooted SAB must retain backing after A GC/drop");
                {
                    let bytes = retained.lock().expect("retained shared backing lock");
                    assert_eq!(bytes[0], 17, "retained backing preserves source byte");
                }
                drop(retained);

                let (obs_b, rel_b) = (b_observed.clone(), release_b.clone());
                assert!(sched.post_completion(
                    id_b,
                    Box::new(move |rt_b: &mut Runtime| {
                        let gb = rt_b.global_object.unwrap();
                        let _ = rt_b.run_script(
                            "globalThis.__observed = new Uint8Array(globalThis.__sab)[0];",
                            "file://cross-agent-sab-retention-b-read",
                        );
                        if let Value::Number(n) = rt_b.object_get(gb, "__observed") {
                            obs_b.store(n as i64, Ordering::SeqCst);
                        }
                        rel_b.store(true, Ordering::SeqCst);
                    }),
                ));

                tb.join().unwrap();
                assert_eq!(
                    b_observed.load(Ordering::SeqCst),
                    17,
                    "agent B must keep reading the SAB after agent A GC/drop"
                );
            })
            .expect("spawn large-stack SAB retention test thread")
            .join()
            .expect("large-stack SAB retention test thread");
    }

    #[test]
    fn cross_agent_atomics_wait_notify_over_shared_backing() {
        use rusty_js_runtime::agent262::{atomics_notify_waiters, atomics_park, WaitResult};
        use rusty_js_runtime::send_ir::SendIr;
        use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
        let _serialize = heavy_worker_test_guard();

        let mut rt_a = Runtime::new_with_agent_id(AgentId::from_raw(566));
        rt_a.install_intrinsics();
        rt_a.run_script(
            "globalThis.sab = new SharedArrayBuffer(8);",
            "file://cross-agent-atomics-a",
        )
        .expect("build SAB on A");
        let ga = rt_a.global_object.unwrap();
        let sab = rt_a.object_get(ga, "sab");
        let ir = send_ir_from_value(&rt_a, &sab).expect("lower SAB on A");
        let arc_a = match &ir {
            SendIr::SharedArrayBuffer { shared, .. } => shared.clone(),
            _ => panic!("expected a SharedArrayBuffer SendIr"),
        };

        let wait_code = Arc::new(AtomicI64::new(-1));
        let same_backing = Arc::new(AtomicBool::new(false));
        let (wc, sb, ir_b, arc_a_for_b) = (
            wait_code.clone(),
            same_backing.clone(),
            ir.clone(),
            arc_a.clone(),
        );

        let tb = std::thread::spawn(move || {
            let mut rt_b = Runtime::new_with_agent_id(AgentId::from_raw(567));
            rt_b.install_intrinsics();

            let remat = value_from_send_ir(&mut rt_b, &ir_b).expect("remat SAB on B");
            let ir_reb = send_ir_from_value(&rt_b, &remat).expect("re-lower SAB on B");
            let arc_b = match &ir_reb {
                SendIr::SharedArrayBuffer { shared, .. } => shared.clone(),
                _ => panic!("expected a SharedArrayBuffer SendIr"),
            };
            sb.store(Arc::ptr_eq(&arc_b, &arc_a_for_b), Ordering::SeqCst);

            let res = atomics_park(&arc_b, 0, &[0, 0, 0, 0], 30_000.0);
            wc.store(
                match res {
                    WaitResult::Ok => 0,
                    WaitResult::NotEqual => 1,
                    WaitResult::TimedOut => 2,
                },
                Ordering::SeqCst,
            );
        });

        let deadline = Instant::now() + Duration::from_secs(10);
        let mut woke = 0;
        while woke == 0 && Instant::now() < deadline {
            woke = atomics_notify_waiters(&arc_a, 0, 1);
            if woke == 0 {
                std::thread::yield_now();
            }
        }

        tb.join().unwrap();
        assert!(
            same_backing.load(Ordering::SeqCst),
            "B's rematerialized SAB must share A's exact backing"
        );
        assert_eq!(
            woke, 1,
            "A's notify must wake exactly one cross-agent waiter"
        );
        assert_eq!(
            wait_code.load(Ordering::SeqCst),
            0,
            "B's Atomics.wait must return Ok (woken by A's notify, not timed out)"
        );
    }

    #[test]
    fn cross_agent_atomics_waitasync_notify_over_shared_backing() {
        std::thread::Builder::new()
            .name("cross-agent-waitasync-test".into())
            .stack_size(16 * 1024 * 1024)
            .spawn(|| {
                use rusty_js_runtime::agent262::atomics_notify_waiters;
                use rusty_js_runtime::send_ir::SendIr;
                use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
                let _serialize = heavy_worker_test_guard();

                let mut rt_a = Runtime::new_with_agent_id(AgentId::from_raw(568));
                rt_a.install_intrinsics();
                rt_a.run_script(
                    "globalThis.sab = new SharedArrayBuffer(8);",
                    "file://cross-agent-waitasync-a",
                )
                .expect("build SAB on A");
                let ga = rt_a.global_object.unwrap();
                let sab = rt_a.object_get(ga, "sab");
                let ir = send_ir_from_value(&rt_a, &sab).expect("lower SAB on A");
                let arc_a = match &ir {
                    SendIr::SharedArrayBuffer { shared, .. } => shared.clone(),
                    _ => panic!("expected a SharedArrayBuffer SendIr"),
                };

                let registered = Arc::new(AtomicBool::new(false));
                let done = Arc::new(AtomicBool::new(false));
                let status_code = Arc::new(AtomicI64::new(-1));
                let same_backing = Arc::new(AtomicBool::new(false));
                let (reg_b, done_b, status_b, same_b, ir_b, arc_a_for_b) = (
                    registered.clone(),
                    done.clone(),
                    status_code.clone(),
                    same_backing.clone(),
                    ir.clone(),
                    arc_a.clone(),
                );

                let tb = std::thread::Builder::new()
                    .name("cross-agent-waitasync-b".into())
                    .stack_size(16 * 1024 * 1024)
                    .spawn(move || {
                        let mut rt_b = Runtime::new_with_agent_id(AgentId::from_raw(569));
                        rt_b.install_intrinsics();
                        let remat = value_from_send_ir(&mut rt_b, &ir_b).expect("remat SAB on B");
                        let ir_reb = send_ir_from_value(&rt_b, &remat).expect("re-lower SAB on B");
                        let arc_b = match &ir_reb {
                            SendIr::SharedArrayBuffer { shared, .. } => shared.clone(),
                            _ => panic!("expected a SharedArrayBuffer SendIr"),
                        };
                        same_b.store(Arc::ptr_eq(&arc_b, &arc_a_for_b), Ordering::SeqCst);
                        let gb = rt_b.global_object.unwrap();
                        rt_b.object_set(gb, "__sab".into(), remat);
                        rt_b.run_script(
                            "globalThis.waitAsyncResult = 'pending'; \
                     var ta = new Int32Array(globalThis.__sab); \
                     var waiter = Atomics.waitAsync(ta, 0, 0, 30000); \
                     if (!waiter.async) { globalThis.waitAsyncResult = waiter.value; } \
                     else waiter.value.then(function(r) { globalThis.waitAsyncResult = r; });",
                            "file://cross-agent-waitasync-b",
                        )
                        .expect("register waitAsync on B");
                        reg_b.store(true, Ordering::SeqCst);

                        let deadline = Instant::now() + Duration::from_secs(10);
                        while Instant::now() < deadline {
                            let _ = rusty_js_runtime::job_queue::pump_one_tick(&mut rt_b)
                                .expect("pump B waitAsync poll");
                            let result = rt_b.object_get(gb, "waitAsyncResult");
                            match result {
                                Value::String(s) if s.as_str() == "ok" => {
                                    status_b.store(0, Ordering::SeqCst);
                                    done_b.store(true, Ordering::SeqCst);
                                    return;
                                }
                                Value::String(s) if s.as_str() == "timed-out" => {
                                    status_b.store(2, Ordering::SeqCst);
                                    done_b.store(true, Ordering::SeqCst);
                                    return;
                                }
                                Value::String(s) if s.as_str() == "not-equal" => {
                                    status_b.store(1, Ordering::SeqCst);
                                    done_b.store(true, Ordering::SeqCst);
                                    return;
                                }
                                _ => std::thread::yield_now(),
                            }
                        }
                        status_b.store(3, Ordering::SeqCst);
                        done_b.store(true, Ordering::SeqCst);
                    })
                    .expect("spawn B waitAsync agent");

                let registration_deadline = Instant::now() + Duration::from_secs(5);
                while !registered.load(Ordering::SeqCst) && Instant::now() < registration_deadline {
                    std::thread::yield_now();
                }
                assert!(
                    registered.load(Ordering::SeqCst),
                    "agent B must register its waitAsync waiter"
                );

                let notify_deadline = Instant::now() + Duration::from_secs(10);
                let mut woke = 0;
                while woke == 0 && Instant::now() < notify_deadline {
                    woke = atomics_notify_waiters(&arc_a, 0, 1);
                    if woke == 0 {
                        std::thread::yield_now();
                    }
                }

                tb.join().unwrap();
                assert!(
                    same_backing.load(Ordering::SeqCst),
                    "B's waitAsync SAB must share A's exact backing"
                );
                assert_eq!(
                    woke, 1,
                    "A's notify must wake exactly one cross-agent async waiter"
                );
                assert_eq!(
                    status_code.load(Ordering::SeqCst),
                    0,
                    "B's waitAsync promise must settle to ok"
                );
                assert!(done.load(Ordering::SeqCst));
            })
            .expect("spawn large-stack waitAsync test thread")
            .join()
            .expect("large-stack waitAsync test thread");
    }

    #[test]
    fn per_agent_gc_isolation_across_two_live_agents() {
        use rusty_js_runtime::agent_scheduler::AgentScheduler;
        use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
        let _serialize = heavy_worker_test_guard();

        let sched = AgentScheduler::global();
        let id_b = AgentId::from_raw(568);
        let b_keep = Arc::new(AtomicI64::new(-1));
        let done = Arc::new(AtomicBool::new(false));

        let done_b = done.clone();
        let tb = std::thread::spawn(move || {
            let mut rt_b = Runtime::new_with_agent_id(id_b);
            rt_b.install_intrinsics();
            assert!(AgentScheduler::global().register(rt_b.agent_handle()));

            rt_b.run_script("globalThis.keep = { v: 7 };", "file://per-agent-gc-b-init")
                .expect("B init");
            rt_b.run_host_completions_until(
                |_rt| done_b.load(Ordering::SeqCst),
                Duration::from_secs(30),
            );
            AgentScheduler::global().deregister(id_b);
        });

        let deadline = Instant::now() + Duration::from_secs(5);
        while !sched.is_registered(id_b) && Instant::now() < deadline {
            std::thread::yield_now();
        }
        assert!(sched.is_registered(id_b), "agent B must come up");

        let mut rt_a = Runtime::new_with_agent_id(AgentId::from_raw(569));
        rt_a.install_intrinsics();
        rt_a.run_script(
            "globalThis.keep = { v: 42 }; \
             globalThis.sink = 0; \
             for (let i = 0; i < 500; i++) { const g = { a: i, b: [i, i] }; globalThis.sink ^= g.a; }",
            "file://per-agent-gc-a-init",
        )
        .expect("A init");
        let reclaimed = rt_a.collect();

        let ga = rt_a.global_object.unwrap();
        assert!(
            matches!(
                {
                    if let Value::Object(o) = rt_a.object_get(ga, "keep") {
                        rt_a.object_get(o, "v")
                    } else {
                        Value::Undefined
                    }
                },
                Value::Number(n) if n == 42.0
            ),
            "A's rooted object must survive A's own GC (reclaimed {reclaimed} objects)"
        );

        let (bk, dn) = (b_keep.clone(), done.clone());
        assert!(sched.post_completion(
            id_b,
            Box::new(move |rt_b: &mut Runtime| {
                let gb = rt_b.global_object.unwrap();
                let v = if let Value::Object(o) = rt_b.object_get(gb, "keep") {
                    rt_b.object_get(o, "v")
                } else {
                    Value::Undefined
                };
                if let Value::Number(n) = v {
                    bk.store(n as i64, Ordering::SeqCst);
                }
                dn.store(true, Ordering::SeqCst);
            }),
        ));

        tb.join().unwrap();
        assert_eq!(
            b_keep.load(Ordering::SeqCst),
            7,
            "agent B's heap must be intact after agent A's independent GC"
        );
    }

    #[test]
    fn global_web_worker_projection_and_lazy_module_property_materialization() {

        let mut rt = Runtime::new_with_agent_id(AgentId::from_raw(571));
        rt.install_intrinsics();
        crate::install_cruft_host(&mut rt, vec!["cruft".to_string(), "main".to_string()]);
        rt.evaluate_module(
            r#"
            // Underlying fix: property access on the global object materializes the
            // lazy host module (was undefined before the interp GetProp change).
            globalThis.wtWorkerType = typeof globalThis.worker_threads.Worker;
            // Web projection: the global Worker exists as a constructor with the
            // Web-shaped surface, delegating to the node worker substrate.
            globalThis.hasGlobalWorker = typeof Worker === "function";
            const proto = Worker.prototype;
            const d = Object.getOwnPropertyDescriptor(proto, "onmessage");
            globalThis.onmessageIsAccessor = !!(d && typeof d.set === "function");
            globalThis.hasPostMessage = typeof proto.postMessage === "function";
            globalThis.hasTerminate = typeof proto.terminate === "function";
            globalThis.hasAddEventListener = typeof proto.addEventListener === "function";
            "#,
            "file://web-worker-projection-surface",
        )
        .expect("web worker projection surface");
        let g = rt.global_object.unwrap();
        assert!(
            matches!(rt.object_get(g, "wtWorkerType"), Value::String(ref s) if s.as_str() == "function"),
            "globalThis.worker_threads.Worker must materialize on property access"
        );
        for (flag, msg) in [
            ("hasGlobalWorker", "global Worker must be a constructor"),
            (
                "onmessageIsAccessor",
                "Worker.prototype.onmessage must be an accessor (Web shape)",
            ),
            ("hasPostMessage", "Worker.prototype.postMessage must exist"),
            ("hasTerminate", "Worker.prototype.terminate must exist"),
            (
                "hasAddEventListener",
                "Worker.prototype.addEventListener must exist",
            ),
        ] {
            assert!(
                matches!(rt.object_get(g, flag), Value::Boolean(true)),
                "{msg}"
            );
        }
    }

    #[test]
    fn agent_compartment_projection_lowers_to_same_agent_substrate() {

        let mut rt = Runtime::new_with_agent_id(AgentId::from_raw(572));
        rt.install_intrinsics();
        crate::install_cruft_host(&mut rt, vec!["cruft".to_string(), "main".to_string()]);
        let agent_before = rt.agent_id();
        let owner_before = rt.owner_thread_id();
        rt.run_script(
            r#"
            const c1 = new Compartment({ globals: { x: 10 } });
            const c2 = new Compartment({ globals: { x: 20 } });
            globalThis.r1 = c1.evaluate("x + 1");
            globalThis.r2 = c2.evaluate("x + 1");
            globalThis.noLeak = (typeof x === "undefined");
            "#,
            "file://agent-compartment-substrate",
        )
        .expect("compartment evaluate");

        assert_eq!(
            rt.agent_id(),
            agent_before,
            "compartment eval must not change the owning agent"
        );
        assert_eq!(
            rt.owner_thread_id(),
            owner_before,
            "compartment eval must stay on the owning agent's thread"
        );
        let g = rt.global_object.unwrap();
        assert!(matches!(rt.object_get(g, "r1"), Value::Number(n) if n == 11.0));
        assert!(matches!(rt.object_get(g, "r2"), Value::Number(n) if n == 21.0));
        assert!(
            matches!(rt.object_get(g, "noLeak"), Value::Boolean(true)),
            "compartment realm globals must be isolated from the parent realm"
        );
    }

    #[test]
    fn worker_threads_main_thread_public_surface_over_agent_substrate() {

        let mut rt = Runtime::new_with_agent_id(AgentId::from_raw(570));
        rt.install_intrinsics();
        crate::install_cruft_host(&mut rt, vec!["cruft".to_string(), "main".to_string()]);
        rt.evaluate_module(
            r#"
            import * as wt from "node:worker_threads";
            globalThis.isMain = wt.isMainThread;
            globalThis.threadId = wt.threadId;
            globalThis.hasWorker = typeof wt.Worker === "function";
            globalThis.parentNull = wt.parentPort === null || wt.parentPort === undefined;
            "#,
            "file://worker-threads-main-surface",
        )
        .expect("worker_threads main-thread surface");
        let g = rt.global_object.unwrap();
        assert!(
            matches!(rt.object_get(g, "isMain"), Value::Boolean(true)),
            "main agent must report worker_threads.isMainThread === true"
        );
        assert!(
            matches!(rt.object_get(g, "threadId"), Value::Number(n) if n == 0.0),
            "main agent threadId must be 0"
        );
        assert!(
            matches!(rt.object_get(g, "hasWorker"), Value::Boolean(true)),
            "worker_threads.Worker must be a constructor"
        );
        assert!(
            matches!(rt.object_get(g, "parentNull"), Value::Boolean(true)),
            "main agent parentPort must be null"
        );
    }

    #[test]
    fn ipc_reader_message_notifies_owner_wake() {
        let rt = Runtime::new_with_agent_id(AgentId::from_raw(521));
        let observed = rt.agent_wake_generation();
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback");
        let addr = listener.local_addr().expect("listener addr");
        let mut writer = TcpStream::connect(addr).expect("connect loopback");
        let (reader, _) = listener.accept().expect("accept loopback");
        let inbox = Arc::new(Mutex::new(Vec::new()));
        let alive = Arc::new(AtomicBool::new(true));

        spawn_reader(reader, inbox.clone(), alive, rt.agent_wake_handle());
        writeln!(writer, "{{\"ok\":true}}").expect("write IPC line");

        assert!(
            rt.wait_agent_wake_timeout(observed, std::time::Duration::from_secs(1)),
            "IPC reader thread must wake the owner runtime after queueing a line"
        );
        assert_eq!(inbox.lock().unwrap().len(), 1);
    }

    #[test]
    fn inprocess_worker_parent_message_notifies_owner_wake() {
        let rt = Runtime::new_with_agent_id(AgentId::from_raw(522));
        let observed = rt.agent_wake_generation();
        let wake = rt.agent_wake_handle();
        let (tx, rx) = mpsc::channel::<SendIr>();

        std::thread::spawn(move || {
            tx.send(SendIr::Undefined)
                .expect("send synthetic worker message");
            notify_agent_wake(&wake);
        });

        rx.recv().expect("receive synthetic worker message");
        assert!(
            rt.wait_agent_wake_timeout(observed, std::time::Duration::from_secs(1)),
            "in-process worker producer must wake the owner runtime"
        );
    }

    #[test]
    fn inprocess_worker_agent_ids_are_non_default_and_distinct() {
        let first = super::next_inprocess_worker_agent_id();
        let second = super::next_inprocess_worker_agent_id();
        assert_ne!(first, AgentId::DEFAULT);
        assert_ne!(second, AgentId::DEFAULT);
        assert_ne!(first, second);
    }

    #[test]
    fn process_ipc_registry_is_scoped_by_runtime_agent_id() {
        let mut rt_a = Runtime::new_with_agent_id(AgentId::from_raw(501));
        let mut rt_b = Runtime::new_with_agent_id(AgentId::from_raw(502));
        let target_b = rt_b.alloc_object(Object::new_ordinary());
        let alive_b = Arc::new(AtomicBool::new(true));
        let inbox_b = Arc::new(Mutex::new(vec!["{\"ok\":true}".to_string()]));

        CHANNELS.with(|v| {
            v.borrow_mut().clear();
            v.borrow_mut().push(Some(IpcChannel {
                agent_id: rt_b.agent_id(),
                writer: Arc::new(Mutex::new(None)),
                pending_writes: Arc::new(Mutex::new(VecDeque::new())),
                inbox: inbox_b.clone(),
                target: target_b,
                realm: rt_b.current_realm,
                alive: alive_b,
                child: None,
                exit_done: false,
            }));
        });

        assert!(!super::has_open(&rt_a));
        assert!(super::has_open(&rt_b));
        assert!(
            !harvest(&mut rt_a).expect("harvest agent A"),
            "agent A must not harvest agent B's process IPC inbox"
        );
        assert_eq!(
            inbox_b.lock().unwrap().len(),
            1,
            "agent B IPC inbox must remain after agent A harvest"
        );
        assert!(
            poll_io(&mut rt_b).expect("poll agent B"),
            "agent B owns IPC liveness"
        );

        CHANNELS.with(|v| v.borrow_mut().clear());
    }

    #[test]
    fn inprocess_worker_registry_is_scoped_by_runtime_agent_id() {
        let mut rt_a = Runtime::new_with_agent_id(AgentId::from_raw(511));
        let mut rt_b = Runtime::new_with_agent_id(AgentId::from_raw(512));
        let target_b = rt_b.alloc_object(Object::new_ordinary());
        let (to_worker_tx, _to_worker_rx) = mpsc::channel();
        let (_from_worker_tx, from_worker_rx) = mpsc::channel();
        let alive_b = Arc::new(AtomicBool::new(true));

        INPROCESS_WORKERS.with(|v| {
            v.borrow_mut().clear();
            v.borrow_mut().push(Some(InProcessWorker {
                agent_id: rt_b.agent_id(),
                worker_agent_id: AgentId::from_raw(1512),
                to_worker: to_worker_tx,
                from_worker: Arc::new(Mutex::new(from_worker_rx)),
                target: target_b,
                realm: rt_b.current_realm,
                alive: alive_b,
                exit_done: false,
            }));
        });

        assert!(!super::has_open_inprocess_workers(&rt_a));
        assert!(super::has_open_inprocess_workers(&rt_b));
        assert!(
            !harvest(&mut rt_a).expect("harvest agent A"),
            "agent A must not harvest agent B's in-process worker registry"
        );
        assert!(
            poll_io(&mut rt_b).expect("poll agent B"),
            "agent B owns in-process worker liveness"
        );

        INPROCESS_WORKERS.with(|v| v.borrow_mut().clear());
    }

    #[test]
    fn inprocess_worker_post_message_transfer_list_detaches_sender_arraybuffer() {
        use rusty_js_runtime::agent_scheduler::AgentScheduler;
        let _serialize = heavy_worker_test_guard();
        std::thread::Builder::new()
            .name("worker-transfer-list-test".into())
            .stack_size(16 * 1024 * 1024)
            .spawn(|| {
                let nonce = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_nanos())
                    .unwrap_or(0);
                let path = std::env::temp_dir().join(format!(
                    "cruft-inprocess-worker-transfer-{}-{nonce}.js",
                    std::process::id()
                ));
                std::fs::write(
                    &path,
                    r#"
                import { parentPort } from "node:worker_threads";
                parentPort.on("message", (ab) => {
                  const view = new Uint8Array(ab);
                  parentPort.postMessage({ len: ab.byteLength, first: view[0] });
                  parentPort.close();
                });
            "#,
                )
                .expect("write worker fixture");

                let mut rt = Runtime::new_with_agent_id(AgentId::from_raw(523));
                rt.install_intrinsics();
                crate::install_cruft_host(
                    &mut rt,
                    vec!["cruft".to_string(), path.to_string_lossy().into_owned()],
                );
                rt.evaluate_module(
                    &format!(
                        r#"
                import {{ Worker }} from "node:worker_threads";
                globalThis.gotLen = -1;
                globalThis.gotFirst = -1;
                globalThis.w = new Worker({:?});
                globalThis.wid = globalThis.w.__inprocess_worker_agent_id;
                globalThis.w.on("message", (m) => {{
                  globalThis.gotLen = m.len;
                  globalThis.gotFirst = m.first;
                }});
                globalThis.w.on("error", (e) => {{
                  globalThis.workerError = String(e && (e.message || e));
                }});
                "#,
                        path.to_string_lossy()
                    ),
                    "file://inprocess-worker-transfer-test",
                )
                .expect("worker transfer script");

                let sched = AgentScheduler::global();
                let worker_id = match rt.object_get(rt.global_object.unwrap(), "wid") {
                    Value::Number(n) => AgentId::from_raw(n as u64),
                    other => panic!("worker agent id not exposed: {other:?}"),
                };
                assert!(
                    pump_until(&mut rt, Duration::from_secs(30), |_rt| sched
                        .is_registered(worker_id)),
                    "worker must be running before the transfer is posted"
                );
                rt.run_script(
                    r#"
                    const ab = new ArrayBuffer(4);
                    new Uint8Array(ab)[0] = 42;
                    globalThis.w.postMessage(ab, [ab]);
                    globalThis.senderLen = ab.byteLength;
                    globalThis.senderDetached = ab.detached;
                    "#,
                    "file://inprocess-worker-transfer-post",
                )
                .expect("post transfer");

                pump_until(&mut rt, Duration::from_secs(30), |rt| {
                    let gt = rt.global_object.unwrap();
                    matches!(rt.object_get(gt, "gotLen"), Value::Number(n) if n == 4.0)
                });
                let gt = rt.global_object.unwrap();
                assert!(
                    matches!(rt.object_get(gt, "senderLen"), Value::Number(n) if n == 0.0),
                    "Worker.postMessage transferList must detach sender ArrayBuffer"
                );
                assert!(
                    matches!(rt.object_get(gt, "senderDetached"), Value::Boolean(true)),
                    "transferred sender ArrayBuffer must report detached=true"
                );
                assert!(
                    matches!(rt.object_get(gt, "gotLen"), Value::Number(n) if n == 4.0),
                    "worker must receive transferred ArrayBuffer byteLength; error={:?}",
                    rt.object_get(gt, "workerError")
                );
                assert!(
                    matches!(rt.object_get(gt, "gotFirst"), Value::Number(n) if n == 42.0),
                    "worker must receive transferred ArrayBuffer bytes"
                );

                let _ = std::fs::remove_file(path);
                INPROCESS_WORKERS.with(|v| v.borrow_mut().clear());
            })
            .expect("spawn large-stack test thread")
            .join()
            .expect("large-stack test thread");
    }

    #[test]
    fn inprocess_worker_registers_in_scheduler_while_alive_and_deregisters_on_exit() {
        use rusty_js_runtime::agent_scheduler::AgentScheduler;
        use rusty_js_runtime::interp::AgentId;
        let _serialize = heavy_worker_test_guard();
        std::thread::Builder::new()
            .name("worker-scheduler-registration-test".into())
            .stack_size(16 * 1024 * 1024)
            .spawn(|| {
                let nonce = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_nanos())
                    .unwrap_or(0);
                let path = std::env::temp_dir().join(format!(
                    "cruft-inprocess-worker-sched-{}-{nonce}.js",
                    std::process::id()
                ));

                std::fs::write(
                    &path,
                    r#"
                import { parentPort } from "node:worker_threads";
                parentPort.on("message", () => {
                  parentPort.postMessage("pong");
                  parentPort.close();
                });
            "#,
                )
                .expect("write worker fixture");

                let mut rt = Runtime::new_with_agent_id(AgentId::from_raw(524));
                rt.install_intrinsics();
                crate::install_cruft_host(
                    &mut rt,
                    vec!["cruft".to_string(), path.to_string_lossy().into_owned()],
                );
                rt.evaluate_module(
                    &format!(
                        r#"
                import {{ Worker }} from "node:worker_threads";
                globalThis.gotPong = false;
                globalThis.w = new Worker({:?});
                globalThis.wid = globalThis.w.__inprocess_worker_agent_id;
                globalThis.w.on("message", () => {{ globalThis.gotPong = true; }});
                "#,
                        path.to_string_lossy()
                    ),
                    "file://inprocess-worker-sched-test",
                )
                .expect("worker sched script");

                let gt = rt.global_object.unwrap();
                let worker_id = match rt.object_get(gt, "wid") {
                    Value::Number(n) => AgentId::from_raw(n as u64),
                    other => panic!("worker agent id not exposed: {other:?}"),
                };
                assert_ne!(worker_id, AgentId::DEFAULT);

                let sched = AgentScheduler::global();
                let saw_registered = pump_until(&mut rt, Duration::from_secs(30), |_rt| {
                    sched.is_registered(worker_id)
                });
                assert!(
                    saw_registered,
                    "worker must register its AgentId in the scheduler while alive"
                );

                rt.run_script("globalThis.w.postMessage('ping');", "file://sched-ping")
                    .expect("post ping");
                pump_until(&mut rt, Duration::from_secs(30), |rt| {
                    let gt = rt.global_object.unwrap();
                    matches!(rt.object_get(gt, "gotPong"), Value::Boolean(true))
                        && !sched.is_registered(worker_id)
                });

                let gt = rt.global_object.unwrap();
                assert!(
                    matches!(rt.object_get(gt, "gotPong"), Value::Boolean(true)),
                    "worker must reply before exiting"
                );
                assert!(
                    !sched.is_registered(worker_id),
                    "worker must deregister its AgentId from the scheduler on exit"
                );

                let _ = std::fs::remove_file(path);
                INPROCESS_WORKERS.with(|v| v.borrow_mut().clear());
            })
            .expect("spawn large-stack test thread")
            .join()
            .expect("large-stack test thread");
    }
}
