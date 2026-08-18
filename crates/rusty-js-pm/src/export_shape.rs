
use rusty_js_ast::{Argument, Expr, MemberProperty, ModuleItem, ObjectKey, ObjectProperty, Stmt};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StaticExportShape {

    pub named: Vec<String>,

    pub module_exports_reassigned: bool,

    pub has_es_module_flag: bool,
}

impl StaticExportShape {

    pub fn is_empty(&self) -> bool {
        self.named.is_empty() && !self.module_exports_reassigned && !self.has_es_module_flag
    }

    fn push_named(&mut self, name: impl Into<String>) {
        let name = name.into();
        if name == "__esModule" {
            self.has_es_module_flag = true;
            return;
        }
        if !self.named.iter().any(|n| n == &name) {
            self.named.push(name);
        }
    }

    pub fn to_json_value(&self) -> rusty_json_manifest::Value {
        let mut object = rusty_json_manifest::Map::new();
        if !self.named.is_empty() {
            object.insert(
                "named".to_string(),
                rusty_json_manifest::Value::Array(
                    self.named
                        .iter()
                        .cloned()
                        .map(rusty_json_manifest::Value::String)
                        .collect(),
                ),
            );
        }
        if self.module_exports_reassigned {
            object.insert(
                "module_exports_reassigned".to_string(),
                rusty_json_manifest::Value::Bool(true),
            );
        }
        if self.has_es_module_flag {
            object.insert(
                "has_es_module_flag".to_string(),
                rusty_json_manifest::Value::Bool(true),
            );
        }
        rusty_json_manifest::Value::Object(object)
    }

    pub fn from_json_value(value: &rusty_json_manifest::Value) -> Self {
        let mut shape = StaticExportShape::default();
        if let Some(items) = value
            .get("named")
            .and_then(rusty_json_manifest::Value::as_array)
        {
            shape.named = items
                .iter()
                .filter_map(rusty_json_manifest::Value::as_str)
                .map(String::from)
                .collect();
        }
        shape.module_exports_reassigned = value
            .get("module_exports_reassigned")
            .and_then(rusty_json_manifest::Value::as_bool)
            .unwrap_or(false);
        shape.has_es_module_flag = value
            .get("has_es_module_flag")
            .and_then(rusty_json_manifest::Value::as_bool)
            .unwrap_or(false);
        shape
    }

    pub fn lower_node_keys(&self) -> Vec<String> {
        let mut keys: Vec<String> = Vec::with_capacity(self.named.len() + 3);
        let push_unique = |keys: &mut Vec<String>, k: String| {
            if !keys.iter().any(|e| e == &k) {
                keys.push(k);
            }
        };

        for n in &self.named {
            push_unique(&mut keys, n.clone());
        }
        push_unique(&mut keys, "default".to_string());
        push_unique(&mut keys, "module.exports".to_string());
        if self.has_es_module_flag {
            push_unique(&mut keys, "__esModule".to_string());
        }
        keys
    }
}

pub fn compile_export_shape(source: &str) -> StaticExportShape {

    let (shape, _reexports) = extract_with_reexports(source);
    shape
}

pub fn compile_export_shape_at(entry: &std::path::Path) -> StaticExportShape {
    let mut seen = std::collections::HashSet::new();
    compile_at(entry, 0, &mut seen)
}

fn compile_at(
    entry: &std::path::Path,
    depth: usize,
    seen: &mut std::collections::HashSet<std::path::PathBuf>,
) -> StaticExportShape {
    if depth > 8 {
        return StaticExportShape::default();
    }
    let canon = entry.canonicalize().unwrap_or_else(|_| entry.to_path_buf());
    if !seen.insert(canon) {
        return StaticExportShape::default();
    }
    let src = match std::fs::read_to_string(entry) {
        Ok(s) => s,
        Err(_) => return StaticExportShape::default(),
    };
    let (mut shape, reexports) = extract_with_reexports(&src);
    let dir = entry.parent().unwrap_or_else(|| std::path::Path::new("."));
    for spec in reexports {
        if let Some(child) = resolve_relative(dir, &spec) {
            let sub = compile_at(&child, depth + 1, seen);
            for n in sub.named {
                shape.push_named(n);
            }

        }
    }
    shape
}

