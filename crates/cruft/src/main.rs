
use cruft::install_cruft_host;
use rusty_js_runtime::{Runtime, RuntimeError, Value};
use std::process::ExitCode;

mod agent;
mod caps_closure;
mod repl_edit;

use agent::cli::run_agent_subcommand;

#[used]
static _NAPI_RETAIN: usize = rusty_js_runtime::napi::NAPI_KEEPALIVE.len();

mod alloctrack {
    use std::alloc::{GlobalAlloc, Layout, System};
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering::Relaxed};
    const NB: usize = 48;
    static ENABLED: AtomicBool = AtomicBool::new(false);
    static LIVE: AtomicUsize = AtomicUsize::new(0);
    static PEAK: AtomicUsize = AtomicUsize::new(0);
    static SNAP_AT: AtomicUsize = AtomicUsize::new(1 << 20);
    static LIVE_CNT: [AtomicUsize; NB] = [const { AtomicUsize::new(0) }; NB];
    static LIVE_SZ: [AtomicUsize; NB] = [const { AtomicUsize::new(0) }; NB];
    static PEAK_CNT: [AtomicUsize; NB] = [const { AtomicUsize::new(0) }; NB];
    static PEAK_SZ: [AtomicUsize; NB] = [const { AtomicUsize::new(0) }; NB];

    #[inline]
    fn bucket(sz: usize) -> usize {
        if sz == 0 {
            0
        } else {
            ((usize::BITS - sz.leading_zeros()) as usize).min(NB - 1)
        }
    }
    #[inline]
    fn note_alloc(sz: usize) {
        LIVE_CNT[bucket(sz)].fetch_add(1, Relaxed);
        LIVE_SZ[bucket(sz)].fetch_add(sz, Relaxed);
        let now = LIVE.fetch_add(sz, Relaxed) + sz;
        if now > SNAP_AT.load(Relaxed) {
            SNAP_AT.store(now + now / 32, Relaxed);
            PEAK.store(now, Relaxed);
            for i in 0..NB {
                PEAK_CNT[i].store(LIVE_CNT[i].load(Relaxed), Relaxed);
                PEAK_SZ[i].store(LIVE_SZ[i].load(Relaxed), Relaxed);
            }
        }
    }
    #[inline]
    fn note_dealloc(sz: usize) {
        LIVE_CNT[bucket(sz)].fetch_sub(1, Relaxed);
        LIVE_SZ[bucket(sz)].fetch_sub(sz, Relaxed);
        LIVE.fetch_sub(sz, Relaxed);
    }
    pub fn enable() {
        ENABLED.store(true, Relaxed);
    }
    pub fn dump() {
        if !ENABLED.load(Relaxed) {
            return;
        }
        eprintln!(
            "[alloc-track] PEAK live requested heap = {} MB   CURRENT(exit) = {} MB",
            PEAK.load(Relaxed) / 1048576,
            LIVE.load(Relaxed) / 1048576
        );
        eprintln!("[alloc-track] size-class  |  count@PEAK  MB@PEAK  |  count@EXIT  MB@EXIT:");
        for b in 0..NB {
            let pc = PEAK_CNT[b].load(Relaxed);
            let cc = LIVE_CNT[b].load(Relaxed);
            if pc == 0 && cc == 0 {
                continue;
            }
            let lo = if b == 0 { 0 } else { 1usize << (b - 1) };
            let hi = (1usize << b) - 1;
            eprintln!(
                "  [{:>11}..{:<11}] | {:>10} {:>6} MB | {:>10} {:>6} MB",
                lo,
                hi,
                pc,
                PEAK_SZ[b].load(Relaxed) / 1048576,
                cc,
                LIVE_SZ[b].load(Relaxed) / 1048576
            );
        }
    }
    pub struct Guard;
    impl Drop for Guard {
        fn drop(&mut self) {
            dump();
        }
    }
    pub struct Tracking;
    unsafe impl GlobalAlloc for Tracking {
        unsafe fn alloc(&self, l: Layout) -> *mut u8 {
            let p = System.alloc(l);
            if !p.is_null() && ENABLED.load(Relaxed) {
                note_alloc(l.size());
            }
            p
        }
        unsafe fn alloc_zeroed(&self, l: Layout) -> *mut u8 {
            let p = System.alloc_zeroed(l);
            if !p.is_null() && ENABLED.load(Relaxed) {
                note_alloc(l.size());
            }
            p
        }
        unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
            if ENABLED.load(Relaxed) {
                note_dealloc(l.size());
            }
            System.dealloc(p, l);
        }
        unsafe fn realloc(&self, p: *mut u8, l: Layout, new: usize) -> *mut u8 {
            let np = System.realloc(p, l, new);
            if !np.is_null() && ENABLED.load(Relaxed) {
                note_dealloc(l.size());
                note_alloc(new);
            }
            np
        }
    }
}
#[global_allocator]
static ALLOC: alloctrack::Tracking = alloctrack::Tracking;

fn format_thrown(rt: &Runtime, v: &Value) -> String {
    match v {
        Value::String(s) => format!("Thrown: {}", s),
        Value::Object(id) => {
            let mut name = match rt.object_get(*id, "name") {
                Value::String(s) => s.as_str().to_string(),
                _ => String::new(),
            };

            if name.is_empty() {
                if let Value::Object(ctor) = rt.object_get(*id, "constructor") {
                    if let Value::String(s) = rt.object_get(ctor, "name") {
                        name = s.as_str().to_string();
                    }
                }
            }
            let message = match rt.object_get(*id, "message") {
                Value::String(s) => s.as_str().to_string(),
                _ => String::new(),
            };
            if !name.is_empty() && !message.is_empty() {
                format!("Thrown: {}: {}", name, message)
            } else if !message.is_empty() {
                format!("Thrown: {}", message)
            } else if !name.is_empty() {
                format!("Thrown: {}", name)
            } else {
                format!("Thrown: {:?}", v)
            }
        }
        _ => format!("Thrown: {:?}", v),
    }
}

fn thrown_stack_or_format_with_mode(
    rt: &Runtime,
    v: &Value,
    mode: DiagnosticDisclosureMode,
) -> String {
    if let Value::Object(id) = v {
        if let Some(desc) = rt.obj(*id).get_own("__error_stack__") {
            if let Value::String(stack) = &desc.value {
                let stack = stack.as_str();
                if !stack.trim().is_empty() {
                    return mode.redact_error_text(&node_resolution_stack_projection(stack));
                }
            }
        }
        if let Value::String(stack) = rt.object_get(*id, "stack") {
            let stack = stack.as_str();
            if !stack.trim().is_empty() {
                return mode.redact_error_text(&node_resolution_stack_projection(stack));
            }
        }
    }
    let rendered = format_thrown(rt, v);
    rendered
        .strip_prefix("Thrown: ")
        .unwrap_or(rendered.as_str())
        .to_string()
}

fn thrown_stack_or_format(rt: &Runtime, v: &Value) -> String {
    thrown_stack_or_format_with_mode(rt, v, DiagnosticDisclosureMode::current())
}

fn is_node_assertion_error(rt: &Runtime, v: &Value) -> bool {
    let Value::Object(id) = v else {
        return false;
    };
    matches!(rt.object_get(*id, "name"), Value::String(s) if s.as_str() == "AssertionError")
        && matches!(rt.object_get(*id, "code"), Value::String(s) if s.as_str() == "ERR_ASSERTION")
}

fn async_hook_fatal_format(rt: &Runtime, v: &Value) -> String {
    match v {
        Value::Null => "Error: null".to_string(),
        Value::Symbol(s) => {
            let desc = s
                .as_str()
                .rsplit_once(':')
                .map(|(_, desc)| desc)
                .unwrap_or_else(|| s.as_str());
            format!("Error: Symbol({desc})")
        }
        _ => thrown_stack_or_format(rt, v),
    }
}

fn node_resolution_stack_projection(stack: &str) -> String {
    if let Some(rest) = stack.strip_prefix("Error: Cannot find module '") {
        return format!("Error [ERR_MODULE_NOT_FOUND]: Cannot find module '{rest}");
    }
    stack.to_string()
}

fn dispatch_uncaught_exception(rt: &mut Runtime, thrown: &Value) -> Option<ExitCode> {
    if !cruft::process::has_process_listener(rt, "uncaughtException") {
        return None;
    }
    cruft::process::emit_process_event(
        rt,
        "uncaughtException",
        vec![thrown.clone(), Value::Undefined],
    );
    let code = cruft::process::current_exit_code(rt).unwrap_or(0);
    cruft::process::emit_process_event(rt, "exit", vec![Value::Number(code as f64)]);
    Some(ExitCode::from(
        (cruft::process::current_exit_code(rt).unwrap_or(0) & 0xff) as u8,
    ))
}

fn dispatch_uncaught_runtime_error(rt: &mut Runtime, err: &RuntimeError) -> Option<ExitCode> {
    let (name, message) = match err {
        RuntimeError::TypeError(message) => ("TypeError", message.as_str()),
        RuntimeError::RangeError(message) => ("RangeError", message.as_str()),
        RuntimeError::ReferenceError(message) => ("ReferenceError", message.as_str()),
        RuntimeError::SyntaxError(message) => ("SyntaxError", message.as_str()),
        _ => return None,
    };
    let thrown = rusty_js_runtime::intrinsics::make_error_instance(rt, name, message)
        .map(Value::Object)
        .unwrap_or_else(|| {
            Value::String(std::rc::Rc::new(rusty_js_runtime::value::JsString::from(
                format!("{name}: {message}"),
            )))
        });
    dispatch_uncaught_exception(rt, &thrown)
}

fn run_test262_sweep(rest: &[String]) -> ExitCode {
    use std::io::{Read, Write};
    #[cfg(unix)]
    use std::os::unix::process::ExitStatusExt;
    use std::process::{Command, Stdio};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{mpsc, Arc, Mutex};
    use std::time::{Duration, Instant};

    let mut paths_file: Option<String> = None;
    let mut runner = std::env::var("T262_RUNNER")
        .ok()
        .unwrap_or_else(|| "scripts/test262/runner-full.mjs".to_string());
    let mut harness = std::env::var("T262_HARNESS_DIR").ok();
    let mut out_file: Option<String> = None;
    let mut progress_file: Option<String> = None;
    let mut summary_file: Option<String> = None;
    let mut parallel: usize = if cfg!(target_os = "macos") { 1 } else { 8 };
    let mut timeout_secs: u64 = 10;
    let mut memory_min_free_percent: u32 = if cfg!(target_os = "macos") { 15 } else { 0 };
    let mut memory_poll_secs: u64 = 15;
    let mut progress_every: usize = 100;
    let mut it = rest.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--paths" => paths_file = it.next().cloned(),
            "--runner" => {
                if let Some(r) = it.next() {
                    runner = r.clone();
                }
            }
            "--harness" => harness = it.next().cloned(),
            "--out" => out_file = it.next().cloned(),
            "--progress" => progress_file = it.next().cloned(),
            "--summary" => summary_file = it.next().cloned(),
            "--parallel" => {
                if let Some(n) = it.next().and_then(|s| s.parse().ok()) {
                    parallel = n;
                }
            }
            "--timeout-secs" => {
                if let Some(n) = it.next().and_then(|s| s.parse().ok()) {
                    timeout_secs = n;
                }
            }
            "--memory-min-free-percent" => {
                if let Some(n) = it.next().and_then(|s| s.parse().ok()) {
                    memory_min_free_percent = n;
                }
            }
            "--memory-poll-secs" => {
                if let Some(n) = it.next().and_then(|s| s.parse().ok()) {
                    memory_poll_secs = n;
                }
            }
            "--progress-every" => {
                if let Some(n) = it.next().and_then(|s| s.parse().ok()) {
                    progress_every = n;
                }
            }
            _ => {}
        }
    }
    let paths_file = match paths_file {
        Some(p) => p,
        None => {
            eprintln!("cruft test262 sweep: --paths <file> is required");
            return ExitCode::from(64);
        }
    };
    let listing = match std::fs::read_to_string(&paths_file) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("cruft test262 sweep: cannot read {}: {}", paths_file, e);
            return ExitCode::from(66);
        }
    };
    let paths: Vec<String> = listing
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .map(String::from)
        .collect();

    let exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("cruft test262 sweep: cannot resolve self exe: {}", e);
            return ExitCode::from(70);
        }
    };
    let timeout = Duration::from_secs(timeout_secs);
    let memory_poll = Duration::from_secs(memory_poll_secs);

    let classify = move |path: &str, stdout: &str, rc: i32| -> String {
        for line in stdout.lines() {
            if line.contains("\"status\":\"PASS\"")
                || line.contains("\"status\":\"FAIL\"")
                || line.contains("\"status\":\"SKIP\"")
            {
                return line.to_string();
            }
        }
        let reason = match rc {
            124 => format!(
                "timeout (>{}s wall-clock cap; THP-3 deterministic budget pending)",
                timeout_secs
            ),
            134 => "abort (SIGABRT; e.g. stack-overflow / proper-tail-calls)".to_string(),
            139 => "segfault (SIGSEGV)".to_string(),
            101 => "panic (engine aborted, rust panic)".to_string(),
            70 => "eval-error exit (rc=70, no verdict emitted)".to_string(),
            1 => "error exit (rc=1, no verdict emitted)".to_string(),
            0 => "silent-exit (clean exit, no verdict emitted)".to_string(),
            n => format!("abnormal-exit (rc={}, no verdict emitted)", n),
        };
        format!(
            "{{\"path\":\"{}\",\"status\":\"FAIL\",\"reason\":\"{}\"}}",
            path, reason
        )
    };
    let memory_free_percent = move || -> Option<u32> {
        if !cfg!(target_os = "macos") || memory_min_free_percent == 0 {
            return None;
        }
        let output = Command::new("memory_pressure")
            .arg("-Q")
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        let text = String::from_utf8_lossy(&output.stdout);
        let marker = "System-wide memory free percentage:";
        let rest = text.split(marker).nth(1)?;
        let number = rest
            .trim_start()
            .chars()
            .take_while(|c| c.is_ascii_digit())
            .collect::<String>();
        number.parse::<u32>().ok()
    };
    let wait_for_memory = move || {
        if memory_min_free_percent == 0 || !cfg!(target_os = "macos") {
            return;
        }
        loop {
            match memory_free_percent() {
                Some(free) if free < memory_min_free_percent => {
                    eprintln!(
                        "cruft test262 sweep: memory throttle free={}%% < {}%%; sleeping {}s",
                        free,
                        memory_min_free_percent,
                        memory_poll.as_secs()
                    );
                    std::thread::sleep(memory_poll);
                }
                _ => return,
            }
        }
    };

    let one = move |path: &str| -> String {
        wait_for_memory();
        let mut cmd = Command::new(&exe);
        cmd.args(["test262", "run", path, "--runner", &runner]);
        if let Some(h) = &harness {
            cmd.env("T262_HARNESS_DIR", h);
        }
        cmd.stdout(Stdio::piped()).stderr(Stdio::null());
        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(_) => return classify(path, "", 70),
        };
        let deadline = Instant::now() + timeout;
        let mut timed_out = false;
        let status = loop {
            match child.try_wait() {
                Ok(Some(s)) => break Some(s),
                Ok(None) => {
                    if Instant::now() >= deadline {
                        let _ = child.kill();
                        let _ = child.wait();
                        timed_out = true;
                        break None;
                    }
                    std::thread::sleep(Duration::from_millis(15));
                }
                Err(_) => break None,
            }
        };
        let mut out = String::new();
        if let Some(mut so) = child.stdout.take() {
            let _ = so.read_to_string(&mut out);
        }
        let rc: i32 = if timed_out {
            124
        } else if let Some(s) = status {
            if let Some(code) = s.code() {
                code
            } else {

                #[cfg(unix)]
                {
                    s.signal().map(|sig| 128 + sig).unwrap_or(-1)
                }
                #[cfg(not(unix))]
                {
                    -1
                }
            }
        } else {
            -1
        };
        classify(path, &out, rc)
    };

    let idx = Arc::new(AtomicUsize::new(0));
    let paths = Arc::new(paths);
    let count = paths.len();
    let one = Arc::new(one);
    let (tx, rx) = mpsc::channel::<String>();
    let mut handles = Vec::new();
    for _ in 0..parallel.max(1) {
        let idx = Arc::clone(&idx);
        let paths = Arc::clone(&paths);
        let one = Arc::clone(&one);
        let tx = tx.clone();
        handles.push(std::thread::spawn(move || loop {
            let i = idx.fetch_add(1, Ordering::Relaxed);
            if i >= paths.len() {
                break;
            }
            let line = one(&paths[i]);
            if tx.send(line).is_err() {
                break;
            }
        }));
    }
    drop(tx);

    let mut out: Box<dyn Write> = match &out_file {
        Some(f) => match std::fs::File::create(f) {
            Ok(file) => Box::new(std::io::BufWriter::new(file)),
            Err(e) => {
                eprintln!("cruft test262 sweep: cannot write {}: {}", f, e);
                return ExitCode::from(73);
            }
        },
        None => Box::new(std::io::BufWriter::new(std::io::stdout())),
    };
    let progress: Option<Arc<Mutex<std::io::BufWriter<std::fs::File>>>> =
        match progress_file.as_ref() {
            Some(f) => match std::fs::File::create(f) {
                Ok(file) => Some(Arc::new(Mutex::new(std::io::BufWriter::new(file)))),
                Err(e) => {
                    eprintln!("cruft test262 sweep: cannot write {}: {}", f, e);
                    return ExitCode::from(73);
                }
            },
            None => None,
        };
    let started = Instant::now();
    let mut completed = 0usize;
    let mut pass = 0usize;
    let mut fail = 0usize;
    let mut skip = 0usize;
    let progress_every = progress_every.max(1);
    for line in rx {
        completed += 1;
        if line.contains("\"status\":\"PASS\"") {
            pass += 1;
        } else if line.contains("\"status\":\"SKIP\"") {
            skip += 1;
        } else {
            fail += 1;
        }
        if let Err(e) = writeln!(out, "{}", line).and_then(|_| out.flush()) {
            eprintln!("cruft test262 sweep: cannot write result: {}", e);
            return ExitCode::from(73);
        }
        if completed == 1 || completed % progress_every == 0 || completed == count {
            let elapsed = started.elapsed().as_secs_f64();
            let rate = if elapsed > 0.0 {
                completed as f64 / elapsed
            } else {
                0.0
            };
            let msg = format!(
                "completed={}/{} pass={} fail={} skip={} rate={:.2}/s",
                completed, count, pass, fail, skip, rate
            );
            eprintln!("{}", msg);
            if let Some(progress) = &progress {
                let mut progress = progress.lock().unwrap();
                let _ = writeln!(progress, "{}", msg);
                let _ = progress.flush();
            }
        }
    }
    for h in handles {
        let _ = h.join();
    }

    if let Some(summary_file) = summary_file {
        let emitted = pass + fail + skip;
        let runnable = pass + fail;
        let runnable_rate = if runnable > 0 {
            100.0 * pass as f64 / runnable as f64
        } else {
            0.0
        };
        let whole_rate = if count > 0 {
            100.0 * pass as f64 / count as f64
        } else {
            0.0
        };
        let summary = format!(
            "test262 sweep\nSuite size:      {}\nResults emitted: {}\nPASS:            {}\nFAIL:            {}\nSKIP:            {}\nPass rate (runnable):    {:.1}%  ({} / {})\nPass rate (whole suite): {:.1}%  ({} / {})\n",
            count, emitted, pass, fail, skip, runnable_rate, pass, runnable, whole_rate, pass, count
        );
        if let Err(e) = std::fs::write(&summary_file, summary) {
            eprintln!("cruft test262 sweep: cannot write {}: {}", summary_file, e);
            return ExitCode::from(73);
        }
    }
    ExitCode::SUCCESS
}

fn run_test262_status(rest: &[String]) -> ExitCode {
    use std::process::Command;
    use std::time::{SystemTime, UNIX_EPOCH};

    let mut root = std::env::var("T262_ROOT").ok();
    let mut do_fetch = false;
    let mut as_json = false;
    let mut it = rest.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--root" => root = it.next().cloned(),
            "--fetch" => do_fetch = true,
            "--json" => as_json = true,
            _ => {}
        }
    }
    let root = match root {
        Some(r) => r,
        None => {
            eprintln!("cruft test262 status: --root <dir> or T262_ROOT required");
            return ExitCode::from(64);
        }
    };
    let git = |gargs: &[&str]| -> Option<String> {
        let out = Command::new("git")
            .arg("-C")
            .arg(&root)
            .args(gargs)
            .output()
            .ok()?;
        if !out.status.success() {
            return None;
        }
        Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
    };
    if git(&["rev-parse", "--is-inside-work-tree"]).as_deref() != Some("true") {
        eprintln!("cruft test262 status: {} is not a git checkout", root);
        return ExitCode::from(66);
    }
    if do_fetch {
        let _ = Command::new("git")
            .arg("-C")
            .arg(&root)
            .args(["fetch", "--quiet", "origin"])
            .status();
    }
    let head = git(&["rev-parse", "HEAD"]).unwrap_or_default();
    let head_short: String = head.chars().take(10).collect();
    let head_date = git(&["log", "-1", "--format=%cI"]).unwrap_or_default();

    let (up_ref, upstream) = match git(&["rev-parse", "origin/main"]) {
        Some(s) => ("origin/main", Some(s)),
        None => ("origin/HEAD", git(&["rev-parse", "origin/HEAD"])),
    };
    let up_short: String = upstream.as_deref().unwrap_or("").chars().take(10).collect();
    let behind: Option<u64> = git(&["rev-list", "--count", &format!("HEAD..{}", up_ref)])
        .and_then(|s| s.parse::<u64>().ok());

    let git_dir = git(&["rev-parse", "--git-dir"]).unwrap_or_else(|| format!("{}/.git", root));
    let git_dir_abs = if std::path::Path::new(&git_dir).is_absolute() {
        git_dir
    } else {
        format!("{}/{}", root, git_dir)
    };
    let fetch_head = std::path::Path::new(&git_dir_abs).join("FETCH_HEAD");
    let (fetched_unix, fetched_age_days) =
        match std::fs::metadata(&fetch_head).and_then(|m| m.modified()) {
            Ok(t) => {
                let unix = t.duration_since(UNIX_EPOCH).map(|d| d.as_secs()).ok();
                let age = SystemTime::now()
                    .duration_since(t)
                    .map(|d| d.as_secs() as f64 / 86_400.0)
                    .ok();
                (unix, age)
            }
            Err(_) => (None, None),
        };

    let is_behind = behind.map(|b| b > 0).unwrap_or(false);

    let tracking_stale = !do_fetch && fetched_age_days.map(|d| d > 7.0).unwrap_or(false);
    let status = if upstream.is_none() {
        "unknown (no upstream ref; run with --fetch)"
    } else if is_behind {
        "behind"
    } else {
        "up-to-date"
    };

    if as_json {
        let f = |o: &Option<String>| o.clone().unwrap_or_default();
        println!(
            "{{\"root\":\"{}\",\"head\":\"{}\",\"head_date\":\"{}\",\"upstream_ref\":\"{}\",\"upstream\":\"{}\",\"behind\":{},\"fetched_unix\":{},\"fetched_age_days\":{},\"fetched\":{},\"tracking_stale\":{},\"status\":\"{}\"}}",
            root,
            head,
            head_date,
            up_ref,
            f(&upstream),
            behind.map(|b| b.to_string()).unwrap_or_else(|| "null".into()),
            fetched_unix.map(|u| u.to_string()).unwrap_or_else(|| "null".into()),
            fetched_age_days
                .map(|d| format!("{:.2}", d))
                .unwrap_or_else(|| "null".into()),
            do_fetch,
            tracking_stale,
            status,
        );
    } else {
        let age_str = fetched_age_days
            .map(|d| format!("{:.1} days ago", d))
            .unwrap_or_else(|| "unknown".into());
        println!("test262 corpus aliveness");
        println!("  root:       {}", root);
        println!("  HEAD:       {}  ({})", head_short, head_date);
        println!(
            "  upstream:   {}  ({}{})",
            if up_short.is_empty() { "?" } else { &up_short },
            up_ref,
            if do_fetch { ", just fetched" } else { "" }
        );
        println!("  last pull:  {}", age_str);
        match (upstream.is_some(), is_behind, behind) {
            (true, true, Some(b)) => println!(
                "  status:     BEHIND {} by {} commit(s) — run: git -C {} pull",
                up_ref, b, root
            ),
            (true, false, _) => println!("  status:     UP-TO-DATE"),
            _ => println!("  status:     UNKNOWN (no upstream ref; re-run with --fetch)"),
        }
        if tracking_stale {
            println!(
                "  note:       remote-tracking ref last fetched {} — \"behind\" may be understated; re-run with --fetch",
                age_str
            );
        }
    }

    if is_behind {
        ExitCode::from(3)
    } else {
        ExitCode::SUCCESS
    }
}

fn is_help_arg(arg: &str) -> bool {
    arg == "-h" || arg == "--help" || arg == "help"
}

fn print_install_help() {
    println!("Usage: cruft install");
    println!();
    println!(
        "Install package.json dependencies into ./node_modules using Cruft's package manager."
    );
    println!();
    println!("Environment:");
    println!("  CRUFT_REGISTRY       Override the npm registry");
    println!("  CRUFT_INSTALL_MODE   linked (default) or isolated/npm/copy");
    println!();
    println!("Options:");
    println!("  -h, --help           Print this help");
}

fn run_install_subcommand(args: &[String]) -> ExitCode {
    if args.first().map(|arg| is_help_arg(arg)).unwrap_or(false) {
        print_install_help();
        return ExitCode::SUCCESS;
    }

    let cwd = match std::env::current_dir() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("cruft install: cannot read cwd: {e}");
            return ExitCode::from(66);
        }
    };
    let registry = std::env::var("CRUFT_REGISTRY")

        .or_else(|_| std::env::var("CRUFTLESS_REGISTRY"))
        .unwrap_or_else(|_| rusty_js_pm::resolver::DEFAULT_REGISTRY.to_string());

    let mode = match std::env::var("CRUFT_INSTALL_MODE").as_deref() {
        Ok("isolated") | Ok("npm") | Ok("copy") => rusty_js_pm::linker::MaterializeMode::Copy,
        _ => rusty_js_pm::linker::MaterializeMode::Link,
    };
    eprintln!(
        "cruft install: project={} registry={} mode={}",
        cwd.display(),
        registry,
        if matches!(mode, rusty_js_pm::linker::MaterializeMode::Copy) {
            "isolated (npm-style copy)"
        } else {
            "linked (pnpm-style)"
        }
    );
    match rusty_js_pm::install::pm_install_with_mode(&cwd, &registry, mode) {
        Ok(report) => {
            for (n, v) in &report.installed {
                println!("+ {n}@{v}");
            }
            for (n, v) in &report.skipped {
                println!("= {n}@{v}");
            }
            for (group, count) in &report.dependency_groups {
                eprintln!("cruft install: dependency group {group}: {count} declared");
            }
            for (name, script) in &report.skipped_lifecycle_scripts {
                eprintln!(
                    "cruft install: skipped lifecycle script {name:?}: {script:?} (execution unsupported; use npm or cruft trust install --enforce for preflight policy)"
                );
            }
            eprintln!(
                "cruft install: {} installed, {} skipped, {} lifecycle scripts skipped",
                report.installed.len(),
                report.skipped.len(),
                report.skipped_lifecycle_scripts.len()
            );
            ExitCode::from(0)
        }
        Err(e) => {
            eprintln!("cruft install: {e:?}");
            ExitCode::from(70)
        }
    }
}

fn run_fts_t0_entry(path: &str, raw: &str, check_only: bool) -> ExitCode {
    let checked_unit =
        match cruftscript_type_checker::CruftScriptCheckedUnit::parse_and_check(path, raw) {
            Ok(checked_unit) => checked_unit,
            Err(rejected) => {
                for diagnostic in &rejected.report.diagnostics {
                    let record = cruftscript_type_checker::CruftScriptDiagnosticRecord::from_check(
                        &rejected.provenance,
                        diagnostic,
                        65,
                    );
                    eprintln!("{}", record.tooling_line());
                    eprintln!(
                        "cruft: fts diagnostic in {} [{:?}] {}..{}: {}",
                        rejected.provenance.path,
                        diagnostic.code,
                        diagnostic.span.start,
                        diagnostic.span.end,
                        diagnostic.message
                    );
                }
                return ExitCode::from(65);
            }
        };
    let handoff = checked_unit.handoff();
    eprintln!(
        "cruft: fts accepted at checked-unit handoff in {}: source_len={}, erased_annotations={}, boundary_imports={}, boundary_functions={}, build_time_declarations={}, static_obligations={}, body_functions={}",
        checked_unit.provenance.path,
        checked_unit.provenance.source_len,
        handoff.erased_annotations.len(),
        handoff.boundary_qualified_imports.len(),
        handoff.boundary_qualified_functions.len(),
        handoff.build_time_declarations.len(),
        handoff.static_obligations.len(),
        checked_unit.body_facts().functions.len()
    );
    eprintln!(
        "cruft: fts accepted at T0 handoff in {}: erased_annotations={}, boundary_imports={}, boundary_functions={}, build_time_declarations={}, static_obligations={}",
        checked_unit.provenance.path,
        handoff.erased_annotations.len(),
        handoff.boundary_qualified_imports.len(),
        handoff.boundary_qualified_functions.len(),
        handoff.build_time_declarations.len(),
        handoff.static_obligations.len()
    );
    emit_fts_boundary_tooling_lines(&checked_unit);
    if check_only {
        ExitCode::SUCCESS
    } else {
        match cruftscript_type_checker::CruftScriptLoweredUnit::lower_minimal_erased_body(
            &checked_unit,
        ) {
            Ok(lowered) => {
                if let Some(stdout) = lowered.static_stdout() {
                    println!("{stdout}");
                    ExitCode::SUCCESS
                } else {

                    run_fts_runtime_module_entry(path, &checked_unit)
                }
            }
            Err(diagnostics) => {

                if diagnostics.iter().all(|diagnostic| {
                    matches!(
                        diagnostic.code,
                        cruftscript_type_checker::LoweringDiagnosticCode::UnsupportedCompartmentShape
                            | cruftscript_type_checker::LoweringDiagnosticCode::UnsupportedFunctionShape
                    )
                }) {
                    return run_fts_runtime_module_entry(path, &checked_unit);
                }
                for diagnostic in &diagnostics {
                    let record =
                        cruftscript_type_checker::CruftScriptDiagnosticRecord::from_lowering(
                            &checked_unit.provenance,
                            diagnostic,
                            70,
                        );
                    eprintln!("{}", record.tooling_line());
                    eprintln!(
                        "cruft: fts lowering diagnostic in {} [{:?}] {}..{}: {}",
                        checked_unit.provenance.path,
                        diagnostic.code,
                        diagnostic.span.start,
                        diagnostic.span.end,
                        diagnostic.message
                    );
                }
                eprintln!("cruft: fts lowering not implemented; checked unit was not executed");
                eprintln!("cruft: fts lowering not implemented; source was not executed");
                ExitCode::from(70)
            }
        }
    }
}

fn build_fts_entry_args(
    checked_unit: &cruftscript_type_checker::CruftScriptCheckedUnit,
    entry_name: &str,
    path: &str,
) -> Result<Vec<Value>, String> {
    use cruftscript_type_checker::EntryParamScalar;
    let Some(plan) = cruftscript_type_checker::entry_param_scalar_plan(checked_unit, entry_name)
    else {

        return Ok(Vec::new());
    };
    if plan.is_empty() {
        return Ok(Vec::new());
    }

    let cli: Vec<String> = std::env::args().collect();
    let file_idx = cli
        .iter()
        .rposition(|arg| arg == path || arg.ends_with(".fts"));
    let after: Vec<&String> = match file_idx {
        Some(idx) => cli.iter().skip(idx + 1).collect(),
        None => Vec::new(),
    };
    if after.len() != plan.len() {
        return Err(format!(
            "entry `{}` expects {} argument(s); {} supplied on the command line (pass them after the .fts path)",
            entry_name,
            plan.len(),
            after.len()
        ));
    }
    let mut values = Vec::with_capacity(plan.len());
    for (i, (kind, raw)) in plan.iter().zip(after.iter()).enumerate() {
        let value = match kind {
            EntryParamScalar::Number => {
                let n: f64 = raw.parse().map_err(|_| {
                    format!("entry `{entry_name}` argument {i} expects a number, got `{raw}`")
                })?;
                Value::Number(n)
            }
            EntryParamScalar::Boolean => match raw.as_str() {
                "true" => Value::Boolean(true),
                "false" => Value::Boolean(false),
                _ => {
                    return Err(format!(
                        "entry `{entry_name}` argument {i} expects `true` or `false`, got `{raw}`"
                    ))
                }
            },
            EntryParamScalar::String => Value::String(std::rc::Rc::new(
                rusty_js_runtime::value::JsString::from(raw.as_str()),
            )),
        };
        values.push(value);
    }
    Ok(values)
}

fn run_fts_runtime_module_entry(
    path: &str,
    checked_unit: &cruftscript_type_checker::CruftScriptCheckedUnit,
) -> ExitCode {
    let mut rt = Runtime::new();
    rt.install_intrinsics();
    install_cruft_host(&mut rt, std::env::args().collect());
    let module_url = std::fs::canonicalize(path)
        .map(|path| format!("file://{}", path.to_string_lossy()))
        .unwrap_or_else(|_| path.to_string());
    let namespace = match rt.load_module(&module_url) {
        Ok(namespace) => namespace,
        Err(err) => {
            eprintln!(
                "cruft: fts runtime module execution rejected in {}: {:?}",
                checked_unit.provenance.path, err
            );
            return ExitCode::from(70);
        }
    };

    let entry_name =
        match cruftscript_type_checker::CruftScriptExportModule::lower_exports(checked_unit) {
            Ok(module) => module
                .selected_entry()
                .map(|export| export.name.clone())
                .unwrap_or_else(|| "main".to_string()),
            Err(_) => "main".to_string(),
        };
    let entry = rt.object_get(namespace, &entry_name);
    let Value::Object(entry_id) = entry.clone() else {
        eprintln!(
            "cruft: fts runtime module execution rejected in {}: exported entry `{}` is missing",
            checked_unit.provenance.path, entry_name
        );
        return ExitCode::from(70);
    };
    if !matches!(
        rt.obj(entry_id).internal_kind,
        rusty_js_runtime::value::InternalKind::Function(_)
            | rusty_js_runtime::value::InternalKind::Closure(_)
            | rusty_js_runtime::value::InternalKind::BoundFunction(_)
            | rusty_js_runtime::value::InternalKind::BoundaryWrapper(_)
    ) {
        eprintln!(
            "cruft: fts runtime module execution rejected in {}: exported entry `{}` is not callable",
            checked_unit.provenance.path, entry_name
        );
        return ExitCode::from(70);
    }

    let entry_args = match build_fts_entry_args(checked_unit, &entry_name, path) {
        Ok(args) => args,
        Err(message) => {
            eprintln!(
                "cruft: fts runtime module execution rejected in {}: {}",
                checked_unit.provenance.path, message
            );
            return ExitCode::from(70);
        }
    };
    match rt.call_function(entry, Value::Undefined, entry_args) {
        Ok(value) => {
            println!("{}", fts_runtime_value_stdout(&mut rt, &value));
            ExitCode::SUCCESS
        }
        Err(err) => {
            let rendered = match &err {
                RuntimeError::Thrown(value) => format_thrown(&rt, value),
                other => format!("{other:?}"),
            };
            eprintln!(
                "cruft: fts runtime module execution rejected in {}: {}",
                checked_unit.provenance.path, rendered
            );
            ExitCode::from(70)
        }
    }
}

fn fts_runtime_value_stdout(rt: &mut Runtime, value: &Value) -> String {
    match value {
        Value::Undefined => "undefined".to_string(),
        Value::Null => "undefined".to_string(),
        Value::String(value) => value.as_str().to_string(),
        Value::Number(value) => value.to_string(),
        Value::Boolean(true) => "true".to_string(),
        Value::Boolean(false) => "false".to_string(),
        _ => rusty_js_runtime::intrinsics::inspect(rt, value),
    }
}

fn emit_fts_boundary_tooling_lines(
    checked_unit: &cruftscript_type_checker::CruftScriptCheckedUnit,
) {
    let handoff = checked_unit.handoff();
    let default_policy = handoff
        .build_time_declarations
        .iter()
        .find(|declaration| {
            declaration.kind == cruftscript_type_checker::BuildTimeDeclarationKind::BoundaryDefault
        })
        .map(|declaration| declaration.name.as_str());

    for import in &handoff.boundary_qualified_imports {
        let (policy_name, resolution_chain) =
            describe_fts_boundary_policy("import", &import.policy, default_policy);
        eprintln!(
            "cruft: fts boundary policy chain in {}: site=import compartment={} target={} policy_name={} resolution_chain={} span={}..{}",
            checked_unit.provenance.path,
            import.compartment,
            import.imported_name,
            policy_name,
            resolution_chain,
            import.span.start,
            import.span.end
        );
    }

    for function in &handoff.boundary_qualified_functions {
        let (policy_name, resolution_chain) =
            describe_fts_boundary_policy("function", &function.policy, default_policy);
        eprintln!(
            "cruft: fts boundary policy chain in {}: site=function compartment={} target={} policy_name={} resolution_chain={} scope={} span={}..{}",
            checked_unit.provenance.path,
            function.compartment,
            function.function_name,
            policy_name,
            resolution_chain,
            function.application_scope.as_str(),
            function.span.start,
            function.span.end
        );
    }
}

fn describe_fts_boundary_policy(
    site: &str,
    policy: &cruftscript_type_checker::BoundaryPolicyRef,
    default_policy: Option<&str>,
) -> (String, String) {
    match policy {
        cruftscript_type_checker::BoundaryPolicyRef::Named(name) if name == "default" => {
            let resolved = default_policy.unwrap_or("unresolved-default");
            (
                resolved.to_string(),
                format!("{site}:default->policy:{resolved}"),
            )
        }
        cruftscript_type_checker::BoundaryPolicyRef::Named(name) => {
            (name.clone(), format!("{site}:{name}"))
        }
        cruftscript_type_checker::BoundaryPolicyRef::Default => {
            let resolved = default_policy.unwrap_or("unresolved-default");
            (
                resolved.to_string(),
                format!("{site}:default->policy:{resolved}"),
            )
        }
        cruftscript_type_checker::BoundaryPolicyRef::WeakenTo(name) => (
            name.clone(),
            format!("{site}:weaken-to:{name}->policy:{name}"),
        ),
        cruftscript_type_checker::BoundaryPolicyRef::Override(name) => (
            name.clone(),
            format!("{site}:override:{name}->policy:{name}"),
        ),
    }
}

fn parse_cap_flags(
    args: Vec<String>,
) -> (
    rusty_js_runtime::caps::CapMode,
    Option<String>,
    Option<String>,
    bool,
    Vec<String>,
) {
    use rusty_js_runtime::caps::CapMode;
    let mut mode = CapMode::Compat;
    let mut audit_path: Option<String> = None;
    let mut diagnostic_path: Option<String> = None;
    let mut allow_net_loopback = false;
    let mut out: Vec<String> = Vec::with_capacity(args.len());
    let mut it = args.into_iter();
    while let Some(a) = it.next() {
        if a == "--" {
            out.push(a);
            out.extend(it);
            break;
        }
        match a.as_str() {
            "--audit" => mode = CapMode::Audit,
            "--sealed-deps" => mode = CapMode::SealedDeps,
            "--sealed" => mode = CapMode::Sealed,
            "--diagnostics" => {
                if let Some(v) = it.next() {
                    std::env::set_var("CRUFT_DIAGNOSTICS", v);
                }
            }
            "--audit-log" => {
                if let Some(p) = it.next() {
                    audit_path = Some(p);
                }
            }
            "--diagnostic-log" => {
                if let Some(p) = it.next() {
                    diagnostic_path = Some(p);
                }
            }
            "--allow-net-loopback" => allow_net_loopback = true,
            _ if a.starts_with("--diagnostic-log=") => {
                let value = a
                    .strip_prefix("--diagnostic-log=")
                    .expect("prefix checked above");
                diagnostic_path = Some(value.to_string());
            }
            _ if a.starts_with("--diagnostics=") => {
                let value = a
                    .strip_prefix("--diagnostics=")
                    .expect("prefix checked above");
                std::env::set_var("CRUFT_DIAGNOSTICS", value);
            }
            _ => out.push(a),
        }
    }

    if mode == CapMode::Compat {
        let env =
            std::env::var("CRUFT_CAPS_MODE").or_else(|_| std::env::var("CRUFTLESS_CAPS_MODE"));
        if let Ok(s) = env {
            if let Some(m) = CapMode::from_str(&s) {
                mode = m;
            }
        }
    }

    std::env::set_var("CRUFT_CAPS_MODE", mode.as_str());
    if !allow_net_loopback {
        allow_net_loopback = std::env::var("CRUFT_ALLOW_NET_LOOPBACK")
            .or_else(|_| std::env::var("CRUFTLESS_ALLOW_NET_LOOPBACK"))
            .map(|v| matches!(v.as_str(), "1" | "true" | "yes"))
            .unwrap_or(false);
    }
    if diagnostic_path.is_none() {
        diagnostic_path = std::env::var("CRUFT_DIAGNOSTIC_LOG")
            .or_else(|_| std::env::var("CRUFT_DIAGNOSTIC_ARTIFACT"))
            .ok();
    }
    (mode, audit_path, diagnostic_path, allow_net_loopback, out)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DiagnosticDisclosureMode {
    Public,
    Structural,
}

impl DiagnosticDisclosureMode {
    fn current() -> Self {
        match std::env::var("CRUFT_DIAGNOSTICS")
            .or_else(|_| std::env::var("CRUFT_DIAGNOSTIC_MODE"))
            .ok()
            .as_deref()
        {
            Some("structural") | Some("developer") | Some("debug") => Self::Structural,
            _ => Self::Public,
        }
    }

    fn redact_type_error(self, msg: &str) -> String {
        if self == Self::Structural {
            return msg.to_string();
        }
        redact_public_type_error(msg)
    }

    fn redact_error_text(self, text: &str) -> String {
        if self == Self::Structural {
            return text.to_string();
        }
        text.lines()
            .map(|line| {
                if let Some(msg) = line.strip_prefix("TypeError: ") {
                    format!("TypeError: {}", self.redact_type_error(msg))
                } else if let Some(msg) = line.strip_prefix("Uncaught TypeError: ") {
                    format!("Uncaught TypeError: {}", self.redact_type_error(msg))
                } else {
                    line.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

fn redact_public_type_error(msg: &str) -> String {
    if let Some(rest) = msg.strip_prefix("callee is not callable: ") {
        let tag = rest
            .split(|c: char| c.is_whitespace() || c == '[' || c == '(')
            .next()
            .filter(|s| !s.is_empty())
            .unwrap_or("value");
        return format!("callee is not callable: {tag}");
    }
    msg.to_string()
}

fn write_diagnostic_artifact(
    path: Option<&str>,
    event: &str,
    url: &str,
    public_message: &str,
    structural_message: &str,
) {
    let Some(path) = path else {
        return;
    };
    let cwd = std::env::current_dir()
        .ok()
        .map(|p| p.display().to_string())
        .unwrap_or_default();
    match std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        Ok(mut file) => {
            use std::io::Write;
            let _ = writeln!(
                file,
                "{{\"type\":\"diagnostic\",\"event\":{},\"mode\":\"structural-artifact\",\"cwd\":{},\"url\":{},\"public_message\":{},\"structural_message\":{},\"telemetry\":\"none-local-file-only\"}}",
                json_string_literal(event),
                json_string_literal(&cwd),
                json_string_literal(url),
                json_string_literal(public_message),
                json_string_literal(structural_message)
            );
        }
        Err(e) => eprintln!("cruft: cannot write diagnostic log: {e}"),
    }
}

fn drive_main_agent(rt: &mut rusty_js_runtime::Runtime) -> Result<(), rusty_js_runtime::RuntimeError> {
    use rusty_js_runtime::agent_reactor::{
        agent_control_source, host_completion_inbox_source, host_poll_io_source,
        js_job_queue_source, AgentReactor, SourceRole,
    };
    let mut reactor = AgentReactor::new();
    reactor.register_with_role(js_job_queue_source(), SourceRole::Js);
    reactor.register_with_role(host_poll_io_source(), SourceRole::Host);
    reactor.register_with_role(host_completion_inbox_source(), SourceRole::Completion);
    reactor.register_with_role(agent_control_source(), SourceRole::Control);
    reactor.run_main(rt)
}

fn drain_audit_log(rt: &rusty_js_runtime::Runtime, dest: Option<&str>) {
    let records = rt.caps.drain_audit();
    if records.is_empty() {
        return;
    }
    let mut sink: Box<dyn std::io::Write> = match dest {
        Some(path) => match std::fs::File::create(path) {
            Ok(f) => Box::new(std::io::BufWriter::new(f)),
            Err(e) => {
                eprintln!("cruft: could not open audit log {path}: {e}; writing to stderr");
                Box::new(std::io::stderr())
            }
        },
        None => Box::new(std::io::stderr()),
    };
    use std::io::Write;
    let _ = writeln!(sink, "# cruft audit log — {} records", records.len());
    let _ = writeln!(
        sink,
        "# format: <caller>\\t<capability>\\t<operation>\\t<unix_micros>"
    );
    for r in &records {
        let _ = writeln!(
            sink,
            "{}\t{}\t{}\t{}",
            r.caller, r.capability, r.operation, r.timestamp_micros
        );
    }
    let _ = sink.flush();
}

fn wire_root_caps_from_config(
    rt: &mut Runtime,
    entry_url: &str,
    cap_mode: rusty_js_runtime::caps::CapMode,
) -> Option<String> {
    use rusty_js_runtime::caps::CapMode;
    use rusty_js_runtime::caps_config::CapsGrant;
    use std::path::{Path, PathBuf};

    let dbg = std::env::var_os("CRUFT_CAPS_VERBOSE").is_some();
    if !matches!(cap_mode, CapMode::Sealed | CapMode::SealedDeps) {

        return None;
    }
    let entry_path = entry_url.strip_prefix("file://").unwrap_or(entry_url);
    let start_dir = Path::new(entry_path).parent()?;

    const CAPS_CONFIG_NAMES: [&str; 2] = ["cruft-caps.json", "cruftless-caps.json"];
    let mut dir = Some(start_dir);
    let mut found: Option<(PathBuf, PathBuf)> = None;
    while let Some(d) = dir {
        for name in CAPS_CONFIG_NAMES {
            let candidate = d.join(name);
            if candidate.is_file() {
                found = Some((candidate, d.to_path_buf()));
                break;
            }
        }
        if found.is_some() {
            break;
        }
        dir = d.parent();
    }
    let (config_path, project_root) = match found {
        Some(x) => x,
        None => {
            if dbg {
                eprintln!(
                    "cruft: root-caps no cruft-caps.json found from {}",
                    start_dir.display()
                );
            }
            return None;
        }
    };

    let mut grant = match CapsGrant::load_file(&config_path) {
        Ok(g) => g,
        Err(e) => {
            if dbg {
                eprintln!(
                    "cruft: root-caps parse error in {}: {:?}",
                    config_path.display(),
                    e
                );
            }
            return None;
        }
    };
    for path in &mut grant.fs {
        let p = Path::new(path);
        if p.is_relative() {
            let resolved = project_root.join(p);
            *path = std::fs::canonicalize(&resolved)
                .unwrap_or(resolved)
                .to_string_lossy()
                .into_owned();
        } else {
            *path = std::fs::canonicalize(p)
                .unwrap_or_else(|_| p.to_path_buf())
                .to_string_lossy()
                .into_owned();
        }
    }
    if grant.is_empty() {
        if dbg {
            eprintln!(
                "cruft: root-caps {} declares no capabilities; nothing granted",
                config_path.display()
            );
        }
        return None;
    }

    fn is_app_module(name: &str) -> bool {
        name.rsplit_once('.')
            .map(|(_, ext)| matches!(ext, "js" | "mjs" | "cjs"))
            .unwrap_or(false)
    }
    let mut app_files: Vec<PathBuf> = Vec::new();
    let mut stack = vec![project_root.clone()];
    while let Some(d) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&d) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let fname = entry.file_name().to_string_lossy().into_owned();
            if path.is_dir() {
                if fname == "node_modules" {
                    continue;
                }
                stack.push(path);
            } else if is_app_module(&fname) {
                app_files.push(path);
            }
        }
    }

    let mut modules_registered = 0usize;
    for file in &app_files {
        let mut urls = vec![format!("file://{}", file.to_string_lossy())];
        if let Ok(canon) = std::fs::canonicalize(file) {
            let c = format!("file://{}", canon.to_string_lossy());
            if !urls.contains(&c) {
                urls.push(c);
            }
        }
        for url in urls {
            rt.caps.grant_module(
                &url,
                grant.to_fs(),
                grant.to_net(),
                grant.to_env(),
                grant.to_process(),
                grant.to_stdio(),
            );
            modules_registered += 1;
        }
    }

    Some(format!(
        "root-caps wired from {}: fs={} net={} env={} exec={} stdio={} across {} app module(s), {} URL grant(s)",
        config_path.display(),
        grant.fs.len(),
        grant.net.len(),
        grant.env.len(),
        grant.exec.len(),
        usize::from(grant.stdio_stdout) + usize::from(grant.stdio_stderr),
        app_files.len(),
        modules_registered
    ))
}

fn print_help(out: &mut dyn std::io::Write) {
    let _ = writeln!(out, "cruft {}", env!("CARGO_PKG_VERSION"));
    let _ = writeln!(out, "Run your cruft.");
    let _ = writeln!(out);
    let _ = writeln!(out, "USAGE:");
    let _ = writeln!(
        out,
        "    cruft <file>                # run a .js/.mjs/.cjs/.ts/.tsx/.fts file"
    );
    let _ = writeln!(out, "    cruft <subcommand> [OPTIONS]");
    let _ = writeln!(out, "    cruft -e <code>             # evaluate <code>");
    let _ = writeln!(
        out,
        "    cruft -p <code>             # evaluate <code> and print the result"
    );
    let _ = writeln!(
        out,
        "    cruft -c <file>             # check <file> for syntax errors, don't run"
    );
    let _ = writeln!(
        out,
        "    cruft -                     # read a program from stdin"
    );
    let _ = writeln!(
        out,
        "    cruft                       # start an interactive REPL (on a terminal)"
    );
    let _ = writeln!(out);
    let _ = writeln!(out, "SUBCOMMANDS:");
    let _ = writeln!(out, "    run        Run JavaScript");
    let _ = writeln!(
        out,
        "    exec       Run a local package executable from node_modules"
    );
    let _ = writeln!(
        out,
        "    cpx        Alias for exec/package-risk preflight workflows"
    );
    let _ = writeln!(
        out,
        "    agent      Run an agent inside an audited Compartment"
    );
    let _ = writeln!(
        out,
        "    compat     Explain packaged compatibility evidence for a package/project/script"
    );
    let _ = writeln!(
        out,
        "    promote    Show exact project mouths ready for Cruft runtime"
    );
    let _ = writeln!(
        out,
        "    policy     Show stable front-door policy profile semantics"
    );
    let _ = writeln!(
        out,
        "    trust      Advisory npm install preflight and pass-through"
    );
    let _ = writeln!(
        out,
        "    wrap       Supervise a Node-family command: cruft wrap -- node app.js"
    );
    let _ = writeln!(
        out,
        "    unwrap     Remove Cruft-managed Node wrapper state"
    );
    let _ = writeln!(
        out,
        "    doctor     Explain Cruft runtime vs Node-wrapper execution modes"
    );
    let _ = writeln!(
        out,
        "    install    Install dependencies into ./node_modules"
    );
    let _ = writeln!(out, "    help       Print this message");
    let _ = writeln!(out);
    let _ = writeln!(out, "OPTIONS:");
    let _ = writeln!(out, "    -e, --eval <code>    Evaluate <code> and exit");
    let _ = writeln!(
        out,
        "    -p, --print <code>   Evaluate <code>, print the result, and exit"
    );
    let _ = writeln!(
        out,
        "    -c, --check <file>   Parse <file> and report syntax errors without running it"
    );
    let _ = writeln!(
        out,
        "    --audit              Enable capability audit mode (log all I/O attempts)"
    );
    let _ = writeln!(
        out,
        "    --sealed-deps        Treat node_modules as sealed (no new I/O)"
    );
    let _ = writeln!(
        out,
        "    --sealed             Seal everything (project + deps)"
    );
    let _ = writeln!(
        out,
        "    --audit-log <path>   Write audit records to <path> (default: stderr)"
    );
    let _ = writeln!(
        out,
        "    --diagnostic-log <path> Write structural diagnostic artifacts to <path>"
    );
    let _ = writeln!(
        out,
        "    --allow-net-loopback Grant explicit loopback network authority for local servers and self-fetches"
    );
    let _ = writeln!(
        out,
        "    --diagnostics=<mode> Select public or structural runtime diagnostics"
    );
    let _ = writeln!(
        out,
        "    --test [paths…]      Run test files (node --test parity)"
    );
    let _ = writeln!(out, "    -h, --help           Print help");
    let _ = writeln!(out, "    -v, -V, --version    Print version");
    let _ = writeln!(out);
    let _ = writeln!(out, "Learn more at https://cruft.sh");
}

fn print_version() {
    println!("cruft {}", env!("CARGO_PKG_VERSION"));
}

fn repl_eval(
    rt: &mut Runtime,
    line: &str,
    n: usize,
) -> Result<rusty_js_runtime::Value, RuntimeError> {
    let url = format!("[repl:{}]", n);

    let t = line.trim_start();
    let decl_start = ["function", "class", "async"].iter().any(|kw| {
        t.strip_prefix(kw).is_some_and(|rest| {
            rest.is_empty()
                || rest.starts_with(|c: char| c.is_whitespace() || c == '*' || c == '(' || c == '{')
        })
    });

    let as_expr = !decl_start && rusty_js_parser::parse_script(&format!("({}\n);", line)).is_ok();
    if as_expr {
        rt.run_repl_script(&format!("globalThis._ = ({}\n);", line), &url)?;
        Ok(rt.global_get("_"))
    } else {

        rt.run_repl_script(line, &url)?;
        let _ = rt.run_repl_script("globalThis._ = undefined;", &url);
        Ok(rusty_js_runtime::Value::Undefined)
    }
}

fn repl_uncaught(rt: &Runtime, e: &RuntimeError) -> String {

    let clean = |msg: &str| -> String {
        let m = msg.strip_prefix("compile: ").unwrap_or(msg);
        let m = m.strip_prefix("parse: ").unwrap_or(m);
        m.split(" @").next().unwrap_or(m).trim().to_string()
    };
    match e {
        RuntimeError::Thrown(v) => {

            let s = format_thrown(rt, v);
            format!("Uncaught {}", s.strip_prefix("Thrown: ").unwrap_or(&s))
        }
        RuntimeError::CompileError(m) | RuntimeError::SyntaxError(m) => {
            format!("Uncaught SyntaxError: {}", clean(m))
        }
        RuntimeError::ReferenceError(m) => format!("Uncaught ReferenceError: {}", clean(m)),
        RuntimeError::TypeError(m) => format!(
            "Uncaught TypeError: {}",
            DiagnosticDisclosureMode::current().redact_type_error(&clean(m))
        ),
        RuntimeError::RangeError(m) => format!("Uncaught RangeError: {}", clean(m)),
        other => format!("Uncaught {:?}", other),
    }
}

fn repl_input_incomplete(buffer: &str) -> bool {
    match rusty_js_parser::parse_script(buffer) {
        Ok(_) => false,
        Err(e) => {
            let m = &e.message;
            let at_eof = e.span.start >= buffer.trim_end().len();

            m.contains("Eof")
                || (at_eof
                    && (m.contains("`RBrace`")
                        || m.contains("`RParen`")
                        || m.contains("`RBracket`")
                        || m.contains("expected object key")))
        }
    }
}

fn repl_complete_path(prefix: &str) -> (String, Vec<String>) {
    let (dir, fname) = match prefix.rfind('/') {
        Some(i) => (&prefix[..=i], &prefix[i + 1..]),
        None => ("", prefix),
    };
    let read_dir = if dir.is_empty() {
        std::path::PathBuf::from(".")
    } else {
        std::path::PathBuf::from(dir)
    };
    let mut cands: Vec<String> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&read_dir) {
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if name.starts_with(fname) {
                let mut c = name;
                if e.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                    c.push('/');
                }
                cands.push(c);
            }
        }
    }
    cands.sort();
    (fname.to_string(), cands)
}

fn repl_complete(rt: &mut Runtime, buf: &str, cursor_char: usize) -> (String, Vec<String>) {
    const KEYWORDS: &[&str] = &[
        "async",
        "await",
        "break",
        "case",
        "catch",
        "class",
        "const",
        "continue",
        "debugger",
        "default",
        "delete",
        "do",
        "else",
        "export",
        "extends",
        "false",
        "finally",
        "for",
        "function",
        "if",
        "import",
        "in",
        "instanceof",
        "let",
        "new",
        "null",
        "of",
        "return",
        "super",
        "switch",
        "this",
        "throw",
        "true",
        "try",
        "typeof",
        "undefined",
        "var",
        "void",
        "while",
        "with",
        "yield",
    ];
    let chars: Vec<char> = buf.chars().collect();
    let end = cursor_char.min(chars.len());

    {
        let line_to_cursor: String = chars[..end].iter().collect();
        let trimmed = line_to_cursor.trim_start();
        for cmd in [".load ", ".save "] {
            if let Some(rest) = trimmed.strip_prefix(cmd) {
                return repl_complete_path(rest);
            }
        }
    }
    let mut start = end;
    while start > 0 {
        let c = chars[start - 1];
        if c.is_alphanumeric() || c == '_' || c == '$' || c == '.' {
            start -= 1;
        } else {
            break;
        }
    }
    let token: String = chars[start..end].iter().collect();
    if token.is_empty() {
        return (String::new(), vec![]);
    }

    if token.starts_with('.') && !token[1..].contains('.') && chars[..start].iter().all(|c| c.is_whitespace())
    {
        const DOTCMDS: &[&str] = &[".break", ".clear", ".exit", ".help", ".load", ".save"];
        let cands: Vec<String> = DOTCMDS
            .iter()
            .filter(|c| c.starts_with(token.as_str()))
            .map(|c| (*c).to_string())
            .collect();
        return (token, cands);
    }
    let (base, prefix) = match token.rfind('.') {
        Some(i) => (token[..i].to_string(), token[i + 1..].to_string()),
        None => ("globalThis".to_string(), token.clone()),
    };

    let base_safe = !base.is_empty()
        && base
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '$' || c == '.')
        && !base.chars().next().unwrap().is_ascii_digit();
    if !base_safe {
        return (prefix, vec![]);
    }

    let snippet = format!(
        "globalThis.__cruft_c=(function(o){{var s=new Set();var p=o;\
         while(p){{Object.getOwnPropertyNames(p).forEach(function(k){{s.add(k);}});\
         p=Object.getPrototypeOf(p);}}return Array.from(s);}})({base})\
         .filter(function(k){{return k.indexOf({prefix:?})===0;}}).join(String.fromCharCode(1));",
    );
    let mut candidates: Vec<String> = if rt.run_script(&snippet, "[completion]").is_ok() {
        match rt.global_get("__cruft_c") {
            Value::String(s) => s
                .split('\u{1}')
                .filter(|x| !x.is_empty())
                .map(String::from)
                .collect(),
            _ => vec![],
        }
    } else {
        vec![]
    };
    let _ = rt.run_script("delete globalThis.__cruft_c;", "[completion]");

    if token.find('.').is_none() {
        for kw in KEYWORDS {
            if kw.starts_with(&prefix) {
                candidates.push((*kw).to_string());
            }
        }
    }
    candidates.sort();
    candidates.dedup();
    (prefix, candidates)
}

fn run_repl(
    cap_mode: rusty_js_runtime::caps::CapMode,
    allow_net_loopback: bool,
    args: &[String],
    is_tty: bool,
) -> ExitCode {
    use std::io::{BufRead, Write};
    let mut rt = Runtime::new();
    rt.set_cap_mode(cap_mode);
    if allow_net_loopback {
        rt.caps = std::sync::Arc::new(
            rusty_js_runtime::caps::CapDispatcher::new(cap_mode)
                .with_net_grant(rusty_js_runtime::caps::Net::loopback_server()),
        );
    }
    rt.install_intrinsics();
    install_cruft_host(&mut rt, args.to_vec());

    println!(
        "Welcome to Cruft {}. Alpha release. Do not use in production.",
        env!("CARGO_PKG_VERSION")
    );

    let mut n = 0usize;

    let mut buffer = String::new();

    let mut history: Vec<String> = Vec::new();

    let mut reader = repl_edit::LineReader::new(is_tty);

    let mut eval_and_print = |rt: &mut Runtime, src: &str, n: usize| {

        match repl_eval(rt, src, n) {
            Ok(v) => {

                let rendered = match &v {
                    Value::String(s) => format!("'{}'", s),
                    _ => rusty_js_runtime::intrinsics::inspect(rt, &v),
                };
                println!("{}", rendered);
            }

            Err(e) => println!("{}", repl_uncaught(rt, &e)),
        }
    };

    loop {
        let prompt = if buffer.is_empty() { "> " } else { "... " };

        let input = {
            let mut completer = |b: &str, c: usize| repl_complete(&mut rt, b, c);
            reader.read_line(prompt, &mut completer)
        };
        let line = match input {
            repl_edit::Input::Line(l) => l,
            repl_edit::Input::Interrupt => {

                buffer.clear();
                continue;
            }
            repl_edit::Input::Eof => {

                if !buffer.trim().is_empty() {
                    eval_and_print(&mut rt, buffer.trim(), n);
                }
                break;
            }
        };
        let lt = line.trim();

        if !buffer.is_empty() && (lt == ".break" || lt == ".clear") {
            buffer.clear();
            continue;
        }
        if buffer.is_empty() && lt.starts_with('.') {
            let mut parts = lt.splitn(2, char::is_whitespace);
            let cmd = parts.next().unwrap_or("");
            let arg = parts.next().unwrap_or("").trim();
            match cmd {
                ".exit" => break,
                ".break" | ".clear" => {}
                ".help" => print_repl_help(),
                ".save" => match std::fs::write(arg, history.join("\n")) {
                    Ok(()) => println!("Session saved to: {}", arg),
                    Err(e) => println!("Failed to save: {} ({})", arg, e),
                },
                ".load" => match std::fs::read_to_string(arg) {
                    Ok(src) => {
                        eval_and_print(&mut rt, src.trim(), n);
                        history.push(src.trim().to_string());
                        n += 1;
                    }
                    Err(e) => println!("Failed to load: {} ({})", arg, e),
                },
                _ => println!("Invalid REPL keyword"),
            }
            continue;
        }
        buffer.push_str(&line);
        buffer.push('\n');

        if repl_input_incomplete(&buffer) {
            continue;
        }
        let stmt = buffer.trim().to_string();
        buffer.clear();
        if stmt.is_empty() {
            continue;
        }
        eval_and_print(&mut rt, &stmt, n);
        history.push(stmt.clone());
        reader.add_history(&stmt);
        n += 1;
    }
    ExitCode::SUCCESS
}

fn print_repl_help() {
    println!(".break   Sometimes you get stuck, this gets you out");
    println!(".clear   Alias for .break");
    println!(".exit    Exit the REPL");
    println!(".help    Print this help message");
    println!(".load    Load JS from a file into the REPL session");
    println!(".save    Save all evaluated commands in this REPL session to a file");
    println!();
    println!("Press Ctrl+C to abort current expression, Ctrl+D to exit the REPL");
}

const BOOL_FLAGS: &[&str] = &[
    "-h",
    "--help",
    "-v",
    "-V",
    "--version",
    "-i",
    "--interactive",
    "--enable-source-maps",
    "--experimental-test-module-mocks",
    "--pending-deprecation",
    "--no-warnings",
    "--completion-bash",
];
const VALUE_FLAGS: &[&str] = &["-e", "--eval", "-p", "--print", "-c", "--check"];

fn recognized_bool_flag(arg: &str) -> bool {
    BOOL_FLAGS.contains(&arg)
        || arg.starts_with("--stack_size=")
        || arg.starts_with("--stack-size=")
        || arg.starts_with("--unhandled-rejections=")
}

fn first_unknown_flag(args: &[String]) -> Option<String> {
    let mut i = 1;
    while i < args.len() {
        let a = args[i].as_str();
        if a == "-" || a == "--" {
            return None;
        }
        if a.starts_with('-') && a.len() > 1 {
            if VALUE_FLAGS.contains(&a) {
                return None;
            }
            if recognized_bool_flag(a) {
                i += 1;
                continue;
            }
            return Some(args[i].clone());
        }
        return None;
    }
    None
}

fn leading_options_end(args: &[String]) -> usize {
    let mut i = 1;
    while i < args.len() {
        let a = args[i].as_str();
        if a.starts_with('-') && a.len() > 1 && a != "--" {
            if VALUE_FLAGS.contains(&a) {
                return i;
            }
            i += 1;
            continue;
        }
        return i;
    }
    i
}

fn print_bash_completion() {
    println!(
        r#"_node_complete() {{
  local cur_word options
  cur_word="${{COMP_WORDS[COMP_CWORD]}}"
  if [[ "${{cur_word}}" == -* ]] ; then
    COMPREPLY=( $(compgen -W '--help --version --eval --print --check --interactive --test --completion-bash -h -v -e -p -c -i' -- "${{cur_word}}") )
    return 0
  else
    COMPREPLY=( $(compgen -f "${{cur_word}}") )
    return 0
  fi
}}
complete -o filenames -o nospace -o bashdefault -F _node_complete node node_g"#
    );
}

fn find_package_json_from_cwd() -> Option<std::path::PathBuf> {
    let mut dir = std::env::current_dir().ok()?;
    loop {
        let candidate = dir.join("package.json");
        if candidate.is_file() {
            return Some(candidate);
        }
        if !dir.pop() {
            return None;
        }
    }
}

fn package_script_value(
    package_json: &std::path::Path,
    name: &str,
) -> Result<Option<String>, String> {
    let text = std::fs::read_to_string(package_json)
        .map_err(|e| format!("cannot read {}: {}", package_json.display(), e))?;
    let json: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| format!("cannot parse {}: {}", package_json.display(), e))?;
    Ok(json
        .get("scripts")
        .and_then(|v| v.as_object())
        .and_then(|scripts| scripts.get(name))
        .and_then(|v| v.as_str())
        .map(str::to_string))
}

fn package_bin_entry(
    package_json: &std::path::Path,
    command: &str,
) -> Result<Option<std::path::PathBuf>, String> {
    let text = std::fs::read_to_string(package_json)
        .map_err(|e| format!("cannot read {}: {}", package_json.display(), e))?;
    let json: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| format!("cannot parse {}: {}", package_json.display(), e))?;
    let package_dir = package_json
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."));
    let Some(bin) = json.get("bin") else {
        return Ok(None);
    };
    if let Some(path) = bin.as_str() {
        return Ok(Some(package_dir.join(path)));
    }
    let Some(map) = bin.as_object() else {
        return Err(format!(
            "{} has unsupported non-string/non-object bin metadata",
            package_json.display()
        ));
    };
    if let Some(path) = map.get(command).and_then(|v| v.as_str()) {
        return Ok(Some(package_dir.join(path)));
    }
    let unscoped = command.rsplit('/').next().unwrap_or(command);
    if let Some(path) = map.get(unscoped).and_then(|v| v.as_str()) {
        return Ok(Some(package_dir.join(path)));
    }
    let mut entries = map.iter().filter_map(|(_, v)| v.as_str());
    match (entries.next(), entries.next()) {
        (Some(path), None) => Ok(Some(package_dir.join(path))),
        _ => Ok(None),
    }
}

fn resolve_local_package_exec(
    package_dir: &std::path::Path,
    command: &str,
) -> Result<Option<std::path::PathBuf>, String> {
    let command_path = std::path::Path::new(command);
    if is_explicit_run_path(command) {
        let p = if command_path.is_absolute() {
            command_path.to_path_buf()
        } else {
            package_dir.join(command_path)
        };
        return Ok(Some(std::fs::canonicalize(&p).unwrap_or(p)));
    }
    let local_bin = package_dir.join("node_modules").join(".bin").join(command);
    if local_bin.exists() {
        #[cfg(windows)]
        if !is_js_or_node_bin(&local_bin) {
            if let Some(candidate) = windows_local_bin_launcher_candidate(package_dir, command) {
                return Ok(Some(candidate));
            }
        }
        return Ok(Some(std::fs::canonicalize(&local_bin).unwrap_or(local_bin)));
    }
    #[cfg(windows)]
    {
        if let Some(candidate) = windows_local_bin_launcher_candidate(package_dir, command) {
            return Ok(Some(candidate));
        }
    }

    let package_json = if let Some((scope, name)) = command.strip_prefix('@').and_then(|rest| {
        let (scope, name) = rest.split_once('/')?;
        Some((scope, name))
    }) {
        package_dir
            .join("node_modules")
            .join(format!("@{scope}"))
            .join(name)
            .join("package.json")
    } else {
        package_dir
            .join("node_modules")
            .join(command)
            .join("package.json")
    };
    if package_json.is_file() {
        return package_bin_entry(&package_json, command)
            .map(|p| p.map(|path| std::fs::canonicalize(&path).unwrap_or(path)));
    }
    Ok(None)
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PackageExecSpec {
    name: String,
    range: String,
}

impl PackageExecSpec {
    fn manifest_value(&self) -> (&str, &str) {
        (&self.name, &self.range)
    }
}

fn parse_package_exec_spec(raw: &str) -> Result<PackageExecSpec, String> {
    if raw.is_empty() || raw.starts_with('.') || raw.contains('\0') {
        return Err(format!("invalid package spec {raw:?}"));
    }
    let (name, range) = if raw.starts_with('@') {
        let Some(slash) = raw.find('/') else {
            return Err(format!("invalid scoped package spec {raw:?}"));
        };
        let after_name = &raw[slash + 1..];
        if after_name.is_empty() {
            return Err(format!("invalid scoped package spec {raw:?}"));
        }
        if let Some(at_rel) = after_name.rfind('@') {
            let at = slash + 1 + at_rel;
            (&raw[..at], &raw[at + 1..])
        } else {
            (raw, "*")
        }
    } else if let Some(at) = raw.rfind('@') {
        if at == 0 {
            return Err(format!("invalid package spec {raw:?}"));
        }
        (&raw[..at], &raw[at + 1..])
    } else {
        (raw, "*")
    };
    if name.is_empty()
        || name == "."
        || name == ".."
        || name.contains('/') && !name.starts_with('@')
    {
        return Err(format!("invalid package name {name:?} in spec {raw:?}"));
    }
    if name.starts_with('@') {
        let rest = &name[1..];
        let Some((scope, package)) = rest.split_once('/') else {
            return Err(format!(
                "invalid scoped package name {name:?} in spec {raw:?}"
            ));
        };
        if scope.is_empty() || package.is_empty() || package.contains('/') {
            return Err(format!(
                "invalid scoped package name {name:?} in spec {raw:?}"
            ));
        }
    }
    if range.is_empty() {
        return Err(format!("missing version/range in package spec {raw:?}"));
    }
    Ok(PackageExecSpec {
        name: name.to_string(),
        range: range.to_string(),
    })
}

fn cpx_cache_root() -> std::path::PathBuf {
    if let Some(path) = std::env::var_os("CRUFT_CPX_CACHE") {
        if !path.is_empty() {
            return std::path::PathBuf::from(path);
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        if !home.is_empty() {
            return std::path::PathBuf::from(home).join(".cruft").join("cpx");
        }
    }
    std::env::temp_dir().join("cruft-cpx")
}

fn cpx_exec_root_key(specs: &[PackageExecSpec], registry: &str) -> String {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    registry.hash(&mut h);
    for spec in specs {
        spec.name.hash(&mut h);
        spec.range.hash(&mut h);
    }
    format!("{:016x}", h.finish())
}

fn write_cpx_synthetic_manifest(
    exec_root: &std::path::Path,
    specs: &[PackageExecSpec],
) -> Result<(), String> {
    let mut deps = serde_json::Map::new();
    for spec in specs {
        let (name, range) = spec.manifest_value();
        deps.insert(
            name.to_string(),
            serde_json::Value::String(range.to_string()),
        );
    }
    let manifest = serde_json::json!({
        "name": "__cruft_cpx_exec__",
        "private": true,
        "dependencies": deps,
    });
    std::fs::create_dir_all(exec_root)
        .map_err(|e| format!("cannot create CPX exec root {}: {e}", exec_root.display()))?;
    let bytes = manifest.to_string();
    std::fs::write(exec_root.join("package.json"), bytes)
        .map_err(|e| format!("cannot write CPX synthetic manifest: {e}"))
}

fn cpx_selected_registry_for_specs(specs: &[PackageExecSpec]) -> Result<String, String> {
    let policy = cpx_registry_policy();
    let mut selected: Option<String> = None;
    for spec in specs {
        if rusty_js_pm::registry_policy::public_fallback_blocks_package(&policy, &spec.name) {
            return Err(format!(
                "public registry fallback blocked for unmapped scoped package {}",
                spec.name
            ));
        }
        let (registry, _) =
            rusty_js_pm::registry_policy::selected_registry_for_package(&policy, &spec.name);
        match selected.as_deref() {
            None => selected = Some(registry.to_string()),
            Some(existing) if existing == registry => {}
            Some(_) => return Err(
                "CPX package-exec does not yet support one exec root spanning multiple registries"
                    .to_string(),
            ),
        }
    }
    Ok(selected.unwrap_or(policy.default_registry))
}

fn materialize_cpx_exec_root(
    specs: &[PackageExecSpec],
    explain: bool,
) -> Result<std::path::PathBuf, String> {
    let registry = cpx_selected_registry_for_specs(specs)?;
    let mut ordered = specs.to_vec();
    ordered.sort_by(|a, b| a.name.cmp(&b.name).then_with(|| a.range.cmp(&b.range)));
    let root = cpx_cache_root().join(cpx_exec_root_key(&ordered, &registry));
    write_cpx_synthetic_manifest(&root, &ordered)?;
    if explain {
        eprintln!(
            "cruft exec: cpx_exec_root={} registry={} packages={}",
            root.display(),
            registry,
            ordered
                .iter()
                .map(|s| format!("{}@{}", s.name, s.range))
                .collect::<Vec<_>>()
                .join(",")
        );
    }
    rusty_js_pm::install::pm_install_with_mode(
        &root,
        &registry,
        rusty_js_pm::linker::MaterializeMode::Copy,
    )
    .map_err(|e| format!("CPX PM materialization failed: {e:?}"))?;
    Ok(root)
}

fn package_name_version(package_json: &std::path::Path) -> Result<(String, String), String> {
    let text = std::fs::read_to_string(package_json)
        .map_err(|e| format!("cannot read {}: {}", package_json.display(), e))?;
    let json: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| format!("cannot parse {}: {}", package_json.display(), e))?;
    let name = json
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let version = json
        .get("version")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    Ok((name, version))
}

fn is_explicit_run_path(arg: &str) -> bool {
    let path = std::path::Path::new(arg);
    path.is_absolute()
        || arg == "."
        || arg == ".."
        || arg.starts_with("./")
        || arg.starts_with("../")
        || arg.starts_with(".\\")
        || arg.starts_with("..\\")
        || arg.starts_with('\\')
        || arg.as_bytes().get(1) == Some(&b':')
}

fn shell_words_simple(src: &str) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut chars = src.chars().peekable();
    let mut quote: Option<char> = None;
    while let Some(c) = chars.next() {
        match quote {
            Some('\'') => {
                if c == '\'' {
                    quote = None;
                } else {
                    cur.push(c);
                }
            }
            Some('"') => {
                if c == '"' {
                    quote = None;
                } else if c == '\\' {
                    if let Some(n) = chars.next() {
                        cur.push(n);
                    }
                } else {
                    cur.push(c);
                }
            }
            Some(q) => {
                return Err(format!("unsupported quote state {}", q));
            }
            None => {
                if c.is_whitespace() {
                    if !cur.is_empty() {
                        out.push(std::mem::take(&mut cur));
                    }
                } else if c == '\'' || c == '"' {
                    quote = Some(c);
                } else if c == '\\' {
                    if let Some(n) = chars.next() {
                        cur.push(n);
                    }
                } else {
                    cur.push(c);
                }
            }
        }
    }
    if let Some(q) = quote {
        return Err(format!("unterminated {} quote", q));
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    Ok(out)
}

fn path_with_local_bin(package_dir: &std::path::Path) -> std::ffi::OsString {
    let bin = package_dir.join("node_modules").join(".bin");
    match std::env::var_os("PATH") {
        Some(path) if !path.is_empty() => {
            let mut paths = vec![bin];
            paths.extend(std::env::split_paths(&path));
            std::env::join_paths(paths).unwrap_or(path)
        }
        _ => bin.into_os_string(),
    }
}

fn resolve_package_script_command(package_dir: &std::path::Path, cmd: &str) -> std::path::PathBuf {
    let cmd_path = std::path::Path::new(cmd);
    if cmd_path.components().count() > 1 {
        let p = if cmd_path.is_absolute() {
            cmd_path.to_path_buf()
        } else {
            package_dir.join(cmd_path)
        };
        return std::fs::canonicalize(&p).unwrap_or(p);
    }
    let local = package_dir.join("node_modules").join(".bin").join(cmd);
    if local.exists() {
        return std::fs::canonicalize(&local).unwrap_or(local);
    }
    cmd_path.to_path_buf()
}

fn is_js_or_node_bin(path: &std::path::Path) -> bool {
    if path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| matches!(e, "js" | "mjs" | "cjs" | "ts" | "mts" | "cts"))
        .unwrap_or(false)
    {
        return true;
    }
    let Ok(bytes) = std::fs::read(path) else {
        return false;
    };
    let first = bytes
        .split(|b| *b == b'\n')
        .next()
        .map(|b| String::from_utf8_lossy(b))
        .unwrap_or_default();
    first.starts_with("#!") && first.contains("node")
}

fn has_node_hashbang(source: &str) -> bool {
    let first = source.lines().next().unwrap_or("");
    first.starts_with("#!") && first.contains("node")
}

fn source_without_hashbang(source: &str) -> &str {
    if !source.starts_with("#!") {
        return source;
    }
    match source.find('\n') {
        Some(i) => &source[i + 1..],
        None => "",
    }
}

fn should_wrap_direct_cjs_entry(path: &str, source: &str, goal_is_module: bool) -> bool {
    if goal_is_module {
        return false;
    }
    if std::env::var_os("CRUFT_NODE_FORK").is_some() && path.ends_with(".js") {
        return true;
    }
    if path.ends_with(".js") && !std::env::var_os("CRUFT_FORCE_SCRIPT").is_some() {
        return true;
    }
    path.ends_with(".cjs") || has_node_hashbang(source)
}

fn wrap_direct_cjs_entry(source: &str, abs_path: &str) -> String {
    let dirname = std::path::Path::new(abs_path)
        .parent()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default();
    let body = source_without_hashbang(source);
    format!(
        "(function() {{\n\
         const __cruft_entry_Module = require(\"module\");\n\
         const module = new __cruft_entry_Module({:?});\n\
         module.id = \".\";\n\
         module.filename = {:?};\n\
         module.path = {:?};\n\
         module.loaded = false;\n\
         module.children = [];\n\
         module.paths = [];\n\
         const __cruft_entry_require = __cruft_entry_Module.createRequire({:?});\n\
         module.require = __cruft_entry_require;\n\
         require.main = module;\n\
         __cruft_entry_require.main = module;\n\
         try {{ process.mainModule = module; }} catch (e) {{}}\n\
         const exports = module.exports;\n\
         const __filename = {:?};\n\
         const __dirname = {:?};\n\
         (function(exports, require, module, __filename, __dirname) {{\n{}\n\
         }}).call(module.exports, exports, module.require, module, __filename, __dirname);\n\
         module.loaded = true;\n\
         }})();",
        abs_path,
        abs_path,
        dirname,
        format!("file://{}", abs_path),
        abs_path,
        dirname,
        body
    )
}

const TEST_FILE_EXTS: &[&str] = &["js", "cjs", "mjs", "ts", "cts", "mts"];

fn is_test_file(path: &std::path::Path) -> bool {
    let name = match path.file_name().and_then(|n| n.to_str()) {
        Some(n) => n,
        None => return false,
    };
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    if !TEST_FILE_EXTS.contains(&ext) {
        return false;
    }
    let stem = &name[..name.len() - ext.len() - 1];
    if stem == "test"
        || stem.ends_with(".test")
        || stem.ends_with("-test")
        || stem.ends_with("_test")
        || stem.starts_with("test-")
    {
        return true;
    }

    path.parent()
        .map(|p| {
            p.components()
                .any(|c| c.as_os_str().to_str().map(|s| s == "test").unwrap_or(false))
        })
        .unwrap_or(false)
}

fn walk_for_tests(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let fname = entry.file_name();
        let fname = fname.to_string_lossy();

        if fname == "node_modules" || fname.starts_with('.') {
            continue;
        }
        let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
        if is_dir {
            walk_for_tests(&path, out);
        } else if is_test_file(&path) {
            out.push(path);
        }
    }
}

fn discover_test_files(targets: &[String]) -> Vec<std::path::PathBuf> {
    let mut files = Vec::new();
    if targets.is_empty() {
        let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        walk_for_tests(&cwd, &mut files);
    } else {
        for t in targets {
            let p = std::path::PathBuf::from(t);
            if p.is_dir() {
                walk_for_tests(&p, &mut files);
            } else if p.is_file() {
                files.push(p);
            }
        }
    }
    let mut abs: Vec<std::path::PathBuf> = files
        .iter()
        .map(|p| std::fs::canonicalize(p).unwrap_or_else(|_| p.clone()))
        .collect();
    abs.sort();
    abs.dedup();
    abs
}

struct TestModeConfig {
    reporter: String,
    reporter_destination: Option<String>,
    name_pattern: Option<String>,
    tag_filter: Option<String>,
    update_snapshots: bool,
    timeout: Option<String>,
    concurrency: Option<String>,
    isolation: Option<String>,
    targets: Vec<String>,
}

fn parse_test_mode(args: &[String]) -> Option<TestModeConfig> {
    let mut saw_test = false;
    let mut reporter = String::from("spec");
    let mut reporter_destination = None;
    let mut name_pattern = None;
    let mut tag_filter = None;
    let mut update_snapshots = false;
    let mut timeout = None;
    let mut concurrency = None;
    let mut isolation = None;
    let mut targets = Vec::new();
    let mut i = 1;
    while i < args.len() {
        let a = args[i].as_str();
        if a == "--test" {
            saw_test = true;
        } else if let Some(rest) = a.strip_prefix("--test-reporter=") {
            saw_test = true;
            reporter = rest.to_string();
        } else if a == "--test-reporter" {
            saw_test = true;
            if let Some(v) = args.get(i + 1) {
                reporter = v.clone();
                i += 1;
            }
        } else if let Some(rest) = a.strip_prefix("--test-reporter-destination=") {
            saw_test = true;
            reporter_destination = Some(rest.to_string());
        } else if a == "--test-reporter-destination" {
            saw_test = true;
            if let Some(v) = args.get(i + 1) {
                reporter_destination = Some(v.clone());
                i += 1;
            }
        } else if let Some(rest) = a.strip_prefix("--test-name-pattern=") {
            saw_test = true;
            name_pattern = Some(rest.to_string());
        } else if a == "--test-name-pattern" {
            saw_test = true;
            if let Some(v) = args.get(i + 1) {
                name_pattern = Some(v.clone());
                i += 1;
            }
        } else if let Some(rest) = a.strip_prefix("--experimental-test-tag-filter=") {
            saw_test = true;
            tag_filter = Some(rest.to_string());
        } else if a == "--experimental-test-tag-filter" {
            saw_test = true;
            if let Some(v) = args.get(i + 1) {
                tag_filter = Some(v.clone());
                i += 1;
            }
        } else if a == "--test-update-snapshots" {
            saw_test = true;
            update_snapshots = true;
        } else if let Some(rest) = a.strip_prefix("--test-timeout=") {
            saw_test = true;
            timeout = Some(rest.to_string());
        } else if a == "--test-timeout" {
            saw_test = true;
            if let Some(v) = args.get(i + 1) {
                timeout = Some(v.clone());
                i += 1;
            }
        } else if let Some(rest) = a.strip_prefix("--test-concurrency=") {
            saw_test = true;
            concurrency = Some(rest.to_string());
        } else if a == "--test-concurrency" {
            saw_test = true;
            if let Some(v) = args.get(i + 1) {
                concurrency = Some(v.clone());
                i += 1;
            }
        } else if let Some(rest) = a.strip_prefix("--test-isolation=") {
            saw_test = true;
            isolation = Some(rest.to_string());
        } else if a == "--test-isolation" {
            saw_test = true;
            if let Some(v) = args.get(i + 1) {
                isolation = Some(v.clone());
                i += 1;
            }
        } else if a == "--" {
            i += 1;
            while i < args.len() {
                targets.push(args[i].clone());
                i += 1;
            }
            break;
        } else if a.starts_with("--test") {
            saw_test = true;

        } else if a.starts_with('-') && a.len() > 1 {

        } else {
            targets.push(a.to_string());
        }
        i += 1;
    }
    if saw_test {
        Some(TestModeConfig {
            reporter,
            reporter_destination,
            name_pattern,
            tag_filter,
            update_snapshots,
            timeout,
            concurrency,
            isolation,
            targets,
        })
    } else {
        None
    }
}

fn synthesize_test_driver(
    files: &[std::path::PathBuf],
    reporter: &str,
    reporter_destination: Option<&str>,
    name_pattern: Option<&str>,
    tag_filter: Option<&str>,
    update_snapshots: bool,
) -> String {
    let mut urls = String::new();
    for f in files {
        let url = format!("file://{}", f.to_string_lossy());
        urls.push_str(&format!("{:?},\n", url));
    }
    let pattern_line = match name_pattern {
        Some(p) => format!("globalThis.__cruft_test_name_pattern = {p:?};\n"),
        None => String::new(),
    };
    let tag_warning_line = match tag_filter {
        Some(tag) if !tag.is_empty() => {
            "if (!globalThis.__cruft_test_tags_warning_emitted && globalThis.process && typeof process.emitWarning === 'function') { globalThis.__cruft_test_tags_warning_emitted = true; process.emitWarning('Test tags is an experimental feature and might change at any time', { type: 'ExperimentalWarning' }); }\n".to_string()
        }
        _ => String::new(),
    };
    let reporter_destination_line = match reporter_destination {
        Some(destination) => {
            format!("globalThis.__cruft_test_reporter_destination = {destination:?};\n")
        }
        None => String::from("globalThis.__cruft_test_reporter_destination = undefined;\n"),
    };
    format!(
        "globalThis.__cruft_test_cli_mode = true;\n\
         globalThis.__cruft_test_reporter = {reporter:?};\n\
         {reporter_destination_line}\
         globalThis.__cruft_test_update_snapshots = {update_snapshots};\n\
         {pattern_line}\
         {tag_warning_line}\
         const __cruft_files = [\n{urls}];\n\
         const __cruft_drive = async () => {{\n\
         \x20 for (const f of __cruft_files) {{\n\
         \x20   globalThis.__cruft_current_test_file = f.startsWith('file://') ? f.slice('file://'.length) : f;\n\
         \x20   globalThis.__filename = globalThis.__cruft_current_test_file;\n\
         \x20   globalThis.__dirname = globalThis.__filename.slice(0, globalThis.__filename.lastIndexOf('/'));\n\
         \x20   try {{ await import(f); }} catch (e) {{\n\
         \x20     if (globalThis.process) process.exitCode = 1;\n\
         \x20     if (globalThis.console) console.error('cruft --test: failed to load ' + f + '\\n' + (e && e.stack ? e.stack : e));\n\
         \x20   }} finally {{ delete globalThis.__cruft_current_test_file; delete globalThis.__filename; delete globalThis.__dirname; }}\n\
         \x20 }}\n\
         \x20 const runAll = globalThis.__cruft_test_run_all;\n\
         \x20 if (typeof runAll === 'function') {{\n\
         \x20   const failed = await runAll();\n\
         \x20   if (failed && globalThis.process) process.exitCode = 1;\n\
         \x20 }} else if (globalThis.console) {{\n\
         \x20   console.log('cruft --test: no test files found');\n\
         \x20 }}\n\
         }};\n\
         __cruft_drive();\n"
    )
}

fn maybe_enter_test_mode(args: &mut Vec<String>) -> Option<std::path::PathBuf> {
    let cfg = parse_test_mode(args)?;
    let files = discover_test_files(&cfg.targets);
    emit_test_runner_debug_config(&cfg);
    let driver_src = synthesize_test_driver(
        &files,
        &cfg.reporter,
        cfg.reporter_destination.as_deref(),
        cfg.name_pattern.as_deref(),
        cfg.tag_filter.as_deref(),
        cfg.update_snapshots,
    );
    let driver_path = std::env::temp_dir().join(format!(
        "cruft-test-driver-{}-{}.mjs",
        std::process::id(),
        files.len()
    ));
    if let Err(e) = std::fs::write(&driver_path, driver_src) {
        eprintln!("cruft --test: cannot write test driver: {e}");
        return None;
    }
    let argv0 = args.first().cloned().unwrap_or_else(|| "cruft".to_string());
    *args = vec![argv0, driver_path.to_string_lossy().into_owned()];
    Some(driver_path)
}

fn emit_test_runner_debug_config(cfg: &TestModeConfig) {
    let Ok(debug) = std::env::var("NODE_DEBUG") else {
        return;
    };
    if !debug
        .split(',')
        .any(|part| part.trim().eq_ignore_ascii_case("test_runner"))
    {
        return;
    }
    let isolation = cfg.isolation.as_deref().unwrap_or("process");
    let concurrency = if isolation == "none" {
        "1".to_string()
    } else {
        cfg.concurrency
            .clone()
            .unwrap_or_else(|| "true".to_string())
    };
    let timeout = cfg
        .timeout
        .clone()
        .unwrap_or_else(|| "Infinity".to_string());
    eprintln!(
        "TEST_RUNNER {}: test runner configuration: [Object: null prototype] {{",
        std::process::id()
    );
    eprintln!("  isTestRunner: true,");
    eprintln!("  concurrency: {concurrency},");
    eprintln!("  isolation: '{isolation}',");
    eprintln!("  timeout: {timeout},");
    eprintln!("}}");
}

fn script_arg_tail(args: &[String], start: usize) -> Vec<String> {
    if args.len() <= start {
        return vec![];
    }
    let mut tail = args[start..].to_vec();
    if tail.first().map(|s| s == "--").unwrap_or(false) {
        tail.remove(0);
    }
    tail
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RunBackend {
    Cruft,
    Node,
    Auto,
}

impl RunBackend {
    fn name(self) -> &'static str {
        match self {
            RunBackend::Cruft => "cruft",
            RunBackend::Node => "node",
            RunBackend::Auto => "auto",
        }
    }
}

fn parse_backend_options(
    label: &str,
    args: &[String],
    mut i: usize,
) -> Result<(RunBackend, bool, usize), ExitCode> {
    let mut backend = RunBackend::Cruft;
    let mut explain = false;
    while i < args.len() {
        let a = args[i].as_str();
        if a == "--explain" {
            explain = true;
            i += 1;
            continue;
        }
        if let Some(value) = a.strip_prefix("--backend=") {
            backend = match value {
                "cruft" => RunBackend::Cruft,
                "node" => RunBackend::Node,
                "auto" => RunBackend::Auto,
                other => {
                    eprintln!(
                        "{label}: unsupported backend {other:?}; expected cruft, node, or auto"
                    );
                    return Err(ExitCode::from(64));
                }
            };
            i += 1;
            continue;
        }
        if a == "--backend" {
            let Some(value) = args.get(i + 1) else {
                eprintln!("{label}: --backend requires an argument");
                return Err(ExitCode::from(64));
            };
            backend = match value.as_str() {
                "cruft" => RunBackend::Cruft,
                "node" => RunBackend::Node,
                "auto" => RunBackend::Auto,
                other => {
                    eprintln!(
                        "{label}: unsupported backend {other:?}; expected cruft, node, or auto"
                    );
                    return Err(ExitCode::from(64));
                }
            };
            i += 2;
            continue;
        }
        if a == "--" {
            i += 1;
            break;
        }
        break;
    }
    Ok((backend, explain, i))
}

fn parse_run_backend(args: &[String], i: usize) -> Result<(RunBackend, bool, usize), ExitCode> {
    parse_backend_options("cruft run", args, i)
}

fn print_run_help() {
    println!(
        "Usage: cruft run [--backend=cruft|node|auto] [--explain] <script-or-file> [--] [args...]"
    );
    println!();
    println!("Run a JS/TS/CruftScript file or a package.json script through Cruft's front door.");
    println!();
    println!("Options:");
    println!("  --backend=<mode>     Select cruft, node, or auto package-script backend");
    println!("  --explain            Explain the selected backend before running");
    println!("  -h, --help           Print this help");
}

fn run_node_backend_command(
    program: &std::path::Path,
    args: &[String],
    cwd: Option<&std::path::Path>,
    path_env: Option<std::ffi::OsString>,
    envs: &[(&str, std::ffi::OsString)],
) -> ExitCode {
    let program_text = program.to_string_lossy().into_owned();
    let resolved = if program_text == "node" {
        std::env::var("CRUFT_NODE").unwrap_or_else(|_| "node".to_string())
    } else {
        program_text.clone()
    };
    let mut cmd = std::process::Command::new(&resolved);
    cmd.args(args)
        .stdin(std::process::Stdio::inherit())
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit());
    if let Some(cwd) = cwd {
        cmd.current_dir(cwd);
    }
    if let Some(path_env) = path_env {
        cmd.env("PATH", path_env);
    }
    for (key, value) in envs {
        cmd.env(key, value);
    }
    match cmd.status() {
        Ok(status) => match status.code() {
            Some(code) => ExitCode::from((code & 0xff) as u8),
            None => ExitCode::from(1),
        },
        Err(e) => {
            eprintln!(
                "cruft run --backend=node: cannot execute {:?}: {e}",
                program_text
            );
            ExitCode::from(66)
        }
    }
}

#[cfg(windows)]
fn windows_local_bin_launcher_candidate(
    package_dir: &std::path::Path,
    command: &str,
) -> Option<std::path::PathBuf> {
    for suffix in [".cmd", ".bat", ".exe", ".ps1"] {
        let candidate = package_dir
            .join("node_modules")
            .join(".bin")
            .join(format!("{command}{suffix}"));
        if candidate.exists() {
            return Some(candidate);
        }
    }
    None
}

#[cfg(windows)]
fn has_windows_command_extension(path: &std::path::Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| matches!(e.to_ascii_lowercase().as_str(), "cmd" | "bat"))
        .unwrap_or(false)
}

#[cfg(windows)]
fn has_windows_powershell_extension(path: &std::path::Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("ps1"))
        .unwrap_or(false)
}

#[cfg(windows)]
fn recover_windows_cmd_node_target(path: &std::path::Path) -> Option<std::path::PathBuf> {
    if !has_windows_command_extension(path) {
        return None;
    }
    let source = std::fs::read_to_string(path).ok()?;
    let parent = path.parent()?;

    fn candidate_from_anchor(
        text: &str,
        marker: &str,
        parent: &std::path::Path,
    ) -> Option<std::path::PathBuf> {
        let pos = text.find(marker)?;
        let anchored = &text[pos..];
        let end = anchored
            .find(|c: char| c == '"' || c == '\'' || c == ')' || c == '&' || c.is_whitespace())
            .unwrap_or(anchored.len());
        let raw = &anchored[..end];
        let lowered = raw
            .replace("%dp0%", &parent.display().to_string())
            .replace("%~dp0", &parent.display().to_string());
        let candidate = std::path::PathBuf::from(lowered);
        if is_js_or_node_bin(&candidate) {
            Some(candidate)
        } else {
            None
        }
    }

    for line in source.lines() {
        for segment in line.split('"') {
            if let Some(candidate) = candidate_from_anchor(segment, "%dp0%", parent)
                .or_else(|| candidate_from_anchor(segment, "%~dp0", parent))
            {
                return Some(candidate);
            }
        }
    }
    None
}

#[cfg(windows)]
fn is_probably_text_file(path: &std::path::Path) -> bool {
    let Ok(bytes) = std::fs::read(path) else {
        return false;
    };
    if bytes.is_empty() {
        return false;
    }
    !bytes.iter().take(256).any(|b| *b == 0)
}

fn append_frontdoor_audit_log(
    path: Option<&str>,
    command: &str,
    argv: &[String],
    backend: &str,
    policy_profile: Option<&str>,
    evidence: Option<&CompatibilityEvidenceRow>,
    risks: &[TrustRisk],
    outcome: Option<u8>,
    install_enforced: bool,
    os_sandbox_enforced: bool,
) {
    let Some(path) = path else {
        return;
    };
    let cwd = std::env::current_dir()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| ".".to_string());
    let npm_version = std::process::Command::new("npm")
        .arg("--version")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());
    let path_entries = current_path_entries();
    let resolved = argv
        .first()
        .and_then(|cmd| find_command_in_path(cmd, &path_entries).map(|p| p.display().to_string()));
    let mut line = String::from("{");
    line.push_str("\"schema_version\":1");
    line.push_str(",\"tool\":\"cruft frontdoor audit\"");
    line.push_str(",\"command\":\"");
    line.push_str(&json_escape(command));
    line.push('"');
    line.push_str(",\"cwd\":\"");
    line.push_str(&json_escape(&cwd));
    line.push('"');
    line.push_str(",\"argv\":[");
    for (idx, arg) in argv.iter().enumerate() {
        if idx > 0 {
            line.push(',');
        }
        line.push('"');
        line.push_str(&json_escape(arg));
        line.push('"');
    }
    line.push(']');
    line.push_str(",\"resolved_binary\":");
    match resolved {
        Some(path) => {
            line.push('"');
            line.push_str(&json_escape(&path));
            line.push('"');
        }
        None => line.push_str("null"),
    }
    line.push_str(",\"package_manager\":{\"name\":\"npm\",\"version\":");
    match npm_version {
        Some(version) => {
            line.push('"');
            line.push_str(&json_escape(&version));
            line.push('"');
        }
        None => line.push_str("null"),
    }
    line.push('}');
    line.push_str(",\"backend\":\"");
    line.push_str(&json_escape(backend));
    line.push('"');
    line.push_str(",\"policy_profile\":");
    match policy_profile {
        Some(profile) => {
            line.push('"');
            line.push_str(&json_escape(profile));
            line.push('"');
        }
        None => line.push_str("null"),
    }
    line.push_str(",\"evidence\":");
    if let Some(row) = evidence {
        line.push_str("{\"package\":\"");
        line.push_str(&json_escape(&row.package));
        line.push_str("\",\"version\":\"");
        line.push_str(&json_escape(&row.version));
        line.push_str("\",\"mouth\":\"");
        line.push_str(&json_escape(&row.mouth));
        line.push_str("\",\"status\":\"");
        line.push_str(&json_escape(&row.status));
        line.push_str("\",\"level\":\"");
        line.push_str(&json_escape(&row.level));
        line.push_str("\",\"pipeline\":\"");
        line.push_str(&json_escape(&row.pipeline));
        line.push_str("\"}");
    } else {
        line.push_str("null");
    }
    line.push_str(",\"lifecycle_script_facts\":[");
    let mut first = true;
    for risk in risks.iter().filter(|r| r.kind == "lifecycle-script") {
        if !first {
            line.push(',');
        }
        first = false;
        line.push_str("{\"subject\":\"");
        line.push_str(&json_escape(&risk.subject));
        line.push_str("\",\"level\":\"");
        line.push_str(&json_escape(&risk.level));
        line.push_str("\",\"reason\":\"");
        line.push_str(&json_escape(&risk.reason));
        line.push_str("\"}");
    }
    line.push(']');
    line.push_str(",\"risks\":");
    push_trust_risks_json(&mut line, risks, install_enforced);
    line.push_str(",\"policy\":");
    push_frontdoor_policy_json_with_install_enforcement(
        &mut line,
        policy_profile.unwrap_or("audit"),
        install_enforced,
        os_sandbox_enforced,
    );
    line.push_str(",\"outcome\":");
    match outcome {
        Some(code) => line.push_str(&format!("{{\"exit_code\":{code}}}")),
        None => line.push_str("null"),
    }
    line.push_str("}\n");
    if let Some(parent) = std::path::Path::new(path).parent() {
        if !parent.as_os_str().is_empty() {
            let _ = std::fs::create_dir_all(parent);
        }
    }
    match std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        Ok(mut file) => {
            use std::io::Write;
            let _ = file.write_all(line.as_bytes());
        }
        Err(e) => eprintln!("cruft: cannot append front-door audit log {path}: {e}"),
    }
}

struct FrontdoorPolicyControl {
    control: &'static str,
    level: &'static str,
    reason: &'static str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WrapSandbox {
    MacosStrict,
    LinuxStrict,
    WindowsStrict,
}

impl WrapSandbox {
    fn name(self) -> &'static str {
        match self {
            WrapSandbox::MacosStrict => "macos-strict",
            WrapSandbox::LinuxStrict => "linux-strict",
            WrapSandbox::WindowsStrict => "windows-strict",
        }
    }

    fn supported(self) -> bool {
        match self {
            WrapSandbox::MacosStrict => cfg!(target_os = "macos"),
            WrapSandbox::LinuxStrict => {
                #[cfg(target_os = "linux")]
                {
                    linux_wrap_sandbox::support_status().is_ok()
                }
                #[cfg(not(target_os = "linux"))]
                {
                    false
                }
            }
            WrapSandbox::WindowsStrict => cfg!(windows),
        }
    }

    fn enforces_os_controls(self) -> bool {
        match self {
            WrapSandbox::MacosStrict => self.supported(),
            WrapSandbox::LinuxStrict => self.supported(),
            WrapSandbox::WindowsStrict => false,
        }
    }

    fn scrubs_env(self) -> bool {
        match self {
            WrapSandbox::MacosStrict => self.supported(),
            WrapSandbox::LinuxStrict => self.supported(),
            WrapSandbox::WindowsStrict => self.supported(),
        }
    }

    fn unsupported_reason(self) -> String {
        match self {
            WrapSandbox::MacosStrict => "requires macOS Seatbelt sandbox-exec support".to_string(),
            WrapSandbox::LinuxStrict => {
                #[cfg(target_os = "linux")]
                {
                    linux_wrap_sandbox::support_status()
                        .err()
                        .unwrap_or_else(|| "linux-strict is available".to_string())
                }
                #[cfg(not(target_os = "linux"))]
                {
                    "requires Linux Landlock support".to_string()
                }
            }
            WrapSandbox::WindowsStrict => "requires Windows Job Object support".to_string(),
        }
    }

    fn known_profiles() -> &'static str {
        if cfg!(target_os = "macos") {
            "macos-strict"
        } else if cfg!(target_os = "linux") {
            "linux-strict"
        } else if cfg!(windows) {
            "windows-strict"
        } else {
            "macos-strict, linux-strict, windows-strict"
        }
    }

    fn controls(self) -> &'static str {
        wrap_sandbox_control_label(self)
    }
}

fn parse_wrap_sandbox(value: &str) -> Option<WrapSandbox> {
    match value {
        "macos-strict" => Some(WrapSandbox::MacosStrict),
        "linux-strict" => Some(WrapSandbox::LinuxStrict),
        "windows-strict" => Some(WrapSandbox::WindowsStrict),
        _ => None,
    }
}

fn sandbox_profile_string_literal(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn macos_strict_sandbox_profile(allowed_exec: &std::path::Path) -> String {
    let allowed_exec =
        std::fs::canonicalize(allowed_exec).unwrap_or_else(|_| allowed_exec.to_path_buf());
    format!(
        "(version 1)\n(allow default)\n(deny network*)\n(deny file-write*)\n(deny process-exec)\n(allow process-exec (literal \"{}\"))\n",
        sandbox_profile_string_literal(&allowed_exec.display().to_string())
    )
}

fn write_wrap_sandbox_profile(
    sandbox: WrapSandbox,
    allowed_exec: &std::path::Path,
) -> Result<std::path::PathBuf, String> {
    let dir = std::env::temp_dir().join(format!(
        "cruft-wrap-sandbox-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("cannot create sandbox profile dir {}: {e}", dir.display()))?;
    let profile = dir.join(format!("{}.sb", sandbox.name()));
    let body = match sandbox {
        WrapSandbox::MacosStrict => macos_strict_sandbox_profile(allowed_exec),
        WrapSandbox::LinuxStrict => String::new(),
        WrapSandbox::WindowsStrict => String::new(),
    };
    std::fs::write(&profile, body)
        .map_err(|e| format!("cannot write sandbox profile {}: {e}", profile.display()))?;
    Ok(profile)
}

#[cfg(target_os = "linux")]
mod linux_wrap_sandbox {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;

    #[repr(C)]
    struct LandlockRulesetAttr {
        handled_access_fs: u64,
    }

    #[repr(C)]
    struct LandlockPathBeneathAttr {
        allowed_access: u64,
        parent_fd: i32,
    }

    const LANDLOCK_CREATE_RULESET_VERSION: u32 = 1;
    const LANDLOCK_RULE_PATH_BENEATH: i32 = 1;
    const LANDLOCK_ACCESS_FS_EXECUTE: u64 = 1 << 0;
    const LANDLOCK_ACCESS_FS_WRITE_FILE: u64 = 1 << 1;
    const LANDLOCK_ACCESS_FS_READ_FILE: u64 = 1 << 2;
    const LANDLOCK_ACCESS_FS_READ_DIR: u64 = 1 << 3;
    const LANDLOCK_ACCESS_FS_REMOVE_DIR: u64 = 1 << 4;
    const LANDLOCK_ACCESS_FS_REMOVE_FILE: u64 = 1 << 5;
    const LANDLOCK_ACCESS_FS_MAKE_CHAR: u64 = 1 << 6;
    const LANDLOCK_ACCESS_FS_MAKE_DIR: u64 = 1 << 7;
    const LANDLOCK_ACCESS_FS_MAKE_REG: u64 = 1 << 8;
    const LANDLOCK_ACCESS_FS_MAKE_SOCK: u64 = 1 << 9;
    const LANDLOCK_ACCESS_FS_MAKE_FIFO: u64 = 1 << 10;
    const LANDLOCK_ACCESS_FS_MAKE_BLOCK: u64 = 1 << 11;
    const LANDLOCK_ACCESS_FS_MAKE_SYM: u64 = 1 << 12;
    const LANDLOCK_ACCESS_FS_REFER: u64 = 1 << 13;
    const LANDLOCK_ACCESS_FS_TRUNCATE: u64 = 1 << 14;
    const LANDLOCK_ACCESS_FS_IOCTL_DEV: u64 = 1 << 15;

    const HANDLED_FS_ABI1: u64 = LANDLOCK_ACCESS_FS_EXECUTE
        | LANDLOCK_ACCESS_FS_WRITE_FILE
        | LANDLOCK_ACCESS_FS_READ_FILE
        | LANDLOCK_ACCESS_FS_READ_DIR
        | LANDLOCK_ACCESS_FS_REMOVE_DIR
        | LANDLOCK_ACCESS_FS_REMOVE_FILE
        | LANDLOCK_ACCESS_FS_MAKE_CHAR
        | LANDLOCK_ACCESS_FS_MAKE_DIR
        | LANDLOCK_ACCESS_FS_MAKE_REG
        | LANDLOCK_ACCESS_FS_MAKE_SOCK
        | LANDLOCK_ACCESS_FS_MAKE_FIFO
        | LANDLOCK_ACCESS_FS_MAKE_BLOCK
        | LANDLOCK_ACCESS_FS_MAKE_SYM;
    const READ_ONLY_FS: u64 = LANDLOCK_ACCESS_FS_READ_FILE | LANDLOCK_ACCESS_FS_READ_DIR;

    fn handled_fs_for_abi(abi: i32) -> u64 {
        let mut handled = HANDLED_FS_ABI1;
        if abi >= 2 {
            handled |= LANDLOCK_ACCESS_FS_REFER;
        }
        if abi >= 3 {
            handled |= LANDLOCK_ACCESS_FS_TRUNCATE;
        }
        if abi >= 5 {
            handled |= LANDLOCK_ACCESS_FS_IOCTL_DEV;
        }
        handled
    }

    fn errno_string() -> String {
        std::io::Error::last_os_error().to_string()
    }

    fn landlock_abi() -> Result<i32, String> {
        let abi = unsafe {
            libc::syscall(
                libc::SYS_landlock_create_ruleset,
                std::ptr::null::<libc::c_void>(),
                0usize,
                LANDLOCK_CREATE_RULESET_VERSION,
            )
        };
        if abi < 0 {
            return Err(format!("landlock unavailable: {}", errno_string()));
        }
        Ok(abi as i32)
    }

    pub(super) fn support_status() -> Result<i32, String> {
        let abi = landlock_abi()?;
        if abi < 1 {
            return Err(format!("landlock ABI {abi} too old; need ABI >= 1"));
        }
        Ok(abi)
    }

    fn c_path(path: &std::path::Path) -> Result<CString, String> {
        CString::new(path.as_os_str().as_bytes())
            .map_err(|_| format!("path contains NUL byte: {}", path.display()))
    }

    fn add_path_rule(ruleset_fd: i32, path: &std::path::Path, allowed: u64) -> Result<(), String> {
        let c_path = c_path(path)?;
        let fd = unsafe {
            libc::open(
                c_path.as_ptr(),
                libc::O_PATH | libc::O_CLOEXEC | libc::O_NOFOLLOW,
            )
        };
        if fd < 0 {
            return Err(format!(
                "cannot open sandbox path {}: {}",
                path.display(),
                errno_string()
            ));
        }
        let rule = LandlockPathBeneathAttr {
            allowed_access: allowed,
            parent_fd: fd,
        };
        let rc = unsafe {
            libc::syscall(
                libc::SYS_landlock_add_rule,
                ruleset_fd,
                LANDLOCK_RULE_PATH_BENEATH,
                &rule as *const _,
                0u32,
            )
        };
        let close_rc = unsafe { libc::close(fd) };
        if rc != 0 {
            return Err(format!(
                "cannot add sandbox path {}: {}",
                path.display(),
                errno_string()
            ));
        }
        if close_rc != 0 {
            return Err(format!(
                "cannot close sandbox path {}: {}",
                path.display(),
                errno_string()
            ));
        }
        Ok(())
    }

    fn add_execute_file_rule(ruleset_fd: i32, path: &std::path::Path) -> Result<(), String> {
        let path = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        add_path_rule(
            ruleset_fd,
            &path,
            LANDLOCK_ACCESS_FS_READ_FILE | LANDLOCK_ACCESS_FS_EXECUTE,
        )
    }

    pub(super) fn apply_linux_strict(allowed_exec: &std::path::Path) -> Result<(), String> {
        let abi = support_status()?;
        let attr = LandlockRulesetAttr {
            handled_access_fs: handled_fs_for_abi(abi),
        };
        let ruleset_fd = unsafe {
            libc::syscall(
                libc::SYS_landlock_create_ruleset,
                &attr as *const _,
                std::mem::size_of::<LandlockRulesetAttr>(),
                0u32,
            )
        };
        if ruleset_fd < 0 {
            return Err(format!(
                "cannot create landlock ruleset: {}",
                errno_string()
            ));
        }
        let ruleset_fd = ruleset_fd as i32;
        let root = std::path::Path::new("/");
        if let Err(e) = add_path_rule(ruleset_fd, root, READ_ONLY_FS) {
            let _ = unsafe { libc::close(ruleset_fd) };
            return Err(e);
        }
        if let Err(e) = add_execute_file_rule(ruleset_fd, allowed_exec) {
            let _ = unsafe { libc::close(ruleset_fd) };
            return Err(e);
        }
        for loader in [
            "/lib64/ld-linux-x86-64.so.2",
            "/lib/x86_64-linux-gnu/ld-linux-x86-64.so.2",
            "/lib/ld-linux-aarch64.so.1",
            "/lib64/ld-linux-aarch64.so.1",
        ] {
            let loader = std::path::Path::new(loader);
            if loader.exists() {
                if let Err(e) = add_execute_file_rule(ruleset_fd, loader) {
                    let _ = unsafe { libc::close(ruleset_fd) };
                    return Err(e);
                }
            }
        }
        let prctl_rc = unsafe { libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) };
        if prctl_rc != 0 {
            let _ = unsafe { libc::close(ruleset_fd) };
            return Err(format!("cannot set no_new_privs: {}", errno_string()));
        }
        let restrict_rc =
            unsafe { libc::syscall(libc::SYS_landlock_restrict_self, ruleset_fd, 0u32) };
        let close_rc = unsafe { libc::close(ruleset_fd) };
        if restrict_rc != 0 {
            return Err(format!(
                "cannot restrict process with landlock: {}",
                errno_string()
            ));
        }
        if close_rc != 0 {
            return Err(format!("cannot close landlock ruleset: {}", errno_string()));
        }
        Ok(())
    }
}

fn configure_wrap_command_env(cmd: &mut std::process::Command, sandbox: Option<WrapSandbox>) {
    let Some(sandbox) = sandbox.filter(|s| s.scrubs_env()) else {
        return;
    };
    match sandbox {
        WrapSandbox::MacosStrict | WrapSandbox::LinuxStrict => {
            let path =
                std::env::var_os("PATH").unwrap_or_else(|| "/usr/bin:/bin:/usr/sbin:/sbin".into());
            let home = std::env::var_os("HOME");
            let tmpdir = std::env::var_os("TMPDIR");
            cmd.env_clear();
            cmd.env("PATH", path);
            if let Some(home) = home {
                cmd.env("HOME", home);
            }
            if let Some(tmpdir) = tmpdir {
                cmd.env("TMPDIR", tmpdir);
            }
            cmd.env("CRUFT_WRAP_SANDBOX", sandbox.name());
        }
        WrapSandbox::WindowsStrict => configure_windows_wrap_command_env(cmd, sandbox),
    }
}

#[cfg(windows)]
fn configure_windows_wrap_command_env(cmd: &mut std::process::Command, sandbox: WrapSandbox) {
    let path = std::env::var_os("PATH").or_else(|| std::env::var_os("Path"));
    let system_root = std::env::var_os("SystemRoot");
    let windir = std::env::var_os("WINDIR");
    let temp = std::env::var_os("TEMP");
    let tmp = std::env::var_os("TMP");
    cmd.env_clear();
    if let Some(path) = path {
        cmd.env("PATH", path);
    }
    if let Some(system_root) = system_root {
        cmd.env("SystemRoot", system_root);
    }
    if let Some(windir) = windir {
        cmd.env("WINDIR", windir);
    }
    if let Some(temp) = temp {
        cmd.env("TEMP", temp);
    }
    if let Some(tmp) = tmp {
        cmd.env("TMP", tmp);
    }
    cmd.env("CRUFT_WRAP_SANDBOX", sandbox.name());
}

#[cfg(not(windows))]
fn configure_windows_wrap_command_env(cmd: &mut std::process::Command, sandbox: WrapSandbox) {
    let _ = (cmd, sandbox);
}

fn wrap_sandbox_env_audit_label(sandbox: Option<WrapSandbox>) -> &'static str {
    match sandbox.filter(|s| s.scrubs_env()) {
        Some(WrapSandbox::MacosStrict) => "scrubbed(PATH,HOME,TMPDIR)",
        Some(WrapSandbox::LinuxStrict) => "scrubbed(PATH,HOME,TMPDIR)",
        Some(WrapSandbox::WindowsStrict) => {
            "scrubbed(PATH,SystemRoot,WINDIR,TEMP,TMP,CRUFT_WRAP_SANDBOX)"
        }
        None => "inherit",
    }
}

fn wrap_sandbox_control_label(sandbox: WrapSandbox) -> &'static str {
    match sandbox {
        WrapSandbox::MacosStrict => "filesystem-write,network,environment,external-process-exec",
        WrapSandbox::LinuxStrict => "filesystem-write,environment,external-process-exec",
        WrapSandbox::WindowsStrict => "process-lifetime,environment",
    }
}

fn wrap_sandbox_limitation_label(sandbox: WrapSandbox) -> Option<&'static str> {
    match sandbox {
            WrapSandbox::MacosStrict => None,
            WrapSandbox::LinuxStrict => Some(
                "linux-strict OS controls currently cover filesystem-write, environment, and external-process-exec; network and complete child-process denial remain not-available",
            ),
            WrapSandbox::WindowsStrict => Some(
                "windows-strict OS controls currently cover process lifetime and environment scrub; filesystem, network, and child-process denial remain not-available",
            ),
    }
}

fn wrap_sandbox_launch_path(resolved: &str) -> std::path::PathBuf {
    let path = std::path::Path::new(resolved);
    let candidate = if path.components().count() > 1 {
        path.to_path_buf()
    } else {
        find_command_in_path(resolved, &current_path_entries())
            .unwrap_or_else(|| path.to_path_buf())
    };
    std::fs::canonicalize(&candidate).unwrap_or(candidate)
}

#[cfg(windows)]
struct WindowsJobHandle(*mut std::ffi::c_void);

#[cfg(windows)]
impl Drop for WindowsJobHandle {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

#[cfg(windows)]
#[repr(C)]
struct IoCounters {
    read_operation_count: u64,
    write_operation_count: u64,
    other_operation_count: u64,
    read_transfer_count: u64,
    write_transfer_count: u64,
    other_transfer_count: u64,
}

#[cfg(windows)]
#[repr(C)]
struct JobObjectBasicLimitInformation {
    per_process_user_time_limit: i64,
    per_job_user_time_limit: i64,
    limit_flags: u32,
    minimum_working_set_size: usize,
    maximum_working_set_size: usize,
    active_process_limit: u32,
    affinity: usize,
    priority_class: u32,
    scheduling_class: u32,
}

#[cfg(windows)]
#[repr(C)]
struct JobObjectExtendedLimitInformation {
    basic_limit_information: JobObjectBasicLimitInformation,
    io_info: IoCounters,
    process_memory_limit: usize,
    job_memory_limit: usize,
    peak_process_memory_used: usize,
    peak_job_memory_used: usize,
}

#[cfg(windows)]
const JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE: u32 = 0x0000_2000;

#[cfg(windows)]
const JOB_OBJECT_EXTENDED_LIMIT_INFORMATION_CLASS: u32 = 9;

#[cfg(windows)]
#[link(name = "kernel32")]
extern "system" {
    fn CreateJobObjectW(
        lp_job_attributes: *mut std::ffi::c_void,
        lp_name: *const u16,
    ) -> *mut std::ffi::c_void;
    fn SetInformationJobObject(
        h_job: *mut std::ffi::c_void,
        job_object_information_class: u32,
        lp_job_object_information: *mut std::ffi::c_void,
        cb_job_object_information_length: u32,
    ) -> i32;
    fn AssignProcessToJobObject(
        h_job: *mut std::ffi::c_void,
        h_process: *mut std::ffi::c_void,
    ) -> i32;
    fn CloseHandle(h_object: *mut std::ffi::c_void) -> i32;
}

#[cfg(windows)]
fn attach_windows_job_object(
    child: &std::process::Child,
    sandbox: Option<WrapSandbox>,
) -> Result<Option<WindowsJobHandle>, String> {
    use std::os::windows::io::AsRawHandle;
    if sandbox != Some(WrapSandbox::WindowsStrict) {
        return Ok(None);
    }
    unsafe {
        let job = CreateJobObjectW(std::ptr::null_mut(), std::ptr::null());
        if job.is_null() {
            return Err(format!(
                "CreateJobObjectW failed: {}",
                std::io::Error::last_os_error()
            ));
        }
        let guard = WindowsJobHandle(job);
        let mut info = JobObjectExtendedLimitInformation {
            basic_limit_information: JobObjectBasicLimitInformation {
                per_process_user_time_limit: 0,
                per_job_user_time_limit: 0,
                limit_flags: JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
                minimum_working_set_size: 0,
                maximum_working_set_size: 0,
                active_process_limit: 0,
                affinity: 0,
                priority_class: 0,
                scheduling_class: 0,
            },
            io_info: IoCounters {
                read_operation_count: 0,
                write_operation_count: 0,
                other_operation_count: 0,
                read_transfer_count: 0,
                write_transfer_count: 0,
                other_transfer_count: 0,
            },
            process_memory_limit: 0,
            job_memory_limit: 0,
            peak_process_memory_used: 0,
            peak_job_memory_used: 0,
        };
        if SetInformationJobObject(
            guard.0,
            JOB_OBJECT_EXTENDED_LIMIT_INFORMATION_CLASS,
            &mut info as *mut _ as *mut std::ffi::c_void,
            std::mem::size_of::<JobObjectExtendedLimitInformation>() as u32,
        ) == 0
        {
            return Err(format!(
                "SetInformationJobObject failed: {}",
                std::io::Error::last_os_error()
            ));
        }
        if AssignProcessToJobObject(guard.0, child.as_raw_handle() as *mut std::ffi::c_void) == 0 {
            return Err(format!(
                "AssignProcessToJobObject failed: {}",
                std::io::Error::last_os_error()
            ));
        }
        Ok(Some(guard))
    }
}

#[cfg(not(windows))]
fn attach_windows_job_object(
    _child: &std::process::Child,
    _sandbox: Option<WrapSandbox>,
) -> Result<Option<()>, String> {
    Ok(None)
}

fn normalize_frontdoor_policy_profile(profile: &str) -> &'static str {
    match profile {
        "audit" => "audit",
        "ci" | "paranoid" | "locked" => "ci",
        _ => "audit",
    }
}

fn known_frontdoor_policy_profile(profile: &str) -> bool {
    matches!(profile, "audit" | "ci" | "paranoid" | "locked")
}

fn frontdoor_policy_controls(profile: &str, audit_requested: bool) -> Vec<FrontdoorPolicyControl> {
    let profile = normalize_frontdoor_policy_profile(profile);
    let dependency_level = match profile {
        "audit" => "audit",
        _ => "advisory",
    };
    let lifecycle_level = match profile {
        "audit" => "audit",
        _ => "advisory",
    };
    vec![
        FrontdoorPolicyControl {
            control: "command-transcript",
            level: if audit_requested { "audit" } else { "audit" },
            reason: "records command/cwd/argv/env-summary when transcript output is requested",
        },
        FrontdoorPolicyControl {
            control: "subprocess-supervision",
            level: "enforced",
            reason: "wrapper boundary preserves cwd/env/argv/stdio/exit status",
        },
        FrontdoorPolicyControl {
            control: "dependency-scan",
            level: dependency_level,
            reason: "OSV and package-risk facts are reported without blocking installs",
        },
        FrontdoorPolicyControl {
            control: "lifecycle-scripts",
            level: lifecycle_level,
            reason: "install/package lifecycle scripts are observed and reported without blocking",
        },
        FrontdoorPolicyControl {
            control: "native-addons",
            level: lifecycle_level,
            reason: "native addon markers are observed and reported without blocking",
        },
        FrontdoorPolicyControl {
            control: "runtime-substitution",
            level: "enforced",
            reason: "Cruft runtime is selected only by explicit backend or exact packaged auto evidence",
        },
        FrontdoorPolicyControl {
            control: "install-blocking",
            level: "not-available",
            reason: "default cockpit/wrap flows do not block installs; explicit cruft trust install --enforce can block lifecycle/native/malicious risks before npm launch",
        },
        FrontdoorPolicyControl {
            control: "malicious-package-blocking",
            level: "not-available",
            reason: "default cockpit/wrap flows report known malicious advisories; explicit cruft trust install --enforce can block known malicious install risks before npm launch",
        },
        FrontdoorPolicyControl {
            control: "os-process-sandbox",
            level: "not-available",
            reason: "Node child processes are not constrained by Cruft runtime caps",
        },
    ]
}

fn push_frontdoor_policy_json(out: &mut String, profile: &str) {
    push_frontdoor_policy_json_with_install_enforcement(out, profile, false, false);
}

fn frontdoor_policy_control_owner(control: &str) -> &'static str {
    match control {
        "os-process-sandbox"
        | "complete-child-process-denial"
        | "linux-windows-sandbox"
        | "filesystem-write"
        | "network"
        | "environment"
        | "external-process-exec"
        | "windows-process-lifetime" => "node-wrapper-os-sandbox",
        "install-blocking"
        | "malicious-package-blocking"
        | "dependency-scan"
        | "lifecycle-scripts"
        | "native-addons" => "front-door-trust-policy",
        "command-transcript" => "front-door-audit-log",
        "runtime-substitution" | "subprocess-supervision" => "front-door-wrapper",
        _ => "front-door-policy",
    }
}

fn frontdoor_policy_control_next_action(control: &str, level: &str) -> &'static str {
    match (control, level) {
        ("install-blocking", "not-available")
        | ("malicious-package-blocking", "not-available")
        | ("dependency-scan", "advisory")
        | ("lifecycle-scripts", "advisory")
        | ("native-addons", "advisory") => "cruft trust install --enforce -- npm install",
        ("install-blocking", "enforced")
        | ("malicious-package-blocking", "enforced")
        | ("lifecycle-scripts", "enforced")
        | ("native-addons", "enforced") => "current command enforces before npm launch",
        ("command-transcript", _) => "use --audit-log <path>",
        ("os-process-sandbox", "not-available") => "use cruft wrap --sandbox=<supported-profile> after checking cruft doctor --json sandbox_profiles",
        ("os-process-sandbox", "enforced") => "run with explicit --sandbox profile",
        ("complete-child-process-denial", "not-available")
        | ("linux-windows-sandbox", "not-available") => {
            "no supported activation command; see node wrapper sandbox arc"
        }
        ("runtime-substitution", "enforced") => "use cruft run --backend=auto --explain app.js",
        ("subprocess-supervision", "enforced") => "use cruft wrap -- node app.js",
        _ => "review cruft doctor --json for current capability state",
    }
}

fn push_frontdoor_policy_json_with_install_enforcement(
    out: &mut String,
    profile: &str,
    install_enforced: bool,
    os_sandbox_enforced: bool,
) {
    out.push('[');
    let mut controls =
        frontdoor_policy_controls_with_install_enforcement(profile, true, install_enforced);
    if os_sandbox_enforced {
        for control in &mut controls {
            if control.control == "os-process-sandbox" {
                control.level = "enforced";
                control.reason = "explicit wrap sandbox profile enforces platform OS controls for this Node child";
            }
        }
    }
    for (idx, control) in controls.iter().enumerate() {
        if idx > 0 {
            out.push(',');
        }
        out.push_str("{\"control\":\"");
        out.push_str(&json_escape(control.control));
        out.push_str("\",\"level\":\"");
        out.push_str(&json_escape(control.level));
        out.push_str("\",\"reason\":\"");
        out.push_str(&json_escape(control.reason));
        out.push_str("\",\"next_action\":\"");
        out.push_str(&json_escape(frontdoor_policy_control_next_action(
            control.control,
            control.level,
        )));
        out.push_str("\",\"owner\":\"");
        out.push_str(frontdoor_policy_control_owner(control.control));
        out.push_str("\"}");
    }
    out.push(']');
}

fn frontdoor_policy_controls_with_install_enforcement(
    profile: &str,
    audit_requested: bool,
    install_enforced: bool,
) -> Vec<FrontdoorPolicyControl> {
    let mut controls = frontdoor_policy_controls(profile, audit_requested);
    if install_enforced {
        for control in &mut controls {
            match control.control {
                "lifecycle-scripts" => {
                    control.level = "enforced";
                    control.reason = "explicit trust-install enforcement blocks observed lifecycle-script risks before npm launch";
                }
                "native-addons" => {
                    control.level = "enforced";
                    control.reason = "explicit trust-install enforcement blocks observed native-addon risks before npm launch";
                }
                "install-blocking" => {
                    control.level = "enforced";
                    control.reason = "explicit trust-install enforcement refuses npm launch when blocking install/package risks are present";
                }
                "malicious-package-blocking" => {
                    control.level = "enforced";
                    control.reason = "explicit trust-install enforcement refuses npm launch when known malicious advisories are present";
                }
                _ => {}
            }
        }
    }
    controls
}

fn frontdoor_policy_controls_for_wrap(
    profile: &str,
    audit_requested: bool,
    sandbox: Option<WrapSandbox>,
) -> Vec<FrontdoorPolicyControl> {
    let mut controls = frontdoor_policy_controls(profile, audit_requested);
    if let Some(sandbox) = sandbox {
        if sandbox.enforces_os_controls() {
            for control in &mut controls {
                if control.control == "os-process-sandbox" {
                    control.level = "enforced";
                    control.reason = "explicit wrap sandbox profile enforces platform OS controls for this Node child";
                }
            }
        }
    }
    controls
}

fn run_policy_subcommand(args: &[String]) -> ExitCode {
    if args.first().map(|arg| is_help_arg(arg)).unwrap_or(false) {
        println!("Usage: cruft policy [explain] [audit|ci|paranoid|locked] [--json]");
        println!();
        println!("Show stable front-door policy profile semantics.");
        println!();
        println!("Options:");
        println!("  --json               Emit the profile report as JSON");
        println!("  -h, --help           Print this help");
        return ExitCode::SUCCESS;
    }
    let mut as_json = false;
    let mut explain = false;
    let mut requested_profile: Option<&str> = None;
    for arg in args {
        match arg.as_str() {
            "--json" => as_json = true,
            "explain" => explain = true,
            profile if known_frontdoor_policy_profile(profile) && requested_profile.is_none() => {
                requested_profile = Some(profile);
            }
            profile if known_frontdoor_policy_profile(profile) => {
                eprintln!("cruft policy: multiple policy profiles supplied; choose one of audit, ci, paranoid, locked");
                return ExitCode::from(64);
            }
            other => {
                eprintln!("cruft policy: unknown argument {other:?}; usage: cruft policy [explain] [audit|ci|paranoid|locked] [--json]");
                return ExitCode::from(64);
            }
        }
    }
    let requested = requested_profile.unwrap_or("audit");
    if !known_frontdoor_policy_profile(requested) {
        eprintln!("cruft policy: unknown profile {requested:?}; supported: audit, ci");
        return ExitCode::from(64);
    }
    let profile = normalize_frontdoor_policy_profile(requested);
    if as_json {
        print_policy_json(requested, profile, explain);
        return ExitCode::SUCCESS;
    }
    println!("Cruft front-door policy profile");
    println!("Profile: {profile}");
    if requested != profile {
        println!("Requested profile {requested:?} is a compatibility alias for ci semantics.");
    }
    println!("Vocabulary: audit, advisory, enforced, not-available");
    println!("Controls:");
    for control in frontdoor_policy_controls(profile, true) {
        println!(
            "  - control={} level={} reason={} next_action={} owner={}",
            control.control,
            control.level,
            control.reason,
            frontdoor_policy_control_next_action(control.control, control.level),
            frontdoor_policy_control_owner(control.control)
        );
    }
    println!("Result: profile names do not imply install blocking, malicious-package blocking, or OS/process sandboxing unless a control says level=enforced.");
    ExitCode::SUCCESS
}

fn print_policy_json(requested_profile: &str, profile: &str, explain: bool) {
    let cwd = std::env::current_dir()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| ".".to_string());
    let lockfile = find_npm_lockfile_from(&std::path::PathBuf::from(&cwd))
        .map(|(_, path)| path.display().to_string());
    let package_json = find_package_json_from_cwd();
    let mut risks = Vec::new();
    if let Some(package_json) = &package_json {
        risks.extend(collect_manifest_trust_risks(package_json));
    }
    if let Some(lockfile_path) = lockfile.as_ref() {
        risks.extend(collect_lockfile_trust_risks(std::path::Path::new(
            lockfile_path,
        )));
    }
    let mut security_scan_status = if lockfile.is_some() {
        "not-run".to_string()
    } else {
        "no-lockfile".to_string()
    };
    let mut security_scan_reason = if lockfile.is_some() {
        "OSV scan not run by policy --json unless CRUFT_OSV_FIXTURE is provided".to_string()
    } else {
        "no npm lockfile found".to_string()
    };
    if std::env::var_os("CRUFT_OSV_FIXTURE").is_some() {
        match collect_osv_trust_risks(lockfile.as_deref().map(std::path::Path::new)) {
            Ok(osv_risks) => {
                security_scan_status = if osv_risks.is_empty() {
                    "clean".to_string()
                } else {
                    "advisory".to_string()
                };
                security_scan_reason = "OSV fixture scan completed".to_string();
                risks.extend(osv_risks);
            }
            Err(e) => {
                security_scan_status = "failed".to_string();
                security_scan_reason = e;
            }
        }
    }
    risks = sorted_dedup_trust_risks(risks);

    let mut policy_json = String::new();
    push_frontdoor_policy_json(&mut policy_json, profile);
    let mut risks_json = String::new();
    push_trust_risks_json(&mut risks_json, &risks, false);

    println!("{{");
    println!("  \"schema_version\": 1,");
    println!("  \"kind\": \"cruft.policy.explain.v1\",");
    println!("  \"tool\": \"cruft policy\",");
    println!(
        "  \"mode\": {},",
        if explain {
            "\"explain\""
        } else {
            "\"summary\""
        }
    );
    println!("  \"profile\": \"{}\",", json_escape(profile));
    println!(
        "  \"requested_profile\": \"{}\",",
        json_escape(requested_profile)
    );
    println!(
        "  \"profile_alias\": {},",
        if requested_profile == profile {
            "null".to_string()
        } else {
            json_string_literal("ci")
        }
    );
    println!("  \"vocabulary\": [\"audit\", \"advisory\", \"enforced\", \"not-available\"],");
    println!("  \"project\": {{");
    println!("    \"root\": \"{}\",", json_escape(&cwd));
    println!("    \"package_manager\": \"npm\",");
    println!("    \"lockfile\": {}", json_optional_string(&lockfile));
    println!("  }},");
    println!("  \"security_scan\": {{");
    println!("    \"provider\": \"osv\",");
    println!(
        "    \"status\": \"{}\",",
        json_escape(&security_scan_status)
    );
    println!("    \"reason\": \"{}\"", json_escape(&security_scan_reason));
    println!("  }},");
    println!("  \"policy\": {},", policy_json);
    println!("  \"risks\": {},", risks_json);
    println!("  \"non_claims\": [");
    println!(
        "    \"profile names do not imply install blocking unless a control says level=enforced\","
    );
    println!("    \"default wrap/cockpit flows do not block npm installs, lifecycle scripts, malicious packages, or native addons\",");
    println!("    \"Node child processes are not constrained by Cruft runtime caps without an explicit supported OS sandbox profile\"");
    println!("  ],");
    println!("  \"next_steps\": [");
    println!("    \"cruft doctor --json\",");
    println!("    \"cruft trust install --enforce -- npm install\",");
    println!("    \"cruft wrap --explain --audit --policy=ci -- node app.js\"");
    println!("  ]");
    println!("}}");
}

fn run_node_backend_file(path: &str, forwarded: &[String]) -> ExitCode {
    let mut child_args = vec![path.to_string()];
    child_args.extend_from_slice(forwarded);
    run_node_backend_command(std::path::Path::new("node"), &child_args, None, None, &[])
}

#[derive(Clone, Debug)]
struct CompatibilityEvidenceRow {
    package: String,
    version: String,
    mouth: String,
    status: String,
    caveat: String,
    pipeline: String,
    evidence_commit: String,
    level: String,
    date: String,
}

#[derive(Clone, Debug)]
struct AutoBackendSelection {
    backend: RunBackend,
    reason: String,
    row: Option<CompatibilityEvidenceRow>,
}

fn packaged_compatibility_index_text() -> Result<String, String> {
    if let Ok(path) = std::env::var("CRUFT_COMPATIBILITY_INDEX") {
        return std::fs::read_to_string(&path)
            .map_err(|e| format!("cannot read compatibility evidence index {path}: {e}"));
    }
    Ok(include_str!("../data/compatibility-evidence-index.json").to_string())
}

fn json_string_field(obj: &serde_json::Map, key: &str) -> String {
    obj.get(key)
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

fn packaged_compatibility_rows() -> Result<Vec<CompatibilityEvidenceRow>, String> {
    let text = packaged_compatibility_index_text()?;
    let json = serde_json::from_str::<serde_json::Value>(&text)
        .map_err(|e| format!("cannot parse packaged compatibility evidence index: {e}"))?;
    let Some(rows) = json.get("rows").and_then(|v| v.as_array()) else {
        return Err("packaged compatibility evidence index has no rows array".to_string());
    };
    let mut out = Vec::new();
    for row in rows {
        let Some(obj) = row.as_object() else {
            continue;
        };
        out.push(CompatibilityEvidenceRow {
            package: json_string_field(obj, "package"),
            version: json_string_field(obj, "version"),
            mouth: json_string_field(obj, "mouth"),
            status: json_string_field(obj, "status"),
            caveat: json_string_field(obj, "caveat"),
            pipeline: json_string_field(obj, "pipeline"),
            evidence_commit: json_string_field(obj, "evidence_commit"),
            level: json_string_field(obj, "level"),
            date: json_string_field(obj, "date"),
        });
    }
    Ok(out)
}

fn row_admits_cruft(row: &CompatibilityEvidenceRow) -> bool {
    row.status.contains("PASS")
        && row.pipeline.trim().eq_ignore_ascii_case("none")
        && (row.mouth.starts_with("package-script:") || row.mouth.starts_with("direct-file:"))
}

fn select_auto_for_exact_mouth(package: &str, version: &str, mouth: &str) -> AutoBackendSelection {
    let rows = match packaged_compatibility_rows() {
        Ok(rows) => rows,
        Err(e) => {
            return AutoBackendSelection {
                backend: RunBackend::Node,
                reason: format!("packaged compatibility evidence index unavailable: {e}"),
                row: None,
            };
        }
    };
    if let Some(row) = rows.into_iter().find(|row| {
        row.package == package
            && row.version == version
            && row.mouth == mouth
            && row_admits_cruft(row)
    }) {
        return AutoBackendSelection {
            backend: RunBackend::Cruft,
            reason: format!(
                "exact packaged compatibility evidence matched package={} version={} mouth={}",
                package, version, mouth
            ),
            row: Some(row),
        };
    }
    AutoBackendSelection {
        backend: RunBackend::Node,
        reason: "no packaged exact compatibility evidence matched this mouth".to_string(),
        row: None,
    }
}

fn select_auto_for_package_script(
    package_json: &std::path::Path,
    script_name: &str,
) -> AutoBackendSelection {
    let (package, version) = match package_name_version(package_json) {
        Ok((name, version)) if !name.is_empty() && !version.is_empty() => (name, version),
        Ok(_) => {
            return AutoBackendSelection {
                backend: RunBackend::Node,
                reason: "package.json lacks exact name/version for compatibility evidence matching"
                    .to_string(),
                row: None,
            };
        }
        Err(e) => {
            return AutoBackendSelection {
                backend: RunBackend::Node,
                reason: format!("cannot inspect package identity for compatibility evidence: {e}"),
                row: None,
            };
        }
    };
    select_auto_for_exact_mouth(&package, &version, &format!("package-script:{script_name}"))
}

fn select_auto_for_direct_file(target: &str) -> AutoBackendSelection {
    let name = std::path::Path::new(target)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(target);
    select_auto_for_exact_mouth("(local)", "*", &format!("direct-file:{name}"))
}

fn package_scripts(package_json: &std::path::Path) -> Result<Vec<String>, String> {
    let text = std::fs::read_to_string(package_json)
        .map_err(|e| format!("cannot read {}: {}", package_json.display(), e))?;
    let json: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| format!("cannot parse {}: {}", package_json.display(), e))?;
    let mut scripts: Vec<String> = json
        .get("scripts")
        .and_then(|v| v.as_object())
        .map(|scripts| scripts.keys().cloned().collect())
        .unwrap_or_default();
    scripts.sort();
    Ok(scripts)
}

fn row_package_matches(row: &CompatibilityEvidenceRow, package: &str) -> bool {
    row.package.eq_ignore_ascii_case(package)
}

fn format_row_backend(row: &CompatibilityEvidenceRow) -> &'static str {
    if row_admits_cruft(row) {
        "cruft"
    } else {
        "node"
    }
}

fn print_compat_row(row: &CompatibilityEvidenceRow) {
    println!(
        "  - {}@{} [{}] {} -> {}",
        row.package, row.version, row.level, row.mouth, row.status
    );
    println!("    recommended_backend: {}", format_row_backend(row));
    if !row.caveat.trim().is_empty() {
        println!("    caveat: {}", row.caveat);
    }
    if !row.pipeline.trim().is_empty() {
        println!("    pipeline: {}", row.pipeline);
    }
    if !row.evidence_commit.trim().is_empty() {
        println!("    evidence_commit: {}", row.evidence_commit);
    }
    if !row.date.trim().is_empty() {
        println!("    date: {}", row.date);
    }
}

fn print_compat_try_next(rows: &[CompatibilityEvidenceRow], target: &str) {
    println!("Try next:");
    let mut printed = 0usize;
    for row in rows.iter().filter(|row| row_admits_cruft(row)) {
        if let Some(script) = row.mouth.strip_prefix("package-script:") {
            println!("  cruft run --backend=auto --explain {script}");
            printed += 1;
        } else if let Some(file) = row.mouth.strip_prefix("direct-file:") {
            println!("  cruft run --backend=auto --explain {file}");
            printed += 1;
        }
    }
    if printed == 0 {
        println!("  cruft run --backend=node <script-or-file>");
        println!("  cruft doctor --security");
        println!("  open substrate pipeline for {target} if this workflow fails");
    } else {
        println!("  cruft run --backend=node <same-mouth>  # override to Node compatibility floor");
    }
}

#[derive(Clone, Debug)]
struct TrustRisk {
    kind: String,
    level: String,
    subject: String,
    reason: String,
}

impl TrustRisk {
    fn evidence_source(&self) -> &'static str {
        match self.kind.as_str() {
            "known-malicious-advisory" | "known-vulnerability" => "osv",
            "lifecycle-script"
            | "native-addon"
            | "git-dependency"
            | "tarball-dependency"
            | "novelty-package-risk" => "local-package-metadata",
            "os-process-sandbox" => "sandbox-policy",
            _ => "frontdoor-policy",
        }
    }

    fn evidence_id(&self) -> Option<&str> {
        match self.kind.as_str() {
            "known-malicious-advisory" | "known-vulnerability" => self
                .reason
                .split_whitespace()
                .next()
                .filter(|s| !s.is_empty()),
            _ => None,
        }
    }

    fn summary(&self) -> &str {
        if let Some(id) = self.evidence_id() {
            self.reason
                .strip_prefix(id)
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .unwrap_or(&self.reason)
        } else {
            &self.reason
        }
    }

    fn owner(&self) -> &'static str {
        match self.kind.as_str() {
            "known-malicious-advisory"
            | "known-vulnerability"
            | "lifecycle-script"
            | "native-addon"
            | "git-dependency"
            | "tarball-dependency"
            | "novelty-package-risk" => "front-door-trust-policy",
            "os-process-sandbox" => "node-wrapper-os-sandbox",
            _ => "front-door-policy",
        }
    }

    fn enforced_for_install(&self, install_enforced: bool) -> bool {
        install_enforced && trust_risk_blocks_install(self)
    }

    fn action_for_install(&self, install_enforced: bool) -> &'static str {
        if self.enforced_for_install(install_enforced) {
            "block"
        } else {
            "proceed"
        }
    }

    fn consequence_for_install(&self, install_enforced: bool) -> &'static str {
        if self.enforced_for_install(install_enforced) {
            "npm was not launched"
        } else {
            "npm install will proceed unless you rerun with --enforce"
        }
    }

    fn next_action_for_install(&self, install_enforced: bool) -> &'static str {
        if self.enforced_for_install(install_enforced) {
            "inspect with cruft doctor --security --json"
        } else if trust_risk_blocks_install(self) {
            "cruft trust install --enforce -- npm install"
        } else {
            "review with cruft doctor --security"
        }
    }
}

fn print_trust_risk(risk: &TrustRisk) {
    print_trust_risk_with_mode(risk, false);
}

fn print_trust_risk_with_mode(risk: &TrustRisk, install_enforced: bool) {
    let blocked = if risk.enforced_for_install(install_enforced) {
        "blocked "
    } else {
        ""
    };
    let evidence_id = risk.evidence_id().unwrap_or("");
    let evidence_suffix = if evidence_id.is_empty() {
        String::new()
    } else {
        format!(" {evidence_id}")
    };
    println!(
        "cruft security: {}{} {}{}",
        blocked, risk.kind, risk.subject, evidence_suffix
    );
    println!(
        "level: {}; enforced: {}; action: {}; source: {}; summary: {}",
        risk.level,
        risk.enforced_for_install(install_enforced),
        risk.consequence_for_install(install_enforced),
        risk.evidence_source(),
        risk.summary()
    );
    println!("next: {}", risk.next_action_for_install(install_enforced));
}

fn npm_install_command_kind(args: &[String]) -> Option<&'static str> {
    let program = args
        .first()?
        .rsplit('/')
        .next()
        .unwrap_or(args.first()?.as_str());
    if program != "npm" {
        return None;
    }
    match args.get(1).map(|s| s.as_str()) {
        Some("install") | Some("i") | Some("add") => Some("npm-install"),
        _ => None,
    }
}

fn package_json_object(path: &std::path::Path) -> Result<serde_json::Value, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("cannot read {}: {}", path.display(), e))?;
    serde_json::from_str::<serde_json::Value>(&text)
        .map_err(|e| format!("cannot parse {}: {}", path.display(), e))
}

fn collect_manifest_trust_risks(package_json: &std::path::Path) -> Vec<TrustRisk> {
    let mut risks = Vec::new();
    let Ok(json) = package_json_object(package_json) else {
        return risks;
    };
    if let Some(scripts) = json.get("scripts").and_then(|v| v.as_object()) {
        for name in ["preinstall", "install", "postinstall", "prepare"] {
            if let Some(script) = scripts.get(name).and_then(|v| v.as_str()) {
                risks.push(TrustRisk {
                    kind: "lifecycle-script".to_string(),
                    level: "advisory".to_string(),
                    subject: name.to_string(),
                    reason: script.to_string(),
                });
            }
        }
    }
    for field in ["dependencies", "devDependencies", "optionalDependencies"] {
        if let Some(deps) = json.get(field).and_then(|v| v.as_object()) {
            for (name, value) in deps {
                let Some(spec) = value.as_str() else {
                    continue;
                };
                let spec_lower = spec.to_ascii_lowercase();
                let risk_kind = if spec_lower.starts_with("git+")
                    || spec_lower.starts_with("git:")
                    || spec_lower.contains("github:")
                    || spec_lower.contains("gitlab:")
                {
                    Some("git-dependency")
                } else if spec_lower.starts_with("http://")
                    || spec_lower.starts_with("https://")
                    || spec_lower.ends_with(".tgz")
                {
                    Some("tarball-dependency")
                } else if spec == "*" || spec_lower == "latest" {
                    Some("novelty-package-risk")
                } else {
                    None
                };
                if let Some(kind) = risk_kind {
                    risks.push(TrustRisk {
                        kind: kind.to_string(),
                        level: "advisory".to_string(),
                        subject: format!("{name}@{spec}"),
                        reason: format!("{field} uses a non-pinned or non-registry-shaped spec"),
                    });
                }
            }
        }
    }
    if package_json
        .parent()
        .map(|p| p.join("binding.gyp").is_file())
        .unwrap_or(false)
        || json
            .get("gypfile")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    {
        risks.push(TrustRisk {
            kind: "native-addon".to_string(),
            level: "advisory".to_string(),
            subject: package_json.display().to_string(),
            reason: "native build marker observed".to_string(),
        });
    }
    risks
}

fn collect_lockfile_trust_risks(lockfile: &std::path::Path) -> Vec<TrustRisk> {
    let mut risks = Vec::new();
    let Ok(json) = package_json_object(lockfile) else {
        return risks;
    };
    if let Some(packages) = json.get("packages").and_then(|v| v.as_object()) {
        for (path, rec) in packages {
            if path.is_empty() {
                continue;
            }
            let name = rec
                .get("name")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .or_else(|| package_name_from_node_modules_path(path))
                .unwrap_or_else(|| path.to_string());
            if rec
                .get("hasInstallScript")
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
            {
                risks.push(TrustRisk {
                    kind: "lifecycle-script".to_string(),
                    level: "advisory".to_string(),
                    subject: name.clone(),
                    reason: "package-lock hasInstallScript=true".to_string(),
                });
            }
            if rec
                .get("gypfile")
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
                || path.ends_with("binding.gyp")
            {
                risks.push(TrustRisk {
                    kind: "native-addon".to_string(),
                    level: "advisory".to_string(),
                    subject: name,
                    reason: "lockfile native build marker observed".to_string(),
                });
            }
        }
    }
    risks
}

fn collect_osv_trust_risks(lockfile: Option<&std::path::Path>) -> Result<Vec<TrustRisk>, String> {
    let Some(lockfile) = lockfile else {
        return Ok(Vec::new());
    };
    let packages = extract_npm_lock_packages(lockfile)?;
    if packages.is_empty() {
        return Ok(Vec::new());
    }
    let findings = query_osv_for_packages(&packages)?;
    Ok(findings
        .into_iter()
        .map(|finding| TrustRisk {
            kind: if finding.malicious {
                "known-malicious-advisory"
            } else {
                "known-vulnerability"
            }
            .to_string(),
            level: "advisory".to_string(),
            subject: format!("{}@{}", finding.package.name, finding.package.version),
            reason: format!("{} {}", finding.id, finding.summary)
                .trim()
                .to_string(),
        })
        .collect())
}

fn trust_project_evidence_summary(package_json: Option<&std::path::Path>) -> String {
    let Some(package_json) = package_json else {
        return "no package.json; run cruft compat <package> for packaged evidence".to_string();
    };
    let Ok((package, version)) = package_name_version(package_json) else {
        return "package identity unavailable; run cruft compat <package> manually".to_string();
    };
    if package.is_empty() || version.is_empty() {
        return "package name/version missing; exact-mouth Cruft evidence cannot match this project".to_string();
    }
    let Ok(rows) = packaged_compatibility_rows() else {
        return "packaged compatibility evidence index unavailable".to_string();
    };
    let mut exact = 0usize;
    let mut executable = 0usize;
    for row in rows {
        if row.package == package && row.version == version {
            exact += 1;
            if row_admits_cruft(&row) {
                executable += 1;
            }
        }
    }
    if exact == 0 {
        format!("no exact packaged rows for {package}@{version}; Node remains compatibility floor")
    } else {
        format!("{exact} exact packaged row(s) for {package}@{version}; {executable} exact executable Cruft mouth(s)")
    }
}

fn trust_risk_blocks_install(risk: &TrustRisk) -> bool {
    matches!(
        risk.kind.as_str(),
        "lifecycle-script"
            | "native-addon"
            | "known-malicious-advisory"
            | "known-vulnerability"
            | "git-dependency"
            | "tarball-dependency"
            | "novelty-package-risk"
    )
}

fn run_trust_subcommand(args: &[String], inherited_audit_log: Option<String>) -> ExitCode {
    if args
        .first()
        .map(|s| s == "--help" || s == "-h")
        .unwrap_or(false)
    {
        println!("Cruft trust");
        println!();
        println!("Usage:");
        println!("  cruft trust install");
        println!("  cruft trust install --enforce");
        println!("  cruft trust install -- npm install");
        println!("  cruft trust install --enforce -- npm install");
        println!();
        println!("Advisory by default: reports lifecycle scripts, native addon markers, git/tarball specs, OSV advisories, novelty risk, and Cruft evidence.");
        println!("With --enforce, reported lifecycle, native, vulnerability, malicious-advisory, git/tarball, and novelty risks block before npm launch.");
        return ExitCode::SUCCESS;
    }
    if args.first().map(|s| s.as_str()) != Some("install") {
        eprintln!("cruft trust: expected install");
        return ExitCode::from(64);
    }
    run_trust_install(&args[1..], inherited_audit_log)
}

fn run_trust_install(args: &[String], inherited_audit_log: Option<String>) -> ExitCode {
    let mut audit_log: Option<String> = inherited_audit_log;
    let mut enforce_install = false;
    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--enforce" => {
                enforce_install = true;
                i += 1;
            }
            "--audit-log" => {
                let Some(value) = args.get(i + 1) else {
                    eprintln!("cruft trust install: --audit-log requires an argument");
                    return ExitCode::from(64);
                };
                audit_log = Some(value.clone());
                i += 2;
            }
            a if a.starts_with("--audit-log=") => {
                audit_log = Some(a["--audit-log=".len()..].to_string());
                i += 1;
            }
            "--" => break,
            a if a.starts_with('-') => {
                eprintln!("cruft trust install: unknown option {a}");
                return ExitCode::from(64);
            }
            _ => break,
        }
    }
    let command_args = if args.get(i).map(|s| s == "--").unwrap_or(false) {
        args[i + 1..].to_vec()
    } else {
        Vec::new()
    };
    if !command_args.is_empty() && npm_install_command_kind(&command_args).is_none() {
        eprintln!("cruft trust install: expected command after -- to start with npm install, npm i, or npm add");
        return ExitCode::from(64);
    }
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let package_json = find_package_json_from_cwd();
    let lockfile = find_npm_lockfile_from(&cwd).map(|(_, lockfile)| lockfile);
    let mut risks = Vec::new();
    if let Some(package_json) = &package_json {
        risks.extend(collect_manifest_trust_risks(package_json));
    }
    if let Some(lockfile) = &lockfile {
        risks.extend(collect_lockfile_trust_risks(lockfile));
    }
    let osv_risks = match collect_osv_trust_risks(lockfile.as_deref()) {
        Ok(risks) => risks,
        Err(e) => {
            eprintln!("cruft trust install: OSV scan failed: {e}");
            return ExitCode::from(1);
        }
    };
    risks.extend(osv_risks);
    risks.sort_by(|a, b| (&a.kind, &a.subject, &a.reason).cmp(&(&b.kind, &b.subject, &b.reason)));
    risks.dedup_by(|a, b| a.kind == b.kind && a.subject == b.subject && a.reason == b.reason);

    println!("Cruft trust install");
    if enforce_install {
        println!(
            "Mode: enforced preflight; reported install/package risks block before npm launch."
        );
    } else {
        println!("Mode: advisory preflight; install blocking is not enforced.");
    }
    println!("Project root: {}", cwd.display());
    println!(
        "Package manifest: {}",
        package_json
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "none".to_string())
    );
    println!(
        "Lockfile: {}",
        lockfile
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "none".to_string())
    );
    if command_args.is_empty() {
        println!("Command: preflight only");
    } else {
        println!("Command: {}", command_args.join(" "));
    }
    println!("Policy:");
    for control in frontdoor_policy_controls_with_install_enforcement("ci", true, enforce_install) {
        println!(
            "  control={} level={} reason={} next_action={} owner={}",
            control.control,
            control.level,
            control.reason,
            frontdoor_policy_control_next_action(control.control, control.level),
            frontdoor_policy_control_owner(control.control)
        );
    }
    println!(
        "Cruft evidence: {}",
        trust_project_evidence_summary(package_json.as_deref())
    );
    println!("Risks: {}", risks.len());
    for risk in &risks {
        print_trust_risk_with_mode(risk, enforce_install);
    }

    if command_args.is_empty() {
        if enforce_install && risks.iter().any(trust_risk_blocks_install) {
            println!("Result: enforceable risks found; provide an npm install command after -- to block before launch.");
        } else if enforce_install {
            println!(
                "Result: enforced preflight passed. No blocking install/package risks were found."
            );
        } else {
            println!("Result: advisory only. Cruft is not blocking npm install and is not sandboxing Node child processes.");
        }
        println!("Try next:");
        if enforce_install {
            println!("  cruft trust install --enforce -- npm install");
        } else if risks.iter().any(trust_risk_blocks_install) {
            println!("  cruft trust install --enforce -- npm install");
        } else {
            println!("  cruft trust install -- npm install");
        }
        append_frontdoor_audit_log(
            audit_log.as_deref(),
            "cruft trust install",
            &[],
            "node",
            None,
            None,
            &risks,
            Some(0),
            enforce_install,
            false,
        );
        return ExitCode::SUCCESS;
    }

    if enforce_install && risks.iter().any(trust_risk_blocks_install) {
        println!(
            "Result: blocked by enforced install/package-script policy. npm was not launched."
        );
        println!("Blocked risks:");
        for risk in risks.iter().filter(|risk| trust_risk_blocks_install(risk)) {
            print_trust_risk_with_mode(risk, true);
        }
        append_frontdoor_audit_log(
            audit_log.as_deref(),
            "cruft trust install",
            &command_args,
            "node",
            Some("ci"),
            None,
            &risks,
            Some(77),
            true,
            false,
        );
        return ExitCode::from(77);
    }

    if enforce_install {
        println!("Result: enforced preflight passed. Launching npm install because no blocking install/package risks were found.");
    } else {
        println!("Result: advisory only. Cruft is not blocking npm install and is not sandboxing Node child processes.");
    }
    eprintln!("cruft trust install: launching advisory pass-through command");
    let mut cmd = std::process::Command::new(&command_args[0]);
    cmd.args(&command_args[1..])
        .stdin(std::process::Stdio::inherit())
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit());
    match cmd.status() {
        Ok(status) => {
            let code = status.code().map(|code| (code & 0xff) as u8).unwrap_or(1);
            append_frontdoor_audit_log(
                audit_log.as_deref(),
                "cruft trust install",
                &command_args,
                "node",
                None,
                None,
                &risks,
                Some(code),
                enforce_install,
                false,
            );
            ExitCode::from(code)
        }
        Err(e) => {
            eprintln!(
                "cruft trust install: cannot execute {:?}: {e}",
                command_args[0]
            );
            append_frontdoor_audit_log(
                audit_log.as_deref(),
                "cruft trust install",
                &command_args,
                "node",
                None,
                None,
                &risks,
                Some(66),
                enforce_install,
                false,
            );
            ExitCode::from(66)
        }
    }
}

fn run_promote_subcommand(args: &[String]) -> ExitCode {
    if args
        .first()
        .map(|s| s == "--help" || s == "-h")
        .unwrap_or(false)
    {
        println!("Cruft runtime promotion map");
        println!();
        println!("Usage:");
        println!("  cruft promote");
        println!();
        println!("Reports project package-script mouths that have exact packaged evidence for Cruft runtime promotion. Node remains the compatibility floor for every unproven mouth.");
        return ExitCode::SUCCESS;
    }
    if !args.is_empty() {
        eprintln!("cruft promote: unexpected argument {}", args[0]);
        return ExitCode::from(64);
    }
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    println!("Cruft runtime promotion map");
    println!("Project root: {}", cwd.display());
    println!("Evidence source: packaged compatibility evidence index");
    println!("Policy: exact executable mouths only; no broad package/runtime substitution claim.");
    println!();
    let Some(package_json) = find_package_json_from_cwd() else {
        println!("Package: none");
        println!("Cruft-ready mouths: 0");
        println!("Node-required mouths: 0");
        println!("Action: run from a project with package.json.");
        return ExitCode::SUCCESS;
    };
    let (package, version) = match package_name_version(&package_json) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("cruft promote: {e}");
            return ExitCode::from(1);
        }
    };
    let scripts = match package_scripts(&package_json) {
        Ok(scripts) => scripts,
        Err(e) => {
            eprintln!("cruft promote: {e}");
            return ExitCode::from(1);
        }
    };
    println!(
        "Package: {}@{}",
        if package.is_empty() {
            "(unnamed)"
        } else {
            &package
        },
        if version.is_empty() {
            "(unversioned)"
        } else {
            &version
        }
    );
    println!(
        "Scripts: {}",
        if scripts.is_empty() {
            "none".to_string()
        } else {
            scripts.join(", ")
        }
    );
    let rows = match packaged_compatibility_rows() {
        Ok(rows) => rows,
        Err(e) => {
            eprintln!("cruft promote: {e}");
            return ExitCode::from(1);
        }
    };
    let exact_rows: Vec<CompatibilityEvidenceRow> = rows
        .into_iter()
        .filter(|row| row.package == package && row.version == version)
        .collect();
    let mut cruft_ready = Vec::new();
    let mut node_required = Vec::new();
    for script in &scripts {
        let mouth = format!("package-script:{script}");
        let row = exact_rows.iter().find(|row| row.mouth == mouth).cloned();
        if row.as_ref().map(row_admits_cruft).unwrap_or(false) {
            cruft_ready.push((script.clone(), row));
        } else {
            node_required.push((script.clone(), row));
        }
    }
    println!("Cruft-ready mouths: {}", cruft_ready.len());
    for (script, row) in &cruft_ready {
        println!("  - package-script:{script} backend=cruft");
        if let Some(row) = row {
            println!(
                "    status={} level={} commit={}",
                row.status, row.level, row.evidence_commit
            );
            if !row.caveat.trim().is_empty() {
                println!("    caveat={}", row.caveat);
            }
        }
        println!("    try=cruft run --backend=auto --explain {script}");
    }
    println!("Node-required mouths: {}", node_required.len());
    for (script, row) in &node_required {
        println!("  - package-script:{script} backend=node");
        match row {
            Some(row) => {
                println!(
                    "    status={} level={} pipeline={}",
                    row.status, row.level, row.pipeline
                );
                if !row.caveat.trim().is_empty() {
                    println!("    caveat={}", row.caveat);
                }
                if row.pipeline.trim().is_empty()
                    || row.pipeline.trim().eq_ignore_ascii_case("none")
                {
                    println!("    owner=no executable Cruft admission for this mouth");
                } else {
                    println!("    owner={}", row.pipeline);
                }
            }
            None => {
                println!("    owner=no exact packaged evidence row");
            }
        }
        println!("    try=cruft run --backend=node {script}");
    }
    let other_rows: Vec<CompatibilityEvidenceRow> = exact_rows
        .into_iter()
        .filter(|row| !row.mouth.starts_with("package-script:"))
        .collect();
    if !other_rows.is_empty() {
        println!("Other exact package evidence:");
        for row in &other_rows {
            println!(
                "  - mouth={} backend={} status={} pipeline={}",
                row.mouth,
                format_row_backend(row),
                row.status,
                if row.pipeline.trim().is_empty() {
                    "none"
                } else {
                    &row.pipeline
                }
            );
        }
    }
    println!();
    println!("Result: promotion map only. Cruft does not silently replace Node for unproven project mouths.");
    ExitCode::SUCCESS
}

fn run_compat_subcommand(args: &[String]) -> ExitCode {
    if args.len() != 1
        || args
            .first()
            .map(|s| s == "--help" || s == "-h")
            .unwrap_or(false)
    {
        println!("Cruft compatibility cockpit");
        println!();
        println!("Usage:");
        println!("  cruft compat .");
        println!("  cruft compat <package>");
        println!("  cruft compat script:<name>");
        println!();
        println!("Reports packaged compatibility evidence only. It never claims whole-package compatibility from a single entry point.");
        return if args.len() == 1 {
            ExitCode::SUCCESS
        } else {
            ExitCode::from(64)
        };
    }
    let target = &args[0];
    let rows = match packaged_compatibility_rows() {
        Ok(rows) => rows,
        Err(e) => {
            eprintln!("cruft compat: {e}");
            return ExitCode::from(1);
        }
    };
    println!("Cruft compatibility cockpit");
    println!("Target: {target}");
    println!("Evidence source: packaged compatibility evidence index");
    println!("Policy: exact-mouth only; Node is the compatibility floor.");
    println!();

    if target == "." || target == "project" {
        let Some(package_json) = find_package_json_from_cwd() else {
            println!("Project: no package.json found from current directory");
            println!("Recommended backend: node");
            println!("Try next:");
            println!("  cruft compat <package>");
            println!("  cruft doctor");
            return ExitCode::SUCCESS;
        };
        let (package, version) = match package_name_version(&package_json) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("cruft compat: {e}");
                return ExitCode::from(1);
            }
        };
        println!(
            "Project root: {}",
            package_json
                .parent()
                .unwrap_or_else(|| std::path::Path::new("."))
                .display()
        );
        println!(
            "Package: {}@{}",
            if package.is_empty() {
                "(unnamed)"
            } else {
                &package
            },
            if version.is_empty() {
                "(unversioned)"
            } else {
                &version
            }
        );
        match package_scripts(&package_json) {
            Ok(scripts) if !scripts.is_empty() => println!("Scripts: {}", scripts.join(", ")),
            Ok(_) => println!("Scripts: none"),
            Err(e) => println!("Scripts: unavailable ({e})"),
        }
        let matches: Vec<CompatibilityEvidenceRow> = rows
            .into_iter()
            .filter(|row| row.package == package && row.version == version)
            .collect();
        println!();
        if matches.is_empty() {
            println!("Evidence: no exact package/version rows found");
            println!("Recommended backend: node");
            print_compat_try_next(&[], target);
            return ExitCode::SUCCESS;
        }
        let admits = matches.iter().any(row_admits_cruft);
        println!(
            "Recommended backend: {}",
            if admits {
                "cruft for exact listed mouths; node otherwise"
            } else {
                "node"
            }
        );
        println!("Evidence rows:");
        for row in &matches {
            print_compat_row(row);
        }
        println!();
        print_compat_try_next(&matches, target);
        return ExitCode::SUCCESS;
    }

    if let Some(script) = target
        .strip_prefix("script:")
        .or_else(|| target.strip_prefix("package-script:"))
    {
        let Some(package_json) = find_package_json_from_cwd() else {
            println!("Script: {script}");
            println!("Evidence: no package.json found from current directory");
            println!("Recommended backend: node");
            print_compat_try_next(&[], target);
            return ExitCode::SUCCESS;
        };
        let selection = select_auto_for_package_script(&package_json, script);
        println!("Script mouth: package-script:{script}");
        println!("Recommended backend: {}", selection.backend.name());
        println!("Reason: {}", selection.reason);
        if let Some(row) = selection.row {
            println!("Evidence rows:");
            print_compat_row(&row);
            println!();
            print_compat_try_next(&[row], target);
        } else {
            println!("Evidence: no exact executable PASS row found");
            println!();
            print_compat_try_next(&[], target);
        }
        return ExitCode::SUCCESS;
    }

    let matches: Vec<CompatibilityEvidenceRow> = rows
        .into_iter()
        .filter(|row| row_package_matches(row, target))
        .collect();
    if matches.is_empty() {
        println!("Evidence: no packaged rows found for package {target}");
        println!("Recommended backend: node");
        print_compat_try_next(&[], target);
        return ExitCode::SUCCESS;
    }
    let admits = matches.iter().any(row_admits_cruft);
    println!(
        "Recommended backend: {}",
        if admits {
            "cruft for exact listed mouths; node otherwise"
        } else {
            "node"
        }
    );
    println!("Evidence rows:");
    for row in &matches {
        print_compat_row(row);
    }
    println!();
    print_compat_try_next(&matches, target);
    ExitCode::SUCCESS
}

fn explain_run_backend(
    requested: RunBackend,
    effective: RunBackend,
    selection: Option<&AutoBackendSelection>,
) {
    if let Some(selection) = selection {
        eprintln!(
            "cruft run: backend=auto selected={} reason={}",
            effective.name(),
            selection.reason
        );
        if let Some(row) = &selection.row {
            eprintln!(
                "cruft run: evidence package={} version={} level={} mouth={} status={} commit={}",
                row.package, row.version, row.level, row.mouth, row.status, row.evidence_commit
            );
            if !row.caveat.trim().is_empty() {
                eprintln!("cruft run: evidence caveat={}", row.caveat);
            }
        }
        eprintln!(
            "cruft run: policy=evidence-advisory override with --backend=node or --backend=cruft"
        );
    } else {
        eprintln!("cruft run: backend={}", requested.name());
    }
    if effective == RunBackend::Node {
        eprintln!("cruft run: note Node child processes are not constrained by Cruft runtime caps");
    }
}

fn run_package_script(
    script_name: &str,
    script: &str,
    package_json: &std::path::Path,
    forwarded: &[String],
    backend: RunBackend,
) -> ExitCode {
    let package_dir = package_json
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."));
    let mut words = match shell_words_simple(script) {
        Ok(w) if !w.is_empty() => w,
        Ok(_) => {
            eprintln!("cruft run: script {:?} is empty", script_name);
            return ExitCode::from(64);
        }
        Err(e) => {
            eprintln!("cruft run: cannot parse script {:?}: {}", script_name, e);
            return ExitCode::from(64);
        }
    };
    let command_word = words.remove(0);
    let resolved = resolve_package_script_command(package_dir, &command_word);
    let path_env = path_with_local_bin(package_dir);
    let mut command_args = words;
    command_args.extend_from_slice(forwarded);

    let is_js_bin = is_js_or_node_bin(&resolved);
    let exe = std::env::current_exe().ok();
    if backend == RunBackend::Node {
        let mut node_args = Vec::new();
        let program = if resolved.is_file() && is_js_bin {
            node_args.push(resolved.to_string_lossy().into_owned());
            std::path::PathBuf::from("node")
        } else {
            resolved.clone()
        };
        node_args.extend(command_args);
        let envs = [
            ("npm_lifecycle_event", std::ffi::OsString::from(script_name)),
            ("npm_lifecycle_script", std::ffi::OsString::from(script)),
            ("npm_package_json", package_json.as_os_str().to_os_string()),
            ("npm_config_node_gyp", std::ffi::OsString::from("")),
            (
                "NODE",
                std::ffi::OsString::from(
                    std::env::var("CRUFT_NODE").unwrap_or_else(|_| "node".to_string()),
                ),
            ),
        ];
        return run_node_backend_command(
            &program,
            &node_args,
            Some(package_dir),
            Some(path_env),
            &envs,
        );
    }

    let mut cmd = if resolved.is_file() && is_js_bin {
        let mut c = std::process::Command::new(
            exe.as_deref()
                .unwrap_or_else(|| std::path::Path::new("cruft")),
        );
        c.arg(&resolved);
        c
    } else {
        std::process::Command::new(&resolved)
    };
    cmd.args(&command_args)
        .current_dir(package_dir)
        .env("PATH", path_env)
        .env("npm_lifecycle_event", script_name)
        .env("npm_lifecycle_script", script)
        .env("npm_package_json", package_json)
        .env("npm_config_node_gyp", "")
        .env(
            "NODE",
            exe.unwrap_or_else(|| std::path::PathBuf::from("cruft")),
        );
    match cmd.status() {
        Ok(status) => match status.code() {
            Some(code) => ExitCode::from(code as u8),
            None => ExitCode::from(1),
        },
        Err(e) => {
            eprintln!(
                "cruft run: cannot execute script {:?} command {:?}: {}",
                script_name, command_word, e
            );
            ExitCode::from(66)
        }
    }
}

fn run_package_executable(
    command_name: &str,
    resolved: &std::path::Path,
    package_dir: &std::path::Path,
    forwarded: &[String],
    backend: RunBackend,
) -> ExitCode {
    let path_env = path_with_local_bin(package_dir);
    let is_js_bin = is_js_or_node_bin(resolved);
    #[cfg(windows)]
    let windows_cmd_node_target = recover_windows_cmd_node_target(resolved);
    if backend == RunBackend::Node {
        #[cfg(windows)]
        if resolved.is_file() && has_windows_command_extension(resolved) {
            let mut cmd = std::process::Command::new("cmd");
            cmd.arg("/d")
                .arg("/c")
                .arg(resolved)
                .args(forwarded)
                .current_dir(package_dir)
                .env("PATH", path_env);
            return match cmd.status() {
                Ok(status) => match status.code() {
                    Some(code) => ExitCode::from((code & 0xff) as u8),
                    None => ExitCode::from(1),
                },
                Err(e) => {
                    eprintln!(
                        "cruft exec: cannot execute Windows command shim {:?} at {}: {}",
                        command_name,
                        resolved.display(),
                        e
                    );
                    ExitCode::from(66)
                }
            };
        }
        #[cfg(windows)]
        if resolved.is_file() && has_windows_powershell_extension(resolved) {
            let mut cmd = std::process::Command::new("powershell");
            cmd.arg("-NoProfile")
                .arg("-ExecutionPolicy")
                .arg("Bypass")
                .arg("-File")
                .arg(resolved)
                .args(forwarded)
                .current_dir(package_dir)
                .env("PATH", path_env);
            return match cmd.status() {
                Ok(status) => match status.code() {
                    Some(code) => ExitCode::from((code & 0xff) as u8),
                    None => ExitCode::from(1),
                },
                Err(e) => {
                    eprintln!(
                        "cruft exec: cannot execute Windows PowerShell shim {:?} at {}: {}",
                        command_name,
                        resolved.display(),
                        e
                    );
                    ExitCode::from(66)
                }
            };
        }
        #[cfg(windows)]
        if resolved.is_file() && !is_js_bin && is_probably_text_file(resolved) {
            eprintln!(
                "cruft exec: cannot execute {:?} at {} on Windows: text package bins require a .cmd/.ps1/native launcher or an explicit shell substrate",
                command_name,
                resolved.display()
            );
            return ExitCode::from(66);
        }
        let mut node_args = Vec::new();
        let program = if resolved.is_file() && is_js_bin {
            node_args.push(resolved.to_string_lossy().into_owned());
            std::path::PathBuf::from("node")
        } else {
            resolved.to_path_buf()
        };
        node_args.extend_from_slice(forwarded);
        return run_node_backend_command(&program, &node_args, None, Some(path_env), &[]);
    }

    let exe = std::env::current_exe().ok();
    #[cfg(windows)]
    let cruft_js_target = windows_cmd_node_target.as_deref().unwrap_or(resolved);
    #[cfg(not(windows))]
    let cruft_js_target = resolved;
    #[cfg(windows)]
    if resolved.is_file()
        && (has_windows_command_extension(resolved) || has_windows_powershell_extension(resolved))
        && windows_cmd_node_target.is_none()
    {
        eprintln!(
            "cruft exec: cannot execute Windows shim {:?} at {} with backend=cruft: no recoverable JS target",
            command_name,
            resolved.display()
        );
        return ExitCode::from(66);
    }
    let mut cmd = if cruft_js_target.is_file() && is_js_or_node_bin(cruft_js_target) {
        let mut c = std::process::Command::new(
            exe.as_deref()
                .unwrap_or_else(|| std::path::Path::new("cruft")),
        );
        c.arg(cruft_js_target);
        if !forwarded.is_empty() {
            c.arg("--");
        }
        c.env("CRUFT_EXEC_AS_RUNTIME", "1");
        c
    } else {
        std::process::Command::new(resolved)
    };
    cmd.args(forwarded).env("PATH", path_env);
    match cmd.status() {
        Ok(status) => match status.code() {
            Some(code) => ExitCode::from((code & 0xff) as u8),
            None => ExitCode::from(1),
        },
        Err(e) => {
            eprintln!(
                "cruft exec: cannot execute {:?} at {}: {}",
                command_name,
                resolved.display(),
                e
            );
            ExitCode::from(66)
        }
    }
}

fn effective_package_exec_backend(
    requested: RunBackend,
    resolved: &std::path::Path,
    explain: bool,
) -> RunBackend {
    if requested != RunBackend::Auto {
        if explain {
            eprintln!("cruft exec: backend={}", requested.name());
        }
        return requested;
    }
    #[cfg(windows)]
    let recovered_cmd_node_target = recover_windows_cmd_node_target(resolved);
    #[cfg(windows)]
    let effective_target = recovered_cmd_node_target.as_deref().unwrap_or(resolved);
    #[cfg(not(windows))]
    let effective_target = resolved;
    let effective = if is_js_or_node_bin(effective_target) {
        RunBackend::Cruft
    } else {
        RunBackend::Node
    };
    if explain {
        let reason = if effective == RunBackend::Cruft {
            "package bin is JS-like and admitted to Cruft execution"
        } else {
            "package bin is not JS-like; routing to system/Node-compatible execution"
        };
        eprintln!(
            "cruft exec: backend=auto selected={} reason={}",
            effective.name(),
            reason
        );
        if effective == RunBackend::Node {
            eprintln!(
                "cruft exec: note routed child processes are not constrained by Cruft runtime caps"
            );
        }
    }
    effective
}

fn run_exec_subcommand(args: &[String], inherited_audit_log: Option<String>) -> ExitCode {
    if args
        .first()
        .map(|s| s == "--help" || s == "-h")
        .unwrap_or(false)
    {
        println!("Usage: cpx [--backend=cruft|node|auto] [--explain] [--no-install] [--yes] [--package <pkg[@range]>] <cmd-or-package> [--] [args...]");
        println!("       cruft exec [options] <cmd-or-package> [--] [args...]");
        println!(
            "Run a local or ephemeral package executable through Cruft's package-exec resolver."
        );
        return ExitCode::SUCCESS;
    }
    let (backend, explain, mut i) = match parse_backend_options("cruft exec", args, 0) {
        Ok(parsed) => parsed,
        Err(code) => return code,
    };
    let mut no_install = false;
    let mut yes = false;
    let mut risk = false;
    let mut risk_json = false;
    let mut deny_risk_high = false;
    let mut audit_log: Option<String> = inherited_audit_log;
    let mut package_specs: Vec<PackageExecSpec> = Vec::new();
    while i < args.len() {
        match args[i].as_str() {
            "--risk" => {
                risk = true;
                i += 1;
            }
            "--risk-json" => {
                risk_json = true;
                i += 1;
            }
            "--deny-risk=high" => {
                deny_risk_high = true;
                i += 1;
            }
            "--audit-log" => {
                let Some(value) = args.get(i + 1) else {
                    eprintln!("cruft exec: --audit-log requires an argument");
                    return ExitCode::from(64);
                };
                audit_log = Some(value.clone());
                i += 2;
            }
            a if a.starts_with("--audit-log=") => {
                audit_log = Some(a["--audit-log=".len()..].to_string());
                i += 1;
            }
            a if a.starts_with("--deny-risk=") => {
                eprintln!(
                    "cruft exec: unsupported deny risk policy {a:?}; supported: --deny-risk=high"
                );
                return ExitCode::from(64);
            }
            "--no-install" => {
                no_install = true;
                i += 1;
            }
            "--yes" | "-y" => {
                yes = true;
                i += 1;
            }
            "--package" => {
                let Some(spec) = args.get(i + 1) else {
                    eprintln!("cruft exec: --package requires an argument");
                    return ExitCode::from(64);
                };
                match parse_package_exec_spec(spec) {
                    Ok(spec) => package_specs.push(spec),
                    Err(e) => {
                        eprintln!("cruft exec: {e}");
                        return ExitCode::from(64);
                    }
                }
                i += 2;
            }
            a if a.starts_with("--package=") => {
                let value = &a["--package=".len()..];
                match parse_package_exec_spec(value) {
                    Ok(spec) => package_specs.push(spec),
                    Err(e) => {
                        eprintln!("cruft exec: {e}");
                        return ExitCode::from(64);
                    }
                }
                i += 1;
            }
            "--" => {
                i += 1;
                break;
            }
            a if a.starts_with('-') => {
                eprintln!("cruft exec: unsupported option {a:?}");
                return ExitCode::from(64);
            }
            _ => break,
        }
    }
    let Some(command) = args.get(i) else {
        eprintln!("cruft exec: missing package executable");
        return ExitCode::from(64);
    };
    let mut executable_command = command.clone();
    let forwarded = script_arg_tail(args, i + 1);

    let package_json = find_package_json_from_cwd();
    let local_package_dir = package_json
        .as_deref()
        .and_then(|p| p.parent())
        .map(|p| p.to_path_buf());
    if package_specs.is_empty() {
        if let Some(package_dir) = &local_package_dir {
            match resolve_local_package_exec(package_dir, command) {
                Ok(Some(path)) if path.is_file() => {
                    if risk || risk_json || deny_risk_high {
                        let risks = collect_package_exec_manifest_risks(package_dir);
                        if risk_json {
                            print_cpx_risk_json(
                                command,
                                &package_specs,
                                Some(package_dir),
                                Some(&path),
                            );
                        }
                        if risk {
                            print!(
                                "{}",
                                cpx_risk_human_report(
                                    command,
                                    &package_specs,
                                    "local",
                                    Some(package_dir),
                                    Some(&path),
                                    &risks
                                )
                            );
                        }
                        if deny_risk_high && risks.iter().any(trust_risk_blocks_install) {
                            eprintln!(
                                "cruft exec: blocked by --deny-risk=high before launching local package executable"
                            );
                            return ExitCode::from(77);
                        }
                        if risk || risk_json {
                            return ExitCode::SUCCESS;
                        }
                    }
                    let effective_backend = effective_package_exec_backend(backend, &path, explain);
                    if explain {
                        eprintln!(
                            "cruft exec: backend={} command={} resolved={}",
                            effective_backend.name(),
                            command,
                            path.display()
                        );
                    }
                    return run_package_executable(
                        command,
                        &path,
                        package_dir,
                        &forwarded,
                        effective_backend,
                    );
                }
                Ok(Some(path)) => {
                    eprintln!(
                        "cruft exec: resolved {:?} to {}, but it is not a file",
                        command,
                        path.display()
                    );
                    return ExitCode::from(66);
                }
                Ok(None) => {}
                Err(e) => {
                    eprintln!("cruft exec: {}", e);
                    return ExitCode::from(65);
                }
            }
        }
        if no_install {
            eprintln!(
                "cruft exec: no local executable {:?} found in node_modules; install first",
                command
            );
            return ExitCode::from(66);
        }
        match parse_package_exec_spec(command) {
            Ok(spec) => {
                executable_command = spec.name.clone();
                package_specs.push(spec);
            }
            Err(e) => {
                eprintln!("cruft exec: {e}; use --package <pkg> <cmd> for command-only names");
                return ExitCode::from(64);
            }
        }
    }

    if risk_json {
        print_cpx_risk_json(command, &package_specs, None, None);
        return ExitCode::SUCCESS;
    }
    if risk {
        print!(
            "{}",
            cpx_risk_human_report(
                command,
                &package_specs,
                "registry-unfetched",
                None,
                None,
                &[]
            )
        );
        return ExitCode::SUCCESS;
    }

    if !yes {
        eprintln!(
            "cruft exec: ephemeral package install requires --yes; pass --no-install for local-only"
        );
        return ExitCode::from(66);
    }
    eprint!(
        "{}",
        cpx_risk_human_report(
            command,
            &package_specs,
            "registry-unfetched",
            None,
            None,
            &[]
        )
    );
    if let Some(path) = audit_log.as_deref() {
        if let Err(e) = append_cpx_yes_audit(path, command, &package_specs) {
            eprintln!("cruft exec: cannot write audit log: {e}");
            return ExitCode::from(74);
        }
    }
    if let Err(e) = cpx_selected_registry_for_specs(&package_specs) {
        eprintln!("cruft exec: blocked by registry policy: {e}");
        return ExitCode::from(77);
    }
    let exec_root = match materialize_cpx_exec_root(&package_specs, explain) {
        Ok(root) => root,
        Err(e) => {
            eprintln!("cruft exec: {e}");
            return ExitCode::from(70);
        }
    };
    let resolved = match resolve_local_package_exec(&exec_root, &executable_command) {
        Ok(Some(path)) if path.is_file() => path,
        Ok(Some(path)) => {
            eprintln!(
                "cruft exec: resolved {:?} to {}, but it is not a file",
                executable_command,
                path.display()
            );
            return ExitCode::from(66);
        }
        Ok(None) => {
            eprintln!(
                "cruft exec: no executable {:?} found after materializing {}",
                executable_command,
                package_specs
                    .iter()
                    .map(|s| format!("{}@{}", s.name, s.range))
                    .collect::<Vec<_>>()
                    .join(",")
            );
            return ExitCode::from(66);
        }
        Err(e) => {
            eprintln!("cruft exec: {}", e);
            return ExitCode::from(65);
        }
    };
    let effective_backend = effective_package_exec_backend(backend, &resolved, explain);
    if explain {
        eprintln!(
            "cruft exec: backend={} command={} resolved={}",
            effective_backend.name(),
            executable_command,
            resolved.display()
        );
    }
    run_package_executable(
        &executable_command,
        &resolved,
        &exec_root,
        &forwarded,
        effective_backend,
    )
}

fn main() -> ExitCode {

    let _alloc_guard = if std::env::var("CRUFT_ALLOC_TRACK").is_ok() {
        alloctrack::enable();
        {
            use rusty_js_runtime::value as v;
            eprintln!(
                "[alloc-track] sizeof(Object)={} B (inline per GC slot) | InternalKind={} PropertyDescriptor={} RegExpResultSlots={} Value={} IndexMap<PK,PD>={}",
                std::mem::size_of::<v::Object>(),
                std::mem::size_of::<v::InternalKind>(),
                std::mem::size_of::<v::PropertyDescriptor>(),
                std::mem::size_of::<v::RegExpResultSlots>(),
                std::mem::size_of::<v::Value>(),
                std::mem::size_of::<indexmap::IndexMap<v::PropertyKey, v::PropertyDescriptor>>(),
            );
            eprintln!(
                "[alloc-track] InternalKind variants: Closure={} Function={} Generator={} RegExp={} Proxy={} BoundFn={} Promise={} BoundaryWrapper={}",
                std::mem::size_of::<v::ClosureInternals>(),
                std::mem::size_of::<v::FunctionInternals>(),
                std::mem::size_of::<v::GeneratorObject>(),
                std::mem::size_of::<v::RegExpInternals>(),
                std::mem::size_of::<v::ProxyInternals>(),
                std::mem::size_of::<v::BoundFunctionInternals>(),
                std::mem::size_of::<v::PromiseState>(),
                std::mem::size_of::<v::BoundaryWrapperInternals>(),
            );
        }
        Some(alloctrack::Guard)
    } else {
        None
    };

    const JS_MAIN_STACK_BYTES: usize = 1024 * 1024 * 1024;
    std::thread::Builder::new()
        .stack_size(JS_MAIN_STACK_BYTES)
        .spawn(|| {

            rusty_js_runtime::interp::publish_native_stack_bounds(JS_MAIN_STACK_BYTES);
            real_main()
        })
        .expect("frame-chain R4: spawn large-stack main thread")
        .join()
        .unwrap_or(ExitCode::FAILURE)
}

fn run_wrap_subcommand(raw_args: &[String]) -> ExitCode {
    if raw_args.get(2).map(|s| s.as_str()) == Some("status") {
        return run_wrap_status();
    }
    if raw_args.get(2).map(|s| s.as_str()) == Some("install") {
        return run_wrap_install_plan(&raw_args[3..]);
    }
    if raw_args
        .get(2)
        .map(|s| s == "--help" || s == "-h" || s == "help")
        .unwrap_or(false)
    {
        print_wrap_help();
        return ExitCode::SUCCESS;
    }
    let mut explain = false;
    let mut audit = false;
    let mut audit_log: Option<String> = None;
    let mut policy: Option<String> = None;
    let mut sandbox: Option<WrapSandbox> = None;
    let mut timeout_ms: Option<u64> = None;
    let mut i = 2;
    while i < raw_args.len() {
        match raw_args[i].as_str() {
            "--" => {
                i += 1;
                break;
            }
            "--explain" => {
                explain = true;
                i += 1;
            }
            "--audit" => {
                audit = true;
                i += 1;
            }
            "--audit-log" => {
                let Some(value) = raw_args.get(i + 1) else {
                    eprintln!("cruft wrap: --audit-log requires an argument");
                    return ExitCode::from(64);
                };
                audit_log = Some(value.clone());
                i += 2;
            }
            "--policy" => {
                let Some(value) = raw_args.get(i + 1) else {
                    eprintln!("cruft wrap: --policy requires an argument");
                    return ExitCode::from(64);
                };
                if !known_frontdoor_policy_profile(value) {
                    eprintln!("cruft wrap: unknown policy profile {value:?}; supported: audit, ci");
                    return ExitCode::from(64);
                }
                policy = Some(value.clone());
                i += 2;
            }
            a if a.starts_with("--policy=") => {
                let value = &a["--policy=".len()..];
                if !known_frontdoor_policy_profile(value) {
                    eprintln!("cruft wrap: unknown policy profile {value:?}; supported: audit, ci");
                    return ExitCode::from(64);
                }
                policy = Some(value.to_string());
                i += 1;
            }
            a if a.starts_with("--audit-log=") => {
                audit_log = Some(a["--audit-log=".len()..].to_string());
                i += 1;
            }
            "--timeout-ms" => {
                let Some(value) = raw_args.get(i + 1) else {
                    eprintln!("cruft wrap: --timeout-ms requires an argument");
                    return ExitCode::from(64);
                };
                match value.parse::<u64>() {
                    Ok(ms) if ms > 0 => timeout_ms = Some(ms),
                    _ => {
                        eprintln!("cruft wrap: --timeout-ms requires a positive integer");
                        return ExitCode::from(64);
                    }
                }
                i += 2;
            }
            a if a.starts_with("--timeout-ms=") => {
                let value = &a["--timeout-ms=".len()..];
                match value.parse::<u64>() {
                    Ok(ms) if ms > 0 => timeout_ms = Some(ms),
                    _ => {
                        eprintln!("cruft wrap: --timeout-ms requires a positive integer");
                        return ExitCode::from(64);
                    }
                }
                i += 1;
            }
            "--sandbox" => {
                let Some(value) = raw_args.get(i + 1) else {
                    eprintln!("cruft wrap: --sandbox requires an argument");
                    return ExitCode::from(64);
                };
                let Some(parsed) = parse_wrap_sandbox(value) else {
                    eprintln!(
                        "cruft wrap: unknown sandbox profile {value:?}; known profiles: {}",
                        WrapSandbox::known_profiles()
                    );
                    return ExitCode::from(64);
                };
                sandbox = Some(parsed);
                i += 2;
            }
            a if a.starts_with("--sandbox=") => {
                let value = &a["--sandbox=".len()..];
                let Some(parsed) = parse_wrap_sandbox(value) else {
                    eprintln!(
                        "cruft wrap: unknown sandbox profile {value:?}; known profiles: {}",
                        WrapSandbox::known_profiles()
                    );
                    return ExitCode::from(64);
                };
                sandbox = Some(parsed);
                i += 1;
            }
            a if a.starts_with('-') => {
                eprintln!("cruft wrap: unknown option {a}");
                return ExitCode::from(64);
            }
            _ => break,
        }
    }
    let Some(program) = raw_args.get(i) else {
        eprintln!("cruft wrap: missing command; usage: cruft wrap -- <program> [args...]");
        return ExitCode::from(64);
    };
    let child_args = &raw_args[i + 1..];
    let resolved = if program == "node" {
        std::env::var("CRUFT_NODE").unwrap_or_else(|_| program.clone())
    } else {
        program.clone()
    };
    if let Some(sandbox) = sandbox {
        if !sandbox.supported() {
            eprintln!(
                "cruft wrap: sandbox profile {:?} is not supported on this platform: {}",
                sandbox.name(),
                sandbox.unsupported_reason()
            );
            return ExitCode::from(78);
        }
    }
    if explain || audit || policy.is_some() || sandbox.is_some() {
        eprintln!("cruft wrap: backend=node command={program:?}");
        if let Some(p) = &policy {
            emit_wrap_policy_report(p, audit, sandbox);
        } else if sandbox.is_some() {
            emit_wrap_policy_report("ci", audit, sandbox);
        }
        if let Some(sandbox) = sandbox {
            eprintln!(
                "cruft wrap: sandbox-profile={} platform={} controls={}",
                sandbox.name(),
                std::env::consts::OS,
                wrap_sandbox_control_label(sandbox)
            );
            if let Some(limitation) = wrap_sandbox_limitation_label(sandbox) {
                eprintln!("cruft wrap: sandbox-limitation={}", limitation);
            }
        }
        if audit {
            eprintln!(
                "cruft wrap: audit cwd={} argv_count={} env={} timeout_ms={}",
                std::env::current_dir()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|_| ".".to_string()),
                child_args.len() + 1,
                wrap_sandbox_env_audit_label(sandbox),
                timeout_ms
                    .map(|ms| ms.to_string())
                    .unwrap_or_else(|| "none".to_string())
            );
        }
        eprintln!(
            "cruft wrap: note Node child processes are not constrained by Cruft runtime caps"
        );
    }
    let mut command;
    let sandbox_profile;
    if let Some(sandbox) = sandbox.filter(|s| s.enforces_os_controls()) {
        let sandbox_launch_path = wrap_sandbox_launch_path(&resolved);
        match sandbox {
            WrapSandbox::MacosStrict => {
                sandbox_profile = match write_wrap_sandbox_profile(sandbox, &sandbox_launch_path) {
                    Ok(path) => Some(path),
                    Err(e) => {
                        eprintln!("cruft wrap: {e}");
                        return ExitCode::from(66);
                    }
                };
                command = std::process::Command::new("sandbox-exec");
                command
                    .arg("-f")
                    .arg(sandbox_profile.as_ref().unwrap())
                    .arg(&sandbox_launch_path)
                    .args(child_args);
            }
            WrapSandbox::LinuxStrict => {
                sandbox_profile = None;
                command = std::process::Command::new(&sandbox_launch_path);
                command.args(child_args);
                #[cfg(target_os = "linux")]
                {
                    use std::os::unix::process::CommandExt;
                    let allowed_exec = sandbox_launch_path.clone();
                    unsafe {
                        command.pre_exec(move || {
                            linux_wrap_sandbox::apply_linux_strict(&allowed_exec)
                                .map_err(std::io::Error::other)
                        });
                    }
                }
                #[cfg(not(target_os = "linux"))]
                {
                    eprintln!(
                        "cruft wrap: sandbox profile {:?} is not supported on this platform",
                        sandbox.name()
                    );
                    return ExitCode::from(78);
                }
            }
            WrapSandbox::WindowsStrict => {
                sandbox_profile = None;
                command = std::process::Command::new(&sandbox_launch_path);
                command.args(child_args);
            }
        }
    } else {
        sandbox_profile = None;
        command = std::process::Command::new(&resolved);
        command.args(child_args);
    }
    configure_wrap_command_env(&mut command, sandbox);
    let status = match command
        .stdin(std::process::Stdio::inherit())
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .spawn()
    {
        Ok(mut child) => {
            let windows_job = match attach_windows_job_object(&child, sandbox) {
                Ok(job) => job,
                Err(e) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    eprintln!("cruft wrap: {e}");
                    append_frontdoor_audit_log(
                        audit_log.as_deref(),
                        "cruft wrap",
                        std::slice::from_ref(program),
                        "node",
                        policy.as_deref(),
                        None,
                        &[],
                        Some(66),
                        false,
                        false,
                    );
                    return ExitCode::from(66);
                }
            };
            #[cfg(windows)]
            if windows_job.is_some() {
                eprintln!("cruft wrap: windows-process-object=job-object control=kill-on-close");
            }
            let status = if let Some(timeout_ms) = timeout_ms {
                let deadline =
                    std::time::Instant::now() + std::time::Duration::from_millis(timeout_ms);
                loop {
                    match child.try_wait() {
                        Ok(Some(status)) => break Ok(status),
                        Ok(None) => {
                            if std::time::Instant::now() >= deadline {
                                eprintln!(
                                    "cruft wrap: timeout-ms={timeout_ms} exceeded; terminating child tree"
                                );
                                let _ = child.kill();
                                drop(windows_job);
                                let _ = child.wait();
                                let mut audit_argv = vec![program.clone()];
                                audit_argv.extend_from_slice(child_args);
                                append_frontdoor_audit_log(
                                    audit_log.as_deref(),
                                    "cruft wrap",
                                    &audit_argv,
                                    "node",
                                    policy.as_deref(),
                                    None,
                                    &[],
                                    Some(124),
                                    false,
                                    sandbox.map(|s| s.enforces_os_controls()).unwrap_or(false),
                                );
                                return ExitCode::from(124);
                            }
                            std::thread::sleep(std::time::Duration::from_millis(10));
                        }
                        Err(e) => break Err(e),
                    }
                }
            } else {
                child.wait()
            };
            drop(windows_job);
            status
        }
        Err(e) => Err(e),
    };
    let _ = sandbox_profile
        .as_ref()
        .and_then(|p| p.parent())
        .map(std::fs::remove_dir_all);
    match status {
        Ok(status) => {
            let code = status.code().map(|code| (code & 0xff) as u8).unwrap_or(1);
            let mut audit_argv = vec![program.clone()];
            audit_argv.extend_from_slice(child_args);
            append_frontdoor_audit_log(
                audit_log.as_deref(),
                "cruft wrap",
                &audit_argv,
                "node",
                policy.as_deref(),
                None,
                &[],
                Some(code),
                false,
                sandbox.map(|s| s.enforces_os_controls()).unwrap_or(false),
            );
            ExitCode::from(code)
        }
        Err(e) => {
            eprintln!("cruft wrap: cannot execute {program:?}: {e}");
            append_frontdoor_audit_log(
                audit_log.as_deref(),
                "cruft wrap",
                std::slice::from_ref(program),
                "node",
                policy.as_deref(),
                None,
                &[],
                Some(66),
                false,
                sandbox.map(|s| s.enforces_os_controls()).unwrap_or(false),
            );
            ExitCode::from(66)
        }
    }
}

fn print_wrap_help() {
    println!("Usage: cruft wrap [OPTIONS] -- <program> [args...]");
    println!("       cruft wrap status");
    println!("       cruft wrap install [--dry-run]");
    println!();
    println!("Supervise a Node-family command from Cruft's front-door boundary.");
    println!("Node child processes are not constrained by Cruft runtime caps unless an");
    println!("explicit supported OS sandbox profile is selected.");
    println!();
    println!("Options:");
    println!("  --explain                 Explain backend, policy, and sandbox choices");
    println!("  --audit                   Emit wrapper audit lines");
    println!("  --audit-log <path>        Append audit JSONL to <path>");
    println!("  --policy <audit|ci>       Select advisory front-door policy profile");
    println!(
        "  --sandbox <{}>  Select an explicit OS sandbox profile",
        WrapSandbox::known_profiles()
    );
    println!("  --timeout-ms <ms>         Terminate the supervised child after <ms>");
    println!("  -h, --help                Print this help");
    println!();
    println!("Examples:");
    println!("  cruft wrap -- node app.js");
    println!("  cruft wrap -- npm test");
    println!("  cruft wrap --explain --audit --policy=ci -- node app.js");
}

fn emit_wrap_policy_report(profile: &str, audit_requested: bool, sandbox: Option<WrapSandbox>) {
    let profile = normalize_frontdoor_policy_profile(profile);
    eprintln!("cruft wrap: policy-profile={profile:?}");
    eprintln!("cruft wrap: policy vocabulary=audit|advisory|enforced|not-available");
    for control in frontdoor_policy_controls_for_wrap(profile, audit_requested, sandbox) {
        eprintln!(
            "cruft wrap: policy control={} level={} reason={} next_action={} owner={}",
            control.control,
            control.level,
            control.reason,
            frontdoor_policy_control_next_action(control.control, control.level),
            frontdoor_policy_control_owner(control.control)
        );
    }
}

const WRAP_MANIFEST_REL: &str = ".cruft/node-wrapper/manifest.json";
const WRAP_SHIM_REL: &str = ".cruft/bin";
const WRAP_MARKER_BEGIN: &str = "BEGIN CRUFT MANAGED NODE WRAPPER";
const WRAP_MARKER_END: &str = "END CRUFT MANAGED NODE WRAPPER";

#[derive(Clone, Debug)]
struct NodeWrapManifest {
    shim_dir: Option<String>,
    profile_blocks: Vec<String>,
    managed_commands: Vec<String>,
    cruft_binary: Option<String>,
}

struct DoctorCommandResolution {
    command: String,
    resolved: Option<String>,
    through_cruft: bool,
}

struct DoctorWrapperReport {
    state: String,
    home: Option<String>,
    manifest: Option<String>,
    shim_dir: Option<String>,
    cruft_binary: Option<String>,
    managed_commands: Vec<String>,
    command_resolution: Vec<DoctorCommandResolution>,
}

fn doctor_wrapper_report() -> Result<DoctorWrapperReport, String> {
    let home = node_wrap_home_dir()?;
    let manifest_path = node_wrap_manifest_path(&home);
    let manifest = read_node_wrap_manifest(&manifest_path)?;
    let profile_markers = profile_marker_evidence(&home, &manifest);
    if manifest.is_none() && profile_markers.is_empty() {
        return Ok(DoctorWrapperReport {
            state: "inactive".to_string(),
            home: Some(home.display().to_string()),
            manifest: Some(manifest_path.display().to_string()),
            shim_dir: None,
            cruft_binary: None,
            managed_commands: Vec::new(),
            command_resolution: Vec::new(),
        });
    }
    let Some(manifest) = manifest else {
        return Ok(DoctorWrapperReport {
            state: "partial".to_string(),
            home: Some(home.display().to_string()),
            manifest: Some(manifest_path.display().to_string()),
            shim_dir: None,
            cruft_binary: None,
            managed_commands: Vec::new(),
            command_resolution: Vec::new(),
        });
    };
    let mut partial = false;
    if let Some(shim_dir) = &manifest.shim_dir {
        partial |= !std::path::Path::new(shim_dir).is_dir();
    } else {
        partial = true;
    }
    if manifest.managed_commands.is_empty() {
        partial = true;
    }
    let current_path = current_path_entries();
    let mut all_commands_resolve_to_shim = !manifest.managed_commands.is_empty();
    let mut command_resolution = Vec::new();
    for cmd in &manifest.managed_commands {
        let resolved = find_command_in_path(cmd, &current_path);
        let expected = manifest
            .shim_dir
            .as_ref()
            .map(|dir| std::path::Path::new(dir).join(cmd));
        let through_cruft = match (&resolved, &expected) {
            (Some(found), Some(expected)) => paths_equal(found, expected),
            _ => false,
        };
        all_commands_resolve_to_shim &= through_cruft;
        command_resolution.push(DoctorCommandResolution {
            command: cmd.clone(),
            resolved: resolved.map(|p| p.display().to_string()),
            through_cruft,
        });
    }
    for p in &manifest.profile_blocks {
        partial |= !profile_file_has_wrap_marker(std::path::Path::new(p));
    }
    let state = if partial {
        "partial"
    } else if all_commands_resolve_to_shim {
        "active-current-shell"
    } else {
        "active-next-shell"
    };
    Ok(DoctorWrapperReport {
        state: state.to_string(),
        home: Some(home.display().to_string()),
        manifest: Some(manifest_path.display().to_string()),
        shim_dir: manifest.shim_dir,
        cruft_binary: manifest.cruft_binary,
        managed_commands: manifest.managed_commands,
        command_resolution,
    })
}

fn run_wrap_status() -> ExitCode {
    println!("Cruft Node wrapper status");
    println!();
    let home = match node_wrap_home_dir() {
        Ok(home) => home,
        Err(e) => {
            eprintln!("cruft wrap status: {e}");
            return ExitCode::from(1);
        }
    };
    let manifest_path = node_wrap_manifest_path(&home);
    println!("Home: {}", home.display());
    println!("Manifest: {}", manifest_path.display());
    let manifest = match read_node_wrap_manifest(&manifest_path) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("cruft wrap status: {e}");
            return ExitCode::from(1);
        }
    };
    let profile_markers = profile_marker_evidence(&home, &manifest);
    if manifest.is_none() && profile_markers.is_empty() {
        println!("State: inactive");
        println!("Evidence: no Cruft wrapper manifest or marker-owned profile block found.");
        return ExitCode::SUCCESS;
    }
    let Some(manifest) = manifest else {
        println!("State: partial");
        println!("Evidence: marker-owned profile block exists, but manifest is missing.");
        for p in profile_markers {
            println!("  marker: {p}");
        }
        return ExitCode::SUCCESS;
    };
    let mut partial = false;
    if let Some(shim_dir) = &manifest.shim_dir {
        let exists = std::path::Path::new(shim_dir).is_dir();
        println!(
            "Shim dir: {shim_dir} ({})",
            if exists { "present" } else { "missing" }
        );
        partial |= !exists;
    } else {
        println!("Shim dir: (not recorded)");
        partial = true;
    }
    if let Some(bin) = &manifest.cruft_binary {
        println!("Cruft binary: {bin}");
    }
    if manifest.managed_commands.is_empty() {
        println!("Managed commands: (none recorded)");
        partial = true;
    } else {
        println!("Managed commands: {}", manifest.managed_commands.join(", "));
    }
    let current_path = current_path_entries();
    let mut all_commands_resolve_to_shim = true;
    if !manifest.managed_commands.is_empty() {
        println!("Current shell:");
        for cmd in &manifest.managed_commands {
            let resolved = find_command_in_path(cmd, &current_path);
            let expected = manifest
                .shim_dir
                .as_ref()
                .map(|dir| std::path::Path::new(dir).join(cmd));
            let through_cruft = match (&resolved, &expected) {
                (Some(found), Some(expected)) => paths_equal(found, expected),
                _ => false,
            };
            all_commands_resolve_to_shim &= through_cruft;
            println!(
                "  {cmd}: {} ({})",
                resolved
                    .as_ref()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| "not found on PATH".to_string()),
                if through_cruft {
                    "Cruft shim active"
                } else {
                    "not using Cruft shim in this process"
                }
            );
        }
    }
    if manifest.profile_blocks.is_empty() {
        println!("Profile blocks: (none recorded)");
    } else {
        println!("Profile blocks:");
        for p in &manifest.profile_blocks {
            let has_marker = profile_file_has_wrap_marker(std::path::Path::new(p));
            println!(
                "  {p} ({})",
                if has_marker {
                    "marker present"
                } else {
                    "marker missing"
                }
            );
            partial |= !has_marker;
        }
    }
    let state = if partial {
        "partial"
    } else if all_commands_resolve_to_shim {
        "active-current-shell"
    } else {
        "active-next-shell"
    };
    println!("State: {state}");
    if state == "active-next-shell" {
        if let Some(shim_dir) = &manifest.shim_dir {
            println!(
                "Next step: open a fresh shell or run: export PATH={}:$PATH",
                shell_single_quote(shim_dir)
            );
        }
        println!("Reason: marker-owned wrapper state exists, but this process PATH resolves at least one managed command outside Cruft.");
        println!("Tip: check Homebrew, nvm, fnm, asdf, or Volta PATH ordering if this persists.");
    }
    ExitCode::SUCCESS
}

fn run_wrap_install_plan(args: &[String]) -> ExitCode {
    let dry_run = args.iter().any(|a| a == "--dry-run");
    if args.iter().any(|a| a != "--dry-run") {
        eprintln!("cruft wrap install: usage: cruft wrap install [--dry-run]");
        return ExitCode::from(64);
    }
    println!("Cruft Node wrapper and CPX shim install");
    println!();
    if dry_run {
        println!("Mode: dry-run; no files are modified.");
    } else {
        println!("Mode: install; writing only Cruft-managed user wrapper state.");
    }
    let home = match node_wrap_home_dir() {
        Ok(home) => home,
        Err(e) => {
            eprintln!("cruft wrap install: {e}");
            return ExitCode::from(1);
        }
    };
    let shim_dir = home.join(WRAP_SHIM_REL);
    let manifest = node_wrap_manifest_path(&home);
    let profiles = node_wrap_profile_paths(&home);
    let cruft_binary = current_cruft_binary();
    let commands = ["node", "npm", "npx", "cpx"];
    let path_entries = current_path_entries();
    let prior = prior_command_paths(&["node", "npm", "npx"], &shim_dir, &path_entries);
    if dry_run {
        println!("Would create shim dir: {}", shim_dir.display());
        println!("Would write manifest: {}", manifest.display());
        println!("Would manage commands: {}", commands.join(", "));
        println!(
            "Would add marker-owned PATH block to: {}",
            profiles
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        );
        return ExitCode::SUCCESS;
    }
    if let Err(e) = install_node_wrap_state(
        &home,
        &shim_dir,
        &manifest,
        &profiles,
        &cruft_binary,
        &commands,
        &prior,
    ) {
        eprintln!("cruft wrap install: {e}");
        return ExitCode::from(1);
    }
    println!("Installed shim dir: {}", shim_dir.display());
    println!("Wrote manifest: {}", manifest.display());
    println!(
        "Updated marker-owned profile block: {}",
        profiles
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join(", ")
    );
    println!("Managed commands: {}", commands.join(", "));
    ExitCode::SUCCESS
}

fn run_unwrap_subcommand(raw_args: &[String]) -> ExitCode {
    if raw_args.iter().skip(2).any(|arg| is_help_arg(arg.as_str())) {
        println!("Usage: cruft unwrap [--dry-run] [--all]");
        println!();
        println!("Remove only Cruft-managed Node wrapper and CPX shim state.");
        println!();
        println!("Options:");
        println!("  --dry-run            Print the unwrap plan without modifying files");
        println!("  --all                Include all Cruft-managed wrapper state");
        println!("  -h, --help           Print this help");
        return ExitCode::SUCCESS;
    }
    let dry_run = raw_args.iter().skip(2).any(|a| a == "--dry-run");
    let all = raw_args.iter().skip(2).any(|a| a == "--all");
    if raw_args
        .iter()
        .skip(2)
        .any(|a| a != "--dry-run" && a != "--all")
    {
        eprintln!("cruft unwrap: usage: cruft unwrap [--dry-run] [--all]");
        return ExitCode::from(64);
    }
    println!("Cruft Node wrapper and CPX shim unwrap");
    println!();
    if dry_run {
        println!("Mode: dry-run; no files are modified.");
    } else {
        println!("Mode: unwrap; removing only Cruft-managed wrapper state.");
    }
    if all {
        println!("Scope: all Cruft-managed Node wrapper and CPX shim state.");
    }
    let home = match node_wrap_home_dir() {
        Ok(home) => home,
        Err(e) => {
            eprintln!("cruft unwrap: {e}");
            return ExitCode::from(1);
        }
    };
    let manifest_path = node_wrap_manifest_path(&home);
    let manifest = match read_node_wrap_manifest(&manifest_path) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("cruft unwrap: {e}");
            return ExitCode::from(1);
        }
    };
    let profile_markers = profile_marker_evidence(&home, &manifest);
    if manifest.is_none() && profile_markers.is_empty() {
        println!("State: already unwrapped");
        println!("No Cruft-managed manifest or profile marker found.");
        return ExitCode::SUCCESS;
    }
    if dry_run {
        println!("Would inspect manifest: {}", manifest_path.display());
        if let Some(manifest) = manifest {
            if let Some(shim_dir) = manifest.shim_dir {
                println!("Would remove Cruft-owned shim dir: {shim_dir}");
            }
            for p in manifest.profile_blocks {
                println!("Would remove marker-owned profile block from: {p}");
            }
            if !manifest.managed_commands.is_empty() {
                println!(
                    "Would stop managing commands: {}",
                    manifest.managed_commands.join(", ")
                );
            }
        }
        for p in profile_markers {
            println!("Would remove discovered marker-owned profile block from: {p}");
        }
        println!("Result: dry-run only; no files modified.");
        return ExitCode::SUCCESS;
    }
    if let Err(e) = remove_node_wrap_state(&manifest_path, manifest, &profile_markers) {
        eprintln!("cruft unwrap: {e}");
        return ExitCode::from(1);
    }
    println!("Removed Cruft-managed Node wrapper state.");
    ExitCode::SUCCESS
}

fn current_cruft_binary() -> String {
    std::env::current_exe()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "cruft".to_string())
}

fn current_path_entries() -> Vec<std::path::PathBuf> {
    std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).collect())
        .unwrap_or_default()
}

fn prior_command_paths(
    commands: &[&str],
    shim_dir: &std::path::Path,
    path_entries: &[std::path::PathBuf],
) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for cmd in commands {
        let found = find_command_excluding(cmd, shim_dir, path_entries)
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| (*cmd).to_string());
        out.push(((*cmd).to_string(), found));
    }
    out
}

fn find_command_excluding(
    cmd: &str,
    excluded_dir: &std::path::Path,
    path_entries: &[std::path::PathBuf],
) -> Option<std::path::PathBuf> {
    for dir in path_entries {
        if paths_equal(dir, excluded_dir) {
            continue;
        }
        let candidate = dir.join(cmd);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

fn find_command_in_path(
    cmd: &str,
    path_entries: &[std::path::PathBuf],
) -> Option<std::path::PathBuf> {
    for dir in path_entries {
        let candidate = dir.join(cmd);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

fn paths_equal(a: &std::path::Path, b: &std::path::Path) -> bool {
    let ca = std::fs::canonicalize(a).unwrap_or_else(|_| a.to_path_buf());
    let cb = std::fs::canonicalize(b).unwrap_or_else(|_| b.to_path_buf());
    ca == cb
}

fn install_node_wrap_state(
    home: &std::path::Path,
    shim_dir: &std::path::Path,
    manifest: &std::path::Path,
    profiles: &[std::path::PathBuf],
    cruft_binary: &str,
    commands: &[&str],
    prior: &[(String, String)],
) -> Result<(), String> {
    std::fs::create_dir_all(shim_dir)
        .map_err(|e| format!("cannot create shim dir {}: {e}", shim_dir.display()))?;
    std::fs::create_dir_all(manifest.parent().unwrap_or(home))
        .map_err(|e| format!("cannot create manifest dir {}: {e}", manifest.display()))?;
    for cmd in commands {
        let path = shim_dir.join(wrap_shim_file_name(cmd));
        if *cmd == "cpx" {
            write_cpx_wrap_shim(&path, cruft_binary)?;
        } else {
            let prior_path = prior
                .iter()
                .find(|(name, _)| name == cmd)
                .map(|(_, path)| path.as_str())
                .unwrap_or(cmd);
            write_node_wrap_shim(&path, cruft_binary, cmd, prior_path)?;
        }
    }
    for profile in profiles {
        install_profile_marker(profile, shim_dir)?;
    }
    let manifest_src = node_wrap_manifest_json(shim_dir, profiles, cruft_binary, commands, prior);
    std::fs::write(manifest, manifest_src)
        .map_err(|e| format!("cannot write wrapper manifest {}: {e}", manifest.display()))?;
    Ok(())
}

fn wrap_shim_file_name(command: &str) -> String {
    if cfg!(windows) {
        format!("{command}.cmd")
    } else {
        command.to_string()
    }
}

fn write_node_wrap_shim(
    path: &std::path::Path,
    cruft_binary: &str,
    command: &str,
    prior_path: &str,
) -> Result<(), String> {
    let upper = command.to_ascii_uppercase();
    #[cfg(windows)]
    let src = format!(
        "@echo off\r\nrem {WRAP_MARKER_BEGIN}\r\nset \"CRUFT_WRAPPED_{upper}={prior}\"\r\n\"{cruft}\" wrap -- \"{prior}\" %*\r\nrem {WRAP_MARKER_END}\r\n",
        prior = prior_path,
        cruft = cruft_binary,
    );
    #[cfg(not(windows))]
    let src = format!(
        "#!/bin/sh\n# {WRAP_MARKER_BEGIN}\nCRUFT_WRAPPED_{upper}={prior}\nexport CRUFT_WRAPPED_{upper}\nexec {cruft} wrap -- {prior} \"$@\"\n# {WRAP_MARKER_END}\n",
        prior = shell_single_quote(prior_path),
        cruft = shell_single_quote(cruft_binary)
    );
    std::fs::write(path, src).map_err(|e| format!("cannot write shim {}: {e}", path.display()))?;
    make_executable(path)?;
    Ok(())
}

fn write_cpx_wrap_shim(path: &std::path::Path, cruft_binary: &str) -> Result<(), String> {
    #[cfg(windows)]
    let src = format!(
        "@echo off\r\nrem {WRAP_MARKER_BEGIN}\r\n\"{cruft}\" exec %*\r\nrem {WRAP_MARKER_END}\r\n",
        cruft = cruft_binary,
    );
    #[cfg(not(windows))]
    let src = format!(
        "#!/bin/sh\n# {WRAP_MARKER_BEGIN}\nexec {cruft} exec \"$@\"\n# {WRAP_MARKER_END}\n",
        cruft = shell_single_quote(cruft_binary)
    );
    std::fs::write(path, src).map_err(|e| format!("cannot write shim {}: {e}", path.display()))?;
    make_executable(path)?;
    Ok(())
}

#[cfg(unix)]
fn make_executable(path: &std::path::Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = std::fs::metadata(path)
        .map_err(|e| format!("cannot stat shim {}: {e}", path.display()))?
        .permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(path, perms)
        .map_err(|e| format!("cannot chmod shim {}: {e}", path.display()))
}

#[cfg(not(unix))]
fn make_executable(_path: &std::path::Path) -> Result<(), String> {
    Ok(())
}

fn install_profile_marker(
    profile: &std::path::Path,
    shim_dir: &std::path::Path,
) -> Result<(), String> {
    if let Some(parent) = profile.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("cannot create profile dir {}: {e}", parent.display()))?;
    }
    let existing = std::fs::read_to_string(profile).unwrap_or_default();
    let without_old = remove_marker_block(&existing);

    #[cfg(windows)]
    let block = format!(
        "# {WRAP_MARKER_BEGIN}\n$env:PATH = '{}' + ';' + $env:PATH\n# {WRAP_MARKER_END}\n",
        shim_dir.display().to_string().replace('\'', "''")
    );
    #[cfg(not(windows))]
    let block = format!(
        "# {WRAP_MARKER_BEGIN}\nexport PATH={}:$PATH\n# {WRAP_MARKER_END}\n",
        shell_single_quote(&shim_dir.display().to_string())
    );
    let next = if without_old.trim().is_empty() {
        block
    } else {
        format!("{}\n{}", without_old.trim_end(), block)
    };
    std::fs::write(profile, next)
        .map_err(|e| format!("cannot write profile {}: {e}", profile.display()))
}

fn node_wrap_profile_paths(home: &std::path::Path) -> Vec<std::path::PathBuf> {
    if let Some(path) = std::env::var_os("CRUFT_WRAP_PROFILE") {
        return vec![std::path::PathBuf::from(path)];
    }
    #[cfg(windows)]
    {

        let docs = home.join("Documents");
        return vec![
            docs.join("PowerShell")
                .join("Microsoft.PowerShell_profile.ps1"),
            docs.join("WindowsPowerShell")
                .join("Microsoft.PowerShell_profile.ps1"),
        ];
    }
    #[cfg(not(windows))]
    {
        node_wrap_profile_paths_posix(home)
    }
}

#[cfg(not(windows))]
fn node_wrap_profile_paths_posix(home: &std::path::Path) -> Vec<std::path::PathBuf> {
    let shell_profile = std::env::var_os("SHELL")
        .and_then(|shell| {
            let shell = std::path::Path::new(&shell);
            shell
                .file_name()
                .and_then(|name| name.to_str())
                .map(|name| {
                    if name.contains("bash") {
                        ".bashrc"
                    } else if name.contains("zsh") {
                        ".zshrc"
                    } else {
                        ".profile"
                    }
                })
        })
        .unwrap_or(".zshrc");
    let mut profiles = vec![home.join(shell_profile)];
    for name in [".zshrc", ".bashrc", ".bash_profile", ".profile"] {
        let path = home.join(name);
        if path.exists() && !profiles.iter().any(|p| paths_equal(p, &path)) {
            profiles.push(path);
        }
    }
    profiles
}

fn node_wrap_manifest_json(
    shim_dir: &std::path::Path,
    profiles: &[std::path::PathBuf],
    cruft_binary: &str,
    commands: &[&str],
    prior: &[(String, String)],
) -> String {
    let commands_json = commands
        .iter()
        .map(|c| format!("\"{}\"", json_escape(c)))
        .collect::<Vec<_>>()
        .join(", ");
    let prior_json = prior
        .iter()
        .map(|(cmd, path)| format!("\"{}\":\"{}\"", json_escape(cmd), json_escape(path)))
        .collect::<Vec<_>>()
        .join(", ");
    let profiles_json = profiles
        .iter()
        .map(|p| format!("\"{}\"", json_escape(&p.display().to_string())))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "{{\n  \"version\": 1,\n  \"created_by\": \"cruft wrap install\",\n  \"shim_dir\": \"{}\",\n  \"profile_blocks\": [{}],\n  \"managed_commands\": [{}],\n  \"cruft_binary\": \"{}\",\n  \"prior_commands\": {{{}}}\n}}\n",
        json_escape(&shim_dir.display().to_string()),
        profiles_json,
        commands_json,
        json_escape(cruft_binary),
        prior_json
    )
}

fn remove_node_wrap_state(
    manifest_path: &std::path::Path,
    manifest: Option<NodeWrapManifest>,
    profile_markers: &[String],
) -> Result<(), String> {
    let mut profiles = profile_markers.to_vec();
    let mut shim_dir = None;
    if let Some(m) = manifest {
        shim_dir = m.shim_dir;
        profiles.extend(m.profile_blocks);
    }
    profiles.sort();
    profiles.dedup();
    for p in profiles {
        remove_profile_marker(std::path::Path::new(&p))?;
    }
    if let Some(shim_dir) = shim_dir {
        remove_cruft_shims(std::path::Path::new(&shim_dir))?;
    }
    if manifest_path.exists() {
        std::fs::remove_file(manifest_path).map_err(|e| {
            format!(
                "cannot remove wrapper manifest {}: {e}",
                manifest_path.display()
            )
        })?;
    }
    Ok(())
}

fn remove_profile_marker(profile: &std::path::Path) -> Result<(), String> {
    let Ok(existing) = std::fs::read_to_string(profile) else {
        return Ok(());
    };
    if !existing.contains(WRAP_MARKER_BEGIN) || !existing.contains(WRAP_MARKER_END) {
        return Ok(());
    }
    let next = remove_marker_block(&existing);
    std::fs::write(profile, next)
        .map_err(|e| format!("cannot write profile {}: {e}", profile.display()))
}

fn remove_marker_block(src: &str) -> String {
    let mut out = String::new();
    let mut in_block = false;
    for line in src.lines() {
        if line.contains(WRAP_MARKER_BEGIN) {
            in_block = true;
            continue;
        }
        if line.contains(WRAP_MARKER_END) {
            in_block = false;
            continue;
        }
        if !in_block {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

fn remove_cruft_shims(shim_dir: &std::path::Path) -> Result<(), String> {
    for cmd in ["node", "npm", "npx", "cpx"] {

        let path = shim_dir.join(wrap_shim_file_name(cmd));
        if !path.exists() {
            continue;
        }
        if !profile_file_has_wrap_marker(&path) {
            continue;
        }
        std::fs::remove_file(&path)
            .map_err(|e| format!("cannot remove Cruft shim {}: {e}", path.display()))?;
    }
    if shim_dir.is_dir()
        && std::fs::read_dir(shim_dir)
            .map(|mut i| i.next().is_none())
            .unwrap_or(false)
    {
        std::fs::remove_dir(shim_dir)
            .map_err(|e| format!("cannot remove empty shim dir {}: {e}", shim_dir.display()))?;
    }
    Ok(())
}

fn shell_single_quote(s: &str) -> String {
    let mut out = String::from("'");
    for ch in s.chars() {
        if ch == '\'' {
            out.push_str("'\"'\"'");
        } else {
            out.push(ch);
        }
    }
    out.push('\'');
    out
}

fn node_wrap_home_dir() -> Result<std::path::PathBuf, String> {

    if let Some(home) = std::env::var_os("CRUFT_WRAP_HOME") {
        if !home.is_empty() {
            return Ok(std::path::PathBuf::from(home));
        }
    }
    if let Some(home) = cruft::platform::user_home_dir() {
        return Ok(home);
    }
    Err(
        "no user home directory found (set HOME, USERPROFILE, or CRUFT_WRAP_HOME); \
         cannot locate user-scoped wrapper state"
            .to_string(),
    )
}

fn node_wrap_manifest_path(home: &std::path::Path) -> std::path::PathBuf {
    home.join(WRAP_MANIFEST_REL)
}

fn read_node_wrap_manifest(path: &std::path::Path) -> Result<Option<NodeWrapManifest>, String> {
    if !path.exists() {
        return Ok(None);
    }
    let src = std::fs::read_to_string(path)
        .map_err(|e| format!("cannot read wrapper manifest {}: {e}", path.display()))?;
    let json = serde_json::from_str::<serde_json::Value>(&src)
        .map_err(|e| format!("cannot parse wrapper manifest {}: {e}", path.display()))?;
    let shim_dir = json
        .get("shim_dir")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let cruft_binary = json
        .get("cruft_binary")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let profile_blocks = json_array_strings(&json, "profile_blocks");
    let managed_commands = json_array_strings(&json, "managed_commands");
    Ok(Some(NodeWrapManifest {
        shim_dir,
        profile_blocks,
        managed_commands,
        cruft_binary,
    }))
}

fn json_array_strings(json: &serde_json::Value, key: &str) -> Vec<String> {
    json.get(key)
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default()
}

fn profile_marker_evidence(
    home: &std::path::Path,
    manifest: &Option<NodeWrapManifest>,
) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(m) = manifest {
        for p in &m.profile_blocks {
            if profile_file_has_wrap_marker(std::path::Path::new(p)) {
                out.push(p.clone());
            }
        }
    }
    for name in [".zshrc", ".bashrc", ".bash_profile", ".profile"] {
        let p = home.join(name);
        if profile_file_has_wrap_marker(&p) {
            out.push(p.display().to_string());
        }
    }
    out.sort();
    out.dedup();
    out
}

fn profile_file_has_wrap_marker(path: &std::path::Path) -> bool {
    let Ok(src) = std::fs::read_to_string(path) else {
        return false;
    };
    src.contains(WRAP_MARKER_BEGIN) && src.contains(WRAP_MARKER_END)
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct SecurityDoctorPackage {
    name: String,
    version: String,
}

#[derive(Clone, Debug)]
struct SecurityDoctorFinding {
    package: SecurityDoctorPackage,
    id: String,
    summary: String,
    malicious: bool,
}

fn run_doctor_subcommand(args: &[String]) -> ExitCode {
    if args.first().map(|s| s.as_str()) == Some("--security") {
        return run_security_doctor();
    }
    if args.first().map(|s| s.as_str()) == Some("--json") {
        return run_doctor_json();
    }
    println!("Cruft doctor");
    println!();
    println!("Recommended next step:");
    println!("  cruft wrap install");
    println!();
    println!("This makes node/npm/npx enter Cruft's wrapper boundary.");
    println!("It does not sandbox Node child processes by default.");
    println!("Undo any time:");
    println!("  cruft unwrap --all");
    println!();
    println!("Execution modes:");
    println!("  cruft <file.js>              run inside the Cruft runtime");
    println!("  cruft run <script-or-file>   run a package script or file in the Cruft runtime");
    println!("  cruft run --backend=auto --explain app.js");
    println!("                                choose the safest backend and explain why");
    println!("  cruft run --backend=node app.js");
    println!("                                force real Node for maximum compatibility");
    println!("  cruft run --backend=cruft app.js");
    println!("                                force the Cruft runtime for proven mouths");
    println!("  cruft --diagnostics=structural <file.js>");
    println!("                                opt in to structural runtime diagnostics");
    println!("  cruft --diagnostic-log ./diagnostics.jsonl <file.js>");
    println!("                                write local structural diagnostic artifacts");
    println!("  cruft wrap -- node app.js    supervise Node as an explicit backend");
    println!("  cruft wrap -- npm test       supervise an existing npm workflow");
    println!();
    println!("Diagnostic disclosure:");
    println!("  - default: public diagnostics redact structural runtime internals");
    println!("  - structural: use --diagnostics=structural or CRUFT_DIAGNOSTICS=structural");
    println!("  - artifact: use --diagnostic-log or CRUFT_DIAGNOSTIC_LOG for local JSONL");
    println!("  - public mode is a disclosure boundary, not a compatibility or sandbox claim");
    println!();
    println!("Node-wrapper status:");
    println!("  - transparent by default: cwd/env/argv/stdin/stdout/stderr/exit code pass through");
    println!("  - opt-in transcript: use --explain, --audit, or --policy before --");
    println!("  - policy is advisory today; Node child processes are not constrained by Cruft runtime caps");
    println!("  - policy output uses levels: audit, advisory, enforced, not-available");
    println!("  - install blocking is not enforced unless `cruft trust install --enforce` is used");
    if WrapSandbox::MacosStrict.supported() {
        println!("  - explicit macOS sandbox: cruft wrap --sandbox=macos-strict -- node app.js");
        println!("    controls: filesystem-write, network, environment, external-process-exec");
        println!("    limitation: same-executable child exec remains allowed; complete child-process denial remains unproven");
    } else if WrapSandbox::LinuxStrict.supported() {
        println!("  - explicit Linux sandbox: cruft wrap --sandbox=linux-strict -- node app.js");
        println!("    controls: filesystem-write, environment, external-process-exec");
        println!("    limitation: network and complete child-process denial remain unproven");
    } else if WrapSandbox::WindowsStrict.supported() {
        println!("  - experimental Windows sandbox mouth: cruft wrap --sandbox=windows-strict -- node app.js");
        println!("    controls: process-lifetime kill-on-close/timeout, environment scrub");
        println!("    not-yet-controls: filesystem-write, filesystem-read, network, child-process denial");
        println!("    limitation: baseline mouth launches transparently; no filesystem, network, or child-process denial is claimed yet");
    } else {
        println!("  - OS/process sandbox profiles are not available on this platform yet");
    }
    println!("  - sandbox control levels:");
    if WrapSandbox::MacosStrict.supported() {
        println!("    filesystem-write: enforced by macos-strict");
        println!("    network: enforced by macos-strict");
        println!("    environment: enforced by macos-strict");
        println!("    external-process-exec: enforced by macos-strict");
    } else if WrapSandbox::LinuxStrict.supported() {
        println!("    filesystem-write: enforced by linux-strict");
        println!("    network: not-available on this platform");
        println!("    environment: enforced by linux-strict");
        println!("    external-process-exec: enforced by linux-strict");
    } else {
        println!("    filesystem-write: not-available on this platform");
        println!("    network: not-available on this platform");
        if WrapSandbox::WindowsStrict.supported() {
            println!("    environment: enforced by windows-strict env scrub");
        } else {
            println!("    environment: not-available on this platform");
        }
        println!("    external-process-exec: not-available on this platform");
    }
    println!("    complete-child-process-denial: not-available");
    println!("    linux-windows-sandbox: not-available");
    println!();
    println!("Try:");
    println!("  cruft wrap -- node -e 'console.log(\"ok\")'");
    println!("  cruft wrap -- npm run dev");
    println!("  cruft wrap status");
    println!("  cruft wrap --explain --audit --policy=locked -- node app.js");
    println!();
    println!("Current policy boundary:");
    println!("  - subprocess supervision: available");
    println!("  - advisory dependency scan: cruft doctor --security");
    println!("  - exact evidence-based Cruft auto-selection: packaged evidence index");
    println!("  - explicit install blocking mouth: cruft trust install --enforce -- npm install");
    if WrapSandbox::MacosStrict.supported() {
        println!("  - enforced OS/process sandbox profile: macos-strict (filesystem-write, network, environment, external-process-exec)");
    } else if WrapSandbox::LinuxStrict.supported() {
        println!("  - enforced OS/process sandbox profile: linux-strict (filesystem-write, environment, external-process-exec)");
    } else if WrapSandbox::WindowsStrict.supported() {
        println!("  - experimental OS/process sandbox profile: windows-strict (process-lifetime kill-on-close/timeout and environment scrub)");
    } else {
        println!("  - enforced OS/process sandbox profiles: not available on this platform");
    }
    ExitCode::SUCCESS
}

fn run_doctor_json() -> ExitCode {
    if let Some(arg) = std::env::args().skip(2).find(|a| a != "--json") {
        eprintln!("cruft doctor --json: unexpected argument {arg}");
        return ExitCode::from(64);
    }
    let wrapper = match doctor_wrapper_report() {
        Ok(report) => report,
        Err(e) => {
            eprintln!("cruft doctor --json: {e}");
            return ExitCode::from(1);
        }
    };
    print_doctor_json(&wrapper);
    ExitCode::SUCCESS
}

fn json_optional_string(value: &Option<String>) -> String {
    match value {
        Some(value) => format!("\"{}\"", json_escape(value)),
        None => "null".to_string(),
    }
}

fn print_json_string_array(values: &[String]) {
    print!("[");
    for (idx, value) in values.iter().enumerate() {
        if idx > 0 {
            print!(",");
        }
        print!("\"{}\"", json_escape(value));
    }
    print!("]");
}

fn print_trust_risks_json(risks: &[TrustRisk]) {
    print_trust_risks_json_with_mode(risks, false);
}

fn print_trust_risks_json_with_mode(risks: &[TrustRisk], install_enforced: bool) {
    let mut out = String::new();
    push_trust_risks_json(&mut out, risks, install_enforced);
    print!("{out}");
}

fn push_trust_risks_json(out: &mut String, risks: &[TrustRisk], install_enforced: bool) {
    fn json_evidence(out: &mut String, risk: &TrustRisk) {
        out.push_str("{\"source\":\"");
        out.push_str(&json_escape(risk.evidence_source()));
        out.push('"');
        if let Some(id) = risk.evidence_id() {
            out.push_str(",\"id\":\"");
            out.push_str(&json_escape(id));
            out.push('"');
        }
        out.push_str(",\"summary\":\"");
        out.push_str(&json_escape(risk.summary()));
        out.push_str("\"}");
    }
    out.push('[');
    for (idx, risk) in risks.iter().enumerate() {
        if idx > 0 {
            out.push(',');
        }
        out.push_str("{\"schema_version\":1,\"kind\":\"");
        out.push_str(&json_escape(&risk.kind));
        out.push_str("\",\"level\":\"");
        out.push_str(&json_escape(&risk.level));
        out.push_str("\",\"subject\":\"");
        out.push_str(&json_escape(&risk.subject));
        out.push_str("\",\"reason\":\"");
        out.push_str(&json_escape(&risk.reason));
        out.push_str("\",\"evidence\":");
        json_evidence(out, risk);
        out.push_str(",\"enforced\":");
        out.push_str(if risk.enforced_for_install(install_enforced) {
            "true"
        } else {
            "false"
        });
        out.push_str(",\"action\":\"");
        out.push_str(risk.action_for_install(install_enforced));
        out.push_str("\",\"next_action\":\"");
        out.push_str(&json_escape(risk.next_action_for_install(install_enforced)));
        out.push_str("\",\"owner\":\"");
        out.push_str(risk.owner());
        out.push_str("\"}");
    }
    out.push(']');
}

fn sorted_dedup_trust_risks(mut risks: Vec<TrustRisk>) -> Vec<TrustRisk> {
    risks.sort_by(|a, b| (&a.kind, &a.subject, &a.reason).cmp(&(&b.kind, &b.subject, &b.reason)));
    risks.dedup_by(|a, b| a.kind == b.kind && a.subject == b.subject && a.reason == b.reason);
    risks
}

fn collect_package_exec_manifest_risks(package_dir: &std::path::Path) -> Vec<TrustRisk> {
    let metadata = rusty_js_pm::security_metadata::package_dir_security_metadata(package_dir);
    sorted_dedup_trust_risks(
        metadata
            .risks
            .into_iter()
            .map(|risk| TrustRisk {
                kind: risk.kind,
                level: risk.level,
                subject: risk.subject,
                reason: risk.reason,
            })
            .collect(),
    )
}

fn cpx_registry_policy() -> rusty_js_pm::registry_policy::RegistryPolicySnapshot {
    rusty_js_pm::registry_policy::resolve_registry_policy()
}

fn cpx_risk_disposition(source_kind: &str, risks: &[TrustRisk]) -> &'static str {
    if risks.iter().any(trust_risk_blocks_install) {
        "requires_approval"
    } else if source_kind == "registry-unfetched" {
        "unknown"
    } else {
        "allow"
    }
}

fn cpx_projected_permissions(source_kind: &str, risks: &[TrustRisk]) -> Vec<&'static str> {
    let mut permissions = Vec::new();
    let mut push = |permission: &'static str| {
        if !permissions.contains(&permission) {
            permissions.push(permission);
        }
    };
    if source_kind == "registry-unfetched" {
        push("unknown_effects");
        push("filesystem_unknown");
        push("network_unknown");
        push("process_unknown");
    }
    for risk in risks {
        match risk.kind.as_str() {
            "lifecycle-script" => {
                push("process_spawn");
                push("filesystem_write");
            }
            "native-addon" => {
                push("native_code");
                push("process_spawn");
                push("filesystem_write");
            }
            "git-dependency" | "tarball-dependency" => {
                push("network_access");
            }
            "known-malicious-advisory" | "known-vulnerability" => {
                push("filesystem_unknown");
                push("network_unknown");
                push("process_unknown");
            }
            _ => {}
        }
    }
    permissions
}

fn cpx_registry_fallback_decision(
    policy: &rusty_js_pm::registry_policy::RegistryPolicySnapshot,
    package: Option<&str>,
) -> &'static str {
    match package {
        Some(package)
            if rusty_js_pm::registry_policy::public_fallback_blocks_package(policy, package) =>
        {
            "blocked_unmapped_scope"
        }
        Some(package) if rusty_js_pm::registry_policy::package_scope(package).is_some() => {
            "scoped_mapping_or_default"
        }
        _ => "not_applicable",
    }
}

fn cpx_registry_source_disposition(
    policy: &rusty_js_pm::registry_policy::RegistryPolicySnapshot,
    package: Option<&str>,
    matched_scope: Option<&str>,
) -> &'static str {
    match package {
        Some(package)
            if rusty_js_pm::registry_policy::public_fallback_blocks_package(policy, package) =>
        {
            "blocked_public_fallback"
        }
        Some(_) if matched_scope.is_some() => "scoped_registry",
        Some(package) if rusty_js_pm::registry_policy::package_scope(package).is_some() => {
            "unmapped_scoped_default_registry"
        }
        Some(_) => "default_registry",
        None => "not_applicable",
    }
}

fn cpx_risk_human_report(
    requested: &str,
    package_specs: &[PackageExecSpec],
    source_kind: &str,
    local_package_dir: Option<&std::path::Path>,
    resolved_executable: Option<&std::path::Path>,
    risks: &[TrustRisk],
) -> String {
    let mut report = String::new();
    let registry_policy = cpx_registry_policy();
    let selected_package = package_specs.first().map(|spec| spec.name.as_str());
    let (selected_registry, matched_scope) = selected_package
        .map(|package| {
            rusty_js_pm::registry_policy::selected_registry_for_package(&registry_policy, package)
        })
        .unwrap_or((&registry_policy.default_registry, None));
    let fallback_decision = cpx_registry_fallback_decision(&registry_policy, selected_package);
    let source_disposition =
        cpx_registry_source_disposition(&registry_policy, selected_package, matched_scope);
    report.push_str("Cruft package exec risk preflight\n");
    report.push_str(&format!("Requested: {requested}\n"));
    report.push_str(&format!("Source: {source_kind}\n"));
    report.push_str(&format!("Registry: {selected_registry}\n"));
    report.push_str(&format!("Registry policy: {}\n", registry_policy.source));
    if let Some(scope) = matched_scope {
        report.push_str(&format!("Registry policy scope: {scope}\n"));
    }
    report.push_str(&format!(
        "Registry public fallback: {}\n",
        registry_policy.public_fallback
    ));
    report.push_str(&format!(
        "Registry fallback decision: {fallback_decision}\n"
    ));
    report.push_str(&format!(
        "Registry source disposition: {source_disposition}\n"
    ));
    report.push_str(&format!(
        "Registry auth mode: {}\n",
        registry_policy.auth_mode
    ));
    if let Some(path) = registry_policy.source_path.as_ref() {
        report.push_str(&format!("Registry policy file: {}\n", path.display()));
    }
    if let Some(path) = local_package_dir {
        report.push_str(&format!("Local package: {}\n", path.display()));
    }
    if let Some(path) = resolved_executable {
        report.push_str(&format!("Executable: {}\n", path.display()));
    }
    if !package_specs.is_empty() {
        let packages = package_specs
            .iter()
            .map(|spec| format!("{}@{}", spec.name, spec.range))
            .collect::<Vec<_>>()
            .join(", ");
        report.push_str(&format!("Packages: {packages}\n"));
    }
    if source_kind == "registry-unfetched" {
        report.push_str(
            "Unknown facts: registry_metadata, tarball_artifact (not fetched before preflight)\n",
        );
    }
    let permissions = cpx_projected_permissions(source_kind, risks);
    let permission_text = if permissions.is_empty() {
        "none_static".to_string()
    } else {
        permissions.join(", ")
    };
    report.push_str(&format!("Required permissions: {permission_text}\n"));
    report.push_str(&format!("Risks: {}\n", risks.len()));
    for risk in risks {
        report.push_str(&format!(
            "  - kind={} level={} subject={} reason={}\n",
            risk.kind, risk.level, risk.subject, risk.reason
        ));
    }
    report.push_str(&format!(
        "Policy disposition: {}\n",
        cpx_risk_disposition(source_kind, risks)
    ));
    report.push_str("Execution: not_run\n");
    report
}

fn print_cpx_risk_json(
    requested: &str,
    package_specs: &[PackageExecSpec],
    local_package_dir: Option<&std::path::Path>,
    resolved_executable: Option<&std::path::Path>,
) {
    let registry_policy = cpx_registry_policy();
    let selected_package = package_specs.first().map(|spec| spec.name.as_str());
    let (registry, matched_scope) = selected_package
        .map(|package| {
            rusty_js_pm::registry_policy::selected_registry_for_package(&registry_policy, package)
        })
        .unwrap_or((&registry_policy.default_registry, None));
    let registry = registry.to_string();
    let matched_scope = matched_scope.map(|scope| scope.to_string());
    let fallback_decision = cpx_registry_fallback_decision(&registry_policy, selected_package);
    let source_disposition = cpx_registry_source_disposition(
        &registry_policy,
        selected_package,
        matched_scope.as_deref(),
    );
    let registry = registry.to_string();
    let risks = local_package_dir
        .map(collect_package_exec_manifest_risks)
        .unwrap_or_default();
    let source_kind = if resolved_executable.is_some() {
        "local"
    } else if package_specs.is_empty() {
        "unknown-local-miss"
    } else {
        "registry-unfetched"
    };
    let disposition = cpx_risk_disposition(source_kind, &risks);
    let projected_permissions = cpx_projected_permissions(source_kind, &risks);
    println!("{{");
    println!("  \"schema\": \"cruft.package_exec_risk.v1\",");
    println!("  \"tool\": \"cpx\",");
    println!("  \"requested\": \"{}\",", json_escape(requested));
    println!("  \"source\": {{");
    println!("    \"kind\": \"{}\",", json_escape(source_kind));
    println!("    \"registry\": \"{}\",", json_escape(&registry));
    println!(
        "    \"local_package_dir\": {},",
        json_optional_string(&local_package_dir.map(|p| p.display().to_string()))
    );
    println!(
        "    \"resolved_executable\": {}",
        json_optional_string(&resolved_executable.map(|p| p.display().to_string()))
    );
    println!("  }},");
    println!("  \"registry_policy\": {{");
    println!(
        "    \"source\": \"{}\",",
        json_escape(&registry_policy.source)
    );
    println!(
        "    \"source_path\": {},",
        json_optional_string(
            &registry_policy
                .source_path
                .as_ref()
                .map(|p| p.display().to_string())
        )
    );
    println!(
        "    \"default_registry\": \"{}\",",
        json_escape(&registry_policy.default_registry)
    );
    println!("    \"selected_registry\": \"{}\",", json_escape(&registry));
    println!(
        "    \"matched_scope\": {},",
        json_optional_string(&matched_scope.map(|scope| scope.to_string()))
    );
    println!("    \"scope_mappings\": [");
    for (idx, mapping) in registry_policy.scopes.iter().enumerate() {
        let comma = if idx + 1 == registry_policy.scopes.len() {
            ""
        } else {
            ","
        };
        println!(
            "      {{\"scope\":\"{}\",\"registry\":\"{}\"}}{}",
            json_escape(&mapping.scope),
            json_escape(&mapping.registry),
            comma
        );
    }
    println!("    ],");
    println!(
        "    \"fallback_policy\": \"{}\",",
        json_escape(&registry_policy.public_fallback)
    );
    println!(
        "    \"fallback_decision\": \"{}\",",
        json_escape(fallback_decision)
    );
    println!(
        "    \"source_disposition\": \"{}\",",
        json_escape(source_disposition)
    );
    println!(
        "    \"auth_mode\": \"{}\"",
        json_escape(&registry_policy.auth_mode)
    );
    println!("  }},");
    println!("  \"packages\": [");
    for (idx, spec) in package_specs.iter().enumerate() {
        let comma = if idx + 1 == package_specs.len() {
            ""
        } else {
            ","
        };
        println!(
            "    {{\"name\":\"{}\",\"range\":\"{}\",\"status\":\"metadata_unfetched\"}}{}",
            json_escape(&spec.name),
            json_escape(&spec.range),
            comma
        );
    }
    println!("  ],");
    println!("  \"unknown_facts\": [");
    if source_kind == "registry-unfetched" {
        println!("    {{\"field\":\"registry_metadata\",\"status\":\"unavailable\",\"reason\":\"risk_json_no_exec_does_not_fetch\"}},");
        println!("    {{\"field\":\"tarball_artifact\",\"status\":\"unavailable\",\"reason\":\"risk_json_no_exec_does_not_fetch\"}}");
    }
    println!("  ],");
    print!("  \"risks\": ");
    print_trust_risks_json(&risks);
    println!(",");
    println!("  \"required_permissions\": [");
    for (idx, permission) in projected_permissions.iter().enumerate() {
        let comma = if idx + 1 == projected_permissions.len() {
            ""
        } else {
            ","
        };
        println!("    \"{}\"{}", json_escape(permission), comma);
    }
    println!("  ],");
    println!(
        "  \"policy_disposition\": \"{}\",",
        json_escape(disposition)
    );
    println!("  \"execution\": \"not_run\"");
    println!("}}");
}

fn cpx_risk_json_compact(
    requested: &str,
    package_specs: &[PackageExecSpec],
    local_package_dir: Option<&std::path::Path>,
    resolved_executable: Option<&std::path::Path>,
) -> String {
    let registry_policy = cpx_registry_policy();
    let selected_package = package_specs.first().map(|spec| spec.name.as_str());
    let (registry, matched_scope) = selected_package
        .map(|package| {
            rusty_js_pm::registry_policy::selected_registry_for_package(&registry_policy, package)
        })
        .unwrap_or((&registry_policy.default_registry, None));
    let registry = registry.to_string();
    let matched_scope = matched_scope.map(|scope| scope.to_string());
    let fallback_decision = cpx_registry_fallback_decision(&registry_policy, selected_package);
    let source_disposition = cpx_registry_source_disposition(
        &registry_policy,
        selected_package,
        matched_scope.as_deref(),
    );
    let risks = local_package_dir
        .map(collect_package_exec_manifest_risks)
        .unwrap_or_default();
    let source_kind = if resolved_executable.is_some() {
        "local"
    } else if package_specs.is_empty() {
        "unknown-local-miss"
    } else {
        "registry-unfetched"
    };
    let projected_permissions = cpx_projected_permissions(source_kind, &risks);
    let mut out = String::new();
    out.push('{');
    out.push_str("\"schema\":\"cruft.package_exec_risk.v1\",");
    out.push_str("\"tool\":\"cpx\",");
    out.push_str(&format!("\"requested\":\"{}\",", json_escape(requested)));
    out.push_str("\"source\":{");
    out.push_str(&format!("\"kind\":\"{}\",", json_escape(source_kind)));
    out.push_str(&format!("\"registry\":\"{}\",", json_escape(&registry)));
    out.push_str(&format!(
        "\"local_package_dir\":{},",
        json_optional_string(&local_package_dir.map(|p| p.display().to_string()))
    ));
    out.push_str(&format!(
        "\"resolved_executable\":{}",
        json_optional_string(&resolved_executable.map(|p| p.display().to_string()))
    ));
    out.push_str("},");
    out.push_str("\"registry_policy\":{");
    out.push_str(&format!(
        "\"source\":\"{}\",",
        json_escape(&registry_policy.source)
    ));
    out.push_str(&format!(
        "\"source_path\":{},",
        json_optional_string(
            &registry_policy
                .source_path
                .as_ref()
                .map(|p| p.display().to_string())
        )
    ));
    out.push_str(&format!(
        "\"default_registry\":\"{}\",",
        json_escape(&registry_policy.default_registry)
    ));
    out.push_str(&format!(
        "\"selected_registry\":\"{}\",",
        json_escape(&registry)
    ));
    out.push_str(&format!(
        "\"matched_scope\":{},",
        json_optional_string(&matched_scope)
    ));
    out.push_str("\"scope_mappings\":[");
    for (idx, mapping) in registry_policy.scopes.iter().enumerate() {
        if idx > 0 {
            out.push(',');
        }
        out.push_str(&format!(
            "{{\"scope\":\"{}\",\"registry\":\"{}\"}}",
            json_escape(&mapping.scope),
            json_escape(&mapping.registry)
        ));
    }
    out.push_str("],");
    out.push_str(&format!(
        "\"fallback_policy\":\"{}\",",
        json_escape(&registry_policy.public_fallback)
    ));
    out.push_str(&format!(
        "\"fallback_decision\":\"{}\",",
        json_escape(fallback_decision)
    ));
    out.push_str(&format!(
        "\"source_disposition\":\"{}\",",
        json_escape(source_disposition)
    ));
    out.push_str(&format!(
        "\"auth_mode\":\"{}\"",
        json_escape(&registry_policy.auth_mode)
    ));
    out.push_str("},");
    out.push_str("\"packages\":[");
    for (idx, spec) in package_specs.iter().enumerate() {
        if idx > 0 {
            out.push(',');
        }
        out.push_str(&format!(
            "{{\"name\":\"{}\",\"range\":\"{}\",\"status\":\"metadata_unfetched\"}}",
            json_escape(&spec.name),
            json_escape(&spec.range)
        ));
    }
    out.push_str("],");
    out.push_str("\"unknown_facts\":[");
    if source_kind == "registry-unfetched" {
        out.push_str("{\"field\":\"registry_metadata\",\"status\":\"unavailable\",\"reason\":\"risk_json_no_exec_does_not_fetch\"},");
        out.push_str("{\"field\":\"tarball_artifact\",\"status\":\"unavailable\",\"reason\":\"risk_json_no_exec_does_not_fetch\"}");
    }
    out.push_str("],");
    out.push_str("\"risks\":[");
    for (idx, risk) in risks.iter().enumerate() {
        if idx > 0 {
            out.push(',');
        }
        out.push_str(&format!(
            "{{\"kind\":\"{}\",\"level\":\"{}\",\"subject\":\"{}\",\"reason\":\"{}\"}}",
            json_escape(&risk.kind),
            json_escape(&risk.level),
            json_escape(&risk.subject),
            json_escape(&risk.reason)
        ));
    }
    out.push_str("],");
    out.push_str("\"required_permissions\":[");
    for (idx, permission) in projected_permissions.iter().enumerate() {
        if idx > 0 {
            out.push(',');
        }
        out.push_str(&format!("\"{}\"", json_escape(permission)));
    }
    out.push_str("],");
    out.push_str(&format!(
        "\"policy_disposition\":\"{}\",",
        json_escape(cpx_risk_disposition(source_kind, &risks))
    ));
    out.push_str("\"execution\":\"not_run\"");
    out.push('}');
    out
}

fn append_cpx_yes_audit(
    audit_log: &str,
    requested: &str,
    package_specs: &[PackageExecSpec],
) -> Result<(), String> {
    let artifact_json = cpx_risk_json_compact(requested, package_specs, None, None);
    let artifact_hash = agent::integrity::agent_source_hash(&artifact_json);
    let mut encoded = format!(
        "{{\"type\":\"cpx_exec_yes_risk_audit\",\"schema_version\":1,\"tool\":\"cpx\",\"requested\":\"{}\",\"artifact_hash\":\"{}\",\"artifact\":{},\"execution\":\"not_run\",\"nonclaims\":[\"fnv1a64 is deterministic tamper evidence, not a cryptographic signature\",\"this row records pre-materialization risk evidence, not successful package execution\"]}}",
        json_escape(requested),
        json_escape(&artifact_hash),
        artifact_json
    );
    encoded.push('\n');
    if let Some(parent) = std::path::Path::new(audit_log).parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(|e| {
                format!(
                    "cannot create audit log directory {}: {e}",
                    parent.display()
                )
            })?;
        }
    }
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(audit_log)
        .map_err(|e| format!("cannot open audit log {audit_log}: {e}"))?;
    use std::io::Write;
    file.write_all(encoded.as_bytes())
        .map_err(|e| format!("cannot append audit log {audit_log}: {e}"))
}

fn print_sandbox_control_json(
    control: &str,
    level: &str,
    profile: Option<&str>,
    predicate: &str,
    comma: bool,
) {
    let comma = if comma { "," } else { "" };
    println!(
        "    {{\"control\":\"{}\",\"level\":\"{}\",\"profile\":{},\"predicate\":\"{}\",\"next_action\":\"{}\",\"owner\":\"{}\"}}{}",
        json_escape(control),
        json_escape(level),
        json_optional_string(&profile.map(str::to_string)),
        json_escape(predicate),
        json_escape(frontdoor_policy_control_next_action(control, level)),
        json_escape(frontdoor_policy_control_owner(control)),
        comma
    );
}

fn print_doctor_json(wrapper: &DoctorWrapperReport) {
    println!("{{");
    println!("  \"schema_version\": 1,");
    println!("  \"tool\": \"cruft doctor\",");
    println!("  \"project\": {{");
    let cwd = std::env::current_dir()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| ".".to_string());
    println!("    \"root\": \"{}\",", json_escape(&cwd));
    println!("    \"package_manager\": \"npm\",");
    let lockfile = find_npm_lockfile_from(&std::path::PathBuf::from(&cwd))
        .map(|(_, path)| path.display().to_string());
    let package_json = find_package_json_from_cwd();
    let mut risks = Vec::new();
    if let Some(package_json) = &package_json {
        risks.extend(collect_manifest_trust_risks(package_json));
    }
    if let Some(lockfile_path) = lockfile.as_ref() {
        risks.extend(collect_lockfile_trust_risks(std::path::Path::new(
            lockfile_path,
        )));
    }
    let mut security_scan_status = if lockfile.is_some() {
        "not-run".to_string()
    } else {
        "no-lockfile".to_string()
    };
    let mut security_scan_reason = if lockfile.is_some() {
        "OSV scan not run by doctor --json unless CRUFT_OSV_FIXTURE is provided".to_string()
    } else {
        "no npm lockfile found".to_string()
    };
    if std::env::var_os("CRUFT_OSV_FIXTURE").is_some() {
        match collect_osv_trust_risks(lockfile.as_deref().map(std::path::Path::new)) {
            Ok(osv_risks) => {
                security_scan_status = if osv_risks.is_empty() {
                    "clean".to_string()
                } else {
                    "advisory".to_string()
                };
                security_scan_reason = "OSV fixture scan completed".to_string();
                risks.extend(osv_risks);
            }
            Err(e) => {
                security_scan_status = "failed".to_string();
                security_scan_reason = e;
            }
        }
    }
    risks.sort_by(|a, b| (&a.kind, &a.subject, &a.reason).cmp(&(&b.kind, &b.subject, &b.reason)));
    risks.dedup_by(|a, b| a.kind == b.kind && a.subject == b.subject && a.reason == b.reason);
    println!("    \"lockfile\": {}", json_optional_string(&lockfile));
    println!("  }},");
    println!("  \"wrapper\": {{");
    println!("    \"state\": \"{}\",", json_escape(&wrapper.state));
    println!("    \"home\": {},", json_optional_string(&wrapper.home));
    println!(
        "    \"manifest\": {},",
        json_optional_string(&wrapper.manifest)
    );
    println!(
        "    \"shim_dir\": {},",
        json_optional_string(&wrapper.shim_dir)
    );
    println!(
        "    \"cruft_binary\": {},",
        json_optional_string(&wrapper.cruft_binary)
    );
    print!("    \"managed_commands\": ");
    print_json_string_array(&wrapper.managed_commands);
    println!(",");
    println!("    \"commands\": [");
    for (idx, cmd) in wrapper.command_resolution.iter().enumerate() {
        let comma = if idx + 1 == wrapper.command_resolution.len() {
            ""
        } else {
            ","
        };
        println!(
            "      {{\"command\":\"{}\",\"resolved\":{},\"through_cruft\":{}}}{}",
            json_escape(&cmd.command),
            json_optional_string(&cmd.resolved),
            if cmd.through_cruft { "true" } else { "false" },
            comma
        );
    }
    println!("    ]");
    println!("  }},");
    println!("  \"backend\": {{");
    println!("    \"recommended\": \"node\",");
    println!("    \"reason\": \"maximum compatibility unless exact packaged executable evidence admits Cruft runtime\",");
    println!("    \"auto_policy\": \"exact-packaged-evidence-only\"");
    println!("  }},");
    println!("  \"diagnostics\": {{");
    println!("    \"default_mode\": \"public\",");
    println!("    \"explicit_mode\": \"structural\",");
    println!("    \"cli_flag\": \"--diagnostics=structural\",");
    println!("    \"artifact_flag\": \"--diagnostic-log\",");
    println!("    \"env\": [\"CRUFT_DIAGNOSTICS\", \"CRUFT_DIAGNOSTIC_MODE\"],");
    println!("    \"artifact_env\": [\"CRUFT_DIAGNOSTIC_LOG\", \"CRUFT_DIAGNOSTIC_ARTIFACT\"],");
    println!("    \"public_projection\": \"redacts structural runtime internals from default user-facing TypeError diagnostics\",");
    println!("    \"structural_projection\": \"preserves receiver, argument count, method, and prototype-chain diagnostics for explicit maintainer use\",");
    println!("    \"artifact_projection\": \"writes explicit local JSONL structural diagnostics without remote telemetry\",");
    println!("    \"non_claim\": \"public diagnostics are a disclosure boundary, not a compatibility or sandbox guarantee\"");
    println!("  }},");
    println!("  \"sandbox_profiles\": [");
    if WrapSandbox::MacosStrict.supported() {
        println!("    {{\"name\":\"macos-strict\",\"supported\":true,\"platform\":\"macos\",\"controls\":[\"filesystem-write\",\"network\",\"environment\",\"external-process-exec\"],\"limitations\":[\"not default behavior\",\"same-executable child exec remains allowed\",\"complete child-process denial unproven\",\"not portable to Linux/Windows\"]}}");
    } else if WrapSandbox::LinuxStrict.supported() {
        println!("    {{\"name\":\"linux-strict\",\"supported\":true,\"platform\":\"linux\",\"controls\":[\"filesystem-write\",\"environment\",\"external-process-exec\"],\"limitations\":[\"not default behavior\",\"network control unproven\",\"complete child-process denial unproven\",\"Linux Landlock support required\"]}}");
    } else if cfg!(target_os = "linux") {
        println!("    {{\"name\":\"linux-strict\",\"supported\":false,\"platform\":\"linux\",\"controls\":[],\"limitations\":[\"{}\"]}}", json_escape(&WrapSandbox::LinuxStrict.unsupported_reason()));
    } else if WrapSandbox::WindowsStrict.supported() {
        println!("    {{\"name\":\"windows-strict\",\"supported\":true,\"platform\":\"windows\",\"controls\":[\"process-lifetime\",\"environment\"],\"limitations\":[\"baseline mouth only\",\"filesystem-write control not yet derived\",\"filesystem-read control not yet derived\",\"network control not yet derived\",\"child-process denial not yet derived\"]}}");
    }
    println!("  ],");
    println!("  \"sandbox_controls\": [");
    if WrapSandbox::MacosStrict.supported() {
        print_sandbox_control_json(
            "filesystem-write",
            "enforced",
            Some("macos-strict"),
            "macOS only; explicit --sandbox=macos-strict",
            true,
        );
        print_sandbox_control_json(
            "network",
            "enforced",
            Some("macos-strict"),
            "macOS only; explicit --sandbox=macos-strict",
            true,
        );
        print_sandbox_control_json(
            "environment",
            "enforced",
            Some("macos-strict"),
            "macOS only; explicit --sandbox=macos-strict",
            true,
        );
        print_sandbox_control_json(
            "external-process-exec",
            "enforced",
            Some("macos-strict"),
            "macOS only; explicit --sandbox=macos-strict; same-executable child exec remains allowed",
            true,
        );
    } else if WrapSandbox::LinuxStrict.supported() {
        print_sandbox_control_json(
            "filesystem-write",
            "enforced",
            Some("linux-strict"),
            "Linux only; explicit --sandbox=linux-strict; Landlock ABI >= 1",
            true,
        );
        print_sandbox_control_json(
            "network",
            "not-available",
            None,
            "Linux network sandbox has not been derived and gated",
            true,
        );
        print_sandbox_control_json(
            "environment",
            "enforced",
            Some("linux-strict"),
            "Linux only; explicit --sandbox=linux-strict",
            true,
        );
        print_sandbox_control_json(
            "external-process-exec",
            "enforced",
            Some("linux-strict"),
            "Linux only; explicit --sandbox=linux-strict; Landlock execute allowlist",
            true,
        );
    } else if cfg!(target_os = "linux") {
        print_sandbox_control_json(
            "filesystem-write",
            "not-available",
            None,
            &WrapSandbox::LinuxStrict.unsupported_reason(),
            true,
        );
        print_sandbox_control_json(
            "network",
            "not-available",
            None,
            "Linux network sandbox has not been derived and gated",
            true,
        );
        print_sandbox_control_json(
            "environment",
            "not-available",
            None,
            &WrapSandbox::LinuxStrict.unsupported_reason(),
            true,
        );
        print_sandbox_control_json(
            "external-process-exec",
            "not-available",
            None,
            &WrapSandbox::LinuxStrict.unsupported_reason(),
            true,
        );
    } else {
        let profile = if WrapSandbox::WindowsStrict.supported() {
            Some("windows-strict")
        } else {
            None
        };
        let predicate = if WrapSandbox::WindowsStrict.supported() {
            "windows-strict baseline mouth exists; this control is not derived or gated yet"
        } else {
            "no supported OS sandbox profile on this platform"
        };
        print_sandbox_control_json(
            "filesystem-write",
            "not-available",
            profile,
            predicate,
            true,
        );
        print_sandbox_control_json("network", "not-available", profile, predicate, true);
        if WrapSandbox::WindowsStrict.supported() {
            print_sandbox_control_json(
                "environment",
                "enforced",
                Some("windows-strict"),
                "windows-strict clears inherited environment and restores only PATH/SystemRoot/WINDIR/TEMP/TMP plus CRUFT_WRAP_SANDBOX",
                true,
            );
        } else {
            print_sandbox_control_json("environment", "not-available", profile, predicate, true);
        }
        print_sandbox_control_json(
            "external-process-exec",
            "not-available",
            profile,
            predicate,
            true,
        );
    }
    if WrapSandbox::WindowsStrict.supported() {
        print_sandbox_control_json(
            "windows-process-lifetime",
            "enforced",
            Some("windows-strict"),
            "Windows Job Object with kill-on-close and optional --timeout-ms tree termination; not a filesystem, network, environment, or child-process denial claim",
            true,
        );
    }
    print_sandbox_control_json(
        "complete-child-process-denial",
        "not-available",
        None,
        "complete child-process denial has not been derived and gated",
        true,
    );
    if WrapSandbox::WindowsStrict.supported() {
        print_sandbox_control_json(
            "linux-windows-sandbox",
            "not-available",
            Some("windows-strict"),
            "Windows has a limited experimental profile for process lifetime and environment only; Linux and broad OS sandbox controls remain not derived",
            false,
        );
    } else {
        print_sandbox_control_json(
            "linux-windows-sandbox",
            "not-available",
            None,
            "cross-platform Linux+Windows sandbox support has not been derived and gated; inspect sandbox_profiles for platform-specific profiles",
            false,
        );
    }
    println!("  ],");
    println!("  \"evidence\": [");
    println!("    {{\"kind\":\"wrapper-state\",\"level\":\"info\",\"subject\":\"{}\",\"reason\":\"doctor wrapper state\"}},", json_escape(&wrapper.state));
    println!(
        "    {{\"kind\":\"package-json\",\"level\":\"{}\",\"subject\":{},\"reason\":\"nearest package manifest from cwd\"}},",
        if package_json.is_some() { "info" } else { "missing" },
        json_optional_string(&package_json.as_ref().map(|p| p.display().to_string()))
    );
    println!(
        "    {{\"kind\":\"npm-lockfile\",\"level\":\"{}\",\"subject\":{},\"reason\":\"nearest npm lockfile from cwd\"}}",
        if lockfile.is_some() { "info" } else { "missing" },
        json_optional_string(&lockfile)
    );
    println!("  ],");
    println!("  \"security_scan\": {{");
    println!("    \"provider\": \"osv\",");
    println!(
        "    \"status\": \"{}\",",
        json_escape(&security_scan_status)
    );
    println!("    \"reason\": \"{}\"", json_escape(&security_scan_reason));
    println!("  }},");
    let mut policy_json = String::new();
    push_frontdoor_policy_json(&mut policy_json, "ci");
    println!("  \"policy\": {},", policy_json);
    print!("  \"risks\": ");
    print_trust_risks_json(&risks);
    println!(",");
    println!("  \"next_steps\": [");
    println!("    \"cruft wrap install\",");
    println!("    \"cruft wrap status\",");
    println!("    \"cruft run --backend=auto --explain app.js\",");
    println!("    \"cruft doctor --security\",");
    println!("    \"cruft trust install --enforce -- npm install\",");
    if WrapSandbox::MacosStrict.supported() {
        println!("    \"cruft wrap --sandbox=macos-strict -- node app.js\",");
    }
    println!("    \"cruft unwrap --all\"");
    println!("  ]");
    println!("}}");
}

fn run_security_doctor() -> ExitCode {
    println!("Cruft security doctor");
    println!();
    println!("Mode: advisory dependency-risk report; no files are modified.");
    let cwd = match std::env::current_dir() {
        Ok(cwd) => cwd,
        Err(e) => {
            eprintln!("cruft doctor --security: cannot read cwd: {e}");
            return ExitCode::from(1);
        }
    };
    let Some((root, lockfile)) = find_npm_lockfile_from(&cwd) else {
        println!("No npm lockfile found from {}.", cwd.display());
        println!("Action: run this from a project with package-lock.json or npm-shrinkwrap.json after dependencies are resolved.");
        println!("Result: advisory scan not run.");
        return ExitCode::SUCCESS;
    };
    let source_name = lockfile
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("npm lockfile");
    println!("Project root: {}", root.display());
    println!("Evidence: {source_name}");
    let packages = match extract_npm_lock_packages(&lockfile) {
        Ok(packages) => packages,
        Err(e) => {
            eprintln!("cruft doctor --security: {e}");
            return ExitCode::from(1);
        }
    };
    println!("Packages: {}", packages.len());
    if packages.is_empty() {
        println!("Result: no installed npm package coordinates found in lockfile.");
        return ExitCode::SUCCESS;
    }
    let findings = match query_osv_for_packages(&packages) {
        Ok(findings) => findings,
        Err(e) => {
            eprintln!("cruft doctor --security: OSV scan failed: {e}");
            return ExitCode::from(1);
        }
    };
    let malicious = findings.iter().filter(|f| f.malicious).count();
    let vulnerable = findings.len().saturating_sub(malicious);
    println!("Known vulnerabilities: {vulnerable}");
    println!("Known malicious advisories: {malicious}");
    if findings.is_empty() {
        println!("Result: no OSV advisories reported for resolved npm package versions.");
    } else {
        println!();
        println!("Advisory rows:");
        for f in &findings {
            let risk = TrustRisk {
                kind: if f.malicious {
                    "known-malicious-advisory"
                } else {
                    "known-vulnerability"
                }
                .to_string(),
                level: "advisory".to_string(),
                subject: format!("{}@{}", f.package.name, f.package.version),
                reason: format!("{} {}", f.id, f.summary).trim().to_string(),
            };
            print_trust_risk(&risk);
        }
        println!();
        println!("Result: advisory only. Cruft is not blocking install or constraining Node child processes in this mode.");
    }
    ExitCode::SUCCESS
}

fn find_npm_lockfile_from(
    start: &std::path::Path,
) -> Option<(std::path::PathBuf, std::path::PathBuf)> {
    for dir in start.ancestors() {
        let package_lock = dir.join("package-lock.json");
        if package_lock.is_file() {
            return Some((dir.to_path_buf(), package_lock));
        }
        let shrinkwrap = dir.join("npm-shrinkwrap.json");
        if shrinkwrap.is_file() {
            return Some((dir.to_path_buf(), shrinkwrap));
        }
    }
    None
}

fn extract_npm_lock_packages(
    lockfile: &std::path::Path,
) -> Result<Vec<SecurityDoctorPackage>, String> {
    let src = std::fs::read_to_string(lockfile)
        .map_err(|e| format!("cannot read {}: {e}", lockfile.display()))?;
    let json = serde_json::from_str::<serde_json::Value>(&src)
        .map_err(|e| format!("cannot parse {}: {e}", lockfile.display()))?;
    let mut packages = Vec::new();
    if let Some(obj) = json.get("packages").and_then(|v| v.as_object()) {
        for (path, rec) in obj {
            if path.is_empty() {
                continue;
            }
            let Some(version) = rec.get("version").and_then(|v| v.as_str()) else {
                continue;
            };
            let name = rec
                .get("name")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .or_else(|| package_name_from_node_modules_path(path));
            let Some(name) = name else {
                continue;
            };
            packages.push(SecurityDoctorPackage {
                name,
                version: version.to_string(),
            });
        }
    } else if let Some(deps) = json.get("dependencies").and_then(|v| v.as_object()) {
        collect_lock_dependencies(deps, &mut packages);
    }
    packages.sort();
    packages.dedup();
    Ok(packages)
}

fn package_name_from_node_modules_path(path: &str) -> Option<String> {
    let marker = "node_modules/";
    let idx = path.rfind(marker)?;
    let tail = &path[idx + marker.len()..];
    let mut parts = tail.split('/');
    let first = parts.next()?.to_string();
    if first.starts_with('@') {
        let second = parts.next()?;
        Some(format!("{first}/{second}"))
    } else {
        Some(first)
    }
}

fn collect_lock_dependencies(obj: &serde_json::Map, out: &mut Vec<SecurityDoctorPackage>) {
    for (name, rec) in obj {
        if let Some(version) = rec.get("version").and_then(|v| v.as_str()) {
            out.push(SecurityDoctorPackage {
                name: name.clone(),
                version: version.to_string(),
            });
        }
        if let Some(children) = rec.get("dependencies").and_then(|v| v.as_object()) {
            collect_lock_dependencies(children, out);
        }
    }
}

fn query_osv_for_packages(
    packages: &[SecurityDoctorPackage],
) -> Result<Vec<SecurityDoctorFinding>, String> {
    let body = if let Ok(path) = std::env::var("CRUFT_OSV_FIXTURE") {
        std::fs::read_to_string(&path)
            .map_err(|e| format!("cannot read CRUFT_OSV_FIXTURE {path}: {e}"))?
    } else {
        let payload = osv_querybatch_payload(packages);
        let output = std::process::Command::new("curl")
            .args([
                "-sS",
                "--max-time",
                "10",
                "-H",
                "Content-Type: application/json",
                "-d",
                &payload,
                "https://api.osv.dev/v1/querybatch",
            ])
            .output()
            .map_err(|e| format!("cannot execute curl for OSV querybatch: {e}"))?;
        if !output.status.success() {
            return Err(format!(
                "OSV querybatch exited {}: {}",
                output.status.code().unwrap_or(1),
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        String::from_utf8_lossy(&output.stdout).into_owned()
    };
    parse_osv_querybatch_response(packages, &body)
}

fn osv_querybatch_payload(packages: &[SecurityDoctorPackage]) -> String {
    let mut out = String::from("{\"queries\":[");
    for (i, p) in packages.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str("{\"package\":{\"ecosystem\":\"npm\",\"name\":\"");
        out.push_str(&json_escape(&p.name));
        out.push_str("\"},\"version\":\"");
        out.push_str(&json_escape(&p.version));
        out.push_str("\"}");
    }
    out.push_str("]}");
    out
}

fn parse_osv_querybatch_response(
    packages: &[SecurityDoctorPackage],
    body: &str,
) -> Result<Vec<SecurityDoctorFinding>, String> {
    let json = serde_json::from_str::<serde_json::Value>(body)
        .map_err(|e| format!("cannot parse OSV response: {e}"))?;
    let results = json
        .get("results")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "OSV response missing results array".to_string())?;
    let mut findings = Vec::new();
    for (idx, result) in results.iter().enumerate() {
        let Some(package) = packages.get(idx) else {
            break;
        };
        let Some(vulns) = result.get("vulns").and_then(|v| v.as_array()) else {
            continue;
        };
        for vuln in vulns {
            let id = vuln
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("UNKNOWN")
                .to_string();
            let summary = vuln
                .get("summary")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let malicious = id.starts_with("MAL-");
            findings.push(SecurityDoctorFinding {
                package: package.clone(),
                id,
                summary,
                malicious,
            });
        }
    }
    Ok(findings)
}

fn json_escape(s: &str) -> String {
    let mut out = String::new();
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c < ' ' => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

fn json_string_literal(s: &str) -> String {
    format!("\"{}\"", json_escape(s))
}

fn real_main() -> ExitCode {
    let raw_args: Vec<String> = std::env::args().collect();

    if raw_args.get(1).map(|s| s.as_str()) == Some("wrap") {
        return run_wrap_subcommand(&raw_args);
    }
    if raw_args.get(1).map(|s| s.as_str()) == Some("unwrap") {
        return run_unwrap_subcommand(&raw_args);
    }
    let (cap_mode, audit_log_path, diagnostic_log_path, allow_net_loopback, mut args) =
        parse_cap_flags(raw_args);
    let invoked_as_cpx = args
        .first()
        .and_then(|a| std::path::Path::new(a).file_stem())
        .and_then(|s| s.to_str())
        .map(|s| s == "cpx")
        .unwrap_or(false);
    if invoked_as_cpx && std::env::var_os("CRUFT_EXEC_AS_RUNTIME").is_none() {
        args.insert(1, "exec".to_string());
    }

    if args.get(1).map(|s| s.as_str()) == Some("test262")
        && std::env::var_os("CRUFT_DEVIATIONS").is_none()
    {
        std::env::set_var("CRUFT_DEVIATIONS", "off");
    }
    if std::env::var_os("CRUFT_NODE_CORE_TEST").is_some()
        && std::env::var_os("CRUFT_DEVIATIONS").is_none()
    {
        std::env::set_var("CRUFT_DEVIATIONS", "node");
    }

    let _test_driver_guard = if args.iter().skip(1).any(|a| a.starts_with("--test")) {
        maybe_enter_test_mode(&mut args)
    } else {
        None
    };

    if args.len() >= 3 && args[1] == "test262" && args[2] == "status" {
        return run_test262_status(&args[3..]);
    }
    if args.len() >= 3 && args[1] == "test262" && args[2] == "sweep" {
        return run_test262_sweep(&args[3..]);
    }
    if args.len() >= 3 && args[1] == "test262" && args[2] == "run" {
        let mut path: Option<String> = None;
        let mut runner = std::env::var("T262_RUNNER").ok();
        let mut harness = std::env::var("T262_HARNESS_DIR").ok();
        let mut it = args[3..].iter();
        while let Some(a) = it.next() {
            match a.as_str() {
                "--runner" => runner = it.next().cloned(),
                "--harness" => harness = it.next().cloned(),
                "--variant" => {
                    let _ = it.next();
                }
                _ if path.is_none() => path = Some(a.clone()),
                _ => {}
            }
        }
        let path = match path {
            Some(p) => p,
            None => {
                eprintln!("cruft test262 run: missing <path>");
                return ExitCode::from(64);
            }
        };
        let runner = runner.unwrap_or_else(|| "scripts/test262/runner-full.mjs".to_string());
        std::env::set_var("T262_TEST_PATH", &path);
        if let Some(h) = harness {
            std::env::set_var("T262_HARNESS_DIR", h);
        }

        args = vec![args[0].clone(), runner];
    }

    if let Ok(no) = std::env::var("NODE_OPTIONS") {
        let toks: Vec<String> = no.split_whitespace().map(String::from).collect();
        let mut insert_at = 1;
        let mut j = 0;
        while j < toks.len() {
            let t = toks[j].as_str();

            let disallowed = matches!(
                t,
                "-e" | "--eval"
                    | "-p"
                    | "--print"
                    | "-i"
                    | "--interactive"
                    | "-c"
                    | "--check"
                    | "-"
            ) || t.starts_with("--env-file");
            if disallowed {
                eprintln!("cruft: {} is not allowed in NODE_OPTIONS", t);
                return ExitCode::from(64);
            }

            if t == "-r" || t == "--require" || t == "--input-type" {
                args.insert(insert_at, t.to_string());
                insert_at += 1;
                j += 1;
                match toks.get(j) {
                    Some(v) => {
                        args.insert(insert_at, v.clone());
                        insert_at += 1;
                    }
                    None => {
                        eprintln!("cruft: {} requires an argument", t);
                        return ExitCode::from(64);
                    }
                }
            } else if let Some(path) = t.strip_prefix("--require=") {
                args.insert(insert_at, "--require".to_string());
                args.insert(insert_at + 1, path.to_string());
                insert_at += 2;
            } else if t.starts_with("--input-type=") {
                args.insert(insert_at, t.to_string());
                insert_at += 1;
            }

            j += 1;
        }
    }

    let mut require_preloads: Vec<String> = Vec::new();

    let mut env_files: Vec<(String, bool)> = Vec::new();

    let mut input_type: Option<String> = None;
    {
        let mut i = 1;
        while i < args.len() {
            let a = args[i].as_str();
            if a == "--input-type" || a.starts_with("--input-type=") {
                if let Some(eq) = a.find('=') {
                    input_type = Some(a[eq + 1..].to_string());
                    args.remove(i);
                } else {
                    if i + 1 >= args.len() {
                        eprintln!("cruft: {} requires an argument", a);
                        return ExitCode::from(64);
                    }
                    input_type = Some(args[i + 1].clone());
                    args.drain(i..=i + 1);
                }
                continue;
            }
            if a == "--env-file"
                || a == "--env-file-if-exists"
                || a.starts_with("--env-file=")
                || a.starts_with("--env-file-if-exists=")
            {
                let if_exists = a.starts_with("--env-file-if-exists");
                if let Some(eq) = a.find('=') {
                    env_files.push((a[eq + 1..].to_string(), if_exists));
                    args.remove(i);
                } else {
                    if i + 1 >= args.len() {
                        eprintln!("cruft: {} requires an argument", a);
                        return ExitCode::from(64);
                    }
                    env_files.push((args[i + 1].clone(), if_exists));
                    args.drain(i..=i + 1);
                }
                continue;
            }
            if a == "-r" || a == "--require" {
                if i + 1 >= args.len() {
                    eprintln!("cruft: {} requires an argument", a);
                    return ExitCode::from(64);
                }
                let f = args[i + 1].clone();
                if !require_preloads.contains(&f) {
                    require_preloads.push(f);
                }
                args.drain(i..=i + 1);
                continue;
            }
            if a == "--pending-deprecation" {
                std::env::set_var("NODE_PENDING_DEPRECATION", "1");
                args.remove(i);
                continue;
            }
            if a == "--no-warnings" {
                std::env::set_var("CRUFT_NO_WARNINGS", "1");
                args.remove(i);
                continue;
            }
            if let Some(policy) = a.strip_prefix("--unhandled-rejections=") {
                match policy {
                    "strict" | "throw" => std::env::remove_var("CRUFT_UNHANDLED_REJECTION"),
                    "warn" => std::env::set_var("CRUFT_UNHANDLED_REJECTION", "warn"),
                    "none" | "silent" => std::env::set_var("CRUFT_UNHANDLED_REJECTION", "none"),
                    _ => {
                        eprintln!("cruft: invalid --unhandled-rejections value: {}", policy);
                        return ExitCode::from(64);
                    }
                }
                args.remove(i);
                continue;
            }
            if a.starts_with("--stack_size=") || a.starts_with("--stack-size=") {
                std::env::set_var("CRUFT_NODE_STACK_SIZE_FLAG", "1");
                args.remove(i);
                continue;
            }
            if a.starts_with('-') && a.len() > 1 && a != "--" {
                if a == "-c" || a == "--check" {

                    i += 1;
                    continue;
                }
                if VALUE_FLAGS.contains(&a) {
                    break;
                }
                i += 1;
                continue;
            }
            break;
        }
    }

    let input_forces_module = match input_type.as_deref() {
        None => None,
        Some("module") | Some("module-typescript") => Some(true),
        Some("commonjs") | Some("commonjs-typescript") => Some(false),
        Some(other) => {
            eprintln!(
                "cruft: --input-type must be \"module\",\"commonjs\", \
                 \"module-typescript\" or \"commonjs-typescript\" (got {:?})",
                other
            );
            return ExitCode::from(64);
        }
    };

    for (path, if_exists) in &env_files {
        match std::fs::read_to_string(path) {
            Ok(body) => {
                for line in body.lines() {
                    let line = line.trim();
                    if line.is_empty() || line.starts_with('#') {
                        continue;
                    }
                    let line = line.strip_prefix("export ").unwrap_or(line);
                    if let Some((k, v)) = line.split_once('=') {
                        let k = k.trim();
                        let mut v = v.trim();
                        if v.len() >= 2
                            && ((v.starts_with('"') && v.ends_with('"'))
                                || (v.starts_with('\'') && v.ends_with('\'')))
                        {
                            v = &v[1..v.len() - 1];
                        }
                        if !k.is_empty() && std::env::var_os(k).is_none() {
                            std::env::set_var(k, v);
                        }
                    }
                }
            }
            Err(_) if *if_exists => {
                eprintln!("cruft: {} not found. Continuing without it.", path);
            }
            Err(_) => {
                eprintln!("cruft: {}: not found", path);
                return ExitCode::from(66);
            }
        }
    }

    if let Some(bad) = first_unknown_flag(&args) {
        eprintln!("cruft: bad option: {}", bad);
        return ExitCode::from(64);
    }

    let mut opts_end = leading_options_end(&args);
    let dashdash = args.get(opts_end).map(|a| a == "--").unwrap_or(false);
    if dashdash {
        args.remove(opts_end);
    }

    let value_scan_end = if dashdash {
        opts_end
    } else {
        (opts_end + 1).min(args.len())
    };
    opts_end = opts_end.min(args.len());

    if args[1..opts_end].iter().any(|a| a == "-h" || a == "--help")
        || args.get(1).map(|a| a == "help").unwrap_or(false)
    {

        print_help(&mut std::io::stdout());
        return ExitCode::SUCCESS;
    }
    if args[1..opts_end].iter().any(|a| a == "--completion-bash") {
        print_bash_completion();
        return ExitCode::SUCCESS;
    }

    if args[1..opts_end]
        .iter()
        .any(|a| a == "-v" || a == "-V" || a == "--version")
    {
        print_version();
        return ExitCode::SUCCESS;
    }
    let entry_idx = opts_end;

    use std::io::IsTerminal;

    let is_tty = std::io::stdin().is_terminal();
    let repl_mode = args[1..opts_end]
        .iter()
        .any(|a| a == "-i" || a == "--interactive")
        || (args.len() < 2 && is_tty);
    if repl_mode {
        return run_repl(cap_mode, allow_net_loopback, &args, is_tty);
    }

    let stdin_script = args.get(entry_idx).map(|a| a == "-").unwrap_or(false)
        || (args.len() <= entry_idx && !std::io::stdin().is_terminal());
    if args.len() <= entry_idx && !stdin_script {

        print_help(&mut std::io::stderr());
        return ExitCode::from(64);
    }
    if args.get(entry_idx).map(|a| a == "install").unwrap_or(false) {
        return run_install_subcommand(&args[entry_idx + 1..]);
    }
    if args
        .get(entry_idx)
        .map(|a| a == "exec" || a == "cpx")
        .unwrap_or(false)
    {
        return run_exec_subcommand(&args[entry_idx + 1..], audit_log_path);
    }
    if args.get(entry_idx).map(|a| a == "doctor").unwrap_or(false) {
        return run_doctor_subcommand(&args[entry_idx + 1..]);
    }
    if args.get(entry_idx).map(|a| a == "agent").unwrap_or(false) {
        return run_agent_subcommand(&args[entry_idx + 1..]);
    }
    if args.get(entry_idx).map(|a| a == "compat").unwrap_or(false) {
        return run_compat_subcommand(&args[entry_idx + 1..]);
    }
    if args.get(entry_idx).map(|a| a == "promote").unwrap_or(false) {
        return run_promote_subcommand(&args[entry_idx + 1..]);
    }
    if args.get(entry_idx).map(|a| a == "policy").unwrap_or(false) {
        return run_policy_subcommand(&args[entry_idx + 1..]);
    }
    if args.get(entry_idx).map(|a| a == "trust").unwrap_or(false) {
        return run_trust_subcommand(&args[entry_idx + 1..], audit_log_path);
    }

    if !dashdash {
        if let Some(word) = args.get(entry_idx) {
            if cruft::reserved::looks_like_bare_command(word)
                && cruft::reserved::is_reserved_stub(word)
            {
                return cruft::reserved::reserved_stub_exit(word);
            }
        }
    }

    if let Some(i) = args[..value_scan_end]
        .iter()
        .enumerate()
        .skip(1)
        .find(|(_, a)| matches!(a.as_str(), "-c" | "--check"))
        .map(|(i, _)| i)
    {

        use std::io::IsTerminal;
        let stdin_check = match args.get(i + 1) {
            Some(f) if f == "-" => true,
            None if !std::io::stdin().is_terminal() => true,
            None => {
                eprintln!("cruft: --check requires a file argument");
                return ExitCode::from(64);
            }
            Some(_) => false,
        };
        let file = if stdin_check {
            "[stdin]".to_string()
        } else {
            args[i + 1].clone()
        };
        let raw = if stdin_check {
            use std::io::Read;
            let mut s = String::new();
            if let Err(e) = std::io::stdin().read_to_string(&mut s) {
                eprintln!("cruft: cannot read stdin: {}", e);
                return ExitCode::from(66);
            }
            s
        } else {
            match std::fs::read_to_string(&file) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("cruft: cannot read {}: {}", file, e);
                    return ExitCode::from(66);
                }
            }
        };
        if file.ends_with(".fts") {
            return run_fts_t0_entry(&file, &raw, true);
        }
        let src = if file.ends_with(".ts") || file.ends_with(".mts") || file.ends_with(".cts") {
            match ts_resolve::transform::ts_source_to_js_for_path(&file, &raw) {
                Ok((s, _)) => s,
                Err(e) => {
                    eprintln!("cruft: ts strip error in {}: {}", file, e);
                    return ExitCode::from(65);
                }
            }
        } else {
            raw
        };
        let url = format!("file://{}", file);

        let is_module = if stdin_check {
            input_forces_module.unwrap_or(false)
        } else {
            file.ends_with(".mjs")
                || file.ends_with(".mts")
                || matches!(
                    rusty_js_runtime::detect_module_kind(&url),
                    rusty_js_runtime::ModuleKind::ESM
                )
        };
        let parsed = if is_module {
            rusty_js_parser::parse_module_goal(&src)
        } else {
            rusty_js_parser::parse_script(&src)
        };
        return match parsed {
            Ok(_) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("cruft: syntax error in {}: {:?}", file, e);
                ExitCode::from(65)
            }
        };
    }

    let eval_flag_pos = args[..value_scan_end]
        .iter()
        .enumerate()
        .skip(1)
        .find(|(_, a)| matches!(a.as_str(), "-e" | "--eval" | "-p" | "--print"))
        .map(|(i, _)| i);

    let (source, entry_url, goal_is_module) = if let Some(i) = eval_flag_pos {
        let print_value = matches!(args[i].as_str(), "-p" | "--print");
        let mut code = match args.get(i + 1) {
            Some(c) => c.clone(),
            None => {
                eprintln!("cruft: {} requires an argument", args[i]);
                return ExitCode::from(64);
            }
        };

        let t = code.trim_start();
        let decl_start = ["function", "class", "async"].iter().any(|kw| {
            t.strip_prefix(kw).is_some_and(|rest| {
                rest.is_empty()
                    || rest.starts_with(|c: char| {
                        c.is_whitespace() || c == '*' || c == '(' || c == '{'
                    })
            })
        });
        let as_expr =
            !decl_start && rusty_js_parser::parse_script(&format!("({}\n);", code)).is_ok();
        if print_value {
            code = if as_expr {
                format!("console.log(({}\n));", code)
            } else {

                format!("console.log((0, eval)({:?}));", code)
            };
        }

        let cwd = std::env::current_dir()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default();
        let forced_module = input_forces_module.unwrap_or(false);
        if !forced_module {
            code = format!(
                "const __cruft_eval_Module = require(\"module\");\n\
                 const module = new __cruft_eval_Module(\"[eval]\");\n\
                 const __cruft_eval_exports = module.exports;\n\
                 module.path = {:?};\n\
                 module.filename = \"[eval]\";\n\
                 module.loaded = false;\n\
                 module.children = [];\n\
                 module.paths = [];\n\
                 module.require = require;\n\
                 const __filename = \"[eval]\";\n\
                 const __dirname = \".\";\n\
                 const exports = module.exports;\n{}",
                cwd, code
            );
        }

        args.drain(1..i + 2);

        (code, format!("file://{}/[eval]", cwd), forced_module)
    } else if stdin_script {

        use std::io::Read;
        let mut code = String::new();
        if let Err(e) = std::io::stdin().read_to_string(&mut code) {
            eprintln!("cruft: cannot read stdin: {}", e);
            return ExitCode::from(66);
        }
        let cwd = std::env::current_dir()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default();

        (
            code,
            format!("file://{}/[stdin]", cwd),
            input_forces_module.unwrap_or(false),
        )
    } else {

        let path = if args[entry_idx] == "run" {
            if args
                .get(entry_idx + 1)
                .map(|arg| is_help_arg(arg))
                .unwrap_or(false)
            {
                print_run_help();
                return ExitCode::SUCCESS;
            }
            let (run_backend, run_explain, run_entry_idx) =
                match parse_run_backend(&args, entry_idx + 1) {
                    Ok(parsed) => parsed,
                    Err(code) => return code,
                };
            if args.len() <= run_entry_idx {
                eprintln!("cruft run: missing file argument");
                return ExitCode::from(64);
            }
            let target = &args[run_entry_idx];

            if !is_explicit_run_path(target) {
                if let Some(package_json) = find_package_json_from_cwd() {
                    match package_script_value(&package_json, target) {
                        Ok(Some(script)) => {
                            let forwarded = script_arg_tail(&args, run_entry_idx + 1);
                            let auto_selection = if run_backend == RunBackend::Auto {
                                Some(select_auto_for_package_script(&package_json, target))
                            } else {
                                None
                            };
                            let effective_backend = auto_selection
                                .as_ref()
                                .map(|selection| selection.backend)
                                .unwrap_or(run_backend);
                            if run_explain {
                                explain_run_backend(
                                    run_backend,
                                    effective_backend,
                                    auto_selection.as_ref(),
                                );
                            }
                            let audit_argv = args[1..].to_vec();
                            append_frontdoor_audit_log(
                                audit_log_path.as_deref(),
                                "cruft run",
                                &audit_argv,
                                effective_backend.name(),
                                None,
                                auto_selection
                                    .as_ref()
                                    .and_then(|selection| selection.row.as_ref()),
                                &[],
                                None,
                                false,
                                false,
                            );
                            return run_package_script(
                                target,
                                &script,
                                &package_json,
                                &forwarded,
                                effective_backend,
                            );
                        }
                        Ok(None) => {}
                        Err(e) => {
                            eprintln!("cruft run: {}", e);
                            return ExitCode::from(65);
                        }
                    }
                }
            }
            let auto_selection = if run_backend == RunBackend::Auto {
                Some(select_auto_for_direct_file(target))
            } else {
                None
            };
            let effective_backend = auto_selection
                .as_ref()
                .map(|selection| selection.backend)
                .unwrap_or(run_backend);
            if run_explain {
                explain_run_backend(run_backend, effective_backend, auto_selection.as_ref());
            }
            let audit_argv = args[1..].to_vec();
            append_frontdoor_audit_log(
                audit_log_path.as_deref(),
                "cruft run",
                &audit_argv,
                effective_backend.name(),
                None,
                auto_selection
                    .as_ref()
                    .and_then(|selection| selection.row.as_ref()),
                &[],
                None,
                false,
                false,
            );
            if effective_backend == RunBackend::Node {
                let forwarded = script_arg_tail(&args, run_entry_idx + 1);
                return run_node_backend_file(target, &forwarded);
            }

            let target_owned = target.clone();
            args.drain(entry_idx..run_entry_idx);
            target_owned
        } else {
            args[entry_idx].clone()
        };

        let path = std::fs::canonicalize(&path)
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or(path);
        args[entry_idx] = path.clone();
        let raw = match std::fs::read_to_string(&path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("cruft: cannot read {}: {}", path, e);
                if std::env::var_os("CRUFT_NODE_FORK").is_some() {
                    return ExitCode::from(1);
                }
                return ExitCode::from(66);
            }
        };
        if path.ends_with(".fts") {
            return run_fts_t0_entry(&path, &raw, false);
        }

        let source = if path.ends_with(".ts") || path.ends_with(".mts") || path.ends_with(".cts") {
            match ts_resolve::transform::ts_source_to_js_for_path(&path, &raw) {
                Ok((stripped, _witnesses)) => stripped,
                Err(e) => {
                    eprintln!("cruft: ts strip error in {}: {}", path, e);
                    return ExitCode::from(65);
                }
            }
        } else {
            raw
        };

        let abs_path = std::fs::canonicalize(&path)
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|_| path.clone());

        if let Some(parent) = std::path::Path::new(&abs_path).parent() {
            rusty_js_runtime::set_node_compat_entry_dir(parent.to_path_buf());
        }
        let url = format!("file://{}", abs_path);

        let goal_is_module = std::env::var_os("CRUFT_FORCE_SCRIPT").is_none()
            && (path.ends_with(".mjs")
                || path.ends_with(".mts")
                || std::env::var_os("CRUFT_FORCE_MODULE").is_some()
                || matches!(
                    rusty_js_runtime::detect_module_kind(&url),
                    rusty_js_runtime::ModuleKind::ESM
                ));
        let source = if should_wrap_direct_cjs_entry(&path, &source, goal_is_module) {
            wrap_direct_cjs_entry(&source, &abs_path)
        } else {
            source
        };
        (source, url, goal_is_module)
    };

    if args
        .get(entry_idx + 1)
        .map(|arg| arg == "--")
        .unwrap_or(false)
    {
        args.remove(entry_idx + 1);
    }

    let startup_profile = std::env::var("CRUFT_STARTUP_PROFILE").is_ok();
    let startup_t0 = std::time::Instant::now();
    let mut startup_last = startup_t0;
    let mut startup_marks: Vec<(&'static str, u128, u128)> = Vec::new();
    let mut startup_mark = |name: &'static str| {
        if startup_profile {
            let now = std::time::Instant::now();
            startup_marks.push((
                name,
                now.duration_since(startup_last).as_nanos(),
                now.duration_since(startup_t0).as_nanos(),
            ));
            startup_last = now;
        }
    };

    let mut rt = Runtime::new();
    startup_mark("runtime_new");

    if let Some(v) = std::env::var_os("CRUFT_MAIN_AGENT_CAN_BLOCK") {
        rt.main_agent_can_block = !matches!(v.to_str(), Some("0") | Some("false"));
    }
    rt.set_cap_mode(cap_mode);
    if allow_net_loopback {
        rt.caps = std::sync::Arc::new(
            rusty_js_runtime::caps::CapDispatcher::new(cap_mode)
                .with_net_grant(rusty_js_runtime::caps::Net::loopback_server()),
        );
    }
    startup_mark("cap_setup");
    rt.install_intrinsics();
    startup_mark("install_intrinsics");
    install_cruft_host(&mut rt, args);
    startup_mark("install_cruft_host");

    if let Some(summary) =
        caps_closure::wire_import_closure_from_lockfile(&mut rt, &entry_url, cap_mode)
    {
        if std::env::var_os("CRUFT_CAPS_VERBOSE").is_some() {
            eprintln!("cruft: {summary}");
        }
    }
    startup_mark("wire_import_closure");

    if let Some(summary) = wire_root_caps_from_config(&mut rt, &entry_url, cap_mode) {
        if std::env::var_os("CRUFT_CAPS_VERBOSE").is_some() {
            eprintln!("cruft: {summary}");
        }
    }
    startup_mark("wire_root_caps");

    for rfile in &require_preloads {
        let abs = std::fs::canonicalize(rfile)
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|_| rfile.clone());
        match std::fs::read_to_string(&abs) {
            Ok(src) => {
                let purl = format!("file://{}", abs);
                if let Err(e) = rt.run_script(&src, &purl) {
                    eprintln!("cruft: preload error in {}: {:?}", rfile, e);
                    return ExitCode::from(70);
                }
            }
            Err(e) => {
                eprintln!("cruft: cannot read {}: {}", rfile, e);
                return ExitCode::from(66);
            }
        }
    }
    if let Some(preload_path) = std::env::var_os("CRUFT_PRELOAD") {
        if let Ok(preload_src) = std::fs::read_to_string(&preload_path) {
            let purl = format!("file://{}", preload_path.to_string_lossy());
            if let Err(e) = rt.run_script(&preload_src, &purl) {
                eprintln!("cruft: preload error: {:?}", e);
                return ExitCode::from(70);
            }
        }
    }
    startup_mark("preload");

    let url = entry_url;

    let eval_result = if goal_is_module {

        rt.pending_parse_goal = Some(true);
        rt.evaluate_module(&source, &url).map(|_| ())
    } else {
        rt.run_script(&source, &url).map(|_| ())
    };
    startup_mark("eval_entry");
    match eval_result {
        Ok(()) => {}
        Err(e) => {
            if let rusty_js_runtime::RuntimeError::Thrown(v) = &e {
                if let Some(code) = dispatch_uncaught_exception(&mut rt, v) {
                    return code;
                }
                if is_node_assertion_error(&rt, v) {
                    eprintln!("{}", thrown_stack_or_format(&rt, v));
                    return ExitCode::from(1);
                }
            }

            if let rusty_js_runtime::RuntimeError::AsyncHookFatal(v) = &e {
                eprintln!("{}", async_hook_fatal_format(&rt, v));
                return ExitCode::from(1);
            }
            let render_eval_error = |mode: DiagnosticDisclosureMode| {
                use rusty_js_runtime::RuntimeError as RE;
                match &e {
                    RE::Thrown(v) => thrown_stack_or_format_with_mode(&rt, v, mode),
                    RE::CompileError(m) | RE::SyntaxError(m) => format!("SyntaxError: {}", m),
                    RE::ReferenceError(m) => format!("ReferenceError: {}", m),
                    RE::TypeError(m) => {
                        if let Some(node_msg) = m.strip_prefix("__node_resolve_error__:") {
                            let code = if node_msg.starts_with("No \"exports\" main defined")
                                || (node_msg.starts_with("Package subpath '")
                                    && node_msg.contains("is not defined by \"exports\""))
                            {
                                "ERR_PACKAGE_PATH_NOT_EXPORTED"
                            } else if node_msg.starts_with("Directory import '") {
                                "ERR_UNSUPPORTED_DIR_IMPORT"
                            } else {
                                "ERR_MODULE_NOT_FOUND"
                            };
                            format!("Error [{}]: {}", code, node_msg)
                        } else {
                            format!("TypeError: {}", mode.redact_type_error(m))
                        }
                    }
                    RE::RangeError(m) => format!("RangeError: {}", m),
                    _ => format!("{:?}", e),
                }
            };
            let msg = render_eval_error(DiagnosticDisclosureMode::current());
            let structural_msg = render_eval_error(DiagnosticDisclosureMode::Structural);
            write_diagnostic_artifact(
                diagnostic_log_path.as_deref(),
                "evaluation-error",
                &url,
                &msg,
                &structural_msg,
            );

            {
                use rusty_js_runtime::RuntimeError as RE;
                let worker_err_msg = match &e {
                    RE::Thrown(rusty_js_runtime::Value::Object(id)) => {
                        match rt.object_get(*id, "message") {
                            rusty_js_runtime::Value::String(s) => s.as_str().to_string(),
                            _ => msg.clone(),
                        }
                    }
                    RE::Thrown(rusty_js_runtime::Value::String(s)) => s.as_str().to_string(),
                    _ => msg.clone(),
                };
                cruft::ipc::report_worker_error(&mut rt, &worker_err_msg);
            }

            let tt = if rt.timetravel {
                format!(" at op #{}", rt.op_index)
            } else {
                String::new()
            };

            if std::env::var("CRUFT_WORKER").is_err() {
                eprintln!("cruft: evaluation error: {}{}", msg, tt);
            }
            if goal_is_module
                && (msg.starts_with("TypeError: __node_resolve_error__:")
                    || msg.starts_with("Error [ERR_MODULE_NOT_FOUND]:")
                    || msg.starts_with("Error [ERR_PACKAGE_PATH_NOT_EXPORTED]:")
                    || msg.starts_with("Error [ERR_UNSUPPORTED_DIR_IMPORT]:")
                    || msg.starts_with("Error [ERR_MODULE_NOT_FOUND]: Cannot find module '")
                    || msg.starts_with("Error: Directory import "))
            {
                return ExitCode::from(1);
            }

            return ExitCode::from(1);
        }
    }

    for warning in rusty_js_runtime::interp::drain_pending_node_warnings() {
        eprintln!("{}", warning);
    }

    let t_loop = if std::env::var("CRUFT_PROFILE_MODULE").is_ok()
        || std::env::var("CRUFT_PROFILE").is_ok()
    {
        Some(std::time::Instant::now())
    } else {
        None
    };
    if let Err(e) = drive_main_agent(&mut rt) {
        if let rusty_js_runtime::RuntimeError::AsyncHookFatal(v) = &e {
            eprintln!("{}", async_hook_fatal_format(&rt, v));
            return ExitCode::from(1);
        }

        if let rusty_js_runtime::RuntimeError::Thrown(v) = &e {
            if let Some(code) = dispatch_uncaught_exception(&mut rt, v) {
                return code;
            }
        }
        if let Some(code) = dispatch_uncaught_runtime_error(&mut rt, &e) {
            return code;
        }

        let msg = match &e {
            rusty_js_runtime::RuntimeError::Thrown(v) => {
                thrown_stack_or_format_with_mode(&rt, v, DiagnosticDisclosureMode::current())
            }
            other => format!("{:?}", other),
        };
        let structural_msg = match &e {
            rusty_js_runtime::RuntimeError::Thrown(v) => {
                thrown_stack_or_format_with_mode(&rt, v, DiagnosticDisclosureMode::Structural)
            }
            other => format!("{:?}", other),
        };
        write_diagnostic_artifact(
            diagnostic_log_path.as_deref(),
            "event-loop-error",
            &url,
            &msg,
            &structural_msg,
        );
        eprintln!("cruft: event-loop error: {}", msg);

        return ExitCode::from(1);
    }
    startup_mark("run_to_completion");
    if startup_profile {
        let ms = |n: u128| (n as f64) / 1.0e6;
        for (name, delta, total) in startup_marks {
            eprintln!(
                "cruft-startup-profile: phase={} delta_ms={:.3} total_ms={:.3}",
                name,
                ms(delta),
                ms(total),
            );
        }
    }
    if let Some(t) = t_loop {
        use rusty_js_bytecode::compile_profile as cp;
        use rusty_js_runtime::module::phase_profile as pp;
        let loop_ns = t.elapsed().as_nanos() as u64;
        let parse = pp::read(&pp::PARSE_NS);
        let compile = pp::read(&pp::COMPILE_NS);
        let eval = pp::read(&pp::EVAL_NS);
        let cjs_body_call = pp::read(&pp::CJS_BODY_CALL_NS);
        let cjs_body_call_count = pp::read(&pp::CJS_BODY_CALL_COUNT);
        let resolve = pp::read(&pp::RESOLVE_NS);
        let resolve_count = pp::read(&pp::RESOLVE_COUNT);
        let resolve_cache_hits = pp::read(&pp::RESOLVE_CACHE_HITS);
        let resolve_cache_misses = pp::read(&pp::RESOLVE_CACHE_MISSES);
        let cjs_require_calls = pp::read(&pp::CJS_REQUIRE_CALLS);
        let cjs_require_builtin = pp::read(&pp::CJS_REQUIRE_BUILTIN_NS);
        let cjs_require_cache = pp::read(&pp::CJS_REQUIRE_CACHE_NS);
        let cjs_require_cache_hits = pp::read(&pp::CJS_REQUIRE_CACHE_HITS);
        let cjs_require_load = pp::read(&pp::CJS_REQUIRE_LOAD_NS);
        let cjs_require_load_exclusive = pp::read(&pp::CJS_REQUIRE_LOAD_EXCLUSIVE_NS);
        let cjs_require_load_calls = pp::read(&pp::CJS_REQUIRE_LOAD_CALLS);
        let cjs_require_export = pp::read(&pp::CJS_REQUIRE_EXPORT_NS);
        let cjs_require_arg = pp::read(&pp::CJS_REQUIRE_ARG_NS);
        let cjs_require_caps = pp::read(&pp::CJS_REQUIRE_CAPS_NS);
        let cjs_require_native_total = pp::read(&pp::CJS_REQUIRE_NATIVE_TOTAL_NS);
        let cjs_require_native_residual = pp::read(&pp::CJS_REQUIRE_NATIVE_RESIDUAL_NS);
        let cjs_require_closure = pp::read(&pp::CJS_REQUIRE_CLOSURE_NS);
        let cjs_require_stack_shadow = pp::read(&pp::CJS_REQUIRE_STACK_SHADOW_NS);
        let cjs_require_inner = pp::read(&pp::CJS_REQUIRE_INNER_NS);
        let cjs_require_resolve = pp::read(&pp::CJS_REQUIRE_RESOLVE_NS);
        let cjs_evaluate_calls = pp::read(&pp::CJS_EVALUATE_CALLS);
        let cjs_wrapper_setup = pp::read(&pp::CJS_WRAPPER_SETUP_NS);
        let cjs_wrapper_parse = pp::read(&pp::CJS_WRAPPER_PARSE_NS);
        let cjs_wrapper_static_export = pp::read(&pp::CJS_WRAPPER_STATIC_EXPORT_NS);
        let cjs_wrapper_compile = pp::read(&pp::CJS_WRAPPER_COMPILE_NS);
        let cjs_wrapper_module_eval = pp::read(&pp::CJS_WRAPPER_MODULE_EVAL_NS);
        let cjs_wrapper_body_exclusive = pp::read(&pp::CJS_WRAPPER_BODY_EXCLUSIVE_NS);
        let cjs_wrapper_post_body = pp::read(&pp::CJS_WRAPPER_POST_BODY_NS);
        let parsed_compile_calls = cp::read(&cp::PARSED_MODULE_CALLS);
        let parsed_compile_line_starts = cp::read(&cp::PARSED_MODULE_LINE_STARTS_NS);
        let parsed_compile_source_text = cp::read(&cp::PARSED_MODULE_SOURCE_TEXT_NS);
        let parsed_compile_source_url = cp::read(&cp::PARSED_MODULE_SOURCE_URL_NS);
        let parsed_compile_config = cp::read(&cp::PARSED_MODULE_CONFIG_NS);
        let parsed_compile_lower = cp::read(&cp::PARSED_MODULE_LOWER_NS);
        let compile_module_calls = cp::read(&cp::COMPILE_MODULE_CALLS);
        let compile_module_strict = cp::read(&cp::COMPILE_MODULE_STRICT_NS);
        let compile_module_eval_imports = cp::read(&cp::COMPILE_MODULE_EVAL_IMPORTS_NS);
        let compile_module_prealloc = cp::read(&cp::COMPILE_MODULE_PREALLOC_NS);
        let compile_module_hoist = cp::read(&cp::COMPILE_MODULE_HOIST_NS);
        let compile_module_body = cp::read(&cp::COMPILE_MODULE_BODY_NS);
        let compile_module_exports = cp::read(&cp::COMPILE_MODULE_EXPORTS_NS);
        let compile_module_assemble = cp::read(&cp::COMPILE_MODULE_ASSEMBLE_NS);
        let export_default_expr = cp::read(&cp::EXPORT_DEFAULT_EXPR_NS);
        let name_hint_function_proto = cp::read(&cp::NAME_HINT_FUNCTION_PROTO_NS);
        let name_hint_function_intern_emit = cp::read(&cp::NAME_HINT_FUNCTION_INTERN_EMIT_NS);
        let function_proto_calls = cp::read(&cp::FUNCTION_PROTO_CALLS);
        let function_proto_setup = cp::read(&cp::FUNCTION_PROTO_SETUP_NS);
        let function_proto_params = cp::read(&cp::FUNCTION_PROTO_PARAMS_NS);
        let function_proto_prealloc = cp::read(&cp::FUNCTION_PROTO_PREALLOC_NS);
        let function_proto_hoist = cp::read(&cp::FUNCTION_PROTO_HOIST_NS);
        let function_proto_body = cp::read(&cp::FUNCTION_PROTO_BODY_NS);
        let function_proto_body_function = cp::read(&cp::FUNCTION_PROTO_BODY_FUNCTION_NS);
        let function_proto_body_function_calls = cp::read(&cp::FUNCTION_PROTO_BODY_FUNCTION_CALLS);
        let function_proto_body_arrow = cp::read(&cp::FUNCTION_PROTO_BODY_ARROW_NS);
        let function_proto_body_arrow_calls = cp::read(&cp::FUNCTION_PROTO_BODY_ARROW_CALLS);
        let function_proto_body_method = cp::read(&cp::FUNCTION_PROTO_BODY_METHOD_NS);
        let function_proto_body_method_calls = cp::read(&cp::FUNCTION_PROTO_BODY_METHOD_CALLS);
        let function_proto_assemble = cp::read(&cp::FUNCTION_PROTO_ASSEMBLE_NS);
        let preflight_cache_hits = pp::read(&pp::PREFLIGHT_CACHE_HITS);
        let preflight_cache_misses = pp::read(&pp::PREFLIGHT_CACHE_MISSES);
        let read = pp::read(&pp::READ_NS);
        let preflight = pp::read(&pp::PREFLIGHT_NS);
        let preflight_classify = pp::read(&pp::PREFLIGHT_CLASSIFY_NS);
        let preflight_parse = pp::read(&pp::PREFLIGHT_PARSE_NS);
        let preflight_named_export_validation = pp::read(&pp::PREFLIGHT_NAMED_EXPORT_VALIDATION_NS);
        let export_name_collection_calls = pp::read(&pp::EXPORT_NAME_COLLECTION_CALLS);
        let export_name_collection_read = pp::read(&pp::EXPORT_NAME_COLLECTION_READ_NS);
        let export_name_collection_parse = pp::read(&pp::EXPORT_NAME_COLLECTION_PARSE_NS);
        let export_name_collection_resolve = pp::read(&pp::EXPORT_NAME_COLLECTION_RESOLVE_NS);
        let export_name_collection_star_edges = pp::read(&pp::EXPORT_NAME_COLLECTION_STAR_EDGES);
        let export_name_collection_cjs_load = pp::read(&pp::EXPORT_NAME_COLLECTION_CJS_LOAD_NS);
        let export_name_collection_cjs_keys = pp::read(&pp::EXPORT_NAME_COLLECTION_CJS_KEYS_NS);
        let static_deps = pp::read(&pp::STATIC_DEPS_NS);
        let static_dep_resolve = pp::read(&pp::STATIC_DEP_RESOLVE_NS);
        let static_dep_load = pp::read(&pp::STATIC_DEP_LOAD_NS);
        let static_dep_load_exclusive = pp::read(&pp::STATIC_DEP_LOAD_EXCLUSIVE_NS);
        let static_dep_post_load = pp::read(&pp::STATIC_DEP_POST_LOAD_NS);
        let static_dep_edges = pp::read(&pp::STATIC_DEP_EDGES);
        let static_dep_import_edges = pp::read(&pp::STATIC_DEP_IMPORT_EDGES);
        let static_dep_reexport_edges = pp::read(&pp::STATIC_DEP_REEXPORT_EDGES);
        let static_dep_load_calls = pp::read(&pp::STATIC_DEP_LOAD_CALLS);
        let static_dep_load_new = pp::read(&pp::STATIC_DEP_LOAD_NEW);
        let static_dep_load_existing_linking = pp::read(&pp::STATIC_DEP_LOAD_EXISTING_LINKING);
        let static_dep_load_existing_evaluating =
            pp::read(&pp::STATIC_DEP_LOAD_EXISTING_EVALUATING);
        let static_dep_load_existing_evaluated = pp::read(&pp::STATIC_DEP_LOAD_EXISTING_EVALUATED);
        let static_dep_load_existing_failed = pp::read(&pp::STATIC_DEP_LOAD_EXISTING_FAILED);
        let static_dep_load_existing_other = pp::read(&pp::STATIC_DEP_LOAD_EXISTING_OTHER);
        let static_dep_visited_before = pp::read(&pp::STATIC_DEP_VISITED_BEFORE);
        let static_dep_cycle_collapses = pp::read(&pp::STATIC_DEP_CYCLE_COLLAPSES);
        let static_dep_wait_checks = pp::read(&pp::STATIC_DEP_WAIT_CHECKS);
        let static_dep_wait_pushes = pp::read(&pp::STATIC_DEP_WAIT_PUSHES);
        let import_bindings = pp::read(&pp::IMPORT_BINDINGS_NS);
        let export_cells = pp::read(&pp::EXPORT_CELLS_NS);
        let namespace = pp::read(&pp::NAMESPACE_NS);
        let modules = pp::read(&pp::MODULE_COUNT);
        let ms = |n: u64| (n as f64) / 1.0e6;
        eprintln!(
            "cruft-profile: modules={} parse={:.1}ms compile={:.1}ms eval={:.1}ms cjs_body_call={:.1}ms cjs_body_call_count={} resolve={:.1}ms resolve_count={} resolve_cache_hits={} resolve_cache_misses={} cjs_require_calls={} cjs_require_arg={:.1}ms cjs_require_caps={:.1}ms cjs_require_native_total={:.1}ms cjs_require_native_residual={:.1}ms cjs_require_closure={:.1}ms cjs_require_stack_shadow={:.1}ms cjs_require_inner={:.1}ms cjs_require_builtin={:.1}ms cjs_require_resolve={:.1}ms cjs_require_cache={:.1}ms cjs_require_cache_hits={} cjs_require_load={:.1}ms cjs_require_load_exclusive={:.1}ms cjs_require_load_calls={} cjs_require_export={:.1}ms cjs_evaluate_calls={} cjs_wrapper_setup={:.1}ms cjs_wrapper_parse={:.1}ms cjs_wrapper_static_export={:.1}ms cjs_wrapper_compile={:.1}ms cjs_wrapper_module_eval={:.1}ms cjs_wrapper_body_exclusive={:.1}ms cjs_wrapper_post_body={:.1}ms parsed_compile_calls={} parsed_compile_line_starts={:.1}ms parsed_compile_source_text={:.1}ms parsed_compile_source_url={:.1}ms parsed_compile_config={:.1}ms parsed_compile_lower={:.1}ms compile_module_calls={} compile_module_strict={:.1}ms compile_module_eval_imports={:.1}ms compile_module_prealloc={:.1}ms compile_module_hoist={:.1}ms compile_module_body={:.1}ms compile_module_exports={:.1}ms compile_module_assemble={:.1}ms export_default_expr={:.1}ms name_hint_function_proto={:.1}ms name_hint_function_intern_emit={:.1}ms function_proto_calls={} function_proto_setup={:.1}ms function_proto_params={:.1}ms function_proto_prealloc={:.1}ms function_proto_hoist={:.1}ms function_proto_body={:.1}ms function_proto_assemble={:.1}ms preflight_cache_hits={} preflight_cache_misses={} read={:.1}ms preflight={:.1}ms preflight_classify={:.1}ms preflight_parse={:.1}ms preflight_named_export_validation={:.1}ms export_name_collection_calls={} export_name_collection_read={:.1}ms export_name_collection_parse={:.1}ms export_name_collection_resolve={:.1}ms export_name_collection_star_edges={} export_name_collection_cjs_load={:.1}ms export_name_collection_cjs_keys={:.1}ms static_deps={:.1}ms static_dep_resolve={:.1}ms static_dep_load={:.1}ms static_dep_load_exclusive={:.1}ms static_dep_post_load={:.1}ms static_dep_edges={} static_dep_import_edges={} static_dep_reexport_edges={} static_dep_load_calls={} static_dep_load_new={} static_dep_load_existing_linking={} static_dep_load_existing_evaluating={} static_dep_load_existing_evaluated={} static_dep_load_existing_failed={} static_dep_load_existing_other={} static_dep_visited_before={} static_dep_cycle_collapses={} static_dep_wait_checks={} static_dep_wait_pushes={} import_bindings={:.1}ms export_cells={:.1}ms namespace={:.1}ms event_loop={:.1}ms total_phases={:.1}ms",
            modules,
            ms(parse),
            ms(compile),
            ms(eval),
            ms(cjs_body_call),
            cjs_body_call_count,
            ms(resolve),
            resolve_count,
            resolve_cache_hits,
            resolve_cache_misses,
            cjs_require_calls,
            ms(cjs_require_arg),
            ms(cjs_require_caps),
            ms(cjs_require_native_total),
            ms(cjs_require_native_residual),
            ms(cjs_require_closure),
            ms(cjs_require_stack_shadow),
            ms(cjs_require_inner),
            ms(cjs_require_builtin),
            ms(cjs_require_resolve),
            ms(cjs_require_cache),
            cjs_require_cache_hits,
            ms(cjs_require_load),
            ms(cjs_require_load_exclusive),
            cjs_require_load_calls,
            ms(cjs_require_export),
            cjs_evaluate_calls,
            ms(cjs_wrapper_setup),
            ms(cjs_wrapper_parse),
            ms(cjs_wrapper_static_export),
            ms(cjs_wrapper_compile),
            ms(cjs_wrapper_module_eval),
            ms(cjs_wrapper_body_exclusive),
            ms(cjs_wrapper_post_body),
            parsed_compile_calls,
            ms(parsed_compile_line_starts),
            ms(parsed_compile_source_text),
            ms(parsed_compile_source_url),
            ms(parsed_compile_config),
            ms(parsed_compile_lower),
            compile_module_calls,
            ms(compile_module_strict),
            ms(compile_module_eval_imports),
            ms(compile_module_prealloc),
            ms(compile_module_hoist),
            ms(compile_module_body),
            ms(compile_module_exports),
            ms(compile_module_assemble),
            ms(export_default_expr),
            ms(name_hint_function_proto),
            ms(name_hint_function_intern_emit),
            function_proto_calls,
            ms(function_proto_setup),
            ms(function_proto_params),
            ms(function_proto_prealloc),
            ms(function_proto_hoist),
            ms(function_proto_body),
            ms(function_proto_assemble),
            preflight_cache_hits,
            preflight_cache_misses,
            ms(read),
            ms(preflight),
            ms(preflight_classify),
            ms(preflight_parse),
            ms(preflight_named_export_validation),
            export_name_collection_calls,
            ms(export_name_collection_read),
            ms(export_name_collection_parse),
            ms(export_name_collection_resolve),
            export_name_collection_star_edges,
            ms(export_name_collection_cjs_load),
            ms(export_name_collection_cjs_keys),
            ms(static_deps),
            ms(static_dep_resolve),
            ms(static_dep_load),
            ms(static_dep_load_exclusive),
            ms(static_dep_post_load),
            static_dep_edges,
            static_dep_import_edges,
            static_dep_reexport_edges,
            static_dep_load_calls,
            static_dep_load_new,
            static_dep_load_existing_linking,
            static_dep_load_existing_evaluating,
            static_dep_load_existing_evaluated,
            static_dep_load_existing_failed,
            static_dep_load_existing_other,
            static_dep_visited_before,
            static_dep_cycle_collapses,
            static_dep_wait_checks,
            static_dep_wait_pushes,
            ms(import_bindings),
            ms(export_cells),
            ms(namespace),
            ms(loop_ns),
            ms(parse + compile + eval + resolve + read + preflight + static_deps + import_bindings + export_cells + namespace + loop_ns),
        );
        if std::env::var("CRUFT_PROFILE_MODULE").is_ok() && std::env::var("CRUFT_PROFILE").is_err()
        {
            if std::env::var("CRUFT_PROFILE_OPS").is_ok() {
                for (op, count, ns) in rusty_js_runtime::interp::interp_op_profile_summary(12) {
                    let avg_ns = if count == 0 { 0 } else { ns / count };
                    eprintln!(
                        "cruft-profile-op: op={} count={} total={:.1}ms avg={}ns",
                        op,
                        count,
                        ms(ns),
                        avg_ns
                    );
                }
            }
            if std::env::var("CRUFT_PROFILE_OPS_BY_FRAME").is_ok() {
                for (frame, op, count, ns) in
                    rusty_js_runtime::interp::interp_op_frame_profile_summary(20)
                {
                    let avg_ns = if count == 0 { 0 } else { ns / count };
                    eprintln!(
                        "cruft-profile-op-frame: frame={} op={} count={} total={:.1}ms avg={}ns",
                        frame,
                        op,
                        count,
                        ms(ns),
                        avg_ns
                    );
                }
            }
            if std::env::var("CRUFT_PROFILE_LOADLOCAL_FRAME").is_ok() {
                for (frame, slot, kind, count) in
                    rusty_js_runtime::interp::loadlocal_frame_profile_summary(30)
                {
                    eprintln!(
                        "cruft-profile-loadlocal-frame: frame={} slot={} kind={} count={}",
                        frame, slot, kind, count
                    );
                }
            }
            if std::env::var("CRUFT_PROFILE_IHI").is_ok() {
                for (entry, count, ns) in rusty_js_runtime::interp::interp_ihi_profile_summary(12) {
                    let avg_ns = if count == 0 { 0 } else { ns / count };
                    eprintln!(
                        "cruft-profile-ihi: entry={} hits={} total={:.1}ms avg={}ns",
                        entry,
                        count,
                        ms(ns),
                        avg_ns
                    );
                }
            }
            if let Some((
                total,
                array_direct,
                object_other,
                string_primitive,
                primitive_other,
                nullish,
            )) = rusty_js_runtime::interp::array_length_getprop_profile_summary()
            {
                eprintln!(
                    "cruft-profile-array-length-getprop: total={} array_direct={} object_other={} string_primitive={} primitive_other={} nullish={}",
                    total, array_direct, object_other, string_primitive, primitive_other, nullish
                );
            }
            if let Some((
                (
                    get_total,
                    get_array_dense,
                    get_array_other,
                    get_typed_array,
                    get_object_other,
                    get_string_primitive,
                    get_primitive_other,
                    get_nullish,
                ),
                (
                    set_total,
                    set_array_dense,
                    set_array_other,
                    set_typed_array,
                    set_object_other,
                    set_primitive,
                    set_nullish,
                ),
            )) = rusty_js_runtime::interp::index_op_family_profile_summary()
            {
                eprintln!(
                    "cruft-profile-index-op-family: get_total={} get_array_dense={} get_array_other={} get_typed_array={} get_object_other={} get_string_primitive={} get_primitive_other={} get_nullish={} set_total={} set_array_dense={} set_array_other={} set_typed_array={} set_object_other={} set_primitive={} set_nullish={}",
                    get_total, get_array_dense, get_array_other, get_typed_array, get_object_other, get_string_primitive, get_primitive_other, get_nullish, set_total, set_array_dense, set_array_other, set_typed_array, set_object_other, set_primitive, set_nullish
                );
            }
            if let Some((
                total,
                object,
                proxy,
                typed_array,
                module_namespace,
                chain_proxy,
                ta_chain,
                setter,
                accessor_no_setter,
                own_nonwritable,
                inherited_nonwritable,
                object_set_existing,
                object_set_new,
                object_set_new_shaped,
                non_extensible,
                primitive,
            )) = rusty_js_runtime::interp::setindex_phase_profile_summary()
            {
                eprintln!(
                    "cruft-profile-setindex-phases: total={} object={} proxy={} typed_array={} module_namespace={} chain_proxy={} ta_chain={} setter={} accessor_no_setter={} own_nonwritable={} inherited_nonwritable={} object_set_existing={} object_set_new={} object_set_new_shaped={} non_extensible={} primitive={}",
                    total, object, proxy, typed_array, module_namespace, chain_proxy, ta_chain, setter, accessor_no_setter, own_nonwritable, inherited_nonwritable, object_set_existing, object_set_new, object_set_new_shaped, non_extensible, primitive
                );
            }
            if let Some((
                total,
                total_ns,
                to_property_key_ns,
                proxy_check_ns,
                chain_proxy_ns,
                ta_chain_ns,
                setter_lookup_ns,
                accessor_lookup_ns,
                nonwritable_lookup_ns,
                object_set_ns,
                other_ns,
            )) = rusty_js_runtime::interp::setindex_timing_profile_summary()
            {
                let avg_ns = if total == 0 { 0 } else { total_ns / total };
                eprintln!(
                    "cruft-profile-setindex-timing: total={} total={:.1}ms avg={}ns to_property_key={:.1}ms proxy_check={:.1}ms chain_proxy={:.1}ms ta_chain={:.1}ms setter_lookup={:.1}ms accessor_lookup={:.1}ms nonwritable_lookup={:.1}ms object_set={:.1}ms other={:.1}ms",
                    total, ms(total_ns), avg_ns, ms(to_property_key_ns), ms(proxy_check_ns), ms(chain_proxy_ns), ms(ta_chain_ns), ms(setter_lookup_ns), ms(accessor_lookup_ns), ms(nonwritable_lookup_ns), ms(object_set_ns), ms(other_ns)
                );
            }
            if std::env::var("CRUFT_PROFILE_CALLS").is_ok() {
                for (name, calls, iterations, roots_ns, inner_ns, self_ns, total_ns) in
                    rusty_js_runtime::interp::interp_call_profile_summary(12)
                {
                    let avg_ns = if calls == 0 { 0 } else { total_ns / calls };
                    eprintln!(
                        "cruft-profile-call: callee={} calls={} iterations={} roots={:.1}ms inner={:.1}ms self={:.1}ms total={:.1}ms avg={}ns",
                        name,
                        calls,
                        iterations,
                        ms(roots_ns),
                        ms(inner_ns),
                        ms(self_ns),
                        ms(total_ns),
                        avg_ns
                    );
                }
            }
            return ExitCode::SUCCESS;
        }
        eprintln!(
            "cruft-profile-stmt: expression={}:{:.1}ms return={}:{:.1}ms block={}:{:.1}ms variable={}:{:.1}ms throw={}:{:.1}ms if={}:{:.1}ms loop={}:{:.1}ms try={}:{:.1}ms switch={}:{:.1}ms decl={}:{:.1}ms control={}:{:.1}ms other={}:{:.1}ms",
            cp::read(&cp::STMT_EXPRESSION_CALLS),
            ms(cp::read(&cp::STMT_EXPRESSION_NS)),
            cp::read(&cp::STMT_RETURN_CALLS),
            ms(cp::read(&cp::STMT_RETURN_NS)),
            cp::read(&cp::STMT_BLOCK_CALLS),
            ms(cp::read(&cp::STMT_BLOCK_NS)),
            cp::read(&cp::STMT_VARIABLE_CALLS),
            ms(cp::read(&cp::STMT_VARIABLE_NS)),
            cp::read(&cp::STMT_THROW_CALLS),
            ms(cp::read(&cp::STMT_THROW_NS)),
            cp::read(&cp::STMT_IF_CALLS),
            ms(cp::read(&cp::STMT_IF_NS)),
            cp::read(&cp::STMT_LOOP_CALLS),
            ms(cp::read(&cp::STMT_LOOP_NS)),
            cp::read(&cp::STMT_TRY_CALLS),
            ms(cp::read(&cp::STMT_TRY_NS)),
            cp::read(&cp::STMT_SWITCH_CALLS),
            ms(cp::read(&cp::STMT_SWITCH_NS)),
            cp::read(&cp::STMT_DECL_CALLS),
            ms(cp::read(&cp::STMT_DECL_NS)),
            cp::read(&cp::STMT_CONTROL_CALLS),
            ms(cp::read(&cp::STMT_CONTROL_NS)),
            cp::read(&cp::STMT_OTHER_CALLS),
            ms(cp::read(&cp::STMT_OTHER_NS)),
        );
        eprintln!(
            "cruft-profile-var: decl_iter={}:{:.1}ms target_slot={}:{:.1}ms init_expr={}:{:.1}ms store={}:{:.1}ms destructure={}:{:.1}ms script_global_mirror={}:{:.1}ms",
            cp::read(&cp::VAR_DECL_ITER_CALLS),
            ms(cp::read(&cp::VAR_DECL_ITER_NS)),
            cp::read(&cp::VAR_TARGET_SLOT_CALLS),
            ms(cp::read(&cp::VAR_TARGET_SLOT_NS)),
            cp::read(&cp::VAR_INIT_EXPR_CALLS),
            ms(cp::read(&cp::VAR_INIT_EXPR_NS)),
            cp::read(&cp::VAR_STORE_CALLS),
            ms(cp::read(&cp::VAR_STORE_NS)),
            cp::read(&cp::VAR_DESTRUCTURE_CALLS),
            ms(cp::read(&cp::VAR_DESTRUCTURE_NS)),
            cp::read(&cp::VAR_SCRIPT_GLOBAL_MIRROR_CALLS),
            ms(cp::read(&cp::VAR_SCRIPT_GLOBAL_MIRROR_NS)),
        );
        eprintln!(
            "cruft-profile-hint-expr: function={}:{:.1}ms arrow={}:{:.1}ms class={}:{:.1}ms paren={}:{:.1}ms literal={}:{:.1}ms identifier={}:{:.1}ms member={}:{:.1}ms call={}:{:.1}ms object_array={}:{:.1}ms operator={}:{:.1}ms other={}:{:.1}ms",
            cp::read(&cp::HINT_EXPR_FUNCTION_CALLS),
            ms(cp::read(&cp::HINT_EXPR_FUNCTION_NS)),
            cp::read(&cp::HINT_EXPR_ARROW_CALLS),
            ms(cp::read(&cp::HINT_EXPR_ARROW_NS)),
            cp::read(&cp::HINT_EXPR_CLASS_CALLS),
            ms(cp::read(&cp::HINT_EXPR_CLASS_NS)),
            cp::read(&cp::HINT_EXPR_PAREN_CALLS),
            ms(cp::read(&cp::HINT_EXPR_PAREN_NS)),
            cp::read(&cp::HINT_EXPR_LITERAL_CALLS),
            ms(cp::read(&cp::HINT_EXPR_LITERAL_NS)),
            cp::read(&cp::HINT_EXPR_IDENTIFIER_CALLS),
            ms(cp::read(&cp::HINT_EXPR_IDENTIFIER_NS)),
            cp::read(&cp::HINT_EXPR_MEMBER_CALLS),
            ms(cp::read(&cp::HINT_EXPR_MEMBER_NS)),
            cp::read(&cp::HINT_EXPR_CALL_CALLS),
            ms(cp::read(&cp::HINT_EXPR_CALL_NS)),
            cp::read(&cp::HINT_EXPR_OBJECT_ARRAY_CALLS),
            ms(cp::read(&cp::HINT_EXPR_OBJECT_ARRAY_NS)),
            cp::read(&cp::HINT_EXPR_OPERATOR_CALLS),
            ms(cp::read(&cp::HINT_EXPR_OPERATOR_NS)),
            cp::read(&cp::HINT_EXPR_OTHER_CALLS),
            ms(cp::read(&cp::HINT_EXPR_OTHER_NS)),
        );
        eprintln!(
            "cruft-profile-var-init: function={}:{:.1}ms arrow={}:{:.1}ms class={}:{:.1}ms paren={}:{:.1}ms literal={}:{:.1}ms identifier={}:{:.1}ms member={}:{:.1}ms call={}:{:.1}ms object_array={}:{:.1}ms operator={}:{:.1}ms other={}:{:.1}ms",
            cp::read(&cp::VAR_INIT_FUNCTION_CALLS),
            ms(cp::read(&cp::VAR_INIT_FUNCTION_NS)),
            cp::read(&cp::VAR_INIT_ARROW_CALLS),
            ms(cp::read(&cp::VAR_INIT_ARROW_NS)),
            cp::read(&cp::VAR_INIT_CLASS_CALLS),
            ms(cp::read(&cp::VAR_INIT_CLASS_NS)),
            cp::read(&cp::VAR_INIT_PAREN_CALLS),
            ms(cp::read(&cp::VAR_INIT_PAREN_NS)),
            cp::read(&cp::VAR_INIT_LITERAL_CALLS),
            ms(cp::read(&cp::VAR_INIT_LITERAL_NS)),
            cp::read(&cp::VAR_INIT_IDENTIFIER_CALLS),
            ms(cp::read(&cp::VAR_INIT_IDENTIFIER_NS)),
            cp::read(&cp::VAR_INIT_MEMBER_CALLS),
            ms(cp::read(&cp::VAR_INIT_MEMBER_NS)),
            cp::read(&cp::VAR_INIT_CALL_CALLS),
            ms(cp::read(&cp::VAR_INIT_CALL_NS)),
            cp::read(&cp::VAR_INIT_OBJECT_ARRAY_CALLS),
            ms(cp::read(&cp::VAR_INIT_OBJECT_ARRAY_NS)),
            cp::read(&cp::VAR_INIT_OPERATOR_CALLS),
            ms(cp::read(&cp::VAR_INIT_OPERATOR_NS)),
            cp::read(&cp::VAR_INIT_OTHER_CALLS),
            ms(cp::read(&cp::VAR_INIT_OTHER_NS)),
        );
        eprintln!(
            "cruft-profile-function-proto-kind: function={}:{:.1}ms arrow={}:{:.1}ms method={}:{:.1}ms",
            function_proto_body_function_calls,
            ms(function_proto_body_function),
            function_proto_body_arrow_calls,
            ms(function_proto_body_arrow),
            function_proto_body_method_calls,
            ms(function_proto_body_method),
        );
        eprintln!(
            "cruft-profile-function-body-stmt: expression={}:{:.1}ms return={}:{:.1}ms block={}:{:.1}ms variable={}:{:.1}ms throw={}:{:.1}ms if={}:{:.1}ms loop={}:{:.1}ms try={}:{:.1}ms switch={}:{:.1}ms decl={}:{:.1}ms control={}:{:.1}ms other={}:{:.1}ms",
            cp::read(&cp::FUNCTION_PROTO_BODY_STMT_EXPRESSION_CALLS),
            ms(cp::read(&cp::FUNCTION_PROTO_BODY_STMT_EXPRESSION_NS)),
            cp::read(&cp::FUNCTION_PROTO_BODY_STMT_RETURN_CALLS),
            ms(cp::read(&cp::FUNCTION_PROTO_BODY_STMT_RETURN_NS)),
            cp::read(&cp::FUNCTION_PROTO_BODY_STMT_BLOCK_CALLS),
            ms(cp::read(&cp::FUNCTION_PROTO_BODY_STMT_BLOCK_NS)),
            cp::read(&cp::FUNCTION_PROTO_BODY_STMT_VARIABLE_CALLS),
            ms(cp::read(&cp::FUNCTION_PROTO_BODY_STMT_VARIABLE_NS)),
            cp::read(&cp::FUNCTION_PROTO_BODY_STMT_THROW_CALLS),
            ms(cp::read(&cp::FUNCTION_PROTO_BODY_STMT_THROW_NS)),
            cp::read(&cp::FUNCTION_PROTO_BODY_STMT_IF_CALLS),
            ms(cp::read(&cp::FUNCTION_PROTO_BODY_STMT_IF_NS)),
            cp::read(&cp::FUNCTION_PROTO_BODY_STMT_LOOP_CALLS),
            ms(cp::read(&cp::FUNCTION_PROTO_BODY_STMT_LOOP_NS)),
            cp::read(&cp::FUNCTION_PROTO_BODY_STMT_TRY_CALLS),
            ms(cp::read(&cp::FUNCTION_PROTO_BODY_STMT_TRY_NS)),
            cp::read(&cp::FUNCTION_PROTO_BODY_STMT_SWITCH_CALLS),
            ms(cp::read(&cp::FUNCTION_PROTO_BODY_STMT_SWITCH_NS)),
            cp::read(&cp::FUNCTION_PROTO_BODY_STMT_DECL_CALLS),
            ms(cp::read(&cp::FUNCTION_PROTO_BODY_STMT_DECL_NS)),
            cp::read(&cp::FUNCTION_PROTO_BODY_STMT_CONTROL_CALLS),
            ms(cp::read(&cp::FUNCTION_PROTO_BODY_STMT_CONTROL_NS)),
            cp::read(&cp::FUNCTION_PROTO_BODY_STMT_OTHER_CALLS),
            ms(cp::read(&cp::FUNCTION_PROTO_BODY_STMT_OTHER_NS)),
        );
        eprintln!(
            "cruft-profile-function-body-stmt-decl: function={}:{:.1}ms class={}:{:.1}ms other={}:{:.1}ms",
            cp::read(&cp::FUNCTION_PROTO_BODY_STMT_DECL_FUNCTION_CALLS),
            ms(cp::read(&cp::FUNCTION_PROTO_BODY_STMT_DECL_FUNCTION_NS)),
            cp::read(&cp::FUNCTION_PROTO_BODY_STMT_DECL_CLASS_CALLS),
            ms(cp::read(&cp::FUNCTION_PROTO_BODY_STMT_DECL_CLASS_NS)),
            cp::read(&cp::FUNCTION_PROTO_BODY_STMT_DECL_OTHER_CALLS),
            ms(cp::read(&cp::FUNCTION_PROTO_BODY_STMT_DECL_OTHER_NS)),
        );
        eprintln!(
            "cruft-profile-function-body-expr-stmt: function={}:{:.1}ms arrow={}:{:.1}ms class={}:{:.1}ms paren={}:{:.1}ms literal={}:{:.1}ms identifier={}:{:.1}ms member={}:{:.1}ms call={}:{:.1}ms object_array={}:{:.1}ms operator={}:{:.1}ms other={}:{:.1}ms",
            cp::read(&cp::FUNCTION_BODY_EXPR_STMT_FUNCTION_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_EXPR_STMT_FUNCTION_NS)),
            cp::read(&cp::FUNCTION_BODY_EXPR_STMT_ARROW_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_EXPR_STMT_ARROW_NS)),
            cp::read(&cp::FUNCTION_BODY_EXPR_STMT_CLASS_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_EXPR_STMT_CLASS_NS)),
            cp::read(&cp::FUNCTION_BODY_EXPR_STMT_PAREN_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_EXPR_STMT_PAREN_NS)),
            cp::read(&cp::FUNCTION_BODY_EXPR_STMT_LITERAL_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_EXPR_STMT_LITERAL_NS)),
            cp::read(&cp::FUNCTION_BODY_EXPR_STMT_IDENTIFIER_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_EXPR_STMT_IDENTIFIER_NS)),
            cp::read(&cp::FUNCTION_BODY_EXPR_STMT_MEMBER_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_EXPR_STMT_MEMBER_NS)),
            cp::read(&cp::FUNCTION_BODY_EXPR_STMT_CALL_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_EXPR_STMT_CALL_NS)),
            cp::read(&cp::FUNCTION_BODY_EXPR_STMT_OBJECT_ARRAY_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_EXPR_STMT_OBJECT_ARRAY_NS)),
            cp::read(&cp::FUNCTION_BODY_EXPR_STMT_OPERATOR_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_EXPR_STMT_OPERATOR_NS)),
            cp::read(&cp::FUNCTION_BODY_EXPR_STMT_OTHER_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_EXPR_STMT_OTHER_NS)),
        );
        eprintln!(
            "cruft-profile-function-body-operator-expr-stmt: unary_update={}:{:.1}ms binary_arith={}:{:.1}ms binary_compare={}:{:.1}ms binary_bitwise={}:{:.1}ms logical={}:{:.1}ms nullish={}:{:.1}ms conditional={}:{:.1}ms assign={}:{:.1}ms sequence={}:{:.1}ms",
            cp::read(&cp::FUNCTION_BODY_EXPR_STMT_OPERATOR_UNARY_UPDATE_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_EXPR_STMT_OPERATOR_UNARY_UPDATE_NS)),
            cp::read(&cp::FUNCTION_BODY_EXPR_STMT_OPERATOR_BINARY_ARITH_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_EXPR_STMT_OPERATOR_BINARY_ARITH_NS)),
            cp::read(&cp::FUNCTION_BODY_EXPR_STMT_OPERATOR_BINARY_COMPARE_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_EXPR_STMT_OPERATOR_BINARY_COMPARE_NS)),
            cp::read(&cp::FUNCTION_BODY_EXPR_STMT_OPERATOR_BINARY_BITWISE_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_EXPR_STMT_OPERATOR_BINARY_BITWISE_NS)),
            cp::read(&cp::FUNCTION_BODY_EXPR_STMT_OPERATOR_LOGICAL_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_EXPR_STMT_OPERATOR_LOGICAL_NS)),
            cp::read(&cp::FUNCTION_BODY_EXPR_STMT_OPERATOR_NULLISH_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_EXPR_STMT_OPERATOR_NULLISH_NS)),
            cp::read(&cp::FUNCTION_BODY_EXPR_STMT_OPERATOR_CONDITIONAL_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_EXPR_STMT_OPERATOR_CONDITIONAL_NS)),
            cp::read(&cp::FUNCTION_BODY_EXPR_STMT_OPERATOR_ASSIGN_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_EXPR_STMT_OPERATOR_ASSIGN_NS)),
            cp::read(&cp::FUNCTION_BODY_EXPR_STMT_OPERATOR_SEQUENCE_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_EXPR_STMT_OPERATOR_SEQUENCE_NS)),
        );
        eprintln!(
            "cruft-profile-function-body-assign-expr-stmt-target: identifier={}:{:.1}ms member={}:{:.1}ms destructure={}:{:.1}ms paren={}:{:.1}ms call_invalid={}:{:.1}ms other={}:{:.1}ms",
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_TARGET_IDENTIFIER_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_TARGET_IDENTIFIER_NS)),
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_TARGET_MEMBER_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_TARGET_MEMBER_NS)),
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_TARGET_DESTRUCTURE_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_TARGET_DESTRUCTURE_NS)),
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_TARGET_PAREN_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_TARGET_PAREN_NS)),
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_TARGET_CALL_INVALID_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_TARGET_CALL_INVALID_NS)),
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_TARGET_OTHER_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_TARGET_OTHER_NS)),
        );
        eprintln!(
            "cruft-profile-function-body-assign-expr-stmt-phase: identifier_value={}:{:.1}ms identifier_store={}:{:.1}ms member_object={}:{:.1}ms member_key={}:{:.1}ms member_value={}:{:.1}ms member_store={}:{:.1}ms destructure_value={}:{:.1}ms destructure_emit={}:{:.1}ms",
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_IDENTIFIER_VALUE_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_IDENTIFIER_VALUE_NS)),
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_IDENTIFIER_STORE_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_IDENTIFIER_STORE_NS)),
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_OBJECT_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_OBJECT_NS)),
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_KEY_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_KEY_NS)),
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_NS)),
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_STORE_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_STORE_NS)),
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_DESTRUCTURE_VALUE_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_DESTRUCTURE_VALUE_NS)),
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_DESTRUCTURE_EMIT_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_DESTRUCTURE_EMIT_NS)),
        );
        eprintln!(
            "cruft-profile-function-body-assign-expr-stmt-member-value: function={}:{:.1}ms arrow={}:{:.1}ms class={}:{:.1}ms paren={}:{:.1}ms literal={}:{:.1}ms identifier={}:{:.1}ms member={}:{:.1}ms call={}:{:.1}ms object_array={}:{:.1}ms operator={}:{:.1}ms other={}:{:.1}ms",
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_NS)),
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_ARROW_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_ARROW_NS)),
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_CLASS_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_CLASS_NS)),
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_PAREN_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_PAREN_NS)),
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_LITERAL_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_LITERAL_NS)),
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_IDENTIFIER_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_IDENTIFIER_NS)),
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_MEMBER_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_MEMBER_NS)),
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_CALL_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_CALL_NS)),
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_OBJECT_ARRAY_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_OBJECT_ARRAY_NS)),
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_OPERATOR_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_OPERATOR_NS)),
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_OTHER_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_OTHER_NS)),
        );
        eprintln!(
            "cruft-profile-function-body-assign-expr-stmt-member-value-function-outer: proto={}:{:.1}ms intern_emit={}:{:.1}ms",
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_OUTER_PROTO_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_OUTER_PROTO_NS)),
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_OUTER_INTERN_EMIT_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_OUTER_INTERN_EMIT_NS)),
        );
        eprintln!(
            "cruft-profile-function-body-assign-expr-stmt-member-value-function-proto: setup={}:{:.1}ms params={}:{:.1}ms prealloc={}:{:.1}ms hoist={}:{:.1}ms body={}:{:.1}ms assemble={}:{:.1}ms",
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_SETUP_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_SETUP_NS)),
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_PARAMS_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_PARAMS_NS)),
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_PREALLOC_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_PREALLOC_NS)),
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_HOIST_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_HOIST_NS)),
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_NS)),
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_ASSEMBLE_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_ASSEMBLE_NS)),
        );
        eprintln!(
            "cruft-profile-function-body-assign-expr-stmt-member-value-function-proto-body-stmt: expression={}:{:.1}ms return={}:{:.1}ms block={}:{:.1}ms variable={}:{:.1}ms throw={}:{:.1}ms if={}:{:.1}ms loop={}:{:.1}ms try={}:{:.1}ms switch={}:{:.1}ms decl={}:{:.1}ms control={}:{:.1}ms other={}:{:.1}ms",
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_EXPRESSION_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_EXPRESSION_NS)),
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_RETURN_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_RETURN_NS)),
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_BLOCK_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_BLOCK_NS)),
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_VARIABLE_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_VARIABLE_NS)),
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_THROW_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_THROW_NS)),
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_NS)),
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_LOOP_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_LOOP_NS)),
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_TRY_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_TRY_NS)),
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_SWITCH_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_SWITCH_NS)),
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_DECL_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_DECL_NS)),
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_CONTROL_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_CONTROL_NS)),
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_OTHER_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_OTHER_NS)),
        );
        eprintln!(
            "cruft-profile-function-body-assign-expr-stmt-member-value-function-proto-body-var: decl_iter={}:{:.1}ms target={}:{:.1}ms init={}:{:.1}ms store={}:{:.1}ms mirror={}:{:.1}ms",
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_VAR_DECL_ITER_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_VAR_DECL_ITER_NS)),
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_VAR_TARGET_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_VAR_TARGET_NS)),
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_VAR_INIT_NS)),
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_VAR_STORE_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_VAR_STORE_NS)),
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_VAR_MIRROR_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_VAR_MIRROR_NS)),
        );
        eprintln!(
            "cruft-profile-function-body-assign-expr-stmt-member-value-function-proto-body-var-init: function={}:{:.1}ms arrow={}:{:.1}ms class={}:{:.1}ms call={}:{:.1}ms object_array={}:{:.1}ms operator={}:{:.1}ms other={}:{:.1}ms",
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_VAR_INIT_FUNCTION_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_VAR_INIT_FUNCTION_NS)),
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_VAR_INIT_ARROW_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_VAR_INIT_ARROW_NS)),
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CLASS_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CLASS_NS)),
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_NS)),
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_VAR_INIT_OBJECT_ARRAY_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_VAR_INIT_OBJECT_ARRAY_NS)),
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_VAR_INIT_OPERATOR_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_VAR_INIT_OPERATOR_NS)),
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_VAR_INIT_OTHER_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_VAR_INIT_OTHER_NS)),
        );
        eprintln!(
            "cruft-profile-function-body-assign-expr-stmt-member-value-function-proto-body-expr: call={}:{:.1}ms operator={}:{:.1}ms other={}:{:.1}ms",
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_EXPR_CALL_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_EXPR_CALL_NS)),
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_EXPR_OPERATOR_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_EXPR_OPERATOR_NS)),
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_EXPR_OTHER_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_EXPR_OTHER_NS)),
        );
        eprintln!(
            "cruft-profile-function-body-assign-expr-stmt-member-value-function-proto-body-if: test={}:{:.1}ms patch={}:{:.1}ms consequent={}:{:.1}ms alternate={}:{:.1}ms",
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_TEST_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_TEST_NS)),
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_PATCH_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_PATCH_NS)),
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_NS)),
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_ALTERNATE_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_ALTERNATE_NS)),
        );
        eprintln!(
            "cruft-profile-function-body-assign-expr-stmt-member-value-function-proto-body-if-clause: consequent_block={}:{:.1}ms consequent_if={}:{:.1}ms consequent_return={}:{:.1}ms consequent_expr={}:{:.1}ms consequent_other={}:{:.1}ms alternate_block={}:{:.1}ms alternate_if={}:{:.1}ms alternate_return={}:{:.1}ms alternate_expr={}:{:.1}ms alternate_other={}:{:.1}ms",
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_CLAUSE_BLOCK_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_CLAUSE_BLOCK_NS)),
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_CLAUSE_IF_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_CLAUSE_IF_NS)),
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_CLAUSE_RETURN_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_CLAUSE_RETURN_NS)),
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_CLAUSE_EXPR_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_CLAUSE_EXPR_NS)),
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_CLAUSE_OTHER_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_CLAUSE_OTHER_NS)),
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_ALTERNATE_CLAUSE_BLOCK_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_ALTERNATE_CLAUSE_BLOCK_NS)),
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_ALTERNATE_CLAUSE_IF_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_ALTERNATE_CLAUSE_IF_NS)),
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_ALTERNATE_CLAUSE_RETURN_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_ALTERNATE_CLAUSE_RETURN_NS)),
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_ALTERNATE_CLAUSE_EXPR_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_ALTERNATE_CLAUSE_EXPR_NS)),
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_ALTERNATE_CLAUSE_OTHER_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_ALTERNATE_CLAUSE_OTHER_NS)),
        );
        eprintln!(
            "cruft-profile-function-body-assign-expr-stmt-member-value-function-proto-body-if-consequent-block: prescan={}:{:.1}ms reset_seed={}:{:.1}ms prebind={}:{:.1}ms body={}:{:.1}ms rename={}:{:.1}ms",
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_BLOCK_PRESCAN_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_BLOCK_PRESCAN_NS)),
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_BLOCK_RESET_SEED_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_BLOCK_RESET_SEED_NS)),
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_BLOCK_PREBIND_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_BLOCK_PREBIND_NS)),
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_NS)),
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_BLOCK_RENAME_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_BLOCK_RENAME_NS)),
        );
        eprintln!(
            "cruft-profile-function-body-assign-expr-stmt-member-value-function-proto-body-if-consequent-block-body-stmt: expression={}:{:.1}ms return={}:{:.1}ms block={}:{:.1}ms variable={}:{:.1}ms if={}:{:.1}ms loop={}:{:.1}ms other={}:{:.1}ms",
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_EXPRESSION_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_EXPRESSION_NS)),
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_RETURN_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_RETURN_NS)),
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_BLOCK_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_BLOCK_NS)),
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_VARIABLE_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_VARIABLE_NS)),
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_NS)),
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_LOOP_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_LOOP_NS)),
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_OTHER_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_OTHER_NS)),
        );
        eprintln!(
            "cruft-profile-function-body-assign-expr-stmt-member-value-function-proto-body-if-consequent-block-body-if: test={}:{:.1}ms patch={}:{:.1}ms consequent={}:{:.1}ms alternate={}:{:.1}ms",
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_TEST_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_TEST_NS)),
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_PATCH_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_PATCH_NS)),
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_CONSEQUENT_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_CONSEQUENT_NS)),
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_ALTERNATE_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_ALTERNATE_NS)),
        );
        eprintln!(
            "cruft-profile-function-body-assign-expr-stmt-member-value-function-proto-body-if-consequent-block-body-if-consequent-clause: block={}:{:.1}ms if={}:{:.1}ms return={}:{:.1}ms expr={}:{:.1}ms other={}:{:.1}ms",
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_CONSEQUENT_CLAUSE_BLOCK_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_CONSEQUENT_CLAUSE_BLOCK_NS)),
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_CONSEQUENT_CLAUSE_IF_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_CONSEQUENT_CLAUSE_IF_NS)),
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_CONSEQUENT_CLAUSE_RETURN_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_CONSEQUENT_CLAUSE_RETURN_NS)),
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_CONSEQUENT_CLAUSE_EXPR_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_CONSEQUENT_CLAUSE_EXPR_NS)),
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_CONSEQUENT_CLAUSE_OTHER_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_CONSEQUENT_CLAUSE_OTHER_NS)),
        );
        eprintln!(
            "cruft-profile-function-body-assign-expr-stmt-member-value-function-proto-body-if-consequent-block-body-if-consequent-block: prescan={}:{:.1}ms reset_seed={}:{:.1}ms prebind={}:{:.1}ms body={}:{:.1}ms rename={}:{:.1}ms",
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_CONSEQUENT_BLOCK_PRESCAN_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_CONSEQUENT_BLOCK_PRESCAN_NS)),
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_CONSEQUENT_BLOCK_RESET_SEED_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_CONSEQUENT_BLOCK_RESET_SEED_NS)),
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_CONSEQUENT_BLOCK_PREBIND_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_CONSEQUENT_BLOCK_PREBIND_NS)),
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_NS)),
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_CONSEQUENT_BLOCK_RENAME_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_CONSEQUENT_BLOCK_RENAME_NS)),
        );
        eprintln!(
            "cruft-profile-function-body-assign-expr-stmt-member-value-function-proto-body-if-consequent-block-body-if-consequent-block-body-stmt: expression={}:{:.1}ms return={}:{:.1}ms variable={}:{:.1}ms if={}:{:.1}ms other={}:{:.1}ms",
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_EXPRESSION_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_EXPRESSION_NS)),
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_RETURN_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_RETURN_NS)),
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_VARIABLE_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_VARIABLE_NS)),
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_NS)),
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_OTHER_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_OTHER_NS)),
        );
        eprintln!(
            "cruft-profile-function-body-assign-expr-stmt-member-value-function-proto-body-if-consequent-block-body-if-consequent-block-body-if: test={}:{:.1}ms patch={}:{:.1}ms consequent={}:{:.1}ms alternate={}:{:.1}ms",
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_TEST_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_TEST_NS)),
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_PATCH_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_PATCH_NS)),
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_CONSEQUENT_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_CONSEQUENT_NS)),
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_ALTERNATE_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_ALTERNATE_NS)),
        );
        eprintln!(
            "cruft-profile-function-body-assign-expr-stmt-member-value-function-proto-body-if-consequent-block-body-if-consequent-block-body-if-consequent-clause: block={}:{:.1}ms if={}:{:.1}ms return={}:{:.1}ms expr={}:{:.1}ms other={}:{:.1}ms",
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_CONSEQUENT_CLAUSE_BLOCK_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_CONSEQUENT_CLAUSE_BLOCK_NS)),
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_CONSEQUENT_CLAUSE_IF_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_CONSEQUENT_CLAUSE_IF_NS)),
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_CONSEQUENT_CLAUSE_RETURN_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_CONSEQUENT_CLAUSE_RETURN_NS)),
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_CONSEQUENT_CLAUSE_EXPR_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_CONSEQUENT_CLAUSE_EXPR_NS)),
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_CONSEQUENT_CLAUSE_OTHER_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_CONSEQUENT_CLAUSE_OTHER_NS)),
        );
        eprintln!(
            "cruft-profile-function-body-assign-expr-stmt-member-value-function-proto-body-if-consequent-block-body-if-consequent-block-body-if-consequent-block: prescan={}:{:.1}ms reset_seed={}:{:.1}ms prebind={}:{:.1}ms body={}:{:.1}ms rename={}:{:.1}ms",
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_CONSEQUENT_BLOCK_PRESCAN_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_CONSEQUENT_BLOCK_PRESCAN_NS)),
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_CONSEQUENT_BLOCK_RESET_SEED_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_CONSEQUENT_BLOCK_RESET_SEED_NS)),
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_CONSEQUENT_BLOCK_PREBIND_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_CONSEQUENT_BLOCK_PREBIND_NS)),
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_NS)),
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_CONSEQUENT_BLOCK_RENAME_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_CONSEQUENT_BLOCK_RENAME_NS)),
        );
        eprintln!(
            "cruft-profile-function-body-assign-expr-stmt-member-value-function-proto-body-if-consequent-block-body-if-consequent-block-body-if-consequent-block-body-stmt: expression={}:{:.1}ms return={}:{:.1}ms variable={}:{:.1}ms if={}:{:.1}ms other={}:{:.1}ms",
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_EXPRESSION_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_EXPRESSION_NS)),
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_RETURN_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_RETURN_NS)),
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_VARIABLE_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_VARIABLE_NS)),
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_NS)),
            cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_OTHER_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_ASSIGN_EXPR_STMT_MEMBER_VALUE_FUNCTION_PROTO_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_IF_CONSEQUENT_BLOCK_BODY_STMT_OTHER_NS)),
        );
        eprintln!(
            "cruft-profile-function-body-call-route: super={}:{:.1}ms super_member={}:{:.1}ms method={}:{:.1}ms spread_apply={}:{:.1}ms plain={}:{:.1}ms new_spread={}:{:.1}ms new_plain={}:{:.1}ms",
            cp::read(&cp::FUNCTION_BODY_CALL_ROUTE_SUPER_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_CALL_ROUTE_SUPER_NS)),
            cp::read(&cp::FUNCTION_BODY_CALL_ROUTE_SUPER_MEMBER_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_CALL_ROUTE_SUPER_MEMBER_NS)),
            cp::read(&cp::FUNCTION_BODY_CALL_ROUTE_METHOD_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_CALL_ROUTE_METHOD_NS)),
            cp::read(&cp::FUNCTION_BODY_CALL_ROUTE_SPREAD_APPLY_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_CALL_ROUTE_SPREAD_APPLY_NS)),
            cp::read(&cp::FUNCTION_BODY_CALL_ROUTE_PLAIN_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_CALL_ROUTE_PLAIN_NS)),
            cp::read(&cp::FUNCTION_BODY_CALL_ROUTE_NEW_SPREAD_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_CALL_ROUTE_NEW_SPREAD_NS)),
            cp::read(&cp::FUNCTION_BODY_CALL_ROUTE_NEW_PLAIN_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_CALL_ROUTE_NEW_PLAIN_NS)),
        );
        eprintln!(
            "cruft-profile-function-body-plain-call-phase: callee={}:{:.1}ms args={}:{:.1}ms emit={}:{:.1}ms",
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_CALLEE_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_CALLEE_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARGS_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARGS_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_EMIT_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_EMIT_NS)),
        );
        eprintln!(
            "cruft-profile-function-body-plain-call-callee: identifier={}:{:.1}ms function={}:{:.1}ms call_new={}:{:.1}ms other={}:{:.1}ms",
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_CALLEE_IDENTIFIER_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_CALLEE_IDENTIFIER_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_CALLEE_FUNCTION_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_CALLEE_FUNCTION_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_CALLEE_CALL_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_CALLEE_CALL_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_CALLEE_OTHER_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_CALLEE_OTHER_NS)),
        );
        eprintln!(
            "cruft-profile-function-body-plain-call-arg: function={}:{:.1}ms arrow={}:{:.1}ms literal={}:{:.1}ms identifier={}:{:.1}ms member={}:{:.1}ms call_new={}:{:.1}ms object_array={}:{:.1}ms operator={}:{:.1}ms other={}:{:.1}ms",
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_ARROW_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_ARROW_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_LITERAL_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_LITERAL_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_IDENTIFIER_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_IDENTIFIER_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_MEMBER_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_MEMBER_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_CALL_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_CALL_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_OBJECT_ARRAY_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_OBJECT_ARRAY_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_OPERATOR_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_OPERATOR_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_OTHER_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_OTHER_NS)),
        );
        eprintln!(
            "cruft-profile-function-body-plain-call-arg-function-proto: setup={}:{:.1}ms params={}:{:.1}ms prealloc={}:{:.1}ms hoist={}:{:.1}ms body={}:{:.1}ms assemble={}:{:.1}ms",
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_SETUP_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_SETUP_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_PARAMS_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_PARAMS_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_PREALLOC_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_PREALLOC_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_HOIST_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_HOIST_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_ASSEMBLE_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_ASSEMBLE_NS)),
        );
        eprintln!(
            "cruft-profile-function-body-plain-call-arg-function-proto-body-stmt: expression={}:{:.1}ms return={}:{:.1}ms block={}:{:.1}ms variable={}:{:.1}ms throw={}:{:.1}ms if={}:{:.1}ms loop={}:{:.1}ms try={}:{:.1}ms switch={}:{:.1}ms decl={}:{:.1}ms control={}:{:.1}ms other={}:{:.1}ms",
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_EXPRESSION_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_EXPRESSION_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_RETURN_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_RETURN_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_BLOCK_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_BLOCK_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VARIABLE_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VARIABLE_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_THROW_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_THROW_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_IF_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_IF_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_LOOP_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_LOOP_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_TRY_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_TRY_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_SWITCH_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_SWITCH_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_DECL_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_DECL_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_CONTROL_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_CONTROL_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_OTHER_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_OTHER_NS)),
        );
        eprintln!(
            "cruft-profile-function-body-plain-call-arg-function-proto-body-var-detail: iter={}:{:.1}ms target={}:{:.1}ms init={}:{:.1}ms store={}:{:.1}ms mirror={}:{:.1}ms",
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_DECL_ITER_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_DECL_ITER_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_TARGET_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_TARGET_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_STORE_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_STORE_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_MIRROR_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_MIRROR_NS)),
        );
        eprintln!(
            "cruft-profile-function-body-plain-call-arg-function-proto-body-var-init: function={}:{:.1}ms arrow={}:{:.1}ms class={}:{:.1}ms call={}:{:.1}ms object_array={}:{:.1}ms operator={}:{:.1}ms other={}:{:.1}ms",
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_FUNCTION_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_FUNCTION_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_ARROW_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_ARROW_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CLASS_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CLASS_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_OBJECT_ARRAY_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_OBJECT_ARRAY_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_OPERATOR_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_OPERATOR_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_OTHER_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_OTHER_NS)),
        );
        eprintln!(
            "cruft-profile-function-body-plain-call-arg-function-proto-body-var-init-call: route_plain={}:{:.1}ms route_method={}:{:.1}ms route_spread_apply={}:{:.1}ms route_new={}:{:.1}ms callee={}:{:.1}ms args={}:{:.1}ms emit={}:{:.1}ms",
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_ROUTE_PLAIN_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_ROUTE_PLAIN_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_ROUTE_METHOD_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_ROUTE_METHOD_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_ROUTE_SPREAD_APPLY_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_ROUTE_SPREAD_APPLY_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_ROUTE_NEW_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_ROUTE_NEW_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_ARGS_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_ARGS_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_EMIT_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_EMIT_NS)),
        );
        eprintln!(
            "cruft-profile-function-body-plain-call-arg-function-proto-body-var-init-call-callee: function={}:{:.1}ms arrow={}:{:.1}ms class={}:{:.1}ms member={}:{:.1}ms identifier={}:{:.1}ms call={}:{:.1}ms object_array={}:{:.1}ms operator={}:{:.1}ms other={}:{:.1}ms",
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_ARROW_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_ARROW_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_CLASS_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_CLASS_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_MEMBER_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_MEMBER_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_IDENTIFIER_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_IDENTIFIER_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_CALL_EXPR_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_CALL_EXPR_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_OBJECT_ARRAY_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_OBJECT_ARRAY_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_OPERATOR_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_OPERATOR_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_OTHER_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_OTHER_NS)),
        );
        eprintln!(
            "cruft-profile-function-body-plain-call-arg-function-proto-body-var-init-call-callee-function: proto={}:{:.1}ms intern_emit={}:{:.1}ms setup={}:{:.1}ms params={}:{:.1}ms prealloc={}:{:.1}ms hoist={}:{:.1}ms body={}:{:.1}ms assemble={}:{:.1}ms",
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_INTERN_EMIT_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_INTERN_EMIT_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_SETUP_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_SETUP_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_PARAMS_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_PARAMS_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_PREALLOC_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_PREALLOC_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_BODY_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_BODY_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_ASSEMBLE_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_ASSEMBLE_NS)),
        );
        eprintln!(
            "cruft-profile-function-body-plain-call-arg-function-proto-body-var-init-call-callee-function-hoist: decl_proto={}:{:.1}ms emit_store={}:{:.1}ms",
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_EMIT_STORE_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_EMIT_STORE_NS)),
        );
        eprintln!(
            "cruft-profile-function-body-plain-call-arg-function-proto-body-var-init-call-callee-function-hoist-decl-proto: setup={}:{:.1}ms params={}:{:.1}ms prealloc={}:{:.1}ms hoist={}:{:.1}ms body={}:{:.1}ms assemble={}:{:.1}ms",
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_SETUP_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_SETUP_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_PARAMS_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_PARAMS_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_PREALLOC_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_PREALLOC_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_HOIST_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_HOIST_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_ASSEMBLE_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_ASSEMBLE_NS)),
        );
        eprintln!(
            "cruft-profile-function-body-plain-call-arg-function-proto-body-var-init-call-callee-function-hoist-decl-proto-recursive-depth: depth1={}:{:.1}ms depth2={}:{:.1}ms depth3={}:{:.1}ms depth4_plus={}:{:.1}ms",
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_RECURSIVE_DEPTH1_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_RECURSIVE_DEPTH1_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_RECURSIVE_DEPTH2_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_RECURSIVE_DEPTH2_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_RECURSIVE_DEPTH3_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_RECURSIVE_DEPTH3_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_RECURSIVE_DEPTH4_PLUS_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_RECURSIVE_DEPTH4_PLUS_NS)),
        );
        eprintln!(
            "cruft-profile-function-body-plain-call-arg-function-proto-body-var-init-call-callee-function-hoist-decl-proto-post-body: backprop={}:{:.1}ms metadata={}:{:.1}ms shrink={}:{:.1}ms build={}:{:.1}ms",
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_POST_BODY_BACKPROP_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_POST_BODY_BACKPROP_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_POST_BODY_METADATA_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_POST_BODY_METADATA_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_POST_BODY_SHRINK_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_POST_BODY_SHRINK_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_POST_BODY_BUILD_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_POST_BODY_BUILD_NS)),
        );
        eprintln!(
            "cruft-profile-function-body-plain-call-arg-function-proto-body-var-init-call-callee-function-hoist-decl-proto-post-body-build: diagnostic_source={}:{:.1}ms struct={}:{:.1}ms",
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_POST_BODY_BUILD_DIAGNOSTIC_SOURCE_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_POST_BODY_BUILD_DIAGNOSTIC_SOURCE_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_POST_BODY_BUILD_STRUCT_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_POST_BODY_BUILD_STRUCT_NS)),
        );
        eprintln!(
            "cruft-profile-function-body-plain-call-arg-function-proto-body-var-init-call-callee-function-hoist-decl-proto-body-stmt: expression={}:{:.1}ms return={}:{:.1}ms block={}:{:.1}ms variable={}:{:.1}ms throw={}:{:.1}ms if={}:{:.1}ms loop={}:{:.1}ms try={}:{:.1}ms switch={}:{:.1}ms decl={}:{:.1}ms control={}:{:.1}ms other={}:{:.1}ms",
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_STMT_EXPRESSION_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_STMT_EXPRESSION_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_STMT_RETURN_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_STMT_RETURN_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_STMT_BLOCK_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_STMT_BLOCK_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_STMT_VARIABLE_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_STMT_VARIABLE_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_STMT_THROW_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_STMT_THROW_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_STMT_IF_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_STMT_IF_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_STMT_LOOP_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_STMT_LOOP_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_STMT_TRY_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_STMT_TRY_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_STMT_SWITCH_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_STMT_SWITCH_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_STMT_DECL_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_STMT_DECL_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_STMT_CONTROL_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_STMT_CONTROL_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_STMT_OTHER_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_STMT_OTHER_NS)),
        );
        eprintln!(
            "cruft-profile-function-body-plain-call-arg-function-proto-body-var-init-call-callee-function-hoist-decl-proto-body-expr: function={}:{:.1}ms arrow={}:{:.1}ms class={}:{:.1}ms paren={}:{:.1}ms literal={}:{:.1}ms identifier={}:{:.1}ms member={}:{:.1}ms call_new={}:{:.1}ms object_array={}:{:.1}ms operator={}:{:.1}ms other={}:{:.1}ms",
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_EXPR_FUNCTION_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_EXPR_FUNCTION_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_EXPR_ARROW_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_EXPR_ARROW_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_EXPR_CLASS_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_EXPR_CLASS_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_EXPR_PAREN_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_EXPR_PAREN_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_EXPR_LITERAL_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_EXPR_LITERAL_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_EXPR_IDENTIFIER_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_EXPR_IDENTIFIER_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_EXPR_MEMBER_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_EXPR_MEMBER_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_EXPR_CALL_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_EXPR_CALL_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_EXPR_OBJECT_ARRAY_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_EXPR_OBJECT_ARRAY_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_EXPR_OPERATOR_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_EXPR_OPERATOR_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_EXPR_OTHER_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_EXPR_OTHER_NS)),
        );
        eprintln!(
            "cruft-profile-function-body-plain-call-arg-function-proto-body-var-init-call-callee-function-hoist-decl-proto-body-operator: unary_update={}:{:.1}ms binary_arith={}:{:.1}ms binary_compare={}:{:.1}ms binary_bitwise={}:{:.1}ms logical={}:{:.1}ms nullish={}:{:.1}ms conditional={}:{:.1}ms assign={}:{:.1}ms sequence={}:{:.1}ms",
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_UNARY_UPDATE_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_UNARY_UPDATE_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_BINARY_ARITH_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_BINARY_ARITH_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_BINARY_COMPARE_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_BINARY_COMPARE_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_BINARY_BITWISE_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_BINARY_BITWISE_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_LOGICAL_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_LOGICAL_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_NULLISH_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_NULLISH_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_CONDITIONAL_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_CONDITIONAL_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_SEQUENCE_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_SEQUENCE_NS)),
        );
        eprintln!(
            "cruft-profile-function-body-plain-call-arg-function-proto-body-var-init-call-callee-function-hoist-decl-proto-body-operator-assign-target: identifier={}:{:.1}ms member={}:{:.1}ms destructure={}:{:.1}ms paren={}:{:.1}ms call_invalid={}:{:.1}ms other={}:{:.1}ms",
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_TARGET_IDENTIFIER_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_TARGET_IDENTIFIER_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_TARGET_MEMBER_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_TARGET_MEMBER_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_TARGET_DESTRUCTURE_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_TARGET_DESTRUCTURE_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_TARGET_PAREN_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_TARGET_PAREN_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_TARGET_CALL_INVALID_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_TARGET_CALL_INVALID_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_TARGET_OTHER_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_TARGET_OTHER_NS)),
        );
        eprintln!(
            "cruft-profile-function-body-plain-call-arg-function-proto-body-var-init-call-callee-function-hoist-decl-proto-body-operator-assign-member-phase: object={}:{:.1}ms key={}:{:.1}ms value={}:{:.1}ms store={}:{:.1}ms",
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_MEMBER_OBJECT_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_MEMBER_OBJECT_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_MEMBER_KEY_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_MEMBER_KEY_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_MEMBER_VALUE_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_MEMBER_VALUE_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_MEMBER_STORE_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_MEMBER_STORE_NS)),
        );
        eprintln!(
            "cruft-profile-function-body-plain-call-arg-function-proto-body-var-init-call-callee-function-hoist-decl-proto-body-operator-assign-identifier-phase: value={}:{:.1}ms store={}:{:.1}ms",
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_STORE_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_STORE_NS)),
        );
        eprintln!(
            "cruft-profile-function-body-plain-call-arg-function-proto-body-var-init-call-callee-function-hoist-decl-proto-body-operator-assign-identifier-pre-value: const_check={}:{:.1}ms with_check={}:{:.1}ms direct_eval_check={}:{:.1}ms strict_global_check={}:{:.1}ms",
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_CONST_CHECK_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_CONST_CHECK_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_WITH_CHECK_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_WITH_CHECK_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_DIRECT_EVAL_CHECK_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_DIRECT_EVAL_CHECK_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_STRICT_GLOBAL_CHECK_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_STRICT_GLOBAL_CHECK_NS)),
        );
        eprintln!(
            "cruft-profile-function-body-plain-call-arg-function-proto-body-var-init-call-callee-function-hoist-decl-proto-body-operator-assign-identifier-store-route: resolve_local={}:{:.1}ms resolve_upvalue={}:{:.1}ms route_local={}:{:.1}ms route_upvalue={}:{:.1}ms route_global={}:{:.1}ms",
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_STORE_RESOLVE_LOCAL_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_STORE_RESOLVE_LOCAL_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_STORE_RESOLVE_UPVALUE_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_STORE_RESOLVE_UPVALUE_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_STORE_ROUTE_LOCAL_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_STORE_ROUTE_LOCAL_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_STORE_ROUTE_UPVALUE_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_STORE_ROUTE_UPVALUE_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_STORE_ROUTE_GLOBAL_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_STORE_ROUTE_GLOBAL_NS)),
        );
        eprintln!(
            "cruft-profile-function-body-plain-call-arg-function-proto-body-var-init-call-callee-function-hoist-decl-proto-body-operator-assign-identifier-value: function={}:{:.1}ms arrow={}:{:.1}ms class={}:{:.1}ms literal={}:{:.1}ms identifier={}:{:.1}ms member={}:{:.1}ms call_new={}:{:.1}ms object_array={}:{:.1}ms operator={}:{:.1}ms other={}:{:.1}ms",
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_FUNCTION_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_FUNCTION_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_ARROW_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_ARROW_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_CLASS_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_CLASS_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_LITERAL_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_LITERAL_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_IDENTIFIER_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_IDENTIFIER_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_MEMBER_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_MEMBER_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_CALL_NEW_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_CALL_NEW_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_OBJECT_ARRAY_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_OBJECT_ARRAY_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_OPERATOR_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_OPERATOR_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_OTHER_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_OTHER_NS)),
        );
        eprintln!(
            "cruft-profile-function-body-plain-call-arg-function-proto-body-var-init-call-callee-function-hoist-decl-proto-body-operator-assign-identifier-value-operator: unary_update={}:{:.1}ms binary_arith={}:{:.1}ms binary_compare={}:{:.1}ms binary_bitwise={}:{:.1}ms logical={}:{:.1}ms nullish={}:{:.1}ms conditional={}:{:.1}ms assign={}:{:.1}ms sequence={}:{:.1}ms",
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_OPERATOR_UNARY_UPDATE_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_OPERATOR_UNARY_UPDATE_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_OPERATOR_BINARY_ARITH_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_OPERATOR_BINARY_ARITH_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_OPERATOR_BINARY_COMPARE_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_OPERATOR_BINARY_COMPARE_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_OPERATOR_BINARY_BITWISE_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_OPERATOR_BINARY_BITWISE_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_OPERATOR_LOGICAL_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_OPERATOR_LOGICAL_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_OPERATOR_NULLISH_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_OPERATOR_NULLISH_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_OPERATOR_CONDITIONAL_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_OPERATOR_CONDITIONAL_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_OPERATOR_ASSIGN_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_OPERATOR_ASSIGN_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_OPERATOR_SEQUENCE_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_OPERATOR_SEQUENCE_NS)),
        );
        eprintln!(
            "cruft-profile-function-body-plain-call-arg-function-proto-body-var-init-call-callee-function-hoist-decl-proto-body-operator-assign-identifier-value-operator-binary-bitwise-phase: left={}:{:.1}ms right={}:{:.1}ms emit={}:{:.1}ms",
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_OPERATOR_BINARY_BITWISE_LEFT_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_OPERATOR_BINARY_BITWISE_LEFT_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_OPERATOR_BINARY_BITWISE_RIGHT_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_OPERATOR_BINARY_BITWISE_RIGHT_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_OPERATOR_BINARY_BITWISE_EMIT_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_OPERATOR_BINARY_BITWISE_EMIT_NS)),
        );
        eprintln!(
            "cruft-profile-function-body-plain-call-arg-function-proto-body-var-init-call-callee-function-hoist-decl-proto-body-operator-assign-identifier-value-operator-binary-bitwise-left: literal={}:{:.1}ms identifier={}:{:.1}ms member={}:{:.1}ms call_new={}:{:.1}ms operator={}:{:.1}ms other={}:{:.1}ms",
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_OPERATOR_BINARY_BITWISE_LEFT_LITERAL_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_OPERATOR_BINARY_BITWISE_LEFT_LITERAL_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_OPERATOR_BINARY_BITWISE_LEFT_IDENTIFIER_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_OPERATOR_BINARY_BITWISE_LEFT_IDENTIFIER_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_OPERATOR_BINARY_BITWISE_LEFT_MEMBER_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_OPERATOR_BINARY_BITWISE_LEFT_MEMBER_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_OPERATOR_BINARY_BITWISE_LEFT_CALL_NEW_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_OPERATOR_BINARY_BITWISE_LEFT_CALL_NEW_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_OPERATOR_BINARY_BITWISE_LEFT_OPERATOR_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_OPERATOR_BINARY_BITWISE_LEFT_OPERATOR_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_OPERATOR_BINARY_BITWISE_LEFT_OTHER_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_OPERATOR_BINARY_BITWISE_LEFT_OTHER_NS)),
        );
        eprintln!(
            "cruft-profile-function-body-plain-call-arg-function-proto-body-var-init-call-callee-function-hoist-decl-proto-body-operator-assign-identifier-value-operator-binary-bitwise-left-member-phase: object={}:{:.1}ms optional={}:{:.1}ms property={}:{:.1}ms",
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_OPERATOR_BINARY_BITWISE_LEFT_MEMBER_OBJECT_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_OPERATOR_BINARY_BITWISE_LEFT_MEMBER_OBJECT_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_OPERATOR_BINARY_BITWISE_LEFT_MEMBER_OPTIONAL_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_OPERATOR_BINARY_BITWISE_LEFT_MEMBER_OPTIONAL_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_OPERATOR_BINARY_BITWISE_LEFT_MEMBER_PROPERTY_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_OPERATOR_BINARY_BITWISE_LEFT_MEMBER_PROPERTY_NS)),
        );
        eprintln!(
            "cruft-profile-function-body-plain-call-arg-function-proto-body-var-init-call-callee-function-hoist-decl-proto-body-operator-assign-identifier-value-operator-binary-bitwise-left-member-object: literal={}:{:.1}ms identifier={}:{:.1}ms member={}:{:.1}ms call_new={}:{:.1}ms object_array={}:{:.1}ms operator={}:{:.1}ms other={}:{:.1}ms",
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_OPERATOR_BINARY_BITWISE_LEFT_MEMBER_OBJECT_LITERAL_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_OPERATOR_BINARY_BITWISE_LEFT_MEMBER_OBJECT_LITERAL_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_OPERATOR_BINARY_BITWISE_LEFT_MEMBER_OBJECT_IDENTIFIER_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_OPERATOR_BINARY_BITWISE_LEFT_MEMBER_OBJECT_IDENTIFIER_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_OPERATOR_BINARY_BITWISE_LEFT_MEMBER_OBJECT_MEMBER_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_OPERATOR_BINARY_BITWISE_LEFT_MEMBER_OBJECT_MEMBER_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_OPERATOR_BINARY_BITWISE_LEFT_MEMBER_OBJECT_CALL_NEW_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_OPERATOR_BINARY_BITWISE_LEFT_MEMBER_OBJECT_CALL_NEW_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_OPERATOR_BINARY_BITWISE_LEFT_MEMBER_OBJECT_OBJECT_ARRAY_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_OPERATOR_BINARY_BITWISE_LEFT_MEMBER_OBJECT_OBJECT_ARRAY_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_OPERATOR_BINARY_BITWISE_LEFT_MEMBER_OBJECT_OPERATOR_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_OPERATOR_BINARY_BITWISE_LEFT_MEMBER_OBJECT_OPERATOR_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_OPERATOR_BINARY_BITWISE_LEFT_MEMBER_OBJECT_OTHER_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_OPERATOR_BINARY_BITWISE_LEFT_MEMBER_OBJECT_OTHER_NS)),
        );
        eprintln!(
            "cruft-profile-function-body-plain-call-arg-function-proto-body-var-init-call-callee-function-hoist-decl-proto-body-operator-assign-identifier-value-operator-binary-bitwise-left-member-object-identifier: with_check={}:{:.1}ms resolve_local={}:{:.1}ms resolve_upvalue={}:{:.1}ms intern_global={}:{:.1}ms emit={}:{:.1}ms local={}:{:.1}ms upvalue={}:{:.1}ms global={}:{:.1}ms with={}:{:.1}ms",
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_OPERATOR_BINARY_BITWISE_LEFT_MEMBER_OBJECT_IDENTIFIER_WITH_CHECK_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_OPERATOR_BINARY_BITWISE_LEFT_MEMBER_OBJECT_IDENTIFIER_WITH_CHECK_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_OPERATOR_BINARY_BITWISE_LEFT_MEMBER_OBJECT_IDENTIFIER_RESOLVE_LOCAL_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_OPERATOR_BINARY_BITWISE_LEFT_MEMBER_OBJECT_IDENTIFIER_RESOLVE_LOCAL_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_OPERATOR_BINARY_BITWISE_LEFT_MEMBER_OBJECT_IDENTIFIER_RESOLVE_UPVALUE_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_OPERATOR_BINARY_BITWISE_LEFT_MEMBER_OBJECT_IDENTIFIER_RESOLVE_UPVALUE_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_OPERATOR_BINARY_BITWISE_LEFT_MEMBER_OBJECT_IDENTIFIER_INTERN_GLOBAL_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_OPERATOR_BINARY_BITWISE_LEFT_MEMBER_OBJECT_IDENTIFIER_INTERN_GLOBAL_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_OPERATOR_BINARY_BITWISE_LEFT_MEMBER_OBJECT_IDENTIFIER_EMIT_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_OPERATOR_BINARY_BITWISE_LEFT_MEMBER_OBJECT_IDENTIFIER_EMIT_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_OPERATOR_BINARY_BITWISE_LEFT_MEMBER_OBJECT_IDENTIFIER_LOCAL_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_OPERATOR_BINARY_BITWISE_LEFT_MEMBER_OBJECT_IDENTIFIER_LOCAL_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_OPERATOR_BINARY_BITWISE_LEFT_MEMBER_OBJECT_IDENTIFIER_UPVALUE_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_OPERATOR_BINARY_BITWISE_LEFT_MEMBER_OBJECT_IDENTIFIER_UPVALUE_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_OPERATOR_BINARY_BITWISE_LEFT_MEMBER_OBJECT_IDENTIFIER_GLOBAL_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_OPERATOR_BINARY_BITWISE_LEFT_MEMBER_OBJECT_IDENTIFIER_GLOBAL_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_OPERATOR_BINARY_BITWISE_LEFT_MEMBER_OBJECT_IDENTIFIER_WITH_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_OPERATOR_BINARY_BITWISE_LEFT_MEMBER_OBJECT_IDENTIFIER_WITH_NS)),
        );
        eprintln!(
            "cruft-profile-function-body-plain-call-arg-function-proto-body-var-init-call-callee-function-hoist-decl-proto-body-operator-assign-identifier-value-operator-binary-bitwise-left-member-object-identifier-upvalue-resolution: local_scan={}:{:.1}ms upvalue_scan={}:{:.1}ms intermediate_local={}:{:.1}ms intermediate_upvalue={}:{:.1}ms current_local={}:{:.1}ms current_upvalue={}:{:.1}ms route_local={}:{:.1}ms route_upvalue={}:{:.1}ms route_miss={}:{:.1}ms",
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_OPERATOR_BINARY_BITWISE_LEFT_MEMBER_OBJECT_IDENTIFIER_UPVALUE_RESOLUTION_LOCAL_SCAN_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_OPERATOR_BINARY_BITWISE_LEFT_MEMBER_OBJECT_IDENTIFIER_UPVALUE_RESOLUTION_LOCAL_SCAN_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_OPERATOR_BINARY_BITWISE_LEFT_MEMBER_OBJECT_IDENTIFIER_UPVALUE_RESOLUTION_UPVALUE_SCAN_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_OPERATOR_BINARY_BITWISE_LEFT_MEMBER_OBJECT_IDENTIFIER_UPVALUE_RESOLUTION_UPVALUE_SCAN_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_OPERATOR_BINARY_BITWISE_LEFT_MEMBER_OBJECT_IDENTIFIER_UPVALUE_RESOLUTION_INTERMEDIATE_LOCAL_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_OPERATOR_BINARY_BITWISE_LEFT_MEMBER_OBJECT_IDENTIFIER_UPVALUE_RESOLUTION_INTERMEDIATE_LOCAL_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_OPERATOR_BINARY_BITWISE_LEFT_MEMBER_OBJECT_IDENTIFIER_UPVALUE_RESOLUTION_INTERMEDIATE_UPVALUE_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_OPERATOR_BINARY_BITWISE_LEFT_MEMBER_OBJECT_IDENTIFIER_UPVALUE_RESOLUTION_INTERMEDIATE_UPVALUE_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_OPERATOR_BINARY_BITWISE_LEFT_MEMBER_OBJECT_IDENTIFIER_UPVALUE_RESOLUTION_CURRENT_LOCAL_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_OPERATOR_BINARY_BITWISE_LEFT_MEMBER_OBJECT_IDENTIFIER_UPVALUE_RESOLUTION_CURRENT_LOCAL_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_OPERATOR_BINARY_BITWISE_LEFT_MEMBER_OBJECT_IDENTIFIER_UPVALUE_RESOLUTION_CURRENT_UPVALUE_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_OPERATOR_BINARY_BITWISE_LEFT_MEMBER_OBJECT_IDENTIFIER_UPVALUE_RESOLUTION_CURRENT_UPVALUE_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_OPERATOR_BINARY_BITWISE_LEFT_MEMBER_OBJECT_IDENTIFIER_UPVALUE_RESOLUTION_ROUTE_LOCAL_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_OPERATOR_BINARY_BITWISE_LEFT_MEMBER_OBJECT_IDENTIFIER_UPVALUE_RESOLUTION_ROUTE_LOCAL_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_OPERATOR_BINARY_BITWISE_LEFT_MEMBER_OBJECT_IDENTIFIER_UPVALUE_RESOLUTION_ROUTE_UPVALUE_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_OPERATOR_BINARY_BITWISE_LEFT_MEMBER_OBJECT_IDENTIFIER_UPVALUE_RESOLUTION_ROUTE_UPVALUE_NS)),
            cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_OPERATOR_BINARY_BITWISE_LEFT_MEMBER_OBJECT_IDENTIFIER_UPVALUE_RESOLUTION_ROUTE_MISS_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_PLAIN_CALL_ARG_FUNCTION_PROTO_BODY_STMT_VAR_INIT_CALL_CALLEE_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OPERATOR_ASSIGN_IDENTIFIER_VALUE_OPERATOR_BINARY_BITWISE_LEFT_MEMBER_OBJECT_IDENTIFIER_UPVALUE_RESOLUTION_ROUTE_MISS_NS)),
        );
        eprintln!(
            "cruft-profile-function-body-method-call-phase: receiver={}:{:.1}ms member_optional={}:{:.1}ms lookup={}:{:.1}ms call_optional={}:{:.1}ms args={}:{:.1}ms emit={}:{:.1}ms sink_patch={}:{:.1}ms",
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_RECEIVER_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_RECEIVER_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_MEMBER_OPTIONAL_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_MEMBER_OPTIONAL_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_LOOKUP_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_LOOKUP_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_CALL_OPTIONAL_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_CALL_OPTIONAL_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARGS_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARGS_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_EMIT_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_EMIT_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_SINK_PATCH_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_SINK_PATCH_NS)),
        );
        eprintln!(
            "cruft-profile-function-body-method-call-arg: function={}:{:.1}ms arrow={}:{:.1}ms class={}:{:.1}ms paren={}:{:.1}ms literal={}:{:.1}ms identifier={}:{:.1}ms member={}:{:.1}ms call={}:{:.1}ms object_array={}:{:.1}ms operator={}:{:.1}ms other={}:{:.1}ms",
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_ARROW_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_ARROW_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_CLASS_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_CLASS_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_PAREN_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_PAREN_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_LITERAL_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_LITERAL_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_IDENTIFIER_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_IDENTIFIER_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_MEMBER_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_MEMBER_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_CALL_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_CALL_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_OBJECT_ARRAY_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_OBJECT_ARRAY_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_OPERATOR_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_OPERATOR_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_OTHER_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_OTHER_NS)),
        );
        eprintln!(
            "cruft-profile-function-body-method-call-arg-function-proto: setup={}:{:.1}ms params={}:{:.1}ms prealloc={}:{:.1}ms hoist={}:{:.1}ms body={}:{:.1}ms assemble={}:{:.1}ms",
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_SETUP_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_SETUP_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_PARAMS_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_PARAMS_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_PREALLOC_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_PREALLOC_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_BODY_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_BODY_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_ASSEMBLE_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_ASSEMBLE_NS)),
        );
        eprintln!(
            "cruft-profile-function-body-method-call-arg-function-proto-hoist: decl_proto={}:{:.1}ms emit_store={}:{:.1}ms",
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_EMIT_STORE_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_EMIT_STORE_NS)),
        );
        eprintln!(
            "cruft-profile-function-body-method-call-arg-function-proto-hoist-decl-proto: setup={}:{:.1}ms params={}:{:.1}ms prealloc={}:{:.1}ms hoist={}:{:.1}ms body={}:{:.1}ms assemble={}:{:.1}ms",
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_SETUP_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_SETUP_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_PARAMS_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_PARAMS_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_PREALLOC_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_PREALLOC_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_HOIST_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_HOIST_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_ASSEMBLE_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_ASSEMBLE_NS)),
        );
        eprintln!(
            "cruft-profile-function-body-method-call-arg-function-proto-hoist-decl-proto-body-stmt: expression={}:{:.1}ms return={}:{:.1}ms block={}:{:.1}ms variable={}:{:.1}ms throw={}:{:.1}ms if={}:{:.1}ms loop={}:{:.1}ms try={}:{:.1}ms switch={}:{:.1}ms decl={}:{:.1}ms control={}:{:.1}ms other={}:{:.1}ms",
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_STMT_EXPRESSION_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_STMT_EXPRESSION_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_STMT_RETURN_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_STMT_RETURN_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_STMT_BLOCK_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_STMT_BLOCK_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_STMT_VARIABLE_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_STMT_VARIABLE_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_STMT_THROW_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_STMT_THROW_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_STMT_IF_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_STMT_IF_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_STMT_LOOP_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_STMT_LOOP_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_STMT_TRY_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_STMT_TRY_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_STMT_SWITCH_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_STMT_SWITCH_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_STMT_DECL_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_STMT_DECL_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_STMT_CONTROL_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_STMT_CONTROL_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_STMT_OTHER_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_STMT_OTHER_NS)),
        );
        eprintln!(
            "cruft-profile-function-body-method-call-arg-function-proto-hoist-decl-proto-body-expr: function={}:{:.1}ms arrow={}:{:.1}ms class={}:{:.1}ms paren={}:{:.1}ms literal={}:{:.1}ms identifier={}:{:.1}ms member={}:{:.1}ms call={}:{:.1}ms object_array={}:{:.1}ms operator={}:{:.1}ms other={}:{:.1}ms",
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_EXPR_FUNCTION_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_EXPR_FUNCTION_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_EXPR_ARROW_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_EXPR_ARROW_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_EXPR_CLASS_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_EXPR_CLASS_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_EXPR_PAREN_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_EXPR_PAREN_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_EXPR_LITERAL_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_EXPR_LITERAL_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_EXPR_IDENTIFIER_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_EXPR_IDENTIFIER_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_EXPR_MEMBER_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_EXPR_MEMBER_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_EXPR_CALL_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_EXPR_CALL_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_EXPR_OBJECT_ARRAY_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_EXPR_OBJECT_ARRAY_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_EXPR_OPERATOR_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_EXPR_OPERATOR_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_EXPR_OTHER_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_EXPR_OTHER_NS)),
        );
        eprintln!(
            "cruft-profile-function-body-method-call-arg-function-proto-hoist-decl-proto-body-detail: decl_function={}:{:.1}ms decl_class={}:{:.1}ms var_iter={}:{:.1}ms var_target={}:{:.1}ms var_init={}:{:.1}ms var_store={}:{:.1}ms var_mirror={}:{:.1}ms",
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_STMT_DECL_FUNCTION_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_STMT_DECL_FUNCTION_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_STMT_DECL_CLASS_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_STMT_DECL_CLASS_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_STMT_VAR_DECL_ITER_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_STMT_VAR_DECL_ITER_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_STMT_VAR_TARGET_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_STMT_VAR_TARGET_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_STMT_VAR_INIT_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_STMT_VAR_INIT_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_STMT_VAR_STORE_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_STMT_VAR_STORE_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_STMT_VAR_MIRROR_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_STMT_VAR_MIRROR_NS)),
        );
        eprintln!(
            "cruft-profile-function-body-method-call-arg-function-proto-hoist-decl-proto-body-next: class_compile={}:{:.1}ms class_bind={}:{:.1}ms var_init_function={}:{:.1}ms var_init_arrow={}:{:.1}ms var_init_class={}:{:.1}ms var_init_call={}:{:.1}ms var_init_object_array={}:{:.1}ms var_init_operator={}:{:.1}ms var_init_other={}:{:.1}ms",
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_STMT_DECL_CLASS_COMPILE_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_STMT_DECL_CLASS_COMPILE_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_STMT_DECL_CLASS_BIND_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_STMT_DECL_CLASS_BIND_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_STMT_VAR_INIT_FUNCTION_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_STMT_VAR_INIT_FUNCTION_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_STMT_VAR_INIT_ARROW_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_STMT_VAR_INIT_ARROW_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_STMT_VAR_INIT_CLASS_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_STMT_VAR_INIT_CLASS_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_STMT_VAR_INIT_CALL_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_STMT_VAR_INIT_CALL_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_STMT_VAR_INIT_OBJECT_ARRAY_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_STMT_VAR_INIT_OBJECT_ARRAY_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_STMT_VAR_INIT_OPERATOR_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_STMT_VAR_INIT_OPERATOR_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_STMT_VAR_INIT_OTHER_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_STMT_VAR_INIT_OTHER_NS)),
        );
        eprintln!(
            "cruft-profile-function-body-method-call-arg-function-proto-hoist-decl-proto-body-deep: class_heritage={}:{:.1}ms class_field_synth={}:{:.1}ms class_ctor_proto={}:{:.1}ms class_static_install={}:{:.1}ms class_member_pass={}:{:.1}ms var_init_array={}:{:.1}ms var_init_object_setup={}:{:.1}ms var_init_object_props={}:{:.1}ms",
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_HERITAGE_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_HERITAGE_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_FIELD_SYNTH_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_FIELD_SYNTH_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_CTOR_PROTO_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_CTOR_PROTO_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_STATIC_INSTALL_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_STATIC_INSTALL_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_MEMBER_PASS_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_MEMBER_PASS_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_VAR_INIT_ARRAY_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_VAR_INIT_ARRAY_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_VAR_INIT_OBJECT_SETUP_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_VAR_INIT_OBJECT_SETUP_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_VAR_INIT_OBJECT_PROPS_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_VAR_INIT_OBJECT_PROPS_NS)),
        );
        eprintln!(
            "cruft-profile-function-body-method-call-arg-function-proto-hoist-decl-proto-body-member-object: class_member_method={}:{:.1}ms class_member_field={}:{:.1}ms class_member_static_block={}:{:.1}ms object_prop_data={}:{:.1}ms object_prop_method={}:{:.1}ms object_prop_accessor={}:{:.1}ms object_prop_computed={}:{:.1}ms object_prop_spread={}:{:.1}ms",
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_MEMBER_METHOD_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_MEMBER_METHOD_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_MEMBER_FIELD_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_MEMBER_FIELD_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_MEMBER_STATIC_BLOCK_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_MEMBER_STATIC_BLOCK_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OBJECT_PROP_DATA_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OBJECT_PROP_DATA_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OBJECT_PROP_METHOD_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OBJECT_PROP_METHOD_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OBJECT_PROP_ACCESSOR_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OBJECT_PROP_ACCESSOR_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OBJECT_PROP_COMPUTED_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OBJECT_PROP_COMPUTED_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OBJECT_PROP_SPREAD_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OBJECT_PROP_SPREAD_NS)),
        );
        eprintln!(
            "cruft-profile-function-body-method-call-arg-function-proto-hoist-decl-proto-body-method-data: class_method_proto={}:{:.1}ms class_method_install={}:{:.1}ms object_data_key={}:{:.1}ms object_data_value={}:{:.1}ms object_data_init={}:{:.1}ms",
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_METHOD_PROTO_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_METHOD_PROTO_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_METHOD_INSTALL_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_METHOD_INSTALL_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OBJECT_DATA_KEY_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OBJECT_DATA_KEY_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OBJECT_DATA_VALUE_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OBJECT_DATA_VALUE_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OBJECT_DATA_INIT_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OBJECT_DATA_INIT_NS)),
        );
        eprintln!(
            "cruft-profile-function-body-method-call-arg-function-proto-hoist-decl-proto-body-method-proto-value: class_method_setup={}:{:.1}ms class_method_params={}:{:.1}ms class_method_prealloc={}:{:.1}ms class_method_hoist={}:{:.1}ms class_method_body={}:{:.1}ms class_method_assemble={}:{:.1}ms object_value_function={}:{:.1}ms object_value_arrow={}:{:.1}ms object_value_class={}:{:.1}ms object_value_call={}:{:.1}ms object_value_object_array={}:{:.1}ms object_value_operator={}:{:.1}ms object_value_other={}:{:.1}ms",
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_METHOD_PROTO_SETUP_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_METHOD_PROTO_SETUP_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_METHOD_PROTO_PARAMS_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_METHOD_PROTO_PARAMS_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_METHOD_PROTO_PREALLOC_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_METHOD_PROTO_PREALLOC_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_METHOD_PROTO_HOIST_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_METHOD_PROTO_HOIST_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_METHOD_PROTO_BODY_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_METHOD_PROTO_BODY_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_METHOD_PROTO_ASSEMBLE_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_METHOD_PROTO_ASSEMBLE_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OBJECT_DATA_VALUE_FUNCTION_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OBJECT_DATA_VALUE_FUNCTION_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OBJECT_DATA_VALUE_ARROW_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OBJECT_DATA_VALUE_ARROW_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OBJECT_DATA_VALUE_CLASS_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OBJECT_DATA_VALUE_CLASS_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OBJECT_DATA_VALUE_CALL_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OBJECT_DATA_VALUE_CALL_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OBJECT_DATA_VALUE_OBJECT_ARRAY_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OBJECT_DATA_VALUE_OBJECT_ARRAY_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OBJECT_DATA_VALUE_OPERATOR_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OBJECT_DATA_VALUE_OPERATOR_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OBJECT_DATA_VALUE_OTHER_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OBJECT_DATA_VALUE_OTHER_NS)),
        );
        eprintln!(
            "cruft-profile-function-body-method-call-arg-function-proto-hoist-decl-proto-body-class-method-setup-detail: source={}:{:.1}ms formal={}:{:.1}ms enclosing={}:{:.1}ms direct_eval_scan={}:{:.1}ms seed={}:{:.1}ms sub_init={}:{:.1}ms direct_eval_resolve={}:{:.1}ms seed_resolve={}:{:.1}ms",
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_METHOD_PROTO_SETUP_SOURCE_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_METHOD_PROTO_SETUP_SOURCE_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_METHOD_PROTO_SETUP_FORMAL_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_METHOD_PROTO_SETUP_FORMAL_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_METHOD_PROTO_SETUP_ENCLOSING_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_METHOD_PROTO_SETUP_ENCLOSING_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_METHOD_PROTO_SETUP_DIRECT_EVAL_SCAN_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_METHOD_PROTO_SETUP_DIRECT_EVAL_SCAN_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_METHOD_PROTO_SETUP_SEED_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_METHOD_PROTO_SETUP_SEED_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_METHOD_PROTO_SETUP_SUB_INIT_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_METHOD_PROTO_SETUP_SUB_INIT_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_METHOD_PROTO_SETUP_DIRECT_EVAL_RESOLVE_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_METHOD_PROTO_SETUP_DIRECT_EVAL_RESOLVE_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_METHOD_PROTO_SETUP_SEED_RESOLVE_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_METHOD_PROTO_SETUP_SEED_RESOLVE_NS)),
        );
        eprintln!(
            "cruft-profile-function-body-method-call-arg-function-proto-hoist-decl-proto-body-method-body-value-other: class_body_expression={}:{:.1}ms class_body_return={}:{:.1}ms class_body_block={}:{:.1}ms class_body_variable={}:{:.1}ms class_body_if={}:{:.1}ms class_body_loop={}:{:.1}ms class_body_decl={}:{:.1}ms class_body_other={}:{:.1}ms object_other_named_function={}:{:.1}ms object_other_named_class={}:{:.1}ms object_other_member={}:{:.1}ms object_other_identifier_literal={}:{:.1}ms object_other_template={}:{:.1}ms object_other_opaque={}:{:.1}ms",
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_METHOD_PROTO_BODY_STMT_EXPRESSION_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_METHOD_PROTO_BODY_STMT_EXPRESSION_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_METHOD_PROTO_BODY_STMT_RETURN_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_METHOD_PROTO_BODY_STMT_RETURN_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_METHOD_PROTO_BODY_STMT_BLOCK_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_METHOD_PROTO_BODY_STMT_BLOCK_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_METHOD_PROTO_BODY_STMT_VARIABLE_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_METHOD_PROTO_BODY_STMT_VARIABLE_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_METHOD_PROTO_BODY_STMT_IF_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_METHOD_PROTO_BODY_STMT_IF_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_METHOD_PROTO_BODY_STMT_LOOP_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_METHOD_PROTO_BODY_STMT_LOOP_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_METHOD_PROTO_BODY_STMT_DECL_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_METHOD_PROTO_BODY_STMT_DECL_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_METHOD_PROTO_BODY_STMT_OTHER_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_METHOD_PROTO_BODY_STMT_OTHER_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OBJECT_DATA_VALUE_OTHER_NAMED_FUNCTION_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OBJECT_DATA_VALUE_OTHER_NAMED_FUNCTION_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OBJECT_DATA_VALUE_OTHER_NAMED_CLASS_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OBJECT_DATA_VALUE_OTHER_NAMED_CLASS_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OBJECT_DATA_VALUE_OTHER_MEMBER_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OBJECT_DATA_VALUE_OTHER_MEMBER_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OBJECT_DATA_VALUE_OTHER_IDENTIFIER_LITERAL_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OBJECT_DATA_VALUE_OTHER_IDENTIFIER_LITERAL_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OBJECT_DATA_VALUE_OTHER_TEMPLATE_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OBJECT_DATA_VALUE_OTHER_TEMPLATE_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OBJECT_DATA_VALUE_OTHER_OPAQUE_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OBJECT_DATA_VALUE_OTHER_OPAQUE_NS)),
        );
        eprintln!(
            "cruft-profile-function-body-method-call-arg-function-proto-hoist-decl-proto-body-method-if-ident-literal: if_test={}:{:.1}ms if_patch={}:{:.1}ms if_consequent={}:{:.1}ms if_alternate={}:{:.1}ms if_clause_block={}:{:.1}ms if_clause_if={}:{:.1}ms if_clause_return={}:{:.1}ms if_clause_expr={}:{:.1}ms if_clause_other={}:{:.1}ms nested_if_test={}:{:.1}ms nested_if_patch={}:{:.1}ms nested_if_consequent={}:{:.1}ms nested_if_alternate={}:{:.1}ms object_identifier={}:{:.1}ms object_this_super_meta={}:{:.1}ms object_null_bool={}:{:.1}ms object_number_bigint={}:{:.1}ms object_string_regexp={}:{:.1}ms object_string={}:{:.1}ms object_string_cook={}:{:.1}ms object_string_intern={}:{:.1}ms object_string_emit={}:{:.1}ms object_wtf_string={}:{:.1}ms object_regexp={}:{:.1}ms object_template_object={}:{:.1}ms",
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_METHOD_PROTO_BODY_STMT_IF_TEST_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_METHOD_PROTO_BODY_STMT_IF_TEST_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_METHOD_PROTO_BODY_STMT_IF_PATCH_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_METHOD_PROTO_BODY_STMT_IF_PATCH_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_METHOD_PROTO_BODY_STMT_IF_CONSEQUENT_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_METHOD_PROTO_BODY_STMT_IF_CONSEQUENT_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_METHOD_PROTO_BODY_STMT_IF_ALTERNATE_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_METHOD_PROTO_BODY_STMT_IF_ALTERNATE_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_METHOD_PROTO_BODY_STMT_IF_CLAUSE_BLOCK_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_METHOD_PROTO_BODY_STMT_IF_CLAUSE_BLOCK_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_METHOD_PROTO_BODY_STMT_IF_CLAUSE_IF_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_METHOD_PROTO_BODY_STMT_IF_CLAUSE_IF_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_METHOD_PROTO_BODY_STMT_IF_CLAUSE_RETURN_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_METHOD_PROTO_BODY_STMT_IF_CLAUSE_RETURN_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_METHOD_PROTO_BODY_STMT_IF_CLAUSE_EXPR_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_METHOD_PROTO_BODY_STMT_IF_CLAUSE_EXPR_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_METHOD_PROTO_BODY_STMT_IF_CLAUSE_OTHER_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_METHOD_PROTO_BODY_STMT_IF_CLAUSE_OTHER_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_METHOD_PROTO_BODY_STMT_IF_CLAUSE_NESTED_IF_TEST_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_METHOD_PROTO_BODY_STMT_IF_CLAUSE_NESTED_IF_TEST_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_METHOD_PROTO_BODY_STMT_IF_CLAUSE_NESTED_IF_PATCH_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_METHOD_PROTO_BODY_STMT_IF_CLAUSE_NESTED_IF_PATCH_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_METHOD_PROTO_BODY_STMT_IF_CLAUSE_NESTED_IF_CONSEQUENT_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_METHOD_PROTO_BODY_STMT_IF_CLAUSE_NESTED_IF_CONSEQUENT_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_METHOD_PROTO_BODY_STMT_IF_CLAUSE_NESTED_IF_ALTERNATE_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_METHOD_PROTO_BODY_STMT_IF_CLAUSE_NESTED_IF_ALTERNATE_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OBJECT_DATA_VALUE_OTHER_IDENTIFIER_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OBJECT_DATA_VALUE_OTHER_IDENTIFIER_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OBJECT_DATA_VALUE_OTHER_THIS_SUPER_META_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OBJECT_DATA_VALUE_OTHER_THIS_SUPER_META_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OBJECT_DATA_VALUE_OTHER_NULL_BOOL_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OBJECT_DATA_VALUE_OTHER_NULL_BOOL_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OBJECT_DATA_VALUE_OTHER_NUMBER_BIGINT_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OBJECT_DATA_VALUE_OTHER_NUMBER_BIGINT_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OBJECT_DATA_VALUE_OTHER_STRING_REGEXP_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OBJECT_DATA_VALUE_OTHER_STRING_REGEXP_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OBJECT_DATA_VALUE_OTHER_STRING_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OBJECT_DATA_VALUE_OTHER_STRING_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OBJECT_DATA_VALUE_OTHER_STRING_COOK_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OBJECT_DATA_VALUE_OTHER_STRING_COOK_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OBJECT_DATA_VALUE_OTHER_STRING_INTERN_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OBJECT_DATA_VALUE_OTHER_STRING_INTERN_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OBJECT_DATA_VALUE_OTHER_STRING_EMIT_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OBJECT_DATA_VALUE_OTHER_STRING_EMIT_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OBJECT_DATA_VALUE_OTHER_WTF_STRING_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OBJECT_DATA_VALUE_OTHER_WTF_STRING_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OBJECT_DATA_VALUE_OTHER_REGEXP_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OBJECT_DATA_VALUE_OTHER_REGEXP_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OBJECT_DATA_VALUE_OTHER_TEMPLATE_OBJECT_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OBJECT_DATA_VALUE_OTHER_TEMPLATE_OBJECT_NS)),
        );
        eprintln!(
            "cruft-profile-function-body-method-call-arg-function-proto-hoist-decl-proto-body-nested-alt-string-cook: nested_alt_block={}:{:.1}ms nested_alt_if={}:{:.1}ms nested_alt_return={}:{:.1}ms nested_alt_expr={}:{:.1}ms nested_alt_other={}:{:.1}ms string_cook_marker={}:{:.1}ms string_cook_string={}:{:.1}ms string_cook_wtf={}:{:.1}ms",
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_METHOD_PROTO_BODY_STMT_IF_CLAUSE_NESTED_IF_ALTERNATE_BLOCK_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_METHOD_PROTO_BODY_STMT_IF_CLAUSE_NESTED_IF_ALTERNATE_BLOCK_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_METHOD_PROTO_BODY_STMT_IF_CLAUSE_NESTED_IF_ALTERNATE_IF_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_METHOD_PROTO_BODY_STMT_IF_CLAUSE_NESTED_IF_ALTERNATE_IF_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_METHOD_PROTO_BODY_STMT_IF_CLAUSE_NESTED_IF_ALTERNATE_RETURN_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_METHOD_PROTO_BODY_STMT_IF_CLAUSE_NESTED_IF_ALTERNATE_RETURN_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_METHOD_PROTO_BODY_STMT_IF_CLAUSE_NESTED_IF_ALTERNATE_EXPR_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_METHOD_PROTO_BODY_STMT_IF_CLAUSE_NESTED_IF_ALTERNATE_EXPR_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_METHOD_PROTO_BODY_STMT_IF_CLAUSE_NESTED_IF_ALTERNATE_OTHER_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_METHOD_PROTO_BODY_STMT_IF_CLAUSE_NESTED_IF_ALTERNATE_OTHER_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OBJECT_DATA_VALUE_OTHER_STRING_COOK_MARKER_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OBJECT_DATA_VALUE_OTHER_STRING_COOK_MARKER_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OBJECT_DATA_VALUE_OTHER_STRING_COOK_STRING_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OBJECT_DATA_VALUE_OTHER_STRING_COOK_STRING_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OBJECT_DATA_VALUE_OTHER_STRING_COOK_WTF_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_OBJECT_DATA_VALUE_OTHER_STRING_COOK_WTF_NS)),
        );
        eprintln!(
            "cruft-profile-function-body-method-call-arg-function-proto-hoist-decl-proto-body-nested-alt-if-phase: test={}:{:.1}ms patch={}:{:.1}ms consequent={}:{:.1}ms alternate={}:{:.1}ms",
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_METHOD_PROTO_BODY_STMT_IF_CLAUSE_NESTED_IF_ALTERNATE_IF_TEST_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_METHOD_PROTO_BODY_STMT_IF_CLAUSE_NESTED_IF_ALTERNATE_IF_TEST_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_METHOD_PROTO_BODY_STMT_IF_CLAUSE_NESTED_IF_ALTERNATE_IF_PATCH_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_METHOD_PROTO_BODY_STMT_IF_CLAUSE_NESTED_IF_ALTERNATE_IF_PATCH_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_METHOD_PROTO_BODY_STMT_IF_CLAUSE_NESTED_IF_ALTERNATE_IF_CONSEQUENT_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_METHOD_PROTO_BODY_STMT_IF_CLAUSE_NESTED_IF_ALTERNATE_IF_CONSEQUENT_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_METHOD_PROTO_BODY_STMT_IF_CLAUSE_NESTED_IF_ALTERNATE_IF_ALTERNATE_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_METHOD_PROTO_BODY_STMT_IF_CLAUSE_NESTED_IF_ALTERNATE_IF_ALTERNATE_NS)),
        );
        eprintln!(
            "cruft-profile-function-body-method-call-arg-function-proto-hoist-decl-proto-body-nested-alt-if-alternate-clause: block={}:{:.1}ms if={}:{:.1}ms return={}:{:.1}ms expr={}:{:.1}ms other={}:{:.1}ms",
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_METHOD_PROTO_BODY_STMT_IF_CLAUSE_NESTED_IF_ALTERNATE_IF_ALTERNATE_BLOCK_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_METHOD_PROTO_BODY_STMT_IF_CLAUSE_NESTED_IF_ALTERNATE_IF_ALTERNATE_BLOCK_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_METHOD_PROTO_BODY_STMT_IF_CLAUSE_NESTED_IF_ALTERNATE_IF_ALTERNATE_IF_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_METHOD_PROTO_BODY_STMT_IF_CLAUSE_NESTED_IF_ALTERNATE_IF_ALTERNATE_IF_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_METHOD_PROTO_BODY_STMT_IF_CLAUSE_NESTED_IF_ALTERNATE_IF_ALTERNATE_RETURN_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_METHOD_PROTO_BODY_STMT_IF_CLAUSE_NESTED_IF_ALTERNATE_IF_ALTERNATE_RETURN_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_METHOD_PROTO_BODY_STMT_IF_CLAUSE_NESTED_IF_ALTERNATE_IF_ALTERNATE_EXPR_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_METHOD_PROTO_BODY_STMT_IF_CLAUSE_NESTED_IF_ALTERNATE_IF_ALTERNATE_EXPR_NS)),
            cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_METHOD_PROTO_BODY_STMT_IF_CLAUSE_NESTED_IF_ALTERNATE_IF_ALTERNATE_OTHER_CALLS),
            ms(cp::read(&cp::FUNCTION_BODY_METHOD_CALL_ARG_FUNCTION_PROTO_HOIST_DECL_PROTO_BODY_CLASS_METHOD_PROTO_BODY_STMT_IF_CLAUSE_NESTED_IF_ALTERNATE_IF_ALTERNATE_OTHER_NS)),
        );
        eprintln!(
            "cruft-profile-arrow: normalize={}:{:.1}ms proto={}:{:.1}ms capture_clone={}:{:.1}ms intern={}:{:.1}ms make_emit={}:{:.1}ms",
            cp::read(&cp::ARROW_BODY_NORMALIZE_CALLS),
            ms(cp::read(&cp::ARROW_BODY_NORMALIZE_NS)),
            cp::read(&cp::ARROW_PROTO_CALLS),
            ms(cp::read(&cp::ARROW_PROTO_NS)),
            cp::read(&cp::ARROW_CAPTURE_CLONE_CALLS),
            ms(cp::read(&cp::ARROW_CAPTURE_CLONE_NS)),
            cp::read(&cp::ARROW_INTERN_CALLS),
            ms(cp::read(&cp::ARROW_INTERN_NS)),
            cp::read(&cp::ARROW_MAKE_EMIT_CALLS),
            ms(cp::read(&cp::ARROW_MAKE_EMIT_NS)),
        );
        eprintln!(
            "cruft-profile-arrow-proto: setup={}:{:.1}ms params={}:{:.1}ms prealloc={}:{:.1}ms hoist={}:{:.1}ms body={}:{:.1}ms assemble={}:{:.1}ms",
            cp::read(&cp::ARROW_PROTO_SETUP_CALLS),
            ms(cp::read(&cp::ARROW_PROTO_SETUP_NS)),
            cp::read(&cp::ARROW_PROTO_PARAMS_CALLS),
            ms(cp::read(&cp::ARROW_PROTO_PARAMS_NS)),
            cp::read(&cp::ARROW_PROTO_PREALLOC_CALLS),
            ms(cp::read(&cp::ARROW_PROTO_PREALLOC_NS)),
            cp::read(&cp::ARROW_PROTO_HOIST_CALLS),
            ms(cp::read(&cp::ARROW_PROTO_HOIST_NS)),
            cp::read(&cp::ARROW_PROTO_BODY_CALLS),
            ms(cp::read(&cp::ARROW_PROTO_BODY_NS)),
            cp::read(&cp::ARROW_PROTO_ASSEMBLE_CALLS),
            ms(cp::read(&cp::ARROW_PROTO_ASSEMBLE_NS)),
        );
        eprintln!(
            "cruft-profile-arrow-body-stmt: expression={}:{:.1}ms return={}:{:.1}ms block={}:{:.1}ms variable={}:{:.1}ms throw={}:{:.1}ms if={}:{:.1}ms loop={}:{:.1}ms try={}:{:.1}ms switch={}:{:.1}ms decl={}:{:.1}ms control={}:{:.1}ms other={}:{:.1}ms",
            cp::read(&cp::ARROW_PROTO_BODY_STMT_EXPRESSION_CALLS),
            ms(cp::read(&cp::ARROW_PROTO_BODY_STMT_EXPRESSION_NS)),
            cp::read(&cp::ARROW_PROTO_BODY_STMT_RETURN_CALLS),
            ms(cp::read(&cp::ARROW_PROTO_BODY_STMT_RETURN_NS)),
            cp::read(&cp::ARROW_PROTO_BODY_STMT_BLOCK_CALLS),
            ms(cp::read(&cp::ARROW_PROTO_BODY_STMT_BLOCK_NS)),
            cp::read(&cp::ARROW_PROTO_BODY_STMT_VARIABLE_CALLS),
            ms(cp::read(&cp::ARROW_PROTO_BODY_STMT_VARIABLE_NS)),
            cp::read(&cp::ARROW_PROTO_BODY_STMT_THROW_CALLS),
            ms(cp::read(&cp::ARROW_PROTO_BODY_STMT_THROW_NS)),
            cp::read(&cp::ARROW_PROTO_BODY_STMT_IF_CALLS),
            ms(cp::read(&cp::ARROW_PROTO_BODY_STMT_IF_NS)),
            cp::read(&cp::ARROW_PROTO_BODY_STMT_LOOP_CALLS),
            ms(cp::read(&cp::ARROW_PROTO_BODY_STMT_LOOP_NS)),
            cp::read(&cp::ARROW_PROTO_BODY_STMT_TRY_CALLS),
            ms(cp::read(&cp::ARROW_PROTO_BODY_STMT_TRY_NS)),
            cp::read(&cp::ARROW_PROTO_BODY_STMT_SWITCH_CALLS),
            ms(cp::read(&cp::ARROW_PROTO_BODY_STMT_SWITCH_NS)),
            cp::read(&cp::ARROW_PROTO_BODY_STMT_DECL_CALLS),
            ms(cp::read(&cp::ARROW_PROTO_BODY_STMT_DECL_NS)),
            cp::read(&cp::ARROW_PROTO_BODY_STMT_CONTROL_CALLS),
            ms(cp::read(&cp::ARROW_PROTO_BODY_STMT_CONTROL_NS)),
            cp::read(&cp::ARROW_PROTO_BODY_STMT_OTHER_CALLS),
            ms(cp::read(&cp::ARROW_PROTO_BODY_STMT_OTHER_NS)),
        );
        eprintln!(
            "cruft-profile-arrow-return: arg={}:{:.1}ms await={}:{:.1}ms finalizer={}:{:.1}ms iter_close={}:{:.1}ms emit={}:{:.1}ms",
            cp::read(&cp::ARROW_BODY_RETURN_ARG_CALLS),
            ms(cp::read(&cp::ARROW_BODY_RETURN_ARG_NS)),
            cp::read(&cp::ARROW_BODY_RETURN_AWAIT_CALLS),
            ms(cp::read(&cp::ARROW_BODY_RETURN_AWAIT_NS)),
            cp::read(&cp::ARROW_BODY_RETURN_FINALIZER_CALLS),
            ms(cp::read(&cp::ARROW_BODY_RETURN_FINALIZER_NS)),
            cp::read(&cp::ARROW_BODY_RETURN_ITER_CLOSE_CALLS),
            ms(cp::read(&cp::ARROW_BODY_RETURN_ITER_CLOSE_NS)),
            cp::read(&cp::ARROW_BODY_RETURN_EMIT_CALLS),
            ms(cp::read(&cp::ARROW_BODY_RETURN_EMIT_NS)),
        );
        eprintln!(
            "cruft-profile-arrow-return-arg: function={}:{:.1}ms arrow={}:{:.1}ms class={}:{:.1}ms paren={}:{:.1}ms literal={}:{:.1}ms identifier={}:{:.1}ms member={}:{:.1}ms call={}:{:.1}ms object_array={}:{:.1}ms operator={}:{:.1}ms other={}:{:.1}ms",
            cp::read(&cp::ARROW_RETURN_ARG_FUNCTION_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_ARG_FUNCTION_NS)),
            cp::read(&cp::ARROW_RETURN_ARG_ARROW_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_ARG_ARROW_NS)),
            cp::read(&cp::ARROW_RETURN_ARG_CLASS_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_ARG_CLASS_NS)),
            cp::read(&cp::ARROW_RETURN_ARG_PAREN_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_ARG_PAREN_NS)),
            cp::read(&cp::ARROW_RETURN_ARG_LITERAL_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_ARG_LITERAL_NS)),
            cp::read(&cp::ARROW_RETURN_ARG_IDENTIFIER_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_ARG_IDENTIFIER_NS)),
            cp::read(&cp::ARROW_RETURN_ARG_MEMBER_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_ARG_MEMBER_NS)),
            cp::read(&cp::ARROW_RETURN_ARG_CALL_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_ARG_CALL_NS)),
            cp::read(&cp::ARROW_RETURN_ARG_OBJECT_ARRAY_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_ARG_OBJECT_ARRAY_NS)),
            cp::read(&cp::ARROW_RETURN_ARG_OPERATOR_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_ARG_OPERATOR_NS)),
            cp::read(&cp::ARROW_RETURN_ARG_OTHER_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_ARG_OTHER_NS)),
        );
        eprintln!(
            "cruft-profile-arrow-return-arg-other: named_function={}:{:.1}ms named_class={}:{:.1}ms template_literal={}:{:.1}ms opaque={}:{:.1}ms",
            cp::read(&cp::ARROW_RETURN_ARG_OTHER_NAMED_FUNCTION_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_ARG_OTHER_NAMED_FUNCTION_NS)),
            cp::read(&cp::ARROW_RETURN_ARG_OTHER_NAMED_CLASS_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_ARG_OTHER_NAMED_CLASS_NS)),
            cp::read(&cp::ARROW_RETURN_ARG_OTHER_TEMPLATE_LITERAL_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_ARG_OTHER_TEMPLATE_LITERAL_NS)),
            cp::read(&cp::ARROW_RETURN_ARG_OTHER_OPAQUE_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_ARG_OTHER_OPAQUE_NS)),
        );
        eprintln!(
            "cruft-profile-arrow-return-named-class: self_name={}:{:.1}ms heritage_proto={}:{:.1}ms key_temps={}:{:.1}ms field_synth={}:{:.1}ms ctor_proto={}:{:.1}ms computed_keys={}:{:.1}ms static_install={}:{:.1}ms member_pass={}:{:.1}ms final={}:{:.1}ms",
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_SELF_NAME_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_SELF_NAME_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_HERITAGE_PROTO_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_HERITAGE_PROTO_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_KEY_TEMPS_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_KEY_TEMPS_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_FIELD_SYNTH_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_FIELD_SYNTH_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_CTOR_PROTO_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_CTOR_PROTO_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_COMPUTED_KEYS_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_COMPUTED_KEYS_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_STATIC_INSTALL_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_STATIC_INSTALL_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_MEMBER_PASS_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_MEMBER_PASS_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_FINAL_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_FINAL_NS)),
        );
        eprintln!(
            "cruft-profile-arrow-return-named-class-member: method_proto={}:{:.1}ms method_install={}:{:.1}ms static_field={}:{:.1}ms static_field_init_proto={}:{:.1}ms static_block_proto={}:{:.1}ms static_block_emit={}:{:.1}ms",
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_PROTO_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_PROTO_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_INSTALL_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_INSTALL_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_STATIC_FIELD_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_STATIC_FIELD_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_STATIC_FIELD_INIT_PROTO_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_STATIC_FIELD_INIT_PROTO_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_STATIC_BLOCK_PROTO_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_STATIC_BLOCK_PROTO_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_STATIC_BLOCK_EMIT_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_STATIC_BLOCK_EMIT_NS)),
        );
        eprintln!(
            "cruft-profile-arrow-return-named-class-method-proto: setup={}:{:.1}ms params={}:{:.1}ms prealloc={}:{:.1}ms hoist={}:{:.1}ms body={}:{:.1}ms assemble={}:{:.1}ms",
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_PROTO_SETUP_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_PROTO_SETUP_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_PROTO_PARAMS_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_PROTO_PARAMS_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_PROTO_PREALLOC_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_PROTO_PREALLOC_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_PROTO_HOIST_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_PROTO_HOIST_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_PROTO_BODY_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_PROTO_BODY_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_PROTO_ASSEMBLE_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_PROTO_ASSEMBLE_NS)),
        );
        eprintln!(
            "cruft-profile-arrow-return-named-class-method-proto-body-phase: profile_setup={}:{:.1}ms compile_stmt={}:{:.1}ms profile_account={}:{:.1}ms",
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_PROTO_BODY_PROFILE_SETUP_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_PROTO_BODY_PROFILE_SETUP_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_PROTO_BODY_COMPILE_STMT_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_PROTO_BODY_COMPILE_STMT_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_PROTO_BODY_PROFILE_ACCOUNT_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_PROTO_BODY_PROFILE_ACCOUNT_NS)),
        );
        eprintln!(
            "cruft-profile-arrow-return-named-class-method-proto-setup: source={}:{:.1}ms formal={}:{:.1}ms enclosing={}:{:.1}ms direct_eval_scan={}:{:.1}ms sub_init={}:{:.1}ms seed_resolve={}:{:.1}ms seed={}:{:.1}ms direct_eval_resolve={}:{:.1}ms",
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_PROTO_SETUP_SOURCE_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_PROTO_SETUP_SOURCE_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_PROTO_SETUP_FORMAL_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_PROTO_SETUP_FORMAL_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_PROTO_SETUP_ENCLOSING_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_PROTO_SETUP_ENCLOSING_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_PROTO_SETUP_DIRECT_EVAL_SCAN_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_PROTO_SETUP_DIRECT_EVAL_SCAN_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_PROTO_SETUP_SUB_INIT_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_PROTO_SETUP_SUB_INIT_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_PROTO_SETUP_SEED_RESOLVE_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_PROTO_SETUP_SEED_RESOLVE_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_PROTO_SETUP_SEED_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_PROTO_SETUP_SEED_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_PROTO_SETUP_DIRECT_EVAL_RESOLVE_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_PROTO_SETUP_DIRECT_EVAL_RESOLVE_NS)),
        );
        eprintln!(
            "cruft-profile-arrow-return-named-class-method-body-stmt: expression={}:{:.1}ms return={}:{:.1}ms block={}:{:.1}ms variable={}:{:.1}ms throw={}:{:.1}ms if={}:{:.1}ms loop={}:{:.1}ms try={}:{:.1}ms switch={}:{:.1}ms decl={}:{:.1}ms control={}:{:.1}ms other={}:{:.1}ms",
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_BODY_STMT_EXPRESSION_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_BODY_STMT_EXPRESSION_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_BODY_STMT_RETURN_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_BODY_STMT_RETURN_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_BODY_STMT_BLOCK_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_BODY_STMT_BLOCK_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_BODY_STMT_VARIABLE_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_BODY_STMT_VARIABLE_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_BODY_STMT_THROW_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_BODY_STMT_THROW_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_BODY_STMT_IF_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_BODY_STMT_IF_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_BODY_STMT_LOOP_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_BODY_STMT_LOOP_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_BODY_STMT_TRY_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_BODY_STMT_TRY_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_BODY_STMT_SWITCH_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_BODY_STMT_SWITCH_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_BODY_STMT_DECL_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_BODY_STMT_DECL_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_BODY_STMT_CONTROL_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_BODY_STMT_CONTROL_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_BODY_STMT_OTHER_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_BODY_STMT_OTHER_NS)),
        );
        eprintln!(
            "cruft-profile-arrow-return-named-class-method-if: test={}:{:.1}ms consequent={}:{:.1}ms alternate={}:{:.1}ms patch={}:{:.1}ms",
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_TEST_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_TEST_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONSEQUENT_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONSEQUENT_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_ALTERNATE_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_ALTERNATE_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_PATCH_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_PATCH_NS)),
        );
        eprintln!(
            "cruft-profile-arrow-return-named-class-method-if-branch-clause: consequent_expression={}:{:.1}ms consequent_return={}:{:.1}ms consequent_block={}:{:.1}ms consequent_variable={}:{:.1}ms consequent_throw={}:{:.1}ms consequent_if={}:{:.1}ms consequent_other={}:{:.1}ms alternate_expression={}:{:.1}ms alternate_return={}:{:.1}ms alternate_block={}:{:.1}ms alternate_variable={}:{:.1}ms alternate_throw={}:{:.1}ms alternate_if={}:{:.1}ms alternate_other={}:{:.1}ms",
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_BRANCH_CONS_EXPRESSION_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_BRANCH_CONS_EXPRESSION_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_BRANCH_CONS_RETURN_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_BRANCH_CONS_RETURN_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_BRANCH_CONS_BLOCK_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_BRANCH_CONS_BLOCK_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_BRANCH_CONS_VARIABLE_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_BRANCH_CONS_VARIABLE_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_BRANCH_CONS_THROW_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_BRANCH_CONS_THROW_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_BRANCH_CONS_IF_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_BRANCH_CONS_IF_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_BRANCH_CONS_OTHER_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_BRANCH_CONS_OTHER_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_BRANCH_ALT_EXPRESSION_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_BRANCH_ALT_EXPRESSION_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_BRANCH_ALT_RETURN_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_BRANCH_ALT_RETURN_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_BRANCH_ALT_BLOCK_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_BRANCH_ALT_BLOCK_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_BRANCH_ALT_VARIABLE_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_BRANCH_ALT_VARIABLE_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_BRANCH_ALT_THROW_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_BRANCH_ALT_THROW_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_BRANCH_ALT_IF_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_BRANCH_ALT_IF_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_BRANCH_ALT_OTHER_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_BRANCH_ALT_OTHER_NS)),
        );
        eprintln!(
            "cruft-profile-arrow-return-named-class-method-if-consequent-stmt: expression={}:{:.1}ms return={}:{:.1}ms block={}:{:.1}ms variable={}:{:.1}ms if={}:{:.1}ms other={}:{:.1}ms",
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_STMT_EXPRESSION_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_STMT_EXPRESSION_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_STMT_RETURN_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_STMT_RETURN_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_STMT_BLOCK_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_STMT_BLOCK_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_STMT_VARIABLE_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_STMT_VARIABLE_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_STMT_IF_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_STMT_IF_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_STMT_OTHER_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_STMT_OTHER_NS)),
        );
        eprintln!(
            "cruft-profile-arrow-return-named-class-method-if-consequent-block: prescan={}:{:.1}ms reset_seed={}:{:.1}ms prebind={}:{:.1}ms body={}:{:.1}ms rename={}:{:.1}ms",
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_PRESCAN_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_PRESCAN_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_RESET_SEED_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_RESET_SEED_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_PREBIND_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_PREBIND_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_RENAME_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_RENAME_NS)),
        );
        eprintln!(
            "cruft-profile-arrow-return-named-class-method-if-consequent-block-body-stmt: expression={}:{:.1}ms return={}:{:.1}ms block={}:{:.1}ms variable={}:{:.1}ms throw={}:{:.1}ms if={}:{:.1}ms loop={}:{:.1}ms try={}:{:.1}ms switch={}:{:.1}ms decl={}:{:.1}ms control={}:{:.1}ms other={}:{:.1}ms",
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_STMT_EXPRESSION_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_STMT_EXPRESSION_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_STMT_RETURN_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_STMT_RETURN_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_STMT_BLOCK_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_STMT_BLOCK_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_STMT_VARIABLE_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_STMT_VARIABLE_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_STMT_THROW_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_STMT_THROW_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_STMT_IF_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_STMT_IF_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_STMT_LOOP_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_STMT_LOOP_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_STMT_TRY_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_STMT_TRY_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_STMT_SWITCH_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_STMT_SWITCH_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_STMT_DECL_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_STMT_DECL_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_STMT_CONTROL_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_STMT_CONTROL_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_STMT_OTHER_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_STMT_OTHER_NS)),
        );
        eprintln!(
            "cruft-profile-arrow-return-named-class-method-if-consequent-block-body-if: test={}:{:.1}ms consequent={}:{:.1}ms alternate={}:{:.1}ms patch={}:{:.1}ms",
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_TEST_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_TEST_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONSEQUENT_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONSEQUENT_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_ALTERNATE_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_ALTERNATE_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_PATCH_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_PATCH_NS)),
        );
        eprintln!(
            "cruft-profile-arrow-return-named-class-method-if-consequent-block-body-if-all: test={}:{:.1}ms consequent={}:{:.1}ms alternate={}:{:.1}ms",
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_ALL_TEST_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_ALL_TEST_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_ALL_CONSEQUENT_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_ALL_CONSEQUENT_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_ALL_ALTERNATE_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_ALL_ALTERNATE_NS)),
        );
        eprintln!(
            "cruft-profile-arrow-return-named-class-method-if-consequent-block-body-if-consequent-stmt: expression={}:{:.1}ms return={}:{:.1}ms block={}:{:.1}ms variable={}:{:.1}ms if={}:{:.1}ms other={}:{:.1}ms",
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_STMT_EXPRESSION_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_STMT_EXPRESSION_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_STMT_RETURN_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_STMT_RETURN_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_STMT_BLOCK_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_STMT_BLOCK_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_STMT_VARIABLE_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_STMT_VARIABLE_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_STMT_IF_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_STMT_IF_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_STMT_OTHER_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_STMT_OTHER_NS)),
        );
        eprintln!(
            "cruft-profile-arrow-return-named-class-method-if-consequent-block-body-if-consequent-block: total={}:{:.1}ms enter={}:{:.1}ms setup={}:{:.1}ms prescan={}:{:.1}ms after_prescan={}:{:.1}ms reset_seed={}:{:.1}ms after_reset_seed={}:{:.1}ms prebind={}:{:.1}ms before_body={}:{:.1}ms body={}:{:.1}ms through_body={}:{:.1}ms rename={}:{:.1}ms",
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_TOTAL_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_TOTAL_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_ENTER_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_ENTER_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_SETUP_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_SETUP_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_PRESCAN_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_PRESCAN_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_AFTER_PRESCAN_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_AFTER_PRESCAN_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_RESET_SEED_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_RESET_SEED_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_AFTER_RESET_SEED_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_AFTER_RESET_SEED_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_PREBIND_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_PREBIND_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BEFORE_BODY_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BEFORE_BODY_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_THROUGH_BODY_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_THROUGH_BODY_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_RENAME_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_RENAME_NS)),
        );
        eprintln!(
            "cruft-profile-arrow-return-named-class-method-if-consequent-block-body-if-consequent-block-body-stmt: expression={}:{:.1}ms return={}:{:.1}ms block={}:{:.1}ms variable={}:{:.1}ms throw={}:{:.1}ms if={}:{:.1}ms loop={}:{:.1}ms try={}:{:.1}ms switch={}:{:.1}ms decl={}:{:.1}ms control={}:{:.1}ms other={}:{:.1}ms",
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_STMT_EXPRESSION_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_STMT_EXPRESSION_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_STMT_RETURN_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_STMT_RETURN_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_STMT_BLOCK_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_STMT_BLOCK_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_STMT_VARIABLE_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_STMT_VARIABLE_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_STMT_THROW_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_STMT_THROW_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_STMT_IF_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_STMT_IF_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_STMT_LOOP_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_STMT_LOOP_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_STMT_TRY_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_STMT_TRY_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_STMT_SWITCH_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_STMT_SWITCH_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_STMT_DECL_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_STMT_DECL_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_STMT_CONTROL_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_STMT_CONTROL_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_STMT_OTHER_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_STMT_OTHER_NS)),
        );
        eprintln!(
            "cruft-profile-arrow-return-named-class-method-if-consequent-block-body-if-consequent-block-body-stmt-if-consequent-clause: block={}:{:.1}ms if={}:{:.1}ms return={}:{:.1}ms expression={}:{:.1}ms other={}:{:.1}ms",
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_STMT_IF_CONS_CLAUSE_BLOCK_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_STMT_IF_CONS_CLAUSE_BLOCK_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_STMT_IF_CONS_CLAUSE_IF_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_STMT_IF_CONS_CLAUSE_IF_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_STMT_IF_CONS_CLAUSE_RETURN_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_STMT_IF_CONS_CLAUSE_RETURN_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_STMT_IF_CONS_CLAUSE_EXPR_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_STMT_IF_CONS_CLAUSE_EXPR_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_STMT_IF_CONS_CLAUSE_OTHER_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_STMT_IF_CONS_CLAUSE_OTHER_NS)),
        );
        eprintln!(
            "cruft-profile-arrow-return-named-class-method-if-consequent-block-body-if-consequent-block-body-stmt-if-consequent-clause-all: block={}:{:.1}ms if={}:{:.1}ms return={}:{:.1}ms expression={}:{:.1}ms other={}:{:.1}ms",
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_STMT_IF_CONS_CLAUSE_ALL_BLOCK_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_STMT_IF_CONS_CLAUSE_ALL_BLOCK_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_STMT_IF_CONS_CLAUSE_ALL_IF_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_STMT_IF_CONS_CLAUSE_ALL_IF_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_STMT_IF_CONS_CLAUSE_ALL_RETURN_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_STMT_IF_CONS_CLAUSE_ALL_RETURN_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_STMT_IF_CONS_CLAUSE_ALL_EXPR_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_STMT_IF_CONS_CLAUSE_ALL_EXPR_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_STMT_IF_CONS_CLAUSE_ALL_OTHER_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_STMT_IF_CONS_CLAUSE_ALL_OTHER_NS)),
        );
        eprintln!(
            "cruft-profile-arrow-return-named-class-method-if-consequent-block-body-if-consequent-block-body-stmt-if-consequent-clause-block-all: total={}:{:.1}ms prescan={}:{:.1}ms reset_seed={}:{:.1}ms prebind={}:{:.1}ms body={}:{:.1}ms rename={}:{:.1}ms",
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_STMT_IF_CONS_CLAUSE_BLOCK_ALL_TOTAL_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_STMT_IF_CONS_CLAUSE_BLOCK_ALL_TOTAL_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_STMT_IF_CONS_CLAUSE_BLOCK_ALL_PRESCAN_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_STMT_IF_CONS_CLAUSE_BLOCK_ALL_PRESCAN_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_STMT_IF_CONS_CLAUSE_BLOCK_ALL_RESET_SEED_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_STMT_IF_CONS_CLAUSE_BLOCK_ALL_RESET_SEED_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_STMT_IF_CONS_CLAUSE_BLOCK_ALL_PREBIND_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_STMT_IF_CONS_CLAUSE_BLOCK_ALL_PREBIND_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_STMT_IF_CONS_CLAUSE_BLOCK_ALL_BODY_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_STMT_IF_CONS_CLAUSE_BLOCK_ALL_BODY_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_STMT_IF_CONS_CLAUSE_BLOCK_ALL_RENAME_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_STMT_IF_CONS_CLAUSE_BLOCK_ALL_RENAME_NS)),
        );
        eprintln!(
            "cruft-profile-arrow-return-named-class-method-if-consequent-block-body-if-consequent-block-body-stmt-if-consequent-clause-block-all-body-stmt: expression={}:{:.1}ms return={}:{:.1}ms variable={}:{:.1}ms if={}:{:.1}ms other={}:{:.1}ms",
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_STMT_IF_CONS_CLAUSE_BLOCK_ALL_BODY_STMT_EXPRESSION_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_STMT_IF_CONS_CLAUSE_BLOCK_ALL_BODY_STMT_EXPRESSION_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_STMT_IF_CONS_CLAUSE_BLOCK_ALL_BODY_STMT_RETURN_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_STMT_IF_CONS_CLAUSE_BLOCK_ALL_BODY_STMT_RETURN_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_STMT_IF_CONS_CLAUSE_BLOCK_ALL_BODY_STMT_VARIABLE_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_STMT_IF_CONS_CLAUSE_BLOCK_ALL_BODY_STMT_VARIABLE_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_STMT_IF_CONS_CLAUSE_BLOCK_ALL_BODY_STMT_IF_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_STMT_IF_CONS_CLAUSE_BLOCK_ALL_BODY_STMT_IF_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_STMT_IF_CONS_CLAUSE_BLOCK_ALL_BODY_STMT_OTHER_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_STMT_IF_CONS_CLAUSE_BLOCK_ALL_BODY_STMT_OTHER_NS)),
        );
        eprintln!(
            "cruft-profile-arrow-return-named-class-method-if-consequent-block-body-if-consequent-block-body-stmt-if-all: test={}:{:.1}ms patch={}:{:.1}ms consequent={}:{:.1}ms alternate={}:{:.1}ms",
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_STMT_IF_ALL_TEST_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_STMT_IF_ALL_TEST_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_STMT_IF_ALL_PATCH_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_STMT_IF_ALL_PATCH_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_STMT_IF_ALL_CONSEQUENT_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_STMT_IF_ALL_CONSEQUENT_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_STMT_IF_ALL_ALTERNATE_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_STMT_IF_ALL_ALTERNATE_NS)),
        );
        eprintln!(
            "cruft-profile-arrow-return-named-class-method-if-consequent-block-body-if-consequent-block-body-expr: function={}:{:.1}ms arrow={}:{:.1}ms class={}:{:.1}ms paren={}:{:.1}ms literal={}:{:.1}ms identifier={}:{:.1}ms member={}:{:.1}ms call_new={}:{:.1}ms object_array={}:{:.1}ms operator={}:{:.1}ms other={}:{:.1}ms",
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_EXPR_FUNCTION_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_EXPR_FUNCTION_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_EXPR_ARROW_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_EXPR_ARROW_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_EXPR_CLASS_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_EXPR_CLASS_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_EXPR_PAREN_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_EXPR_PAREN_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_EXPR_LITERAL_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_EXPR_LITERAL_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_EXPR_IDENTIFIER_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_EXPR_IDENTIFIER_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_EXPR_MEMBER_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_EXPR_MEMBER_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_EXPR_CALL_NEW_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_EXPR_CALL_NEW_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_EXPR_OBJECT_ARRAY_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_EXPR_OBJECT_ARRAY_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_EXPR_OPERATOR_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_EXPR_OPERATOR_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_EXPR_OTHER_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_EXPR_OTHER_NS)),
        );
        eprintln!(
            "cruft-profile-arrow-return-named-class-method-if-consequent-block-body-if-consequent-block-body-call-new: call={}:{:.1}ms new={}:{:.1}ms",
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_NEW_CALL_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_NEW_CALL_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_NEW_NEW_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_NEW_NEW_NS)),
        );
        eprintln!(
            "cruft-profile-arrow-return-named-class-method-if-consequent-block-body-if-consequent-block-body-call-callee: template_helper={}:{:.1}ms member={}:{:.1}ms identifier={}:{:.1}ms other={}:{:.1}ms",
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_CALLEE_TEMPLATE_HELPER_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_CALLEE_TEMPLATE_HELPER_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_CALLEE_MEMBER_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_CALLEE_MEMBER_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_CALLEE_IDENTIFIER_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_CALLEE_IDENTIFIER_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_CALLEE_OTHER_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_CALLEE_OTHER_NS)),
        );
        eprintln!(
            "cruft-profile-arrow-return-named-class-method-if-consequent-block-body-if-consequent-block-body-call-args: none={}:{:.1}ms fixed={}:{:.1}ms spread_wide={}:{:.1}ms",
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_ARGS_NONE_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_ARGS_NONE_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_ARGS_FIXED_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_ARGS_FIXED_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_ARGS_SPREAD_WIDE_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_ARGS_SPREAD_WIDE_NS)),
        );
        eprintln!(
            "cruft-profile-arrow-return-named-class-method-if-consequent-block-body-if-consequent-block-body-call-member-property: identifier={}:{:.1}ms computed={}:{:.1}ms private={}:{:.1}ms",
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_PROPERTY_IDENTIFIER_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_PROPERTY_IDENTIFIER_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_PROPERTY_COMPUTED_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_PROPERTY_COMPUTED_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_PROPERTY_PRIVATE_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_PROPERTY_PRIVATE_NS)),
        );
        eprintln!(
            "cruft-profile-arrow-return-named-class-method-if-consequent-block-body-if-consequent-block-body-call-member-optional: none={}:{:.1}ms member={}:{:.1}ms call={}:{:.1}ms both={}:{:.1}ms",
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_OPTIONAL_NONE_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_OPTIONAL_NONE_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_OPTIONAL_MEMBER_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_OPTIONAL_MEMBER_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_OPTIONAL_CALL_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_OPTIONAL_CALL_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_OPTIONAL_BOTH_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_OPTIONAL_BOTH_NS)),
        );
        eprintln!(
            "cruft-profile-arrow-return-named-class-method-if-consequent-block-body-if-consequent-block-body-call-member-args-count: none={}:{:.1}ms fixed={}:{:.1}ms",
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARGS_NONE_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARGS_NONE_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARGS_FIXED_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARGS_FIXED_NS)),
        );
        eprintln!(
            "cruft-profile-arrow-return-named-class-method-if-consequent-block-body-if-consequent-block-body-call-member-phase: receiver={}:{:.1}ms lookup={}:{:.1}ms args={}:{:.1}ms emit={}:{:.1}ms",
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_RECEIVER_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_RECEIVER_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_LOOKUP_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_LOOKUP_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARGS_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARGS_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_EMIT_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_EMIT_NS)),
        );
        eprintln!(
            "cruft-profile-arrow-return-named-class-method-if-consequent-block-body-if-consequent-block-body-call-member-arg-expr: function={}:{:.1}ms arrow={}:{:.1}ms class={}:{:.1}ms paren={}:{:.1}ms literal={}:{:.1}ms identifier={}:{:.1}ms member={}:{:.1}ms call_new={}:{:.1}ms object_array={}:{:.1}ms operator={}:{:.1}ms other={}:{:.1}ms",
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_EXPR_FUNCTION_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_EXPR_FUNCTION_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_EXPR_ARROW_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_EXPR_ARROW_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_EXPR_CLASS_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_EXPR_CLASS_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_EXPR_PAREN_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_EXPR_PAREN_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_EXPR_LITERAL_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_EXPR_LITERAL_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_EXPR_IDENTIFIER_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_EXPR_IDENTIFIER_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_EXPR_MEMBER_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_EXPR_MEMBER_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_EXPR_CALL_NEW_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_EXPR_CALL_NEW_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_EXPR_OBJECT_ARRAY_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_EXPR_OBJECT_ARRAY_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_EXPR_OPERATOR_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_EXPR_OPERATOR_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_EXPR_OTHER_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_EXPR_OTHER_NS)),
        );
        eprintln!(
            "cruft-profile-arrow-return-named-class-method-if-consequent-block-body-if-consequent-block-body-call-member-arg-member-property: identifier={}:{:.1}ms computed={}:{:.1}ms private={}:{:.1}ms",
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_PROPERTY_IDENTIFIER_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_PROPERTY_IDENTIFIER_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_PROPERTY_COMPUTED_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_PROPERTY_COMPUTED_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_PROPERTY_PRIVATE_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_PROPERTY_PRIVATE_NS)),
        );
        eprintln!(
            "cruft-profile-arrow-return-named-class-method-if-consequent-block-body-if-consequent-block-body-call-member-arg-member-optional: false={}:{:.1}ms true={}:{:.1}ms",
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OPTIONAL_FALSE_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OPTIONAL_FALSE_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OPTIONAL_TRUE_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OPTIONAL_TRUE_NS)),
        );
        eprintln!(
            "cruft-profile-arrow-return-named-class-method-if-consequent-block-body-if-consequent-block-body-call-member-arg-member-phase: object={}:{:.1}ms property={}:{:.1}ms chain_patch={}:{:.1}ms",
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_PROPERTY_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_PROPERTY_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_CHAIN_PATCH_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_CHAIN_PATCH_NS)),
        );
        eprintln!(
            "cruft-profile-arrow-return-named-class-method-if-consequent-block-body-if-consequent-block-body-call-member-arg-member-object-expr: function={}:{:.1}ms arrow={}:{:.1}ms class={}:{:.1}ms paren={}:{:.1}ms literal={}:{:.1}ms identifier={}:{:.1}ms member={}:{:.1}ms call_new={}:{:.1}ms object_array={}:{:.1}ms operator={}:{:.1}ms other={}:{:.1}ms",
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_EXPR_FUNCTION_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_EXPR_FUNCTION_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_EXPR_ARROW_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_EXPR_ARROW_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_EXPR_CLASS_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_EXPR_CLASS_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_EXPR_PAREN_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_EXPR_PAREN_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_EXPR_LITERAL_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_EXPR_LITERAL_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_EXPR_IDENTIFIER_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_EXPR_IDENTIFIER_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_EXPR_MEMBER_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_EXPR_MEMBER_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_EXPR_CALL_NEW_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_EXPR_CALL_NEW_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_EXPR_OBJECT_ARRAY_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_EXPR_OBJECT_ARRAY_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_EXPR_OPERATOR_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_EXPR_OPERATOR_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_EXPR_OTHER_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_EXPR_OTHER_NS)),
        );
        eprintln!(
            "cruft-profile-arrow-return-named-class-method-if-consequent-block-body-if-consequent-block-body-call-member-arg-member-object-identifier-outcome: with={}:{:.1}ms local={}:{:.1}ms upvalue={}:{:.1}ms global={}:{:.1}ms",
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_WITH_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_WITH_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_LOCAL_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_LOCAL_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_GLOBAL_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_GLOBAL_NS)),
        );
        eprintln!(
            "cruft-profile-arrow-return-named-class-method-if-consequent-block-body-if-consequent-block-body-call-member-arg-member-object-identifier-phase: with_check={}:{:.1}ms resolve_local={}:{:.1}ms resolve_upvalue={}:{:.1}ms intern_global={}:{:.1}ms emit={}:{:.1}ms",
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_WITH_CHECK_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_WITH_CHECK_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_RESOLVE_LOCAL_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_RESOLVE_LOCAL_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_RESOLVE_UPVALUE_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_RESOLVE_UPVALUE_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_INTERN_GLOBAL_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_INTERN_GLOBAL_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_EMIT_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_EMIT_NS)),
        );
        eprintln!(
            "cruft-profile-arrow-return-named-class-method-if-consequent-block-body-if-consequent-block-body-call-member-arg-member-object-identifier-upvalue: source_local={}:{:.1}ms source_upvalue={}:{:.1}ms slot_created={}:{:.1}ms slot_reused={}:{:.1}ms self_name_true={}:{:.1}ms self_name_false={}:{:.1}ms",
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_SOURCE_LOCAL_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_SOURCE_LOCAL_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_SOURCE_UPVALUE_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_SOURCE_UPVALUE_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_SLOT_CREATED_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_SLOT_CREATED_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_SLOT_REUSED_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_SLOT_REUSED_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_SELF_NAME_TRUE_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_SELF_NAME_TRUE_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_SELF_NAME_FALSE_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_SELF_NAME_FALSE_NS)),
        );
        eprintln!(
            "cruft-profile-arrow-return-named-class-method-if-consequent-block-body-if-consequent-block-body-call-member-arg-member-object-identifier-upvalue-chain: depth1={}:{:.1}ms depth2={}:{:.1}ms depth3_plus={}:{:.1}ms terminal_local={}:{:.1}ms terminal_missing={}:{:.1}ms",
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_CHAIN_DEPTH_1_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_CHAIN_DEPTH_1_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_CHAIN_DEPTH_2_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_CHAIN_DEPTH_2_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_CHAIN_DEPTH_3_PLUS_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_CHAIN_DEPTH_3_PLUS_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_CHAIN_TERMINAL_LOCAL_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_CHAIN_TERMINAL_LOCAL_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_CHAIN_TERMINAL_MISSING_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_CHAIN_TERMINAL_MISSING_NS)),
        );
        eprintln!(
            "cruft-profile-arrow-return-named-class-method-if-consequent-block-body-if-consequent-block-body-call-member-arg-member-object-identifier-upvalue-depth2-reuse: parent_created={}:{:.1}ms parent_reused={}:{:.1}ms current_created_parent_created={}:{:.1}ms current_created_parent_reused={}:{:.1}ms current_reused_parent_created={}:{:.1}ms current_reused_parent_reused={}:{:.1}ms",
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_PARENT_CREATED_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_PARENT_CREATED_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_PARENT_REUSED_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_PARENT_REUSED_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_CREATED_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_CREATED_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_REUSED_PARENT_CREATED_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_REUSED_PARENT_CREATED_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_REUSED_PARENT_REUSED_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_REUSED_PARENT_REUSED_NS)),
        );
        eprintln!(
            "cruft-profile-arrow-return-named-class-method-if-consequent-block-body-if-consequent-block-body-call-member-arg-member-object-identifier-upvalue-depth2-current-created-parent-reused: parent_idx_0_7={}:{:.1}ms parent_idx_8_15={}:{:.1}ms parent_idx_16_plus={}:{:.1}ms local_slot_0_15={}:{:.1}ms local_slot_16_31={}:{:.1}ms local_slot_32_plus={}:{:.1}ms",
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_0_7_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_0_7_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_8_15_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_8_15_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_16_PLUS_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_16_PLUS_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_LOCAL_SLOT_0_15_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_LOCAL_SLOT_0_15_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_LOCAL_SLOT_16_31_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_LOCAL_SLOT_16_31_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_LOCAL_SLOT_32_PLUS_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_LOCAL_SLOT_32_PLUS_NS)),
        );
        eprintln!(
            "cruft-profile-arrow-return-named-class-method-if-consequent-block-body-if-consequent-block-body-call-member-arg-member-object-identifier-upvalue-depth2-current-created-parent-reused-low-parent-high-local: parent_idx_0={}:{:.1}ms parent_idx_1={}:{:.1}ms parent_idx_2={}:{:.1}ms parent_idx_3_plus={}:{:.1}ms local_slot_32_63={}:{:.1}ms local_slot_64_127={}:{:.1}ms local_slot_128_plus={}:{:.1}ms",
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_0_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_0_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_1_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_1_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_3_PLUS_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_3_PLUS_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_LOCAL_SLOT_32_63_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_LOCAL_SLOT_32_63_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_LOCAL_SLOT_64_127_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_LOCAL_SLOT_64_127_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_LOCAL_SLOT_128_PLUS_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_LOCAL_SLOT_128_PLUS_NS)),
        );
        eprintln!(
            "cruft-profile-arrow-return-named-class-method-if-consequent-block-body-if-consequent-block-body-call-member-arg-member-object-identifier-upvalue-depth2-current-created-parent-reused-parent-local-cross: parent_idx_0_local_slot_64_127={}:{:.1}ms parent_idx_0_local_slot_128_plus={}:{:.1}ms parent_idx_2_local_slot_64_127={}:{:.1}ms parent_idx_2_local_slot_128_plus={}:{:.1}ms",
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_0_LOCAL_SLOT_64_127_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_0_LOCAL_SLOT_64_127_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_0_LOCAL_SLOT_128_PLUS_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_0_LOCAL_SLOT_128_PLUS_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_64_127_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_64_127_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_128_PLUS_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_128_PLUS_NS)),
        );
        eprintln!(
            "cruft-profile-arrow-return-named-class-method-if-consequent-block-body-if-consequent-block-body-call-member-arg-member-object-identifier-upvalue-depth2-current-created-parent-reused-parent2-local64-127: local_slot_64_79={}:{:.1}ms local_slot_80_95={}:{:.1}ms local_slot_96_111={}:{:.1}ms local_slot_112_127={}:{:.1}ms",
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_64_79_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_64_79_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_80_95_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_80_95_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_96_111_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_96_111_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_112_127_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_112_127_NS)),
        );
        eprintln!(
            "cruft-profile-arrow-return-named-class-method-if-consequent-block-body-if-consequent-block-body-call-member-arg-member-object-identifier-upvalue-depth2-current-created-parent-reused-parent2-local96-111: local_slot_96_103={}:{:.1}ms local_slot_104_111={}:{:.1}ms",
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_96_103_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_96_103_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_104_111_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_104_111_NS)),
        );
        eprintln!(
            "cruft-profile-arrow-return-named-class-method-if-consequent-block-body-if-consequent-block-body-call-member-arg-member-object-identifier-upvalue-depth2-current-created-parent-reused-parent2-local96-103: local_slot_96_99={}:{:.1}ms local_slot_100_103={}:{:.1}ms",
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_96_99_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_96_99_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_100_103_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_100_103_NS)),
        );
        eprintln!(
            "cruft-profile-arrow-return-named-class-method-if-consequent-block-body-if-consequent-block-body-call-member-arg-member-object-identifier-upvalue-depth2-current-created-parent-reused-parent2-local96-99: local_slot_96_97={}:{:.1}ms local_slot_98_99={}:{:.1}ms",
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_96_97_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_96_97_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_99_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_99_NS)),
        );
        eprintln!(
            "cruft-profile-arrow-return-named-class-method-if-consequent-block-body-if-consequent-block-body-call-member-arg-member-object-identifier-upvalue-depth2-current-created-parent-reused-parent2-local98-99: local_slot_98={}:{:.1}ms local_slot_99={}:{:.1}ms",
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_99_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_99_NS)),
        );
        eprintln!(
            "cruft-profile-arrow-return-named-class-method-if-consequent-block-body-if-consequent-block-body-call-member-arg-member-object-identifier-upvalue-depth2-current-created-parent-reused-parent2-local98-name-shape: len_1_3={}:{:.1}ms len_4_7={}:{:.1}ms len_8_15={}:{:.1}ms len_16_plus={}:{:.1}ms initial_underscore={}:{:.1}ms initial_lower={}:{:.1}ms initial_other={}:{:.1}ms",
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_LEN_1_3_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_LEN_1_3_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_LEN_4_7_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_LEN_4_7_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_LEN_8_15_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_LEN_8_15_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_LEN_16_PLUS_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_LEN_16_PLUS_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_INITIAL_UNDERSCORE_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_INITIAL_UNDERSCORE_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_INITIAL_LOWER_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_INITIAL_LOWER_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_INITIAL_OTHER_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_INITIAL_OTHER_NS)),
        );
        eprintln!(
            "cruft-profile-arrow-return-named-class-method-if-consequent-block-body-if-consequent-block-body-call-member-arg-member-object-identifier-upvalue-depth2-current-created-parent-reused-parent2-local98-name-shape-refined: len_8_11={}:{:.1}ms len_12_15={}:{:.1}ms initial_upper={}:{:.1}ms initial_dollar={}:{:.1}ms initial_digit={}:{:.1}ms initial_nonascii={}:{:.1}ms initial_punct_other={}:{:.1}ms",
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_LEN_8_11_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_LEN_8_11_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_LEN_12_15_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_LEN_12_15_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_INITIAL_UPPER_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_INITIAL_UPPER_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_INITIAL_DOLLAR_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_INITIAL_DOLLAR_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_INITIAL_DIGIT_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_INITIAL_DIGIT_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_INITIAL_NONASCII_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_INITIAL_NONASCII_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_INITIAL_PUNCT_OTHER_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_INITIAL_PUNCT_OTHER_NS)),
        );
        eprintln!(
            "cruft-profile-arrow-return-named-class-method-if-consequent-block-body-if-consequent-block-body-call-member-arg-member-object-identifier-upvalue-depth2-current-created-parent-reused-parent2-local98-uppercase-short-name: len_8={}:{:.1}ms len_9={}:{:.1}ms len_10={}:{:.1}ms len_11={}:{:.1}ms upper_a_m={}:{:.1}ms upper_n_z={}:{:.1}ms",
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_LEN_8_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_LEN_8_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_LEN_9_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_LEN_9_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_LEN_10_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_LEN_10_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_LEN_11_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_LEN_11_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_INITIAL_UPPER_A_M_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_INITIAL_UPPER_A_M_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_INITIAL_UPPER_N_Z_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_INITIAL_UPPER_N_Z_NS)),
        );
        eprintln!(
            "cruft-profile-arrow-return-named-class-method-if-consequent-block-body-if-consequent-block-body-call-member-arg-member-object-identifier-upvalue-depth2-current-created-parent-reused-parent2-local98-uppercase-len10-am: upper_a_f={}:{:.1}ms upper_g_m={}:{:.1}ms second_lower={}:{:.1}ms second_upper={}:{:.1}ms second_digit_underscore={}:{:.1}ms second_other={}:{:.1}ms",
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_INITIAL_UPPER_A_F_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_INITIAL_UPPER_A_F_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_INITIAL_UPPER_G_M_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_INITIAL_UPPER_G_M_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_SECOND_LOWER_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_SECOND_LOWER_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_SECOND_UPPER_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_SECOND_UPPER_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_SECOND_DIGIT_UNDERSCORE_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_SECOND_DIGIT_UNDERSCORE_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_SECOND_OTHER_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_SECOND_OTHER_NS)),
        );
        eprintln!(
            "cruft-profile-arrow-return-named-class-method-if-consequent-block-body-if-consequent-block-body-call-member-arg-member-object-identifier-upvalue-depth2-current-created-parent-reused-parent2-local98-uppercase-len10-af-second-lower: upper_a_c={}:{:.1}ms upper_d_f={}:{:.1}ms third_lower={}:{:.1}ms third_upper_digit_underscore={}:{:.1}ms third_other={}:{:.1}ms",
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_INITIAL_UPPER_A_C_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_INITIAL_UPPER_A_C_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_INITIAL_UPPER_D_F_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_INITIAL_UPPER_D_F_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_THIRD_LOWER_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_THIRD_LOWER_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_THIRD_UPPER_DIGIT_UNDERSCORE_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_THIRD_UPPER_DIGIT_UNDERSCORE_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_THIRD_OTHER_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_THIRD_OTHER_NS)),
        );
        eprintln!(
            "cruft-profile-arrow-return-named-class-method-if-consequent-block-body-if-consequent-block-body-call-member-arg-member-object-identifier-upvalue-depth2-current-created-parent-reused-parent2-local98-uppercase-len10-df-second-lower-third-lower: initial_d={}:{:.1}ms initial_e={}:{:.1}ms initial_f={}:{:.1}ms fourth_lower={}:{:.1}ms fourth_upper_digit_underscore={}:{:.1}ms fourth_other={}:{:.1}ms",
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_INITIAL_D_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_INITIAL_D_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_INITIAL_E_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_INITIAL_E_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_INITIAL_F_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_INITIAL_F_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_FOURTH_LOWER_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_FOURTH_LOWER_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_FOURTH_UPPER_DIGIT_UNDERSCORE_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_FOURTH_UPPER_DIGIT_UNDERSCORE_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_FOURTH_OTHER_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_FOURTH_OTHER_NS)),
        );
        eprintln!(
            "cruft-profile-arrow-return-named-class-method-if-consequent-block-body-if-consequent-block-body-call-member-arg-member-object-identifier-upvalue-depth2-current-created-parent-reused-parent2-local98-uppercase-len10-f-second-third-fourth-lower: fourth_a_m={}:{:.1}ms fourth_n_z={}:{:.1}ms fifth_lower={}:{:.1}ms fifth_upper_digit_underscore={}:{:.1}ms fifth_other={}:{:.1}ms",
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_FOURTH_A_M_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_FOURTH_A_M_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_FOURTH_N_Z_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_FOURTH_N_Z_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_FIFTH_LOWER_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_FIFTH_LOWER_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_FIFTH_UPPER_DIGIT_UNDERSCORE_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_FIFTH_UPPER_DIGIT_UNDERSCORE_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_FIFTH_OTHER_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_FIFTH_OTHER_NS)),
        );
        eprintln!(
            "cruft-profile-arrow-return-named-class-method-if-consequent-block-body-if-consequent-block-body-call-member-arg-member-object-identifier-upvalue-depth2-current-created-parent-reused-parent2-local98-uppercase-len10-f-fourth-nz-fifth-upper-digit: fifth_upper={}:{:.1}ms fifth_digit_underscore={}:{:.1}ms sixth_lower={}:{:.1}ms sixth_upper_digit_underscore={}:{:.1}ms sixth_other={}:{:.1}ms",
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_FIFTH_UPPER_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_FIFTH_UPPER_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_FIFTH_DIGIT_UNDERSCORE_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_FIFTH_DIGIT_UNDERSCORE_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_SIXTH_LOWER_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_SIXTH_LOWER_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_SIXTH_UPPER_DIGIT_UNDERSCORE_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_SIXTH_UPPER_DIGIT_UNDERSCORE_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_SIXTH_OTHER_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_SIXTH_OTHER_NS)),
        );
        eprintln!(
            "cruft-profile-arrow-return-named-class-method-if-consequent-block-body-if-consequent-block-body-call-member-arg-member-object-identifier-upvalue-depth2-current-created-parent-reused-parent2-local98-uppercase-len10-f-fourth-nz-fifth-upper-sixth-lower: sixth_a_m={}:{:.1}ms sixth_n_z={}:{:.1}ms seventh_lower={}:{:.1}ms seventh_upper_digit_underscore={}:{:.1}ms seventh_other={}:{:.1}ms",
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_SIXTH_A_M_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_SIXTH_A_M_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_SIXTH_N_Z_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_SIXTH_N_Z_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_SEVENTH_LOWER_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_SEVENTH_LOWER_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_SEVENTH_UPPER_DIGIT_UNDERSCORE_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_SEVENTH_UPPER_DIGIT_UNDERSCORE_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_SEVENTH_OTHER_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_SEVENTH_OTHER_NS)),
        );
        eprintln!(
            "cruft-profile-arrow-return-named-class-method-if-consequent-block-body-if-consequent-block-body-call-member-arg-member-object-identifier-upvalue-depth2-current-created-parent-reused-parent2-local98-uppercase-len10-f-sixth-nz-seventh-lower: seventh_a_m={}:{:.1}ms seventh_n_z={}:{:.1}ms eighth_lower={}:{:.1}ms eighth_upper_digit_underscore={}:{:.1}ms eighth_other={}:{:.1}ms",
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_SEVENTH_A_M_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_SEVENTH_A_M_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_SEVENTH_N_Z_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_SEVENTH_N_Z_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_EIGHTH_LOWER_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_EIGHTH_LOWER_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_EIGHTH_UPPER_DIGIT_UNDERSCORE_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_EIGHTH_UPPER_DIGIT_UNDERSCORE_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_EIGHTH_OTHER_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_EIGHTH_OTHER_NS)),
        );
        eprintln!(
            "cruft-profile-arrow-return-named-class-method-if-consequent-block-body-if-consequent-block-body-call-member-arg-member-object-identifier-upvalue-depth2-current-created-parent-reused-parent2-local98-uppercase-len10-f-seventh-nz-eighth-lower: eighth_a_m={}:{:.1}ms eighth_n_z={}:{:.1}ms ninth_lower={}:{:.1}ms ninth_upper_digit_underscore={}:{:.1}ms ninth_other={}:{:.1}ms",
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_EIGHTH_A_M_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_EIGHTH_A_M_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_EIGHTH_N_Z_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_EIGHTH_N_Z_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_NINTH_LOWER_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_NINTH_LOWER_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_NINTH_UPPER_DIGIT_UNDERSCORE_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_NINTH_UPPER_DIGIT_UNDERSCORE_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_NINTH_OTHER_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_NINTH_OTHER_NS)),
        );
        eprintln!(
            "cruft-profile-arrow-return-named-class-method-if-consequent-block-body-if-consequent-block-body-call-member-arg-member-object-identifier-upvalue-depth2-current-created-parent-reused-parent2-local98-uppercase-len10-f-eighth-nz-ninth-lower: ninth_a_m={}:{:.1}ms ninth_n_z={}:{:.1}ms tenth_lower={}:{:.1}ms tenth_upper_digit_underscore={}:{:.1}ms tenth_other={}:{:.1}ms",
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_NINTH_A_M_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_NINTH_A_M_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_NINTH_N_Z_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_NINTH_N_Z_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_TENTH_LOWER_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_TENTH_LOWER_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_TENTH_UPPER_DIGIT_UNDERSCORE_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_TENTH_UPPER_DIGIT_UNDERSCORE_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_TENTH_OTHER_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_IF_CONS_BLOCK_BODY_CALL_MEMBER_ARG_MEMBER_OBJECT_IDENTIFIER_UPVALUE_DEPTH2_CURRENT_CREATED_PARENT_REUSED_PARENT_IDX_2_LOCAL_SLOT_98_NAME_TENTH_OTHER_NS)),
        );
        eprintln!(
            "cruft-profile-arrow-return-named-class-method-if-consequent-block-body-expr: function={}:{:.1}ms arrow={}:{:.1}ms class={}:{:.1}ms paren={}:{:.1}ms literal={}:{:.1}ms identifier={}:{:.1}ms member={}:{:.1}ms call_new={}:{:.1}ms object_array={}:{:.1}ms operator={}:{:.1}ms other={}:{:.1}ms",
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_EXPR_FUNCTION_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_EXPR_FUNCTION_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_EXPR_ARROW_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_EXPR_ARROW_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_EXPR_CLASS_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_EXPR_CLASS_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_EXPR_PAREN_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_EXPR_PAREN_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_EXPR_LITERAL_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_EXPR_LITERAL_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_EXPR_IDENTIFIER_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_EXPR_IDENTIFIER_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_EXPR_MEMBER_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_EXPR_MEMBER_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_EXPR_CALL_NEW_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_EXPR_CALL_NEW_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_EXPR_OBJECT_ARRAY_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_EXPR_OBJECT_ARRAY_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_EXPR_OPERATOR_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_EXPR_OPERATOR_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_EXPR_OTHER_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_EXPR_OTHER_NS)),
        );
        eprintln!(
            "cruft-profile-arrow-return-named-class-method-if-consequent-block-body-operator: unary={}:{:.1}ms update={}:{:.1}ms binary_logical={}:{:.1}ms binary_other={}:{:.1}ms conditional={}:{:.1}ms assign={}:{:.1}ms sequence={}:{:.1}ms",
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_OPERATOR_UNARY_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_OPERATOR_UNARY_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_OPERATOR_UPDATE_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_OPERATOR_UPDATE_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_OPERATOR_BINARY_LOGICAL_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_OPERATOR_BINARY_LOGICAL_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_OPERATOR_BINARY_OTHER_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_OPERATOR_BINARY_OTHER_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_OPERATOR_CONDITIONAL_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_OPERATOR_CONDITIONAL_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_OPERATOR_ASSIGN_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_OPERATOR_ASSIGN_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_OPERATOR_SEQUENCE_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_OPERATOR_SEQUENCE_NS)),
        );
        eprintln!(
            "cruft-profile-arrow-return-named-class-method-if-consequent-block-body-assign-op: simple={}:{:.1}ms compound={}:{:.1}ms",
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_OP_SIMPLE_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_OP_SIMPLE_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_OP_COMPOUND_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_OP_COMPOUND_NS)),
        );
        eprintln!(
            "cruft-profile-arrow-return-named-class-method-if-consequent-block-body-assign-target: identifier={}:{:.1}ms member_static={}:{:.1}ms member_computed={}:{:.1}ms array_pattern={}:{:.1}ms object_pattern={}:{:.1}ms other={}:{:.1}ms",
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_TARGET_IDENTIFIER_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_TARGET_IDENTIFIER_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_TARGET_MEMBER_STATIC_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_TARGET_MEMBER_STATIC_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_TARGET_MEMBER_COMPUTED_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_TARGET_MEMBER_COMPUTED_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_TARGET_ARRAY_PATTERN_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_TARGET_ARRAY_PATTERN_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_TARGET_OBJECT_PATTERN_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_TARGET_OBJECT_PATTERN_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_TARGET_OTHER_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_TARGET_OTHER_NS)),
        );
        eprintln!(
            "cruft-profile-arrow-return-named-class-method-if-consequent-block-body-assign-static-member: object={}:{:.1}ms value={}:{:.1}ms store={}:{:.1}ms",
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_OBJECT_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_OBJECT_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_STORE_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_STORE_NS)),
        );
        eprintln!(
            "cruft-profile-arrow-return-named-class-method-if-consequent-block-body-assign-static-member-value-expr: function={}:{:.1}ms arrow={}:{:.1}ms class={}:{:.1}ms paren={}:{:.1}ms literal={}:{:.1}ms identifier={}:{:.1}ms member={}:{:.1}ms call_new={}:{:.1}ms object_array={}:{:.1}ms operator={}:{:.1}ms other={}:{:.1}ms",
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_EXPR_FUNCTION_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_EXPR_FUNCTION_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_EXPR_ARROW_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_EXPR_ARROW_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_EXPR_CLASS_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_EXPR_CLASS_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_EXPR_PAREN_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_EXPR_PAREN_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_EXPR_LITERAL_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_EXPR_LITERAL_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_EXPR_IDENTIFIER_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_EXPR_IDENTIFIER_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_EXPR_MEMBER_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_EXPR_MEMBER_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_EXPR_CALL_NEW_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_EXPR_CALL_NEW_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_EXPR_OBJECT_ARRAY_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_EXPR_OBJECT_ARRAY_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_EXPR_OPERATOR_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_EXPR_OPERATOR_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_EXPR_OTHER_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_EXPR_OTHER_NS)),
        );
        eprintln!(
            "cruft-profile-arrow-return-named-class-method-if-consequent-block-body-assign-static-member-value-call-new: call={}:{:.1}ms new={}:{:.1}ms",
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_CALL_NEW_CALL_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_CALL_NEW_CALL_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_CALL_NEW_NEW_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_CALL_NEW_NEW_NS)),
        );
        eprintln!(
            "cruft-profile-arrow-return-named-class-method-if-consequent-block-body-assign-static-member-value-call-callee: template_helper={}:{:.1}ms member={}:{:.1}ms identifier={}:{:.1}ms other={}:{:.1}ms",
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_CALL_CALLEE_TEMPLATE_HELPER_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_CALL_CALLEE_TEMPLATE_HELPER_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_CALL_CALLEE_MEMBER_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_CALL_CALLEE_MEMBER_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_CALL_CALLEE_IDENTIFIER_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_CALL_CALLEE_IDENTIFIER_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_CALL_CALLEE_OTHER_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_CALL_CALLEE_OTHER_NS)),
        );
        eprintln!(
            "cruft-profile-arrow-return-named-class-method-if-consequent-block-body-assign-static-member-value-call-args: none={}:{:.1}ms fixed={}:{:.1}ms spread_wide={}:{:.1}ms",
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_CALL_ARGS_NONE_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_CALL_ARGS_NONE_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_CALL_ARGS_FIXED_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_CALL_ARGS_FIXED_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_CALL_ARGS_SPREAD_WIDE_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_CALL_ARGS_SPREAD_WIDE_NS)),
        );
        eprintln!(
            "cruft-profile-arrow-return-named-class-method-if-consequent-block-body-assign-static-member-value-call-member-property: identifier={}:{:.1}ms computed={}:{:.1}ms private={}:{:.1}ms",
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_CALL_MEMBER_PROPERTY_IDENTIFIER_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_CALL_MEMBER_PROPERTY_IDENTIFIER_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_CALL_MEMBER_PROPERTY_COMPUTED_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_CALL_MEMBER_PROPERTY_COMPUTED_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_CALL_MEMBER_PROPERTY_PRIVATE_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_CALL_MEMBER_PROPERTY_PRIVATE_NS)),
        );
        eprintln!(
            "cruft-profile-arrow-return-named-class-method-if-consequent-block-body-assign-static-member-value-call-member-optional: none={}:{:.1}ms member={}:{:.1}ms call={}:{:.1}ms both={}:{:.1}ms",
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_CALL_MEMBER_OPTIONAL_NONE_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_CALL_MEMBER_OPTIONAL_NONE_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_CALL_MEMBER_OPTIONAL_MEMBER_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_CALL_MEMBER_OPTIONAL_MEMBER_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_CALL_MEMBER_OPTIONAL_CALL_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_CALL_MEMBER_OPTIONAL_CALL_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_CALL_MEMBER_OPTIONAL_BOTH_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_CALL_MEMBER_OPTIONAL_BOTH_NS)),
        );
        eprintln!(
            "cruft-profile-arrow-return-named-class-method-if-consequent-block-body-assign-static-member-value-call-member-phase: receiver={}:{:.1}ms lookup={}:{:.1}ms args={}:{:.1}ms emit={}:{:.1}ms",
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_CALL_MEMBER_RECEIVER_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_CALL_MEMBER_RECEIVER_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_CALL_MEMBER_LOOKUP_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_CALL_MEMBER_LOOKUP_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_CALL_MEMBER_ARGS_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_CALL_MEMBER_ARGS_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_CALL_MEMBER_EMIT_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_CALL_MEMBER_EMIT_NS)),
        );
        eprintln!(
            "cruft-profile-arrow-return-named-class-method-if-consequent-block-body-assign-static-member-value-call-member-args-count: none={}:{:.1}ms fixed={}:{:.1}ms",
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_CALL_MEMBER_ARGS_NONE_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_CALL_MEMBER_ARGS_NONE_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_CALL_MEMBER_ARGS_FIXED_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_CALL_MEMBER_ARGS_FIXED_NS)),
        );
        eprintln!(
            "cruft-profile-arrow-return-named-class-method-if-consequent-block-body-assign-static-member-value-call-member-arg-expr: function={}:{:.1}ms arrow={}:{:.1}ms class={}:{:.1}ms paren={}:{:.1}ms literal={}:{:.1}ms identifier={}:{:.1}ms member={}:{:.1}ms call_new={}:{:.1}ms object_array={}:{:.1}ms operator={}:{:.1}ms other={}:{:.1}ms",
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_CALL_MEMBER_ARG_EXPR_FUNCTION_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_CALL_MEMBER_ARG_EXPR_FUNCTION_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_CALL_MEMBER_ARG_EXPR_ARROW_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_CALL_MEMBER_ARG_EXPR_ARROW_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_CALL_MEMBER_ARG_EXPR_CLASS_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_CALL_MEMBER_ARG_EXPR_CLASS_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_CALL_MEMBER_ARG_EXPR_PAREN_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_CALL_MEMBER_ARG_EXPR_PAREN_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_CALL_MEMBER_ARG_EXPR_LITERAL_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_CALL_MEMBER_ARG_EXPR_LITERAL_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_CALL_MEMBER_ARG_EXPR_IDENTIFIER_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_CALL_MEMBER_ARG_EXPR_IDENTIFIER_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_CALL_MEMBER_ARG_EXPR_MEMBER_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_CALL_MEMBER_ARG_EXPR_MEMBER_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_CALL_MEMBER_ARG_EXPR_CALL_NEW_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_CALL_MEMBER_ARG_EXPR_CALL_NEW_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_CALL_MEMBER_ARG_EXPR_OBJECT_ARRAY_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_CALL_MEMBER_ARG_EXPR_OBJECT_ARRAY_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_CALL_MEMBER_ARG_EXPR_OPERATOR_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_CALL_MEMBER_ARG_EXPR_OPERATOR_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_CALL_MEMBER_ARG_EXPR_OTHER_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_CALL_MEMBER_ARG_EXPR_OTHER_NS)),
        );
        eprintln!(
            "cruft-profile-arrow-return-named-class-method-if-consequent-block-body-assign-static-member-value-call-member-arg-arrow: normalize={}:{:.1}ms proto={}:{:.1}ms capture_clone={}:{:.1}ms intern={}:{:.1}ms make_emit={}:{:.1}ms",
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_CALL_MEMBER_ARG_ARROW_NORMALIZE_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_CALL_MEMBER_ARG_ARROW_NORMALIZE_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_CALL_MEMBER_ARG_ARROW_PROTO_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_CALL_MEMBER_ARG_ARROW_PROTO_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_CALL_MEMBER_ARG_ARROW_CAPTURE_CLONE_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_CALL_MEMBER_ARG_ARROW_CAPTURE_CLONE_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_CALL_MEMBER_ARG_ARROW_INTERN_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_CALL_MEMBER_ARG_ARROW_INTERN_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_CALL_MEMBER_ARG_ARROW_MAKE_EMIT_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_CALL_MEMBER_ARG_ARROW_MAKE_EMIT_NS)),
        );
        eprintln!(
            "cruft-profile-arrow-return-named-class-method-if-consequent-block-body-assign-static-member-value-call-member-arg-arrow-proto: setup={}:{:.3}ms params={}:{:.3}ms prealloc={}:{:.3}ms hoist={}:{:.3}ms body={}:{:.3}ms assemble={}:{:.3}ms",
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_CALL_MEMBER_ARG_ARROW_PROTO_SETUP_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_CALL_MEMBER_ARG_ARROW_PROTO_SETUP_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_CALL_MEMBER_ARG_ARROW_PROTO_PARAMS_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_CALL_MEMBER_ARG_ARROW_PROTO_PARAMS_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_CALL_MEMBER_ARG_ARROW_PROTO_PREALLOC_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_CALL_MEMBER_ARG_ARROW_PROTO_PREALLOC_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_CALL_MEMBER_ARG_ARROW_PROTO_HOIST_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_CALL_MEMBER_ARG_ARROW_PROTO_HOIST_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_CALL_MEMBER_ARG_ARROW_PROTO_BODY_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_CALL_MEMBER_ARG_ARROW_PROTO_BODY_NS)),
            cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_CALL_MEMBER_ARG_ARROW_PROTO_ASSEMBLE_CALLS),
            ms(cp::read(&cp::ARROW_RETURN_NAMED_CLASS_METHOD_IF_CONS_BLOCK_BODY_ASSIGN_STATIC_MEMBER_VALUE_CALL_MEMBER_ARG_ARROW_PROTO_ASSEMBLE_NS)),
        );
        if std::env::var("CRUFT_PROFILE_OPS").is_ok() {
            for (op, count, ns) in rusty_js_runtime::interp::interp_op_profile_summary(12) {
                let avg_ns = if count == 0 { 0 } else { ns / count };
                eprintln!(
                    "cruft-profile-op: op={} count={} total={:.1}ms avg={}ns",
                    op,
                    count,
                    ms(ns),
                    avg_ns
                );
            }
        }
        if std::env::var("CRUFT_PROFILE_OPS_BY_FRAME").is_ok() {
            for (frame, op, count, ns) in
                rusty_js_runtime::interp::interp_op_frame_profile_summary(20)
            {
                let avg_ns = if count == 0 { 0 } else { ns / count };
                eprintln!(
                    "cruft-profile-op-frame: frame={} op={} count={} total={:.1}ms avg={}ns",
                    frame,
                    op,
                    count,
                    ms(ns),
                    avg_ns
                );
            }
        }
        if std::env::var("CRUFT_PROFILE_MEMBER_OPS_BY_FRAME").is_ok() {
            for (frame, op, key, count, ns) in
                rusty_js_runtime::interp::interp_member_op_frame_profile_summary(30)
            {
                let avg_ns = if count == 0 { 0 } else { ns / count };
                eprintln!(
                    "cruft-profile-member-op-frame: frame={} op={} key={} count={} total={:.1}ms avg={}ns",
                    frame,
                    op,
                    key,
                    count,
                    ms(ns),
                    avg_ns
                );
            }
        }
        if std::env::var("CRUFT_PROFILE_LOADLOCAL_FRAME").is_ok() {
            for (frame, slot, kind, count) in
                rusty_js_runtime::interp::loadlocal_frame_profile_summary(30)
            {
                eprintln!(
                    "cruft-profile-loadlocal-frame: frame={} slot={} kind={} count={}",
                    frame, slot, kind, count
                );
            }
        }
        if std::env::var("CRUFT_PROFILE_IHI").is_ok() {
            for (entry, count, ns) in rusty_js_runtime::interp::interp_ihi_profile_summary(12) {
                let avg_ns = if count == 0 { 0 } else { ns / count };
                eprintln!(
                    "cruft-profile-ihi: entry={} hits={} total={:.1}ms avg={}ns",
                    entry,
                    count,
                    ms(ns),
                    avg_ns
                );
            }
        }
        if let Some((
            total,
            array_direct,
            object_other,
            string_primitive,
            primitive_other,
            nullish,
        )) = rusty_js_runtime::interp::array_length_getprop_profile_summary()
        {
            eprintln!(
                "cruft-profile-array-length-getprop: total={} array_direct={} object_other={} string_primitive={} primitive_other={} nullish={}",
                total, array_direct, object_other, string_primitive, primitive_other, nullish
            );
        }
        if let Some((
            (
                get_total,
                get_array_dense,
                get_array_other,
                get_typed_array,
                get_object_other,
                get_string_primitive,
                get_primitive_other,
                get_nullish,
            ),
            (
                set_total,
                set_array_dense,
                set_array_other,
                set_typed_array,
                set_object_other,
                set_primitive,
                set_nullish,
            ),
        )) = rusty_js_runtime::interp::index_op_family_profile_summary()
        {
            eprintln!(
                "cruft-profile-index-op-family: get_total={} get_array_dense={} get_array_other={} get_typed_array={} get_object_other={} get_string_primitive={} get_primitive_other={} get_nullish={} set_total={} set_array_dense={} set_array_other={} set_typed_array={} set_object_other={} set_primitive={} set_nullish={}",
                get_total, get_array_dense, get_array_other, get_typed_array, get_object_other, get_string_primitive, get_primitive_other, get_nullish, set_total, set_array_dense, set_array_other, set_typed_array, set_object_other, set_primitive, set_nullish
            );
        }
        if let Some((
            total,
            object,
            proxy,
            typed_array,
            module_namespace,
            chain_proxy,
            ta_chain,
            setter,
            accessor_no_setter,
            own_nonwritable,
            inherited_nonwritable,
            object_set_existing,
            object_set_new,
            object_set_new_shaped,
            non_extensible,
            primitive,
        )) = rusty_js_runtime::interp::setindex_phase_profile_summary()
        {
            eprintln!(
                "cruft-profile-setindex-phases: total={} object={} proxy={} typed_array={} module_namespace={} chain_proxy={} ta_chain={} setter={} accessor_no_setter={} own_nonwritable={} inherited_nonwritable={} object_set_existing={} object_set_new={} object_set_new_shaped={} non_extensible={} primitive={}",
                total, object, proxy, typed_array, module_namespace, chain_proxy, ta_chain, setter, accessor_no_setter, own_nonwritable, inherited_nonwritable, object_set_existing, object_set_new, object_set_new_shaped, non_extensible, primitive
            );
        }
        if let Some((
            total,
            total_ns,
            to_property_key_ns,
            proxy_check_ns,
            chain_proxy_ns,
            ta_chain_ns,
            setter_lookup_ns,
            accessor_lookup_ns,
            nonwritable_lookup_ns,
            object_set_ns,
            other_ns,
        )) = rusty_js_runtime::interp::setindex_timing_profile_summary()
        {
            let avg_ns = if total == 0 { 0 } else { total_ns / total };
            eprintln!(
                "cruft-profile-setindex-timing: total={} total={:.1}ms avg={}ns to_property_key={:.1}ms proxy_check={:.1}ms chain_proxy={:.1}ms ta_chain={:.1}ms setter_lookup={:.1}ms accessor_lookup={:.1}ms nonwritable_lookup={:.1}ms object_set={:.1}ms other={:.1}ms",
                total, ms(total_ns), avg_ns, ms(to_property_key_ns), ms(proxy_check_ns), ms(chain_proxy_ns), ms(ta_chain_ns), ms(setter_lookup_ns), ms(accessor_lookup_ns), ms(nonwritable_lookup_ns), ms(object_set_ns), ms(other_ns)
            );
        }
        if std::env::var("CRUFT_PROFILE_CALLS").is_ok() {
            for (name, calls, iterations, roots_ns, inner_ns, self_ns, total_ns) in
                rusty_js_runtime::interp::interp_call_profile_summary(12)
            {
                let avg_ns = if calls == 0 { 0 } else { total_ns / calls };
                eprintln!(
                    "cruft-profile-call: callee={} calls={} iterations={} roots={:.1}ms inner={:.1}ms self={:.1}ms total={:.1}ms avg={}ns",
                    name,
                    calls,
                    iterations,
                    ms(roots_ns),
                    ms(inner_ns),
                    ms(self_ns),
                    ms(total_ns),
                    avg_ns
                );
            }
        }
    }

    if std::env::var("CRUFT_PROFILE").is_err() && std::env::var("CRUFT_PROFILE_MODULE").is_err() {
        let ms = |ns: u64| ns as f64 / 1_000_000.0;
        if std::env::var("CRUFT_PROFILE_OPS").is_ok() {
            for (op, count, ns) in rusty_js_runtime::interp::interp_op_profile_summary(12) {
                let avg_ns = if count == 0 { 0 } else { ns / count };
                eprintln!(
                    "cruft-profile-op: op={} count={} total={:.1}ms avg={}ns",
                    op,
                    count,
                    ms(ns),
                    avg_ns
                );
            }
        }
        if std::env::var("CRUFT_PROFILE_OPS_BY_FRAME").is_ok() {
            for (frame, op, count, ns) in
                rusty_js_runtime::interp::interp_op_frame_profile_summary(20)
            {
                let avg_ns = if count == 0 { 0 } else { ns / count };
                eprintln!(
                    "cruft-profile-op-frame: frame={} op={} count={} total={:.1}ms avg={}ns",
                    frame,
                    op,
                    count,
                    ms(ns),
                    avg_ns
                );
            }
        }
        if std::env::var("CRUFT_PROFILE_MEMBER_OPS_BY_FRAME").is_ok() {
            for (frame, op, key, count, ns) in
                rusty_js_runtime::interp::interp_member_op_frame_profile_summary(30)
            {
                let avg_ns = if count == 0 { 0 } else { ns / count };
                eprintln!(
                    "cruft-profile-member-op-frame: frame={} op={} key={} count={} total={:.1}ms avg={}ns",
                    frame,
                    op,
                    key,
                    count,
                    ms(ns),
                    avg_ns
                );
            }
        }
        if std::env::var("CRUFT_PROFILE_LOADLOCAL_FRAME").is_ok() {
            for (frame, slot, kind, count) in
                rusty_js_runtime::interp::loadlocal_frame_profile_summary(30)
            {
                eprintln!(
                    "cruft-profile-loadlocal-frame: frame={} slot={} kind={} count={}",
                    frame, slot, kind, count
                );
            }
        }
        if std::env::var("CRUFT_PROFILE_IHI").is_ok() {
            for (entry, count, ns) in rusty_js_runtime::interp::interp_ihi_profile_summary(12) {
                let avg_ns = if count == 0 { 0 } else { ns / count };
                eprintln!(
                    "cruft-profile-ihi: entry={} hits={} total={:.1}ms avg={}ns",
                    entry,
                    count,
                    ms(ns),
                    avg_ns
                );
            }
        }
        if let Some((
            total,
            array_direct,
            object_other,
            string_primitive,
            primitive_other,
            nullish,
        )) = rusty_js_runtime::interp::array_length_getprop_profile_summary()
        {
            eprintln!(
                "cruft-profile-array-length-getprop: total={} array_direct={} object_other={} string_primitive={} primitive_other={} nullish={}",
                total, array_direct, object_other, string_primitive, primitive_other, nullish
            );
        }
        if let Some((
            (
                get_total,
                get_array_dense,
                get_array_other,
                get_typed_array,
                get_object_other,
                get_string_primitive,
                get_primitive_other,
                get_nullish,
            ),
            (
                set_total,
                set_array_dense,
                set_array_other,
                set_typed_array,
                set_object_other,
                set_primitive,
                set_nullish,
            ),
        )) = rusty_js_runtime::interp::index_op_family_profile_summary()
        {
            eprintln!(
                "cruft-profile-index-op-family: get_total={} get_array_dense={} get_array_other={} get_typed_array={} get_object_other={} get_string_primitive={} get_primitive_other={} get_nullish={} set_total={} set_array_dense={} set_array_other={} set_typed_array={} set_object_other={} set_primitive={} set_nullish={}",
                get_total, get_array_dense, get_array_other, get_typed_array, get_object_other, get_string_primitive, get_primitive_other, get_nullish, set_total, set_array_dense, set_array_other, set_typed_array, set_object_other, set_primitive, set_nullish
            );
        }
        if let Some((
            total,
            object,
            proxy,
            typed_array,
            module_namespace,
            chain_proxy,
            ta_chain,
            setter,
            accessor_no_setter,
            own_nonwritable,
            inherited_nonwritable,
            object_set_existing,
            object_set_new,
            object_set_new_shaped,
            non_extensible,
            primitive,
        )) = rusty_js_runtime::interp::setindex_phase_profile_summary()
        {
            eprintln!(
                "cruft-profile-setindex-phases: total={} object={} proxy={} typed_array={} module_namespace={} chain_proxy={} ta_chain={} setter={} accessor_no_setter={} own_nonwritable={} inherited_nonwritable={} object_set_existing={} object_set_new={} object_set_new_shaped={} non_extensible={} primitive={}",
                total, object, proxy, typed_array, module_namespace, chain_proxy, ta_chain, setter, accessor_no_setter, own_nonwritable, inherited_nonwritable, object_set_existing, object_set_new, object_set_new_shaped, non_extensible, primitive
            );
        }
        if let Some((
            total,
            total_ns,
            to_property_key_ns,
            proxy_check_ns,
            chain_proxy_ns,
            ta_chain_ns,
            setter_lookup_ns,
            accessor_lookup_ns,
            nonwritable_lookup_ns,
            object_set_ns,
            other_ns,
        )) = rusty_js_runtime::interp::setindex_timing_profile_summary()
        {
            let avg_ns = if total == 0 { 0 } else { total_ns / total };
            eprintln!(
                "cruft-profile-setindex-timing: total={} total={:.1}ms avg={}ns to_property_key={:.1}ms proxy_check={:.1}ms chain_proxy={:.1}ms ta_chain={:.1}ms setter_lookup={:.1}ms accessor_lookup={:.1}ms nonwritable_lookup={:.1}ms object_set={:.1}ms other={:.1}ms",
                total, ms(total_ns), avg_ns, ms(to_property_key_ns), ms(proxy_check_ns), ms(chain_proxy_ns), ms(ta_chain_ns), ms(setter_lookup_ns), ms(accessor_lookup_ns), ms(nonwritable_lookup_ns), ms(object_set_ns), ms(other_ns)
            );
        }
        if std::env::var("CRUFT_PROFILE_CALLS").is_ok() {
            for (name, calls, iterations, roots_ns, inner_ns, self_ns, total_ns) in
                rusty_js_runtime::interp::interp_call_profile_summary(12)
            {
                let avg_ns = if calls == 0 { 0 } else { total_ns / calls };
                eprintln!(
                    "cruft-profile-call: callee={} calls={} iterations={} roots={:.1}ms inner={:.1}ms self={:.1}ms total={:.1}ms avg={}ns",
                    name,
                    calls,
                    iterations,
                    ms(roots_ns),
                    ms(inner_ns),
                    ms(self_ns),
                    ms(total_ns),
                    avg_ns
                );
            }
        }
    }

    rt.diffspec_summary();

    let unhandled = rt.drain_unhandled_rejections();
    if !unhandled.is_empty() {

        let has_handler = {
            let handled = cruft::process::has_process_listener(&mut rt, "unhandledRejection");
            if handled {
                for (id, reason) in &unhandled {

                    cruft::process::emit_process_event(
                        &mut rt,
                        "unhandledRejection",
                        vec![reason.clone(), Value::Object(*id)],
                    );
                }
            }
            handled
        };

        let policy = std::env::var("CRUFT_UNHANDLED_REJECTION").unwrap_or_default();
        let node_stack_size_projection =
            policy == "none" && std::env::var_os("CRUFT_NODE_STACK_SIZE_FLAG").is_some();
        let (diagnose, fatal) = if has_handler {
            (false, false)
        } else if node_stack_size_projection && unhandled.len() > 32 {
            eprintln!("RangeError: Maximum call stack size exceeded");
            (false, false)
        } else {
            match policy.as_str() {
                "warn" => (true, false),
                "none" | "silent" => (false, false),
                _ => (true, true),
            }
        };
        if diagnose {

            let mut rendered: Vec<String> = Vec::with_capacity(unhandled.len());
            for (_id, reason) in &unhandled {
                rendered.push(match reason {
                    Value::Object(oid) => {
                        let stack = match rt.object_get(*oid, "stack") {
                            Value::String(s) if !s.as_str().trim().is_empty() => {
                                Some(s.to_string())
                            }
                            _ => None,
                        };
                        if let Some(s) = stack {
                            DiagnosticDisclosureMode::current().redact_error_text(&s)
                        } else {
                            let name = match rt.object_get(*oid, "name") {
                                Value::String(s) => s.to_string(),
                                _ => String::new(),
                            };
                            let msg = match rt.object_get(*oid, "message") {
                                Value::String(s) => s.to_string(),
                                _ => String::new(),
                            };
                            match (name.is_empty(), msg.is_empty()) {
                                (false, false) => DiagnosticDisclosureMode::current()
                                    .redact_error_text(&format!("{name}: {msg}")),
                                (false, true) => name,
                                (true, false) => {
                                    DiagnosticDisclosureMode::current().redact_error_text(&msg)
                                }
                                (true, true) => format!("{reason:?}"),
                            }
                        }
                    }
                    Value::String(s) => s.to_string(),
                    other => format!("{other:?}"),
                });
            }
            for line in &rendered {
                eprintln!("cruft: unhandled promise rejection: {line}");
            }
        }
        if fatal {
            drain_audit_log(&rt, audit_log_path.as_deref());
            return ExitCode::from(1);
        }
    }

    drain_audit_log(&rt, audit_log_path.as_deref());

    for _ in 0..1000 {
        let code = cruft::process::current_exit_code(&mut rt).unwrap_or(0);
        let ran = cruft::process::emit_process_event(
            &mut rt,
            "beforeExit",
            vec![Value::Number(code as f64)],
        );
        if !ran || !cruft::timer::has_pending(&rt) {
            break;
        }
        if drive_main_agent(&mut rt).is_err() {
            break;
        }
    }

    let pre_code = cruft::process::current_exit_code(&mut rt).unwrap_or(0);
    cruft::process::emit_process_event(&mut rt, "exit", vec![Value::Number(pre_code as f64)]);
    for warning in rusty_js_runtime::interp::drain_pending_node_warnings() {
        eprintln!("{}", warning);
    }
    match cruft::process::current_exit_code(&mut rt) {
        Some(n) => ExitCode::from((n & 0xff) as u8),
        None => ExitCode::SUCCESS,
    }
}
