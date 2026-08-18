
use crate::interp::{FrameSnapshot, Runtime, RuntimeError};
use crate::value::{ObjectRef, PromiseReactionHandler, Value};
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;

static STRUCTURED_PROMISE_REACTION_JOBS: AtomicU64 = AtomicU64::new(0);
static EMPTY_ASYNC_CONTEXT_FAST_JOBS: AtomicU64 = AtomicU64::new(0);
static JOB_STATIC_CALLS: AtomicU64 = AtomicU64::new(0);
static JOB_STATIC_TOTAL_NS: AtomicU64 = AtomicU64::new(0);
static JOB_STATIC_KIND_NS: AtomicU64 = AtomicU64::new(0);
static JOB_STATIC_ROOTS: AtomicU64 = AtomicU64::new(0);
static JOB_STATIC_SPLIT_CALLS: AtomicU64 = AtomicU64::new(0);
static JOB_STATIC_SPLIT_PUSH_NS: AtomicU64 = AtomicU64::new(0);
static JOB_STATIC_SPLIT_KIND_NS: AtomicU64 = AtomicU64::new(0);
static JOB_STATIC_SPLIT_POP_NS: AtomicU64 = AtomicU64::new(0);
static JOB_STATIC_SPLIT_CONTEXT_NS: AtomicU64 = AtomicU64::new(0);
static EVENT_LOOP_MICROTASKS: AtomicU64 = AtomicU64::new(0);
static EVENT_LOOP_NEXTTICKS: AtomicU64 = AtomicU64::new(0);
static EVENT_LOOP_MACROTASKS: AtomicU64 = AtomicU64::new(0);
static EVENT_LOOP_POLLS: AtomicU64 = AtomicU64::new(0);
static EVENT_LOOP_POLL_PROGRESS: AtomicU64 = AtomicU64::new(0);

fn event_loop_counters_enabled() -> bool {
    static CACHED: OnceLock<bool> = OnceLock::new();
    *CACHED.get_or_init(|| {
        std::env::var("CRUFT_EVENT_LOOP_COUNTERS")
            .or_else(|_| std::env::var("CRUFT_PROFILE_EVENT_LOOP"))
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false)
    })
}

fn event_loop_counters_verbose() -> bool {
    static CACHED: OnceLock<bool> = OnceLock::new();
    *CACHED.get_or_init(|| {
        std::env::var("CRUFT_EVENT_LOOP_COUNTERS_VERBOSE")
            .or_else(|_| std::env::var("CRUFT_PROFILE_EVENT_LOOP_VERBOSE"))
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false)
    })
}

fn event_loop_job_trace_enabled() -> bool {
    static CACHED: OnceLock<bool> = OnceLock::new();
    *CACHED.get_or_init(|| {
        std::env::var("CRUFT_EVENT_LOOP_JOB_TRACE")
            .or_else(|_| std::env::var("CRUFT_PROFILE_EVENT_LOOP_JOBS"))
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false)
    })
}

pub(crate) fn check_microtask_budget(
    rt: &mut Runtime,
    label: &'static str,
) -> Result<(), RuntimeError> {
    let Some(limit) = rt.microtask_budget_limit else {
        return Ok(());
    };
    rt.microtask_budget_used = rt.microtask_budget_used.saturating_add(1);
    if rt.microtask_budget_used > limit {
        return Err(RuntimeError::TypeError(format!(
            "microtask budget exceeded: limit={limit} attempted={} label={label}",
            rt.microtask_budget_used
        )));
    }
    Ok(())
}

fn trace_event_loop_job(
    phase: &str,
    label: &'static str,
    kind: &'static str,
    roots_len: usize,
    detail: impl AsRef<str>,
) {
    if !event_loop_job_trace_enabled() {
        return;
    }
    eprintln!(
        "[event-loop-job-trace] phase={phase} label={label} kind={kind} roots={roots_len} {}",
        detail.as_ref()
    );
}

fn record_event_loop_job(
    phase: &'static str,
    label: &'static str,
    nexttick_len: usize,
    microtask_len: usize,
    macrotask_len: usize,
) {
    if !event_loop_counters_enabled() {
        return;
    }
    let n = match phase {
        "nexttick" => EVENT_LOOP_NEXTTICKS.fetch_add(1, Ordering::Relaxed) + 1,
        "microtask" => EVENT_LOOP_MICROTASKS.fetch_add(1, Ordering::Relaxed) + 1,
        "macrotask" => EVENT_LOOP_MACROTASKS.fetch_add(1, Ordering::Relaxed) + 1,
        _ => 0,
    };
    if event_loop_counters_verbose() || n <= 16 || n % 1024 == 0 {
        eprintln!(
            "[event-loop-counters] phase={phase} count={n} label={label} nexttick_len={nexttick_len} microtask_len={microtask_len} macrotask_len={macrotask_len}"
        );
    }
}

