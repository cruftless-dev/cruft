
use std::cell::Cell;
use std::collections::VecDeque;
use std::rc::Rc;
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::time::Instant;

use crate::interp::{ArrayBufferRecord, Runtime};
use crate::value::{Object, Value};

#[derive(Default)]
pub struct BroadcastState {
    pub generation: u64,
    pub payload: Option<(Arc<Mutex<Vec<u8>>>, usize)>,
}

pub struct AgentHub {

    pub reports: Mutex<VecDeque<String>>,

    pub start: Instant,

    pub threads: Mutex<Vec<std::thread::JoinHandle<()>>>,

    pub broadcast: Mutex<BroadcastState>,
    pub bcv: Condvar,
}

static HUB: OnceLock<Arc<AgentHub>> = OnceLock::new();

pub fn hub() -> Arc<AgentHub> {
    HUB.get_or_init(|| {
        Arc::new(AgentHub {
            reports: Mutex::new(VecDeque::new()),
            start: Instant::now(),
            threads: Mutex::new(Vec::new()),
            broadcast: Mutex::new(BroadcastState::default()),
            bcv: Condvar::new(),
        })
    })
    .clone()
}

fn install_agent_harness(rt: &mut Runtime, hub: &Arc<AgentHub>) {
    let agent = rt.alloc_object(Object::new_ordinary());
    {
        let _agent_root = rt.push_temporary_value_roots(&[Value::Object(agent)]);
        let v262 = rt.alloc_object(Object::new_ordinary());
        rt.object_set(v262, "agent".into(), Value::Object(agent));
        rt.define_global_property("$262", Value::Object(v262));
    }

    let h = hub.clone();
    let report = rt.alloc_object(crate::intrinsics::make_native(
        "report",
        move |_rt, args| {
            let m = crate::abstract_ops::to_string(args.first().unwrap_or(&Value::Undefined));
            h.reports
                .lock()
                .expect("agent reports")
                .push_back((*m).clone());
            Ok(Value::Undefined)
        },
    ));
    rt.object_set(agent, "report".into(), Value::Object(report));

    let leaving = rt.alloc_object(crate::intrinsics::make_native("leaving", |_rt, _a| {
        Ok(Value::Undefined)
    }));
    rt.object_set(agent, "leaving".into(), Value::Object(leaving));

    let sleep = rt.alloc_object(crate::intrinsics::make_native("sleep", |_rt, args| {
        if let Some(Value::Number(ms)) = args.first() {
            if *ms > 0.0 && ms.is_finite() {
                std::thread::sleep(std::time::Duration::from_millis(*ms as u64));
            }
        }
        Ok(Value::Undefined)
    }));
    rt.object_set(agent, "sleep".into(), Value::Object(sleep));

    let h = hub.clone();
    let mono = rt.alloc_object(crate::intrinsics::make_native(
        "monotonicNow",
        move |_rt, _a| Ok(Value::Number(h.start.elapsed().as_secs_f64() * 1000.0)),
    ));
    rt.object_set(agent, "monotonicNow".into(), Value::Object(mono));

    let h = hub.clone();
    let last_gen: Cell<u64> = Cell::new(0);
    let recv = rt.alloc_object(crate::intrinsics::make_native(
        "receiveBroadcast",
        move |rt, args| {
            let cb = args.first().cloned().unwrap_or(Value::Undefined);
            let (arc, len) = {
                let mut g = h.broadcast.lock().expect("broadcast lock");
                while g.generation <= last_gen.get() {
                    g = h.bcv.wait(g).expect("broadcast wait");
                }
                last_gen.set(g.generation);
                match &g.payload {
                    Some((arc, len)) => (arc.clone(), *len),
                    None => return Ok(Value::Undefined),
                }
            };
            let sab = build_shared_sab(rt, arc, len);
            let _call_roots = rt.push_temporary_value_roots(&[cb.clone(), sab.clone()]);
            rt.call_function(cb, Value::Undefined, vec![sab])?;
            Ok(Value::Undefined)
        },
    ));
    rt.object_set(agent, "receiveBroadcast".into(), Value::Object(recv));
}

