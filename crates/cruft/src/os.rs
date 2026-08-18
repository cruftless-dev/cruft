
use crate::register::{new_object, register_method, set_constant};
use rusty_js_runtime::caps;
use rusty_js_runtime::caps::{ModuleId, ModuleProvenance};
use rusty_js_runtime::value::{Object, ObjectRef};
use rusty_js_runtime::{Runtime, RuntimeError, Value};
use std::collections::HashMap;
use std::ffi::CStr;
use std::net::{Ipv4Addr, Ipv6Addr};
use std::rc::Rc;

fn check_env(rt: &Runtime, op: caps::EnvOp) -> Result<(), RuntimeError> {
    let url = rt.current_module_url.last().cloned().unwrap_or_default();
    let provenance = if url.contains("/node_modules/") {
        ModuleProvenance::Dependency
    } else if url.starts_with("node:") {
        ModuleProvenance::Builtin
    } else {
        ModuleProvenance::Application
    };
    let caller = ModuleId { url, provenance };
    rt.caps
        .require_env(&caps::Env::none(), op, &caller)
        .map_err(|e| RuntimeError::TypeError(e.to_string()))
}

#[cfg(windows)]
pub(crate) const OS_EOL: &str = "\r\n";
#[cfg(not(windows))]
pub(crate) const OS_EOL: &str = "\n";

#[cfg(windows)]
const HOMEDIR_ENV: &str = "USERPROFILE";
#[cfg(not(windows))]
const HOMEDIR_ENV: &str = "HOME";

#[cfg(windows)]
const TMPDIR_ENV: &str = "TEMP";
#[cfg(not(windows))]
const TMPDIR_ENV: &str = "TMPDIR";

fn js_process_env_var(rt: &mut Runtime, name: &str) -> Option<String> {
    let Value::Object(process) = rt.global_get("process") else {
        return None;
    };
    let Value::Object(env) = rt.object_get(process, "env") else {
        return None;
    };
    match rt.object_get(env, name) {
        Value::String(s) if !s.is_empty() => Some(s.as_str().to_string()),
        _ => None,
    }
}

fn os_homedir_fallback() -> String {
    crate::platform::user_home_dir()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| {
            if cfg!(windows) {
                "C:\\".to_string()
            } else {
                "/".to_string()
            }
        })
}

fn os_homedir_from_rt(rt: &mut Runtime) -> String {
    if let Some(home) = js_process_env_var(rt, HOMEDIR_ENV) {
        return home;
    }

    #[cfg(windows)]
    {
        if let Some(profile) = js_process_env_var(rt, "USERPROFILE") {
            return profile;
        }
        if let (Some(drive), Some(path)) = (
            js_process_env_var(rt, "HOMEDRIVE"),
            js_process_env_var(rt, "HOMEPATH"),
        ) {
            return format!("{drive}{path}");
        }
    }

    os_homedir_fallback()
}

fn os_tmpdir() -> String {
    #[cfg(windows)]
    {
        std::env::var("TEMP")
            .or_else(|_| std::env::var("TMP"))
            .unwrap_or_else(|_| "C:\\Windows\\Temp".to_string())
    }
    #[cfg(not(windows))]
    {
        std::env::var("TMPDIR").unwrap_or_else(|_| "/tmp".to_string())
    }
}

