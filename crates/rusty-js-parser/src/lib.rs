
pub mod expr;
mod generated_unicode {
    pub(crate) mod property_escape_names;
}
pub mod lexer;
pub mod parser;
pub mod private_names_valid;
pub mod stmt;
pub mod token;

pub use lexer::{LexError, LexErrorKind, Lexer, LexerGoal};
pub use parser::{ParseError, ParseGoal, Parser};
pub use token::{NumberKind, Punct, Span, TemplatePart, Token, TokenKind};

pub const DEFAULT_MAX_SOURCE_BYTES: usize = 32 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectEvalStaticForm {
    NewTarget,
    SuperProperty,

    Arguments,
}

pub fn parse_module(src: &str) -> Result<rusty_js_ast::Module, ParseError> {
    validate_source_admission(src, DEFAULT_MAX_SOURCE_BYTES)?;
    let mut p = Parser::new_with_strict_and_goal(src, false, ParseGoal::Untagged)?;
    let module = p.parse_module()?;

    private_names_valid::validate_all_private_names(&module, src)?;
    Ok(module)
}

pub fn parse_script(src: &str) -> Result<rusty_js_ast::Module, ParseError> {
    validate_source_admission(src, DEFAULT_MAX_SOURCE_BYTES)?;
    let mut p = Parser::new_with_strict_and_goal(src, false, ParseGoal::Script)?;
    let module = p.parse_module()?;
    private_names_valid::validate_all_private_names(&module, src)?;
    Ok(module)
}

pub fn parse_module_goal(src: &str) -> Result<rusty_js_ast::Module, ParseError> {
    validate_source_admission(src, DEFAULT_MAX_SOURCE_BYTES)?;
    let mut p = Parser::new_with_strict_and_goal(src, false, ParseGoal::Module)?;
    let module = p.parse_module()?;
    private_names_valid::validate_all_private_names(&module, src)?;
    Ok(module)
}

pub fn parse_module_force_strict(src: &str) -> Result<rusty_js_ast::Module, ParseError> {
    validate_source_admission(src, DEFAULT_MAX_SOURCE_BYTES)?;
    let mut p = Parser::new_with_strict(src, true)?;
    p.parse_module()
}

pub fn direct_eval_static_form(
    src: &str,
    force_strict: bool,
) -> Result<Option<DirectEvalStaticForm>, ParseError> {
    validate_source_admission(src, DEFAULT_MAX_SOURCE_BYTES)?;
    let mut p = Parser::new_with_strict_and_goal(src, force_strict, ParseGoal::Script)?;
    let module = p.parse_module()?;
    Ok(module_items_direct_eval_static_form(&module.body))
}

fn validate_source_admission(src: &str, max_bytes: usize) -> Result<(), ParseError> {
    if src.len() > max_bytes {
        return Err(ParseError {
            span: Span::new(max_bytes, max_bytes.saturating_add(1)),
            message: format!("source input exceeds parser limit of {max_bytes} bytes"),
        });
    }
    validate_source_nesting(src)
}

fn validate_source_nesting(src: &str) -> Result<(), ParseError> {
    let bytes = src.as_bytes();
    let mut i = 0usize;
    let mut paren = 0usize;
    let mut brace = 0usize;
    let mut bracket = 0usize;
    while i < bytes.len() {
        match bytes[i] {
            b'\'' | b'"' | b'`' => {
                let quote = bytes[i];
                i += 1;
                while i < bytes.len() {
                    if bytes[i] == b'\\' {
                        i = (i + 2).min(bytes.len());
                        continue;
                    }
                    if bytes[i] == quote {
                        i += 1;
                        break;
                    }
                    i += 1;
                }
                continue;
            }
            b'/' if bytes.get(i + 1) == Some(&b'/') => {
                i += 2;
                while i < bytes.len() && !matches!(bytes[i], b'\n' | b'\r') {
                    i += 1;
                }
                continue;
            }
            b'/' if bytes.get(i + 1) == Some(&b'*') => {
                i += 2;
                while i + 1 < bytes.len() {
                    if bytes[i] == b'*' && bytes[i + 1] == b'/' {
                        i += 2;
                        break;
                    }
                    i += 1;
                }
                continue;
            }
            b'(' => {
                paren += 1;
                if paren > Parser::MAX_PARSE_DEPTH {
                    return Err(nesting_error(i));
                }
            }
            b')' => paren = paren.saturating_sub(1),
            b'{' => {
                brace += 1;
                if brace > Parser::MAX_PARSE_DEPTH {
                    return Err(nesting_error(i));
                }
            }
            b'}' => brace = brace.saturating_sub(1),
            b'[' => {
                bracket += 1;
                if bracket > Parser::MAX_PARSE_DEPTH {
                    return Err(nesting_error(i));
                }
            }
            b']' => bracket = bracket.saturating_sub(1),
            _ => {}
        }
        i += 1;
    }
    Ok(())
}