fn extract_with_reexports(source: &str) -> (StaticExportShape, Vec<String>) {
    let module = match rusty_js_parser::parse_script(source) {
        Ok(m) => m,
        Err(_) => return (StaticExportShape::default(), Vec::new()),
    };
    let mut shape = StaticExportShape::default();
    let mut reexports: Vec<String> = Vec::new();

    let mut rebound = false;
    for item in &module.body {
        if let ModuleItem::Statement(stmt) = item {
            walk_stmt_for_exports(stmt, &mut shape, &mut reexports, &mut rebound);
        }
    }
    (shape, reexports)
}

fn walk_stmt_for_exports(
    stmt: &Stmt,
    shape: &mut StaticExportShape,
    reex: &mut Vec<String>,
    rebound: &mut bool,
) {
    match stmt {
        Stmt::Expression { expr, .. } => visit_expr_inner(expr, shape, reex, rebound),
        Stmt::Variable(vs) => {
            for d in &vs.declarators {
                if let Some(init) = &d.init {
                    visit_expr_inner(init, shape, reex, rebound);
                }
            }
        }

        Stmt::If {
            consequent,
            alternate,
            ..
        } => {
            walk_stmt_for_exports(consequent, shape, reex, rebound);
            if let Some(alt) = alternate {
                walk_stmt_for_exports(alt, shape, reex, rebound);
            }
        }
        Stmt::Block { body, .. } => {
            for s in body {
                walk_stmt_for_exports(s, shape, reex, rebound);
            }
        }
        _ => {}
    }
}

fn require_spec(e: &Expr) -> Option<String> {
    if let Expr::Call {
        callee, arguments, ..
    } = unwrap_paren(e)
    {
        if is_ident(callee, "require") {
            if let Some(Argument::Expr(Expr::StringLiteral { value, .. })) = arguments.first() {
                if value.starts_with("./") || value.starts_with("../") {
                    return Some(value.clone());
                }
            }
        }
    }
    None
}

fn resolve_relative(dir: &std::path::Path, spec: &str) -> Option<std::path::PathBuf> {
    let base = dir.join(spec);

    let appended = |ext: &str| -> std::path::PathBuf {
        let mut s = base.clone().into_os_string();
        s.push(ext);
        std::path::PathBuf::from(s)
    };
    let candidates = [
        base.clone(),
        appended(".js"),
        appended(".json"),
        base.join("index.js"),
    ];
    for c in &candidates {
        if c.is_file() {
            return Some(c.clone());
        }
    }

    let pj = base.join("package.json");
    if pj.is_file() {
        if let Ok(body) = std::fs::read_to_string(&pj) {
            if let Ok(m) = rusty_json_manifest::from_str::<rusty_json_manifest::Value>(&body) {
                if let Some(rusty_json_manifest::Value::String(main)) = m.get("main") {
                    let p = base.join(main.trim_start_matches("./"));
                    if p.is_file() {
                        return Some(p);
                    }
                }
            }
        }
    }
    None
}

fn visit_expr_for_exports(expr: &Expr, shape: &mut StaticExportShape, reex: &mut Vec<String>) {
    let mut rebound = false;
    visit_expr_inner(expr, shape, reex, &mut rebound);
}