pub fn install_canonical(rt: &mut Runtime) {
    let os = new_object(rt);

    register_method(rt, os, "platform", |_rt, _args| {
        Ok(Value::String(Rc::new(
            rusty_js_runtime::value::JsString::from(detect_platform().to_string()),
        )))
    });
    register_method(rt, os, "arch", |_rt, _args| {
        Ok(Value::String(Rc::new(
            rusty_js_runtime::value::JsString::from(detect_arch().to_string()),
        )))
    });
    register_method(rt, os, "type", |_rt, _args| {
        Ok(Value::String(Rc::new(
            rusty_js_runtime::value::JsString::from(detect_os_type().to_string()),
        )))
    });
    register_method(rt, os, "release", |rt, _args| {
        check_env(rt, caps::EnvOp::SystemInfo("release"))?;
        Ok(Value::String(Rc::new(
            rusty_js_runtime::value::JsString::from(host_release()),
        )))
    });
    register_method(rt, os, "version", |rt, _args| {
        check_env(rt, caps::EnvOp::SystemInfo("version"))?;
        Ok(Value::String(Rc::new(
            rusty_js_runtime::value::JsString::from(host_version()),
        )))
    });
    register_method(rt, os, "hostname", |rt, _args| {
        check_env(rt, caps::EnvOp::SystemInfo("hostname"))?;
        let h = std::env::var("HOSTNAME").unwrap_or_else(|_| "localhost".to_string());
        Ok(Value::String(Rc::new(
            rusty_js_runtime::value::JsString::from(h),
        )))
    });
    register_method(rt, os, "homedir", |rt, _args| {
        check_env(rt, caps::EnvOp::ReadVar(HOMEDIR_ENV.into()))?;
        let h = os_homedir_from_rt(rt);
        Ok(Value::String(Rc::new(
            rusty_js_runtime::value::JsString::from(h),
        )))
    });
    register_method(rt, os, "tmpdir", |rt, _args| {
        check_env(rt, caps::EnvOp::ReadVar(TMPDIR_ENV.into()))?;
        let t = os_tmpdir();
        Ok(Value::String(Rc::new(
            rusty_js_runtime::value::JsString::from(t),
        )))
    });
    register_method(rt, os, "endianness", |_rt, _args| {
        Ok(Value::String(Rc::new(
            rusty_js_runtime::value::JsString::from(if cfg!(target_endian = "little") {
                "LE"
            } else {
                "BE"
            }),
        )))
    });
    register_method(rt, os, "cpus", |rt, _args| {
        check_env(rt, caps::EnvOp::SystemInfo("cpus"))?;
        let cpus = host_cpus();
        let arr = rt.alloc_object(rusty_js_runtime::value::Object::new_array());
        for (i, entry) in cpus.iter().enumerate() {
            let cpu = rt.alloc_object(rusty_js_runtime::value::Object::new_ordinary());
            rt.object_set(
                cpu,
                "model".into(),
                Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                    entry.model.clone(),
                ))),
            );
            rt.object_set(cpu, "speed".into(), Value::Number(entry.speed_mhz));
            let times = rt.alloc_object(rusty_js_runtime::value::Object::new_ordinary());
            rt.object_set(times, "user".into(), Value::Number(entry.times.user));
            rt.object_set(times, "nice".into(), Value::Number(entry.times.nice));
            rt.object_set(times, "sys".into(), Value::Number(entry.times.sys));
            rt.object_set(times, "idle".into(), Value::Number(entry.times.idle));
            rt.object_set(times, "irq".into(), Value::Number(entry.times.irq));
            rt.object_set(cpu, "times".into(), Value::Object(times));
            rt.object_set(arr, i.to_string(), Value::Object(cpu));
        }
        rt.object_set(arr, "length".into(), Value::Number(cpus.len() as f64));
        Ok(Value::Object(arr))
    });
    register_method(rt, os, "totalmem", |rt, _args| {
        check_env(rt, caps::EnvOp::SystemInfo("totalmem"))?;
        Ok(Value::Number(host_memory().0 as f64))
    });
    register_method(rt, os, "freemem", |rt, _args| {
        check_env(rt, caps::EnvOp::SystemInfo("freemem"))?;
        Ok(Value::Number(host_memory().1 as f64))
    });
    register_method(rt, os, "uptime", |rt, _args| {
        check_env(rt, caps::EnvOp::SystemInfo("uptime"))?;
        Ok(Value::Number(host_uptime()))
    });
    register_method(rt, os, "availableParallelism", |_rt, _args| {

        let n = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1);
        Ok(Value::Number(n as f64))
    });
    set_constant(
        rt,
        os,
        "EOL",
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
            OS_EOL.to_string(),
        ))),
    );

    rt.define_global_property("__cruft_os", Value::Object(os));
}

