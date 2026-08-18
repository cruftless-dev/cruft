
use rusty_js_runtime::interp::Runtime;
use rusty_js_runtime::Value;
use rusty_js_runtime::value::JsString;
use std::rc::Rc;

const DENO_PRELUDE: &str = include_str!("deno_prelude.js");

pub fn install(rt: &mut Runtime) {

    if matches!(std::env::var("CRUFT_DENO_COMPAT").as_deref(), Ok("0") | Ok("false")) {
        return;
    }
    let mode = std::env::var("CRUFT_CAPS_MODE").unwrap_or_else(|_| "compat".to_string());
    rt.define_global_property("__cruft_caps_mode", Value::String(Rc::new(JsString::from(mode))));
    if let Err(e) = rt.run_script(DENO_PRELUDE, "cruft:internal/deno.js") {
        eprintln!("[cruft] Deno compat prelude failed: {e:?}");
    }
}
