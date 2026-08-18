
use rusty_js_ast::{
    Argument, ArrowBody, ClassMember, ClassMemberName, DefaultExportBody, ExportDeclaration, Expr,
    ForInit, MemberProperty, Module, ModuleItem, ObjectProperty, Parameter, Stmt,
};

use crate::ParseError;

thread_local! {
    static AMBIENT_PRIVATE_NAMES: std::cell::RefCell<Vec<String>> = const { std::cell::RefCell::new(Vec::new()) };

    static VALIDATION_SOURCE: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
}

pub fn set_ambient_private_names(names: Vec<String>) {
    AMBIENT_PRIVATE_NAMES.with(|c| *c.borrow_mut() = names);
}

pub fn validate_all_private_names(module: &Module, src: &str) -> Result<(), ParseError> {
    VALIDATION_SOURCE.with(|c| {
        let mut b = c.borrow_mut();
        b.clear();
        b.push_str(src);
    });
    let scope: Vec<String> = AMBIENT_PRIVATE_NAMES.with(|c| c.borrow().clone());
    let result = (|| {
        for item in &module.body {
            match item {
                ModuleItem::Statement(s) => walk_stmt(s, &scope)?,
                ModuleItem::Export(e) => walk_export(e, &scope)?,
                ModuleItem::Import(_) => {}
            }
        }
        Ok(())
    })();
    VALIDATION_SOURCE.with(|c| c.borrow_mut().clear());
    result
}

fn check_private_in_lhs(left: &Expr, scope: &[String]) -> Result<(), ParseError> {
    let Expr::StringLiteral { value, span } = left else {
        return Ok(());
    };
    let Some(pn) = value.strip_prefix('#') else {
        return Ok(());
    };
    let is_private = VALIDATION_SOURCE.with(|c| {
        c.borrow()
            .get(span.start..span.end)
            .is_some_and(|s| s.starts_with('#'))
    });
    if is_private && !scope.iter().any(|n| n == pn) {
        return Err(ParseError {
            span: *span,
            message: format!("Private name #{pn} is not declared in an enclosing class"),
        });
    }
    Ok(())
}

fn walk_export(export: &ExportDeclaration, scope: &[String]) -> Result<(), ParseError> {
    match export {
        ExportDeclaration::Declaration {
            decl_stmt: Some(stmt),
            ..
        } => walk_stmt(stmt, scope),
        ExportDeclaration::Default { body, .. } => match body {
            DefaultExportBody::Expression { expr } => walk_expr(expr, scope),
            DefaultExportBody::HoistableFunction { params, body, .. } => {
                walk_params(params, scope)?;
                walk_stmts(body, scope)
            }
            DefaultExportBody::Class {
                super_class,
                members,
                ..
            } => walk_class(super_class.as_ref(), members, scope),
        },
        _ => Ok(()),
    }
}

fn class_declared_privates(members: &[ClassMember]) -> Vec<String> {
    let mut names = Vec::new();
    for m in members {
        let n = match m {
            ClassMember::Method { name, .. } | ClassMember::Field { name, .. } => match name {
                ClassMemberName::Private { name, .. } => Some(name.clone()),
                _ => None,
            },
            ClassMember::StaticBlock { .. } => None,
        };
        if let Some(n) = n {
            if !names.contains(&n) {
                names.push(n);
            }
        }
    }
    names
}

fn walk_class(
    super_class: Option<&Expr>,
    members: &[ClassMember],
    scope: &[String],
) -> Result<(), ParseError> {

    if let Some(sc) = super_class {
        walk_expr(sc, scope)?;
    }

    let mut inner = scope.to_vec();
    for n in class_declared_privates(members) {
        if !inner.contains(&n) {
            inner.push(n);
        }
    }
    for m in members {
        match m {
            ClassMember::Method {
                name, params, body, ..
            } => {
                walk_member_name(name, &inner)?;
                walk_params(params, &inner)?;
                walk_stmts(body, &inner)?;
            }
            ClassMember::Field { name, init, .. } => {
                walk_member_name(name, &inner)?;
                if let Some(init) = init {
                    walk_expr(init, &inner)?;
                }
            }
            ClassMember::StaticBlock { body, .. } => walk_stmts(body, &inner)?,
        }
    }
    Ok(())
}

fn walk_member_name(name: &ClassMemberName, scope: &[String]) -> Result<(), ParseError> {
    if let ClassMemberName::Computed { expr, .. } = name {
        walk_expr(expr, scope)?;
    }
    Ok(())
}

fn walk_params(params: &[Parameter], scope: &[String]) -> Result<(), ParseError> {
    for p in params {
        if let Some(d) = &p.default {
            walk_expr(d, scope)?;
        }
    }
    Ok(())
}

fn walk_stmts(stmts: &[Stmt], scope: &[String]) -> Result<(), ParseError> {
    for s in stmts {
        walk_stmt(s, scope)?;
    }
    Ok(())
}