pub fn install(rt: &mut Runtime) {
    let os = new_object(rt);

    register_method(rt, os, "platform", |_rt, _args| {
        Ok(Value::String(Rc::new(
            rusty_js_runtime::value::JsString::from(detect_platform().to_string()),
        )))
    });
    register_method(rt, os, "arch", |_rt, _args| {
        Ok(Value::String(Rc::new(
            rusty_js_runtime::value::JsString::from(detect_arch().to_string()),
        )))
    });
    register_method(rt, os, "type", |_rt, _args| {
        Ok(Value::String(Rc::new(
            rusty_js_runtime::value::JsString::from(detect_os_type().to_string()),
        )))
    });
    register_method(rt, os, "release", |rt, _args| {
        check_env(rt, caps::EnvOp::SystemInfo("release"))?;
        Ok(Value::String(Rc::new(
            rusty_js_runtime::value::JsString::from(host_release()),
        )))
    });
    register_method(rt, os, "version", |rt, _args| {
        check_env(rt, caps::EnvOp::SystemInfo("version"))?;
        Ok(Value::String(Rc::new(
            rusty_js_runtime::value::JsString::from(host_version()),
        )))
    });
    register_method(rt, os, "machine", |rt, _args| {
        check_env(rt, caps::EnvOp::SystemInfo("machine"))?;
        Ok(Value::String(Rc::new(
            rusty_js_runtime::value::JsString::from(host_machine()),
        )))
    });
    register_method(rt, os, "hostname", |rt, _args| {
        check_env(rt, caps::EnvOp::SystemInfo("hostname"))?;
        let h = std::env::var("HOSTNAME").unwrap_or_else(|_| "localhost".to_string());
        Ok(Value::String(Rc::new(
            rusty_js_runtime::value::JsString::from(h),
        )))
    });
    register_method(rt, os, "homedir", |rt, _args| {
        check_env(rt, caps::EnvOp::ReadVar(HOMEDIR_ENV.into()))?;
        let h = os_homedir_from_rt(rt);
        Ok(Value::String(Rc::new(
            rusty_js_runtime::value::JsString::from(h),
        )))
    });
    register_method(rt, os, "tmpdir", |rt, _args| {
        check_env(rt, caps::EnvOp::ReadVar(TMPDIR_ENV.into()))?;
        let t = os_tmpdir();
        Ok(Value::String(Rc::new(
            rusty_js_runtime::value::JsString::from(t),
        )))
    });
    register_method(rt, os, "endianness", |_rt, _args| {
        Ok(Value::String(Rc::new(
            rusty_js_runtime::value::JsString::from(if cfg!(target_endian = "little") {
                "LE"
            } else {
                "BE"
            }),
        )))
    });

    set_constant(
        rt,
        os,
        "EOL",
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
            OS_EOL.to_string(),
        ))),
    );
    set_constant(
        rt,
        os,
        "devNull",
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
            dev_null_path(),
        ))),
    );

    register_method(rt, os, "getPriority", |_rt, _args| Ok(Value::Number(0.0)));
    register_method(rt, os, "setPriority", |_rt, _args| Ok(Value::Undefined));
    register_method(rt, os, "cpus", |rt, _args| {
        check_env(rt, caps::EnvOp::SystemInfo("cpus"))?;
        let cpus = host_cpus();
        let arr = rt.alloc_object(rusty_js_runtime::value::Object::new_array());
        for (i, entry) in cpus.iter().enumerate() {
            let cpu = rt.alloc_object(rusty_js_runtime::value::Object::new_ordinary());
            rt.object_set(
                cpu,
                "model".into(),
                Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                    entry.model.clone(),
                ))),
            );
            rt.object_set(cpu, "speed".into(), Value::Number(entry.speed_mhz));
            let times = rt.alloc_object(rusty_js_runtime::value::Object::new_ordinary());
            rt.object_set(times, "user".into(), Value::Number(entry.times.user));
            rt.object_set(times, "nice".into(), Value::Number(entry.times.nice));
            rt.object_set(times, "sys".into(), Value::Number(entry.times.sys));
            rt.object_set(times, "idle".into(), Value::Number(entry.times.idle));
            rt.object_set(times, "irq".into(), Value::Number(entry.times.irq));
            rt.object_set(cpu, "times".into(), Value::Object(times));
            rt.object_set(arr, i.to_string(), Value::Object(cpu));
        }
        rt.object_set(arr, "length".into(), Value::Number(cpus.len() as f64));
        Ok(Value::Object(arr))
    });
    register_method(rt, os, "totalmem", |rt, _args| {
        check_env(rt, caps::EnvOp::SystemInfo("totalmem"))?;
        Ok(Value::Number(host_memory().0 as f64))
    });
    register_method(rt, os, "freemem", |rt, _args| {
        check_env(rt, caps::EnvOp::SystemInfo("freemem"))?;
        Ok(Value::Number(host_memory().1 as f64))
    });
    register_method(rt, os, "uptime", |rt, _args| {
        check_env(rt, caps::EnvOp::SystemInfo("uptime"))?;
        Ok(Value::Number(host_uptime()))
    });

    register_method(rt, os, "availableParallelism", |_rt, _args| {

        let n = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1);
        Ok(Value::Number(n as f64))
    });
    register_method(rt, os, "loadavg", |rt, _args| {
        check_env(rt, caps::EnvOp::SystemInfo("loadavg"))?;
        let loads = host_loadavg();
        let arr = rt.alloc_object(rusty_js_runtime::value::Object::new_array());
        for (i, load) in loads.iter().enumerate() {
            rt.object_set(arr, i.to_string(), Value::Number(*load));
        }
        rt.object_set(arr, "length".into(), Value::Number(3.0));
        Ok(Value::Object(arr))
    });
    register_method(rt, os, "networkInterfaces", |rt, _args| {
        check_env(rt, caps::EnvOp::SystemInfo("networkInterfaces"))?;
        Ok(Value::Object(network_interfaces(rt)))
    });
    register_method(rt, os, "userInfo", |rt, args| {
        check_env(rt, caps::EnvOp::SystemInfo("userInfo"))?;
        let buffer_encoding = user_info_wants_buffer(rt, args.first());
        let o = rt.alloc_object(rusty_js_runtime::value::Object::new_ordinary());
        let info = host_user_info();
        set_user_info_field(rt, o, "username", &info.username, buffer_encoding);
        rt.object_set(o, "uid".into(), Value::Number(info.uid as f64));
        rt.object_set(o, "gid".into(), Value::Number(info.gid as f64));
        set_user_info_field(rt, o, "shell", &info.shell, buffer_encoding);
        set_user_info_field(rt, o, "homedir", &info.homedir, buffer_encoding);
        Ok(Value::Object(o))
    });

    let constants = new_object(rt);
    let signals = new_object(rt);

    for (name, num) in &[
        ("SIGHUP", 1),
        ("SIGINT", 2),
        ("SIGQUIT", 3),
        ("SIGILL", 4),
        ("SIGTRAP", 5),
        ("SIGABRT", 6),
        ("SIGIOT", 6),
        ("SIGBUS", 7),
        ("SIGFPE", 8),
        ("SIGKILL", 9),
        ("SIGUSR1", 10),
        ("SIGSEGV", 11),
        ("SIGUSR2", 12),
        ("SIGPIPE", 13),
        ("SIGALRM", 14),
        ("SIGTERM", 15),
        ("SIGSTKFLT", 16),
        ("SIGCHLD", 17),
        ("SIGCONT", 18),
        ("SIGSTOP", 19),
        ("SIGTSTP", 20),
        ("SIGTTIN", 21),
        ("SIGTTOU", 22),
        ("SIGURG", 23),
        ("SIGXCPU", 24),
        ("SIGXFSZ", 25),
        ("SIGVTALRM", 26),
        ("SIGPROF", 27),
        ("SIGWINCH", 28),
        ("SIGIO", 29),
        ("SIGPOLL", 29),
        ("SIGPWR", 30),
        ("SIGSYS", 31),
    ] {
        set_constant(rt, signals, name, Value::Number(*num as f64));
    }
    set_constant(rt, constants, "signals", Value::Object(signals));

    let errno = new_object(rt);
    for (name, num) in &[
        ("E2BIG", 7),
        ("EACCES", 13),
        ("EADDRINUSE", 98),
        ("EADDRNOTAVAIL", 99),
        ("EAFNOSUPPORT", 97),
        ("EAGAIN", 11),
        ("EALREADY", 114),
        ("EBADF", 9),
        ("EBADMSG", 74),
        ("EBUSY", 16),
        ("ECANCELED", 125),
        ("ECHILD", 10),
        ("ECONNABORTED", 103),
        ("ECONNREFUSED", 111),
        ("ECONNRESET", 104),
        ("EDEADLK", 35),
        ("EDESTADDRREQ", 89),
        ("EDOM", 33),
        ("EDQUOT", 122),
        ("EEXIST", 17),
        ("EFAULT", 14),
        ("EFBIG", 27),
        ("EHOSTUNREACH", 113),
        ("EIDRM", 43),
        ("EILSEQ", 84),
        ("EINPROGRESS", 115),
        ("EINTR", 4),
        ("EINVAL", 22),
        ("EIO", 5),
        ("EISCONN", 106),
        ("EISDIR", 21),
        ("ELOOP", 40),
        ("EMFILE", 24),
        ("EMLINK", 31),
        ("EMSGSIZE", 90),
        ("EMULTIHOP", 72),
        ("ENAMETOOLONG", 36),
        ("ENETDOWN", 100),
        ("ENETRESET", 102),
        ("ENETUNREACH", 101),
        ("ENFILE", 23),
        ("ENOBUFS", 105),
        ("ENODATA", 61),
        ("ENODEV", 19),
        ("ENOENT", 2),
        ("ENOEXEC", 8),
        ("ENOLCK", 37),
        ("ENOLINK", 67),
        ("ENOMEM", 12),
        ("ENOMSG", 42),
        ("ENOPROTOOPT", 92),
        ("ENOSPC", 28),
        ("ENOSR", 63),
        ("ENOSTR", 60),
        ("ENOSYS", 38),
        ("ENOTCONN", 107),
        ("ENOTDIR", 20),
        ("ENOTEMPTY", 39),
        ("ENOTSOCK", 88),
        ("ENOTSUP", 95),
        ("ENOTTY", 25),
        ("ENXIO", 6),
        ("EOPNOTSUPP", 95),
        ("EOVERFLOW", 75),
        ("EPERM", 1),
        ("EPIPE", 32),
        ("EPROTO", 71),
        ("EPROTONOSUPPORT", 93),
        ("EPROTOTYPE", 91),
        ("ERANGE", 34),
        ("EROFS", 30),
        ("ESPIPE", 29),
        ("ESRCH", 3),
        ("ESTALE", 116),
        ("ETIME", 62),
        ("ETIMEDOUT", 110),
        ("ETXTBSY", 26),
        ("EWOULDBLOCK", 11),
        ("EXDEV", 18),
    ] {
        set_constant(rt, errno, name, Value::Number(*num as f64));
    }
    set_constant(rt, constants, "errno", Value::Object(errno));

    let priority = new_object(rt);
    for (name, num) in &[
        ("PRIORITY_LOW", 19),
        ("PRIORITY_BELOW_NORMAL", 10),
        ("PRIORITY_NORMAL", 0),
        ("PRIORITY_ABOVE_NORMAL", -7),
        ("PRIORITY_HIGH", -14),
        ("PRIORITY_HIGHEST", -20),
    ] {
        set_constant(rt, priority, name, Value::Number(*num as f64));
    }
    set_constant(rt, constants, "priority", Value::Object(priority));

    let dlopen = new_object(rt);
    for (name, num) in &[
        ("RTLD_LAZY", 1),
        ("RTLD_NOW", 2),
        ("RTLD_GLOBAL", 256),
        ("RTLD_LOCAL", 0),
        ("RTLD_DEEPBIND", 8),
    ] {
        set_constant(rt, dlopen, name, Value::Number(*num as f64));
    }
    set_constant(rt, constants, "dlopen", Value::Object(dlopen));

    set_constant(rt, constants, "UV_UDP_REUSEADDR", Value::Number(4.0));

    set_constant(rt, os, "constants", Value::Object(constants));

    set_constant(
        rt,
        os,
        "EOL",
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(OS_EOL))),
    );

    rt.define_global_property("os", Value::Object(os));
}

