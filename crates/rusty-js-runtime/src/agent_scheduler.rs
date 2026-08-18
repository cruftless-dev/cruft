
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock};

use crate::interp::{AgentId, HostCompletionJob, Runtime};

pub type AgentWakeHandle = Arc<(Mutex<u64>, Condvar)>;

pub type AgentInbox = Arc<Mutex<VecDeque<HostCompletionJob>>>;

#[derive(Default)]
pub struct AgentStatusCell {
    blocked: std::sync::atomic::AtomicBool,
    heap_bytes: std::sync::atomic::AtomicUsize,
    last_error: Mutex<Option<String>>,
}

impl AgentStatusCell {
    pub fn set_blocked(&self, blocked: bool) {
        self.blocked
            .store(blocked, std::sync::atomic::Ordering::Relaxed);
    }
    pub fn is_blocked(&self) -> bool {
        self.blocked.load(std::sync::atomic::Ordering::Relaxed)
    }
    pub fn set_heap_bytes(&self, bytes: usize) {
        self.heap_bytes
            .store(bytes, std::sync::atomic::Ordering::Relaxed);
    }
    pub fn heap_bytes(&self) -> usize {
        self.heap_bytes.load(std::sync::atomic::Ordering::Relaxed)
    }
    pub fn set_last_error(&self, err: Option<String>) {
        if let Ok(mut slot) = self.last_error.lock() {
            *slot = err;
        }
    }
    pub fn last_error(&self) -> Option<String> {
        self.last_error.lock().ok().and_then(|s| s.clone())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AgentControl {

    Terminate,

    Cancel(u64),
}

pub type AgentControlQueue = Arc<Mutex<VecDeque<AgentControl>>>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentMetrics {
    pub agent_id: AgentId,
    pub queue_length: usize,
    pub wake_generation: u64,
    pub blocked: bool,
    pub heap_bytes: usize,
    pub last_error: Option<String>,
}

#[derive(Clone)]
pub struct AgentHandle {
    agent_id: AgentId,
    wake: AgentWakeHandle,
    inbox: AgentInbox,
    status: Arc<AgentStatusCell>,
    control: AgentControlQueue,
}

impl AgentHandle {
    pub fn new(
        agent_id: AgentId,
        wake: AgentWakeHandle,
        inbox: AgentInbox,
        status: Arc<AgentStatusCell>,
        control: AgentControlQueue,
    ) -> Self {
        Self {
            agent_id,
            wake,
            inbox,
            status,
            control,
        }
    }

    pub fn post_control(&self, ctrl: AgentControl) -> bool {
        let ok = self
            .control
            .lock()
            .map(|mut q| {
                q.push_back(ctrl);
                true
            })
            .unwrap_or(false);
        if ok {
            self.wake();
        }
        ok
    }

    pub fn drain_control(&self) -> Vec<AgentControl> {
        self.control
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn status(&self) -> &Arc<AgentStatusCell> {
        &self.status
    }

    pub fn metrics(&self) -> AgentMetrics {
        AgentMetrics {
            agent_id: self.agent_id,
            queue_length: self.pending_completions(),
            wake_generation: self.wake_generation(),
            blocked: self.status.is_blocked(),
            heap_bytes: self.status.heap_bytes(),
            last_error: self.status.last_error(),
        }
    }

    pub fn agent_id(&self) -> AgentId {
        self.agent_id
    }

    pub fn wake(&self) {
        let (lock, cv) = &*self.wake;
        if let Ok(mut generation) = lock.lock() {
            *generation = generation.wrapping_add(1);
            cv.notify_all();
        }
    }

    pub fn wake_generation(&self) -> u64 {
        self.wake
            .0
            .lock()
            .map(|generation| *generation)
            .unwrap_or(0)
    }

    pub fn post_completion(&self, job: HostCompletionJob) -> bool {
        let posted = {
            let Ok(mut inbox) = self.inbox.lock() else {
                return false;
            };
            inbox.push_back(job);
            true
        };
        if posted {
            self.wake();
        }
        posted
    }

    pub fn pending_completions(&self) -> usize {
        self.inbox.lock().map(|inbox| inbox.len()).unwrap_or(0)
    }
}

pub struct AgentScheduler {
    agents: Mutex<HashMap<AgentId, AgentHandle>>,

    total_registered: AtomicU64,
    total_deregistered: AtomicU64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AgentSchedulerMetrics {
    pub live: usize,
    pub total_registered: u64,
    pub total_deregistered: u64,
}

static GLOBAL: OnceLock<AgentScheduler> = OnceLock::new();

impl AgentScheduler {

    pub fn global() -> &'static AgentScheduler {
        GLOBAL.get_or_init(|| AgentScheduler {
            agents: Mutex::new(HashMap::new()),
            total_registered: AtomicU64::new(0),
            total_deregistered: AtomicU64::new(0),
        })
    }

    pub fn register(&self, handle: AgentHandle) -> bool {
        let Ok(mut agents) = self.agents.lock() else {
            return false;
        };
        if agents.contains_key(&handle.agent_id) {
            return false;
        }
        agents.insert(handle.agent_id, handle);

        self.total_registered.fetch_add(1, Ordering::Relaxed);
        true
    }

    pub fn deregister(&self, agent_id: AgentId) -> bool {
        let Ok(mut agents) = self.agents.lock() else {
            return false;
        };
        if agents.remove(&agent_id).is_some() {
            self.total_deregistered.fetch_add(1, Ordering::Relaxed);
            true
        } else {
            false
        }
    }

    pub fn metrics(&self) -> AgentSchedulerMetrics {
        match self.agents.lock() {
            Ok(agents) => AgentSchedulerMetrics {
                live: agents.len(),
                total_registered: self.total_registered.load(Ordering::Relaxed),
                total_deregistered: self.total_deregistered.load(Ordering::Relaxed),
            },
            Err(_) => AgentSchedulerMetrics {
                live: 0,
                total_registered: self.total_registered.load(Ordering::Relaxed),
                total_deregistered: self.total_deregistered.load(Ordering::Relaxed),
            },
        }
    }

    pub fn post_control(&self, target: AgentId, ctrl: AgentControl) -> bool {
        match self.lookup(target) {
            Some(handle) => handle.post_control(ctrl),
            None => false,
        }
    }

    pub fn terminate(&self, target: AgentId) -> bool {
        self.post_control(target, AgentControl::Terminate)
    }

    pub fn cancel(&self, target: AgentId, token: u64) -> bool {
        self.post_control(target, AgentControl::Cancel(token))
    }

    pub fn agent_metrics(&self, agent_id: AgentId) -> Option<AgentMetrics> {
        self.lookup(agent_id).map(|h| h.metrics())
    }

    pub fn all_agent_metrics(&self) -> Vec<AgentMetrics> {
        self.agents
            .lock()
            .map(|agents| agents.values().map(|h| h.metrics()).collect())
            .unwrap_or_default()
    }

    pub fn lookup(&self, agent_id: AgentId) -> Option<AgentHandle> {
        self.agents
            .lock()
            .ok()
            .and_then(|agents| agents.get(&agent_id).cloned())
    }

    pub fn is_registered(&self, agent_id: AgentId) -> bool {
        self.agents
            .lock()
            .map(|agents| agents.contains_key(&agent_id))
            .unwrap_or(false)
    }

    pub fn agent_ids(&self) -> Vec<AgentId> {
        self.agents
            .lock()
            .map(|agents| agents.keys().copied().collect())
            .unwrap_or_default()
    }

    pub fn len(&self) -> usize {
        self.agents.lock().map(|agents| agents.len()).unwrap_or(0)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn post_completion(&self, target: AgentId, job: HostCompletionJob) -> bool {
        match self.lookup(target) {
            Some(handle) => handle.post_completion(job),
            None => false,
        }
    }
}

pub struct AgentRuntime {
    runtime: Runtime,
    handle: AgentHandle,
    registered: bool,
}

impl AgentRuntime {

    pub fn new(agent_id: AgentId) -> Self {
        let runtime = Runtime::new_with_agent_id(agent_id);
        let handle = runtime.agent_handle();
        let registered = AgentScheduler::global().register(handle.clone());
        Self {
            runtime,
            handle,
            registered,
        }
    }

    pub fn agent_id(&self) -> AgentId {
        self.handle.agent_id()
    }

    pub fn handle(&self) -> &AgentHandle {
        &self.handle
    }

    pub fn runtime_mut(&mut self) -> &mut Runtime {
        &mut self.runtime
    }

    pub fn run_until<F>(&mut self, done: F, poll_timeout: std::time::Duration)
    where
        F: FnMut(&mut Runtime) -> bool,
    {
        self.runtime.run_host_completions_until(done, poll_timeout);
    }

    pub fn metrics(&self) -> AgentMetrics {
        self.handle.metrics()
    }

    pub fn terminate_requested(&self) -> bool {
        self.runtime.agent_terminate_requested()
    }
}

impl Drop for AgentRuntime {
    fn drop(&mut self) {
        if self.registered {
            AgentScheduler::global().deregister(self.handle.agent_id());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interp::Runtime;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TEST_AGENT_ID: AtomicU64 = AtomicU64::new(0x8000_0000);
    fn fresh_agent_id() -> AgentId {
        AgentId::from_raw(NEXT_TEST_AGENT_ID.fetch_add(1, Ordering::Relaxed))
    }

    #[test]
    fn register_lookup_enumerate_deregister() {
        let sched = AgentScheduler::global();
        let a = Runtime::new_with_agent_id(fresh_agent_id());
        let b = Runtime::new_with_agent_id(fresh_agent_id());
        let (ha, hb) = (a.agent_handle(), b.agent_handle());
        let (ida, idb) = (ha.agent_id(), hb.agent_id());

        assert!(sched.register(ha));
        assert!(sched.register(hb));

        assert!(!sched.register(a.agent_handle()));

        assert!(sched.is_registered(ida));
        assert!(sched.is_registered(idb));
        let ids = sched.agent_ids();
        assert!(ids.contains(&ida) && ids.contains(&idb));
        assert!(sched.lookup(ida).is_some());

        assert!(sched.deregister(ida));
        assert!(!sched.is_registered(ida));
        assert!(sched.is_registered(idb));
        assert!(sched.deregister(idb));
        assert!(!sched.deregister(idb));
    }

    #[test]
    fn post_completion_targets_only_the_addressed_agent() {
        let sched = AgentScheduler::global();
        let mut a = Runtime::new_with_agent_id(fresh_agent_id());
        let mut b = Runtime::new_with_agent_id(fresh_agent_id());
        let (ida, idb) = (a.agent_id(), b.agent_id());
        assert!(sched.register(a.agent_handle()));
        assert!(sched.register(b.agent_handle()));

        let gen_a_before = a.agent_wake_generation();

        assert!(sched.post_completion(ida, Box::new(|_rt: &mut Runtime| {})));

        assert_eq!(sched.lookup(ida).unwrap().pending_completions(), 1);
        assert_eq!(sched.lookup(idb).unwrap().pending_completions(), 0);
        assert!(a.agent_wake_generation() != gen_a_before);

        let drained = a.drain_host_completion_inbox();
        assert_eq!(drained, 1);
        assert_eq!(sched.lookup(ida).unwrap().pending_completions(), 0);
        assert_eq!(b.drain_host_completion_inbox(), 0);

        sched.deregister(ida);
        assert!(!sched.post_completion(ida, Box::new(|_rt: &mut Runtime| {})));
        sched.deregister(idb);
    }

    #[test]
    fn cross_thread_post_runs_on_owner_thread_only() {
        let sched = AgentScheduler::global();
        let mut a = Runtime::new_with_agent_id(fresh_agent_id());
        let ida = a.agent_id();
        assert!(sched.register(a.agent_handle()));

        let owner = std::thread::current().id();
        let ran_on = Arc::new(Mutex::new(None::<std::thread::ThreadId>));
        let ran_on_job = ran_on.clone();

        let handle = std::thread::spawn(move || {
            let posting_thread = std::thread::current().id();
            let posted = AgentScheduler::global().post_completion(
                ida,
                Box::new(move |_rt: &mut Runtime| {
                    *ran_on_job.lock().unwrap() = Some(std::thread::current().id());
                }),
            );
            (posted, posting_thread)
        });
        let (posted, posting_thread) = handle.join().unwrap();
        assert!(posted);
        assert_ne!(posting_thread, owner);

        assert!(ran_on.lock().unwrap().is_none());

        assert_eq!(a.drain_host_completion_inbox(), 1);
        assert_eq!(*ran_on.lock().unwrap(), Some(owner));

        sched.deregister(ida);
    }

    #[test]
    fn lifecycle_metrics_track_registrations_consistently() {
        let sched = AgentScheduler::global();

        let check = |m: AgentSchedulerMetrics| {
            assert_eq!(
                m.live as u64,
                m.total_registered - m.total_deregistered,
                "live must equal total_registered - total_deregistered: {m:?}"
            );
        };
        let m0 = sched.metrics();
        check(m0);

        let ids: Vec<AgentId> = (0..3).map(|_| fresh_agent_id()).collect();
        let rts: Vec<Runtime> = ids
            .iter()
            .map(|&id| Runtime::new_with_agent_id(id))
            .collect();
        for rt in &rts {
            assert!(sched.register(rt.agent_handle()));
        }
        let m1 = sched.metrics();
        check(m1);
        assert!(
            m1.total_registered >= m0.total_registered + 3,
            "three registers must advance the cumulative spawn counter by >=3"
        );

        assert!(sched.deregister(ids[0]));
        assert!(sched.deregister(ids[1]));
        let m2 = sched.metrics();
        check(m2);
        assert!(
            m2.total_deregistered >= m0.total_deregistered + 2,
            "two deregisters must advance the cumulative exit counter by >=2"
        );

        assert!(sched.deregister(ids[2]));
        check(sched.metrics());
    }

    #[test]
    fn per_agent_metrics_report_queue_wake_and_published_status() {
        let sched = AgentScheduler::global();
        let mut a = Runtime::new_with_agent_id(fresh_agent_id());
        let id = a.agent_id();
        assert!(sched.register(a.agent_handle()));

        let m0 = sched.agent_metrics(id).expect("registered agent has metrics");
        assert_eq!(m0.queue_length, 0);
        assert!(!m0.blocked);
        assert!(m0.last_error.is_none());
        assert!(sched.post_completion(id, Box::new(|_rt: &mut Runtime| {})));
        assert!(sched.post_completion(id, Box::new(|_rt: &mut Runtime| {})));
        let m1 = sched.agent_metrics(id).unwrap();
        assert_eq!(m1.queue_length, 2, "queue length reflects posted completions");
        assert!(
            m1.wake_generation > m0.wake_generation,
            "posting advances the wake generation"
        );

        a.set_agent_blocked(true);
        a.publish_agent_heap_bytes(4096);
        a.set_agent_last_error(Some("boom".to_string()));
        let m2 = sched.agent_metrics(id).unwrap();
        assert!(m2.blocked, "published blocked state is visible to the scheduler");
        assert_eq!(m2.heap_bytes, 4096);
        assert_eq!(m2.last_error.as_deref(), Some("boom"));

        a.drain_host_completion_inbox();
        assert_eq!(sched.agent_metrics(id).unwrap().queue_length, 0);

        assert!(sched.all_agent_metrics().iter().any(|m| m.agent_id == id));

        sched.deregister(id);
        assert!(sched.agent_metrics(id).is_none());
    }

    #[test]
    fn agent_runtime_bundles_register_drive_metrics_terminate_drop() {
        use std::time::{Duration, Instant};

        let sched = AgentScheduler::global();
        let id = fresh_agent_id();

        let tb = std::thread::spawn(move || {
            let mut agent = AgentRuntime::new(id);
            assert_eq!(agent.agent_id(), id);

            assert_eq!(agent.metrics().agent_id, id);

            agent.run_until(|_rt| false, Duration::from_millis(25));
            agent.terminate_requested()

        });

        let deadline = Instant::now() + Duration::from_secs(5);
        while !sched.is_registered(id) && Instant::now() < deadline {
            std::thread::yield_now();
        }
        assert!(sched.is_registered(id), "AgentRuntime::new registered the agent");
        assert!(sched.agent_metrics(id).is_some(), "scheduler sees the agent's metrics");

        assert!(sched.terminate(id));
        let terminated = tb.join().unwrap();
        assert!(terminated, "AgentRuntime observed the Terminate and its loop exited");
        assert!(
            !sched.is_registered(id),
            "AgentRuntime Drop deregistered the agent"
        );
    }

    #[test]
    fn scheduler_terminate_and_cancel_control_reach_a_live_agent() {
        use std::time::{Duration, Instant};

        let sched = AgentScheduler::global();
        let id = fresh_agent_id();

        let tb = std::thread::spawn(move || {
            let mut rt = Runtime::new_with_agent_id(id);
            assert!(AgentScheduler::global().register(rt.agent_handle()));
            rt.run_host_completions_until(|_rt| false, Duration::from_millis(25));
            let cancelled = rt.agent_cancelled_tokens();
            AgentScheduler::global().deregister(id);
            (rt.agent_terminate_requested(), cancelled)
        });

        let deadline = Instant::now() + Duration::from_secs(5);
        while !sched.is_registered(id) && Instant::now() < deadline {
            std::thread::yield_now();
        }
        assert!(sched.is_registered(id), "agent must come up");

        assert!(sched.cancel(id, 42));
        assert!(sched.terminate(id));

        let (terminated, cancelled) = tb.join().unwrap();
        assert!(terminated, "agent observed the Terminate control message");
        assert!(
            cancelled.contains(&42),
            "agent recorded the Cancel(42) control message before exiting"
        );
        assert!(!sched.is_registered(id), "agent exited its loop and deregistered");
    }

    #[test]
    fn two_live_agents_run_concurrently_and_isolated() {
        use std::sync::atomic::AtomicUsize;
        use std::time::Duration;

        let sched = AgentScheduler::global();
        let ida = fresh_agent_id();
        let idb = fresh_agent_id();

        let recv_a = Arc::new(AtomicUsize::new(0));
        let recv_b = Arc::new(AtomicUsize::new(0));
        let thread_a = Arc::new(Mutex::new(None::<std::thread::ThreadId>));
        let thread_b = Arc::new(Mutex::new(None::<std::thread::ThreadId>));
        const N_A: usize = 64;
        const N_B: usize = 40;

        let spawn_agent = |id: AgentId,
                           recv: Arc<AtomicUsize>,
                           thread_rec: Arc<Mutex<Option<std::thread::ThreadId>>>,
                           target: usize| {
            std::thread::spawn(move || {
                let mut rt = Runtime::new_with_agent_id(id);
                assert!(AgentScheduler::global().register(rt.agent_handle()));
                rt.run_host_completions_until(
                    |_rt| recv.load(Ordering::SeqCst) >= target,
                    Duration::from_millis(50),
                );
                let _ = thread_rec;
                AgentScheduler::global().deregister(id);
                std::thread::current().id()
            })
        };
        let ta = spawn_agent(ida, recv_a.clone(), thread_a.clone(), N_A);
        let tb = spawn_agent(idb, recv_b.clone(), thread_b.clone(), N_B);

        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while (!sched.is_registered(ida) || !sched.is_registered(idb))
            && std::time::Instant::now() < deadline
        {
            std::thread::yield_now();
        }

        for i in 0..N_A.max(N_B) {
            if i < N_A {
                let (r, t) = (recv_a.clone(), thread_a.clone());
                sched.post_completion(
                    ida,
                    Box::new(move |_rt: &mut Runtime| {
                        r.fetch_add(1, Ordering::SeqCst);
                        *t.lock().unwrap() = Some(std::thread::current().id());
                    }),
                );
            }
            if i < N_B {
                let (r, t) = (recv_b.clone(), thread_b.clone());
                sched.post_completion(
                    idb,
                    Box::new(move |_rt: &mut Runtime| {
                        r.fetch_add(1, Ordering::SeqCst);
                        *t.lock().unwrap() = Some(std::thread::current().id());
                    }),
                );
            }
        }

        let owner_a = ta.join().unwrap();
        let owner_b = tb.join().unwrap();

        assert_eq!(recv_a.load(Ordering::SeqCst), N_A);
        assert_eq!(recv_b.load(Ordering::SeqCst), N_B);

        let (exec_a, exec_b) = (
            thread_a.lock().unwrap().unwrap(),
            thread_b.lock().unwrap().unwrap(),
        );
        assert_eq!(exec_a, owner_a);
        assert_eq!(exec_b, owner_b);
        assert_ne!(exec_a, exec_b);
        assert_ne!(exec_a, std::thread::current().id());
    }

    #[test]
    fn one_agent_termination_does_not_disturb_another() {
        use std::sync::atomic::AtomicUsize;
        use std::time::Duration;

        let sched = AgentScheduler::global();
        let ida = fresh_agent_id();
        let idb = fresh_agent_id();

        let done_a = Arc::new(AtomicUsize::new(0));
        let da = done_a.clone();
        let ta = std::thread::spawn(move || {
            let mut rt = Runtime::new_with_agent_id(ida);
            assert!(AgentScheduler::global().register(rt.agent_handle()));
            rt.run_host_completions_until(
                |_rt| da.load(Ordering::SeqCst) >= 1,
                Duration::from_millis(50),
            );
            AgentScheduler::global().deregister(ida);
        });

        let recv_b = Arc::new(AtomicUsize::new(0));
        let rb = recv_b.clone();
        let tb = std::thread::spawn(move || {
            let mut rt = Runtime::new_with_agent_id(idb);
            assert!(AgentScheduler::global().register(rt.agent_handle()));
            rt.run_host_completions_until(
                |_rt| rb.load(Ordering::SeqCst) >= 2,
                Duration::from_millis(50),
            );
            AgentScheduler::global().deregister(idb);
        });

        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while (!sched.is_registered(ida) || !sched.is_registered(idb))
            && std::time::Instant::now() < deadline
        {
            std::thread::yield_now();
        }

        let r1 = recv_b.clone();
        sched.post_completion(
            idb,
            Box::new(move |_rt: &mut Runtime| {
                r1.fetch_add(1, Ordering::SeqCst);
            }),
        );
        done_a.store(1, Ordering::SeqCst);
        sched.lookup(ida).map(|h| h.wake());
        ta.join().unwrap();

        assert!(!sched.is_registered(ida));
        assert!(!sched.post_completion(ida, Box::new(|_rt: &mut Runtime| {})));

        let r2 = recv_b.clone();
        sched.post_completion(
            idb,
            Box::new(move |_rt: &mut Runtime| {
                r2.fetch_add(1, Ordering::SeqCst);
            }),
        );
        tb.join().unwrap();
        assert_eq!(recv_b.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn terminating_agent_drops_pending_completions_survivor_gc_intact() {
        std::thread::Builder::new()
            .name("agent-scheduler-teardown-gc-test".into())
            .stack_size(16 * 1024 * 1024)
            .spawn(|| {
                use std::sync::atomic::{AtomicBool, AtomicUsize};
                use std::time::Duration;

                struct DropFlag(Arc<AtomicBool>);
                impl Drop for DropFlag {
                    fn drop(&mut self) {
                        self.0.store(true, Ordering::SeqCst);
                    }
                }

                let sched = AgentScheduler::global();
                let ida = fresh_agent_id();
                let idb = fresh_agent_id();

                let release_a = Arc::new(AtomicBool::new(false));
                let a_registered = Arc::new(AtomicBool::new(false));
                let pending_job_dropped = Arc::new(AtomicBool::new(false));
                let release_for_a = release_a.clone();
                let reg_for_a = a_registered.clone();
                let ta = std::thread::Builder::new()
                    .name("agent-scheduler-teardown-a".into())
                    .stack_size(16 * 1024 * 1024)
                    .spawn(move || {

                        let rt = Runtime::new_with_agent_id(ida);
                        assert!(AgentScheduler::global().register(rt.agent_handle()));
                        reg_for_a.store(true, Ordering::SeqCst);
                        while !release_for_a.load(Ordering::SeqCst) {
                            std::thread::yield_now();
                        }
                        AgentScheduler::global().deregister(ida);
                        drop(rt);
                    })
                    .expect("spawn teardown agent A");

                let b_done = Arc::new(AtomicBool::new(false));
                let b_root_after_gc = Arc::new(AtomicUsize::new(0));
                let done_for_b = b_done.clone();
                let tb = std::thread::Builder::new()
                    .name("agent-scheduler-teardown-b".into())
                    .stack_size(16 * 1024 * 1024)
                    .spawn(move || {
                        let mut rt = Runtime::new_with_agent_id(idb);
                        rt.install_intrinsics();
                        rt.run_script(
                            "globalThis.keep = { v: 13 }; \
                 for (let i = 0; i < 250; i++) { const g = { i, a: [i] }; }",
                            "file://scheduler-survivor-gc-init",
                        )
                        .expect("B init");
                        assert!(AgentScheduler::global().register(rt.agent_handle()));
                        rt.run_host_completions_until(
                            |_rt| done_for_b.load(Ordering::SeqCst),
                            Duration::from_millis(50),
                        );
                        AgentScheduler::global().deregister(idb);
                    })
                    .expect("spawn teardown survivor B");

                let deadline = std::time::Instant::now() + Duration::from_secs(5);
                while (!a_registered.load(Ordering::SeqCst) || !sched.is_registered(idb))
                    && std::time::Instant::now() < deadline
                {
                    std::thread::yield_now();
                }
                assert!(a_registered.load(Ordering::SeqCst), "agent A must register");
                assert!(sched.is_registered(idb), "agent B must register");

                let flag = DropFlag(pending_job_dropped.clone());
                assert!(sched.post_completion(
                    ida,
                    Box::new(move |_rt: &mut Runtime| {

                        let _keep_until_drop = &flag;
                        panic!("dead agent must not run queued completion");
                    }),
                ));
                assert_eq!(
                    sched.lookup(ida).unwrap().pending_completions(),
                    1,
                    "A must have one queued inbound completion before teardown"
                );

                release_a.store(true, Ordering::SeqCst);
                sched.lookup(ida).map(|h| h.wake());
                ta.join().unwrap();
                assert!(!sched.is_registered(ida));
                assert!(
                    pending_job_dropped.load(Ordering::SeqCst),
                    "terminating agent must drop queued completions and their captures"
                );
                assert!(
                    !sched.post_completion(ida, Box::new(|_rt: &mut Runtime| {})),
                    "dead agent must refuse new inbound completions"
                );

                let (root_after_gc, done) = (b_root_after_gc.clone(), b_done.clone());
                assert!(sched.post_completion(
                    idb,
                    Box::new(move |rt: &mut Runtime| {
                        let _ = rt.collect();
                        let g = rt.global_object.unwrap();
                        let keep_v =
                            if let crate::value::Value::Object(o) = rt.object_get(g, "keep") {
                                rt.object_get(o, "v")
                            } else {
                                crate::value::Value::Undefined
                            };
                        if let crate::value::Value::Number(n) = keep_v {
                            root_after_gc.store(n as usize, Ordering::SeqCst);
                        }
                        done.store(true, Ordering::SeqCst);
                    }),
                ));

                tb.join().unwrap();
                assert_eq!(
                    b_root_after_gc.load(Ordering::SeqCst),
                    13,
                    "surviving agent's own roots must survive GC after sibling teardown"
                );
            })
            .expect("spawn large-stack teardown GC test")
            .join()
            .expect("large-stack teardown GC test");
    }
}