fn visit_expr_inner(
    expr: &Expr,
    shape: &mut StaticExportShape,
    reex: &mut Vec<String>,
    rebound: &mut bool,
) {
    match expr {
        Expr::Assign { target, value, .. } => {

            if matches!(unwrap_paren(target), Expr::Identifier { name, .. } if name == "exports") {
                *rebound = true;
            }
            match classify_member_target(target) {

                Some(TargetKind::ExportsBare(name)) => shape.push_named(name),

                Some(TargetKind::ExportsModule(name)) => shape.push_named(name),
                Some(TargetKind::ModuleExports) => {

                    shape.module_exports_reassigned = true;

                    if let Some(spec) = require_spec(value) {
                        reex.push(spec);
                    }
                    if let Expr::Object { properties, .. } = unwrap_paren(value) {

                        for p in properties {
                            match p {
                                ObjectProperty::Spread { expr, .. } => {
                                    if let Some(spec) = require_spec(expr) {
                                        reex.push(spec);
                                    }
                                }
                                ObjectProperty::Property { key, value: v, .. } => {
                                    match unwrap_paren(v) {
                                        Expr::Identifier { .. }
                                        | Expr::BoolLiteral { .. }
                                        | Expr::NullLiteral { .. } => {
                                            if let Some(name) = object_key_name(key) {
                                                shape.push_named(name);
                                            }
                                        }
                                        Expr::Member { .. }
                                        | Expr::Call { .. }
                                        | Expr::New { .. }
                                        | Expr::Function { .. }
                                        | Expr::Class { .. }
                                        | Expr::Binary { .. }
                                        | Expr::Conditional { .. }
                                        | Expr::Assign { .. }
                                        | Expr::Sequence { .. } => {
                                            if let Some(name) = object_key_name(key) {
                                                shape.push_named(name);
                                            }
                                            break;
                                        }
                                        _ => break,
                                    }
                                }
                            }
                        }
                    }
                }
                None => {}
            }

            visit_expr_inner(value, shape, reex, rebound);
        }
        Expr::Call {
            callee, arguments, ..
        } => {
            if is_object_define_property(callee) {
                if let (Some(Argument::Expr(obj)), Some(Argument::Expr(key))) =
                    (arguments.first(), arguments.get(1))
                {

                    if is_ident(obj, "exports") && !*rebound {
                        if let Expr::StringLiteral { value, .. } = key {
                            shape.push_named(value.clone());
                        }
                    }
                }
            }

            if is_object_assign(callee) || is_export_star(callee) {
                for arg in arguments {
                    if let Argument::Expr(e) = arg {
                        if let Some(spec) = require_spec(e) {
                            reex.push(spec);
                        }
                    }
                }
            }

            if let Some(body) = umd_factory_body(callee) {
                walk_body_for_exports(body, shape, reex);
            }

            if let Some(body) = wrapper_iife_body(callee) {
                walk_body_for_exports(body, shape, reex);
            }
            for arg in arguments {
                if let Argument::Expr(e) = arg {
                    if let Some(body) = umd_factory_body(e) {
                        walk_body_for_exports(body, shape, reex);
                    } else if !matches!(unwrap_paren(e), Expr::Function { .. } | Expr::Arrow { .. })
                    {

                        visit_expr_inner(e, shape, reex, rebound);
                    }
                }
            }
        }

        Expr::Binary { left, right, .. } => {
            visit_expr_inner(left, shape, reex, rebound);
            visit_expr_inner(right, shape, reex, rebound);
        }
        Expr::Conditional {
            consequent,
            alternate,
            ..
        } => {
            visit_expr_inner(consequent, shape, reex, rebound);
            visit_expr_inner(alternate, shape, reex, rebound);
        }
        Expr::Parenthesized { expr, .. } => visit_expr_inner(expr, shape, reex, rebound),
        Expr::Sequence { expressions, .. } => {
            for e in expressions {
                visit_expr_inner(e, shape, reex, rebound);
            }
        }
        _ => {}
    }
}

fn is_object_assign(callee: &Expr) -> bool {
    if let Expr::Member {
        object, property, ..
    } = unwrap_paren(callee)
    {
        if let Some(prop) = member_prop_name(property) {
            return is_ident(object, "Object") && prop == "assign";
        }
    }
    false
}

fn is_export_star(callee: &Expr) -> bool {
    match unwrap_paren(callee) {
        Expr::Identifier { name, .. } => {
            name == "__export" || name == "__exportStar" || name == "__createBinding"
        }
        Expr::Member { property, .. } => {
            matches!(
                member_prop_name(property).as_deref(),
                Some("__export") | Some("__exportStar") | Some("__createBinding")
            )
        }
        _ => false,
    }
}

enum TargetKind {

    ExportsBare(String),

    ExportsModule(String),

    ModuleExports,
}

fn classify_member_target(target: &Expr) -> Option<TargetKind> {
    if let Expr::Member {
        object, property, ..
    } = unwrap_paren(target)
    {
        let prop = member_prop_name(property)?;

        if is_ident(object, "exports") {
            return Some(TargetKind::ExportsBare(prop));
        }

        if is_ident(object, "module") && prop == "exports" {
            return Some(TargetKind::ModuleExports);
        }

        if is_module_exports(object) {
            return Some(TargetKind::ExportsModule(prop));
        }
    }
    None
}