fn host_version() -> String {
    #[cfg(unix)]
    {
        uname_field(|u| &u.version)
    }
    #[cfg(not(unix))]
    {
        "unknown".to_string()
    }
}

fn host_release() -> String {
    #[cfg(unix)]
    {
        uname_field(|u| &u.release)
    }
    #[cfg(not(unix))]
    {
        "unknown".to_string()
    }
}

fn host_machine() -> String {
    #[cfg(unix)]
    {
        uname_field(|u| &u.machine)
    }
    #[cfg(not(unix))]
    {
        detect_arch().to_string()
    }
}

#[derive(Clone, Debug)]
struct CpuTimes {
    user: f64,
    nice: f64,
    sys: f64,
    idle: f64,
    irq: f64,
}

#[derive(Clone, Debug)]
struct CpuEntry {
    model: String,
    speed_mhz: f64,
    times: CpuTimes,
}

fn host_memory() -> (u64, u64) {
    #[cfg(target_os = "linux")]
    unsafe {
        let mut info = std::mem::zeroed::<libc::sysinfo>();
        if libc::sysinfo(&mut info) == 0 {
            let unit = if info.mem_unit == 0 {
                1
            } else {
                info.mem_unit as u64
            };
            return (
                info.totalram.saturating_mul(unit),
                info.freeram.saturating_mul(unit),
            );
        }
    }
    #[cfg(target_os = "macos")]
    unsafe {
        let total = sysctl_u64("hw.memsize").unwrap_or(0);
        let free_pages = sysctl_u64("vm.page_free_count").unwrap_or(0);
        let page_size = libc::sysconf(libc::_SC_PAGESIZE);
        if total > 0 && free_pages > 0 && page_size > 0 {
            return (total, free_pages.saturating_mul(page_size as u64));
        }
        if total > 0 {
            return (total, 0);
        }
    }
    (0, 0)
}

