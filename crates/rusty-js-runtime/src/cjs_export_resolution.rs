
use rusty_js_ast::{
    Argument, ArrowBody, DefaultExportBody, ExportDeclaration, Expr, ForBinding, ForInit,
    MemberProperty, Module, ModuleItem, ObjectKey, ObjectProperty, Stmt, VariableStatement,
};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StaticExportShape {

    pub named: Vec<String>,

    pub module_exports_reassigned: bool,

    pub has_es_module_flag: bool,

    pub module_exports_require_reexport: bool,

    pub module_exports_direct_require_reexport: bool,

    pub module_exports_require_specs: Vec<String>,

    pub cjs_star_reexport_specs: Vec<String>,

    pub cjs_object_keys_reexport_specs: Vec<String>,
}

impl StaticExportShape {
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

    fn push_require_reexport_spec(&mut self, spec: impl Into<String>) {
        let spec = spec.into();
        self.module_exports_require_reexport = true;
        if !self
            .module_exports_require_specs
            .iter()
            .any(|existing| existing == &spec)
        {
            self.module_exports_require_specs.push(spec);
        }
    }

    fn push_star_reexport_spec(&mut self, spec: impl Into<String>) {
        let spec = spec.into();
        if !self
            .cjs_star_reexport_specs
            .iter()
            .any(|existing| existing == &spec)
        {
            self.cjs_star_reexport_specs.push(spec);
        }
    }

    fn push_object_keys_reexport_spec(&mut self, spec: impl Into<String>) {
        let spec = spec.into();
        if !self
            .cjs_object_keys_reexport_specs
            .iter()
            .any(|existing| existing == &spec)
        {
            self.cjs_object_keys_reexport_specs.push(spec);
        }
    }

