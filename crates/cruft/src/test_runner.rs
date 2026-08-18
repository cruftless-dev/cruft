
use rusty_js_runtime::Runtime;

const TEST_RUNNER_JS: &str = include_str!("test_runner.js");

pub fn install(rt: &mut Runtime) {
    if let Err(e) = rt.run_script(TEST_RUNNER_JS, "cruft:internal/test_runner.js") {
        eprintln!("[cruft] node:test install failed: {e:?}");
    }
}