#[cfg(target_os = "macos")]
unsafe fn sysctl_u64(name: &str) -> Option<u64> {
    let c_name = std::ffi::CString::new(name).ok()?;
    let mut value64 = 0_u64;
    let mut len = std::mem::size_of::<u64>();
    if libc::sysctlbyname(
        c_name.as_ptr(),
        &mut value64 as *mut _ as *mut libc::c_void,
        &mut len,
        std::ptr::null_mut(),
        0,
    ) == 0
        && len == std::mem::size_of::<u64>()
    {
        return Some(value64);
    }

    let mut value32 = 0_u32;
    len = std::mem::size_of::<u32>();
    if libc::sysctlbyname(
        c_name.as_ptr(),
        &mut value32 as *mut _ as *mut libc::c_void,
        &mut len,
        std::ptr::null_mut(),
        0,
    ) == 0
        && len == std::mem::size_of::<u32>()
    {
        return Some(value32 as u64);
    }
    None
}

fn host_uptime() -> f64 {
    #[cfg(target_os = "linux")]
    unsafe {
        let mut info = std::mem::zeroed::<libc::sysinfo>();
        if libc::sysinfo(&mut info) == 0 {
            return info.uptime as f64;
        }
    }
    0.0
}

fn host_loadavg() -> [f64; 3] {
    #[cfg(unix)]
    unsafe {
        let mut loads = [0.0_f64; 3];
        if libc::getloadavg(loads.as_mut_ptr(), loads.len() as libc::c_int) == 3 {
            return loads;
        }
    }
    [0.0, 0.0, 0.0]
}

fn host_cpus() -> Vec<CpuEntry> {
    #[cfg(target_os = "linux")]
    {
        let mut cpus = parse_linux_cpuinfo();
        let times = parse_linux_cpu_times();
        let fallback_count = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1);
        let count = cpus.len().max(times.len()).max(fallback_count);
        cpus.resize_with(count, || CpuEntry {
            model: "Unknown CPU".to_string(),
            speed_mhz: 0.0,
            times: CpuTimes {
                user: 0.0,
                nice: 0.0,
                sys: 0.0,
                idle: 0.0,
                irq: 0.0,
            },
        });
        for (idx, time) in times.into_iter().enumerate() {
            if let Some(cpu) = cpus.get_mut(idx) {
                cpu.times = time;
            }
        }
        return cpus;
    }
    #[allow(unreachable_code)]
    {
        let count = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1);
        (0..count)
            .map(|_| CpuEntry {
                model: "Unknown CPU".to_string(),
                speed_mhz: 0.0,
                times: CpuTimes {
                    user: 0.0,
                    nice: 0.0,
                    sys: 0.0,
                    idle: 0.0,
                    irq: 0.0,
                },
            })
            .collect()
    }
}