fn umd_factory_body(e: &Expr) -> Option<&[Stmt]> {
    if let Expr::Function { params, body, .. } = unwrap_paren(e) {
        let binds_exports = params.iter().any(param_binds_exports);
        if binds_exports {
            return Some(body);
        }
    }
    None
}

fn wrapper_iife_body(callee: &Expr) -> Option<&[Stmt]> {
    match unwrap_paren(callee) {
        Expr::Function { params, body, .. } if params.is_empty() => Some(body),
        Expr::Member {
            object, property, ..
        } if matches!(
            member_prop_name(property).as_deref(),
            Some("call") | Some("apply")
        ) =>
        {
            match unwrap_paren(object) {
                Expr::Function { params, body, .. } if params.is_empty() => Some(body),
                _ => None,
            }
        }
        _ => None,
    }
}

fn param_binds_exports(p: &rusty_js_ast::Parameter) -> bool {

    matches!(&p.target, rusty_js_ast::BindingPattern::Identifier(id) if id.name == "exports")
}

fn walk_body_for_exports(body: &[Stmt], shape: &mut StaticExportShape, reex: &mut Vec<String>) {

    let mut rebound = false;
    for stmt in body {
        walk_stmt_for_exports(stmt, shape, reex, &mut rebound);
    }
}

fn is_module_exports(e: &Expr) -> bool {
    if let Expr::Member {
        object, property, ..
    } = unwrap_paren(e)
    {
        if let Some(prop) = member_prop_name(property) {
            return is_ident(object, "module") && prop == "exports";
        }
    }
    false
}

fn member_prop_name(p: &MemberProperty) -> Option<String> {
    match p {
        MemberProperty::Identifier { name, .. } => Some(name.clone()),
        MemberProperty::Computed { expr, .. } => match expr {
            Expr::StringLiteral { value, .. } => Some(value.clone()),
            _ => None,
        },
        MemberProperty::Private { .. } => None,
    }
}

fn object_key_name(key: &ObjectKey) -> Option<String> {
    match key {
        ObjectKey::Identifier { name, .. } => Some(name.clone()),
        ObjectKey::String { value, .. } => Some(value.clone()),
        _ => None,
    }
}

fn is_object_define_property(callee: &Expr) -> bool {
    if let Expr::Member {
        object, property, ..
    } = unwrap_paren(callee)
    {
        if let Some(prop) = member_prop_name(property) {
            return is_ident(object, "Object") && prop == "defineProperty";
        }
    }
    false
}

fn is_ident(e: &Expr, name: &str) -> bool {
    matches!(unwrap_paren(e), Expr::Identifier { name: n, .. } if n == name)
}