fn walk_stmt(stmt: &Stmt, scope: &[String]) -> Result<(), ParseError> {
    match stmt {
        Stmt::Expression { expr, .. } => walk_expr(expr, scope),
        Stmt::Variable(vs) => {
            for d in &vs.declarators {
                if let Some(init) = &d.init {
                    walk_expr(init, scope)?;
                }
            }
            Ok(())
        }
        Stmt::Block { body, .. } => walk_stmts(body, scope),
        Stmt::FunctionDecl { body, params, .. } => {
            walk_params(params, scope)?;
            walk_stmts(body, scope)
        }
        Stmt::ClassDecl {
            super_class,
            members,
            ..
        } => walk_class(super_class.as_ref(), members, scope),
        Stmt::If {
            test,
            consequent,
            alternate,
            ..
        } => {
            walk_expr(test, scope)?;
            walk_stmt(consequent, scope)?;
            if let Some(a) = alternate {
                walk_stmt(a, scope)?;
            }
            Ok(())
        }
        Stmt::For {
            init,
            test,
            update,
            body,
            ..
        } => {
            match init {
                Some(ForInit::Expression(e)) => walk_expr(e, scope)?,
                Some(ForInit::Variable(vs)) => {
                    for d in &vs.declarators {
                        if let Some(i) = &d.init {
                            walk_expr(i, scope)?;
                        }
                    }
                }
                None => {}
            }
            if let Some(t) = test {
                walk_expr(t, scope)?;
            }
            if let Some(u) = update {
                walk_expr(u, scope)?;
            }
            walk_stmt(body, scope)
        }
        Stmt::ForIn { right, body, .. } | Stmt::ForOf { right, body, .. } => {
            walk_expr(right, scope)?;
            walk_stmt(body, scope)
        }
        Stmt::While { test, body, .. } | Stmt::DoWhile { test, body, .. } => {
            walk_expr(test, scope)?;
            walk_stmt(body, scope)
        }
        Stmt::With { object, body, .. } => {
            walk_expr(object, scope)?;
            walk_stmt(body, scope)
        }
        Stmt::Switch {
            discriminant,
            cases,
            ..
        } => {
            walk_expr(discriminant, scope)?;
            for c in cases {
                if let Some(t) = &c.test {
                    walk_expr(t, scope)?;
                }
                walk_stmts(&c.consequent, scope)?;
            }
            Ok(())
        }
        Stmt::Try {
            block,
            handler,
            finalizer,
            ..
        } => {
            walk_stmt(block, scope)?;
            if let Some(h) = handler {
                walk_stmt(&h.body, scope)?;
            }
            if let Some(f) = finalizer {
                walk_stmt(f, scope)?;
            }
            Ok(())
        }
        Stmt::Return { argument, .. } => {
            if let Some(a) = argument {
                walk_expr(a, scope)?;
            }
            Ok(())
        }
        Stmt::Throw { argument, .. } => walk_expr(argument, scope),
        Stmt::Labelled { body, .. } => walk_stmt(body, scope),
        _ => Ok(()),
    }
}

fn walk_expr(expr: &Expr, scope: &[String]) -> Result<(), ParseError> {
    match expr {
        Expr::Member {
            object, property, ..
        } => {
            walk_expr(object, scope)?;
            if let MemberProperty::Private { name, span } = property.as_ref() {
                if !scope.iter().any(|n| n == name) {
                    return Err(ParseError {
                        span: *span,
                        message: format!(
                            "Private name #{name} is not declared in an enclosing class"
                        ),
                    });
                }
            }
            if let MemberProperty::Computed { expr, .. } = property.as_ref() {
                walk_expr(expr, scope)?;
            }
            Ok(())
        }
        Expr::Call {
            callee, arguments, ..
        }
        | Expr::New {
            callee, arguments, ..
        } => {
            walk_expr(callee, scope)?;
            walk_args(arguments, scope)
        }
        Expr::Parenthesized { expr, .. }
        | Expr::Update { argument: expr, .. }
        | Expr::Unary { argument: expr, .. } => walk_expr(expr, scope),
        Expr::Binary {
            operator,
            left,
            right,
            ..
        } => {
            if matches!(operator, rusty_js_ast::BinaryOp::In) {
                check_private_in_lhs(left, scope)?;
            }
            walk_expr(left, scope)?;
            walk_expr(right, scope)
        }
        Expr::Assign {
            target: left,
            value: right,
            ..
        } => {
            walk_expr(left, scope)?;
            walk_expr(right, scope)
        }
        Expr::Conditional {
            test,
            consequent,
            alternate,
            ..
        } => {
            walk_expr(test, scope)?;
            walk_expr(consequent, scope)?;
            walk_expr(alternate, scope)
        }
        Expr::Sequence { expressions, .. } | Expr::TemplateLiteral { expressions, .. } => {
            for e in expressions {
                walk_expr(e, scope)?;
            }
            Ok(())
        }
        Expr::Array { elements, .. } => {
            for el in elements {
                match el {
                    rusty_js_ast::ArrayElement::Expr(e)
                    | rusty_js_ast::ArrayElement::Spread { expr: e, .. } => walk_expr(e, scope)?,
                    rusty_js_ast::ArrayElement::Elision { .. } => {}
                }
            }
            Ok(())
        }
        Expr::Object { properties, .. } => {
            for p in properties {
                match p {
                    ObjectProperty::Property { value, .. } => walk_expr(value, scope)?,
                    ObjectProperty::Spread { expr, .. } => walk_expr(expr, scope)?,
                }
            }
            Ok(())
        }
        Expr::Function { params, body, .. } => {
            walk_params(params, scope)?;
            walk_stmts(body, scope)
        }
        Expr::Arrow { params, body, .. } => {
            walk_params(params, scope)?;
            match body {
                ArrowBody::Expression(e) => walk_expr(e, scope),
                ArrowBody::Block(stmts) => walk_stmts(stmts, scope),
            }
        }
        Expr::Class {
            super_class,
            members,
            ..
        } => walk_class(super_class.as_deref(), members, scope),
        _ => Ok(()),
    }
}

fn walk_args(args: &[Argument], scope: &[String]) -> Result<(), ParseError> {
    for a in args {
        match a {
            Argument::Expr(e) | Argument::Spread { expr: e, .. } => walk_expr(e, scope)?,
        }
    }
    Ok(())
}