thread_local! {

    static IS_AGENT_THREAD: Cell<bool> = const { Cell::new(false) };
}

pub fn is_agent_thread() -> bool {
    IS_AGENT_THREAD.with(|c| c.get())
}

pub fn agent_start(src: String) {
    let the_hub = hub();
    let agent_hub = the_hub.clone();

    const AGENT_STACK_BYTES: usize = 1024 * 1024 * 1024;
    let handle = std::thread::Builder::new()
        .name("cruft-agent262".to_string())
        .stack_size(AGENT_STACK_BYTES)
        .spawn(move || {
            crate::interp::publish_native_stack_bounds(AGENT_STACK_BYTES);
            IS_AGENT_THREAD.with(|c| c.set(true));

            static AGENT_INIT_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
            let mut rt = {
                let _g = AGENT_INIT_LOCK.lock().expect("agent init lock");
                let mut rt = Runtime::new();
                rt.install_intrinsics();
                install_agent_harness(&mut rt, &agent_hub);
                rt
            };
            let _ = rt.evaluate_script(&src, "file://agent262");

            loop {
                match crate::job_queue::pump_one_tick(&mut rt) {
                    Ok(true) => continue,
                    _ => break,
                }
            }
        })
        .expect("frame-chain R6: spawn agent262 thread");
    the_hub.threads.lock().expect("agent threads").push(handle);
}

pub fn agent_get_report() -> Option<String> {
    hub().reports.lock().expect("agent reports").pop_front()
}

pub fn agent_monotonic_now() -> f64 {
    hub().start.elapsed().as_secs_f64() * 1000.0
}

struct WaitSlot {
    woken: Mutex<bool>,
    cv: Condvar,
}

struct Waiter {
    key: (usize, usize),
    slot: Arc<WaitSlot>,
}

static WAITERS: OnceLock<Mutex<Vec<Waiter>>> = OnceLock::new();
fn waiters() -> &'static Mutex<Vec<Waiter>> {
    WAITERS.get_or_init(|| Mutex::new(Vec::new()))
}

pub enum WaitResult {

    NotEqual,

    Ok,

    TimedOut,
}

fn region_key(arc: &Arc<Mutex<Vec<u8>>>, byte_index: usize) -> (usize, usize) {
    (Arc::as_ptr(arc) as *const () as usize, byte_index)
}

fn atomics_trace_enabled() -> bool {
    std::env::var("CRUFT_ATOMICS_TRACE")
        .map(|v| !(v == "0" || v.eq_ignore_ascii_case("false")))
        .unwrap_or(false)
}

pub fn atomics_park(
    arc: &Arc<Mutex<Vec<u8>>>,
    byte_index: usize,
    expected: &[u8],
    timeout_ms: f64,
) -> WaitResult {
    let slot = Arc::new(WaitSlot {
        woken: Mutex::new(false),
        cv: Condvar::new(),
    });
    {
        let mut w = waiters().lock().expect("waiters lock");

        {
            let bytes = arc.lock().expect("SAB bytes lock");
            let end = byte_index + expected.len();
            if end > bytes.len() || &bytes[byte_index..end] != expected {
                return WaitResult::NotEqual;
            }
        }
        if atomics_trace_enabled() {
            eprintln!(
                "[cruft-atomics] park key={:?} expected_len={} timeout_ms={}",
                region_key(arc, byte_index),
                expected.len(),
                timeout_ms
            );
        }
        w.push(Waiter {
            key: region_key(arc, byte_index),
            slot: slot.clone(),
        });
    }
    let timed_out = {
        let woken = slot.woken.lock().expect("wake slot");
        if timeout_ms.is_infinite() {
            let mut g = woken;
            while !*g {
                g = slot.cv.wait(g).expect("wait");
            }
            false
        } else {
            let dur = std::time::Duration::from_millis(timeout_ms.max(0.0) as u64);
            let (g, res) = slot
                .cv
                .wait_timeout_while(woken, dur, |w| !*w)
                .expect("wait_timeout");
            res.timed_out() && !*g
        }
    };

    waiters()
        .lock()
        .expect("waiters lock")
        .retain(|x| !Arc::ptr_eq(&x.slot, &slot));
    if timed_out {
        WaitResult::TimedOut
    } else {
        WaitResult::Ok
    }
}

