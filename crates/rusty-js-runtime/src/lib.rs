
pub mod abstract_ops;
pub mod agent262;
pub(crate) mod big5_table;
pub mod bigint;
pub mod caps;
pub mod caps_config;
pub mod cjs_export_resolution;
pub(crate) mod euc_jp_table;
pub(crate) mod euc_kr_table;
pub(crate) mod gb18030_table;
pub mod interp;
pub mod interp_ic_table;
pub mod agent_reactor;
pub mod agent_scheduler;
pub mod intl_datetime_patterns;
#[path = "intl_japanese_era_generated.rs"]
pub(crate) mod intl_japanese_era_generated;
pub mod intl_locale_data;
pub mod intl_segmenter;
pub mod intrinsics;
pub mod iterator;
pub mod job_queue;
pub mod module;
pub mod module_map_bridge;
pub mod napi;
pub mod native_api_manifest;
pub(crate) mod native_api_manifest_generated;
pub mod promise;
pub mod prototype;
pub mod realm_adapter;
pub mod regexp;
pub mod rusty_js_regex;
pub mod send_ir;
pub(crate) mod shift_jis_table;
pub mod value;
pub mod worker_realm;

mod generated_unicode {
    pub(crate) mod property_escapes;
}

pub use job_queue::{HostEnqueuePhase, Job, JobKind, JobQueue};
pub use module::{detect_module_kind, HostHook, ModuleKind, ModuleStatus};

pub use crate::value::{InternalKind, Object, ObjectRef, PropertyDescriptor, Value};
pub use interp::{set_node_compat_entry_dir, AgentId, RealmCollectionError, Runtime, RuntimeError};

pub fn run_module(src: &str) -> Result<Value, RuntimeError> {
    let mut return_probe_slot = false;
    let module = match rusty_js_bytecode::compile_module(src) {
        Ok(module) => module,
        Err(e) if e.message.contains("Illegal return statement") => {

            return_probe_slot = true;
            let wrapped =
                format!("globalThis.__cruft_run_module_result = (function() {{\n{src}\n}})();");
            rusty_js_bytecode::compile_module(&wrapped)
                .map_err(|e| RuntimeError::CompileError(format!("{}", e.message)))?
        }
        Err(e) => return Err(RuntimeError::CompileError(format!("{}", e.message))),
    };
    let mut rt = Runtime::new();
    rt.install_intrinsics();
    let result = rt.run_module(&module)?;
    if return_probe_slot {
        Ok(rt.global_get("__cruft_run_module_result"))
    } else {
        Ok(result)
    }
}
pub mod generated;