fn record_event_loop_poll(
    progressed: bool,
    nexttick_len: usize,
    microtask_len: usize,
    macrotask_len: usize,
) {
    if !event_loop_counters_enabled() {
        return;
    }
    let n = EVENT_LOOP_POLLS.fetch_add(1, Ordering::Relaxed) + 1;
    let progressed_total = if progressed {
        EVENT_LOOP_POLL_PROGRESS.fetch_add(1, Ordering::Relaxed) + 1
    } else {
        EVENT_LOOP_POLL_PROGRESS.load(Ordering::Relaxed)
    };
    if event_loop_counters_verbose() || n <= 16 || n % 1024 == 0 {
        eprintln!(
            "[event-loop-counters] phase=poll count={n} progressed={progressed} progressed_total={progressed_total} nexttick_len={nexttick_len} microtask_len={microtask_len} macrotask_len={macrotask_len}"
        );
    }
}

fn job_kind_label(kind: &JobKind) -> &'static str {
    match kind {
        JobKind::Closure(_) => "closure",
        JobKind::QueueMicrotaskCallback(_) => "queueMicrotaskCallback",
        JobKind::PromiseReaction { .. } => "promiseReaction",
        JobKind::AsyncAwaitContinuation { .. } => "asyncAwaitContinuation",
    }
}

fn promise_reaction_job_counters_enabled() -> bool {
    static CACHED: OnceLock<bool> = OnceLock::new();
    *CACHED.get_or_init(|| {
        std::env::var("CRUFT_PROMISE_REACTION_JOB_COUNTERS")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false)
    })
}

fn record_structured_promise_reaction_job(handler: &Option<PromiseReactionHandler>) {
    if !promise_reaction_job_counters_enabled() {
        return;
    }
    let n = STRUCTURED_PROMISE_REACTION_JOBS.fetch_add(1, Ordering::Relaxed) + 1;
    if n <= 16 || n % 1024 == 0 {
        let handler_kind = match handler {
            Some(PromiseReactionHandler::Callable(_)) => "callable",
            Some(PromiseReactionHandler::LazyArrow(_)) => "lazy-arrow",
            Some(PromiseReactionHandler::LazyArrowOneCell(_)) => "lazy-arrow-one-cell",
            Some(PromiseReactionHandler::AsyncAwaitContinuation { .. }) => "async-await",
            None => "empty",
        };
        eprintln!(
            "[promise-reaction-job-counters] structured_jobs={} handler={}",
            n, handler_kind
        );
    }
}

fn empty_async_context_fast_enabled() -> bool {
    static CACHED: OnceLock<bool> = OnceLock::new();
    *CACHED.get_or_init(|| {
        std::env::var("CRUFT_JOB_EMPTY_ASYNC_CONTEXT_FAST")
            .map(|v| !(v == "0" || v.eq_ignore_ascii_case("false")))
            .unwrap_or(true)
    })
}

fn empty_async_context_counters_enabled() -> bool {
    static CACHED: OnceLock<bool> = OnceLock::new();
    *CACHED.get_or_init(|| {
        std::env::var("CRUFT_JOB_EMPTY_ASYNC_CONTEXT_COUNTERS")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false)
    })
}

fn record_empty_async_context_fast_job(label: &'static str) {
    if !empty_async_context_counters_enabled() {
        return;
    }
    let n = EMPTY_ASYNC_CONTEXT_FAST_JOBS.fetch_add(1, Ordering::Relaxed) + 1;
    if n <= 16 || n % 1024 == 0 {
        eprintln!("[job-queue-counters] empty_async_context_fast_jobs={n} label={label}");
    }
}

fn job_static_counters_enabled() -> bool {
    static CACHED: OnceLock<bool> = OnceLock::new();
    *CACHED.get_or_init(|| {
        std::env::var("CRUFT_JOB_STATIC_COUNTERS")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false)
    })
}

fn job_static_split_counters_enabled() -> bool {
    static CACHED: OnceLock<bool> = OnceLock::new();
    *CACHED.get_or_init(|| {
        std::env::var("CRUFT_JOB_STATIC_SPLIT_COUNTERS")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false)
    })
}