fn nesting_error(offset: usize) -> ParseError {
    ParseError {
        span: Span::new(offset, offset + 1),
        message: "parser nesting depth exceeded".into(),
    }
}

fn module_items_direct_eval_static_form(
    body: &[rusty_js_ast::ModuleItem],
) -> Option<DirectEvalStaticForm> {
    use rusty_js_ast::{
        Argument, ArrayElement, ArrowBody, ClassMember, ClassMemberName, DefaultExportBody,
        ExportDeclaration, Expr, ForInit, MemberProperty, ModuleItem, ObjectKey, ObjectProperty,
        Stmt,
    };

    fn expr_form(expr: &Expr) -> Option<DirectEvalStaticForm> {
        match expr {
            Expr::MetaProperty { meta, property, .. } if meta == "new" && property == "target" => {
                Some(DirectEvalStaticForm::NewTarget)
            }
            Expr::Member {
                object, property, ..
            } => {
                if matches!(object.as_ref(), Expr::Super { .. }) {
                    return Some(DirectEvalStaticForm::SuperProperty);
                }
                expr_form(object).or_else(|| match property.as_ref() {
                    MemberProperty::Computed { expr, .. } => expr_form(expr),
                    _ => None,
                })
            }
            Expr::Call {
                callee, arguments, ..
            }
            | Expr::New {
                callee, arguments, ..
            } => {
                if matches!(callee.as_ref(), Expr::Super { .. }) {
                    return Some(DirectEvalStaticForm::SuperProperty);
                }
                expr_form(callee).or_else(|| {
                    arguments.iter().find_map(|arg| match arg {
                        Argument::Expr(e) | Argument::Spread { expr: e, .. } => expr_form(e),
                    })
                })
            }
            Expr::Parenthesized { expr, .. }
            | Expr::Update { argument: expr, .. }
            | Expr::Unary { argument: expr, .. } => expr_form(expr),
            Expr::Binary { left, right, .. }
            | Expr::Assign {
                target: left,
                value: right,
                ..
            } => expr_form(left).or_else(|| expr_form(right)),
            Expr::Conditional {
                test,
                consequent,
                alternate,
                ..
            } => expr_form(test)
                .or_else(|| expr_form(consequent))
                .or_else(|| expr_form(alternate)),
            Expr::Sequence { expressions, .. } | Expr::TemplateLiteral { expressions, .. } => {
                expressions.iter().find_map(expr_form)
            }
            Expr::Array { elements, .. } => elements.iter().find_map(|el| match el {
                ArrayElement::Expr(e) | ArrayElement::Spread { expr: e, .. } => expr_form(e),
                ArrayElement::Elision { .. } => None,
            }),
            Expr::Object { properties, .. } => properties.iter().find_map(|prop| match prop {
                ObjectProperty::Property { key, value, .. } => {
                    let key_hit = match key {
                        ObjectKey::Computed { expr, .. } => expr_form(expr),
                        _ => None,
                    };
                    key_hit.or_else(|| expr_form(value))
                }
                ObjectProperty::Spread { expr, .. } => expr_form(expr),
            }),
            Expr::Class {
                super_class,
                members,
                ..
            } => super_class
                .as_deref()
                .and_then(expr_form)
                .or_else(|| class_members_form(members)),

            Expr::Identifier { name, .. } if name == "arguments" => {
                Some(DirectEvalStaticForm::Arguments)
            }

            Expr::Function { .. } => None,
            Expr::Arrow { body, .. } => match body {
                ArrowBody::Expression(e) => expr_form(e),
                ArrowBody::Block(stmts) => stmts_form(stmts),
            },
            _ => None,
        }
    }

    fn class_members_form(members: &[ClassMember]) -> Option<DirectEvalStaticForm> {

        members.iter().find_map(|member| match member {
            ClassMember::Field { name, .. } | ClassMember::Method { name, .. } => match name {
                ClassMemberName::Computed { expr, .. } => expr_form(expr),
                _ => None,
            },
            ClassMember::StaticBlock { .. } => None,
        })
    }

    fn stmt_form(stmt: &Stmt) -> Option<DirectEvalStaticForm> {
        match stmt {
            Stmt::Expression { expr, .. } => expr_form(expr),
            Stmt::Variable(vs) => vs
                .declarators
                .iter()
                .filter_map(|d| d.init.as_ref())
                .find_map(expr_form),
            Stmt::Block { body, .. } => stmts_form(body),
            Stmt::ClassDecl {
                super_class,
                members,
                ..
            } => super_class
                .as_ref()
                .and_then(expr_form)
                .or_else(|| class_members_form(members)),
            Stmt::If {
                test,
                consequent,
                alternate,
                ..
            } => expr_form(test)
                .or_else(|| stmt_form(consequent))
                .or_else(|| alternate.as_deref().and_then(stmt_form)),
            Stmt::For {
                init,
                test,
                update,
                body,
                ..
            } => init
                .as_ref()
                .and_then(|init| match init {
                    ForInit::Expression(e) => expr_form(e),
                    ForInit::Variable(vs) => vs
                        .declarators
                        .iter()
                        .filter_map(|d| d.init.as_ref())
                        .find_map(expr_form),
                })
                .or_else(|| test.as_ref().and_then(expr_form))
                .or_else(|| update.as_ref().and_then(expr_form))
                .or_else(|| stmt_form(body)),
            Stmt::ForIn { right, body, .. } | Stmt::ForOf { right, body, .. } => {
                expr_form(right).or_else(|| stmt_form(body))
            }
            Stmt::While { test, body, .. } | Stmt::DoWhile { test, body, .. } => {
                expr_form(test).or_else(|| stmt_form(body))
            }
            Stmt::With { object, body, .. } => expr_form(object).or_else(|| stmt_form(body)),
            Stmt::Switch {
                discriminant,
                cases,
                ..
            } => expr_form(discriminant).or_else(|| {
                cases.iter().find_map(|case| {
                    case.test
                        .as_ref()
                        .and_then(expr_form)
                        .or_else(|| stmts_form(&case.consequent))
                })
            }),
            Stmt::Try {
                block,
                handler,
                finalizer,
                ..
            } => stmt_form(block)
                .or_else(|| {
                    handler
                        .as_ref()
                        .and_then(|handler| stmt_form(&handler.body))
                })
                .or_else(|| finalizer.as_deref().and_then(stmt_form)),
            Stmt::Return { argument, .. } => argument.as_ref().and_then(expr_form),
            Stmt::Throw { argument, .. } => expr_form(argument),
            Stmt::Labelled { body, .. } => stmt_form(body),
            Stmt::FunctionDecl { .. } => None,
            _ => None,
        }
    }

    fn stmts_form(stmts: &[Stmt]) -> Option<DirectEvalStaticForm> {
        stmts.iter().find_map(stmt_form)
    }

    body.iter().find_map(|item| match item {
        ModuleItem::Statement(stmt) => stmt_form(stmt),
        ModuleItem::Export(ExportDeclaration::Declaration {
            decl_stmt: Some(stmt),
            ..
        }) => stmt_form(stmt),
        ModuleItem::Export(ExportDeclaration::Default { body, .. }) => match body {
            DefaultExportBody::Expression { expr } => expr_form(expr),
            DefaultExportBody::Class {
                super_class,
                members,
                ..
            } => super_class
                .as_ref()
                .and_then(expr_form)
                .or_else(|| class_members_form(members)),
            DefaultExportBody::HoistableFunction { .. } => None,
        },
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use super::{parse_module_force_strict, parse_script, DEFAULT_MAX_SOURCE_BYTES};

    #[test]
    fn sloppy_yield_identifier_keeps_following_slash_as_division() {
        parse_script(
            "var yield = 12, a = 3, b = 6, g = 2;\n\
             yield /a; yieldParsedAsIdentifier = true; b/g;",
        )
        .expect("sloppy script yield identifier before / should parse as division");
    }

    #[test]
    fn module_leading_await_regexp_lexes_as_regexp() {

        super::parse_module_goal("await /1/;")
            .expect("leading top-level-await regexp operand should parse");
    }

    #[test]
    fn strict_yield_binding_still_rejects() {
        let err = parse_module_force_strict("\"use strict\";\nvar yield = 1;")
            .expect_err("strict yield binding must stay rejected");
        assert!(err.message.contains("yield"));
    }

    #[test]
    fn oversized_source_is_rejected_before_parse() {
        let src = " ".repeat(DEFAULT_MAX_SOURCE_BYTES + 1);
        let err = parse_script(&src).expect_err("oversized source must reject");
        assert!(err.message.contains("parser limit"));
    }
}