#[derive(Clone)]
pub struct AsyncWaitHandle(Arc<WaitSlot>);

impl AsyncWaitHandle {

    pub fn woken(&self) -> bool {
        *self.0.woken.lock().expect("wake slot")
    }

    pub fn deregister(&self) {
        waiters()
            .lock()
            .expect("waiters lock")
            .retain(|x| !Arc::ptr_eq(&x.slot, &self.0));
    }
}

pub enum AsyncWaitRegistration {
    NotEqual,
    Registered(AsyncWaitHandle),
}

pub fn register_async_waiter_if_equal(
    arc: &Arc<Mutex<Vec<u8>>>,
    byte_index: usize,
    expected: &[u8],
) -> AsyncWaitRegistration {
    let slot = Arc::new(WaitSlot {
        woken: Mutex::new(false),
        cv: Condvar::new(),
    });
    let mut w = waiters().lock().expect("waiters lock");
    {
        let bytes = arc.lock().expect("SAB bytes lock");
        let end = byte_index + expected.len();
        if end > bytes.len() || &bytes[byte_index..end] != expected {
            return AsyncWaitRegistration::NotEqual;
        }
    }
    w.push(Waiter {
        key: region_key(arc, byte_index),
        slot: slot.clone(),
    });
    AsyncWaitRegistration::Registered(AsyncWaitHandle(slot))
}

pub fn atomics_notify_waiters(arc: &Arc<Mutex<Vec<u8>>>, byte_index: usize, count: usize) -> usize {
    let key = region_key(arc, byte_index);
    let mut w = waiters().lock().expect("waiters lock");
    if atomics_trace_enabled() {
        eprintln!(
            "[cruft-atomics] notify key={key:?} count={count} waiters={}",
            w.len()
        );
    }
    let mut woken = 0usize;
    let mut i = 0;
    while i < w.len() && woken < count {
        if w[i].key == key {
            let waiter = w.remove(i);
            *waiter.slot.woken.lock().expect("wake slot") = true;
            waiter.slot.cv.notify_all();
            woken += 1;
        } else {
            i += 1;
        }
    }
    woken
}

pub fn agent_broadcast(arc: Arc<Mutex<Vec<u8>>>, byte_len: usize) {
    let hub = hub();
    let mut g = hub.broadcast.lock().expect("broadcast lock");
    g.generation += 1;
    g.payload = Some((arc, byte_len));
    drop(g);
    hub.bcv.notify_all();
}

