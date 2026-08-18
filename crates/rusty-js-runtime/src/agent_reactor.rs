
use crate::interp::{Runtime, RuntimeError};
use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReactorPoll {

    Ready,

    NotReady,

    Closed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceRole {

    Js,

    Host,

    Completion,

    Control,

    Other,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TurnOutcome {

    pub js_progressed: bool,

    pub host_progressed: bool,

    pub host_idle: bool,

    pub any: bool,
}

pub trait ReactorSource {
    fn poll(&mut self, rt: &mut Runtime) -> Result<ReactorPoll, RuntimeError>;
}

impl<F> ReactorSource for F
where
    F: FnMut(&mut Runtime) -> Result<ReactorPoll, RuntimeError>,
{
    fn poll(&mut self, rt: &mut Runtime) -> Result<ReactorPoll, RuntimeError> {
        self(rt)
    }
}

struct RegisteredSource {
    source: Box<dyn ReactorSource>,
    role: SourceRole,
}

#[derive(Default)]
pub struct AgentReactor {
    sources: Vec<RegisteredSource>,
}

impl AgentReactor {
    pub fn new() -> Self {
        Self {
            sources: Vec::new(),
        }
    }

    pub fn register(&mut self, source: Box<dyn ReactorSource>) {
        self.register_with_role(source, SourceRole::Other);
    }

    pub fn register_with_role(&mut self, source: Box<dyn ReactorSource>, role: SourceRole) {
        self.sources.push(RegisteredSource { source, role });
    }

    pub fn is_empty(&self) -> bool {
        self.sources.is_empty()
    }

    pub fn len(&self) -> usize {
        self.sources.len()
    }

    pub fn turn_detailed(&mut self, rt: &mut Runtime) -> Result<TurnOutcome, RuntimeError> {
        let mut out = TurnOutcome::default();
        let mut i = 0;
        while i < self.sources.len() {
            let role = self.sources[i].role;
            match self.sources[i].source.poll(rt)? {
                ReactorPoll::Ready => {
                    out.any = true;
                    match role {
                        SourceRole::Js => out.js_progressed = true,
                        SourceRole::Host => out.host_progressed = true,
                        _ => {}
                    }
                    i += 1;
                }
                ReactorPoll::NotReady => {
                    if role == SourceRole::Host {
                        out.host_idle = true;
                    }
                    i += 1;
                }
                ReactorPoll::Closed => {
                    self.sources.remove(i);
                }
            }
        }
        Ok(out)
    }

    pub fn turn(&mut self, rt: &mut Runtime, park_timeout: Duration) -> Result<bool, RuntimeError> {
        let observed = rt.agent_wake_generation();
        let outcome = self.turn_detailed(rt)?;
        if !outcome.any {
            rt.set_agent_blocked(true);
            rt.wait_agent_wake_timeout(observed, park_timeout);
            rt.set_agent_blocked(false);
        }
        Ok(outcome.any)
    }

    pub fn run_main(&mut self, rt: &mut Runtime) -> Result<(), RuntimeError> {
        let max_iterations = 10_000_000usize;
        let mut iter = 0;
        loop {
            if rt.agent_terminate_requested() {
                return Ok(());
            }
            iter += 1;
            if iter > max_iterations {
                return Err(RuntimeError::TypeError(
                    "run_main: max-iteration safety bound exceeded".into(),
                ));
            }
            let outcome = self.turn_detailed(rt)?;
            if rt.io_wait_tick || outcome.host_progressed {
                rt.io_wait_tick = false;
                iter = 0;
            }
            if !outcome.any {
                return Ok(());
            }
        }
    }
}

pub fn host_completion_inbox_source() -> Box<dyn ReactorSource> {
    Box::new(|rt: &mut Runtime| {
        Ok(if rt.drain_host_completion_inbox() > 0 {
            ReactorPoll::Ready
        } else {
            ReactorPoll::NotReady
        })
    })
}

pub fn host_poll_io_source() -> Box<dyn ReactorSource> {
    Box::new(|rt: &mut Runtime| {
        Ok(if rt.run_host_poll_hook()? {
            ReactorPoll::Ready
        } else {
            ReactorPoll::NotReady
        })
    })
}

pub fn js_job_queue_source() -> Box<dyn ReactorSource> {
    Box::new(|rt: &mut Runtime| {

        Ok(if rt.agent_run_tick()? {
            ReactorPoll::Ready
        } else {
            ReactorPoll::NotReady
        })
    })
}

pub fn agent_control_source() -> Box<dyn ReactorSource> {
    Box::new(|rt: &mut Runtime| {
        rt.drain_agent_control();
        Ok(ReactorPoll::NotReady)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::time::Instant;

    fn fresh_id() -> crate::interp::AgentId {
        use std::sync::atomic::AtomicU64;
        static N: AtomicU64 = AtomicU64::new(0x9000_0000);
        crate::interp::AgentId::from_raw(N.fetch_add(1, Ordering::Relaxed))
    }

    #[test]
    fn reactor_parks_when_idle_and_dispatches_on_wake() {

        struct FlagSource {
            ready: Arc<AtomicBool>,
            dispatched: Arc<AtomicUsize>,
            done: bool,
        }
        impl ReactorSource for FlagSource {
            fn poll(&mut self, _rt: &mut Runtime) -> Result<ReactorPoll, RuntimeError> {
                if self.done {
                    return Ok(ReactorPoll::Closed);
                }
                Ok(if self.ready.load(Ordering::SeqCst) {
                    self.dispatched.fetch_add(1, Ordering::SeqCst);
                    self.done = true;
                    ReactorPoll::Ready
                } else {
                    ReactorPoll::NotReady
                })
            }
        }

        let mut rt = Runtime::new_with_agent_id(fresh_id());
        let ready = Arc::new(AtomicBool::new(false));
        let dispatched = Arc::new(AtomicUsize::new(0));
        let turns = Arc::new(AtomicUsize::new(0));

        let mut reactor = AgentReactor::new();
        reactor.register(Box::new(FlagSource {
            ready: ready.clone(),
            dispatched: dispatched.clone(),
            done: false,
        }));

        let handle = rt.agent_handle();
        let r2 = ready.clone();
        let producer = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(40));
            r2.store(true, Ordering::SeqCst);
            handle.wake();
        });

        let deadline = Instant::now() + Duration::from_secs(5);
        while dispatched.load(Ordering::SeqCst) == 0 && Instant::now() < deadline {
            reactor.turn(&mut rt, Duration::from_millis(200)).unwrap();
            turns.fetch_add(1, Ordering::SeqCst);
        }
        producer.join().unwrap();

        assert_eq!(dispatched.load(Ordering::SeqCst), 1, "source dispatched exactly once");

        reactor.turn(&mut rt, Duration::from_millis(1)).unwrap();
        assert!(reactor.is_empty(), "closed source retired from the reactor");

        assert!(
            turns.load(Ordering::SeqCst) < 50,
            "reactor parked rather than busy-polled (turns={})",
            turns.load(Ordering::SeqCst)
        );
    }

    #[test]
    fn reactor_drives_host_completion_inbox_and_control_for_a_live_agent() {
        use crate::agent_scheduler::AgentScheduler;
        use std::sync::atomic::AtomicUsize;

        let sched = AgentScheduler::global();
        let id = fresh_id();
        let received = Arc::new(AtomicUsize::new(0));
        let done = Arc::new(AtomicBool::new(false));
        let (r2, d2) = (received.clone(), done.clone());

        let tb = std::thread::spawn(move || {
            let mut rt = Runtime::new_with_agent_id(id);
            assert!(AgentScheduler::global().register(rt.agent_handle()));
            let mut reactor = AgentReactor::new();
            reactor.register(agent_control_source());
            reactor.register(host_completion_inbox_source());
            while !d2.load(Ordering::SeqCst) && !rt.agent_terminate_requested() {
                reactor.turn(&mut rt, Duration::from_millis(50)).unwrap();
            }
            let terminated = rt.agent_terminate_requested();
            AgentScheduler::global().deregister(id);
            terminated
        });

        let deadline = Instant::now() + Duration::from_secs(5);
        while !sched.is_registered(id) && Instant::now() < deadline {
            std::thread::yield_now();
        }
        assert!(sched.is_registered(id), "agent B must come up");

        const N: usize = 20;
        for _ in 0..N {
            let r = r2.clone();
            assert!(sched.post_completion(
                id,
                Box::new(move |_rt: &mut Runtime| {
                    r.fetch_add(1, Ordering::SeqCst);
                }),
            ));
        }
        while received.load(Ordering::SeqCst) < N && Instant::now() < deadline {
            std::thread::yield_now();
        }
        assert_eq!(
            received.load(Ordering::SeqCst),
            N,
            "the reactor delivered every cross-agent completion"
        );

        assert!(sched.terminate(id));

        sched.lookup(id).map(|h| h.wake());
        let terminated = tb.join().unwrap();
        assert!(terminated, "reactor-driven agent honored Terminate control");
        assert!(!sched.is_registered(id));
    }

    #[test]
    fn reactor_drives_a_full_agent_js_jobs_and_completions_together() {

        use crate::job_queue::HostEnqueuePhase;
        use std::cell::Cell;
        use std::rc::Rc;

        let mut rt = Runtime::new_with_agent_id(fresh_id());

        let js_ran = Rc::new(Cell::new(0u32));
        let jr = js_ran.clone();
        rt.enqueue_host_phase_rooted(
            HostEnqueuePhase::HostCompletionMacrotask,
            "reactor-js-macro",
            Vec::new(),
            move |_rt| {
                jr.set(jr.get() + 1);
                Ok(())
            },
        );

        let comp_ran = Arc::new(AtomicUsize::new(0));
        let cr = comp_ran.clone();
        assert!(rt.agent_handle().post_completion(Box::new(move |_rt: &mut Runtime| {
            cr.fetch_add(1, Ordering::SeqCst);
        })));

        let mut reactor = AgentReactor::new();
        reactor.register(js_job_queue_source());
        reactor.register(host_completion_inbox_source());

        for _ in 0..16 {
            if !reactor.turn(&mut rt, std::time::Duration::from_millis(1)).unwrap() {
                break;
            }
        }

        assert_eq!(js_ran.get(), 1, "reactor pumped the JS job queue (macrotask ran)");
        assert_eq!(
            comp_ran.load(Ordering::SeqCst),
            1,
            "reactor drained the cross-agent completion"
        );
    }

    #[test]
    fn reactor_js_source_has_run_to_completion_turn_parity() {

        let mut rt = Runtime::new_with_agent_id(fresh_id());
        rt.record_async_hook_fatal_exception(RuntimeError::TypeError("fatal hook".to_string()));
        let mut reactor = AgentReactor::new();
        reactor.register(js_job_queue_source());
        assert!(
            matches!(
                reactor.turn(&mut rt, Duration::from_millis(1)),
                Err(RuntimeError::AsyncHookFatal(_))
            ),
            "async-hook fatal propagates through the reactor JS source"
        );

        let mut rt2 = Runtime::new_with_agent_id(fresh_id());
        rt2.microtask_budget_used = 9_999_999;
        assert!(!rt2.agent_run_tick().unwrap(), "no jobs -> no work this turn");
        assert_eq!(
            rt2.microtask_budget_used, 0,
            "agent_run_tick resets the per-turn microtask budget"
        );
    }

    #[test]
    fn reactor_drives_host_poll_hook_chain_and_propagates_errors() {

        let calls = Arc::new(AtomicUsize::new(0));
        let mut rt = Runtime::new_with_agent_id(fresh_id());

        let c = calls.clone();
        rt.install_host_hook(crate::module::HostHook::PollIo(Box::new(move |_rt| {

            Ok(c.fetch_add(1, Ordering::SeqCst) < 3)
        })));
        let mut reactor = AgentReactor::new();
        reactor.register(host_poll_io_source());
        assert!(reactor.turn(&mut rt, Duration::from_millis(1)).unwrap());
        assert!(reactor.turn(&mut rt, Duration::from_millis(1)).unwrap());
        assert!(reactor.turn(&mut rt, Duration::from_millis(1)).unwrap());

        assert!(!reactor.turn(&mut rt, Duration::from_millis(1)).unwrap());
        assert_eq!(calls.load(Ordering::SeqCst), 4);

        rt.install_host_hook(crate::module::HostHook::PollIo(Box::new(|_rt| {
            Err(RuntimeError::TypeError("host boom".to_string()))
        })));
        let mut reactor2 = AgentReactor::new();
        reactor2.register(host_poll_io_source());
        assert!(
            reactor2.turn(&mut rt, Duration::from_millis(1)).is_err(),
            "host poll error propagates through the reactor turn"
        );
    }

    #[test]
    fn reactor_dispatches_multiple_ready_sources_in_order() {
        let order = Arc::new(std::sync::Mutex::new(Vec::<u32>::new()));
        let mut rt = Runtime::new_with_agent_id(fresh_id());
        let mut reactor = AgentReactor::new();
        for tag in [1u32, 2, 3] {
            let order = order.clone();
            let mut fired = false;
            reactor.register(Box::new(move |_rt: &mut Runtime| {
                if fired {
                    return Ok(ReactorPoll::Closed);
                }
                order.lock().unwrap().push(tag);
                fired = true;
                Ok(ReactorPoll::Ready)
            }));
        }

        assert!(reactor.turn(&mut rt, Duration::from_millis(1)).unwrap());
        assert_eq!(*order.lock().unwrap(), vec![1, 2, 3]);

        reactor.turn(&mut rt, Duration::from_millis(1)).unwrap();
        assert!(reactor.is_empty());
    }

    #[test]
    fn run_main_exits_when_fully_idle() {
        let mut rt = Runtime::new_with_agent_id(fresh_id());
        let mut reactor = AgentReactor::new();
        reactor.register_with_role(
            Box::new(|_rt: &mut Runtime| Ok(ReactorPoll::NotReady)),
            SourceRole::Js,
        );
        reactor.register_with_role(
            Box::new(|_rt: &mut Runtime| Ok(ReactorPoll::NotReady)),
            SourceRole::Host,
        );

        reactor.run_main(&mut rt).expect("idle main-agent exits cleanly");
    }

    #[test]
    fn run_main_stays_while_host_progresses_then_exits() {
        let mut rt = Runtime::new_with_agent_id(fresh_id());
        let mut reactor = AgentReactor::new();
        let remaining = Arc::new(AtomicUsize::new(5));
        let r = remaining.clone();
        reactor.register_with_role(
            Box::new(move |_rt: &mut Runtime| {
                Ok(if r.fetch_sub(1, Ordering::SeqCst) > 1 {
                    ReactorPoll::Ready
                } else {

                    ReactorPoll::NotReady
                })
            }),
            SourceRole::Host,
        );
        reactor.run_main(&mut rt).expect("exits after host drains");

        assert_eq!(remaining.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn run_main_bound_trips_on_hot_js_loop() {
        let mut rt = Runtime::new_with_agent_id(fresh_id());
        let mut reactor = AgentReactor::new();

        reactor.register_with_role(
            Box::new(|_rt: &mut Runtime| Ok(ReactorPoll::Ready)),
            SourceRole::Js,
        );
        let outcome = reactor.run_main(&mut rt);
        assert!(
            matches!(outcome, Err(RuntimeError::TypeError(_))),
            "hot JS loop must trip the safety bound, got {outcome:?}"
        );
    }

    #[test]
    fn run_main_host_progress_resets_bound_survives_past_limit() {
        let mut rt = Runtime::new_with_agent_id(fresh_id());
        let mut reactor = AgentReactor::new();

        let remaining = Arc::new(AtomicUsize::new(10_000_005));
        let r = remaining.clone();
        reactor.register_with_role(
            Box::new(move |_rt: &mut Runtime| {
                Ok(if r.fetch_sub(1, Ordering::SeqCst) > 1 {
                    ReactorPoll::Ready
                } else {
                    ReactorPoll::NotReady
                })
            }),
            SourceRole::Host,
        );
        reactor
            .run_main(&mut rt)
            .expect("host-progress resets the bound; idle server survives past the limit");
        assert_eq!(remaining.load(Ordering::SeqCst), 0);
    }
}
