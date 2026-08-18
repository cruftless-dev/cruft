
use rusty_js_pm::export_shape::compile_export_shape_at;

fn string_array_json(items: &[String]) -> String {
    rusty_json_manifest::Value::Array(
        items
            .iter()
            .cloned()
            .map(rusty_json_manifest::Value::String)
            .collect(),
    )
    .to_compact_string()
}

fn main() {
    let path = match std::env::args().nth(1) {
        Some(p) => p,
        None => {
            eprintln!("usage: export_shape_probe <entry.js>");
            std::process::exit(2);
        }
    };
    let shape = compile_export_shape_at(std::path::Path::new(&path));
    let mut keys = shape.lower_node_keys();
    keys.sort();
    let named: Vec<String> = {
        let mut n = shape.named.clone();
        n.sort();
        n
    };
    println!(
        "{{\"named\":{},\"reassigned\":{},\"esmodule\":{},\"node_keys\":{}}}",
        string_array_json(&named),
        shape.module_exports_reassigned,
        shape.has_es_module_flag,
        string_array_json(&keys)
    );
}