fn unwrap_paren(e: &Expr) -> &Expr {
    let mut cur = e;
    while let Expr::Parenthesized { expr, .. } = cur {
        cur = expr;
    }
    cur
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exports_named_assignments() {
        let s = compile_export_shape("exports.Foo = function(){}; exports.BAR = 5;");
        assert_eq!(s.named, vec!["Foo", "BAR"]);
        assert!(!s.module_exports_reassigned);
    }

    #[test]
    fn ajv_ts_idiom_then_reassignment() {
        let s = compile_export_shape(
            "exports.KeywordCxt = exports.Ajv = void 0; exports.Ajv = Ajv; module.exports = Ajv;",
        );
        assert!(s.named.contains(&"Ajv".to_string()));
        assert!(s.named.contains(&"KeywordCxt".to_string()));
        assert!(s.module_exports_reassigned);
    }

    #[test]
    fn module_exports_object_literal_is_opaque() {

        let s = compile_export_shape("module.exports = { a: 1, b: function(){} };");
        assert!(s.named.is_empty());
        assert!(s.module_exports_reassigned);
        let mut keys = s.lower_node_keys();
        keys.sort();
        assert_eq!(
            keys,
            vec!["default".to_string(), "module.exports".to_string()]
        );
    }

    #[test]
    fn ajv_names_survive_reassignment() {

        let s = compile_export_shape(
            "exports.KeywordCxt = exports.Ajv = void 0; exports.Ajv = Ajv; \
             exports.KeywordCxt = KeywordCxt; module.exports = Ajv;",
        );
        assert!(s.module_exports_reassigned);
        let keys = s.lower_node_keys();
        assert!(keys.contains(&"Ajv".to_string()));
        assert!(keys.contains(&"KeywordCxt".to_string()));
        assert!(keys.contains(&"default".to_string()));
        assert!(keys.contains(&"module.exports".to_string()));
    }

    #[test]
    fn es_module_flag_and_define_property() {
        let s = compile_export_shape(
            "Object.defineProperty(exports, \"__esModule\", { value: true }); \
             Object.defineProperty(exports, \"Foo\", { get: function(){} });",
        );
        assert!(s.has_es_module_flag);
        assert_eq!(s.named, vec!["Foo"]);
        assert!(s.lower_node_keys().contains(&"__esModule".to_string()));
    }

    #[test]
    fn module_exports_member_assignment() {

        let s = compile_export_shape(
            "module.exports = createColors(); module.exports.createColors = createColors;",
        );
        assert!(s.module_exports_reassigned);
        assert_eq!(s.named, vec!["createColors"]);
        let keys = s.lower_node_keys();
        assert!(keys.contains(&"createColors".to_string()));
        assert!(keys.contains(&"default".to_string()));
        assert!(keys.contains(&"module.exports".to_string()));
    }

    #[test]
    fn object_literal_shorthand_exports() {

        let s = compile_export_shape(
            "module.exports = { SemVer, parse, valid: valid, opaque: 1, fn: function(){} };",
        );
        assert!(s.module_exports_reassigned);
        assert!(s.named.contains(&"SemVer".to_string()));
        assert!(s.named.contains(&"parse".to_string()));
        assert!(s.named.contains(&"valid".to_string()));
        assert!(!s.named.contains(&"opaque".to_string()));
        assert!(!s.named.contains(&"fn".to_string()));
    }

    #[test]
    fn object_literal_records_first_complex_value_then_halts() {

        let jwt = compile_export_shape(
            "module.exports = { decode: require('./decode'), verify: require('./verify') };",
        );
        assert!(jwt.named.contains(&"decode".to_string()));
        assert!(!jwt.named.contains(&"verify".to_string()));

        let sv = compile_export_shape(
            "module.exports = { parse, valid, re: internalRe.re, src: internalRe.src };",
        );
        assert!(sv.named.contains(&"parse".to_string()));
        assert!(sv.named.contains(&"valid".to_string()));
        assert!(sv.named.contains(&"re".to_string()));
        assert!(!sv.named.contains(&"src".to_string()));

        let toml = compile_export_shape(
            "module.exports = { parse: function(i){ return i; }, stringify: function(x){ return x; } };",
        );
        assert!(toml.named.contains(&"parse".to_string()));
        assert!(!toml.named.contains(&"stringify".to_string()));

        let bools = compile_export_shape("module.exports = { a: true, b: null, c };");
        assert!(bools.named.contains(&"a".to_string()));
        assert!(bools.named.contains(&"b".to_string()));
        assert!(bools.named.contains(&"c".to_string()));

        let arr = compile_export_shape("module.exports = { a: [1], b };");
        assert!(!arr.named.contains(&"a".to_string()));
        assert!(!arr.named.contains(&"b".to_string()));
    }

    #[test]
    fn export_star_helper_and_ts_enum() {

        let (_s, reex) = extract_with_reexports("__export(require('./yamlAST'));");
        assert!(reex.contains(&"./yamlAST".to_string()));

        let s = compile_export_shape(
            "var Kind;\n(function (Kind) { Kind[Kind[\"A\"] = 0] = \"A\"; })(Kind = exports.Kind || (exports.Kind = {}));",
        );
        assert!(s.named.contains(&"Kind".to_string()));
    }

    #[test]
    fn wrapper_iife_call_this_descent() {

        let s = compile_export_shape(
            "(function() { var exports; exports = module.exports = require('./lib/node_cache'); exports.version = '5.1.2'; }).call(this);",
        );
        assert!(s.module_exports_reassigned);
        assert!(s.named.contains(&"version".to_string()));

        let s2 = compile_export_shape("(function(){ exports.foo = 1; exports.bar = baz; })();");
        assert!(s2.named.contains(&"bar".to_string()));
    }

    #[test]
    fn resolve_relative_dotted_filename() {

        let dir = std::env::temp_dir().join(format!("cruft_rr_{}_{}", std::process::id(), "dot"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("exclude.decorator.js"), "exports.Exclude = 1;").unwrap();
        let r = resolve_relative(&dir, "./exclude.decorator");
        assert!(r.is_some(), "dotted-filename reexport must resolve");
        assert!(r.unwrap().ends_with("exclude.decorator.js"));

        std::fs::write(dir.join("plain.js"), "exports.X = 1;").unwrap();
        assert!(resolve_relative(&dir, "./plain").is_some());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn umd_factory_descent() {

        let s = compile_export_shape(
            "(function (factory) { factory(require, exports); })(function (require, exports) { \
             Object.defineProperty(exports, \"__esModule\", { value: true }); \
             exports.parse = parse; exports.visit = visit; });",
        );
        assert!(s.has_es_module_flag);
        assert!(s.named.contains(&"parse".to_string()));
        assert!(s.named.contains(&"visit".to_string()));
    }

    #[test]
    fn bare_function_reassignment_fixed_keys_only() {
        let s = compile_export_shape("module.exports = function greet(){ return 1; };");
        assert!(s.named.is_empty());
        assert!(s.module_exports_reassigned);
        let mut keys = s.lower_node_keys();
        keys.sort();
        assert_eq!(
            keys,
            vec!["default".to_string(), "module.exports".to_string()]
        );
    }

    #[test]
    fn transitive_barrel_spread_and_export_star() {

        let dir = std::env::temp_dir().join("cjser_transitive_test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.js"), "exports.alpha = 1; exports.aTwo = 2;").unwrap();
        std::fs::write(dir.join("b.js"), "exports.beta = 3;").unwrap();
        std::fs::write(dir.join("c.js"), "exports.gamma = 4;").unwrap();
        std::fs::write(
            dir.join("index.js"),
            "module.exports = { ...require('./a'), ...require('./b'), top: 1 }; \
             __exportStar(require('./c'), exports);",
        )
        .unwrap();
        let shape = compile_export_shape_at(&dir.join("index.js"));
        for n in ["alpha", "aTwo", "beta", "gamma"] {
            assert!(shape.named.contains(&n.to_string()), "missing {n}");
        }
        assert!(!shape.named.contains(&"top".to_string()));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn bare_exports_rebound_stops_surfacing() {

        let s = compile_export_shape(
            "exports = module.exports = bodyParser; \
             Object.defineProperty(exports, 'json', { get: function(){} }); \
             Object.defineProperty(exports, 'raw', { get: function(){} });",
        );
        assert!(s.named.is_empty(), "got {:?}", s.named);
        assert!(s.module_exports_reassigned);
    }

    #[test]
    fn direct_exports_assign_surfaces_even_after_rebind() {

        let s = compile_export_shape(
            "exports.CodeGen = void 0; exports.Ajv = void 0; \
             module.exports = exports = createApplication; exports.application = 1;",
        );
        assert!(s.named.contains(&"CodeGen".to_string()));
        assert!(s.named.contains(&"Ajv".to_string()));
        assert!(s.named.contains(&"application".to_string()));
    }

    #[test]
    fn var_declarator_export_assignment() {

        let s = compile_export_shape(
            "var Validator = module.exports.Validator = require('./validator'); \
             module.exports.scan = require('./scan').scan;",
        );
        assert!(s.named.contains(&"Validator".to_string()));
        assert!(s.named.contains(&"scan".to_string()));
    }

    #[test]
    fn getter_define_property_gated_after_rebind() {

        let gated = compile_export_shape(
            "exports = module.exports = fn; \
             Object.defineProperty(exports, 'json', { get: function(){} });",
        );
        assert!(gated.named.is_empty());
        let babel = compile_export_shape(
            "Object.defineProperty(exports, 'X', { enumerable: true, get: function(){} });",
        );
        assert!(babel.named.contains(&"X".to_string()));
    }

    #[test]
    fn json_adapter_skips_empty() {
        let s = StaticExportShape::default();
        assert!(s.is_empty());
        assert_eq!(s.to_json_value().to_compact_string(), "{}");
    }
}
