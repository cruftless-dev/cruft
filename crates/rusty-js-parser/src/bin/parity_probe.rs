
use rusty_js_parser::parse_module;
use std::path::{Path, PathBuf};

fn resolve_entry(pkg_root: &Path) -> Option<PathBuf> {
    let pkg_json_path = pkg_root.join("package.json");
    let raw = std::fs::read_to_string(&pkg_json_path).ok()?;
    for key in &["\"module\"", "\"main\""] {
        if let Some(idx) = raw.find(key) {
            let after = &raw[idx + key.len()..];
            if let Some(qstart) = after.find('"') {
                let after2 = &after[qstart + 1..];
                if let Some(qend) = after2.find('"') {
                    let rel = &after2[..qend];
                    let p = pkg_root.join(rel);
                    if p.is_file() {
                        return Some(p);
                    }
                }
            }
        }
    }
    for fname in &["index.js", "index.mjs", "index.cjs"] {
        let p = pkg_root.join(fname);
        if p.is_file() {
            return Some(p);
        }
    }
    None
}

fn main() {
    let list_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "/home/jaredef/rusty-bun/host/tools/parity-top100.txt".to_string());
    let raw = std::fs::read_to_string(&list_path).expect("list.txt read failed");

    let mut total = 0usize;
    let mut parsed = 0usize;
    let mut skip_no_source = 0usize;
    let mut skip_too_large = 0usize;
    let mut failures: Vec<(String, String, usize)> = Vec::new();

    for line in raw.lines() {
        let pkg = line.trim();
        if pkg.is_empty() || pkg.starts_with('#') {
            continue;
        }
        total += 1;
        let safe = pkg.replace('/', "--");
        let sandbox = format!("/tmp/parity-sandbox/{}/node_modules/{}", safe, pkg);
        let pkg_root = Path::new(&sandbox);
        if !pkg_root.is_dir() {
            skip_no_source += 1;
            continue;
        }
        let Some(entry) = resolve_entry(pkg_root) else {
            skip_no_source += 1;
            continue;
        };
        let Ok(src) = std::fs::read_to_string(&entry) else {
            skip_no_source += 1;
            continue;
        };

        if src.len() > 500_000 {
            skip_too_large += 1;
            continue;
        }

        match parse_module(&src) {
            Ok(_) => parsed += 1,
            Err(e) => failures.push((pkg.to_string(), e.message, e.span.start)),
        }
    }

    let measurable = total - skip_no_source - skip_too_large;
    let pct = if measurable > 0 {
        (parsed as f64) / (measurable as f64) * 100.0
    } else {
        0.0
    };

    println!();
    println!("═══════════════════════════════════════════════");
    println!("rusty-js-parser parity-corpus acceptance probe");
    println!("═══════════════════════════════════════════════");
    println!("Total packages:        {}", total);
    println!("Skipped (no source):   {}", skip_no_source);
    println!("Skipped (>500KB):      {}", skip_too_large);
    println!("Measured:              {}", measurable);
    println!("Parsed OK:             {}", parsed);
    println!("Parse failures:        {}", failures.len());
    println!("Acceptance:            {:.1}%", pct);
    if !failures.is_empty() {
        println!();
        println!("First 25 failures:");
        for (pkg, msg, off) in failures.iter().take(25) {
            println!("  {}  @{:6}  {}", pkg, off, msg);
        }
    }
}
