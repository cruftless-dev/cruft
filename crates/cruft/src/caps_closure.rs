
use rusty_js_pm::lockfile::{Lockfile, LEGACY_LOCKFILE_NAME, LOCKFILE_NAME};
use rusty_js_runtime::caps::CapMode;
use rusty_js_runtime::Runtime;
use std::path::{Path, PathBuf};

const NODE_BUILTINS: &[&str] = &[
    "assert",
    "buffer",
    "child_process",
    "cluster",
    "console",
    "constants",
    "crypto",
    "dgram",
    "diagnostics_channel",
    "dns",
    "domain",
    "events",
    "fs",
    "http",
    "http2",
    "https",
    "inspector",
    "module",
    "net",
    "os",
    "path",
    "perf_hooks",
    "process",
    "punycode",
    "querystring",
    "readline",
    "repl",
    "stream",
    "string_decoder",
    "sys",
    "timers",
    "tls",
    "trace_events",
    "tty",
    "url",
    "util",
    "v8",
    "vm",
    "wasi",
    "worker_threads",
    "zlib",
];

const MODULE_EXTS: &[&str] = &["js", "mjs", "cjs", "json", "node"];

fn is_module_file(name: &str) -> bool {
    name.rsplit_once('.')
        .map(|(_, ext)| MODULE_EXTS.contains(&ext))
        .unwrap_or(false)
}

fn find_lockfile(start_dir: &Path) -> Option<(PathBuf, PathBuf)> {
    let mut dir = Some(start_dir);
    while let Some(d) = dir {
        for name in [LOCKFILE_NAME, LEGACY_LOCKFILE_NAME] {
            let candidate = d.join(name);
            if candidate.is_file() {
                return Some((candidate, d.to_path_buf()));
            }
        }
        dir = d.parent();
    }
    None
}

fn placement_base(project_root: &Path, place: &str, name: &str) -> PathBuf {
    let mut base = project_root.join("node_modules");
    if !place.is_empty() {
        for parent in place.split('/') {
            base = base.join(parent).join("node_modules");
        }
    }

    base.join(name)
}

fn collect_module_files(base: &Path) -> Vec<Vec<String>> {
    let mut out = Vec::new();
    let mut stack = vec![base.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
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
            } else if is_module_file(&fname) {
                if let Ok(rel) = path.strip_prefix(base) {
                    out.push(
                        rel.components()
                            .map(|c| c.as_os_str().to_string_lossy().into_owned())
                            .collect(),
                    );
                }
            }
        }
    }
    out
}

fn strip_ext(name: &str) -> Option<&str> {
    name.rsplit_once('.').and_then(|(stem, ext)| {
        if MODULE_EXTS.contains(&ext) {
            Some(stem)
        } else {
            None
        }
    })
}

fn relative_specifiers(from: &[String], to: &[String], out: &mut Vec<String>) {
    let from_dir = &from[..from.len().saturating_sub(1)];

    let to_dir = &to[..to.len().saturating_sub(1)];
    let mut common = 0;
    while common < from_dir.len() && common < to_dir.len() && from_dir[common] == to_dir[common] {
        common += 1;
    }
    let ups = from_dir.len() - common;
    let mut prefix = String::new();
    if ups == 0 {
        prefix.push_str("./");
    } else {
        for _ in 0..ups {
            prefix.push_str("../");
        }
    }

    let down: Vec<String> = to_dir[common..].to_vec();
    let mut dir_part = prefix.clone();
    for seg in &down {
        dir_part.push_str(seg);
        dir_part.push('/');
    }
    let file = &to[to.len() - 1];

    out.push(format!("{dir_part}{file}"));

    if let Some(stem) = strip_ext(file) {
        out.push(format!("{dir_part}{stem}"));

        if stem == "index" {
            let mut d = dir_part.clone();

            if d.ends_with('/') && d.len() > 2 {
                d.pop();
            }
            out.push(d.clone());

            if dir_part == "./" {
                out.push(".".to_string());
            }
        }
    }
}

fn file_urls(base: &Path, rel: &[String]) -> Vec<String> {
    let mut joined = base.to_path_buf();
    for seg in rel {
        joined = joined.join(seg);
    }
    let mut urls = vec![format!("file://{}", joined.to_string_lossy())];
    if let Ok(canon) = std::fs::canonicalize(&joined) {
        let c = format!("file://{}", canon.to_string_lossy());
        if !urls.contains(&c) {
            urls.push(c);
        }
    }
    urls
}

pub fn wire_import_closure_from_lockfile(
    rt: &mut Runtime,
    entry_url: &str,
    cap_mode: CapMode,
) -> Option<String> {
    let dbg = std::env::var_os("CRUFT_CAPS_VERBOSE").is_some();
    if !matches!(cap_mode, CapMode::Sealed | CapMode::SealedDeps) {
        if dbg {
            eprintln!("cruft: caps-closure skipped (mode not sealed): {cap_mode:?}");
        }
        return None;
    }
    let entry_path = entry_url.strip_prefix("file://").unwrap_or(entry_url);
    let start_dir = Path::new(entry_path).parent()?;
    if dbg {
        eprintln!(
            "cruft: caps-closure entry_path={entry_path} start_dir={}",
            start_dir.display()
        );
    }
    let (lock_path, project_root) = match find_lockfile(start_dir) {
        Some(x) => x,
        None => {
            if dbg {
                eprintln!(
                    "cruft: caps-closure no lockfile found from {}",
                    start_dir.display()
                );
            }
            return None;
        }
    };
    let lock = Lockfile::read_from(&lock_path).ok()?;

    let mut modules_registered = 0usize;
    let mut packages_registered = 0usize;
    let mut integrity_registered = 0usize;

    for pkg in lock.packages.values() {

        let mut bare: Vec<String> = Vec::new();
        bare.extend(pkg.dep.dependencies.keys().cloned());
        bare.extend(pkg.dep.peer_dependencies.keys().cloned());

        bare.extend(pkg.module_map.keys().cloned());
        bare.extend(NODE_BUILTINS.iter().map(|s| s.to_string()));

        let places = if pkg.placements.is_empty() {
            vec![String::new()]
        } else {
            pkg.placements.clone()
        };
        let mut wired_this_pkg = false;
        for place in &places {
            let base = placement_base(&project_root, place, &pkg.dep.name);
            if !base.is_dir() {
                continue;
            }
            let files = collect_module_files(&base);
            if files.is_empty() {
                continue;
            }
            for from in &files {
                let mut allowed = bare.clone();
                for to in &files {
                    relative_specifiers(from, to, &mut allowed);
                }
                allowed.sort();
                allowed.dedup();

                let rel_key = from.join("/");
                let sri = pkg.module_integrity.get(&rel_key);
                for url in file_urls(&base, from) {
                    rt.register_import_closure(&url, allowed.clone());
                    modules_registered += 1;
                    if let Some(sri) = sri {
                        rt.register_module_integrity(&url, sri);
                        integrity_registered += 1;
                    }
                }
                wired_this_pkg = true;
            }
        }
        if wired_this_pkg {
            packages_registered += 1;
        }
    }

    Some(format!(
        "sealed import-closure wired from {}: {} packages, {} module URLs, {} integrity SRIs",
        lock_path.display(),
        packages_registered,
        modules_registered,
        integrity_registered
    ))
}