fn record_job_static_split(
    label: &'static str,
    push_ns: u64,
    kind_ns: u64,
    pop_ns: u64,
    context_ns: u64,
) {
    if !job_static_split_counters_enabled() {
        return;
    }
    let n = JOB_STATIC_SPLIT_CALLS.fetch_add(1, Ordering::Relaxed) + 1;
    let push_total = JOB_STATIC_SPLIT_PUSH_NS.fetch_add(push_ns, Ordering::Relaxed) + push_ns;
    let kind_total = JOB_STATIC_SPLIT_KIND_NS.fetch_add(kind_ns, Ordering::Relaxed) + kind_ns;
    let pop_total = JOB_STATIC_SPLIT_POP_NS.fetch_add(pop_ns, Ordering::Relaxed) + pop_ns;
    let context_total =
        JOB_STATIC_SPLIT_CONTEXT_NS.fetch_add(context_ns, Ordering::Relaxed) + context_ns;
    if n <= 16 || n % 1024 == 0 {
        eprintln!(
            "[job-static-split-counters] calls={n} label={label} avg_push_ns={} avg_kind_ns={} avg_pop_ns={} avg_context_ns={}",
            push_total / n,
            kind_total / n,
            pop_total / n,
            context_total / n
        );
    }
}

fn record_job_static(
    label: &'static str,
    roots_len: usize,
    kind_ns: u64,
    total_start: Option<std::time::Instant>,
) {
    let Some(total_start) = total_start else {
        return;
    };
    let n = JOB_STATIC_CALLS.fetch_add(1, Ordering::Relaxed) + 1;
    let total_ns = total_start.elapsed().as_nanos() as u64;
    let total_total = JOB_STATIC_TOTAL_NS.fetch_add(total_ns, Ordering::Relaxed) + total_ns;
    let kind_total = JOB_STATIC_KIND_NS.fetch_add(kind_ns, Ordering::Relaxed) + kind_ns;
    let roots_total =
        JOB_STATIC_ROOTS.fetch_add(roots_len as u64, Ordering::Relaxed) + roots_len as u64;
    if n <= 16 || n % 1024 == 0 {
        eprintln!(
            "[job-static-counters] calls={n} label={label} avg_total_ns={} avg_kind_ns={} avg_roots={}",
            total_total / n,
            kind_total / n,
            roots_total / n
        );
    }
}

pub struct Job {

    pub label: &'static str,
    pub kind: JobKind,

    pub async_context: HashMap<ObjectRef, Value>,

    pub roots: Vec<ObjectRef>,
}

pub enum JobKind {

    Closure(Box<dyn FnOnce(&mut Runtime) -> Result<(), RuntimeError>>),
    QueueMicrotaskCallback(Value),
    PromiseReaction {
        handler: Option<PromiseReactionHandler>,
        value: Value,
        chain: ObjectRef,
        cap_resolve: Option<Value>,
        cap_reject: Option<Value>,
        is_rejected: bool,
    },
    AsyncAwaitContinuation {
        chain: ObjectRef,
        promise: ObjectRef,
        snapshot: Box<FrameSnapshot>,
        value: Value,
        is_rejected: bool,
    },
}

impl std::fmt::Debug for Job {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Job {{ label: {:?} }}", self.label)
    }
}

#[derive(Default)]
pub struct JobQueue {

    pub(crate) microtasks: VecDeque<Job>,

    pub(crate) nexttick: VecDeque<Job>,

    pub(crate) macrotasks: VecDeque<Job>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostEnqueuePhase {
    HostCompletionMacrotask,
    MessageDeliveryMacrotask,
    EventSemanticMacrotask,
    TimerCallbackMacrotask,
    AtomicsWaitAsyncPollMacrotask,
}

impl JobQueue {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn microtask_count(&self) -> usize {
        self.microtasks.len()
    }
    pub fn macrotask_count(&self) -> usize {
        self.macrotasks.len()
    }
    pub fn is_empty(&self) -> bool {
        self.microtasks.is_empty() && self.macrotasks.is_empty() && self.nexttick.is_empty()
    }

    pub fn collect_roots(&self, roots: &mut Vec<ObjectRef>) {
        for job in self
            .microtasks
            .iter()
            .chain(self.macrotasks.iter())
            .chain(self.nexttick.iter())
        {
            roots.extend(job.roots.iter().copied());
            roots.extend(job.async_context.keys().copied());
            for value in job.async_context.values() {
                if let Value::Object(id) = value {
                    roots.push(*id);
                }
            }
        }
    }
}

impl Runtime {