#[cfg(target_os = "linux")]
fn parse_linux_cpuinfo() -> Vec<CpuEntry> {
    let Ok(text) = std::fs::read_to_string("/proc/cpuinfo") else {
        return Vec::new();
    };
    let mut cpus = Vec::new();
    let mut model = String::new();
    let mut hardware = String::new();
    let mut speed_mhz = 0.0;

    for line in text.lines().chain(std::iter::once("")) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if !model.is_empty() || !hardware.is_empty() || speed_mhz > 0.0 {
                let name = if !model.is_empty() {
                    model.clone()
                } else if !hardware.is_empty() {
                    hardware.clone()
                } else {
                    "Unknown CPU".to_string()
                };
                cpus.push(CpuEntry {
                    model: name,
                    speed_mhz,
                    times: CpuTimes {
                        user: 0.0,
                        nice: 0.0,
                        sys: 0.0,
                        idle: 0.0,
                        irq: 0.0,
                    },
                });
            }
            model.clear();
            speed_mhz = 0.0;
            continue;
        }

        let Some((key, value)) = trimmed.split_once(':') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim();
        match key {
            "model name" | "Processor" => model = value.to_string(),
            "Hardware" if hardware.is_empty() => hardware = value.to_string(),
            "cpu MHz" | "BogoMIPS" => {
                if let Ok(parsed) = value.parse::<f64>() {
                    speed_mhz = parsed;
                }
            }
            _ => {}
        }
    }

    if cpus.is_empty() && !hardware.is_empty() {
        let count = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1);
        cpus = (0..count)
            .map(|_| CpuEntry {
                model: hardware.clone(),
                speed_mhz,
                times: CpuTimes {
                    user: 0.0,
                    nice: 0.0,
                    sys: 0.0,
                    idle: 0.0,
                    irq: 0.0,
                },
            })
            .collect();
    }
    cpus
}

#[cfg(target_os = "linux")]
fn parse_linux_cpu_times() -> Vec<CpuTimes> {
    let Ok(text) = std::fs::read_to_string("/proc/stat") else {
        return Vec::new();
    };
    let ticks_ms = clock_tick_ms();
    text.lines()
        .filter_map(|line| {
            let mut parts = line.split_whitespace();
            let name = parts.next()?;
            if !name.starts_with("cpu") || name == "cpu" {
                return None;
            }
            let suffix = &name[3..];
            if suffix.is_empty() || !suffix.chars().all(|c| c.is_ascii_digit()) {
                return None;
            }
            let vals: Vec<f64> = parts
                .take(7)
                .map(|p| p.parse::<f64>().unwrap_or(0.0) * ticks_ms)
                .collect();
            Some(CpuTimes {
                user: *vals.first().unwrap_or(&0.0),
                nice: *vals.get(1).unwrap_or(&0.0),
                sys: *vals.get(2).unwrap_or(&0.0),
                idle: *vals.get(3).unwrap_or(&0.0),
                irq: vals.get(5).unwrap_or(&0.0) + vals.get(6).unwrap_or(&0.0),
            })
        })
        .collect()
}

#[cfg(target_os = "linux")]
fn clock_tick_ms() -> f64 {
    unsafe {
        let ticks = libc::sysconf(libc::_SC_CLK_TCK);
        if ticks > 0 {
            return 1000.0 / ticks as f64;
        }
    }
    10.0
}

#[derive(Clone, Debug)]
struct UserInfo {
    username: String,
    uid: u32,
    gid: u32,
    shell: String,
    homedir: String,
}

fn host_user_info() -> UserInfo {
    #[cfg(unix)]
    unsafe {
        let uid = libc::geteuid();
        let mut pwd = std::mem::zeroed::<libc::passwd>();
        let mut result: *mut libc::passwd = std::ptr::null_mut();
        let mut buf_len = libc::sysconf(libc::_SC_GETPW_R_SIZE_MAX);
        if buf_len < 1024 {
            buf_len = 16384;
        }

        let mut buf = vec![0 as libc::c_char; buf_len as usize];
        if libc::getpwuid_r(uid, &mut pwd, buf.as_mut_ptr(), buf.len(), &mut result) == 0
            && !result.is_null()
        {
            return UserInfo {
                username: c_string_field(pwd.pw_name, "user"),
                uid: pwd.pw_uid,
                gid: pwd.pw_gid,
                shell: c_string_field(pwd.pw_shell, "/bin/sh"),
                homedir: c_string_field(pwd.pw_dir, "/"),
            };
        }
    }
    UserInfo {
        username: std::env::var("USERNAME")
            .or_else(|_| std::env::var("USER"))
            .unwrap_or_else(|_| "user".into()),
        uid: 1000,
        gid: 1000,
        shell: "/bin/sh".into(),
        homedir: os_homedir_fallback(),
    }
}