fn build_shared_sab(rt: &mut Runtime, arc: Arc<Mutex<Vec<u8>>>, len: usize) -> Value {
    let proto = match rt.global_get("SharedArrayBuffer") {
        Value::Object(ctor) => match rt.object_get(ctor, "prototype") {
            Value::Object(p) => Some(p),
            _ => None,
        },
        _ => None,
    };
    let mut o = Object::new_ordinary();
    o.set_own_internal(
        "__kind".into(),
        Value::String(Rc::new(crate::value::JsString::from("SharedArrayBuffer"))),
    );
    o.proto = proto;
    let id = rt.alloc_object(o);
    rt.object_set(id, "byteLength".into(), Value::Number(len as f64));
    rt.array_buffers.insert(
        id,
        ArrayBufferRecord {
            byte_length: len,
            max_byte_length: len,
            backing_epoch: 0,
            data: Vec::new(),
            detached: false,
            untransferable: false,
            shared: Some(arc),
        },
    );
    Value::Object(id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::Duration;

    #[test]
    fn atomics_waiter_parks_and_notify_wakes_same_shared_backing_slot() {
        let shared = Arc::new(Mutex::new(vec![0, 0, 0, 0]));
        let waiting_shared = shared.clone();
        let (started_tx, started_rx) = mpsc::channel();
        let (result_tx, result_rx) = mpsc::channel();

        let waiter = std::thread::spawn(move || {
            started_tx.send(()).expect("signal waiter start");
            let result = atomics_park(&waiting_shared, 0, &[0, 0, 0, 0], 1_000.0);
            result_tx.send(result).expect("send wait result");
        });

        started_rx
            .recv_timeout(Duration::from_millis(100))
            .expect("waiter thread should start");
        std::thread::sleep(Duration::from_millis(20));
        assert_eq!(
            atomics_notify_waiters(&shared, 0, 1),
            1,
            "notify should wake exactly one waiter on the same shared backing/index"
        );

        let result = result_rx
            .recv_timeout(Duration::from_millis(250))
            .expect("waiter should report promptly after notify");
        waiter.join().expect("waiter thread should join");
        assert!(
            matches!(result, WaitResult::Ok),
            "notified waiter should observe Atomics.wait ok"
        );
        assert_eq!(
            atomics_notify_waiters(&shared, 0, 1),
            0,
            "woken waiter should be removed from the registry"
        );
    }

    #[test]
    fn atomics_waiter_key_is_shared_backing_and_byte_index() {
        let shared = Arc::new(Mutex::new(vec![0, 0, 0, 0, 0, 0, 0, 0]));
        let waiting_shared = shared.clone();
        let (result_tx, result_rx) = mpsc::channel();

        let waiter = std::thread::spawn(move || {
            let result = atomics_park(&waiting_shared, 4, &[0, 0, 0, 0], 1_000.0);
            result_tx.send(result).expect("send wait result");
        });

        std::thread::sleep(Duration::from_millis(20));
        assert_eq!(
            atomics_notify_waiters(&shared, 0, 1),
            0,
            "notify at a sibling byte index must not wake the waiter"
        );
        assert_eq!(
            atomics_notify_waiters(&shared, 4, 1),
            1,
            "notify at the exact byte index should wake the waiter"
        );

        let result = result_rx
            .recv_timeout(Duration::from_millis(250))
            .expect("waiter should report promptly after exact-index notify");
        waiter.join().expect("waiter thread should join");
        assert!(matches!(result, WaitResult::Ok));
    }

    #[test]
    fn atomics_async_waiter_compare_and_register_is_atomic() {
        let shared = Arc::new(Mutex::new(vec![1, 0, 0, 0]));
        assert!(matches!(
            register_async_waiter_if_equal(&shared, 0, &[0, 0, 0, 0]),
            AsyncWaitRegistration::NotEqual
        ));
        assert_eq!(
            atomics_notify_waiters(&shared, 0, 1),
            0,
            "not-equal async registration must not leave a stale waiter"
        );

        {
            let mut bytes = shared.lock().expect("shared backing lock");
            bytes[0] = 0;
        }
        let handle = match register_async_waiter_if_equal(&shared, 0, &[0, 0, 0, 0]) {
            AsyncWaitRegistration::Registered(handle) => handle,
            AsyncWaitRegistration::NotEqual => panic!("expected async waiter registration"),
        };
        assert_eq!(
            atomics_notify_waiters(&shared, 0, 1),
            1,
            "same-key notify should wake the registered async waiter"
        );
        assert!(handle.woken());
    }

    #[test]
    fn native_agent_sab_wait_notify_projection_reports_ok() {
        std::thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn(|| {
                while agent_get_report().is_some() {}

                let mut rt = Runtime::new();
                rt.install_intrinsics();
                rt.evaluate_script(
                    r#"
            var sab = new SharedArrayBuffer(4);
            var ta = new Int32Array(sab);
            __cruft_agent_start(
              "$262.agent.receiveBroadcast(function(sab) {" +
              "  var ta = new Int32Array(sab);" +
              "  $262.agent.report('ready');" +
              "  var r = Atomics.wait(ta, 0, 0, 1000);" +
              "  $262.agent.report(r + ':' + Atomics.load(ta, 0));" +
              "});"
            );
            __cruft_agent_broadcast(sab);
            var ready = null;
            for (var i = 0; i < 200 && ready === null; i++) {
              ready = __cruft_agent_get_report();
              if (ready === null) __cruft_agent_sleep(1);
            }
            if (ready !== 'ready') throw new Error('agent not ready: ' + ready);
            Atomics.store(ta, 0, 7);
            globalThis.__notified = Atomics.notify(ta, 0, 1);
            var done = null;
            for (var j = 0; j < 200 && done === null; j++) {
              done = __cruft_agent_get_report();
              if (done === null) __cruft_agent_sleep(1);
            }
            globalThis.__agent_done = done;
            "#,
                    "file://agent262-wait-notify-projection-test",
                )
                .expect("agent wait/notify projection script should run");

                let global = rt.global_object.expect("global object");
                assert!(matches!(
                    rt.object_get(global, "__notified"),
                    Value::Number(n) if n == 1.0
                ));
                assert!(matches!(
                    rt.object_get(global, "__agent_done"),
                    Value::String(ref s) if s.as_str() == "ok:7"
                ));
            })
            .expect("spawn large-stack agent projection test")
            .join()
            .expect("large-stack agent projection test should not panic");
    }

    #[test]
    fn native_agent_sab_waitasync_notify_projection_reports_ok() {
        std::thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn(|| {
                while agent_get_report().is_some() {}

                let mut rt = Runtime::new();
                rt.install_intrinsics();
                rt.evaluate_script(
                    r#"
            var sab = new SharedArrayBuffer(4);
            var ta = new Int32Array(sab);
            __cruft_agent_start(
              "$262.agent.receiveBroadcast(function(sab) {" +
              "  var ta = new Int32Array(sab);" +
              "  var waiter = Atomics.waitAsync(ta, 0, 0, 1000);" +
              "  if (!waiter.async) $262.agent.report('sync:' + waiter.value);" +
              "  waiter.value.then(function(r) {" +
              "    $262.agent.report(r + ':' + Atomics.load(ta, 0));" +
              "  });" +
              "  $262.agent.report('ready');" +
              "});"
            );
            __cruft_agent_broadcast(sab);
            var ready = null;
            for (var i = 0; i < 200 && ready === null; i++) {
              ready = __cruft_agent_get_report();
              if (ready === null) __cruft_agent_sleep(1);
            }
            if (ready !== 'ready') throw new Error('agent not ready: ' + ready);
            Atomics.store(ta, 0, 7);
            globalThis.__notified = Atomics.notify(ta, 0, 1);
            var done = null;
            for (var j = 0; j < 400 && done === null; j++) {
              done = __cruft_agent_get_report();
              if (done === null) __cruft_agent_sleep(1);
            }
            globalThis.__agent_done = done;
            "#,
                    "file://agent262-waitasync-notify-projection-test",
                )
                .expect("agent waitAsync/notify projection script should run");

                let global = rt.global_object.expect("global object");
                assert!(matches!(
                    rt.object_get(global, "__notified"),
                    Value::Number(n) if n == 1.0
                ));
                assert!(matches!(
                    rt.object_get(global, "__agent_done"),
                    Value::String(ref s) if s.as_str() == "ok:7"
                ));
            })
            .expect("spawn large-stack waitAsync projection test")
            .join()
            .expect("large-stack waitAsync projection test should not panic");
    }
}