    fn clear_reexports(&mut self) {

        self.cjs_star_reexport_specs.clear();
        self.cjs_object_keys_reexport_specs.clear();

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

pub fn extract_static_export_shape_from_cjs_wrapper(ast: &Module) -> Option<StaticExportShape> {
    let Some(ModuleItem::Export(ExportDeclaration::Default {
        body: DefaultExportBody::Expression { expr },
        ..
    })) = ast.body.first()
    else {
        return None;
    };
    let Expr::Function { params, body, .. } = unwrap_paren(&expr) else {
        return None;
    };
    let expected = ["exports", "module", "require", "__filename", "__dirname"];
    if params.len() != expected.len()
        || !params.iter().zip(expected.iter()).all(|(p, expected)| {
            matches!(&p.target,
                rusty_js_ast::BindingPattern::Identifier(id) if id.name == *expected)
        })
    {
        return None;
    }
    Some(extract_static_export_shape(body))
}

pub fn extract_static_export_shape_from_cjs_source(source: &str) -> StaticExportShape {
    let mut shape = StaticExportShape::default();
    scan_static_export_names(source, &mut shape);
    shape
}

pub fn merge_static_export_shapes(
    mut a: StaticExportShape,
    b: StaticExportShape,
) -> StaticExportShape {
    for name in b.named {
        a.push_named(name);
    }
    a.module_exports_reassigned |= b.module_exports_reassigned;
    a.has_es_module_flag |= b.has_es_module_flag;
    a.module_exports_require_reexport |= b.module_exports_require_reexport;
    a.module_exports_direct_require_reexport |= b.module_exports_direct_require_reexport;
    for spec in b.module_exports_require_specs {
        if !a.module_exports_require_specs.iter().any(|s| s == &spec) {
            a.module_exports_require_specs.push(spec);
        }
    }
    for spec in b.cjs_star_reexport_specs {
        if !a.cjs_star_reexport_specs.iter().any(|s| s == &spec) {
            a.cjs_star_reexport_specs.push(spec);
        }
    }
    for spec in b.cjs_object_keys_reexport_specs {
        if !a.cjs_object_keys_reexport_specs.iter().any(|s| s == &spec) {
            a.cjs_object_keys_reexport_specs.push(spec);
        }
    }
    a
}

pub fn extract_static_export_shape(body: &[Stmt]) -> StaticExportShape {
    let mut shape = StaticExportShape::default();
    for stmt in body {
        visit_stmt_for_exports(stmt, &mut shape, true);
    }
    shape
}

fn visit_stmt_for_exports(stmt: &Stmt, shape: &mut StaticExportShape, top: bool) {
    match stmt {
        Stmt::Variable(vs) => visit_var_for_exports(vs, shape),
        Stmt::Expression { expr, .. } => visit_expr_for_exports(expr, shape, top),
        Stmt::Block { body, .. } | Stmt::FunctionDecl { body, .. } => {
            for s in body {
                visit_stmt_for_exports(s, shape, false);
            }
        }
        Stmt::ClassDecl { super_class, .. } => {
            if let Some(e) = super_class {
                visit_expr_for_exports(e, shape, false);
            }
        }
        Stmt::If {
            test,
            consequent,
            alternate,
            ..
        } => {
            visit_expr_for_exports(test, shape, false);
            visit_stmt_for_exports(consequent, shape, false);
            if let Some(a) = alternate {
                visit_stmt_for_exports(a, shape, false);
            }
        }
        Stmt::For {
            init,
            test,
            update,
            body,
            ..
        } => {
            if let Some(init) = init {
                visit_for_init_for_exports(init, shape);
            }
            if let Some(test) = test {
                visit_expr_for_exports(test, shape, false);
            }
            if let Some(update) = update {
                visit_expr_for_exports(update, shape, false);
            }
            visit_stmt_for_exports(body, shape, false);
        }
        Stmt::ForIn {
            left, right, body, ..
        }
        | Stmt::ForOf {
            left, right, body, ..
        } => {
            visit_for_binding_for_exports(left, shape);
            visit_expr_for_exports(right, shape, false);
            visit_stmt_for_exports(body, shape, false);
        }
        Stmt::While { test, body, .. } | Stmt::DoWhile { test, body, .. } => {
            visit_expr_for_exports(test, shape, false);
            visit_stmt_for_exports(body, shape, false);
        }
        Stmt::With { object, body, .. } => {
            visit_expr_for_exports(object, shape, false);
            visit_stmt_for_exports(body, shape, false);
        }
        Stmt::Switch {
            discriminant,
            cases,
            ..
        } => {
            visit_expr_for_exports(discriminant, shape, false);
            for case in cases {
                if let Some(test) = &case.test {
                    visit_expr_for_exports(test, shape, false);
                }
                for s in &case.consequent {
                    visit_stmt_for_exports(s, shape, false);
                }
            }
        }
        Stmt::Try {
            block,
            handler,
            finalizer,
            ..
        } => {
            visit_stmt_for_exports(block, shape, false);
            if let Some(h) = handler {
                visit_stmt_for_exports(&h.body, shape, false);
            }
            if let Some(f) = finalizer {
                visit_stmt_for_exports(f, shape, false);
            }
        }
        Stmt::Return { argument, .. } => {
            if let Some(e) = argument {
                visit_expr_for_exports(e, shape, false);
            }
        }
        Stmt::Throw { argument, .. } => visit_expr_for_exports(argument, shape, false),
        Stmt::Labelled { body, .. } => visit_stmt_for_exports(body, shape, false),
        Stmt::Empty { .. }
        | Stmt::Break { .. }
        | Stmt::Continue { .. }
        | Stmt::Debugger { .. }
        | Stmt::Opaque { .. } => {}
    }
}

fn visit_var_for_exports(vs: &VariableStatement, shape: &mut StaticExportShape) {
    for d in &vs.declarators {
        if let Some(init) = &d.init {
            visit_expr_for_exports(init, shape, false);
        }
    }
}

fn visit_for_init_for_exports(init: &ForInit, shape: &mut StaticExportShape) {
    match init {
        ForInit::Variable(vs) => visit_var_for_exports(vs, shape),
        ForInit::Expression(e) => visit_expr_for_exports(e, shape, false),
    }
}

fn visit_for_binding_for_exports(left: &ForBinding, shape: &mut StaticExportShape) {
    if let ForBinding::AssignmentTarget(e) = left {
        visit_expr_for_exports(e, shape, false);
    }
}

fn visit_expr_for_exports(expr: &Expr, shape: &mut StaticExportShape, top: bool) {
    match expr {

        Expr::Assign { target, value, .. } => {
            match classify_member_target(target) {
                Some(TargetKind::ExportsNamed(name)) => shape.push_named(name),
                Some(TargetKind::ModuleExportsNamed(name)) => shape.push_named(name),
                Some(TargetKind::ModuleExports) => {

                    shape.clear_reexports();
                    if let Some(spec) = require_reexport_spec(unwrap_paren(value)) {
                        let plain_call = matches!(unwrap_paren(value), Expr::Call { callee, .. } if is_ident(callee, "require"));
                        if plain_call {
                            shape.module_exports_direct_require_reexport = true;
                        }

                        if plain_call || top {
                            shape.push_require_reexport_spec(spec);
                        }
                    }
                    if let Expr::Object { properties, .. } = unwrap_paren(value) {

                        shape.module_exports_reassigned = true;
                        for p in properties.iter() {
                            if let ObjectProperty::Property {
                                key, value, kind, ..
                            } = p
                            {
                                match node_cjs_object_export_property_action(*kind, value) {
                                    ObjectExportPropertyAction::Continue => {
                                        if let Some(name) = object_key_name(key) {
                                            shape.push_named(name);
                                        }
                                    }
                                    ObjectExportPropertyAction::EmitThenStop => {
                                        if let Some(name) = object_key_name(key) {
                                            shape.push_named(name);
                                        }
                                        if let Some(spec) = require_reexport_spec(value) {
                                            shape.push_require_reexport_spec(spec);
                                        }
                                        break;
                                    }
                                    ObjectExportPropertyAction::Stop => break,
                                }
                            } else if let ObjectProperty::Spread { expr, .. } = p {

                                if let Some(spec) = require_reexport_spec(expr) {
                                    shape.push_star_reexport_spec(spec);
                                    continue;
                                }
                                if matches!(unwrap_paren(expr), Expr::Identifier { .. }) {
                                    continue;
                                }
                                break;
                            } else {
                                break;
                            }
                        }
                    } else {
                        shape.module_exports_reassigned = true;
                    }
                }
                None => {}
            }

            visit_expr_for_exports(value, shape, false);
        }

        Expr::Call {
            callee, arguments, ..
        } => {

            if top && is_export_star_callee(callee) {
                if let (
                    Some(Argument::Expr(Expr::Call {
                        callee: require_callee,
                        arguments: require_args,
                        ..
                    })),
                    Some(Argument::Expr(target)),
                ) = (arguments.first(), arguments.get(1))
                {
                    if is_ident(require_callee, "require") && is_ident(target, "exports") {
                        if let Some(spec) = require_call_string_arg(require_args) {
                            shape.push_star_reexport_spec(spec);
                        }
                    }
                }
            }
            if is_object_define_property(callee) {
                if let (Some(Argument::Expr(obj)), Some(Argument::Expr(key))) =
                    (arguments.first(), arguments.get(1))
                {
                    if (is_ident(obj, "exports") || is_module_exports(obj))
                        && define_property_descriptor_admits(arguments.get(2))
                    {
                        if let Expr::StringLiteral { value, .. } = key {
                            shape.push_named(value.clone());
                        }
                    }
                }
            }
            visit_expr_for_exports(callee, shape, false);
            for a in arguments {
                match a {
                    Argument::Expr(e) | Argument::Spread { expr: e, .. } => {
                        visit_expr_for_exports(e, shape, false)
                    }
                }
            }
        }
        Expr::Parenthesized { expr, .. } => visit_expr_for_exports(expr, shape, top),
        Expr::Sequence { expressions, .. } => {
            for e in expressions {
                visit_expr_for_exports(e, shape, false);
            }
        }
        Expr::Function { body, .. } => {
            for s in body {
                visit_stmt_for_exports(s, shape, false);
            }
        }
        Expr::Arrow { body, .. } => match body {
            ArrowBody::Expression(e) => visit_expr_for_exports(e, shape, false),
            ArrowBody::Block(body) => {
                for s in body {
                    visit_stmt_for_exports(s, shape, false);
                }
            }
        },
        Expr::Array { elements, .. } => {
            for el in elements {
                match el {
                    rusty_js_ast::ArrayElement::Expr(e)
                    | rusty_js_ast::ArrayElement::Spread { expr: e, .. } => {
                        visit_expr_for_exports(e, shape, false)
                    }
                    rusty_js_ast::ArrayElement::Elision { .. } => {}
                }
            }
        }
        Expr::Object { properties, .. } => {
            for p in properties {
                match p {
                    ObjectProperty::Property { value, .. }
                    | ObjectProperty::Spread { expr: value, .. } => {
                        visit_expr_for_exports(value, shape, false)
                    }
                }
            }
        }
        Expr::Member {
            object, property, ..
        } => {
            visit_expr_for_exports(object, shape, false);
            if let MemberProperty::Computed { expr, .. } = property.as_ref() {
                visit_expr_for_exports(expr, shape, false);
            }
        }
        Expr::New {
            callee, arguments, ..
        } => {
            visit_expr_for_exports(callee, shape, false);
            for a in arguments {
                match a {
                    Argument::Expr(e) | Argument::Spread { expr: e, .. } => {
                        visit_expr_for_exports(e, shape, false)
                    }
                }
            }
        }
        Expr::Update { argument, .. } | Expr::Unary { argument, .. } => {
            visit_expr_for_exports(argument, shape, false);
        }
        Expr::Binary { left, right, .. } => {
            visit_expr_for_exports(left, shape, false);
            visit_expr_for_exports(right, shape, false);
        }
        Expr::Conditional {
            test,
            consequent,
            alternate,
            ..
        } => {
            visit_expr_for_exports(test, shape, false);
            visit_expr_for_exports(consequent, shape, false);
            visit_expr_for_exports(alternate, shape, false);
        }
        Expr::Class { super_class, .. } => {
            if let Some(e) = super_class {
                visit_expr_for_exports(e, shape, false);
            }
        }
        Expr::TemplateLiteral { expressions, .. } => {
            for e in expressions {
                visit_expr_for_exports(e, shape, false);
            }
        }
        _ => {}
    }
}

enum TargetKind {

    ExportsNamed(String),

    ModuleExportsNamed(String),

    ModuleExports,
}

fn classify_member_target(target: &Expr) -> Option<TargetKind> {
    if let Expr::Member {
        object, property, ..
    } = unwrap_paren(target)
    {
        let prop = member_prop_name(property)?;
        if is_ident(object, "exports") {
            return Some(TargetKind::ExportsNamed(prop));
        }
        if is_ident(object, "module") && prop == "exports" {
            return Some(TargetKind::ModuleExports);
        }
        if is_module_exports(object) {
            return Some(TargetKind::ModuleExportsNamed(prop));
        }
    }
    None
}

fn is_module_exports(e: &Expr) -> bool {
    if let Expr::Member {
        object, property, ..
    } = unwrap_paren(e)
    {
        return is_ident(object, "module")
            && member_prop_name(property).as_deref() == Some("exports");
    }
    false
}

fn member_prop_name(p: &MemberProperty) -> Option<String> {
    match p {
        MemberProperty::Identifier { name, .. } => Some(name.clone()),
        MemberProperty::Computed { expr, .. } => {
            if let Expr::StringLiteral { value, .. } = expr {
                Some(value.clone())
            } else {
                None
            }
        }
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

enum ObjectExportPropertyAction {
    Continue,
    EmitThenStop,
    Stop,
}

fn node_cjs_object_export_property_action(
    kind: rusty_js_ast::ObjectPropertyKind,
    value: &Expr,
) -> ObjectExportPropertyAction {

    if !matches!(kind, rusty_js_ast::ObjectPropertyKind::Init) {
        return ObjectExportPropertyAction::Stop;
    }
    match unwrap_paren(value) {
        Expr::Identifier { .. } => ObjectExportPropertyAction::Continue,
        Expr::BoolLiteral { .. } | Expr::NullLiteral { .. } => ObjectExportPropertyAction::Continue,

        e @ (Expr::Member { .. } | Expr::Call { .. } | Expr::New { .. }) => {
            if value_leftmost_base_is_identifier(e) {
                ObjectExportPropertyAction::EmitThenStop
            } else {
                ObjectExportPropertyAction::Stop
            }
        }

        Expr::Function { .. } => ObjectExportPropertyAction::EmitThenStop,

        e => {
            if value_leftmost_base_is_identifier(e) {
                ObjectExportPropertyAction::EmitThenStop
            } else {
                ObjectExportPropertyAction::Stop
            }
        }
    }
}

fn value_leftmost_base_is_identifier(e: &Expr) -> bool {
    match e {
        Expr::Identifier { .. } => true,
        Expr::Member { object, .. } => value_leftmost_base_is_identifier(object),
        Expr::Call { callee, .. } => value_leftmost_base_is_identifier(callee),
        Expr::New { .. } => true,
        Expr::Binary { left, .. } => value_leftmost_base_is_identifier(left),
        Expr::Conditional { test, .. } => value_leftmost_base_is_identifier(test),
        _ => false,
    }
}

fn define_property_descriptor_admits(desc: Option<&Argument>) -> bool {
    let Some(Argument::Expr(desc)) = desc else {
        return false;
    };

    let Expr::Object { properties, .. } = desc else {
        return false;
    };
    let props: Vec<(&str, &Expr, rusty_js_ast::ObjectPropertyKind)> = {
        let mut out = Vec::new();
        for p in properties {
            let ObjectProperty::Property {
                key, value, kind, ..
            } = p
            else {
                return false;
            };
            let name = match key {
                ObjectKey::Identifier { name, .. } => name.as_str(),
                ObjectKey::String { value, .. } => value.as_str(),
                _ => return false,
            };
            out.push((name, value, *kind));
        }
        out
    };
    match props.as_slice() {

        [("value", _, rusty_js_ast::ObjectPropertyKind::Init), ..] => true,
        [(
            "enumerable",
            Expr::BoolLiteral { value: true, .. },
            rusty_js_ast::ObjectPropertyKind::Init,
        ), ("value", _, rusty_js_ast::ObjectPropertyKind::Init), ..] => true,

        [(
            "enumerable",
            Expr::BoolLiteral { value: true, .. },
            rusty_js_ast::ObjectPropertyKind::Init,
        ), ("get", getter, rusty_js_ast::ObjectPropertyKind::Init)] => {
            define_property_getter_admits(getter)
        }
        [("get", getter, rusty_js_ast::ObjectPropertyKind::Init)] => {
            define_property_getter_admits(getter)
        }
        _ => false,
    }
}

fn define_property_getter_admits(getter: &Expr) -> bool {
    let Expr::Function { body, .. } = unwrap_paren(getter) else {
        return false;
    };
    let [Stmt::Return {
        argument: Some(ret),
        ..
    }] = body.as_slice()
    else {
        return false;
    };
    match unwrap_paren(ret) {
        Expr::Identifier { .. } => true,
        Expr::Member {
            object, property, ..
        } => {
            matches!(unwrap_paren(object), Expr::Identifier { .. })
                && match property.as_ref() {
                    MemberProperty::Identifier { .. } => true,
                    MemberProperty::Computed { expr, .. } => {
                        matches!(expr, Expr::StringLiteral { .. })
                    }
                    MemberProperty::Private { .. } => false,
                }
        }
        _ => false,
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

fn is_export_star_callee(callee: &Expr) -> bool {
    match unwrap_paren(callee) {
        Expr::Identifier { name, .. } => name == "__exportStar",
        Expr::Member { property, .. } => {
            member_prop_name(property).as_deref() == Some("__exportStar")
        }
        _ => false,
    }
}

fn require_reexport_spec(expr: &Expr) -> Option<String> {
    match unwrap_paren(expr) {
        Expr::Call {
            callee, arguments, ..
        } if is_ident(callee, "require") => require_call_string_arg(arguments),
        Expr::Member { object, .. } => require_reexport_spec(object),
        _ => None,
    }
}

fn require_call_string_arg(arguments: &[Argument]) -> Option<String> {
    match arguments.first()? {
        Argument::Expr(Expr::StringLiteral { value, .. }) => Some(value.clone()),
        _ => None,
    }
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

fn scan_static_export_names(source: &str, shape: &mut StaticExportShape) {
    let bytes = source.as_bytes();
    let require_bindings = scan_require_bindings(bytes);
    let mut i = 0;

    let mut depth: usize = 0;
    while i < bytes.len() {
        if let Some(next) = skip_js_comment_or_string(bytes, i) {
            i = next;
            continue;
        }
        match bytes[i] {
            b'{' => depth += 1,
            b'}' => depth = depth.saturating_sub(1),
            _ => {}
        }
        if depth == 0 && starts_ident_at(bytes, i, b"__exportStar") {
            if let Some((spec, next)) = scan_export_star_call(bytes, i + "__exportStar".len()) {
                shape.push_star_reexport_spec(spec);
                i = next;
                continue;
            }
        }
        if depth == 0 && starts_ident_at(bytes, i, b"__export") {
            if let Some((spec, next)) = scan_legacy_export_call(bytes, i + "__export".len()) {
                shape.push_star_reexport_spec(spec);
                i = next;
                continue;
            }
        }
        if starts_free_ident_at(bytes, i, b"Object") {
            if let Some((binding, next)) = scan_object_keys_for_each_binding(bytes, i) {
                if let Some((_, spec)) = require_bindings
                    .iter()
                    .find(|(candidate, _)| candidate.as_slice() == binding.as_slice())
                {
                    shape.push_object_keys_reexport_spec(spec.clone());
                    i = next;
                    continue;
                }
            }
        }
        if starts_free_ident_at(bytes, i, b"exports") {
            if let Some((name, next)) =
                scan_export_assignment_member_after(bytes, i + "exports".len())
            {
                shape.push_named(name);
                i = next;
                continue;
            }
        }
        if starts_free_ident_at(bytes, i, b"module") {
            let mut j = skip_ws(bytes, i + "module".len());
            if j < bytes.len() && bytes[j] == b'.' {
                j = skip_ws(bytes, j + 1);
                if starts_ident_at(bytes, j, b"exports") {
                    let after_exports = j + "exports".len();
                    if let Some((name, next)) =
                        scan_export_assignment_member_after(bytes, after_exports)
                    {
                        shape.push_named(name);
                        i = next;
                        continue;
                    }
                    if let Some((names, specs, next)) = scan_module_exports_inline_object(bytes, i)
                    {
                        for name in names {
                            shape.push_named(name);
                        }
                        for spec in specs {
                            shape.push_require_reexport_spec(spec);
                        }
                        shape.module_exports_reassigned = true;
                        i = next;
                        continue;
                    }

                    if let Some(after) = scan_module_exports_lvalue(bytes, i) {
                        let k = skip_ws(bytes, after);
                        if bytes.get(k) == Some(&b'=') && bytes.get(k + 1) != Some(&b'=') {
                            shape.clear_reexports();
                        }
                    }
                    shape.module_exports_reassigned = true;
                }
            }
        }
        i += 1;
    }
}

fn scan_require_bindings(bytes: &[u8]) -> Vec<(Vec<u8>, String)> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if let Some(next) = skip_js_comment_or_string(bytes, i) {
            i = next;
            continue;
        }
        let keyword_len = if starts_ident_at(bytes, i, b"var") {
            3
        } else if starts_ident_at(bytes, i, b"let") {
            3
        } else if starts_ident_at(bytes, i, b"const") {
            5
        } else {
            i += 1;
            continue;
        };
        let mut j = skip_ws(bytes, i + keyword_len);
        if j >= bytes.len() || !is_ident_start(bytes[j]) {
            i += keyword_len;
            continue;
        }
        let ident_start = j;
        j += 1;
        while j < bytes.len() && is_ident_continue(bytes[j]) {
            j += 1;
        }
        let ident = bytes[ident_start..j].to_vec();
        j = skip_ws(bytes, j);
        if bytes.get(j) != Some(&b'=') || bytes.get(j + 1) == Some(&b'=') {
            i += keyword_len;
            continue;
        }
        j = skip_ws(bytes, j + 1);
        if let Some((spec, next)) = scan_require_call_spec(bytes, j)
            .or_else(|| scan_interop_wrapped_require_call_spec(bytes, j))
        {
            out.push((ident, spec));
            i = next;
            continue;
        }
        i += keyword_len;
    }
    out
}

fn scan_object_keys_for_each_binding(
    bytes: &[u8],
    object_start: usize,
) -> Option<(Vec<u8>, usize)> {
    let mut j = object_start + "Object".len();
    j = skip_ws(bytes, j);
    if bytes.get(j) != Some(&b'.') {
        return None;
    }
    j = skip_ws(bytes, j + 1);
    if !starts_ident_at(bytes, j, b"keys") {
        return None;
    }
    j = skip_ws(bytes, j + "keys".len());
    if bytes.get(j) != Some(&b'(') {
        return None;
    }
    j = skip_ws(bytes, j + 1);
    if j >= bytes.len() || !is_ident_start(bytes[j]) {
        return None;
    }
    let ident_start = j;
    j += 1;
    while j < bytes.len() && is_ident_continue(bytes[j]) {
        j += 1;
    }
    let ident = bytes[ident_start..j].to_vec();
    j = skip_ws(bytes, j);
    if bytes.get(j) != Some(&b')') {
        return None;
    }
    j = skip_ws(bytes, j + 1);
    if bytes.get(j) != Some(&b'.') {
        return None;
    }
    j = skip_ws(bytes, j + 1);
    if !starts_ident_at(bytes, j, b"forEach") {
        return None;
    }
    j = skip_ws(bytes, j + "forEach".len());
    if bytes.get(j) != Some(&b'(') {
        return None;
    }
    let next = skip_call_expr(bytes, j)?;
    let body = &bytes[j..next];

    let callback_start = skip_ws(bytes, j + 1);
    if !starts_ident_at(bytes, callback_start, b"function") {
        return None;
    }

    let has_default =
        contains_subslice(body, b"\"default\"") || contains_subslice(body, b"'default'");
    let has_guard = contains_subslice(body, b"return") || contains_subslice(body, b"!==");
    if !has_default || !has_guard {
        return None;
    }
    let defines_export =
        contains_subslice(body, b"Object.defineProperty") && contains_free_ident(body, b"exports");
    let assigns_export = contains_exports_bracket_assignment(body);
    if !defines_export && !assigns_export {
        return None;
    }
    Some((ident, next))
}

fn scan_interop_wrapped_require_call_spec(
    bytes: &[u8],
    call_start: usize,
) -> Option<(String, usize)> {
    let mut j = call_start;
    if j >= bytes.len() || !is_ident_start(bytes[j]) {
        return None;
    }
    j += 1;
    while j < bytes.len() && is_ident_continue(bytes[j]) {
        j += 1;
    }
    j = skip_ws(bytes, j);
    if bytes.get(j) != Some(&b'(') {
        return None;
    }
    j = skip_ws(bytes, j + 1);
    let (spec, next) = scan_require_call_spec(bytes, j)?;
    j = skip_ws(bytes, next);
    if bytes.get(j) != Some(&b')') {
        return None;
    }
    Some((spec, j + 1))
}

fn contains_exports_bracket_assignment(bytes: &[u8]) -> bool {
    let mut i = 0;
    while i < bytes.len() {
        if let Some(next) = skip_js_comment_or_string(bytes, i) {
            i = next;
            continue;
        }
        if !starts_free_ident_at(bytes, i, b"exports") {
            i += 1;
            continue;
        }
        let mut j = skip_ws(bytes, i + "exports".len());
        if bytes.get(j) != Some(&b'[') {
            i += 1;
            continue;
        }
        let Some(after_bracket) = skip_balanced_bracket(bytes, j, b'[', b']') else {
            i += 1;
            continue;
        };
        j = after_bracket;
        j = skip_ws(bytes, j);
        if bytes.get(j) == Some(&b'=')
            && bytes.get(j + 1) != Some(&b'=')
            && bytes.get(j + 1) != Some(&b'>')
        {
            return true;
        }
        i += 1;
    }
    false
}

fn skip_balanced_bracket(bytes: &[u8], open: usize, open_ch: u8, close_ch: u8) -> Option<usize> {
    if bytes.get(open) != Some(&open_ch) {
        return None;
    }
    let mut depth = 1usize;
    let mut i = open + 1;
    while i < bytes.len() {
        if let Some(next) = skip_js_comment_or_string(bytes, i) {
            i = next;
            continue;
        }
        if bytes[i] == open_ch {
            depth += 1;
        } else if bytes[i] == close_ch {
            depth = depth.saturating_sub(1);
            if depth == 0 {
                return Some(i + 1);
            }
        }
        i += 1;
    }
    None
}

fn contains_subslice(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

fn contains_free_ident(haystack: &[u8], needle: &[u8]) -> bool {
    let mut i = 0;
    while i + needle.len() <= haystack.len() {
        if starts_free_ident_at(haystack, i, needle) {
            return true;
        }
        i += 1;
    }
    false
}

fn scan_export_star_call(bytes: &[u8], after_callee: usize) -> Option<(String, usize)> {
    let mut j = skip_ws(bytes, after_callee);
    if bytes.get(j) != Some(&b'(') {
        return None;
    }
    j = skip_ws(bytes, j + 1);
    let (spec, next) = scan_require_call_spec(bytes, j)?;
    j = skip_ws(bytes, next);
    if bytes.get(j) != Some(&b',') {
        return None;
    }
    j = skip_ws(bytes, j + 1);
    if !starts_free_ident_at(bytes, j, b"exports") {
        return None;
    }
    j = skip_ws(bytes, j + "exports".len());
    if bytes.get(j) != Some(&b')') {
        return None;
    }
    Some((spec, j + 1))
}

fn scan_legacy_export_call(bytes: &[u8], after_callee: usize) -> Option<(String, usize)> {
    let mut j = skip_ws(bytes, after_callee);
    if bytes.get(j) != Some(&b'(') {
        return None;
    }
    j = skip_ws(bytes, j + 1);
    let (spec, next) = scan_require_call_spec(bytes, j)?;
    j = skip_ws(bytes, next);
    if bytes.get(j) != Some(&b')') {
        return None;
    }
    Some((spec, j + 1))
}

fn scan_module_exports_lvalue(bytes: &[u8], start: usize) -> Option<usize> {
    if !starts_free_ident_at(bytes, start, b"module") {
        return None;
    }
    let mut j = skip_ws(bytes, start + "module".len());
    if bytes.get(j) != Some(&b'.') {
        return None;
    }
    j = skip_ws(bytes, j + 1);
    if !starts_ident_at(bytes, j, b"exports") {
        return None;
    }
    Some(j + "exports".len())
}

fn scan_module_exports_inline_object(
    bytes: &[u8],
    module_start: usize,
) -> Option<(Vec<String>, Vec<String>, usize)> {
    let after_exports = scan_module_exports_lvalue(bytes, module_start)?;
    let mut j = skip_ws(bytes, after_exports);
    if bytes.get(j) != Some(&b'=') || bytes.get(j + 1) == Some(&b'=') {
        return None;
    }
    j = skip_ws(bytes, j + 1);
    if bytes.get(j) != Some(&b'{') {
        return None;
    }
    let (names, specs, next) = scan_inline_object_literal_entries(bytes, j);
    Some((names, specs, next))
}

fn scan_empty_object_literal(bytes: &[u8], object_start: usize) -> Option<usize> {
    if bytes.get(object_start) != Some(&b'{') {
        return None;
    }
    let j = skip_ws(bytes, object_start + 1);
    if bytes.get(j) == Some(&b'}') {
        Some(j + 1)
    } else {
        None
    }
}

fn scan_inline_object_literal_entries(
    bytes: &[u8],
    object_start: usize,
) -> (Vec<String>, Vec<String>, usize) {
    let mut names = Vec::new();
    let mut specs = Vec::new();
    let mut i = object_start + 1;
    let mut depth = 1usize;
    let mut expecting_key = true;
    while i < bytes.len() && depth > 0 {

        if depth == 1
            && expecting_key
            && (matches!(bytes[i], b'\'' | b'"') || is_ident_start(bytes[i]))
        {
            let (name, mut j) = if matches!(bytes[i], b'\'' | b'"') {
                let quote = bytes[i];
                let start = i + 1;
                let end = skip_quoted(bytes, i, quote).saturating_sub(1);
                (
                    String::from_utf8_lossy(&bytes[start..end]).into_owned(),
                    skip_quoted(bytes, i, quote),
                )
            } else {
                let start = i;
                i += 1;
                while i < bytes.len() && is_ident_continue(bytes[i]) {
                    i += 1;
                }
                (String::from_utf8_lossy(&bytes[start..i]).into_owned(), i)
            };
            j = skip_ws(bytes, j);
            if bytes.get(j) != Some(&b':') {

                if (name == "get" || name == "set")
                    && bytes
                        .get(j)
                        .copied()
                        .is_some_and(|b| is_ident_start(b) || matches!(b, b'\'' | b'"'))
                {
                    return (names, specs, j);
                }

                if bytes.get(j) == Some(&b'(') {
                    push_unique_name(&mut names, name);
                    return (names, specs, j);
                }

                if matches!(bytes.get(j), Some(b',') | Some(b'}')) {
                    push_unique_name(&mut names, name);
                }
                i = j;
                expecting_key = false;
                continue;
            }
            j = skip_ws(bytes, j + 1);
            match inline_object_value_action(bytes, j) {
                InlineObjectValueAction::Continue => {
                    push_unique_name(&mut names, name);
                    i = skip_value_expr(bytes, j);
                    expecting_key = false;
                }
                InlineObjectValueAction::EmitThenStop { require_spec } => {
                    push_unique_name(&mut names, name);
                    if let Some(spec) = require_spec {
                        push_unique_name(&mut specs, spec);
                    }
                    return (names, specs, skip_value_expr(bytes, j));
                }
                InlineObjectValueAction::Stop => return (names, specs, j),
            }
            continue;
        }
        if let Some(next) = skip_js_comment_or_string(bytes, i) {
            i = next;
            continue;
        }
        match bytes[i] {
            b'{' | b'[' | b'(' => {
                depth += 1;
                i += 1;
            }
            b'}' => {
                depth -= 1;
                i += 1;
            }
            b']' | b')' => {
                depth = depth.saturating_sub(1);
                i += 1;
            }
            b',' if depth == 1 => {
                expecting_key = true;
                i += 1;
            }

            b'.' if depth == 1 && expecting_key => {
                expecting_key = false;
                i += 1;
            }
            b':' if depth == 1 => {
                expecting_key = false;
                i += 1;
            }
            _ => i += 1,
        }
    }
    (names, specs, i)
}

enum InlineObjectValueAction {
    Continue,
    EmitThenStop { require_spec: Option<String> },
    Stop,
}

fn inline_object_value_action(bytes: &[u8], value_start: usize) -> InlineObjectValueAction {
    let j = skip_ws(bytes, value_start);
    if let Some((spec, _)) = scan_require_call_spec(bytes, j) {
        return InlineObjectValueAction::EmitThenStop {
            require_spec: Some(spec),
        };
    }
    if starts_ident_at(bytes, j, b"function") {
        return InlineObjectValueAction::EmitThenStop { require_spec: None };
    }

    if starts_ident_at(bytes, j, b"new") {
        return InlineObjectValueAction::EmitThenStop { require_spec: None };
    }
    if j < bytes.len() && is_ident_start(bytes[j]) {
        let mut k = j + 1;
        while k < bytes.len() && is_ident_continue(bytes[k]) {
            k += 1;
        }
        k = skip_ws(bytes, k);

        if matches!(bytes.get(k), Some(b',') | Some(b'}')) {
            InlineObjectValueAction::Continue
        } else {
            InlineObjectValueAction::EmitThenStop { require_spec: None }
        }
    } else {
        InlineObjectValueAction::Stop
    }
}

fn scan_require_call_spec(bytes: &[u8], start: usize) -> Option<(String, usize)> {
    if !starts_free_ident_at(bytes, start, b"require") {
        return None;
    }
    let mut j = skip_ws(bytes, start + "require".len());
    if bytes.get(j) != Some(&b'(') {
        return None;
    }
    j = skip_ws(bytes, j + 1);
    if !matches!(bytes.get(j), Some(b'\'') | Some(b'"')) {
        return None;
    }
    let quote = bytes[j];
    let value_start = j + 1;
    let end = skip_quoted(bytes, j, quote).saturating_sub(1);
    let spec = String::from_utf8_lossy(&bytes[value_start..end]).into_owned();
    j = skip_ws(bytes, skip_quoted(bytes, j, quote));
    if bytes.get(j) != Some(&b')') {
        return None;
    }
    Some((spec, j + 1))
}

fn skip_value_expr(bytes: &[u8], mut i: usize) -> usize {
    let mut depth = 0usize;
    while i < bytes.len() {
        if let Some(next) = skip_js_comment_or_string(bytes, i) {
            i = next;
            continue;
        }
        match bytes[i] {
            b'{' | b'[' | b'(' => {
                depth += 1;
                i += 1;
            }
            b'}' if depth == 0 => return i,
            b'}' | b']' | b')' => {
                depth = depth.saturating_sub(1);
                i += 1;
            }
            b',' if depth == 0 => return i,
            _ => i += 1,
        }
    }
    i
}

fn skip_call_expr(bytes: &[u8], open_paren: usize) -> Option<usize> {
    if bytes.get(open_paren) != Some(&b'(') {
        return None;
    }
    let mut i = open_paren + 1;
    let mut depth = 1usize;
    while i < bytes.len() {
        if let Some(next) = skip_js_comment_or_string(bytes, i) {
            i = next;
            continue;
        }
        match bytes[i] {
            b'(' | b'{' | b'[' => {
                depth += 1;
                i += 1;
            }
            b')' | b'}' | b']' => {
                depth = depth.saturating_sub(1);
                i += 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => i += 1,
        }
    }
    None
}

fn push_unique_name(names: &mut Vec<String>, name: String) {
    if !names.iter().any(|n| n == &name) {
        names.push(name);
    }
}

fn scan_export_assignment_member_after(
    bytes: &[u8],
    after_exports: usize,
) -> Option<(String, usize)> {
    let mut j = skip_ws(bytes, after_exports);
    if j >= bytes.len() {
        return None;
    }
    if bytes[j] == b'.' {
        j = skip_ws(bytes, j + 1);
        let start = j;
        if j < bytes.len() && is_ident_start(bytes[j]) {
            j += 1;
            while j < bytes.len() && is_ident_continue(bytes[j]) {
                j += 1;
            }
            if export_member_is_assignment_target(bytes, j) {
                return Some((String::from_utf8_lossy(&bytes[start..j]).into_owned(), j));
            }
            return None;
        }
    }
    if bytes[j] == b'[' {
        j = skip_ws(bytes, j + 1);
        if j < bytes.len() && (bytes[j] == b'\'' || bytes[j] == b'"') {
            let quote = bytes[j];
            j += 1;
            let start = j;
            while j < bytes.len() {
                if bytes[j] == b'\\' {
                    j = j.saturating_add(2);
                    continue;
                }
                if bytes[j] == quote {
                    let name = String::from_utf8_lossy(&bytes[start..j]).into_owned();
                    j += 1;
                    j = skip_ws(bytes, j);
                    if j < bytes.len() && bytes[j] == b']' {
                        let next = j + 1;
                        if export_member_is_assignment_target(bytes, next) {
                            return Some((name, next));
                        }
                        return None;
                    }
                    return None;
                }
                j += 1;
            }
        }
    }
    None
}

fn export_member_is_assignment_target(bytes: &[u8], after_member: usize) -> bool {
    let j = skip_ws(bytes, after_member);
    bytes.get(j) == Some(&b'=') && bytes.get(j + 1) != Some(&b'=')
}

fn skip_js_comment_or_string(bytes: &[u8], i: usize) -> Option<usize> {
    match bytes.get(i).copied()? {
        b'\'' | b'"' => Some(skip_quoted(bytes, i, bytes[i])),
        b'`' => Some(skip_quoted(bytes, i, b'`')),
        b'/' if bytes.get(i + 1) == Some(&b'/') => {
            let mut j = i + 2;
            while j < bytes.len() && !matches!(bytes[j], b'\n' | b'\r') {
                j += 1;
            }
            Some(j)
        }
        b'/' if bytes.get(i + 1) == Some(&b'*') => {
            let mut j = i + 2;
            while j + 1 < bytes.len() {
                if bytes[j] == b'*' && bytes[j + 1] == b'/' {
                    return Some(j + 2);
                }
                j += 1;
            }
            Some(bytes.len())
        }
        _ => None,
    }
}

fn skip_quoted(bytes: &[u8], i: usize, quote: u8) -> usize {
    let mut j = i + 1;
    while j < bytes.len() {
        if bytes[j] == b'\\' {
            j = j.saturating_add(2);
            continue;
        }
        if bytes[j] == quote {
            return j + 1;
        }
        j += 1;
    }
    bytes.len()
}

fn starts_free_ident_at(bytes: &[u8], i: usize, needle: &[u8]) -> bool {
    starts_ident_at(bytes, i, needle) && (i == 0 || bytes[i - 1] != b'.')
}

fn starts_ident_at(bytes: &[u8], i: usize, needle: &[u8]) -> bool {
    bytes.get(i..i + needle.len()) == Some(needle)
        && (i == 0 || !is_ident_continue(bytes[i - 1]))
        && bytes
            .get(i + needle.len())
            .copied()
            .is_none_or(|b| !is_ident_continue(b))
}

fn skip_ws(bytes: &[u8], mut i: usize) -> usize {
    while i < bytes.len() && matches!(bytes[i], b' ' | b'\n' | b'\r' | b'\t' | 0x0b | 0x0c) {
        i += 1;
    }
    i
}

fn is_ident_start(b: u8) -> bool {
    b == b'_' || b == b'$' || b.is_ascii_alphabetic()
}

fn is_ident_continue(b: u8) -> bool {
    is_ident_start(b) || b.is_ascii_digit()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusty_js_parser::{parse_module, parse_script};

    fn shape_of(src: &str) -> StaticExportShape {
        let module = parse_script(src).expect("parse");

        let stmts: Vec<Stmt> = module
            .body
            .into_iter()
            .filter_map(|item| match item {
                rusty_js_ast::ModuleItem::Statement(s) => Some(s),
                _ => None,
            })
            .collect();
        extract_static_export_shape(&stmts)
    }

    #[test]
    fn exports_named_assignments() {
        let s = shape_of("exports.Foo = function(){}; exports.BAR = 5;");
        assert_eq!(s.named, vec!["Foo", "BAR"]);
        assert!(!s.module_exports_reassigned);
        let keys = s.lower_node_keys();
        assert!(keys.contains(&"Foo".to_string()));
        assert!(keys.contains(&"default".to_string()));
        assert!(keys.contains(&"module.exports".to_string()));
    }

    #[test]
    fn ts_declaration_idiom_and_reassignment() {

        let s = shape_of(
            "exports.KeywordCxt = exports.Ajv = void 0; exports.Ajv = Ajv; module.exports = Ajv;",
        );
        assert!(s.named.contains(&"Ajv".to_string()));
        assert!(s.named.contains(&"KeywordCxt".to_string()));
        assert!(s.module_exports_reassigned);
    }

    #[test]
    fn module_exports_object_literal() {
        let s = shape_of("module.exports = { a: 1, b: function(){} };");
        assert!(s.named.is_empty());
        assert!(s.module_exports_reassigned);
    }

    #[test]
    fn module_exports_object_literal_member_value_is_named() {
        let s = shape_of(
            "const defaults = schema.defaults; \
             module.exports = { createInstrumenter, defaultOpts: defaults.instrumenter, lit: 1, fn: function() {} };",
        );
        assert!(s.module_exports_reassigned);
        assert!(s.named.contains(&"createInstrumenter".to_string()));
        assert!(s.named.contains(&"defaultOpts".to_string()));
        assert!(!s.named.contains(&"lit".to_string()));
        assert!(!s.named.contains(&"fn".to_string()));
    }

    #[test]
    fn module_exports_object_literal_member_value_emits_then_stops() {
        let s = shape_of(
            "var internalRe = require('./internal/re'); \
             module.exports = { \
                re: internalRe.re, \
                src: internalRe.src, \
                tokens: internalRe.t \
             };",
        );
        assert!(s.module_exports_reassigned);
        assert_eq!(s.named, vec!["re"]);
    }

    #[test]
    fn source_scan_object_literal_member_value_emits_then_stops() {
        let s = extract_static_export_shape_from_cjs_source(
            "module.exports = { \
                Service: services.Service, \
                Type: types.Type, \
                assembleProtocol: specs.assembleProtocol \
             };",
        );
        assert!(s.module_exports_reassigned);
        assert_eq!(s.named, vec!["Service"]);
    }

    #[test]
    fn module_exports_object_literal_first_method_is_named() {
        let s = shape_of(
            "module.exports = { \
                createCoverageSummary(obj) { return obj; }, \
                createCoverageMap(obj) { return obj; } \
             }; \
             module.exports.classes = {};",
        );
        assert!(s.module_exports_reassigned);
        assert!(s.named.contains(&"createCoverageSummary".to_string()));
        assert!(s.named.contains(&"classes".to_string()));
        assert!(!s.named.contains(&"createCoverageMap".to_string()));
    }

    #[test]
    fn module_exports_object_literal_call_value_terminates_scan() {
        let s = shape_of(
            "module.exports = { \
                bit: bit, \
                bool: bool, \
                equal: curry2(equal), \
                error: isError, \
                fn: isFn \
             };",
        );
        assert!(s.module_exports_reassigned);
        assert_eq!(s.named, vec!["bit", "bool", "equal"]);
    }

    #[test]
    fn module_exports_object_literal_function_value_emits_then_stops() {
        let s = shape_of(
            "module.exports = { \
                first: first, \
                getFile: function(name) { return name; }, \
                getFilePath: function(name) { return name; } \
             };",
        );
        assert!(s.module_exports_reassigned);
        assert_eq!(s.named, vec!["first", "getFile"]);
    }

    #[test]
    fn module_exports_object_literal_require_value_records_source_spec() {
        let s = shape_of(
            "module.exports = { \
                api: require('./api'), \
                css: require('./css') \
             };",
        );
        assert!(s.module_exports_reassigned);
        assert!(s.module_exports_require_reexport);
        assert!(!s.module_exports_direct_require_reexport);
        assert_eq!(s.named, vec!["api"]);
        assert_eq!(s.module_exports_require_specs, vec!["./api"]);
    }

    #[test]
    fn source_scan_recovers_inline_object_require_value_source_spec() {
        let s = extract_static_export_shape_from_cjs_source(
            "module.exports = { \
                api: require('./api'), \
                css: require('./css') \
             };",
        );
        assert!(s.module_exports_reassigned);
        assert!(s.module_exports_require_reexport);
        assert!(!s.module_exports_direct_require_reexport);
        assert_eq!(s.named, vec!["api"]);
        assert_eq!(s.module_exports_require_specs, vec!["./api"]);
    }

    #[test]
    fn module_exports_object_literal_literal_value_terminates_scan() {
        let s = shape_of("module.exports = { lit: 1, later: later };");
        assert!(s.module_exports_reassigned);
        assert!(s.named.is_empty());
    }

    #[test]
    fn module_exports_object_literal_boolean_map_is_named() {
        let s = shape_of(
            r#"module.exports = { "area": true, "base": true, br: false, count: 1, later: true };"#,
        );
        assert!(s.module_exports_reassigned);
        assert_eq!(s.named, vec!["area", "base", "br"]);
    }

    #[test]
    fn es_module_flag_and_define_property() {
        let s = shape_of(
            "Object.defineProperty(exports, \"__esModule\", { value: true }); \
             Object.defineProperty(exports, \"Foo\", { get: function(){ return q; } });",
        );
        assert!(s.has_es_module_flag);
        assert_eq!(s.named, vec!["Foo"]);
        assert!(s.lower_node_keys().contains(&"__esModule".to_string()));

        let s = shape_of("Object.defineProperty(exports, \"Bar\", { get: function(){} });");
        assert!(s.named.is_empty());

        let s = shape_of(
            "Object.defineProperty(exports, \"__esModule\", ({ value: true })); exports.a = 1;",
        );
        assert!(!s.has_es_module_flag);
        assert_eq!(s.named, vec!["a"]);
    }

    #[test]
    fn bare_function_reassignment_has_no_named() {
        let s = shape_of("module.exports = function greet(){ return 1; };");
        assert!(s.named.is_empty());
        assert!(s.module_exports_reassigned);
        assert!(!s.module_exports_require_reexport);

        let mut keys = s.lower_node_keys();
        keys.sort();
        assert_eq!(
            keys,
            vec!["default".to_string(), "module.exports".to_string()]
        );
    }

    #[test]
    fn module_exports_require_reexport_is_flagged() {
        let s = shape_of("module.exports = require('./core.cjs.development.js');");
        assert!(s.module_exports_reassigned);
        assert!(s.module_exports_require_reexport);
        assert!(s.module_exports_direct_require_reexport);
        assert_eq!(
            s.module_exports_require_specs,
            vec!["./core.cjs.development.js"]
        );
        assert_eq!(s.lower_node_keys(), vec!["default", "module.exports"]);
    }

    #[test]
    fn module_exports_require_member_reexport_records_source_spec() {
        let s = shape_of("module.exports = require('ci-info').isCI;");
        assert!(s.module_exports_reassigned);
        assert!(s.module_exports_require_reexport);
        assert!(!s.module_exports_direct_require_reexport);
        assert_eq!(s.module_exports_require_specs, vec!["ci-info"]);
        assert_eq!(s.lower_node_keys(), vec!["default", "module.exports"]);
    }

    #[test]
    fn export_star_helper_records_source_specs() {
        let s = shape_of(
            "__exportStar(require('./alpha'), exports); \
             tslib_1.__exportStar(require(\"./beta\"), exports);",
        );
        assert_eq!(s.cjs_star_reexport_specs, vec!["./alpha", "./beta"]);
        assert_eq!(s.lower_node_keys(), vec!["default", "module.exports"]);
    }

    #[test]
    fn source_scan_recovers_export_star_helper_specs() {
        let s = extract_static_export_shape_from_cjs_source(
            "__exportStar(require('./alpha'), exports); \
             tslib_1.__exportStar(require(\"./beta\"), exports);",
        );
        assert_eq!(s.cjs_star_reexport_specs, vec!["./alpha", "./beta"]);
    }

    #[test]
    fn source_scan_recovers_legacy_export_helper_specs() {
        let s = extract_static_export_shape_from_cjs_source(
            "function __export(m) { \
               for (var p in m) if (!exports.hasOwnProperty(p)) exports[p] = m[p]; \
             } \
             exports.__esModule = true; \
             __export(require('./dist'));",
        );
        assert_eq!(s.cjs_star_reexport_specs, vec!["./dist"]);
        assert_eq!(
            s.lower_node_keys(),
            vec!["default", "module.exports", "__esModule"]
        );
    }

    #[test]
    fn source_scan_recovers_object_keys_reexport_helper_specs() {
        let s = extract_static_export_shape_from_cjs_source(
            "var _presets = require('./presets'); \
             Object.keys(_presets).forEach(function (key) { \
               if (key === 'default' || key === '__esModule') return; \
               Object.defineProperty(exports, key, { enumerable: true, get: function () { return _presets[key]; } }); \
             });",
        );
        assert_eq!(s.cjs_object_keys_reexport_specs, vec!["./presets"]);
    }

    #[test]
    fn source_scan_recovers_object_keys_direct_export_assignment_specs() {
        let s = extract_static_export_shape_from_cjs_source(
            "var _traverse = require('./traverse'); \
             Object.keys(_traverse).forEach(function (key) { \
               if (key === 'default' || key === '__esModule') return; \
               if (key in exports && exports[key] === _traverse[key]) return; \
               exports[key] = _traverse[key]; \
             });",
        );
        assert_eq!(s.cjs_object_keys_reexport_specs, vec!["./traverse"]);
    }

    #[test]
    fn source_scan_recovers_object_keys_interop_wrapped_require_specs() {
        let s = extract_static_export_shape_from_cjs_source(
            "var _next = _interopRequireWildcard(require('./next')); \
             Object.keys(_next).forEach(function (key) { \
               if (key === 'default' || key === '__esModule') return; \
               if (key in exports && exports[key] === _next[key]) return; \
               Object.defineProperty(exports, key, { enumerable: true, get: function () { return _next[key]; } }); \
             });",
        );
        assert_eq!(s.cjs_object_keys_reexport_specs, vec!["./next"]);
    }

    #[test]
    fn names_survive_module_exports_reassignment() {
        let s = shape_of(
            "exports.KeywordCxt = exports.Ajv = void 0; \
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
    fn module_exports_member_assignment_is_named() {
        let s = shape_of(
            "module.exports = createColors(); module.exports.createColors = createColors;",
        );
        assert!(s.module_exports_reassigned);
        assert!(s.named.contains(&"createColors".to_string()));
        assert!(s.lower_node_keys().contains(&"createColors".to_string()));
    }

    #[test]
    fn variable_initializer_assignment_chain_is_named() {
        let s = shape_of("var Script = exports.Script = function NodeScript() {};");
        assert_eq!(s.named, vec!["Script"]);
        assert!(s.lower_node_keys().contains(&"Script".to_string()));
    }

    #[test]
    fn nested_wrapper_body_is_walked_flow_insensitively() {
        let s = shape_of(
            "(function(factory) { module.exports = factory(); })(function() { \
                exports.Runner = runner; \
                if (enabled) { exports.describe = describe; } \
                return exports; \
            });",
        );
        assert!(s.module_exports_reassigned);
        let keys = s.lower_node_keys();
        assert!(keys.contains(&"Runner".to_string()));
        assert!(keys.contains(&"describe".to_string()));
        assert!(keys.contains(&"default".to_string()));
        assert!(keys.contains(&"module.exports".to_string()));
    }

    #[test]
    fn source_scan_recovers_bracket_and_bundle_exports() {
        let s = extract_static_export_shape_from_cjs_source(
            "exports.reporters = reporters; \
             exports[\"json-stream\"] = jsonStream; \
             var Script = exports.Script = function NodeScript() {};",
        );
        assert!(s.named.contains(&"reporters".to_string()));
        assert!(s.named.contains(&"json-stream".to_string()));
        assert!(s.named.contains(&"Script".to_string()));
    }

    #[test]
    fn source_scan_recovers_nested_mocha_public_names() {
        let s = extract_static_export_shape_from_cjs_source(
            "module.exports = factory(); \
             function inner(exports) { \
               exports.utils = utils; \
               exports.Context = context; \
               exports.describe = function() {}; \
               exports.xit = exports.it.skip; \
             }",
        );
        assert!(s.module_exports_reassigned);
        let keys = s.lower_node_keys();
        assert!(keys.contains(&"utils".to_string()));
        assert!(keys.contains(&"Context".to_string()));
        assert!(keys.contains(&"describe".to_string()));
        assert!(keys.contains(&"xit".to_string()));
    }

    #[test]
    fn source_scan_ignores_property_exports_and_text() {
        let s = extract_static_export_shape_from_cjs_source(
            "src$3.exports.postcss = true; \
             wasmHash.exports.MAX_SHORT_STRING = MAX_SHORT_STRING; \
             this.exports.init(); \
             exports.init(); \
             exports.memory.buffer; \
             // exports.comment = nope\n\
             const msg = 'exports.If is text'; \
             exports.real = value; \
             module.exports.named = value;",
        );
        assert_eq!(s.named, vec!["real", "named"]);
    }

    #[test]
    fn source_scan_ignores_module_exports_alias_properties() {
        let s = extract_static_export_shape_from_cjs_source(
            "var assert = module.exports = ok; \
             assert.fail = fail; \
             assert.AssertionError = AssertionError;",
        );
        assert!(s.module_exports_reassigned);
        assert!(s.named.is_empty());
    }

    #[test]
    fn object_literal_value_actions_match_node_oracle() {

        let s = shape_of("var g = 1; module.exports = { get a(){ return g; }, b: g };");
        assert!(s.named.is_empty());

        let s = shape_of("var g = 1; module.exports = { b: g, a(){}, c: g };");
        assert_eq!(s.named, vec!["b", "a"]);

        let s = shape_of("function F(){} module.exports = { a: new F(), b: F };");
        assert_eq!(s.named, vec!["a"]);

        let s = shape_of("var g = 1, h = 2; module.exports = { a: g, b: g && h, c: g };");
        assert_eq!(s.named, vec!["a", "b"]);

        let s = shape_of("var v = []; module.exports = { extends: [1].map(f), rules: v };");
        assert!(s.named.is_empty());
    }

    #[test]
    fn star_reexport_top_level_and_rebind_clear() {
        let star = "__exportStar(require(\"./sub\"), exports);";
        let s = extract_static_export_shape_from_cjs_source(star);
        assert_eq!(s.cjs_star_reexport_specs, vec!["./sub"]);
        let s = extract_static_export_shape_from_cjs_source(&format!("if (x) {{ {star} }}"));
        assert!(s.cjs_star_reexport_specs.is_empty());
        let s = extract_static_export_shape_from_cjs_source(&format!("{star} module.exports = f;"));
        assert!(s.cjs_star_reexport_specs.is_empty());
        let s = extract_static_export_shape_from_cjs_source(&format!("module.exports = f; {star}"));
        assert_eq!(s.cjs_star_reexport_specs, vec!["./sub"]);
    }

    #[test]
    fn object_keys_foreach_requires_canonical_shape() {
        let pre = "var emo = require(\"./sub\");";
        let guarded = "Object.keys(emo).forEach(function (key) { if (key === \"default\" || key === \"__esModule\") return; exports[key] = emo[key]; });";
        let s = extract_static_export_shape_from_cjs_source(&format!("{pre} {guarded}"));
        assert_eq!(s.cjs_object_keys_reexport_specs, vec!["./sub"]);
        let unguarded = "Object.keys(emo).forEach(function (key) { exports[key] = emo[key]; });";
        let s = extract_static_export_shape_from_cjs_source(&format!("{pre} {unguarded}"));
        assert!(s.cjs_object_keys_reexport_specs.is_empty());
        let arrow = "Object.keys(emo).forEach(key => { if (key === \"default\") return; exports[key] = emo[key]; });";
        let s = extract_static_export_shape_from_cjs_source(&format!("{pre} {arrow}"));
        assert!(s.cjs_object_keys_reexport_specs.is_empty());
    }

    #[test]
    fn source_scan_identifier_rebind_admits_no_names() {
        let s = extract_static_export_shape_from_cjs_source(
            "var punycode = { \
                'version': '2.1.0', \
                ucs2: { decode: ucs2decode, encode: ucs2encode }, \
                decode: decode, \
                toASCII: toASCII \
             }; \
             module.exports = punycode;",
        );
        assert!(s.module_exports_reassigned);
        assert!(s.named.is_empty());
        let s = extract_static_export_shape_from_cjs_source(
            "var check = { every: logic.every, map: logic.map, defined: defined }; \
             module.exports = check;",
        );
        assert!(s.module_exports_reassigned);
        assert!(s.named.is_empty());
    }

    #[test]
    fn source_scan_does_not_treat_object_member_rebind_as_object_literal_rebind() {
        let s = extract_static_export_shape_from_cjs_source(
            "const sharedGlobalObject = { Array, Error, Object, Promise, String, TypeError }; \
             install(sharedGlobalObject); \
             module.exports = sharedGlobalObject.DOMException;",
        );
        assert!(s.module_exports_reassigned);
        assert!(s.named.is_empty());
        assert_eq!(s.lower_node_keys(), vec!["default", "module.exports"]);
    }

    #[test]
    fn cjs_wrapper_static_shape_is_recovered() {
        let wrapped = r#"
            export default (function (exports, module, require, __filename, __dirname) {
                exports.KeywordCxt = exports.Ajv = void 0;
                exports.Ajv = 1;
                exports.KeywordCxt = 2;
                module.exports = {};
            });
        "#;
        let ast = parse_module(wrapped).expect("parse wrapper");
        let shape = extract_static_export_shape_from_cjs_wrapper(&ast).expect("wrapper shape");
        assert!(shape.module_exports_reassigned);
        let keys = shape.lower_node_keys();
        assert!(keys.contains(&"Ajv".to_string()));
        assert!(keys.contains(&"KeywordCxt".to_string()));
        assert!(keys.contains(&"default".to_string()));
        assert!(keys.contains(&"module.exports".to_string()));
    }
}