#[cfg(unix)]
unsafe fn c_string_field(ptr: *const libc::c_char, fallback: &str) -> String {
    if ptr.is_null() {
        return fallback.to_string();
    }
    CStr::from_ptr(ptr).to_string_lossy().into_owned()
}

fn user_info_wants_buffer(rt: &Runtime, value: Option<&Value>) -> bool {
    let Some(Value::Object(id)) = value else {
        return false;
    };
    matches!(
        rt.object_get(*id, "encoding"),
        Value::String(s) if s.as_str() == "buffer"
    )
}

fn set_user_info_field(rt: &mut Runtime, obj: ObjectRef, name: &str, value: &str, as_buffer: bool) {
    let value = if as_buffer {
        os_buffer_from_bytes(rt, value.as_bytes())
    } else {
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
            value.to_string(),
        )))
    };
    rt.object_set(obj, name.into(), value);
}

fn os_buffer_from_bytes(rt: &mut Runtime, bytes: &[u8]) -> Value {
    let mut o = Object::new_ordinary();
    o.set_own("length".into(), Value::Number(bytes.len() as f64));
    o.set_own_internal("__is_buffer__".into(), Value::Boolean(true));
    for (i, b) in bytes.iter().enumerate() {
        o.set_own(i.to_string(), Value::Number(*b as f64));
    }
    let id = rt.alloc_object(o);
    crate::node_stubs::install_buffer_methods(rt, id);
    Value::Object(id)
}

#[cfg(unix)]
fn uname_field(field: impl FnOnce(&libc::utsname) -> &[libc::c_char]) -> String {
    unsafe {
        let mut uts = std::mem::zeroed::<libc::utsname>();
        if libc::uname(&mut uts) != 0 {
            return "unknown".to_string();
        }
        CStr::from_ptr(field(&uts).as_ptr())
            .to_string_lossy()
            .into_owned()
    }
}

fn dev_null_path() -> String {
    if cfg!(windows) {
        r"\\.\nul".to_string()
    } else {
        "/dev/null".to_string()
    }
}

fn network_interfaces(rt: &mut Runtime) -> ObjectRef {
    let root = rt.alloc_object(rusty_js_runtime::value::Object::new_ordinary());
    for entry in collect_network_interfaces() {
        let arr = match rt.object_get(root, &entry.name) {
            Value::Object(obj) => obj,
            Value::Undefined => {
                let arr = rt.alloc_object(rusty_js_runtime::value::Object::new_array());
                rt.object_set(root, entry.name.clone(), Value::Object(arr));
                arr
            }
            _ => continue,
        };
        let len = match rt.object_get(arr, "length") {
            Value::Number(n) if n.is_finite() && n >= 0.0 => n as usize,
            _ => 0,
        };
        let item = rt.alloc_object(rusty_js_runtime::value::Object::new_ordinary());
        rt.object_set(
            item,
            "address".into(),
            Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                entry.address,
            ))),
        );
        rt.object_set(
            item,
            "netmask".into(),
            Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                entry.netmask,
            ))),
        );
        rt.object_set(
            item,
            "family".into(),
            Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                entry.family,
            ))),
        );
        rt.object_set(
            item,
            "mac".into(),
            Value::String(Rc::new(rusty_js_runtime::value::JsString::from(entry.mac))),
        );
        rt.object_set(item, "internal".into(), Value::Boolean(entry.internal));
        rt.object_set(
            item,
            "cidr".into(),
            Value::String(Rc::new(rusty_js_runtime::value::JsString::from(entry.cidr))),
        );
        if let Some(scopeid) = entry.scopeid {
            rt.object_set(item, "scopeid".into(), Value::Number(scopeid as f64));
        }
        rt.object_set(arr, len.to_string(), Value::Object(item));
        rt.object_set(arr, "length".into(), Value::Number((len + 1) as f64));
    }
    root
}

#[derive(Clone, Debug)]
struct NetInterfaceEntry {
    name: String,
    address: String,
    netmask: String,
    family: String,
    mac: String,
    internal: bool,
    cidr: String,
    scopeid: Option<u32>,
}

#[cfg(unix)]
fn collect_network_interfaces() -> Vec<NetInterfaceEntry> {
    unsafe {
        let mut ifap: *mut libc::ifaddrs = std::ptr::null_mut();
        if libc::getifaddrs(&mut ifap) != 0 || ifap.is_null() {
            return Vec::new();
        }

        let mut macs = HashMap::new();
        let mut cur = ifap;
        while !cur.is_null() {
            let ifa = &*cur;
            if !ifa.ifa_addr.is_null() {
                #[cfg(target_os = "linux")]
                if (*ifa.ifa_addr).sa_family as i32 == libc::AF_PACKET {
                    if let Some(name) = iface_name(ifa) {
                        if let Some(mac) = packet_mac(ifa.ifa_addr) {
                            macs.insert(name, mac);
                        }
                    }
                }
            }
            cur = ifa.ifa_next;
        }

        let mut entries = Vec::new();
        cur = ifap;
        while !cur.is_null() {
            let ifa = &*cur;
            if !ifa.ifa_addr.is_null() {
                if let Some(name) = iface_name(ifa) {
                    if let Some(entry) = iface_addr_entry(ifa, name, &macs) {
                        entries.push(entry);
                    }
                }
            }
            cur = ifa.ifa_next;
        }

        libc::freeifaddrs(ifap);
        entries
    }
}