    pub fn enqueue_microtask<F>(&mut self, label: &'static str, f: F)
    where
        F: FnOnce(&mut Runtime) -> Result<(), RuntimeError> + 'static,
    {
        self.enqueue_microtask_rooted(label, Vec::new(), f);
    }

    pub fn enqueue_microtask_rooted<F>(&mut self, label: &'static str, roots: Vec<ObjectRef>, f: F)
    where
        F: FnOnce(&mut Runtime) -> Result<(), RuntimeError> + 'static,
    {
        self.job_queue.microtasks.push_back(Job {
            label,
            kind: JobKind::Closure(Box::new(f)),
            async_context: self.als_context.clone(),
            roots,
        });
    }

    pub fn enqueue_microtask_rooted_with_async_context<F>(
        &mut self,
        label: &'static str,
        roots: Vec<ObjectRef>,
        async_context: HashMap<ObjectRef, Value>,
        f: F,
    ) where
        F: FnOnce(&mut Runtime) -> Result<(), RuntimeError> + 'static,
    {
        self.job_queue.microtasks.push_back(Job {
            label,
            kind: JobKind::Closure(Box::new(f)),
            async_context,
            roots,
        });
    }

    pub fn enqueue_queue_microtask_callback(&mut self, cb: Value, roots: Vec<ObjectRef>) {
        self.job_queue.microtasks.push_back(Job {
            label: "queueMicrotask",
            kind: JobKind::QueueMicrotaskCallback(cb),
            async_context: self.als_context.clone(),
            roots,
        });
    }

    pub(crate) fn enqueue_promise_reaction_job(
        &mut self,
        roots: Vec<ObjectRef>,
        handler: Option<PromiseReactionHandler>,
        value: Value,
        chain: ObjectRef,
        cap_resolve: Option<Value>,
        cap_reject: Option<Value>,
        is_rejected: bool,
    ) {
        record_structured_promise_reaction_job(&handler);
        self.job_queue.microtasks.push_back(Job {
            label: "PromiseReactionJob",
            kind: JobKind::PromiseReaction {
                handler,
                value,
                chain,
                cap_resolve,
                cap_reject,
                is_rejected,
            },
            async_context: self.als_context.clone(),
            roots,
        });
    }

    pub(crate) fn enqueue_async_await_continuation_job(
        &mut self,
        roots: Vec<ObjectRef>,
        chain: ObjectRef,
        promise: ObjectRef,
        snapshot: Box<FrameSnapshot>,
        value: Value,
        is_rejected: bool,
    ) {
        let async_context = snapshot.als_context.clone();
        self.job_queue.microtasks.push_back(Job {
            label: "AsyncAwaitContinuationJob",
            kind: JobKind::AsyncAwaitContinuation {
                chain,
                promise,
                snapshot,
                value,
                is_rejected,
            },
            async_context,
            roots,
        });
    }

    pub fn enqueue_nexttick_rooted<F>(&mut self, label: &'static str, roots: Vec<ObjectRef>, f: F)
    where
        F: FnOnce(&mut Runtime) -> Result<(), RuntimeError> + 'static,
    {
        self.job_queue.nexttick.push_back(Job {
            label,
            kind: JobKind::Closure(Box::new(f)),
            async_context: self.als_context.clone(),
            roots,
        });
    }

    pub fn enqueue_macrotask<F>(&mut self, label: &'static str, f: F)
    where
        F: FnOnce(&mut Runtime) -> Result<(), RuntimeError> + 'static,
    {
        self.enqueue_macrotask_rooted(label, Vec::new(), f);
    }

    pub fn enqueue_macrotask_rooted<F>(&mut self, label: &'static str, roots: Vec<ObjectRef>, f: F)
    where
        F: FnOnce(&mut Runtime) -> Result<(), RuntimeError> + 'static,
    {
        self.job_queue.macrotasks.push_back(Job {
            label,
            kind: JobKind::Closure(Box::new(f)),
            async_context: self.als_context.clone(),
            roots,
        });
    }

    pub fn enqueue_macrotask_rooted_with_async_context<F>(
        &mut self,
        label: &'static str,
        roots: Vec<ObjectRef>,
        async_context: HashMap<ObjectRef, Value>,
        f: F,
    ) where
        F: FnOnce(&mut Runtime) -> Result<(), RuntimeError> + 'static,
    {
        self.job_queue.macrotasks.push_back(Job {
            label,
            kind: JobKind::Closure(Box::new(f)),
            async_context,
            roots,
        });
    }

    pub fn enqueue_host_phase_rooted<F>(
        &mut self,
        phase: HostEnqueuePhase,
        label: &'static str,
        roots: Vec<ObjectRef>,
        f: F,
    ) where
        F: FnOnce(&mut Runtime) -> Result<(), RuntimeError> + 'static,
    {
        match phase {
            HostEnqueuePhase::HostCompletionMacrotask
            | HostEnqueuePhase::MessageDeliveryMacrotask
            | HostEnqueuePhase::EventSemanticMacrotask
            | HostEnqueuePhase::TimerCallbackMacrotask
            | HostEnqueuePhase::AtomicsWaitAsyncPollMacrotask => {
                self.enqueue_macrotask_rooted(label, roots, f);
            }
        }
    }

    pub fn enqueue_host_phase_rooted_with_async_context<F>(
        &mut self,
        phase: HostEnqueuePhase,
        label: &'static str,
        roots: Vec<ObjectRef>,
        async_context: HashMap<ObjectRef, Value>,
        f: F,
    ) where
        F: FnOnce(&mut Runtime) -> Result<(), RuntimeError> + 'static,
    {
        match phase {
            HostEnqueuePhase::HostCompletionMacrotask
            | HostEnqueuePhase::MessageDeliveryMacrotask
            | HostEnqueuePhase::EventSemanticMacrotask
            | HostEnqueuePhase::TimerCallbackMacrotask
            | HostEnqueuePhase::AtomicsWaitAsyncPollMacrotask => {
                self.enqueue_macrotask_rooted_with_async_context(label, roots, async_context, f);
            }
        }
    }

    pub fn als_get_store(&self, id: ObjectRef) -> Value {
        self.als_context
            .get(&id)
            .cloned()
            .unwrap_or(Value::Undefined)
    }

    pub fn als_has_store(&self, id: ObjectRef) -> bool {
        self.als_context.contains_key(&id)
    }

    pub fn als_clear_store(&mut self, id: ObjectRef) {
        self.als_context.remove(&id);
    }

    pub fn als_context_snapshot(&self) -> HashMap<ObjectRef, Value> {
        self.als_context.clone()
    }

    pub fn als_context_replace(
        &mut self,
        context: HashMap<ObjectRef, Value>,
    ) -> HashMap<ObjectRef, Value> {
        std::mem::replace(&mut self.als_context, context)
    }

    pub fn als_set_store(&mut self, id: ObjectRef, store: Value) {
        self.als_context.insert(id, store);
    }

    pub fn run_to_completion(&mut self) -> Result<(), RuntimeError> {

        let max_iterations = 10_000_000usize;
        let mut iter = 0;
        loop {
            if let Some(value) = self.take_async_hook_fatal_exception() {
                return Err(RuntimeError::AsyncHookFatal(value));
            }
            iter += 1;
            if iter > max_iterations {
                return Err(RuntimeError::TypeError(
                    "run_to_completion: max-iteration safety bound exceeded".into(),
                ));
            }

            self.microtask_budget_used = 0;

            let did_work = pump_one_tick(self)?;

            if self.io_wait_tick {
                self.io_wait_tick = false;
                iter = 0;
            }

            if did_work {
                continue;
            }

            if let Some(poll) = self.host_hooks.poll_io.take() {
                let progressed = poll(self)?;
                self.host_hooks.poll_io = Some(poll);
                record_event_loop_poll(
                    progressed,
                    self.job_queue.nexttick.len(),
                    self.job_queue.microtasks.len(),
                    self.job_queue.macrotasks.len(),
                );
                if progressed {

                    iter = 0;
                    continue;
                }
            }
            return Ok(());
        }
    }
}

pub fn run_job_static(rt: &mut Runtime, job: Job) -> Result<(), RuntimeError> {
    let total_start = job_static_counters_enabled().then(std::time::Instant::now);
    let split_enabled = job_static_split_counters_enabled();
    let Job {
        label,
        kind,
        async_context,
        roots,
    } = job;
    let roots_len = roots.len();
    let kind_label = job_kind_label(&kind);
    let trace_start = event_loop_job_trace_enabled().then(std::time::Instant::now);
    trace_event_loop_job(
        "begin",
        label,
        kind_label,
        roots_len,
        format!(
            "async_context_len={} active_job_roots={}",
            async_context.len(),
            rt.active_job_roots.len()
        ),
    );
    if empty_async_context_fast_enabled() && async_context.is_empty() && rt.als_context.is_empty() {
        record_empty_async_context_fast_job(label);
        let push_start = split_enabled.then(std::time::Instant::now);
        rt.active_job_roots.push(roots);
        let push_ns = push_start
            .as_ref()
            .map(|start| start.elapsed().as_nanos() as u64)
            .unwrap_or(0);
        let kind_start =
            (split_enabled || job_static_counters_enabled()).then(std::time::Instant::now);
        let result = run_job_kind(rt, kind);
        let kind_ns = kind_start
            .as_ref()
            .map(|start| start.elapsed().as_nanos() as u64)
            .unwrap_or(0);
        let pop_start = split_enabled.then(std::time::Instant::now);
        rt.active_job_roots.pop();
        let pop_ns = pop_start
            .as_ref()
            .map(|start| start.elapsed().as_nanos() as u64)
            .unwrap_or(0);
        record_job_static_split(label, push_ns, kind_ns, pop_ns, 0);
        record_job_static(label, roots_len, kind_ns, total_start);
        trace_event_loop_job(
            "end",
            label,
            kind_label,
            roots_len,
            format!(
                "ok={} elapsed_ms={:.3} active_job_roots={}",
                result.is_ok(),
                trace_start
                    .as_ref()
                    .map(|start| start.elapsed().as_secs_f64() * 1000.0)
                    .unwrap_or(0.0),
                rt.active_job_roots.len()
            ),
        );
        return result;
    }
    let context_start = split_enabled.then(std::time::Instant::now);
    let saved = std::mem::replace(&mut rt.als_context, async_context);
    let mut context_ns = context_start
        .as_ref()
        .map(|start| start.elapsed().as_nanos() as u64)
        .unwrap_or(0);
    let push_start = split_enabled.then(std::time::Instant::now);
    rt.active_job_roots.push(roots);
    let push_ns = push_start
        .as_ref()
        .map(|start| start.elapsed().as_nanos() as u64)
        .unwrap_or(0);
    let kind_start = (split_enabled || job_static_counters_enabled()).then(std::time::Instant::now);
    let result = run_job_kind(rt, kind);
    let kind_ns = kind_start
        .as_ref()
        .map(|start| start.elapsed().as_nanos() as u64)
        .unwrap_or(0);
    let pop_start = split_enabled.then(std::time::Instant::now);
    rt.active_job_roots.pop();
    let pop_ns = pop_start
        .as_ref()
        .map(|start| start.elapsed().as_nanos() as u64)
        .unwrap_or(0);
    let restore_start = split_enabled.then(std::time::Instant::now);
    rt.als_context = saved;
    context_ns += restore_start
        .as_ref()
        .map(|start| start.elapsed().as_nanos() as u64)
        .unwrap_or(0);
    record_job_static_split(label, push_ns, kind_ns, pop_ns, context_ns);
    record_job_static(label, roots_len, kind_ns, total_start);
    trace_event_loop_job(
        "end",
        label,
        kind_label,
        roots_len,
        format!(
            "ok={} elapsed_ms={:.3} active_job_roots={}",
            result.is_ok(),
            trace_start
                .as_ref()
                .map(|start| start.elapsed().as_secs_f64() * 1000.0)
                .unwrap_or(0.0),
            rt.active_job_roots.len()
        ),
    );
    result
}

fn run_job_kind(rt: &mut Runtime, kind: JobKind) -> Result<(), RuntimeError> {
    match kind {
        JobKind::Closure(f) => f(rt),
        JobKind::QueueMicrotaskCallback(cb) => {
            let _ = rt.call_function(cb, Value::Undefined, Vec::new());
            Ok(())
        }
        JobKind::PromiseReaction {
            handler,
            value,
            chain,
            cap_resolve,
            cap_reject,
            is_rejected,
        } => crate::promise::run_reaction_job(
            rt,
            handler,
            value,
            chain,
            cap_resolve,
            cap_reject,
            is_rejected,
        ),
        JobKind::AsyncAwaitContinuation {
            chain,
            promise,
            snapshot,
            value,
            is_rejected,
        } => crate::promise::run_async_await_continuation_job(
            rt,
            chain,
            promise,
            snapshot,
            value,
            is_rejected,
        ),
    }
}

pub fn pump_one_microtask(rt: &mut Runtime) -> Result<bool, RuntimeError> {

    if let Some(job) = rt.job_queue.nexttick.pop_front() {
        run_job_static(rt, job)?;
        return Ok(true);
    }
    let Some(job) = rt.job_queue.microtasks.pop_front() else {
        return Ok(false);
    };
    check_microtask_budget(rt, job.label)?;
    run_job_static(rt, job)?;
    Ok(true)
}

pub fn drain_microtask_checkpoint(rt: &mut Runtime) -> Result<(), RuntimeError> {
    loop {
        while let Some(job) = rt.job_queue.nexttick.pop_front() {
            run_job_static(rt, job)?;
        }
        while let Some(job) = rt.job_queue.microtasks.pop_front() {
            check_microtask_budget(rt, job.label)?;
            run_job_static(rt, job)?;
        }
        if rt.job_queue.nexttick.is_empty() {
            break;
        }
    }
    Ok(())
}

pub fn pump_one_tick(rt: &mut Runtime) -> Result<bool, RuntimeError> {

    let mut did_work = false;

    loop {
        while let Some(job) = rt.job_queue.nexttick.pop_front() {
            let label = job.label;
            record_event_loop_job(
                "nexttick",
                label,
                rt.job_queue.nexttick.len(),
                rt.job_queue.microtasks.len(),
                rt.job_queue.macrotasks.len(),
            );
            run_job_static(rt, job)?;
            if let Some(value) = rt.take_async_hook_fatal_exception() {
                return Err(RuntimeError::AsyncHookFatal(value));
            }
            rt.maybe_collect_between_jobs();
            did_work = true;
        }
        while let Some(job) = rt.job_queue.microtasks.pop_front() {
            let label = job.label;
            check_microtask_budget(rt, label)?;
            record_event_loop_job(
                "microtask",
                label,
                rt.job_queue.nexttick.len(),
                rt.job_queue.microtasks.len(),
                rt.job_queue.macrotasks.len(),
            );
            run_job_static(rt, job)?;
            if let Some(value) = rt.take_async_hook_fatal_exception() {
                return Err(RuntimeError::AsyncHookFatal(value));
            }
            rt.maybe_collect_between_jobs();
            did_work = true;
        }
        if rt.job_queue.nexttick.is_empty() {
            break;
        }
    }

    if let Some(job) = rt.job_queue.macrotasks.pop_front() {
        let label = job.label;
        record_event_loop_job(
            "macrotask",
            label,
            rt.job_queue.nexttick.len(),
            rt.job_queue.microtasks.len(),
            rt.job_queue.macrotasks.len(),
        );
        run_job_static(rt, job)?;
        if let Some(value) = rt.take_async_hook_fatal_exception() {
            return Err(RuntimeError::AsyncHookFatal(value));
        }
        rt.maybe_collect_between_jobs();
        did_work = true;
    }
    Ok(did_work)
}

#[cfg(test)]
mod tests {
    use super::HostEnqueuePhase;
    use crate::interp::RuntimeError;
    use crate::Runtime;
    use std::cell::RefCell;
    use std::rc::Rc;

    #[test]
    fn pump_one_tick_drains_whole_microtask_checkpoint_before_nexttick() {
        let mut rt = Runtime::new();
        let trace = Rc::new(RefCell::new(Vec::new()));

        let t = trace.clone();
        rt.enqueue_microtask("m1", move |rt| {
            t.borrow_mut().push("m1");
            let t2 = t.clone();
            rt.enqueue_nexttick_rooted("n", Vec::new(), move |_rt| {
                t2.borrow_mut().push("n");
                Ok(())
            });
            Ok(())
        });
        let t = trace.clone();
        rt.enqueue_microtask("m2", move |_rt| {
            t.borrow_mut().push("m2");
            Ok(())
        });

        super::pump_one_tick(&mut rt).expect("pump_one_tick drains to quiescence");

        assert_eq!(&*trace.borrow(), &["m1", "m2", "n"]);
    }

    #[test]
    fn pump_one_tick_aborts_turn_on_per_job_async_hook_fatal() {
        let mut rt = Runtime::new();
        let ran_second = Rc::new(RefCell::new(false));

        rt.enqueue_nexttick_rooted("fatal-setter", Vec::new(), move |rt| {
            rt.pending_async_hook_fatal = Some(crate::value::Value::Undefined);
            Ok(())
        });
        let r = ran_second.clone();
        rt.enqueue_nexttick_rooted("should-not-run", Vec::new(), move |_rt| {
            *r.borrow_mut() = true;
            Ok(())
        });

        let outcome = super::pump_one_tick(&mut rt);
        assert!(
            matches!(outcome, Err(RuntimeError::AsyncHookFatal(_))),
            "per-job fatal must abort the turn, got {outcome:?}"
        );
        assert!(
            !*ran_second.borrow(),
            "the job after a fatal must NOT run (IC-6)"
        );
    }

    #[test]
    fn run_to_completion_delegates_canonical_drainer_across_turns() {
        let mut rt = Runtime::new();
        let trace = Rc::new(RefCell::new(Vec::new()));

        let t = trace.clone();
        rt.enqueue_macrotask("mac", move |rt| {
            t.borrow_mut().push("mac");
            let t1 = t.clone();
            rt.enqueue_microtask("m1", move |rt| {
                t1.borrow_mut().push("m1");
                let t2 = t1.clone();
                rt.enqueue_nexttick_rooted("n", Vec::new(), move |_rt| {
                    t2.borrow_mut().push("n");
                    Ok(())
                });
                Ok(())
            });
            let t2b = t.clone();
            rt.enqueue_microtask("m2", move |_rt| {
                t2b.borrow_mut().push("m2");
                Ok(())
            });
            Ok(())
        });

        rt.run_to_completion().expect("delegated loop runs to completion and exits");

        assert_eq!(&*trace.borrow(), &["mac", "m1", "m2", "n"]);
    }

    #[test]
    fn host_completion_phase_preserves_tick_microtask_before_macrotask_order() {
        let mut rt = Runtime::new();
        let trace = Rc::new(RefCell::new(Vec::new()));

        let t = trace.clone();
        rt.enqueue_host_phase_rooted(
            HostEnqueuePhase::HostCompletionMacrotask,
            "host-completion",
            Vec::new(),
            move |_rt| {
                t.borrow_mut().push("host");
                Ok(())
            },
        );

        let t = trace.clone();
        rt.enqueue_microtask("microtask", move |_rt| {
            t.borrow_mut().push("microtask");
            Ok(())
        });

        let t = trace.clone();
        rt.enqueue_nexttick_rooted("nexttick", Vec::new(), move |_rt| {
            t.borrow_mut().push("nexttick");
            Ok(())
        });

        rt.run_to_completion()
            .expect("host phase ordering should run to completion");
        assert_eq!(&*trace.borrow(), &["nexttick", "microtask", "host"]);
    }

    #[test]
    fn message_delivery_phase_preserves_tick_microtask_before_macrotask_order() {
        let mut rt = Runtime::new();
        let trace = Rc::new(RefCell::new(Vec::new()));

        let t = trace.clone();
        rt.enqueue_host_phase_rooted(
            HostEnqueuePhase::MessageDeliveryMacrotask,
            "message-delivery",
            Vec::new(),
            move |_rt| {
                t.borrow_mut().push("message");
                Ok(())
            },
        );

        let t = trace.clone();
        rt.enqueue_microtask("microtask", move |_rt| {
            t.borrow_mut().push("microtask");
            Ok(())
        });

        let t = trace.clone();
        rt.enqueue_nexttick_rooted("nexttick", Vec::new(), move |_rt| {
            t.borrow_mut().push("nexttick");
            Ok(())
        });

        rt.run_to_completion()
            .expect("message phase ordering should run to completion");
        assert_eq!(&*trace.borrow(), &["nexttick", "microtask", "message"]);
    }

    #[test]
    fn event_semantic_phase_preserves_tick_microtask_before_macrotask_order() {
        let mut rt = Runtime::new();
        let trace = Rc::new(RefCell::new(Vec::new()));

        let t = trace.clone();
        rt.enqueue_host_phase_rooted(
            HostEnqueuePhase::EventSemanticMacrotask,
            "event-semantic",
            Vec::new(),
            move |_rt| {
                t.borrow_mut().push("event");
                Ok(())
            },
        );

        let t = trace.clone();
        rt.enqueue_microtask("microtask", move |_rt| {
            t.borrow_mut().push("microtask");
            Ok(())
        });

        let t = trace.clone();
        rt.enqueue_nexttick_rooted("nexttick", Vec::new(), move |_rt| {
            t.borrow_mut().push("nexttick");
            Ok(())
        });

        rt.run_to_completion()
            .expect("event semantic phase ordering should run to completion");
        assert_eq!(&*trace.borrow(), &["nexttick", "microtask", "event"]);
    }

    #[test]
    fn timer_phase_preserves_explicit_async_context() {
        use crate::value::Object;
        let mut rt = Runtime::new();
        let als_id = rt.alloc_object(Object::new_ordinary());
        let store = rt.alloc_object(Object::new_ordinary());
        let observed = Rc::new(RefCell::new(None));
        let mut async_context = std::collections::HashMap::new();
        async_context.insert(als_id, crate::Value::Object(store));

        let out = observed.clone();
        rt.enqueue_host_phase_rooted_with_async_context(
            HostEnqueuePhase::TimerCallbackMacrotask,
            "timer callback",
            Vec::new(),
            async_context,
            move |rt| {
                *out.borrow_mut() = Some(rt.als_get_store(als_id));
                Ok(())
            },
        );

        rt.run_to_completion()
            .expect("timer phase with captured context should run");
        assert_eq!(*observed.borrow(), Some(crate::Value::Object(store)));
    }

    #[test]
    fn atomics_waitasync_poll_phase_preserves_tick_microtask_before_macrotask_order() {
        let mut rt = Runtime::new();
        let trace = Rc::new(RefCell::new(Vec::new()));

        let t = trace.clone();
        rt.enqueue_host_phase_rooted(
            HostEnqueuePhase::AtomicsWaitAsyncPollMacrotask,
            "atomics-waitasync-poll",
            Vec::new(),
            move |_rt| {
                t.borrow_mut().push("atomics");
                Ok(())
            },
        );

        let t = trace.clone();
        rt.enqueue_microtask("microtask", move |_rt| {
            t.borrow_mut().push("microtask");
            Ok(())
        });

        let t = trace.clone();
        rt.enqueue_nexttick_rooted("nexttick", Vec::new(), move |_rt| {
            t.borrow_mut().push("nexttick");
            Ok(())
        });

        rt.run_to_completion()
            .expect("Atomics waitAsync poll phase should run");
        assert_eq!(&*trace.borrow(), &["nexttick", "microtask", "atomics"]);
    }
}