#[cfg(not(unix))]
fn collect_network_interfaces() -> Vec<NetInterfaceEntry> {
    Vec::new()
}

#[cfg(unix)]
unsafe fn iface_name(ifa: &libc::ifaddrs) -> Option<String> {
    if ifa.ifa_name.is_null() {
        return None;
    }
    Some(CStr::from_ptr(ifa.ifa_name).to_string_lossy().into_owned())
}

#[cfg(target_os = "linux")]
unsafe fn packet_mac(addr: *const libc::sockaddr) -> Option<String> {
    let ll = &*(addr as *const libc::sockaddr_ll);
    if ll.sll_halen < 6 {
        return None;
    }
    Some(format!(
        "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
        ll.sll_addr[0],
        ll.sll_addr[1],
        ll.sll_addr[2],
        ll.sll_addr[3],
        ll.sll_addr[4],
        ll.sll_addr[5]
    ))
}

#[cfg(all(unix, not(target_os = "linux")))]
unsafe fn packet_mac(_addr: *const libc::sockaddr) -> Option<String> {
    None
}

#[cfg(unix)]
unsafe fn iface_addr_entry(
    ifa: &libc::ifaddrs,
    name: String,
    macs: &HashMap<String, String>,
) -> Option<NetInterfaceEntry> {
    let family = (*ifa.ifa_addr).sa_family as i32;
    let internal = (ifa.ifa_flags & libc::IFF_LOOPBACK as u32) != 0;
    let mac = macs
        .get(&name)
        .cloned()
        .unwrap_or_else(|| "00:00:00:00:00:00".to_string());
    match family {
        libc::AF_INET => {
            let addr = &*(ifa.ifa_addr as *const libc::sockaddr_in);
            let ip = Ipv4Addr::from(u32::from_be(addr.sin_addr.s_addr));
            let mask = if ifa.ifa_netmask.is_null() {
                Ipv4Addr::UNSPECIFIED
            } else {
                let mask = &*(ifa.ifa_netmask as *const libc::sockaddr_in);
                Ipv4Addr::from(u32::from_be(mask.sin_addr.s_addr))
            };
            let prefix = ipv4_prefix(mask);
            Some(NetInterfaceEntry {
                name,
                address: ip.to_string(),
                netmask: mask.to_string(),
                family: "IPv4".to_string(),
                mac,
                internal,
                cidr: format!("{ip}/{prefix}"),
                scopeid: None,
            })
        }
        libc::AF_INET6 => {
            let addr = &*(ifa.ifa_addr as *const libc::sockaddr_in6);
            let ip = Ipv6Addr::from(addr.sin6_addr.s6_addr);
            let mask = if ifa.ifa_netmask.is_null() {
                Ipv6Addr::UNSPECIFIED
            } else {
                let mask = &*(ifa.ifa_netmask as *const libc::sockaddr_in6);
                Ipv6Addr::from(mask.sin6_addr.s6_addr)
            };
            let prefix = ipv6_prefix(mask);
            Some(NetInterfaceEntry {
                name,
                address: ip.to_string(),
                netmask: mask.to_string(),
                family: "IPv6".to_string(),
                mac,
                internal,
                cidr: format!("{ip}/{prefix}"),
                scopeid: Some(addr.sin6_scope_id),
            })
        }
        _ => None,
    }
}

fn ipv4_prefix(mask: Ipv4Addr) -> u32 {
    u32::from(mask).count_ones()
}

fn ipv6_prefix(mask: Ipv6Addr) -> u32 {
    mask.octets().iter().map(|b| b.count_ones()).sum()
}

fn detect_platform() -> &'static str {
    if cfg!(target_os = "linux") {
        "linux"
    } else if cfg!(target_os = "macos") {
        "darwin"
    } else if cfg!(target_os = "windows") {
        "win32"
    } else {
        "unknown"
    }
}

fn detect_arch() -> &'static str {
    if cfg!(target_arch = "x86_64") {
        "x64"
    } else if cfg!(target_arch = "aarch64") {
        "arm64"
    } else if cfg!(target_arch = "arm") {
        "arm"
    } else {
        "unknown"
    }
}

fn detect_os_type() -> &'static str {
    if cfg!(target_os = "linux") {
        "Linux"
    } else if cfg!(target_os = "macos") {
        "Darwin"
    } else if cfg!(target_os = "windows") {
        "Windows_NT"
    } else {
        "Unknown"
    }
}
