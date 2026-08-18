
use crate::lexer::LexerGoal;
use crate::parser::{ParseError, Parser};
use crate::token::{Punct, TokenKind};
use rusty_js_ast::{
    Argument, ArrayElement, ArrayPattern, AssignOp, BindingElement, BindingIdentifier,
    BindingPattern, CatchClause, Expr, ForBinding, ForInit, ObjectKey, ObjectPattern,
    ObjectPatternProperty, ObjectProperty, PropertyKey, Span, Stmt, SwitchCase, UnaryOp,
    VariableDeclarator, VariableKind, VariableStatement,
};

fn is_using_kind(k: VariableKind) -> bool {
    matches!(k, VariableKind::Using | VariableKind::AwaitUsing)
}

fn using_dispose_stmt(name: &str, is_async: bool, sp: Span) -> Stmt {
    let call = Expr::Call {
        callee: Box::new(Expr::Identifier {
            name: "__dispose_resource".into(),
            span: sp,
        }),
        arguments: vec![
            Argument::Expr(Expr::Identifier {
                name: name.to_string(),
                span: sp,
            }),
            Argument::Expr(Expr::BoolLiteral {
                value: is_async,
                span: sp,
            }),
        ],
        optional: false,
        span: sp,
    };
    let expr = if is_async {
        Expr::Unary {
            operator: UnaryOp::Await,
            argument: Box::new(call),
            span: sp,
        }
    } else {
        call
    };
    using_var(
        &format!("__using_dispose_result_{}_{}", sp.start, name),
        Some(expr),
        sp,
    )
}

fn using_ident(name: &str, sp: Span) -> Expr {
    Expr::Identifier {
        name: name.to_string(),
        span: sp,
    }
}

fn using_binding(name: &str, sp: Span) -> BindingPattern {
    BindingPattern::Identifier(BindingIdentifier {
        name: name.to_string(),
        span: sp,
    })
}

fn using_var(name: &str, init: Option<Expr>, sp: Span) -> Stmt {
    Stmt::Variable(VariableStatement {
        kind: VariableKind::Let,
        declarators: vec![VariableDeclarator {
            target: using_binding(name, sp),
            init,
            span: sp,
        }],
        span: sp,
    })
}

fn using_assign_stmt(name: &str, value: Expr, sp: Span) -> Stmt {
    Stmt::Expression {
        expr: Expr::Assign {
            operator: AssignOp::Assign,
            target: Box::new(using_ident(name, sp)),
            value: Box::new(value),
            span: sp,
        },
        span: sp,
    }
}

fn using_suppressed_error_expr(error: Expr, suppressed: Expr, sp: Span) -> Expr {
    Expr::Call {
        callee: Box::new(using_ident("__suppressed_error", sp)),
        arguments: vec![Argument::Expr(error), Argument::Expr(suppressed)],
        optional: false,
        span: sp,
    }
}

fn using_dispose_with_suppression_stmt(
    name: &str,
    is_async: bool,
    has_error_name: &str,
    error_name: &str,
    sp: Span,
) -> Stmt {
    let dispose_error_name = format!("__using_dispose_error_{}_{}", sp.start, name);
    let throw_suppressed = Stmt::If {
        test: using_ident(has_error_name, sp),
        consequent: Box::new(Stmt::Throw {
            argument: using_suppressed_error_expr(
                using_ident(&dispose_error_name, sp),
                using_ident(error_name, sp),
                sp,
            ),
            span: sp,
        }),
        alternate: None,
        span: sp,
    };
    Stmt::Try {
        disposal: true,
        block: Box::new(Stmt::Block {
            body: vec![using_dispose_stmt(name, is_async, sp)],
            span: sp,
        }),
        handler: Some(CatchClause {
            param: Some(using_binding(&dispose_error_name, sp)),
            body: Box::new(Stmt::Block {
                body: vec![
                    throw_suppressed,
                    Stmt::Throw {
                        argument: using_ident(&dispose_error_name, sp),
                        span: sp,
                    },
                ],
                span: sp,
            }),
            span: sp,
        }),
        finalizer: None,
        span: sp,
    }
}

fn using_scope_try(body: Vec<Stmt>, finalizer_body: Vec<Stmt>, sp: Span, tag: &str) -> Vec<Stmt> {
    let has_error_name = format!("__using_has_error_{}_{}", sp.start, tag);
    let error_name = format!("__using_error_{}_{}", sp.start, tag);
    let caught_name = format!("__using_caught_{}_{}", sp.start, tag);

    vec![
        using_var(
            &has_error_name,
            Some(Expr::BoolLiteral {
                value: false,
                span: sp,
            }),
            sp,
        ),
        using_var(&error_name, None, sp),
        Stmt::Try {
            disposal: true,
            block: Box::new(Stmt::Block { body, span: sp }),
            handler: Some(CatchClause {
                param: Some(using_binding(&caught_name, sp)),
                body: Box::new(Stmt::Block {
                    body: vec![
                        using_assign_stmt(
                            &has_error_name,
                            Expr::BoolLiteral {
                                value: true,
                                span: sp,
                            },
                            sp,
                        ),
                        using_assign_stmt(&error_name, using_ident(&caught_name, sp), sp),
                    ],
                    span: sp,
                }),
                span: sp,
            }),
            finalizer: Some(Box::new(Stmt::Block {
                body: {
                    let mut body = finalizer_body;
                    body.push(Stmt::If {
                        test: using_ident(&has_error_name, sp),
                        consequent: Box::new(Stmt::Throw {
                            argument: using_ident(&error_name, sp),
                            span: sp,
                        }),
                        alternate: None,
                        span: sp,
                    });
                    body
                },
                span: sp,
            })),
            span: sp,
        },
    ]
}

pub(crate) fn desugar_using_block(body: Vec<Stmt>) -> Vec<Stmt> {
    let pos = body
        .iter()
        .position(|s| matches!(s, Stmt::Variable(v) if is_using_kind(v.kind)));
    let Some(i) = pos else {
        return body;
    };
    let using_vs = match &body[i] {
        Stmt::Variable(v) => v.clone(),
        _ => unreachable!(),
    };
    let is_async = matches!(using_vs.kind, VariableKind::AwaitUsing);
    let sp = using_vs.span;

    if using_vs.declarators.len() > 1 {
        let mut expanded: Vec<Stmt> = body[..i].to_vec();
        for declarator in using_vs.declarators {
            let span = declarator.span;
            expanded.push(Stmt::Variable(VariableStatement {
                kind: using_vs.kind,
                declarators: vec![declarator],
                span,
            }));
        }
        expanded.extend_from_slice(&body[i + 1..]);
        return desugar_using_block(expanded);
    }

    let mut out: Vec<Stmt> = body[..i].to_vec();

    let const_vs = VariableStatement {
        kind: VariableKind::Const,
        declarators: using_vs.declarators.clone(),
        span: sp,
    };
    out.push(Stmt::Variable(const_vs));

    let names: Vec<String> = using_vs
        .declarators
        .iter()
        .filter_map(|d| match &d.target {
            BindingPattern::Identifier(id) => Some(id.name.clone()),
            _ => None,
        })
        .collect();

    let rest = desugar_using_block(body[i + 1..].to_vec());
    let has_error_name = format!("__using_has_error_{}_block", sp.start);
    let error_name = format!("__using_error_{}_block", sp.start);
    let finalizer_body: Vec<Stmt> = names
        .iter()
        .rev()
        .map(|n| using_dispose_with_suppression_stmt(n, is_async, &has_error_name, &error_name, sp))
        .collect();
    out.extend(using_scope_try(rest, finalizer_body, sp, "block"));
    out
}

fn desugar_using_for_statement(
    init_vs: VariableStatement,
    test: Option<Expr>,
    update: Option<Expr>,
    body: Stmt,
    span: Span,
) -> Stmt {
    if !is_using_kind(init_vs.kind) {
        return Stmt::For {
            init: Some(ForInit::Variable(init_vs)),
            test,
            update,
            body: Box::new(body),
            span,
        };
    }

    let for_stmt = Stmt::For {
        init: None,
        test,
        update,
        body: Box::new(body),
        span,
    };
    Stmt::Block {
        body: desugar_using_block(vec![Stmt::Variable(init_vs), for_stmt]),
        span,
    }
}

fn desugar_using_for_of_statement(
    left: ForBinding,
    right: Expr,
    body: Stmt,
    await_: bool,
    span: Span,
) -> Stmt {
    let ForBinding::Decl {
        kind,
        target,
        span: left_span,
    } = left
    else {
        return Stmt::ForOf {
            left,
            right,
            body: Box::new(body),
            await_,
            span,
        };
    };
    if !is_using_kind(kind) {
        return Stmt::ForOf {
            left: ForBinding::Decl {
                kind,
                target,
                span: left_span,
            },
            right,
            body: Box::new(body),
            await_,
            span,
        };
    }

    let is_async = matches!(kind, VariableKind::AwaitUsing);
    let names: Vec<String> = match &target {
        BindingPattern::Identifier(id) => vec![id.name.clone()],
        _ => target
            .collect_names()
            .iter()
            .map(|id| id.name.clone())
            .collect(),
    };
    let has_error_name = format!("__using_has_error_{}_forof", left_span.start);
    let error_name = format!("__using_error_{}_forof", left_span.start);
    let finalizer_body: Vec<Stmt> = names
        .iter()
        .rev()
        .map(|name| {
            using_dispose_with_suppression_stmt(
                name,
                is_async,
                &has_error_name,
                &error_name,
                left_span,
            )
        })
        .collect();
    let wrapped_body = Stmt::Block {
        body: using_scope_try(vec![body], finalizer_body, left_span, "forof"),
        span,
    };

    Stmt::ForOf {
        left: ForBinding::Decl {
            kind: VariableKind::Const,
            target,
            span: left_span,
        },
        right,
        body: Box::new(wrapped_body),
        await_,
        span,
    }
}

fn expr_to_binding_pattern(e: Expr) -> Option<BindingPattern> {
    match e {
        Expr::Identifier { name, span } => {
            Some(BindingPattern::Identifier(BindingIdentifier { name, span }))
        }
        Expr::Array {
            elements,
            trailing_comma_after_spread,
            span,
        } => {

            if trailing_comma_after_spread {
                return None;
            }
            let mut out: Vec<Option<BindingElement>> = Vec::with_capacity(elements.len());
            let mut rest: Option<Box<BindingPattern>> = None;
            let n = elements.len();
            for (i, el) in elements.into_iter().enumerate() {
                match el {
                    ArrayElement::Elision { .. } => out.push(None),
                    ArrayElement::Expr(inner) => {
                        let (target_expr, default) = match inner {
                            Expr::Assign {
                                operator: rusty_js_ast::AssignOp::Assign,
                                target,
                                value,
                                ..
                            } => (*target, Some(*value)),
                            other => (other, None),
                        };
                        let span = target_expr.span();
                        let target = expr_to_binding_pattern(target_expr)?;
                        out.push(Some(BindingElement {
                            target,
                            default,
                            span,
                        }));
                    }
                    ArrayElement::Spread { expr, .. } => {

                        if i + 1 != n {
                            return None;
                        }
                        rest = Some(Box::new(expr_to_binding_pattern(expr)?));
                    }
                }
            }
            Some(BindingPattern::Array(ArrayPattern {
                elements: out,
                rest,
                span,
            }))
        }
        Expr::Object {
            properties,
            span,
            trailing_comma_after_spread,
        } => {

            if trailing_comma_after_spread {
                return None;
            }
            let mut props: Vec<ObjectPatternProperty> = Vec::with_capacity(properties.len());
            let mut rest: Option<Box<BindingIdentifier>> = None;
            let n = properties.len();
            for (i, p) in properties.into_iter().enumerate() {
                match p {
                    ObjectProperty::Property {
                        key,
                        value,
                        shorthand,
                        kind: _,
                        span: pspan,
                    } => {
                        let pk = match key {
                            ObjectKey::Identifier { name, span } => {
                                PropertyKey::Identifier(BindingIdentifier { name, span })
                            }
                            ObjectKey::String { value, .. } => {
                                PropertyKey::String(std::rc::Rc::new(value))
                            }
                            ObjectKey::Number { value, .. } => PropertyKey::Number(value),
                            ObjectKey::Computed { expr, .. } => PropertyKey::Computed(expr),
                        };
                        let (target_expr, default) = match value {
                            Expr::Assign {
                                operator: rusty_js_ast::AssignOp::Assign,
                                target,
                                value,
                                ..
                            } => (*target, Some(*value)),
                            other => (other, None),
                        };
                        let target = expr_to_binding_pattern(target_expr)?;
                        props.push(ObjectPatternProperty {
                            key: pk,
                            value: BindingElement {
                                target,
                                default,
                                span: pspan,
                            },
                            shorthand,
                            span: pspan,
                        });
                    }
                    ObjectProperty::Spread { expr, .. } => {
                        if i + 1 != n {
                            return None;
                        }
                        if let Expr::Identifier { name, span } = expr {
                            rest = Some(Box::new(BindingIdentifier { name, span }));
                        } else {
                            return None;
                        }
                    }
                }
            }
            Some(BindingPattern::Object(ObjectPattern {
                properties: props,
                rest,
                span,
            }))
        }
        _ => None,
    }
}

fn is_valid_for_assignment_target(e: &Expr) -> bool {
    match e {
        Expr::Identifier { .. } => true,
        Expr::Member { optional, .. } => !optional,
        Expr::Parenthesized { expr, .. } => is_valid_for_assignment_target(expr),
        _ => false,
    }
}

fn is_valid_assignment_pattern_expr(e: &Expr) -> bool {
    match e {
        Expr::Identifier { .. } => true,
        Expr::Member { optional, .. } => !optional,
        Expr::Parenthesized { expr, .. } => is_valid_assignment_pattern_expr(expr),
        Expr::Array {
            elements,
            trailing_comma_after_spread,
            ..
        } => {
            if *trailing_comma_after_spread {
                return false;
            }
            let n = elements.len();
            elements.iter().enumerate().all(|(i, el)| match el {
                ArrayElement::Elision { .. } => true,
                ArrayElement::Expr(expr) => match expr {
                    Expr::Assign {
                        operator: rusty_js_ast::AssignOp::Assign,
                        target,
                        ..
                    } => is_valid_assignment_pattern_expr(target),
                    other => is_valid_assignment_pattern_expr(other),
                },
                ArrayElement::Spread { expr, .. } => {
                    i + 1 == n && is_valid_assignment_pattern_expr(expr)
                }
            })
        }
        Expr::Object {
            properties,
            trailing_comma_after_spread,
            ..
        } => {

            if *trailing_comma_after_spread {
                return false;
            }
            let n = properties.len();
            properties.iter().enumerate().all(|(i, prop)| match prop {
                ObjectProperty::Property { value, .. } => match value {
                    Expr::Assign {
                        operator: rusty_js_ast::AssignOp::Assign,
                        target,
                        ..
                    } => is_valid_assignment_pattern_expr(target),
                    other => is_valid_assignment_pattern_expr(other),
                },
                ObjectProperty::Spread { expr, .. } => {
                    i + 1 == n && is_valid_for_assignment_target(expr)
                }
            })
        }
        _ => false,
    }
}

impl<'src> Parser<'src> {
    fn parse_assignment_expression_no_in(&mut self) -> Result<Expr, ParseError> {
        let saved_in_disallowed = self.in_disallowed;
        self.in_disallowed = true;
        let result = self.parse_assignment_expression();
        self.in_disallowed = saved_in_disallowed;
        result
    }

    fn parse_expression_no_in(&mut self) -> Result<Expr, ParseError> {
        let saved_in_disallowed = self.in_disallowed;
        let saved_allow_cover = self.allow_cover_initialized_name_in_for_head;
        self.in_disallowed = true;
        self.allow_cover_initialized_name_in_for_head = true;
        let result = self.parse_expression();
        self.in_disallowed = saved_in_disallowed;
        self.allow_cover_initialized_name_in_for_head = saved_allow_cover;
        result
    }

    pub fn parse_substatement(&mut self) -> Result<Stmt, ParseError> {

        if self.is_ident("const") {
            return Err(
                self.err_here("LexicalDeclaration `const` is not allowed as Statement body".into())
            );
        }
        if self.is_contextual_keyword("let") {

            let pos = self.lookahead_span().end;
            let bytes = self.source().as_bytes();
            let (p, saw_lt) = Self::skip_ws_and_comments_track_lt(bytes, pos);
            if p < bytes.len() {
                let b = bytes[p];
                if b == b'[' || (!saw_lt && Self::byte_can_start_binding_list(b)) {
                    return Err(self.err_here(
                        "LexicalDeclaration `let` is not allowed as Statement body".into(),
                    ));
                }
            }
        }

        if self.is_ident("using") && self.using_starts_declaration(self.lookahead_span().end) {
            return Err(
                self.err_here("`using` declaration is not allowed as Statement body".into())
            );
        }
        if self.in_async && self.is_ident("await") && self.await_using_starts_declaration() {
            return Err(
                self.err_here("`await using` declaration is not allowed as Statement body".into())
            );
        }

        if self.is_ident("class") {
            return Err(self.err_here("ClassDeclaration is not allowed as Statement body".into()));
        }

        if self.is_ident("function") {
            if self.allow_annex_b_function_in_substatement && !self.strict_mode {

                let pos = self.lookahead_span().end;
                let bytes = self.source().as_bytes();
                let mut p = pos;
                while p < bytes.len() && bytes[p].is_ascii_whitespace() {
                    p += 1;
                }
                if p >= bytes.len() || bytes[p] != b'*' {
                    return self.parse_statement();
                }
            }
            return Err(
                self.err_here("HoistableDeclaration is not allowed as Statement body".into())
            );
        }
        if self.is_ident("async") {
            let pos = self.lookahead_span().end;
            let bytes = self.source().as_bytes();
            let mut p = pos;
            while p < bytes.len() && bytes[p].is_ascii_whitespace() {
                p += 1;
            }
            if Self::bytes_at_identifier_keyword(bytes, p, "function") {
                return Err(self
                    .err_here("AsyncFunctionDeclaration is not allowed as Statement body".into()));
            }
        }
        if matches!(self.current_kind(), TokenKind::Punct(Punct::At)) {
            return Err(
                self.err_here("Decorated ClassDeclaration is not allowed as Statement body".into())
            );
        }
        self.parse_statement()
    }

    pub fn parse_statement(&mut self) -> Result<Stmt, ParseError> {
        let _profile =
            crate::parser::parse_profile::Guard::new(crate::parser::parse_profile::Kind::Statement);
        let start = self.lookahead_span().start;

        if self.is_ident("import") && !self.is_dynamic_import_call_after_import() {
            if self.next_token_text_is("typeof") {
                return Err(self.err_here("Unexpected token 'typeof'".into()));
            }
            return Err(self.err_here(
                "import declarations may only appear at the top level of a module".into(),
            ));
        }
        if self.is_ident("export") {
            return Err(self.err_here(
                "export declarations may only appear at the top level of a module".into(),
            ));
        }

        if self.is_ident("var")
            || self.is_ident("const")
            || (self.is_contextual_keyword("let") && self.let_starts_lexical_declaration())
        {
            let _stmt_profile = crate::parser::parse_profile::Guard::new(
                crate::parser::parse_profile::Kind::StmtVar,
            );
            let _nested_profile = crate::parser::parse_profile::nested_statement_guard(
                crate::parser::parse_profile::Kind::NestedStmtVar,
            );
            let _nested_depth_profile = crate::parser::parse_profile::nested_statement_depth_guard(
                crate::parser::parse_profile::StatementFamily::Var,
            );
            let v = self.parse_variable_statement()?;
            return Ok(Stmt::Variable(v));
        }

        if self.is_ident("using") && self.using_starts_declaration(self.lookahead_span().end) {
            let _stmt_profile = crate::parser::parse_profile::Guard::new(
                crate::parser::parse_profile::Kind::StmtVar,
            );
            let _nested_profile = crate::parser::parse_profile::nested_statement_guard(
                crate::parser::parse_profile::Kind::NestedStmtVar,
            );
            let _nested_depth_profile = crate::parser::parse_profile::nested_statement_depth_guard(
                crate::parser::parse_profile::StatementFamily::Var,
            );
            let v = self.parse_using_statement(false)?;
            return Ok(Stmt::Variable(v));
        }
        if self.is_ident("await")
            && (self.in_async
                || (self.function_body_depth == 0 && self.goal_allows_top_level_await()))
            && self.await_using_starts_declaration()
        {
            let _stmt_profile = crate::parser::parse_profile::Guard::new(
                crate::parser::parse_profile::Kind::StmtVar,
            );
            let _nested_profile = crate::parser::parse_profile::nested_statement_guard(
                crate::parser::parse_profile::Kind::NestedStmtVar,
            );
            let _nested_depth_profile = crate::parser::parse_profile::nested_statement_depth_guard(
                crate::parser::parse_profile::StatementFamily::Var,
            );
            let v = self.parse_using_statement(true)?;
            return Ok(Stmt::Variable(v));
        }

        if self.is_ident("function") {
            let _stmt_profile = crate::parser::parse_profile::Guard::new(
                crate::parser::parse_profile::Kind::StmtFunction,
            );
            let _nested_profile = crate::parser::parse_profile::nested_statement_guard(
                crate::parser::parse_profile::Kind::NestedStmtFunction,
            );
            let _nested_depth_profile = crate::parser::parse_profile::nested_statement_depth_guard(
                crate::parser::parse_profile::StatementFamily::Function,
            );
            let _top_family_profile =
                crate::parser::parse_profile::FunctionBodyTopFunctionFamilyGuard::new(
                    crate::parser::parse_profile::FunctionFamilyKind::Function,
                );
            return self.parse_function_decl_stmt(false, None);
        }
        if self.is_ident("async") {

            let async_start = self.lookahead_span().start;
            let pos = self.lookahead_span().end;
            let bytes = self.source().as_bytes();
            if let Some(p) = Self::skip_ws_and_comments_no_lt(bytes, pos) {
                if Self::bytes_at_identifier_keyword(bytes, p, "function") {
                    self.reject_escaped_contextual_keyword("async")?;
                    self.bump()?;
                    let _stmt_profile = crate::parser::parse_profile::Guard::new(
                        crate::parser::parse_profile::Kind::StmtFunction,
                    );
                    let _nested_profile = crate::parser::parse_profile::nested_statement_guard(
                        crate::parser::parse_profile::Kind::NestedStmtFunction,
                    );
                    let _nested_depth_profile =
                        crate::parser::parse_profile::nested_statement_depth_guard(
                            crate::parser::parse_profile::StatementFamily::Function,
                        );
                    let _top_family_profile =
                        crate::parser::parse_profile::FunctionBodyTopFunctionFamilyGuard::new(
                            crate::parser::parse_profile::FunctionFamilyKind::AsyncFunction,
                        );
                    return self.parse_function_decl_stmt(true, Some(async_start));
                }
            }
        }

        if self.is_ident("class") {
            let _stmt_profile = crate::parser::parse_profile::Guard::new(
                crate::parser::parse_profile::Kind::StmtFunction,
            );
            let _nested_profile = crate::parser::parse_profile::nested_statement_guard(
                crate::parser::parse_profile::Kind::NestedStmtFunction,
            );
            let _nested_depth_profile = crate::parser::parse_profile::nested_statement_depth_guard(
                crate::parser::parse_profile::StatementFamily::Function,
            );
            let _top_family_profile =
                crate::parser::parse_profile::FunctionBodyTopFunctionFamilyGuard::new(
                    crate::parser::parse_profile::FunctionFamilyKind::Class,
                );
            return self.parse_class_decl_stmt();
        }
        if matches!(self.current_kind(), TokenKind::Punct(Punct::At)) {
            let decorators = self.parse_class_decorators()?;
            if !self.is_ident("class") {
                return Err(
                    self.err_here("decorators are only supported on class declarations".into())
                );
            }
            let _stmt_profile = crate::parser::parse_profile::Guard::new(
                crate::parser::parse_profile::Kind::StmtFunction,
            );
            let _nested_profile = crate::parser::parse_profile::nested_statement_guard(
                crate::parser::parse_profile::Kind::NestedStmtFunction,
            );
            let _nested_depth_profile = crate::parser::parse_profile::nested_statement_depth_guard(
                crate::parser::parse_profile::StatementFamily::Function,
            );
            let _top_family_profile =
                crate::parser::parse_profile::FunctionBodyTopFunctionFamilyGuard::new(
                    crate::parser::parse_profile::FunctionFamilyKind::DecoratedClass,
                );
            return self.parse_class_decl_stmt_with_decorators(decorators);
        }

        if matches!(self.current_kind(), TokenKind::Punct(Punct::LBrace)) {
            let _stmt_profile = crate::parser::parse_profile::Guard::new(
                crate::parser::parse_profile::Kind::StmtBlock,
            );
            let _nested_profile = crate::parser::parse_profile::nested_statement_guard(
                crate::parser::parse_profile::Kind::NestedStmtBlock,
            );
            let _nested_depth_profile = crate::parser::parse_profile::nested_statement_depth_guard(
                crate::parser::parse_profile::StatementFamily::Block,
            );
            return self.parse_block_statement();
        }

        if matches!(self.current_kind(), TokenKind::Punct(Punct::Semicolon)) {
            let span = self.lookahead_span();
            self.bump()?;
            return Ok(Stmt::Empty { span });
        }

        if self.is_ident("if") {
            let _stmt_profile = crate::parser::parse_profile::Guard::new(
                crate::parser::parse_profile::Kind::StmtControl,
            );
            let _nested_profile = crate::parser::parse_profile::nested_statement_guard(
                crate::parser::parse_profile::Kind::NestedStmtControl,
            );
            let _nested_depth_profile = crate::parser::parse_profile::nested_statement_depth_guard(
                crate::parser::parse_profile::StatementFamily::Control,
            );
            let _nested_control_profile = crate::parser::parse_profile::nested_statement_guard(
                crate::parser::parse_profile::Kind::NestedControlIf,
            );
            let _top_class_method_body_control =
                crate::parser::parse_profile::FunctionBodyTopClassMethodBodyControlFamilyGuard::new(
                    crate::parser::parse_profile::ControlFamily::If,
                );
            let _scoped_class_method_body_control =
                crate::parser::parse_profile::TopVarInitIdentArrowExprCondClassMethodBodyControlFamilyGuard::new(
                    crate::parser::parse_profile::ControlFamily::If,
                );
            let _branch_block_control =
                crate::parser::parse_profile::nested_if_branch_block_control_guard(
                    crate::parser::parse_profile::ControlFamily::If,
                );
            let _top_class_method_body_if_branch_block_control =
                crate::parser::parse_profile::function_body_top_class_method_body_if_branch_block_control_guard(
                    crate::parser::parse_profile::ControlFamily::If,
                );
            let _branch_block_if_consequent_block_control = crate::parser::parse_profile::nested_if_branch_block_if_consequent_block_control_guard(crate::parser::parse_profile::ControlFamily::If);
            return self.parse_if_statement();
        }
        if self.is_ident("for") {
            let _stmt_profile = crate::parser::parse_profile::Guard::new(
                crate::parser::parse_profile::Kind::StmtControl,
            );
            let _nested_profile = crate::parser::parse_profile::nested_statement_guard(
                crate::parser::parse_profile::Kind::NestedStmtControl,
            );
            let _nested_depth_profile = crate::parser::parse_profile::nested_statement_depth_guard(
                crate::parser::parse_profile::StatementFamily::Control,
            );
            let _nested_control_profile = crate::parser::parse_profile::nested_statement_guard(
                crate::parser::parse_profile::Kind::NestedControlFor,
            );
            let _top_class_method_body_control =
                crate::parser::parse_profile::FunctionBodyTopClassMethodBodyControlFamilyGuard::new(
                    crate::parser::parse_profile::ControlFamily::For,
                );
            let _scoped_class_method_body_control =
                crate::parser::parse_profile::TopVarInitIdentArrowExprCondClassMethodBodyControlFamilyGuard::new(
                    crate::parser::parse_profile::ControlFamily::For,
                );
            let _branch_block_control =
                crate::parser::parse_profile::nested_if_branch_block_control_guard(
                    crate::parser::parse_profile::ControlFamily::For,
                );
            let _top_class_method_body_if_branch_block_control =
                crate::parser::parse_profile::function_body_top_class_method_body_if_branch_block_control_guard(
                    crate::parser::parse_profile::ControlFamily::For,
                );
            let _branch_block_if_consequent_block_control = crate::parser::parse_profile::nested_if_branch_block_if_consequent_block_control_guard(crate::parser::parse_profile::ControlFamily::For);
            return self.parse_for_statement();
        }
        if self.is_ident("while") {
            let _stmt_profile = crate::parser::parse_profile::Guard::new(
                crate::parser::parse_profile::Kind::StmtControl,
            );
            let _nested_profile = crate::parser::parse_profile::nested_statement_guard(
                crate::parser::parse_profile::Kind::NestedStmtControl,
            );
            let _nested_depth_profile = crate::parser::parse_profile::nested_statement_depth_guard(
                crate::parser::parse_profile::StatementFamily::Control,
            );
            let _nested_control_profile = crate::parser::parse_profile::nested_statement_guard(
                crate::parser::parse_profile::Kind::NestedControlLoop,
            );
            let _top_class_method_body_control =
                crate::parser::parse_profile::FunctionBodyTopClassMethodBodyControlFamilyGuard::new(
                    crate::parser::parse_profile::ControlFamily::Loop,
                );
            let _scoped_class_method_body_control =
                crate::parser::parse_profile::TopVarInitIdentArrowExprCondClassMethodBodyControlFamilyGuard::new(
                    crate::parser::parse_profile::ControlFamily::Loop,
                );
            let _branch_block_control =
                crate::parser::parse_profile::nested_if_branch_block_control_guard(
                    crate::parser::parse_profile::ControlFamily::Loop,
                );
            let _top_class_method_body_if_branch_block_control =
                crate::parser::parse_profile::function_body_top_class_method_body_if_branch_block_control_guard(
                    crate::parser::parse_profile::ControlFamily::Loop,
                );
            let _branch_block_if_consequent_block_control = crate::parser::parse_profile::nested_if_branch_block_if_consequent_block_control_guard(crate::parser::parse_profile::ControlFamily::Loop);
            return self.parse_while_statement();
        }
        if self.is_ident("do") {
            self.reject_escaped_contextual_keyword("do")?;
            let _stmt_profile = crate::parser::parse_profile::Guard::new(
                crate::parser::parse_profile::Kind::StmtControl,
            );
            let _nested_profile = crate::parser::parse_profile::nested_statement_guard(
                crate::parser::parse_profile::Kind::NestedStmtControl,
            );
            let _nested_depth_profile = crate::parser::parse_profile::nested_statement_depth_guard(
                crate::parser::parse_profile::StatementFamily::Control,
            );
            let _nested_control_profile = crate::parser::parse_profile::nested_statement_guard(
                crate::parser::parse_profile::Kind::NestedControlLoop,
            );
            let _top_class_method_body_control =
                crate::parser::parse_profile::FunctionBodyTopClassMethodBodyControlFamilyGuard::new(
                    crate::parser::parse_profile::ControlFamily::Loop,
                );
            let _scoped_class_method_body_control =
                crate::parser::parse_profile::TopVarInitIdentArrowExprCondClassMethodBodyControlFamilyGuard::new(
                    crate::parser::parse_profile::ControlFamily::Loop,
                );
            let _branch_block_control =
                crate::parser::parse_profile::nested_if_branch_block_control_guard(
                    crate::parser::parse_profile::ControlFamily::Loop,
                );
            let _top_class_method_body_if_branch_block_control =
                crate::parser::parse_profile::function_body_top_class_method_body_if_branch_block_control_guard(
                    crate::parser::parse_profile::ControlFamily::Loop,
                );
            let _branch_block_if_consequent_block_control = crate::parser::parse_profile::nested_if_branch_block_if_consequent_block_control_guard(crate::parser::parse_profile::ControlFamily::Loop);
            return self.parse_do_while_statement();
        }
        if self.is_ident("switch") {
            let _stmt_profile = crate::parser::parse_profile::Guard::new(
                crate::parser::parse_profile::Kind::StmtControl,
            );
            let _nested_profile = crate::parser::parse_profile::nested_statement_guard(
                crate::parser::parse_profile::Kind::NestedStmtControl,
            );
            let _nested_depth_profile = crate::parser::parse_profile::nested_statement_depth_guard(
                crate::parser::parse_profile::StatementFamily::Control,
            );
            let _nested_control_profile = crate::parser::parse_profile::nested_statement_guard(
                crate::parser::parse_profile::Kind::NestedControlSwitchTryWith,
            );
            let _top_class_method_body_control =
                crate::parser::parse_profile::FunctionBodyTopClassMethodBodyControlFamilyGuard::new(
                    crate::parser::parse_profile::ControlFamily::SwitchTryWith,
                );
            let _scoped_class_method_body_control =
                crate::parser::parse_profile::TopVarInitIdentArrowExprCondClassMethodBodyControlFamilyGuard::new(
                    crate::parser::parse_profile::ControlFamily::SwitchTryWith,
                );
            let _branch_block_control =
                crate::parser::parse_profile::nested_if_branch_block_control_guard(
                    crate::parser::parse_profile::ControlFamily::SwitchTryWith,
                );
            let _top_class_method_body_if_branch_block_control =
                crate::parser::parse_profile::function_body_top_class_method_body_if_branch_block_control_guard(
                    crate::parser::parse_profile::ControlFamily::SwitchTryWith,
                );
            let _branch_block_if_consequent_block_control = crate::parser::parse_profile::nested_if_branch_block_if_consequent_block_control_guard(crate::parser::parse_profile::ControlFamily::SwitchTryWith);
            return self.parse_switch_statement();
        }
        if self.is_ident("try") {
            let _stmt_profile = crate::parser::parse_profile::Guard::new(
                crate::parser::parse_profile::Kind::StmtControl,
            );
            let _nested_profile = crate::parser::parse_profile::nested_statement_guard(
                crate::parser::parse_profile::Kind::NestedStmtControl,
            );
            let _nested_depth_profile = crate::parser::parse_profile::nested_statement_depth_guard(
                crate::parser::parse_profile::StatementFamily::Control,
            );
            let _nested_control_profile = crate::parser::parse_profile::nested_statement_guard(
                crate::parser::parse_profile::Kind::NestedControlSwitchTryWith,
            );
            let _top_class_method_body_control =
                crate::parser::parse_profile::FunctionBodyTopClassMethodBodyControlFamilyGuard::new(
                    crate::parser::parse_profile::ControlFamily::SwitchTryWith,
                );
            let _scoped_class_method_body_control =
                crate::parser::parse_profile::TopVarInitIdentArrowExprCondClassMethodBodyControlFamilyGuard::new(
                    crate::parser::parse_profile::ControlFamily::SwitchTryWith,
                );
            let _branch_block_control =
                crate::parser::parse_profile::nested_if_branch_block_control_guard(
                    crate::parser::parse_profile::ControlFamily::SwitchTryWith,
                );
            let _top_class_method_body_if_branch_block_control =
                crate::parser::parse_profile::function_body_top_class_method_body_if_branch_block_control_guard(
                    crate::parser::parse_profile::ControlFamily::SwitchTryWith,
                );
            let _branch_block_if_consequent_block_control = crate::parser::parse_profile::nested_if_branch_block_if_consequent_block_control_guard(crate::parser::parse_profile::ControlFamily::SwitchTryWith);
            return self.parse_try_statement();
        }
        if self.is_ident("return") {
            let _stmt_profile = crate::parser::parse_profile::Guard::new(
                crate::parser::parse_profile::Kind::StmtControl,
            );
            let _nested_profile = crate::parser::parse_profile::nested_statement_guard(
                crate::parser::parse_profile::Kind::NestedStmtControl,
            );
            let _nested_depth_profile = crate::parser::parse_profile::nested_statement_depth_guard(
                crate::parser::parse_profile::StatementFamily::Control,
            );
            let _nested_control_profile = crate::parser::parse_profile::nested_statement_guard(
                crate::parser::parse_profile::Kind::NestedControlReturnThrow,
            );
            let _top_class_method_body_control =
                crate::parser::parse_profile::FunctionBodyTopClassMethodBodyControlFamilyGuard::new(
                    crate::parser::parse_profile::ControlFamily::ReturnThrow,
                );
            let _scoped_class_method_body_control =
                crate::parser::parse_profile::TopVarInitIdentArrowExprCondClassMethodBodyControlFamilyGuard::new(
                    crate::parser::parse_profile::ControlFamily::ReturnThrow,
                );
            let _branch_block_control =
                crate::parser::parse_profile::nested_if_branch_block_control_guard(
                    crate::parser::parse_profile::ControlFamily::ReturnThrow,
                );
            let _top_class_method_body_if_branch_block_control =
                crate::parser::parse_profile::function_body_top_class_method_body_if_branch_block_control_guard(
                    crate::parser::parse_profile::ControlFamily::ReturnThrow,
                );
            let _branch_block_if_consequent_block_control = crate::parser::parse_profile::nested_if_branch_block_if_consequent_block_control_guard(crate::parser::parse_profile::ControlFamily::ReturnThrow);
            return self.parse_return_statement();
        }
        if self.is_ident("throw") {
            let _stmt_profile = crate::parser::parse_profile::Guard::new(
                crate::parser::parse_profile::Kind::StmtControl,
            );
            let _nested_profile = crate::parser::parse_profile::nested_statement_guard(
                crate::parser::parse_profile::Kind::NestedStmtControl,
            );
            let _nested_depth_profile = crate::parser::parse_profile::nested_statement_depth_guard(
                crate::parser::parse_profile::StatementFamily::Control,
            );
            let _nested_control_profile = crate::parser::parse_profile::nested_statement_guard(
                crate::parser::parse_profile::Kind::NestedControlReturnThrow,
            );
            let _top_class_method_body_control =
                crate::parser::parse_profile::FunctionBodyTopClassMethodBodyControlFamilyGuard::new(
                    crate::parser::parse_profile::ControlFamily::ReturnThrow,
                );
            let _scoped_class_method_body_control =
                crate::parser::parse_profile::TopVarInitIdentArrowExprCondClassMethodBodyControlFamilyGuard::new(
                    crate::parser::parse_profile::ControlFamily::ReturnThrow,
                );
            let _branch_block_control =
                crate::parser::parse_profile::nested_if_branch_block_control_guard(
                    crate::parser::parse_profile::ControlFamily::ReturnThrow,
                );
            let _top_class_method_body_if_branch_block_control =
                crate::parser::parse_profile::function_body_top_class_method_body_if_branch_block_control_guard(
                    crate::parser::parse_profile::ControlFamily::ReturnThrow,
                );
            let _branch_block_if_consequent_block_control = crate::parser::parse_profile::nested_if_branch_block_if_consequent_block_control_guard(crate::parser::parse_profile::ControlFamily::ReturnThrow);
            return self.parse_throw_statement();
        }
        if self.is_ident("break") {
            let _stmt_profile = crate::parser::parse_profile::Guard::new(
                crate::parser::parse_profile::Kind::StmtControl,
            );
            let _nested_profile = crate::parser::parse_profile::nested_statement_guard(
                crate::parser::parse_profile::Kind::NestedStmtControl,
            );
            let _nested_depth_profile = crate::parser::parse_profile::nested_statement_depth_guard(
                crate::parser::parse_profile::StatementFamily::Control,
            );
            let _nested_control_profile = crate::parser::parse_profile::nested_statement_guard(
                crate::parser::parse_profile::Kind::NestedControlBreakContinue,
            );
            let _top_class_method_body_control =
                crate::parser::parse_profile::FunctionBodyTopClassMethodBodyControlFamilyGuard::new(
                    crate::parser::parse_profile::ControlFamily::BreakContinue,
                );
            let _scoped_class_method_body_control =
                crate::parser::parse_profile::TopVarInitIdentArrowExprCondClassMethodBodyControlFamilyGuard::new(
                    crate::parser::parse_profile::ControlFamily::BreakContinue,
                );
            let _branch_block_control =
                crate::parser::parse_profile::nested_if_branch_block_control_guard(
                    crate::parser::parse_profile::ControlFamily::BreakContinue,
                );
            let _top_class_method_body_if_branch_block_control =
                crate::parser::parse_profile::function_body_top_class_method_body_if_branch_block_control_guard(
                    crate::parser::parse_profile::ControlFamily::BreakContinue,
                );
            let _branch_block_if_consequent_block_control = crate::parser::parse_profile::nested_if_branch_block_if_consequent_block_control_guard(crate::parser::parse_profile::ControlFamily::BreakContinue);
            return self.parse_break_statement();
        }
        if self.is_ident("continue") {
            let _stmt_profile = crate::parser::parse_profile::Guard::new(
                crate::parser::parse_profile::Kind::StmtControl,
            );
            let _nested_profile = crate::parser::parse_profile::nested_statement_guard(
                crate::parser::parse_profile::Kind::NestedStmtControl,
            );
            let _nested_depth_profile = crate::parser::parse_profile::nested_statement_depth_guard(
                crate::parser::parse_profile::StatementFamily::Control,
            );
            let _nested_control_profile = crate::parser::parse_profile::nested_statement_guard(
                crate::parser::parse_profile::Kind::NestedControlBreakContinue,
            );
            let _top_class_method_body_control =
                crate::parser::parse_profile::FunctionBodyTopClassMethodBodyControlFamilyGuard::new(
                    crate::parser::parse_profile::ControlFamily::BreakContinue,
                );
            let _scoped_class_method_body_control =
                crate::parser::parse_profile::TopVarInitIdentArrowExprCondClassMethodBodyControlFamilyGuard::new(
                    crate::parser::parse_profile::ControlFamily::BreakContinue,
                );
            let _branch_block_control =
                crate::parser::parse_profile::nested_if_branch_block_control_guard(
                    crate::parser::parse_profile::ControlFamily::BreakContinue,
                );
            let _top_class_method_body_if_branch_block_control =
                crate::parser::parse_profile::function_body_top_class_method_body_if_branch_block_control_guard(
                    crate::parser::parse_profile::ControlFamily::BreakContinue,
                );
            let _branch_block_if_consequent_block_control = crate::parser::parse_profile::nested_if_branch_block_if_consequent_block_control_guard(crate::parser::parse_profile::ControlFamily::BreakContinue);
            return self.parse_continue_statement();
        }
        if self.is_ident("debugger") {
            let span = self.lookahead_span();
            self.bump()?;
            self.consume_semicolon_pub()?;
            return Ok(Stmt::Debugger { span });
        }

        if self.is_ident("with") {
            let _stmt_profile = crate::parser::parse_profile::Guard::new(
                crate::parser::parse_profile::Kind::StmtControl,
            );
            let _nested_profile = crate::parser::parse_profile::nested_statement_guard(
                crate::parser::parse_profile::Kind::NestedStmtControl,
            );
            let _nested_depth_profile = crate::parser::parse_profile::nested_statement_depth_guard(
                crate::parser::parse_profile::StatementFamily::Control,
            );
            let _nested_control_profile = crate::parser::parse_profile::nested_statement_guard(
                crate::parser::parse_profile::Kind::NestedControlSwitchTryWith,
            );
            let _top_class_method_body_control =
                crate::parser::parse_profile::FunctionBodyTopClassMethodBodyControlFamilyGuard::new(
                    crate::parser::parse_profile::ControlFamily::SwitchTryWith,
                );
            let _scoped_class_method_body_control =
                crate::parser::parse_profile::TopVarInitIdentArrowExprCondClassMethodBodyControlFamilyGuard::new(
                    crate::parser::parse_profile::ControlFamily::SwitchTryWith,
                );
            let _branch_block_control =
                crate::parser::parse_profile::nested_if_branch_block_control_guard(
                    crate::parser::parse_profile::ControlFamily::SwitchTryWith,
                );
            let _top_class_method_body_if_branch_block_control =
                crate::parser::parse_profile::function_body_top_class_method_body_if_branch_block_control_guard(
                    crate::parser::parse_profile::ControlFamily::SwitchTryWith,
                );
            let _branch_block_if_consequent_block_control = crate::parser::parse_profile::nested_if_branch_block_if_consequent_block_control_guard(crate::parser::parse_profile::ControlFamily::SwitchTryWith);
            return self.parse_with_statement();
        }

        if let TokenKind::Ident(_) = self.current_kind() {
            let peek_pos = self.lookahead_span().end;
            let bytes = self.source().as_bytes();
            let mut p = peek_pos;
            while p < bytes.len() && bytes[p].is_ascii_whitespace() {
                p += 1;
            }
            if bytes.get(p) == Some(&b':') {
                let name = if let TokenKind::Ident(n) = self.current_kind().clone() {
                    n
                } else {
                    unreachable!()
                };
                let label_span = self.lookahead_span();
                if (self.in_generator || self.strict_mode) && name == "yield" {
                    return Err(ParseError {
                        span: label_span,
                        message: "`yield` is not a valid label in this context".into(),
                    });
                }
                if (self.in_async || self.is_module_goal()) && name == "await" {
                    return Err(ParseError {
                        span: label_span,
                        message: "`await` is not a valid label in async function or module code"
                            .into(),
                    });
                }

                if crate::parser::is_unconditional_reserved_word(&name)
                    || (self.strict_mode && crate::parser::is_strict_reserved_word(&name))
                {
                    return Err(ParseError {
                        span: label_span,
                        message: format!(
                            "`{}` is a reserved word and cannot be used as a label",
                            name
                        ),
                    });
                }
                self.bump()?;
                self.expect_punct(Punct::Colon)?;

                let prev_allow = self.allow_annex_b_function_in_substatement;
                if !self.strict_mode {
                    self.allow_annex_b_function_in_substatement = true;
                }
                let _stmt_profile = crate::parser::parse_profile::Guard::new(
                    crate::parser::parse_profile::Kind::StmtLabel,
                );
                let body = self.parse_substatement();
                self.allow_annex_b_function_in_substatement = prev_allow;
                let body = body?;
                let end = body.span().start.max(self.last_span_end());
                return Ok(Stmt::Labelled {
                    label: BindingIdentifier {
                        name,
                        span: label_span,
                    },
                    body: Box::new(body),
                    span: Span::new(start, end),
                });
            }
        }

        let _stmt_profile = crate::parser::parse_profile::Guard::new(
            crate::parser::parse_profile::Kind::StmtExpression,
        );
        let _nested_profile = crate::parser::parse_profile::nested_statement_guard(
            crate::parser::parse_profile::Kind::NestedStmtExpression,
        );
        let _nested_depth_profile = crate::parser::parse_profile::nested_statement_depth_guard(
            crate::parser::parse_profile::StatementFamily::Expression,
        );
        let expr = self.parse_expression()?;
        self.consume_semicolon_pub()?;
        let end = self.last_span_end();
        Ok(Stmt::Expression {
            expr,
            span: Span::new(start, end),
        })
    }

    pub(crate) fn parse_variable_statement(&mut self) -> Result<VariableStatement, ParseError> {
        let start = self.lookahead_span().start;
        let kind = match self.current_kind() {
            TokenKind::Ident(s) if s == "var" => VariableKind::Var,
            TokenKind::Ident(s) if s == "let" && self.is_contextual_keyword("let") => {
                VariableKind::Let
            }
            TokenKind::Ident(s) if s == "const" => VariableKind::Const,
            _ => return Err(self.err_here("expected var/let/const".into())),
        };
        self.bump()?;
        let mut declarators = Vec::new();
        loop {
            let d_start = self.lookahead_span().start;
            let target = {
                let _top_profile =
                    crate::parser::parse_profile::FunctionBodyTopVariableDeclPhaseGuard::new(
                        crate::parser::parse_profile::VariableDeclPhase::Target,
                    );
                let _profile = crate::parser::parse_profile::nested_phase_guard(
                    crate::parser::parse_profile::Kind::NestedVarTarget,
                );
                let _depth3_profile = crate::parser::parse_profile::nested_variable_depth3_guard(
                    crate::parser::parse_profile::VariableDeclPhase::Target,
                );
                self.parse_binding_target()?
            };

            {
                let _top_profile =
                    crate::parser::parse_profile::FunctionBodyTopVariableDeclPhaseGuard::new(
                        crate::parser::parse_profile::VariableDeclPhase::NoLet,
                    );
                let _profile = crate::parser::parse_profile::nested_phase_guard(
                    crate::parser::parse_profile::Kind::NestedVarNoLet,
                );
                let _depth3_profile = crate::parser::parse_profile::nested_variable_depth3_guard(
                    crate::parser::parse_profile::VariableDeclPhase::NoLet,
                );
                Self::check_no_let_bound_name(kind, &target)?;
            }
            let init = if matches!(self.current_kind(), TokenKind::Punct(Punct::Assign)) {
                let _top_profile =
                    crate::parser::parse_profile::FunctionBodyTopVariableDeclPhaseGuard::new(
                        crate::parser::parse_profile::VariableDeclPhase::Init,
                    );
                let _profile = crate::parser::parse_profile::nested_phase_guard(
                    crate::parser::parse_profile::Kind::NestedVarInit,
                );
                let _depth3_profile = crate::parser::parse_profile::nested_variable_depth3_guard(
                    crate::parser::parse_profile::VariableDeclPhase::Init,
                );
                {
                    let _assign_profile = crate::parser::parse_profile::nested_phase_guard(
                        crate::parser::parse_profile::Kind::NestedVarInitAssign,
                    );
                    self.bump()?;
                }
                let (init_kind, top_init_family) = match self.current_kind() {
                    TokenKind::Ident(s) if s == "function" || s == "class" => (
                        crate::parser::parse_profile::Kind::NestedVarInitFnClass,
                        crate::parser::parse_profile::VarInitFamily::FnClass,
                    ),
                    TokenKind::Punct(Punct::LParen) => (
                        crate::parser::parse_profile::Kind::NestedVarInitParen,
                        crate::parser::parse_profile::VarInitFamily::Paren,
                    ),
                    TokenKind::Punct(Punct::LBrace) | TokenKind::Punct(Punct::LBracket) => (
                        crate::parser::parse_profile::Kind::NestedVarInitObjectArray,
                        crate::parser::parse_profile::VarInitFamily::ObjectArray,
                    ),
                    TokenKind::Ident(_) => (
                        crate::parser::parse_profile::Kind::NestedVarInitIdent,
                        crate::parser::parse_profile::VarInitFamily::Ident,
                    ),
                    TokenKind::Number(..)
                    | TokenKind::BigInt(..)
                    | TokenKind::String(_)
                    | TokenKind::WtfString(_)
                    | TokenKind::Template { .. }
                    | TokenKind::Regex { .. } => (
                        crate::parser::parse_profile::Kind::NestedVarInitLiteral,
                        crate::parser::parse_profile::VarInitFamily::Literal,
                    ),
                    _ => (
                        crate::parser::parse_profile::Kind::NestedVarInitOther,
                        crate::parser::parse_profile::VarInitFamily::Other,
                    ),
                };
                let _init_profile = crate::parser::parse_profile::nested_phase_guard(init_kind);
                let _top_init_family_profile =
                    crate::parser::parse_profile::FunctionBodyTopVarInitFamilyGuard::new(
                        top_init_family,
                    );
                let _top_ident_assign_profile =
                    crate::parser::parse_profile::TopVarInitIdentGuard::new(matches!(
                        top_init_family,
                        crate::parser::parse_profile::VarInitFamily::Ident
                    ));
                let top_init_expr_start =
                    crate::parser::parse_profile::function_body_top_var_init_expr_start();
                let direct_start = crate::parser::parse_profile::nested_var_init_direct_start();
                let depth3_init_start = (crate::parser::parse_profile::function_body_depth() == 3)
                    .then(std::time::Instant::now);
                let init_starts_with_array =
                    matches!(self.current_kind(), TokenKind::Punct(Punct::LBracket));
                let depth3_object_array_start = matches!(
                    self.current_kind(),
                    TokenKind::Punct(Punct::LBrace) | TokenKind::Punct(Punct::LBracket)
                )
                .then(|| {
                    (crate::parser::parse_profile::function_body_depth() == 3)
                        .then(std::time::Instant::now)
                })
                .flatten();
                let parsed = {
                    let _expr_profile = crate::parser::parse_profile::nested_phase_guard(
                        crate::parser::parse_profile::Kind::NestedVarInitExpr,
                    );
                    let _depth3_object_init =
                        matches!(self.current_kind(), TokenKind::Punct(Punct::LBrace))
                            .then(crate::parser::parse_profile::depth3_object_init_guard)
                            .flatten();
                    self.parse_assignment_expression()
                };
                crate::parser::parse_profile::record_function_body_top_var_init_expr(
                    top_init_family,
                    top_init_expr_start,
                );
                crate::parser::parse_profile::record_nested_var_init_direct(
                    init_kind,
                    direct_start,
                );
                crate::parser::parse_profile::record_nested_var_init_depth3(
                    init_kind,
                    depth3_init_start,
                );
                crate::parser::parse_profile::record_nested_var_init_depth3_object_array(
                    init_starts_with_array,
                    depth3_object_array_start,
                );
                Some(parsed?)
            } else {
                None
            };
            let _top_finish_profile =
                crate::parser::parse_profile::FunctionBodyTopVariableDeclPhaseGuard::new(
                    crate::parser::parse_profile::VariableDeclPhase::Finish,
                );
            let _finish_profile = crate::parser::parse_profile::nested_phase_guard(
                crate::parser::parse_profile::Kind::NestedVarFinish,
            );
            let _depth3_finish_profile = crate::parser::parse_profile::nested_variable_depth3_guard(
                crate::parser::parse_profile::VariableDeclPhase::Finish,
            );

            if matches!(kind, VariableKind::Const) && init.is_none() {
                return Err(ParseError {
                    span: Span::new(d_start, self.last_span_end()),
                    message: "Missing initializer in const declaration".into(),
                });
            }
            if init.is_none()
                && !matches!(
                    self.current_kind(),
                    TokenKind::Punct(Punct::Comma)
                        | TokenKind::Punct(Punct::Semicolon)
                        | TokenKind::Punct(Punct::RParen)
                        | TokenKind::Punct(Punct::RBrace)
                        | TokenKind::Eof
                )
                && !self.lookahead_preceded_by_lt()
            {
                return Err(ParseError {
                    span: self.lookahead_span(),
                    message: "expected initializer, comma, or semicolon after declaration".into(),
                });
            }
            let d_end = self.last_span_end();
            declarators.push(VariableDeclarator {
                target,
                init,
                span: Span::new(d_start, d_end),
            });
            if matches!(self.current_kind(), TokenKind::Punct(Punct::Comma)) {
                self.bump()?;
            } else {
                break;
            }
        }
        self.consume_semicolon_pub()?;
        let end = self.last_span_end();
        Ok(VariableStatement {
            kind,
            declarators,
            span: Span::new(start, end),
        })
    }

    pub(crate) fn using_starts_declaration(&self, after: usize) -> bool {
        let bytes = self.source().as_bytes();
        let mut p = after;

        while p < bytes.len() {
            let b = bytes[p];
            if b == b'\n' || b == b'\r' {
                return false;
            }
            if b == b' ' || b == b'\t' || b == 0x0c || b == 0x0b {
                p += 1;
            } else {
                break;
            }
        }
        if p >= bytes.len() {
            return false;
        }

        let b = bytes[p];
        if !(b.is_ascii_alphabetic() || b == b'_' || b == b'$') {
            return false;
        }

        let mut q = p;
        while q < bytes.len()
            && (bytes[q].is_ascii_alphanumeric() || bytes[q] == b'_' || bytes[q] == b'$')
        {
            q += 1;
        }
        let word = &self.source()[p..q];
        !matches!(word, "of" | "in")
    }

    fn using_starts_for_head_declaration(&self, after: usize) -> bool {
        if self.using_starts_declaration(after) {
            return true;
        }
        let bytes = self.source().as_bytes();
        let mut p = after;
        while p < bytes.len() {
            let b = bytes[p];
            if b == b'\n' || b == b'\r' {
                return false;
            }
            if b == b' ' || b == b'\t' || b == 0x0c || b == 0x0b {
                p += 1;
            } else {
                break;
            }
        }
        let word_start = p;
        while p < bytes.len()
            && (bytes[p].is_ascii_alphanumeric() || bytes[p] == b'_' || bytes[p] == b'$')
        {
            p += 1;
        }
        let word = &self.source()[word_start..p];
        if !matches!(word, "of" | "in") {
            return false;
        }
        while p < bytes.len() {
            let b = bytes[p];
            if b == b'\n' || b == b'\r' {
                return false;
            }
            if b == b' ' || b == b'\t' || b == 0x0c || b == 0x0b {
                p += 1;
            } else {
                break;
            }
        }
        matches!(bytes.get(p), Some(b'=') | Some(b',') | Some(b';'))
    }

    pub(crate) fn await_using_starts_declaration(&self) -> bool {
        let bytes = self.source().as_bytes();
        let mut p = self.lookahead_span().end;

        while p < bytes.len() && (bytes[p] == b' ' || bytes[p] == b'\t') {
            p += 1;
        }
        if !self.source()[p..].starts_with("using") {
            return false;
        }
        let after_using = p + "using".len();

        if let Some(&c) = bytes.get(after_using) {
            if c.is_ascii_alphanumeric() || c == b'_' || c == b'$' {
                return false;
            }
        }
        self.using_starts_declaration(after_using)
    }

    fn await_using_starts_for_head_declaration(&self) -> bool {
        let bytes = self.source().as_bytes();
        let mut p = self.lookahead_span().end;
        while p < bytes.len() && (bytes[p] == b' ' || bytes[p] == b'\t') {
            p += 1;
        }
        if !self.source()[p..].starts_with("using") {
            return false;
        }
        let after_using = p + "using".len();
        if let Some(&c) = bytes.get(after_using) {
            if c.is_ascii_alphanumeric() || c == b'_' || c == b'$' {
                return false;
            }
        }
        if self.using_starts_for_head_declaration(after_using) {
            return true;
        }
        p = after_using;
        while p < bytes.len() {
            let b = bytes[p];
            if b == b'\n' || b == b'\r' {
                return false;
            }
            if b == b' ' || b == b'\t' || b == 0x0c || b == 0x0b {
                p += 1;
            } else {
                break;
            }
        }
        if !self.source()[p..].starts_with("of") {
            return false;
        }
        let after_binding = p + "of".len();
        if let Some(&c) = bytes.get(after_binding) {
            if c.is_ascii_alphanumeric() || c == b'_' || c == b'$' {
                return false;
            }
        }
        let mut q = after_binding;
        while q < bytes.len() {
            let b = bytes[q];
            if b == b'\n' || b == b'\r' {
                return false;
            }
            if b == b' ' || b == b'\t' || b == 0x0c || b == 0x0b {
                q += 1;
            } else {
                break;
            }
        }
        self.source()[q..].starts_with("of")
            && bytes
                .get(q + "of".len())
                .is_none_or(|c| !(c.is_ascii_alphanumeric() || *c == b'_' || *c == b'$'))
    }

    pub(crate) fn parse_using_statement(
        &mut self,
        is_await: bool,
    ) -> Result<VariableStatement, ParseError> {
        let start = self.lookahead_span().start;
        if is_await {
            self.bump()?;
        }

        self.bump()?;
        let kind = if is_await {
            VariableKind::AwaitUsing
        } else {
            VariableKind::Using
        };
        self.saw_using_declaration = true;
        let mut declarators = Vec::new();
        loop {
            let d_start = self.lookahead_span().start;
            let target = self.parse_binding_target()?;

            if !matches!(target, rusty_js_ast::BindingPattern::Identifier(_)) {
                return Err(ParseError {
                    span: Span::new(d_start, self.last_span_end()),
                    message: "`using` declaration requires a plain identifier binding".into(),
                });
            }
            let init = if matches!(self.current_kind(), TokenKind::Punct(Punct::Assign)) {
                self.bump()?;
                Some(self.parse_assignment_expression()?)
            } else {
                None
            };
            if init.is_none() {
                return Err(ParseError {
                    span: Span::new(d_start, self.last_span_end()),
                    message: "Missing initializer in `using` declaration".into(),
                });
            }
            let d_end = self.last_span_end();
            declarators.push(VariableDeclarator {
                target,
                init,
                span: Span::new(d_start, d_end),
            });
            if matches!(self.current_kind(), TokenKind::Punct(Punct::Comma)) {
                self.bump()?;
            } else {
                break;
            }
        }
        self.consume_semicolon_pub()?;
        let end = self.last_span_end();
        Ok(VariableStatement {
            kind,
            declarators,
            span: Span::new(start, end),
        })
    }

    fn let_starts_lexical_declaration(&self) -> bool {
        if !self.is_contextual_keyword("let") {
            return false;
        }
        let bytes = self.source().as_bytes();
        let p = Self::skip_ws_and_comments_allow_lt(bytes, self.lookahead_span().end);

        let mut word_end = p;
        while bytes
            .get(word_end)
            .map(|b| b.is_ascii_alphanumeric() || *b == b'_' || *b == b'$')
            .unwrap_or(false)
        {
            word_end += 1;
        }
        if &bytes[p..word_end] == b"in" {
            return false;
        }
        bytes
            .get(p)
            .copied()
            .map(Self::byte_can_start_binding_list)
            .unwrap_or(false)
    }

    pub(crate) fn parse_function_decl_stmt(
        &mut self,
        is_async: bool,
        async_start: Option<usize>,
    ) -> Result<Stmt, ParseError> {
        let start = async_start.unwrap_or_else(|| self.lookahead_span().start);
        let (is_generator, name) = {
            let _top_profile =
                crate::parser::parse_profile::FunctionBodyTopFunctionDeclPhaseGuard::new(
                    crate::parser::parse_profile::FunctionDeclPhase::Name,
                );
            let _profile = crate::parser::parse_profile::nested_phase_guard(
                crate::parser::parse_profile::Kind::NestedFnDeclName,
            );
            let _depth2_profile = crate::parser::parse_profile::nested_function_decl_depth2_guard(
                crate::parser::parse_profile::FunctionDeclPhase::Name,
            );
            self.expect_keyword("function")?;
            let is_generator = if matches!(self.current_kind(), TokenKind::Punct(Punct::Star)) {
                self.bump()?;
                true
            } else {
                false
            };
            let name = if let TokenKind::Ident(n) = self.current_kind().clone() {
                let span = self.lookahead_span();

                if crate::parser::is_unconditional_reserved_word(&n) {
                    return Err(ParseError {
                        span,
                        message: format!(
                            "`{}` is a reserved word and cannot be used as a function name",
                            n
                        ),
                    });
                }

                if self.strict_mode && crate::parser::is_strict_reserved_word(&n) {
                    return Err(ParseError {
                        span,
                        message: format!(
                            "`{}` is a reserved word in strict mode and cannot be used as a function name",
                            n
                        ),
                    });
                }
                if self.strict_mode && (n == "eval" || n == "arguments") {
                    return Err(ParseError {
                        span,
                        message: format!("Function name '{}' is not allowed in strict mode", n),
                    });
                }
                if (self.in_generator || self.strict_mode) && n == "yield" {
                    return Err(ParseError {
                        span,
                        message: "`yield` is not a valid function name in this context".into(),
                    });
                }

                if (self.in_async || self.is_module_goal()) && n == "await" {
                    return Err(ParseError {
                        span,
                        message: "`await` is not a valid function name in async or module context"
                            .into(),
                    });
                }
                self.bump()?;
                Some(BindingIdentifier { name: n, span })
            } else {
                None
            };
            (is_generator, name)
        };
        let params = {
            let _top_profile =
                crate::parser::parse_profile::FunctionBodyTopFunctionDeclPhaseGuard::new(
                    crate::parser::parse_profile::FunctionDeclPhase::Params,
                );
            let _profile = crate::parser::parse_profile::nested_phase_guard(
                crate::parser::parse_profile::Kind::NestedFnDeclParams,
            );
            let _depth2_profile = crate::parser::parse_profile::nested_function_decl_depth2_guard(
                crate::parser::parse_profile::FunctionDeclPhase::Params,
            );
            self.parse_function_parameters_ga(is_generator, is_async)?
        };
        {
            let _top_profile =
                crate::parser::parse_profile::FunctionBodyTopFunctionDeclPhaseGuard::new(
                    crate::parser::parse_profile::FunctionDeclPhase::Dups,
                );
            let _profile = crate::parser::parse_profile::nested_phase_guard(
                crate::parser::parse_profile::Kind::NestedFnDeclDups,
            );
            let _depth2_profile = crate::parser::parse_profile::nested_function_decl_depth2_guard(
                crate::parser::parse_profile::FunctionDeclPhase::Dups,
            );
            self.check_formal_parameter_dups(&params)?;
        }

        self.pending_fn_body_close_regexp = true;
        let body = {
            let _top_profile =
                crate::parser::parse_profile::FunctionBodyTopFunctionDeclPhaseGuard::new(
                    crate::parser::parse_profile::FunctionDeclPhase::Body,
                );
            let _profile = crate::parser::parse_profile::nested_phase_guard(
                crate::parser::parse_profile::Kind::NestedFnDeclBody,
            );
            let _depth2_profile = crate::parser::parse_profile::nested_function_decl_depth2_guard(
                crate::parser::parse_profile::FunctionDeclPhase::Body,
            );
            self.parse_function_body_gs(
                Some(is_generator),
                Some(is_async),
                Self::is_simple_param_list(&params),
            )?
        };
        if self.last_body_became_strict {
            let _top_profile =
                crate::parser::parse_profile::FunctionBodyTopFunctionDeclPhaseGuard::new(
                    crate::parser::parse_profile::FunctionDeclPhase::Revalidate,
                );
            let _profile = crate::parser::parse_profile::nested_phase_guard(
                crate::parser::parse_profile::Kind::NestedFnDeclRevalidate,
            );
            let _depth2_profile = crate::parser::parse_profile::nested_function_decl_depth2_guard(
                crate::parser::parse_profile::FunctionDeclPhase::Revalidate,
            );
            self.revalidate_params_after_strict_promotion(&params, name.as_ref())?;
        }
        {
            let _top_profile =
                crate::parser::parse_profile::FunctionBodyTopFunctionDeclPhaseGuard::new(
                    crate::parser::parse_profile::FunctionDeclPhase::Finish,
                );
            let _profile = crate::parser::parse_profile::nested_phase_guard(
                crate::parser::parse_profile::Kind::NestedFnDeclFinish,
            );
            let _depth2_profile = crate::parser::parse_profile::nested_function_decl_depth2_guard(
                crate::parser::parse_profile::FunctionDeclPhase::Finish,
            );
            let end = self.last_span_end();
            Ok(Stmt::FunctionDecl {
                name,
                is_async,
                is_generator,
                params,
                body,
                span: Span::new(start, end),
            })
        }
    }

    pub(crate) fn parse_function_parameters(
        &mut self,
    ) -> Result<Vec<rusty_js_ast::Parameter>, ParseError> {
        self.parse_function_parameters_g(false)
    }

    pub(crate) fn parse_function_parameters_g(
        &mut self,
        is_generator: bool,
    ) -> Result<Vec<rusty_js_ast::Parameter>, ParseError> {
        self.parse_function_parameters_ga(is_generator, false)
    }

    pub(crate) fn parse_unique_formal_parameters_ga(
        &mut self,
        is_generator: bool,
        is_async: bool,
    ) -> Result<Vec<rusty_js_ast::Parameter>, ParseError> {
        let params = self.parse_function_parameters_ga(is_generator, is_async)?;
        let mut seen: Vec<String> = Vec::new();
        for p in &params {
            for id in p.target.collect_names() {
                if seen.iter().any(|s| s == &id.name) {
                    return Err(self.err_at(
                        id.span,
                        format!("method has duplicate parameter name `{}`", id.name),
                    ));
                }
                seen.push(id.name.clone());
            }
        }
        Ok(params)
    }

    pub(crate) fn check_formal_parameter_dups(
        &self,
        params: &[rusty_js_ast::Parameter],
    ) -> Result<(), ParseError> {
        if !self.strict_mode && Self::is_simple_param_list(params) {
            return Ok(());
        }
        let mut seen: Vec<String> = Vec::new();
        for p in params {
            for id in p.target.collect_names() {
                if seen.iter().any(|s| s == &id.name) {
                    return Err(
                        self.err_at(id.span, format!("duplicate parameter name `{}`", id.name))
                    );
                }
                seen.push(id.name.clone());
            }
        }
        Ok(())
    }

    pub(crate) fn revalidate_params_after_strict_promotion(
        &self,
        params: &[rusty_js_ast::Parameter],
        name: Option<&rusty_js_ast::BindingIdentifier>,
    ) -> Result<(), ParseError> {
        let strict_forbidden = |n: &str| {
            n == "eval"
                || n == "arguments"
                || n == "yield"
                || crate::parser::is_strict_reserved_word(n)
        };
        if let Some(n) = name {
            if strict_forbidden(&n.name) {
                return Err(self.err_at(
                    n.span,
                    format!("`{}` is not a valid function name in strict mode", n.name),
                ));
            }
        }
        let mut seen: Vec<String> = Vec::new();
        for p in params {
            for id in p.target.collect_names() {
                if strict_forbidden(&id.name) {
                    return Err(self.err_at(
                        id.span,
                        format!("`{}` is not a valid parameter name in strict mode", id.name),
                    ));
                }
                if seen.iter().any(|s| s == &id.name) {
                    return Err(
                        self.err_at(id.span, format!("duplicate parameter name `{}`", id.name))
                    );
                }
                seen.push(id.name.clone());
            }
        }
        Ok(())
    }

    pub(crate) fn validate_accessor_parameters(
        &self,
        kind: rusty_js_ast::MethodKind,
        params: &[rusty_js_ast::Parameter],
    ) -> Result<(), ParseError> {
        match kind {
            rusty_js_ast::MethodKind::Getter => {
                if let Some(param) = params.first() {
                    return Err(
                        self.err_at(param.span, "Getter must not have formal parameters".into())
                    );
                }
            }
            rusty_js_ast::MethodKind::Setter => {
                if params.len() != 1 {
                    let span = params
                        .first()
                        .map(|p| p.span)
                        .unwrap_or_else(|| self.lookahead_span());
                    return Err(
                        self.err_at(span, "Setter must have exactly one formal parameter".into())
                    );
                }
                if params[0].rest {
                    return Err(self.err_at(
                        params[0].span,
                        "Setter parameter may not be a rest parameter".into(),
                    ));
                }
            }
            rusty_js_ast::MethodKind::Method | rusty_js_ast::MethodKind::Constructor => {}
        }
        Ok(())
    }

    pub(crate) fn parse_function_parameters_ga(
        &mut self,
        is_generator: bool,
        is_async: bool,
    ) -> Result<Vec<rusty_js_ast::Parameter>, ParseError> {
        self.parse_function_parameters_gai(is_generator, is_async, false)
    }

    pub(crate) fn parse_function_parameters_gai(
        &mut self,
        is_generator: bool,
        is_async: bool,
        inherit_context: bool,
    ) -> Result<Vec<rusty_js_ast::Parameter>, ParseError> {
        self.expect_punct(Punct::LParen)?;
        let prior_in_params = self.in_function_params;
        let prior_in_generator = self.in_generator;
        let prior_in_async = self.in_async;
        let prior_function_body_depth = self.function_body_depth;
        self.in_function_params = true;
        self.function_body_depth += 1;
        if inherit_context {

            if is_async {
                self.in_async = true;
            }
        } else {

            self.in_generator = is_generator;
            self.in_async = is_async;
        }

        let prior_in_disallowed = std::mem::take(&mut self.in_disallowed);
        let result = self.parse_function_parameters_inner();
        self.in_disallowed = prior_in_disallowed;
        self.in_function_params = prior_in_params;
        self.in_generator = prior_in_generator;
        self.in_async = prior_in_async;
        self.function_body_depth = prior_function_body_depth;
        result
    }

    fn parse_function_parameters_inner(
        &mut self,
    ) -> Result<Vec<rusty_js_ast::Parameter>, ParseError> {
        let mut out = Vec::new();
        while !matches!(self.current_kind(), TokenKind::Punct(Punct::RParen)) {
            let p_start = self.lookahead_span().start;
            let rest = if matches!(self.current_kind(), TokenKind::Punct(Punct::Spread)) {
                self.bump()?;
                true
            } else {
                false
            };
            let target = self.parse_binding_target()?;
            let default = if matches!(self.current_kind(), TokenKind::Punct(Punct::Assign)) {
                self.bump()?;
                Some(self.parse_assignment_expression()?)
            } else {
                None
            };

            if rest && default.is_some() {
                return Err(ParseError {
                    span: Span::new(p_start, self.last_span_end()),
                    message: "Rest parameter may not have a default initializer".into(),
                });
            }
            let p_end = self.last_span_end();
            out.push(rusty_js_ast::Parameter {
                target,
                default,
                rest,
                span: Span::new(p_start, p_end),
            });
            if matches!(self.current_kind(), TokenKind::Punct(Punct::Comma)) {

                if rest {
                    return Err(ParseError {
                        span: self.lookahead_span(),
                        message: "Rest parameter may not be followed by a trailing comma".into(),
                    });
                }
                self.bump()?;
            } else {
                break;
            }
        }
        self.expect_punct(Punct::RParen)?;
        Ok(out)
    }

    pub(crate) fn parse_function_body(&mut self) -> Result<Vec<Stmt>, ParseError> {
        self.parse_function_body_g(None)
    }

    pub(crate) fn parse_function_body_g(
        &mut self,
        is_generator: Option<bool>,
    ) -> Result<Vec<Stmt>, ParseError> {
        self.parse_function_body_gs(is_generator, None, true)
    }

    pub(crate) fn check_no_let_bound_name(
        kind: rusty_js_ast::VariableKind,
        target: &rusty_js_ast::BindingPattern,
    ) -> Result<(), ParseError> {
        if matches!(kind, rusty_js_ast::VariableKind::Var) {
            return Ok(());
        }
        for id in target.collect_names() {
            if id.name == "let" {
                return Err(ParseError {
                    span: id.span,
                    message: "Lexical declaration may not bind the name 'let'".into(),
                });
            }
        }
        Ok(())
    }

    pub(crate) fn is_simple_param_list(params: &[rusty_js_ast::Parameter]) -> bool {
        params.iter().all(|p| {
            matches!(p.target, rusty_js_ast::BindingPattern::Identifier(_))
                && p.default.is_none()
                && !p.rest
        })
    }

    pub(crate) fn parse_function_body_gs(
        &mut self,
        is_generator: Option<bool>,
        is_async: Option<bool>,
        is_simple: bool,
    ) -> Result<Vec<Stmt>, ParseError> {
        let _profile = crate::parser::parse_profile::Guard::new(
            crate::parser::parse_profile::Kind::FunctionBody,
        );
        let _body_depth_guard = crate::parser::parse_profile::FunctionBodyDepthGuard::new();
        let body_start = self.lookahead_span();

        let close_under_regexp = std::mem::take(&mut self.pending_fn_body_close_regexp);

        let saved_allow_cover = std::mem::take(&mut self.allow_cover_initialized_name_in_for_head);
        self.expect_punct(Punct::LBrace)?;

        self.function_body_depth += 1;

        let prior_strict = self.strict_mode;
        let has_strict_directive = self.peek_use_strict_directive();

        let became_strict = has_strict_directive && !prior_strict;
        if has_strict_directive {
            if !is_simple {
                return Err(ParseError {
                    span: body_start,
                    message:
                        "Illegal 'use strict' directive in function with non-simple parameter list"
                            .into(),
                });
            }
            self.strict_mode = true;
            self.set_lexer_strict(true);

            if self.last_string_had_legacy_escape() {
                return Err(ParseError {
                    span: body_start,
                    message: "legacy octal/non-octal escape sequence in strict-mode string literal"
                        .into(),
                });
            }
        }

        let prior_gen = self.in_generator;
        if let Some(g) = is_generator {
            self.in_generator = g;
        }
        let prior_async = self.in_async;
        if let Some(a) = is_async {
            self.in_async = a;
        }
        let prior_in_function_params = self.in_function_params;
        self.in_function_params = false;

        let prior_in_disallowed = std::mem::take(&mut self.in_disallowed);
        let mut out = Vec::new();
        {
            let _nested_body_profile = crate::parser::parse_profile::nested_phase_guard(
                crate::parser::parse_profile::Kind::NestedFnBodyLoop,
            );
            let _profile = crate::parser::parse_profile::Guard::new(
                crate::parser::parse_profile::Kind::FnBodyLoop,
            );
            let loop_depth_kind = if crate::parser::parse_profile::function_body_depth() <= 1 {
                crate::parser::parse_profile::Kind::FnBodyLoopTop
            } else {
                crate::parser::parse_profile::Kind::FnBodyLoopNested
            };
            let _profile_depth = crate::parser::parse_profile::Guard::new(loop_depth_kind);
            let nested_loop_depth_kind = match crate::parser::parse_profile::function_body_depth() {
                2 => Some(crate::parser::parse_profile::Kind::NestedFnBodyLoopDepth2),
                3 => Some(crate::parser::parse_profile::Kind::NestedFnBodyLoopDepth3),
                4.. => Some(crate::parser::parse_profile::Kind::NestedFnBodyLoopDepth4Plus),
                _ => None,
            };
            let _nested_loop_depth_profile =
                nested_loop_depth_kind.map(crate::parser::parse_profile::Guard::new);
            while !matches!(self.current_kind(), TokenKind::Punct(Punct::RBrace))
                && !self.at_eof_internal()
            {
                if crate::parser::parse_profile::enabled() {
                    let family = self.profile_current_substatement_family();
                    let _top_stmt_family_profile =
                        crate::parser::parse_profile::FunctionBodyTopStatementFamilyGuard::new(
                            family,
                        );
                    let _top_class_method_body_stmt_family_profile =
                        crate::parser::parse_profile::FunctionBodyTopClassMethodBodyStatementFamilyGuard::new(family);
                    let _scoped_class_method_body_stmt_family_profile =
                        crate::parser::parse_profile::TopVarInitIdentArrowExprCondClassMethodBodyStatementFamilyGuard::new(family);
                    out.push(self.parse_statement()?);
                } else {
                    out.push(self.parse_statement()?);
                }
            }
        }

        {
            let _nested_body_profile = crate::parser::parse_profile::nested_phase_guard(
                crate::parser::parse_profile::Kind::NestedFnBodyBoundNames,
            );
            let _profile = crate::parser::parse_profile::Guard::new(
                crate::parser::parse_profile::Kind::FnBodyBoundNames,
            );
            self.check_function_body_bound_names(&out)?;
        }

        if close_under_regexp {
            let _nested_body_profile = crate::parser::parse_profile::nested_phase_guard(
                crate::parser::parse_profile::Kind::NestedFnBodyClose,
            );
            self.consume_statement_rbrace()?;
        } else {
            let _nested_body_profile = crate::parser::parse_profile::nested_phase_guard(
                crate::parser::parse_profile::Kind::NestedFnBodyClose,
            );
            self.expect_punct(Punct::RBrace)?;
        }
        {
            let _nested_body_profile = crate::parser::parse_profile::nested_phase_guard(
                crate::parser::parse_profile::Kind::NestedFnBodyRestore,
            );
            self.function_body_depth = self.function_body_depth.saturating_sub(1);

            self.allow_cover_initialized_name_in_for_head = saved_allow_cover;
            self.strict_mode = prior_strict;
            self.set_lexer_strict(prior_strict);
            self.in_generator = prior_gen;
            self.in_async = prior_async;
            self.in_function_params = prior_in_function_params;
            self.in_disallowed = prior_in_disallowed;
        }

        let out = if self.saw_using_declaration {
            let _nested_body_profile = crate::parser::parse_profile::nested_phase_guard(
                crate::parser::parse_profile::Kind::NestedFnBodyDesugar,
            );
            let _profile = crate::parser::parse_profile::Guard::new(
                crate::parser::parse_profile::Kind::FnBodyDesugar,
            );
            desugar_using_block(out)
        } else {
            out
        };

        self.last_body_became_strict = became_strict;
        Ok(out)
    }

    pub(crate) fn parse_class_decl_stmt(&mut self) -> Result<Stmt, ParseError> {
        self.parse_class_decl_stmt_with_decorators(Vec::new())
    }

    pub(crate) fn parse_class_decorators(&mut self) -> Result<Vec<String>, ParseError> {
        let mut decorators = Vec::new();
        while matches!(self.current_kind(), TokenKind::Punct(Punct::At)) {
            self.bump()?;

            if let TokenKind::Ident(name) = self.current_kind().clone() {
                if name == "cloneable" || name == "transferable" {
                    self.bump()?;
                    if matches!(self.current_kind(), TokenKind::Punct(Punct::LParen)) {
                        self.bump()?;
                        if !matches!(self.current_kind(), TokenKind::Punct(Punct::RParen)) {
                            return Err(ParseError {
                                span: self.lookahead_span(),
                                message: "class decorator factory arguments are not supported"
                                    .into(),
                            });
                        }
                        self.expect_punct(Punct::RParen)?;
                    }
                    decorators.push(name);
                    continue;
                }
            }

            let _ = self.parse_left_hand_side_expression()?;
            decorators.push("<decorator>".to_string());
        }
        Ok(decorators)
    }

    fn parse_class_decl_stmt_with_decorators(
        &mut self,
        decorators: Vec<String>,
    ) -> Result<Stmt, ParseError> {
        let start = self.lookahead_span().start;
        self.expect_keyword("class")?;
        let name = if let TokenKind::Ident(n) = self.current_kind().clone() {
            let _top_profile =
                crate::parser::parse_profile::FunctionBodyTopClassDeclPhaseGuard::new(
                    crate::parser::parse_profile::ClassDeclPhase::Name,
                );
            if n != "extends" {
                let span = self.lookahead_span();

                if crate::parser::is_unconditional_reserved_word(&n) {
                    return Err(ParseError {
                        span,
                        message: format!(
                            "`{}` is a reserved word and cannot be used as a class name",
                            n
                        ),
                    });
                }

                if crate::parser::is_strict_reserved_word(&n) {
                    return Err(ParseError {
                        span,
                        message: format!(
                            "`{}` is a reserved word and cannot be used as a class name",
                            n
                        ),
                    });
                }
                if n == "yield" {
                    return Err(ParseError {
                        span,
                        message: "`yield` is not a valid class name".into(),
                    });
                }
                if (self.in_async || self.is_module_goal()) && n == "await" {
                    return Err(ParseError {
                        span,
                        message: "`await` is not a valid class name in async or module code".into(),
                    });
                }
                self.bump()?;
                Some(BindingIdentifier { name: n, span })
            } else {
                None
            }
        } else {
            None
        };
        let super_class = if self.is_ident("extends") {
            let _top_profile =
                crate::parser::parse_profile::FunctionBodyTopClassDeclPhaseGuard::new(
                    crate::parser::parse_profile::ClassDeclPhase::Super,
                );
            self.bump()?;

            let prior_strict = self.strict_mode;
            self.strict_mode = true;
            let heritage = self.parse_left_hand_side_expression();
            self.strict_mode = prior_strict;
            Some(heritage?)
        } else {
            None
        };

        self.pending_fn_body_close_regexp = true;
        let members = {
            let _top_profile =
                crate::parser::parse_profile::FunctionBodyTopClassDeclPhaseGuard::new(
                    crate::parser::parse_profile::ClassDeclPhase::Body,
                );
            self.parse_class_body()?
        };
        let _top_finish_profile =
            crate::parser::parse_profile::FunctionBodyTopClassDeclPhaseGuard::new(
                crate::parser::parse_profile::ClassDeclPhase::Finish,
            );
        let end = self.last_span_end();
        Ok(Stmt::ClassDecl {
            decorators,
            name,
            super_class,
            members,
            span: Span::new(start, end),
        })
    }

    pub(crate) fn parse_class_body(
        &mut self,
    ) -> Result<Vec<rusty_js_ast::ClassMember>, ParseError> {
        let _profile =
            crate::parser::parse_profile::Guard::new(crate::parser::parse_profile::Kind::ClassBody);

        let prior_strict = self.strict_mode;
        let setup_start =
            crate::parser::parse_profile::top_var_init_ident_arrow_expr_cond_classbody_phase_start(
            );
        self.strict_mode = true;
        crate::parser::parse_profile::record_top_var_init_ident_arrow_expr_cond_classbody_phase(
            crate::parser::parse_profile::TopVarInitIdentArrowExprCondClassBodyPhase::Setup,
            setup_start,
        );
        let result = self.parse_class_body_inner();
        self.strict_mode = prior_strict;
        result
    }

    fn parse_class_body_inner(&mut self) -> Result<Vec<rusty_js_ast::ClassMember>, ParseError> {
        use rusty_js_ast::{ClassMember, ClassMemberName, MethodKind};

        let close_under_regexp = std::mem::take(&mut self.pending_fn_body_close_regexp);
        self.expect_punct(Punct::LBrace)?;
        let mut out = Vec::new();
        let member_loop_start =
            crate::parser::parse_profile::top_var_init_ident_arrow_expr_cond_classbody_phase_start(
            );
        while !matches!(self.current_kind(), TokenKind::Punct(Punct::RBrace))
            && !self.at_eof_internal()
        {

            if matches!(self.current_kind(), TokenKind::Punct(Punct::Semicolon)) {
                self.bump()?;
                continue;
            }

            if matches!(self.current_kind(), TokenKind::Punct(Punct::At)) {
                let _ = self.parse_class_decorators()?;
            }
            let m_start = self.lookahead_span().start;
            let is_static = if self.is_ident("static") {

                let pos = self.lookahead_span().end;
                let bytes = self.source().as_bytes();
                let mut p = pos;
                while p < bytes.len() && bytes[p].is_ascii_whitespace() {
                    p += 1;
                }
                let next = bytes.get(p).copied();
                if next == Some(b'{') {
                    let _top_member_profile =
                        crate::parser::parse_profile::FunctionBodyTopClassMemberFamilyGuard::new(
                            crate::parser::parse_profile::ClassMemberFamily::StaticBlock,
                        );
                    let _scoped_member_profile = crate::parser::parse_profile::TopVarInitIdentArrowExprCondClassMemberFamilyGuard::new(
                        crate::parser::parse_profile::ClassMemberFamily::StaticBlock,
                    );

                    self.reject_escaped_contextual_keyword("static")?;
                    self.bump()?;

                    let prior_strict = self.strict_mode;
                    let prior_gen = self.in_generator;
                    self.strict_mode = true;
                    self.set_lexer_strict(true);
                    self.in_generator = false;
                    let body_result = self.parse_function_body_gs(Some(false), Some(false), true);
                    self.strict_mode = prior_strict;
                    self.set_lexer_strict(prior_strict);
                    self.in_generator = prior_gen;
                    let body = body_result?;
                    let end = self.last_span_end();
                    out.push(ClassMember::StaticBlock {
                        body,
                        span: Span::new(m_start, end),
                    });
                    continue;
                }

                if matches!(next, Some(b'(') | Some(b'=') | Some(b';') | Some(b'}')) {
                    false
                } else {

                    self.reject_escaped_contextual_keyword("static")?;
                    self.bump()?;
                    true
                }
            } else {
                false
            };

            if self.is_ident("accessor") && !self.next_is_method_open_or_field_terminator() {
                let _top_member_profile =
                    crate::parser::parse_profile::FunctionBodyTopClassMemberFamilyGuard::new(
                        crate::parser::parse_profile::ClassMemberFamily::Accessor,
                    );
                let _scoped_member_profile = crate::parser::parse_profile::TopVarInitIdentArrowExprCondClassMemberFamilyGuard::new(
                    crate::parser::parse_profile::ClassMemberFamily::Accessor,
                );
                self.reject_escaped_contextual_keyword("accessor")?;
                self.bump()?;
                let acc_name = self.parse_class_member_name()?;
                let init = if matches!(self.current_kind(), TokenKind::Punct(Punct::Assign)) {
                    self.bump()?;
                    Some(self.parse_class_field_initializer_expression()?)
                } else {
                    None
                };
                self.consume_class_field_terminator()?;
                let end = self.last_span_end();
                let sp = Span::new(m_start, end);

                let backing = format!("__aa_{}", m_start);
                let backing_member = || rusty_js_ast::Expr::Member {
                    object: Box::new(rusty_js_ast::Expr::This { span: sp }),
                    property: Box::new(rusty_js_ast::MemberProperty::Private {
                        name: backing.clone(),
                        span: sp,
                    }),
                    optional: false,
                    span: sp,
                };

                out.push(ClassMember::Field {
                    name: ClassMemberName::Private {
                        name: backing.clone(),
                        span: sp,
                    },
                    is_static,
                    init,
                    span: sp,
                });

                out.push(ClassMember::Method {
                    name: acc_name.clone(),
                    kind: MethodKind::Getter,
                    is_static,
                    is_async: false,
                    is_generator: false,
                    params: Vec::new(),
                    body: vec![rusty_js_ast::Stmt::Return {
                        argument: Some(backing_member()),
                        span: sp,
                    }],
                    span: sp,
                });

                out.push(ClassMember::Method {
                    name: acc_name,
                    kind: MethodKind::Setter,
                    is_static,
                    is_async: false,
                    is_generator: false,
                    params: vec![rusty_js_ast::Parameter {
                        target: rusty_js_ast::BindingPattern::Identifier(
                            rusty_js_ast::BindingIdentifier {
                                name: "value".to_string(),
                                span: sp,
                            },
                        ),
                        default: None,
                        rest: false,
                        span: sp,
                    }],
                    body: vec![rusty_js_ast::Stmt::Expression {
                        expr: rusty_js_ast::Expr::Assign {
                            operator: rusty_js_ast::AssignOp::Assign,
                            target: Box::new(backing_member()),
                            value: Box::new(rusty_js_ast::Expr::Identifier {
                                name: "value".to_string(),
                                span: sp,
                            }),
                            span: sp,
                        },
                        span: sp,
                    }],
                    span: sp,
                });
                continue;
            }

            let mut kind = MethodKind::Method;
            let mut is_async = false;
            let mut is_generator = false;

            if self.is_ident("get") {

                if !self.next_is_method_open_or_field_terminator() {
                    self.reject_escaped_contextual_keyword("get")?;
                    self.bump()?;
                    kind = MethodKind::Getter;
                }
            } else if self.is_ident("set") {
                if !self.next_is_method_open_or_field_terminator() {
                    self.reject_escaped_contextual_keyword("set")?;
                    self.bump()?;
                    kind = MethodKind::Setter;
                }
            } else if self.is_ident("async") {
                if !self.next_is_method_open_or_field_terminator() {
                    self.reject_escaped_contextual_keyword("async")?;
                    self.bump()?;
                    is_async = true;
                }
            }
            if matches!(self.current_kind(), TokenKind::Punct(Punct::Star)) {
                is_generator = true;
                self.bump()?;
            }

            let name = self.parse_class_member_name()?;

            if matches!(self.current_kind(), TokenKind::Punct(Punct::LParen)) {
                let _top_member_profile =
                    crate::parser::parse_profile::FunctionBodyTopClassMemberFamilyGuard::new(
                        crate::parser::parse_profile::ClassMemberFamily::Method,
                    );
                let _scoped_member_profile = crate::parser::parse_profile::TopVarInitIdentArrowExprCondClassMemberFamilyGuard::new(
                    crate::parser::parse_profile::ClassMemberFamily::Method,
                );

                let params = {
                    let _top_method_phase_profile =
                        crate::parser::parse_profile::FunctionBodyTopClassMethodPhaseGuard::new(
                            crate::parser::parse_profile::ClassMethodPhase::Params,
                        );
                    let _scoped_method_phase_profile = crate::parser::parse_profile::TopVarInitIdentArrowExprCondClassMethodPhaseGuard::new(
                        crate::parser::parse_profile::ClassMethodPhase::Params,
                    );
                    self.parse_unique_formal_parameters_ga(is_generator, is_async)?
                };
                let body = {
                    let _top_method_phase_profile =
                        crate::parser::parse_profile::FunctionBodyTopClassMethodPhaseGuard::new(
                            crate::parser::parse_profile::ClassMethodPhase::Body,
                        );
                    let _scoped_method_phase_profile = crate::parser::parse_profile::TopVarInitIdentArrowExprCondClassMethodPhaseGuard::new(
                        crate::parser::parse_profile::ClassMethodPhase::Body,
                    );
                    let _top_method_body_profile =
                        crate::parser::parse_profile::FunctionBodyTopClassMethodBodyGuard::new();
                    self.parse_function_body_gs(
                        Some(is_generator),
                        Some(is_async),
                        Self::is_simple_param_list(&params),
                    )?
                };
                let end = self.last_span_end();

                let method_kind = {
                    let _top_method_phase_profile =
                        crate::parser::parse_profile::FunctionBodyTopClassMethodPhaseGuard::new(
                            crate::parser::parse_profile::ClassMethodPhase::KindValidate,
                        );
                    let _scoped_method_phase_profile = crate::parser::parse_profile::TopVarInitIdentArrowExprCondClassMethodPhaseGuard::new(
                        crate::parser::parse_profile::ClassMethodPhase::KindValidate,
                    );
                    let method_kind = if !is_static && kind == MethodKind::Method {
                        if matches!(
                            &name,
                            ClassMemberName::Identifier { name: n, .. }
                                | ClassMemberName::String { value: n, .. }
                                if n == "constructor"
                        ) {
                            MethodKind::Constructor
                        } else {
                            MethodKind::Method
                        }
                    } else {
                        kind
                    };
                    self.validate_accessor_parameters(method_kind, &params)?;
                    method_kind
                };

                {
                    let _top_method_phase_profile =
                        crate::parser::parse_profile::FunctionBodyTopClassMethodPhaseGuard::new(
                            crate::parser::parse_profile::ClassMethodPhase::StrictRevalidate,
                        );
                    let _scoped_method_phase_profile = crate::parser::parse_profile::TopVarInitIdentArrowExprCondClassMethodPhaseGuard::new(
                        crate::parser::parse_profile::ClassMethodPhase::StrictRevalidate,
                    );
                    self.revalidate_params_after_strict_promotion(&params, None)?;
                }
                {
                    let _top_method_phase_profile =
                        crate::parser::parse_profile::FunctionBodyTopClassMethodPhaseGuard::new(
                            crate::parser::parse_profile::ClassMethodPhase::Push,
                        );
                    let _scoped_method_phase_profile = crate::parser::parse_profile::TopVarInitIdentArrowExprCondClassMethodPhaseGuard::new(
                        crate::parser::parse_profile::ClassMethodPhase::Push,
                    );
                    out.push(ClassMember::Method {
                        name,
                        kind: method_kind,
                        is_static,
                        is_async,
                        is_generator,
                        params,
                        body,
                        span: Span::new(m_start, end),
                    });
                }
                continue;
            }

            let _top_member_profile =
                crate::parser::parse_profile::FunctionBodyTopClassMemberFamilyGuard::new(
                    crate::parser::parse_profile::ClassMemberFamily::Field,
                );
            let _scoped_member_profile = crate::parser::parse_profile::TopVarInitIdentArrowExprCondClassMemberFamilyGuard::new(
                crate::parser::parse_profile::ClassMemberFamily::Field,
            );
            let init = if matches!(self.current_kind(), TokenKind::Punct(Punct::Assign)) {
                self.bump()?;
                Some(self.parse_class_field_initializer_expression()?)
            } else {
                None
            };
            self.consume_class_field_terminator()?;
            let end = self.last_span_end();
            out.push(ClassMember::Field {
                name,
                is_static,
                init,
                span: Span::new(m_start, end),
            });
        }
        crate::parser::parse_profile::record_top_var_init_ident_arrow_expr_cond_classbody_phase(
            crate::parser::parse_profile::TopVarInitIdentArrowExprCondClassBodyPhase::MemberLoop,
            member_loop_start,
        );

        let close_start =
            crate::parser::parse_profile::top_var_init_ident_arrow_expr_cond_classbody_phase_start(
            );
        if close_under_regexp {
            self.consume_statement_rbrace()?;
        } else {
            self.expect_punct(Punct::RBrace)?;
        }
        crate::parser::parse_profile::record_top_var_init_ident_arrow_expr_cond_classbody_phase(
            crate::parser::parse_profile::TopVarInitIdentArrowExprCondClassBodyPhase::Close,
            close_start,
        );
        let validate_start =
            crate::parser::parse_profile::top_var_init_ident_arrow_expr_cond_classbody_phase_start(
            );
        self.validate_class_static_semantics(&out)?;
        crate::parser::parse_profile::record_top_var_init_ident_arrow_expr_cond_classbody_phase(
            crate::parser::parse_profile::TopVarInitIdentArrowExprCondClassBodyPhase::Validate,
            validate_start,
        );
        Ok(out)
    }

    fn parse_class_field_initializer_expression(
        &mut self,
    ) -> Result<rusty_js_ast::Expr, ParseError> {
        let prior_in_async = self.in_async;
        let prior_function_body_depth = self.function_body_depth;
        self.in_async = false;
        self.function_body_depth += 1;
        let result = self.parse_assignment_expression();
        self.in_async = prior_in_async;
        self.function_body_depth = prior_function_body_depth;
        result
    }

    fn validate_class_static_semantics(
        &self,
        members: &[rusty_js_ast::ClassMember],
    ) -> Result<(), ParseError> {
        use rusty_js_ast::{ClassMember, ClassMemberName, MethodKind};
        let mut private_names = std::collections::HashSet::new();

        let mut private_getters = std::collections::HashMap::new();
        let mut private_setters = std::collections::HashMap::new();
        let mut constructor_seen = false;
        for m in members {
            match m {
                ClassMember::Field {
                    name,
                    is_static,
                    span,
                    ..
                } => {
                    let prop_name = self.class_member_prop_name(name);
                    if prop_name == Some("constructor")
                        || (*is_static && prop_name == Some("prototype"))
                    {
                        return Err(ParseError {
                            message: "class field name is an early-error reserved name".into(),
                            span: *span,
                        });
                    }
                    if let ClassMemberName::Private { name, .. } = name {

                        if private_getters.contains_key(name.as_str())
                            || private_setters.contains_key(name.as_str())
                            || !private_names.insert(name.as_str())
                        {
                            return Err(ParseError {
                                message: format!("duplicate private name #{}", name),
                                span: *span,
                            });
                        }
                    }
                }
                ClassMember::Method {
                    name,
                    kind,
                    span,
                    is_static,
                    is_async,
                    is_generator,
                    ..
                } => {
                    let prop_name = self.class_member_prop_name(name);

                    if *is_static && prop_name == Some("prototype") {
                        return Err(ParseError {
                            message: "class static method may not be named 'prototype'".into(),
                            span: *span,
                        });
                    }

                    if !*is_static && prop_name == Some("constructor") {
                        if matches!(kind, MethodKind::Getter | MethodKind::Setter)
                            || *is_async
                            || *is_generator
                        {
                            return Err(ParseError {
                                message: "class constructor may not be an accessor, generator, or async method".into(),
                                span: *span,
                            });
                        }

                        if constructor_seen {
                            return Err(ParseError {
                                message: "class may not have more than one constructor".into(),
                                span: *span,
                            });
                        }
                        constructor_seen = true;
                    }
                    if let ClassMemberName::Private { name, .. } = name {
                        match kind {
                            MethodKind::Getter => {
                                if private_names.contains(name.as_str())
                                    || private_getters.contains_key(name.as_str())
                                {
                                    return Err(ParseError {
                                        message: format!("duplicate private name #{}", name),
                                        span: *span,
                                    });
                                }

                                if let Some(&setter_static) = private_setters.get(name.as_str()) {
                                    if setter_static != *is_static {
                                        return Err(ParseError {
                                            message: format!(
                                                "private accessor #{} has a getter and setter with mismatched static placement",
                                                name
                                            ),
                                            span: *span,
                                        });
                                    }
                                }
                                private_getters.insert(name.as_str(), *is_static);
                            }
                            MethodKind::Setter => {
                                if private_names.contains(name.as_str())
                                    || private_setters.contains_key(name.as_str())
                                {
                                    return Err(ParseError {
                                        message: format!("duplicate private name #{}", name),
                                        span: *span,
                                    });
                                }
                                if let Some(&getter_static) = private_getters.get(name.as_str()) {
                                    if getter_static != *is_static {
                                        return Err(ParseError {
                                            message: format!(
                                                "private accessor #{} has a getter and setter with mismatched static placement",
                                                name
                                            ),
                                            span: *span,
                                        });
                                    }
                                }
                                private_setters.insert(name.as_str(), *is_static);
                            }
                            MethodKind::Method | MethodKind::Constructor => {
                                if private_getters.contains_key(name.as_str())
                                    || private_setters.contains_key(name.as_str())
                                    || !private_names.insert(name.as_str())
                                {
                                    return Err(ParseError {
                                        message: format!("duplicate private name #{}", name),
                                        span: *span,
                                    });
                                }
                            }
                        }
                    }
                }
                ClassMember::StaticBlock { .. } => {}
            }
        }

        let mut declared_private_names = private_names.clone();
        declared_private_names.extend(private_getters.keys().copied());
        declared_private_names.extend(private_setters.keys().copied());
        for m in members {
            match m {
                ClassMember::Field {
                    name, init, span, ..
                } => {
                    if let Some(init) = init {
                        if self.expr_contains_arguments(init) {
                            return Err(ParseError {
                                message: "class field initializer cannot contain arguments".into(),
                                span: *span,
                            });
                        }
                        if self.expr_contains_private_delete(init) {
                            return Err(ParseError {
                                message: "private name cannot be the operand of delete".into(),
                                span: *span,
                            });
                        }
                    }
                    if let ClassMemberName::Computed { expr, span } = name {
                        if let Some(missing) =
                            self.first_unbound_private_name(expr, &declared_private_names)
                        {
                            return Err(ParseError {
                                message: format!("PrivateName #{} is not declared", missing),
                                span: *span,
                            });
                        }
                    }
                }
                ClassMember::Method {
                    name, body, span, ..
                } => {
                    if self.stmts_contain_private_delete(body) {
                        return Err(ParseError {
                            message: "private name cannot be the operand of delete".into(),
                            span: *span,
                        });
                    }
                    if let ClassMemberName::Computed { expr, span } = name {
                        if let Some(missing) =
                            self.first_unbound_private_name(expr, &declared_private_names)
                        {
                            return Err(ParseError {
                                message: format!("PrivateName #{} is not declared", missing),
                                span: *span,
                            });
                        }
                    }
                }
                ClassMember::StaticBlock { body, span } => {
                    if self.stmts_contain_arguments(body) {
                        return Err(ParseError {
                            message: "class static block cannot contain arguments".into(),
                            span: *span,
                        });
                    }
                    if self.stmts_contain_await(body) {
                        return Err(ParseError {
                            message: "class static block cannot contain await".into(),
                            span: *span,
                        });
                    }

                    if self.stmts_contain_return(body) {
                        return Err(ParseError {
                            message: "class static block cannot contain a return statement".into(),
                            span: *span,
                        });
                    }
                    self.validate_class_static_block_names(body, *span)?;
                }
            }
        }
        Ok(())
    }

    fn class_member_prop_name<'a>(
        &self,
        name: &'a rusty_js_ast::ClassMemberName,
    ) -> Option<&'a str> {
        use rusty_js_ast::ClassMemberName;
        match name {
            ClassMemberName::Identifier { name, .. }
            | ClassMemberName::String { value: name, .. } => Some(name.as_str()),
            ClassMemberName::Number { .. }
            | ClassMemberName::Computed { .. }
            | ClassMemberName::Private { .. } => None,
        }
    }

    fn validate_class_static_block_names(
        &self,
        body: &[rusty_js_ast::Stmt],
        span: Span,
    ) -> Result<(), ParseError> {
        let mut labels = std::collections::HashSet::new();
        if self.stmts_contain_duplicate_label(body, &mut labels) {
            return Err(ParseError {
                message: "class static block contains duplicate label".into(),
                span,
            });
        }

        let mut lexical_names = Vec::new();
        let mut var_names = Vec::new();
        self.collect_static_block_declared_names(body, &mut lexical_names, &mut var_names);

        let mut seen_lex = std::collections::HashSet::new();
        for name in &lexical_names {
            if !seen_lex.insert(name.as_str()) {
                return Err(ParseError {
                    message: format!(
                        "duplicate lexical declaration `{}` in class static block",
                        name
                    ),
                    span,
                });
            }
        }
        if lexical_names
            .iter()
            .any(|lex| var_names.iter().any(|var| var == lex))
        {
            return Err(ParseError {
                message: "class static block lexical declaration conflicts with var declaration"
                    .into(),
                span,
            });
        }
        Ok(())
    }

    fn expr_contains_arguments(&self, expr: &rusty_js_ast::Expr) -> bool {
        use rusty_js_ast::{Argument, ArrowBody, Expr, MemberProperty, ObjectKey, ObjectProperty};
        match expr {
            Expr::Identifier { name, .. } => name == "arguments",
            Expr::Array { elements, .. } => elements.iter().any(|e| match e {
                rusty_js_ast::ArrayElement::Expr(e)
                | rusty_js_ast::ArrayElement::Spread { expr: e, .. } => {
                    self.expr_contains_arguments(e)
                }
                rusty_js_ast::ArrayElement::Elision { .. } => false,
            }),
            Expr::Object { properties, .. } => properties.iter().any(|p| match p {
                ObjectProperty::Property { key, value, .. } => {
                    matches!(key, ObjectKey::Computed { expr, .. } if self.expr_contains_arguments(expr))
                        || self.expr_contains_arguments(value)
                }
                ObjectProperty::Spread { expr, .. } => self.expr_contains_arguments(expr),
            }),
            Expr::Parenthesized { expr, .. }
            | Expr::Update { argument: expr, .. }
            | Expr::Unary { argument: expr, .. } => self.expr_contains_arguments(expr),
            Expr::Member {
                object, property, ..
            } => {
                self.expr_contains_arguments(object)
                    || matches!(
                        property.as_ref(),
                        MemberProperty::Computed { expr, .. } if self.expr_contains_arguments(expr)
                    )
            }
            Expr::Call {
                callee, arguments, ..
            }
            | Expr::New {
                callee, arguments, ..
            } => {
                self.expr_contains_arguments(callee)
                    || arguments.iter().any(|a| match a {
                        Argument::Expr(e) | Argument::Spread { expr: e, .. } => {
                            self.expr_contains_arguments(e)
                        }
                    })
            }
            Expr::Binary { left, right, .. } | Expr::Assign { target: left, value: right, .. } => {
                self.expr_contains_arguments(left) || self.expr_contains_arguments(right)
            }
            Expr::Conditional {
                test,
                consequent,
                alternate,
                ..
            } => {
                self.expr_contains_arguments(test)
                    || self.expr_contains_arguments(consequent)
                    || self.expr_contains_arguments(alternate)
            }
            Expr::Sequence { expressions, .. } => {
                expressions.iter().any(|e| self.expr_contains_arguments(e))
            }
            Expr::Arrow { body, .. } => match body {
                ArrowBody::Expression(e) => self.expr_contains_arguments(e),
                ArrowBody::Block(stmts) => self.stmts_contain_arguments(stmts),
            },
            Expr::TemplateLiteral { expressions, .. } => {
                expressions.iter().any(|e| self.expr_contains_arguments(e))
            }
            Expr::Class {
                name,
                super_class,
                members,
                ..
            } => {
                name.as_ref().is_some_and(|n| n.name == "arguments")
                    || super_class
                        .as_deref()
                        .is_some_and(|e| self.expr_contains_arguments(e))
                    || members
                        .iter()
                        .any(|m| self.class_member_contains_arguments(m))
            }
            Expr::Function { .. }
            | Expr::NullLiteral { .. }
            | Expr::BoolLiteral { .. }
            | Expr::NumberLiteral { .. }
            | Expr::BigIntLiteral { .. }
            | Expr::StringLiteral { .. }
            | Expr::WtfStringLiteral { .. }
            | Expr::This { .. }
            | Expr::Super { .. }
            | Expr::MetaProperty { .. }
            | Expr::TemplateObject { .. }
            | Expr::RegExp { .. }
            | Expr::Opaque { .. } => false,
        }
    }

    fn class_member_contains_arguments(&self, member: &rusty_js_ast::ClassMember) -> bool {
        use rusty_js_ast::{ClassMember, ClassMemberName};
        match member {
            ClassMember::Field { name, init, .. } => {
                matches!(name, ClassMemberName::Computed { expr, .. } if self.expr_contains_arguments(expr))
                    || init
                        .as_ref()
                        .is_some_and(|expr| self.expr_contains_arguments(expr))
            }
            ClassMember::Method { name, .. } => {
                matches!(name, ClassMemberName::Computed { expr, .. } if self.expr_contains_arguments(expr))
            }
            ClassMember::StaticBlock { .. } => false,
        }
    }

    fn stmts_contain_return(&self, stmts: &[rusty_js_ast::Stmt]) -> bool {
        stmts.iter().any(|s| self.stmt_contains_return(s))
    }

    fn stmt_contains_return(&self, s: &rusty_js_ast::Stmt) -> bool {
        use rusty_js_ast::Stmt;
        match s {
            Stmt::Return { .. } => true,
            Stmt::Block { body, .. } => self.stmts_contain_return(body),
            Stmt::If {
                consequent,
                alternate,
                ..
            } => {
                self.stmt_contains_return(consequent)
                    || alternate
                        .as_ref()
                        .is_some_and(|a| self.stmt_contains_return(a))
            }
            Stmt::For { body, .. }
            | Stmt::ForIn { body, .. }
            | Stmt::ForOf { body, .. }
            | Stmt::While { body, .. }
            | Stmt::DoWhile { body, .. }
            | Stmt::With { body, .. }
            | Stmt::Labelled { body, .. } => self.stmt_contains_return(body),
            Stmt::Try {
                block,
                handler,
                finalizer,
                ..
            } => {
                self.stmt_contains_return(block)
                    || handler
                        .as_ref()
                        .is_some_and(|h| self.stmt_contains_return(&h.body))
                    || finalizer
                        .as_ref()
                        .is_some_and(|f| self.stmt_contains_return(f))
            }
            Stmt::Switch { cases, .. } => cases
                .iter()
                .any(|c| c.consequent.iter().any(|s| self.stmt_contains_return(s))),

            _ => false,
        }
    }

    fn stmts_contain_arguments(&self, stmts: &[rusty_js_ast::Stmt]) -> bool {
        use rusty_js_ast::Stmt;
        stmts.iter().any(|s| match s {
            Stmt::Variable(v) => v.declarators.iter().any(|d| {
                d.init
                    .as_ref()
                    .is_some_and(|expr| self.expr_contains_arguments(expr))
            }),
            Stmt::Expression { expr, .. } => self.expr_contains_arguments(expr),
            Stmt::Block { body, .. } => self.stmts_contain_arguments(body),
            Stmt::ClassDecl {
                name,
                super_class,
                members,
                ..
            } => {
                name.as_ref().is_some_and(|n| n.name == "arguments")
                    || super_class
                        .as_ref()
                        .is_some_and(|e| self.expr_contains_arguments(e))
                    || members
                        .iter()
                        .any(|m| self.class_member_contains_arguments(m))
            }
            Stmt::If {
                test,
                consequent,
                alternate,
                ..
            } => {
                self.expr_contains_arguments(test)
                    || self.stmt_contains_arguments(consequent)
                    || alternate
                        .as_deref()
                        .is_some_and(|s| self.stmt_contains_arguments(s))
            }
            Stmt::Return { argument, .. } => argument
                .as_ref()
                .is_some_and(|e| self.expr_contains_arguments(e)),
            Stmt::Throw { argument, .. } => self.expr_contains_arguments(argument),
            _ => false,
        })
    }

    fn stmt_contains_arguments(&self, stmt: &rusty_js_ast::Stmt) -> bool {
        self.stmts_contain_arguments(std::slice::from_ref(stmt))
    }

    fn expr_contains_await(&self, expr: &rusty_js_ast::Expr) -> bool {
        use rusty_js_ast::{Argument, Expr, MemberProperty, ObjectKey, ObjectProperty, UnaryOp};
        match expr {
            Expr::Identifier { name, .. } => name == "await",
            Expr::Unary {
                operator: UnaryOp::Await,
                ..
            } => true,
            Expr::Array { elements, .. } => elements.iter().any(|e| match e {
                rusty_js_ast::ArrayElement::Expr(e)
                | rusty_js_ast::ArrayElement::Spread { expr: e, .. } => self.expr_contains_await(e),
                rusty_js_ast::ArrayElement::Elision { .. } => false,
            }),
            Expr::Object { properties, .. } => properties.iter().any(|p| match p {
                ObjectProperty::Property { key, value, .. } => {
                    matches!(key, ObjectKey::Computed { expr, .. } if self.expr_contains_await(expr))
                        || self.expr_contains_await(value)
                }
                ObjectProperty::Spread { expr, .. } => self.expr_contains_await(expr),
            }),
            Expr::Parenthesized { expr, .. }
            | Expr::Update { argument: expr, .. }
            | Expr::Unary { argument: expr, .. } => self.expr_contains_await(expr),
            Expr::Member {
                object, property, ..
            } => {
                self.expr_contains_await(object)
                    || matches!(
                        property.as_ref(),
                        MemberProperty::Computed { expr, .. } if self.expr_contains_await(expr)
                    )
            }
            Expr::Call {
                callee, arguments, ..
            }
            | Expr::New {
                callee, arguments, ..
            } => {
                self.expr_contains_await(callee)
                    || arguments.iter().any(|a| match a {
                        Argument::Expr(e) | Argument::Spread { expr: e, .. } => {
                            self.expr_contains_await(e)
                        }
                    })
            }
            Expr::Binary { left, right, .. } | Expr::Assign { target: left, value: right, .. } => {
                self.expr_contains_await(left) || self.expr_contains_await(right)
            }
            Expr::Conditional {
                test,
                consequent,
                alternate,
                ..
            } => {
                self.expr_contains_await(test)
                    || self.expr_contains_await(consequent)
                    || self.expr_contains_await(alternate)
            }
            Expr::Sequence { expressions, .. } | Expr::TemplateLiteral { expressions, .. } => {
                expressions.iter().any(|e| self.expr_contains_await(e))
            }
            Expr::Arrow { params, .. } => {

                params.iter().any(|p| {
                    self.binding_pattern_contains_name(&p.target, "await")
                        || p.default
                            .as_ref()
                            .is_some_and(|e| self.expr_contains_await(e))
                })
            }
            Expr::Class {
                name,
                super_class,
                members,
                ..
            } => {
                name.as_ref().is_some_and(|n| n.name == "await")
                    || super_class
                        .as_deref()
                        .is_some_and(|e| self.expr_contains_await(e))
                    || members.iter().any(|m| self.class_member_contains_await(m))
            }
            Expr::Function { .. }
            | Expr::NullLiteral { .. }
            | Expr::BoolLiteral { .. }
            | Expr::NumberLiteral { .. }
            | Expr::BigIntLiteral { .. }
            | Expr::StringLiteral { .. }
            | Expr::WtfStringLiteral { .. }
            | Expr::This { .. }
            | Expr::Super { .. }
            | Expr::MetaProperty { .. }
            | Expr::TemplateObject { .. }
            | Expr::RegExp { .. }
            | Expr::Opaque { .. } => false,
        }
    }

    fn class_member_contains_await(&self, member: &rusty_js_ast::ClassMember) -> bool {
        use rusty_js_ast::{ClassMember, ClassMemberName};
        match member {
            ClassMember::Field { name, init, .. } => {
                matches!(name, ClassMemberName::Computed { expr, .. } if self.expr_contains_await(expr))
                    || init
                        .as_ref()
                        .is_some_and(|expr| self.expr_contains_await(expr))
            }
            ClassMember::Method { name, .. } => {
                matches!(name, ClassMemberName::Computed { expr, .. } if self.expr_contains_await(expr))
            }
            ClassMember::StaticBlock { .. } => false,
        }
    }

    fn binding_pattern_contains_name(
        &self,
        pattern: &rusty_js_ast::BindingPattern,
        needle: &str,
    ) -> bool {
        pattern
            .collect_names()
            .iter()
            .any(|name| name.name == needle)
    }

    fn stmts_contain_await(&self, stmts: &[rusty_js_ast::Stmt]) -> bool {
        use rusty_js_ast::{ForBinding, ForInit, Stmt};
        stmts.iter().any(|s| match s {
            Stmt::Variable(v) => v.declarators.iter().any(|d| {
                self.binding_pattern_contains_name(&d.target, "await")
                    || d.init
                        .as_ref()
                        .is_some_and(|expr| self.expr_contains_await(expr))
            }),
            Stmt::FunctionDecl { name, .. } => {
                name.as_ref().is_some_and(|name| name.name == "await")
            }
            Stmt::Labelled { label, body, .. } => {
                label.name == "await" || self.stmt_contains_await(body)
            }
            Stmt::For {
                init,
                test,
                update,
                body,
                ..
            } => {
                init.as_ref().is_some_and(|init| match init {
                    ForInit::Variable(v) => v.declarators.iter().any(|d| {
                        self.binding_pattern_contains_name(&d.target, "await")
                            || d.init
                                .as_ref()
                                .is_some_and(|expr| self.expr_contains_await(expr))
                    }),
                    ForInit::Expression(expr) => self.expr_contains_await(expr),
                }) || test
                    .as_ref()
                    .is_some_and(|expr| self.expr_contains_await(expr))
                    || update
                        .as_ref()
                        .is_some_and(|expr| self.expr_contains_await(expr))
                    || self.stmt_contains_await(body)
            }
            Stmt::ForIn {
                left, right, body, ..
            }
            | Stmt::ForOf {
                left, right, body, ..
            } => {
                (match left {
                    ForBinding::Decl { target, .. } | ForBinding::Pattern(target) => {
                        self.binding_pattern_contains_name(target, "await")
                    }
                    ForBinding::AssignmentTarget(expr) => self.expr_contains_await(expr),
                }) || self.expr_contains_await(right)
                    || self.stmt_contains_await(body)
            }
            Stmt::Expression { expr, .. } => self.expr_contains_await(expr),
            Stmt::Block { body, .. } => self.stmts_contain_await(body),
            Stmt::ClassDecl {
                name,
                super_class,
                members,
                ..
            } => {
                name.as_ref().is_some_and(|n| n.name == "await")
                    || super_class
                        .as_ref()
                        .is_some_and(|e| self.expr_contains_await(e))
                    || members.iter().any(|m| self.class_member_contains_await(m))
            }
            Stmt::If {
                test,
                consequent,
                alternate,
                ..
            } => {
                self.expr_contains_await(test)
                    || self.stmt_contains_await(consequent)
                    || alternate
                        .as_deref()
                        .is_some_and(|s| self.stmt_contains_await(s))
            }
            Stmt::Return { argument, .. } => argument
                .as_ref()
                .is_some_and(|e| self.expr_contains_await(e)),
            Stmt::Throw { argument, .. } => self.expr_contains_await(argument),
            Stmt::Try {
                block,
                handler,
                finalizer,
                ..
            } => {
                self.stmt_contains_await(block)
                    || handler.as_ref().is_some_and(|handler| {
                        handler
                            .param
                            .as_ref()
                            .is_some_and(|param| self.binding_pattern_contains_name(param, "await"))
                            || self.stmt_contains_await(&handler.body)
                    })
                    || finalizer
                        .as_deref()
                        .is_some_and(|stmt| self.stmt_contains_await(stmt))
            }
            _ => false,
        })
    }

    fn stmt_contains_await(&self, stmt: &rusty_js_ast::Stmt) -> bool {
        self.stmts_contain_await(std::slice::from_ref(stmt))
    }

    fn stmts_contain_duplicate_label(
        &self,
        stmts: &[rusty_js_ast::Stmt],
        labels: &mut std::collections::HashSet<String>,
    ) -> bool {
        use rusty_js_ast::Stmt;
        stmts.iter().any(|stmt| match stmt {
            Stmt::Labelled { label, body, .. } => {
                !labels.insert(label.name.clone())
                    || self.stmts_contain_duplicate_label(std::slice::from_ref(body), labels)
            }
            Stmt::Block { body, .. } => self.stmts_contain_duplicate_label(body, labels),
            Stmt::If {
                consequent,
                alternate,
                ..
            } => {
                self.stmts_contain_duplicate_label(std::slice::from_ref(consequent), labels)
                    || alternate.as_deref().is_some_and(|stmt| {
                        self.stmts_contain_duplicate_label(std::slice::from_ref(stmt), labels)
                    })
            }
            _ => false,
        })
    }

    fn collect_static_block_declared_names(
        &self,
        stmts: &[rusty_js_ast::Stmt],
        lexical_names: &mut Vec<String>,
        var_names: &mut Vec<String>,
    ) {
        use rusty_js_ast::{Stmt, VariableKind};
        for stmt in stmts {
            match stmt {
                Stmt::Variable(v) => {
                    let out = if matches!(v.kind, VariableKind::Var) {
                        &mut *var_names
                    } else {
                        &mut *lexical_names
                    };
                    for d in &v.declarators {
                        out.extend(d.target.collect_names().iter().map(|n| n.name.clone()));
                    }
                }
                Stmt::FunctionDecl { name, .. } => {
                    if let Some(name) = name {
                        var_names.push(name.name.clone());
                    }
                }
                Stmt::Block { body, .. } => {
                    self.collect_static_block_declared_names(body, lexical_names, var_names);
                }
                Stmt::If {
                    consequent,
                    alternate,
                    ..
                } => {
                    self.collect_static_block_declared_names(
                        std::slice::from_ref(consequent),
                        lexical_names,
                        var_names,
                    );
                    if let Some(alternate) = alternate {
                        self.collect_static_block_declared_names(
                            std::slice::from_ref(alternate),
                            lexical_names,
                            var_names,
                        );
                    }
                }
                _ => {}
            }
        }
    }

    fn collect_for_body_var_names(&self, stmts: &[rusty_js_ast::Stmt], out: &mut Vec<String>) {
        use rusty_js_ast::{Stmt, VariableKind};
        for stmt in stmts {
            match stmt {
                Stmt::Variable(v) if matches!(v.kind, VariableKind::Var) => {
                    for d in &v.declarators {
                        out.extend(d.target.collect_names().iter().map(|n| n.name.clone()));
                    }
                }
                Stmt::Block { body, .. } => self.collect_for_body_var_names(body, out),
                Stmt::If {
                    consequent,
                    alternate,
                    ..
                } => {
                    self.collect_for_body_var_names(std::slice::from_ref(consequent), out);
                    if let Some(a) = alternate {
                        self.collect_for_body_var_names(std::slice::from_ref(a), out);
                    }
                }
                Stmt::While { body, .. }
                | Stmt::DoWhile { body, .. }
                | Stmt::For { body, .. }
                | Stmt::ForIn { body, .. }
                | Stmt::ForOf { body, .. }
                | Stmt::Labelled { body, .. } => {
                    self.collect_for_body_var_names(std::slice::from_ref(body), out);
                }
                Stmt::Try {
                    block,
                    handler,
                    finalizer,
                    ..
                } => {
                    self.collect_for_body_var_names(std::slice::from_ref(block), out);
                    if let Some(h) = handler {
                        self.collect_for_body_var_names(std::slice::from_ref(&h.body), out);
                    }
                    if let Some(f) = finalizer {
                        self.collect_for_body_var_names(std::slice::from_ref(f), out);
                    }
                }
                Stmt::Switch { cases, .. } => {
                    for c in cases {
                        self.collect_for_body_var_names(&c.consequent, out);
                    }
                }
                _ => {}
            }
        }
    }

    fn check_c_style_for_lexical_declarators(
        kind: VariableKind,
        declarators: &[VariableDeclarator],
    ) -> Result<(), ParseError> {
        if !matches!(kind, VariableKind::Let | VariableKind::Const) {
            return Ok(());
        }
        let mut seen: std::collections::HashMap<&str, Span> = std::collections::HashMap::new();
        for decl in declarators {
            if matches!(kind, VariableKind::Const) && decl.init.is_none() {
                return Err(ParseError {
                    span: decl.span,
                    message: "const declaration in for-statement head requires an initializer"
                        .into(),
                });
            }
            for id in decl.target.collect_names() {
                if let Some(first) = seen.get(id.name.as_str()) {
                    return Err(ParseError {
                        span: Span::new(first.start, id.span.end),
                        message: format!(
                            "duplicate lexical binding `{}` in for-statement head",
                            id.name
                        ),
                    });
                }
                seen.insert(id.name.as_str(), id.span);
            }
        }
        Ok(())
    }

    fn expr_contains_private_delete(&self, expr: &rusty_js_ast::Expr) -> bool {
        use rusty_js_ast::{
            Argument, ArrowBody, Expr, MemberProperty, ObjectKey, ObjectProperty, UnaryOp,
        };
        match expr {
            Expr::Unary {
                operator: UnaryOp::Delete,
                argument,
                ..
            } => self.delete_operand_is_private_member(argument),
            Expr::Array { elements, .. } => elements.iter().any(|e| match e {
                rusty_js_ast::ArrayElement::Expr(e)
                | rusty_js_ast::ArrayElement::Spread { expr: e, .. } => {
                    self.expr_contains_private_delete(e)
                }
                rusty_js_ast::ArrayElement::Elision { .. } => false,
            }),
            Expr::Object { properties, .. } => properties.iter().any(|p| match p {
                ObjectProperty::Property { key, value, .. } => {
                    matches!(key, ObjectKey::Computed { expr, .. } if self.expr_contains_private_delete(expr))
                        || self.expr_contains_private_delete(value)
                }
                ObjectProperty::Spread { expr, .. } => self.expr_contains_private_delete(expr),
            }),
            Expr::Parenthesized { expr, .. }
            | Expr::Update { argument: expr, .. }
            | Expr::Unary { argument: expr, .. } => self.expr_contains_private_delete(expr),
            Expr::Member {
                object, property, ..
            } => {
                self.expr_contains_private_delete(object)
                    || matches!(
                        property.as_ref(),
                        MemberProperty::Computed { expr, .. } if self.expr_contains_private_delete(expr)
                    )
            }
            Expr::Call {
                callee, arguments, ..
            }
            | Expr::New {
                callee, arguments, ..
            } => {
                self.expr_contains_private_delete(callee)
                    || arguments.iter().any(|a| match a {
                        Argument::Expr(e) | Argument::Spread { expr: e, .. } => {
                            self.expr_contains_private_delete(e)
                        }
                    })
            }
            Expr::Binary { left, right, .. } | Expr::Assign { target: left, value: right, .. } => {
                self.expr_contains_private_delete(left) || self.expr_contains_private_delete(right)
            }
            Expr::Conditional {
                test,
                consequent,
                alternate,
                ..
            } => {
                self.expr_contains_private_delete(test)
                    || self.expr_contains_private_delete(consequent)
                    || self.expr_contains_private_delete(alternate)
            }
            Expr::Sequence { expressions, .. } | Expr::TemplateLiteral { expressions, .. } => {
                expressions.iter().any(|e| self.expr_contains_private_delete(e))
            }
            Expr::Arrow { body, .. } => match body {
                ArrowBody::Expression(e) => self.expr_contains_private_delete(e),
                ArrowBody::Block(stmts) => self.stmts_contain_private_delete(stmts),
            },
            Expr::Function { .. }
            | Expr::Class { .. }
            | Expr::NullLiteral { .. }
            | Expr::BoolLiteral { .. }
            | Expr::NumberLiteral { .. }
            | Expr::BigIntLiteral { .. }
            | Expr::StringLiteral { .. }
            | Expr::WtfStringLiteral { .. }
            | Expr::Identifier { .. }
            | Expr::This { .. }
            | Expr::Super { .. }
            | Expr::MetaProperty { .. }
            | Expr::TemplateObject { .. }
            | Expr::RegExp { .. }
            | Expr::Opaque { .. } => false,
        }
    }

    fn delete_operand_is_private_member(&self, expr: &rusty_js_ast::Expr) -> bool {
        use rusty_js_ast::{Expr, MemberProperty};
        match expr {
            Expr::Parenthesized { expr, .. } => self.delete_operand_is_private_member(expr),
            Expr::Member { property, .. } => {
                matches!(property.as_ref(), MemberProperty::Private { .. })
            }
            _ => false,
        }
    }

    fn stmts_contain_private_delete(&self, stmts: &[rusty_js_ast::Stmt]) -> bool {
        use rusty_js_ast::Stmt;
        stmts.iter().any(|s| match s {
            Stmt::Expression { expr, .. } => self.expr_contains_private_delete(expr),
            Stmt::Block { body, .. } => self.stmts_contain_private_delete(body),
            Stmt::If {
                test,
                consequent,
                alternate,
                ..
            } => {
                self.expr_contains_private_delete(test)
                    || self.stmt_contains_private_delete(consequent)
                    || alternate
                        .as_deref()
                        .is_some_and(|s| self.stmt_contains_private_delete(s))
            }
            Stmt::Return { argument, .. } => argument
                .as_ref()
                .is_some_and(|e| self.expr_contains_private_delete(e)),
            Stmt::Throw { argument, .. } => self.expr_contains_private_delete(argument),
            _ => false,
        })
    }

    fn stmt_contains_private_delete(&self, stmt: &rusty_js_ast::Stmt) -> bool {
        self.stmts_contain_private_delete(std::slice::from_ref(stmt))
    }

    fn first_unbound_private_name<'a>(
        &self,
        expr: &'a rusty_js_ast::Expr,
        private_names: &std::collections::HashSet<&str>,
    ) -> Option<&'a str> {
        use rusty_js_ast::{Argument, Expr, MemberProperty};
        match expr {
            Expr::Member {
                object, property, ..
            } => self
                .first_unbound_private_name(object, private_names)
                .or_else(|| {
                    if let MemberProperty::Private { name, .. } = property.as_ref() {
                        if !private_names.contains(name.as_str()) {
                            return Some(name.as_str());
                        }
                    }
                    None
                }),
            Expr::Call {
                callee, arguments, ..
            }
            | Expr::New {
                callee, arguments, ..
            } => self
                .first_unbound_private_name(callee, private_names)
                .or_else(|| {
                    arguments.iter().find_map(|a| match a {
                        Argument::Expr(e) | Argument::Spread { expr: e, .. } => {
                            self.first_unbound_private_name(e, private_names)
                        }
                    })
                }),
            Expr::Parenthesized { expr, .. }
            | Expr::Update { argument: expr, .. }
            | Expr::Unary { argument: expr, .. } => {
                self.first_unbound_private_name(expr, private_names)
            }
            Expr::Binary { left, right, .. }
            | Expr::Assign {
                target: left,
                value: right,
                ..
            } => self
                .first_unbound_private_name(left, private_names)
                .or_else(|| self.first_unbound_private_name(right, private_names)),
            Expr::Conditional {
                test,
                consequent,
                alternate,
                ..
            } => self
                .first_unbound_private_name(test, private_names)
                .or_else(|| self.first_unbound_private_name(consequent, private_names))
                .or_else(|| self.first_unbound_private_name(alternate, private_names)),
            Expr::Sequence { expressions, .. } | Expr::TemplateLiteral { expressions, .. } => {
                expressions
                    .iter()
                    .find_map(|e| self.first_unbound_private_name(e, private_names))
            }
            _ => None,
        }
    }

    fn consume_class_field_terminator(&mut self) -> Result<(), ParseError> {
        if matches!(self.current_kind(), TokenKind::Punct(Punct::Semicolon)) {
            self.bump()?;
            return Ok(());
        }
        if matches!(self.current_kind(), TokenKind::Punct(Punct::RBrace))
            || self.lookahead_preceded_by_lt()
        {
            return Ok(());
        }
        Err(self.err_here("expected class field terminator".into()))
    }

    fn parse_class_member_name(&mut self) -> Result<rusty_js_ast::ClassMemberName, ParseError> {
        use rusty_js_ast::ClassMemberName;
        let span = self.lookahead_span();
        match self.current_kind().clone() {
            TokenKind::Ident(name) => {
                self.bump()?;
                Ok(ClassMemberName::Identifier { name, span })
            }
            TokenKind::PrivateIdent(name) => {
                if name == "constructor" {
                    return Err(ParseError {
                        message: "PrivateName cannot be #constructor".into(),
                        span,
                    });
                }
                self.bump()?;
                Ok(ClassMemberName::Private { name, span })
            }
            TokenKind::String(value) => {
                self.bump()?;
                Ok(ClassMemberName::String { value, span })
            }
            TokenKind::Number(value, _) => {
                self.bump()?;
                Ok(ClassMemberName::Number { value, span })
            }
            TokenKind::BigInt(digits, kind) => {
                self.bump()?;
                Ok(ClassMemberName::String {
                    value: Self::bigint_literal_property_name(&digits, kind),
                    span,
                })
            }
            TokenKind::Punct(Punct::LBracket) => {
                self.bump()?;
                let saved_in_disallowed = self.in_disallowed;
                self.in_disallowed = false;
                let expr = self.parse_assignment_expression();
                self.in_disallowed = saved_in_disallowed;
                let expr = expr?;
                self.expect_punct(Punct::RBracket)?;
                Ok(ClassMemberName::Computed {
                    expr,
                    span: Span::new(span.start, self.last_span_end()),
                })
            }
            _ => Err(self.err_here("expected class member name".into())),
        }
    }

    fn next_is_method_open_or_field_terminator(&self) -> bool {
        let pos = self.lookahead_span().end;
        let bytes = self.source().as_bytes();
        let mut p = pos;
        while p < bytes.len() && (bytes[p] == b' ' || bytes[p] == b'\t') {
            p += 1;
        }
        matches!(
            bytes.get(p),
            Some(&b'(') | Some(&b'=') | Some(&b';') | Some(&b'\n') | Some(&b'\r') | Some(&b'}')
        )
    }

    fn skip_ws_and_comments_no_lt(bytes: &[u8], mut p: usize) -> Option<usize> {
        loop {
            while p < bytes.len() {
                match bytes[p] {
                    b' ' | b'\t' => p += 1,
                    b'\n' | b'\r' => return None,
                    _ => break,
                }
            }
            if bytes.get(p) == Some(&b'/') && bytes.get(p + 1) == Some(&b'/') {
                return None;
            }
            if bytes.get(p) == Some(&b'/') && bytes.get(p + 1) == Some(&b'*') {
                p += 2;
                while p + 1 < bytes.len() && !(bytes[p] == b'*' && bytes[p + 1] == b'/') {
                    if bytes[p] == b'\n' || bytes[p] == b'\r' {
                        return None;
                    }
                    p += 1;
                }
                p = (p + 2).min(bytes.len());
                continue;
            }
            return Some(p);
        }
    }

    pub(crate) fn skip_ws_and_comments_allow_lt(bytes: &[u8], mut p: usize) -> usize {
        loop {
            while p < bytes.len() && bytes[p].is_ascii_whitespace() {
                p += 1;
            }
            if bytes.get(p) == Some(&b'/') && bytes.get(p + 1) == Some(&b'/') {
                p += 2;
                while p < bytes.len() && bytes[p] != b'\n' && bytes[p] != b'\r' {
                    p += 1;
                }
                continue;
            }
            if bytes.get(p) == Some(&b'/') && bytes.get(p + 1) == Some(&b'*') {
                p += 2;
                while p + 1 < bytes.len() && !(bytes[p] == b'*' && bytes[p + 1] == b'/') {
                    p += 1;
                }
                p = (p + 2).min(bytes.len());
                continue;
            }
            return p;
        }
    }

    fn skip_ws_and_comments_track_lt(bytes: &[u8], mut p: usize) -> (usize, bool) {
        let mut saw_lt = false;
        loop {
            while p < bytes.len() && bytes[p].is_ascii_whitespace() {
                if matches!(bytes[p], b'\n' | b'\r') {
                    saw_lt = true;
                }
                p += 1;
            }
            if bytes.get(p) == Some(&b'/') && bytes.get(p + 1) == Some(&b'/') {
                saw_lt = true;
                p += 2;
                while p < bytes.len() && bytes[p] != b'\n' && bytes[p] != b'\r' {
                    p += 1;
                }
                continue;
            }
            if bytes.get(p) == Some(&b'/') && bytes.get(p + 1) == Some(&b'*') {
                p += 2;
                while p + 1 < bytes.len() && !(bytes[p] == b'*' && bytes[p + 1] == b'/') {
                    if matches!(bytes[p], b'\n' | b'\r') {
                        saw_lt = true;
                    }
                    p += 1;
                }
                p = (p + 2).min(bytes.len());
                continue;
            }
            return (p, saw_lt);
        }
    }

    fn byte_can_start_binding_list(b: u8) -> bool {
        matches!(b, b'[' | b'{' | b'_' | b'$' | b'\\') || b.is_ascii_alphabetic() || b >= 0x80
    }

    fn parse_block_statement(&mut self) -> Result<Stmt, ParseError> {
        self.enter_parse_depth()?;
        let start = self.lookahead_span().start;
        if let Err(err) = self.expect_punct(Punct::LBrace) {
            self.leave_parse_depth();
            return Err(err);
        }
        let _if_branch_block_depth =
            crate::parser::parse_profile::nested_if_branch_block_depth_guard();
        let _top_class_method_body_if_branch_block_depth =
            crate::parser::parse_profile::function_body_top_class_method_body_if_branch_block_depth_guard();
        let _if_branch_block_if_consequent_block_depth =
            crate::parser::parse_profile::nested_if_branch_block_if_consequent_block_depth_guard();
        let mut body = Vec::new();
        while !matches!(self.current_kind(), TokenKind::Punct(Punct::RBrace))
            && !self.at_eof_internal()
        {
            if crate::parser::parse_profile::enabled() {
                let family = self.profile_current_substatement_family();
                let _if_branch_block_stmt =
                    crate::parser::parse_profile::nested_if_branch_block_statement_guard(family);
                let _top_class_method_body_if_branch_block_stmt =
                    crate::parser::parse_profile::function_body_top_class_method_body_if_branch_block_statement_guard(family);
                let _if_branch_block_if_consequent_block_stmt = crate::parser::parse_profile::nested_if_branch_block_if_consequent_block_statement_guard(family);
                match self.parse_statement() {
                    Ok(stmt) => body.push(stmt),
                    Err(err) => {
                        self.leave_parse_depth();
                        return Err(err);
                    }
                }
            } else {
                match self.parse_statement() {
                    Ok(stmt) => body.push(stmt),
                    Err(err) => {
                        self.leave_parse_depth();
                        return Err(err);
                    }
                }
            }
        }
        if let Err(err) = self.consume_statement_rbrace() {
            self.leave_parse_depth();
            return Err(err);
        }

        if let Err(err) = self.check_block_bound_names(&body) {
            self.leave_parse_depth();
            return Err(err);
        }

        let body = if self.saw_using_declaration {
            desugar_using_block(body)
        } else {
            body
        };
        let end = self.last_span_end();
        self.leave_parse_depth();
        Ok(Stmt::Block {
            body,
            span: Span::new(start, end),
        })
    }

    fn skip_to_top_terminator(&mut self) -> Result<Span, ParseError> {
        let start = self.lookahead_span().start;
        let mut depth_paren = 0i32;
        let mut depth_brace = 0i32;
        let mut depth_bracket = 0i32;
        while !self.at_eof_internal() {
            let kind = self.current_kind().clone();
            match kind {
                TokenKind::Punct(Punct::LParen) => depth_paren += 1,
                TokenKind::Punct(Punct::RParen) => depth_paren -= 1,
                TokenKind::Punct(Punct::LBrace) => depth_brace += 1,
                TokenKind::Punct(Punct::RBrace) => {
                    if depth_brace == 0 {
                        break;
                    }
                    depth_brace -= 1;

                    if depth_brace == 0 && depth_paren == 0 && depth_bracket == 0 {
                        let end = self.lookahead_span().end;
                        self.bump()?;
                        return Ok(Span::new(start, end));
                    }
                }
                TokenKind::Punct(Punct::LBracket) => depth_bracket += 1,
                TokenKind::Punct(Punct::RBracket) => depth_bracket -= 1,
                TokenKind::Punct(Punct::Semicolon) => {
                    if depth_paren == 0 && depth_brace == 0 && depth_bracket == 0 {
                        let end = self.lookahead_span().end;
                        self.bump()?;
                        return Ok(Span::new(start, end));
                    }
                }
                _ => {}
            }

            if depth_paren == 0
                && depth_brace == 0
                && depth_bracket == 0
                && self.lookahead_preceded_by_lt()
                && self.lookahead_span().start != start
            {
                break;
            }
            self.bump()?;
        }
        Ok(Span::new(start, self.last_span_end()))
    }

    fn err_if_labelled_function(&self, s: &Stmt, ctx: &str) -> Result<(), ParseError> {
        fn is_lf(s: &Stmt) -> bool {
            match s {
                Stmt::Labelled { body, .. } => {
                    matches!(body.as_ref(), Stmt::FunctionDecl { .. }) || is_lf(body.as_ref())
                }
                _ => false,
            }
        }
        if is_lf(s) {
            return Err(ParseError {
                span: s.span(),
                message: format!(
                    "a labelled function declaration is not allowed as the body of a {ctx}"
                ),
            });
        }
        Ok(())
    }

    fn parse_if_statement(&mut self) -> Result<Stmt, ParseError> {
        let start = self.lookahead_span().start;
        self.expect_keyword("if")?;
        self.expect_punct(Punct::LParen)?;
        let test = {
            let _profile = crate::parser::parse_profile::nested_statement_guard(
                crate::parser::parse_profile::Kind::NestedIfTest,
            );
            let _branch_block_if_phase =
                crate::parser::parse_profile::nested_if_branch_block_if_phase_guard(
                    crate::parser::parse_profile::IfPhase::Test,
                );
            let _top_class_method_body_if_phase =
                crate::parser::parse_profile::FunctionBodyTopClassMethodBodyIfPhaseGuard::new(
                    crate::parser::parse_profile::IfPhase::Test,
                );
            let _top_var_init_ident_arrow_expr_cond_class_method_body_if_phase =
                crate::parser::parse_profile::TopVarInitIdentArrowExprCondClassMethodBodyIfPhaseGuard::new(
                    crate::parser::parse_profile::IfPhase::Test,
                );
            let _top_class_method_body_if_branch_block_if_phase =
                crate::parser::parse_profile::function_body_top_class_method_body_if_branch_block_if_phase_guard(
                    crate::parser::parse_profile::IfPhase::Test,
                );
            let _branch_block_if_consequent_block_if_phase = crate::parser::parse_profile::nested_if_branch_block_if_consequent_block_if_phase_guard(crate::parser::parse_profile::IfPhase::Test);
            let test = self.parse_expression()?;
            self.consume_cond_rparen()?;
            test
        };

        let prev_allow = self.allow_annex_b_function_in_substatement;
        self.allow_annex_b_function_in_substatement = true;
        let consequent = {
            let _profile = crate::parser::parse_profile::nested_statement_guard(
                crate::parser::parse_profile::Kind::NestedIfConsequent,
            );
            let _branch_block_if_phase =
                crate::parser::parse_profile::nested_if_branch_block_if_phase_guard(
                    crate::parser::parse_profile::IfPhase::Consequent,
                );
            let _top_class_method_body_if_phase =
                crate::parser::parse_profile::FunctionBodyTopClassMethodBodyIfPhaseGuard::new(
                    crate::parser::parse_profile::IfPhase::Consequent,
                );
            let _top_var_init_ident_arrow_expr_cond_class_method_body_if_phase =
                crate::parser::parse_profile::TopVarInitIdentArrowExprCondClassMethodBodyIfPhaseGuard::new(
                    crate::parser::parse_profile::IfPhase::Consequent,
                );
            let _top_class_method_body_if_branch_block_if_phase =
                crate::parser::parse_profile::function_body_top_class_method_body_if_branch_block_if_phase_guard(
                    crate::parser::parse_profile::IfPhase::Consequent,
                );
            let _branch_block_if_consequent_block_if_phase = crate::parser::parse_profile::nested_if_branch_block_if_consequent_block_if_phase_guard(crate::parser::parse_profile::IfPhase::Consequent);
            if crate::parser::parse_profile::enabled() {
                let family = self.profile_current_substatement_family();
                let _family_profile = crate::parser::parse_profile::nested_if_substatement_guard(
                    crate::parser::parse_profile::IfBranch::Consequent,
                    family,
                );
                let _top_class_method_body_if_substmt =
                    crate::parser::parse_profile::FunctionBodyTopClassMethodBodyIfSubstatementGuard::new(
                        crate::parser::parse_profile::IfBranch::Consequent,
                        family,
                    );
                let _top_var_init_ident_arrow_expr_cond_class_method_body_if_substmt =
                    crate::parser::parse_profile::TopVarInitIdentArrowExprCondClassMethodBodyIfSubstatementGuard::new(
                        crate::parser::parse_profile::IfBranch::Consequent,
                        family,
                    );
                let _top_class_method_body_if_branch_block_if_substmt =
                    crate::parser::parse_profile::function_body_top_class_method_body_if_branch_block_if_substatement_guard(
                        crate::parser::parse_profile::IfBranch::Consequent,
                        family,
                    );
                let _branch_block_if_branch =
                    crate::parser::parse_profile::nested_if_branch_block_if_branch_guard(
                        crate::parser::parse_profile::IfBranch::Consequent,
                        family,
                    );
                let _branch_block_if_consequent_block_profile = matches!(
                    family,
                    crate::parser::parse_profile::StatementFamily::Block
                )
                .then(
                    crate::parser::parse_profile::nested_if_branch_block_if_consequent_block_guard,
                );
                let _top_class_method_body_if_branch_block =
                    matches!(family, crate::parser::parse_profile::StatementFamily::Block).then(|| {
                        crate::parser::parse_profile::function_body_top_class_method_body_if_branch_block_guard(
                            crate::parser::parse_profile::IfBranch::Consequent,
                        )
                    });
                let _branch_block_profile =
                    matches!(family, crate::parser::parse_profile::StatementFamily::Block).then(
                        || {
                            crate::parser::parse_profile::nested_if_branch_block_guard(
                                crate::parser::parse_profile::IfBranch::Consequent,
                            )
                        },
                    );
                self.parse_substatement()?
            } else {
                self.parse_substatement()?
            }
        };
        let alternate = if self.is_ident("else") {
            self.bump()?;
            let _profile = crate::parser::parse_profile::nested_statement_guard(
                crate::parser::parse_profile::Kind::NestedIfAlternate,
            );
            let _branch_block_if_phase =
                crate::parser::parse_profile::nested_if_branch_block_if_phase_guard(
                    crate::parser::parse_profile::IfPhase::Alternate,
                );
            let _top_class_method_body_if_phase =
                crate::parser::parse_profile::FunctionBodyTopClassMethodBodyIfPhaseGuard::new(
                    crate::parser::parse_profile::IfPhase::Alternate,
                );
            let _top_var_init_ident_arrow_expr_cond_class_method_body_if_phase =
                crate::parser::parse_profile::TopVarInitIdentArrowExprCondClassMethodBodyIfPhaseGuard::new(
                    crate::parser::parse_profile::IfPhase::Alternate,
                );
            let _top_class_method_body_if_branch_block_if_phase =
                crate::parser::parse_profile::function_body_top_class_method_body_if_branch_block_if_phase_guard(
                    crate::parser::parse_profile::IfPhase::Alternate,
                );
            let _branch_block_if_consequent_block_if_phase = crate::parser::parse_profile::nested_if_branch_block_if_consequent_block_if_phase_guard(crate::parser::parse_profile::IfPhase::Alternate);
            if crate::parser::parse_profile::enabled() {
                let family = self.profile_current_substatement_family();
                let _family_profile = crate::parser::parse_profile::nested_if_substatement_guard(
                    crate::parser::parse_profile::IfBranch::Alternate,
                    family,
                );
                let _top_class_method_body_if_substmt =
                    crate::parser::parse_profile::FunctionBodyTopClassMethodBodyIfSubstatementGuard::new(
                        crate::parser::parse_profile::IfBranch::Alternate,
                        family,
                    );
                let _top_var_init_ident_arrow_expr_cond_class_method_body_if_substmt =
                    crate::parser::parse_profile::TopVarInitIdentArrowExprCondClassMethodBodyIfSubstatementGuard::new(
                        crate::parser::parse_profile::IfBranch::Alternate,
                        family,
                    );
                let _top_class_method_body_if_branch_block_if_substmt =
                    crate::parser::parse_profile::function_body_top_class_method_body_if_branch_block_if_substatement_guard(
                        crate::parser::parse_profile::IfBranch::Alternate,
                        family,
                    );
                let _branch_block_if_branch =
                    crate::parser::parse_profile::nested_if_branch_block_if_branch_guard(
                        crate::parser::parse_profile::IfBranch::Alternate,
                        family,
                    );
                let _top_class_method_body_if_branch_block =
                    matches!(family, crate::parser::parse_profile::StatementFamily::Block).then(|| {
                        crate::parser::parse_profile::function_body_top_class_method_body_if_branch_block_guard(
                            crate::parser::parse_profile::IfBranch::Alternate,
                        )
                    });
                let _branch_block_profile =
                    matches!(family, crate::parser::parse_profile::StatementFamily::Block).then(
                        || {
                            crate::parser::parse_profile::nested_if_branch_block_guard(
                                crate::parser::parse_profile::IfBranch::Alternate,
                            )
                        },
                    );
                Some(Box::new(self.parse_substatement()?))
            } else {
                Some(Box::new(self.parse_substatement()?))
            }
        } else {
            None
        };
        self.allow_annex_b_function_in_substatement = prev_allow;
        let end = {
            let _profile = crate::parser::parse_profile::nested_statement_guard(
                crate::parser::parse_profile::Kind::NestedIfClose,
            );
            let _branch_block_if_phase =
                crate::parser::parse_profile::nested_if_branch_block_if_phase_guard(
                    crate::parser::parse_profile::IfPhase::Close,
                );
            let _top_class_method_body_if_phase =
                crate::parser::parse_profile::FunctionBodyTopClassMethodBodyIfPhaseGuard::new(
                    crate::parser::parse_profile::IfPhase::Close,
                );
            let _top_var_init_ident_arrow_expr_cond_class_method_body_if_phase =
                crate::parser::parse_profile::TopVarInitIdentArrowExprCondClassMethodBodyIfPhaseGuard::new(
                    crate::parser::parse_profile::IfPhase::Close,
                );
            let _top_class_method_body_if_branch_block_if_phase =
                crate::parser::parse_profile::function_body_top_class_method_body_if_branch_block_if_phase_guard(
                    crate::parser::parse_profile::IfPhase::Close,
                );
            let _branch_block_if_consequent_block_if_phase = crate::parser::parse_profile::nested_if_branch_block_if_consequent_block_if_phase_guard(crate::parser::parse_profile::IfPhase::Close);
            self.err_if_labelled_function(&consequent, "`if` statement")?;
            if let Some(a) = &alternate {
                self.err_if_labelled_function(a, "`if` statement")?;
            }
            self.last_span_end()
        };
        Ok(Stmt::If {
            test,
            consequent: Box::new(consequent),
            alternate,
            span: Span::new(start, end),
        })
    }

    fn profile_current_substatement_family(&self) -> crate::parser::parse_profile::StatementFamily {
        use crate::parser::parse_profile::StatementFamily;
        match self.current_kind() {
            TokenKind::Ident(s)
                if s == "var"
                    || s == "const"
                    || (s == "let" && self.is_contextual_keyword("let")) =>
            {
                StatementFamily::Var
            }
            TokenKind::Ident(s) if s == "function" || s == "class" => StatementFamily::Function,
            TokenKind::Punct(Punct::LBrace) => StatementFamily::Block,
            TokenKind::Ident(s)
                if matches!(
                    s.as_str(),
                    "if" | "for"
                        | "while"
                        | "do"
                        | "switch"
                        | "try"
                        | "return"
                        | "throw"
                        | "break"
                        | "continue"
                        | "with"
                ) =>
            {
                StatementFamily::Control
            }
            _ => StatementFamily::Expression,
        }
    }

    fn parse_for_statement(&mut self) -> Result<Stmt, ParseError> {
        let start = self.lookahead_span().start;
        self.expect_keyword("for")?;

        let await_form = if self.is_ident("await") {
            self.bump()?;
            true
        } else {
            false
        };
        if await_form
            && !(self.in_async
                || (self.function_body_depth == 0 && self.goal_allows_top_level_await()))
        {
            return Err(ParseError {
                span: Span::new(start, self.last_span_end()),
                message: "`for await` is only valid in async function code".into(),
            });
        }
        self.expect_punct(Punct::LParen)?;

        let head_is_using = self.is_ident("using")
            && self.using_starts_for_head_declaration(self.lookahead_span().end);
        let head_is_await_using =
            self.is_ident("await") && self.await_using_starts_for_head_declaration();
        let head_is_var = self.is_ident("var")
            || self.is_ident("const")
            || (self.is_contextual_keyword("let") && self.let_starts_lexical_declaration())
            || head_is_using
            || head_is_await_using;
        let head_is_empty = matches!(self.current_kind(), TokenKind::Punct(Punct::Semicolon));
        if await_form && head_is_empty {
            return Err(ParseError {
                span: Span::new(start, self.lookahead_span().end),
                message: "`for await` requires a for-of head".into(),
            });
        }

        if head_is_var {

            let kind = match self.current_kind() {
                TokenKind::Ident(s) if s == "var" => VariableKind::Var,
                TokenKind::Ident(s) if s == "let" && self.is_contextual_keyword("let") => {
                    VariableKind::Let
                }
                TokenKind::Ident(s) if s == "const" => VariableKind::Const,
                TokenKind::Ident(s) if s == "using" => VariableKind::Using,
                TokenKind::Ident(s) if s == "await" && head_is_await_using => {
                    VariableKind::AwaitUsing
                }
                _ => unreachable!(),
            };
            let kw_span = self.lookahead_span();
            self.bump()?;
            if matches!(kind, VariableKind::AwaitUsing) {
                self.bump()?;
            }

            if matches!(
                self.current_kind(),
                TokenKind::Punct(Punct::LBracket) | TokenKind::Punct(Punct::LBrace)
            ) {
                let pat_start = self.lookahead_span().start;
                let target = self.parse_binding_target()?;

                Self::check_no_let_bound_name(kind, &target)?;
                let pat_end = self.last_span_end();
                if self.is_ident("in") || self.is_contextual_keyword("of") {
                    let is_of = self.is_contextual_keyword("of");
                    self.reject_escaped_contextual_keyword(if is_of { "of" } else { "in" })?;
                    self.bump()?;
                    if is_of {
                        self.current_lex_goal = LexerGoal::RegExp;
                    }
                    let right = if is_of {
                        self.parse_assignment_expression()?
                    } else {
                        self.parse_expression()?
                    };
                    self.consume_cond_rparen()?;
                    let body = self.parse_substatement()?;
                    self.err_if_labelled_function(&body, "`for` statement")?;
                    let end = self.last_span_end();
                    let left = ForBinding::Decl {
                        kind,
                        target,
                        span: Span::new(pat_start, pat_end),
                    };
                    return if is_of {
                        Ok(desugar_using_for_of_statement(
                            left,
                            right,
                            body,
                            await_form,
                            Span::new(start, end),
                        ))
                    } else {
                        if await_form {
                            return Err(ParseError {
                                span: Span::new(start, end),
                                message: "`for await` requires a for-of head".into(),
                            });
                        }
                        if is_using_kind(kind) {
                            return Err(ParseError {
                                span: kw_span,
                                message: "`using` declarations are not allowed in for-in heads"
                                    .into(),
                            });
                        }
                        Ok(Stmt::ForIn {
                            left,
                            right,
                            body: Box::new(body),
                            span: Span::new(start, end),
                        })
                    };
                }

                let init = if matches!(self.current_kind(), TokenKind::Punct(Punct::Assign)) {
                    self.bump()?;
                    Some(self.parse_assignment_expression_no_in()?)
                } else {
                    None
                };
                let mut declarators = vec![VariableDeclarator {
                    target,
                    init,
                    span: Span::new(pat_start, self.last_span_end()),
                }];
                while matches!(self.current_kind(), TokenKind::Punct(Punct::Comma)) {
                    self.bump()?;
                    let d_start = self.lookahead_span().start;
                    let dt = self.parse_binding_target()?;
                    Self::check_no_let_bound_name(kind, &dt)?;
                    let di = if matches!(self.current_kind(), TokenKind::Punct(Punct::Assign)) {
                        self.bump()?;
                        Some(self.parse_assignment_expression_no_in()?)
                    } else {
                        None
                    };
                    declarators.push(VariableDeclarator {
                        target: dt,
                        init: di,
                        span: Span::new(d_start, self.last_span_end()),
                    });
                }
                self.expect_punct(Punct::Semicolon)?;
                if await_form {
                    return Err(ParseError {
                        span: Span::new(start, self.last_span_end()),
                        message: "`for await` requires a for-of head".into(),
                    });
                }
                let test = if !matches!(self.current_kind(), TokenKind::Punct(Punct::Semicolon)) {
                    Some(self.parse_expression()?)
                } else {
                    None
                };
                self.expect_punct(Punct::Semicolon)?;
                let update = if !matches!(self.current_kind(), TokenKind::Punct(Punct::RParen)) {
                    Some(self.parse_expression()?)
                } else {
                    None
                };
                self.consume_cond_rparen()?;
                let body = self.parse_substatement()?;
                self.err_if_labelled_function(&body, "`for` statement")?;
                let end = self.last_span_end();
                Self::check_c_style_for_lexical_declarators(kind, &declarators)?;

                if matches!(kind, VariableKind::Let | VariableKind::Const) {
                    let mut head_names = Vec::new();
                    for d in &declarators {
                        head_names.extend(d.target.collect_names().iter().map(|n| n.name.clone()));
                    }
                    let mut body_vars = Vec::new();
                    self.collect_for_body_var_names(std::slice::from_ref(&body), &mut body_vars);
                    if let Some(dup) = head_names
                        .iter()
                        .find(|h| body_vars.iter().any(|v| v == *h))
                    {
                        return Err(ParseError {
                            span: Span::new(start, end),
                            message: format!(
                                "for-statement lexical binding `{dup}` conflicts with a var declaration in the body"
                            ),
                        });
                    }
                }
                let init_vs = VariableStatement {
                    kind,
                    declarators,
                    span: Span::new(kw_span.start, kw_span.end),
                };
                return Ok(desugar_using_for_statement(
                    init_vs,
                    test,
                    update,
                    body,
                    Span::new(start, end),
                ));
            }
            if let TokenKind::Ident(n) = self.current_kind().clone() {
                let id_span = self.lookahead_span();

                if crate::parser::is_unconditional_reserved_word(&n) {
                    return Err(ParseError {
                        span: id_span,
                        message: format!(
                            "`{}` is a reserved word and cannot be used as a binding identifier",
                            n
                        ),
                    });
                }

                if self.strict_mode && crate::parser::is_strict_reserved_word(&n) {
                    return Err(ParseError {
                        span: id_span,
                        message: format!(
                            "`{}` is a reserved word in strict mode and cannot be used as a binding identifier",
                            n
                        ),
                    });
                }

                if self.strict_mode && (n == "eval" || n == "arguments") {
                    return Err(ParseError {
                        span: id_span,
                        message: format!(
                            "Binding identifier '{}' is not allowed in strict mode",
                            n
                        ),
                    });
                }
                if (self.in_generator || self.strict_mode) && n == "yield" {
                    return Err(ParseError {
                        span: id_span,
                        message: "`yield` is not a valid binding in this context".into(),
                    });
                }

                if !matches!(kind, VariableKind::Var) && n == "let" {
                    return Err(ParseError {
                        span: id_span,
                        message: "Lexical declaration may not bind the name 'let'".into(),
                    });
                }
                self.bump()?;

                if self.is_ident("in") || self.is_contextual_keyword("of") {
                    let is_of = self.is_contextual_keyword("of");
                    self.reject_escaped_contextual_keyword(if is_of { "of" } else { "in" })?;
                    self.bump()?;
                    if is_of {
                        self.current_lex_goal = LexerGoal::RegExp;
                    }
                    let right = if is_of {
                        self.parse_assignment_expression()?
                    } else {
                        self.parse_expression()?
                    };
                    self.consume_cond_rparen()?;
                    let body = self.parse_substatement()?;
                    self.err_if_labelled_function(&body, "`for` statement")?;
                    let end = self.last_span_end();
                    let left = ForBinding::Decl {
                        kind,
                        target: BindingPattern::Identifier(BindingIdentifier {
                            name: n,
                            span: id_span,
                        }),
                        span: Span::new(kw_span.start, id_span.end),
                    };
                    return if is_of {
                        Ok(desugar_using_for_of_statement(
                            left,
                            right,
                            body,
                            await_form,
                            Span::new(start, end),
                        ))
                    } else {
                        if await_form {
                            return Err(ParseError {
                                span: Span::new(start, end),
                                message: "`for await` requires a for-of head".into(),
                            });
                        }
                        if is_using_kind(kind) {
                            return Err(ParseError {
                                span: kw_span,
                                message: "`using` declarations are not allowed in for-in heads"
                                    .into(),
                            });
                        }
                        Ok(Stmt::ForIn {
                            left,
                            right,
                            body: Box::new(body),
                            span: Span::new(start, end),
                        })
                    };
                }

                if await_form {
                    return Err(ParseError {
                        span: Span::new(start, self.last_span_end()),
                        message: "`for await` requires a for-of head".into(),
                    });
                }
                let target = BindingPattern::Identifier(BindingIdentifier {
                    name: n.clone(),
                    span: id_span,
                });
                let init = if matches!(self.current_kind(), TokenKind::Punct(Punct::Assign)) {
                    self.bump()?;

                    let prev_in_disallowed = self.in_disallowed;
                    self.in_disallowed = true;
                    let e = self.parse_assignment_expression();
                    self.in_disallowed = prev_in_disallowed;
                    Some(e?)
                } else {
                    None
                };

                if init.is_some()
                    && matches!(kind, VariableKind::Var)
                    && !self.strict_mode
                    && self.is_ident("in")
                {
                    self.bump()?;
                    let right = self.parse_expression()?;
                    self.consume_cond_rparen()?;
                    let body = self.parse_substatement()?;
                    self.err_if_labelled_function(&body, "`for` statement")?;
                    let end = self.last_span_end();

                    let var_stmt = VariableStatement {
                        kind,
                        declarators: vec![VariableDeclarator {
                            target: BindingPattern::Identifier(BindingIdentifier {
                                name: n.clone(),
                                span: id_span,
                            }),
                            init,
                            span: Span::new(id_span.start, id_span.end),
                        }],
                        span: kw_span,
                    };
                    let left = ForBinding::Pattern(BindingPattern::Identifier(BindingIdentifier {
                        name: n,
                        span: id_span,
                    }));
                    let for_in = Stmt::ForIn {
                        left,
                        right,
                        body: Box::new(body),
                        span: Span::new(start, end),
                    };
                    return Ok(Stmt::Block {
                        body: vec![Stmt::Variable(var_stmt), for_in],
                        span: Span::new(start, end),
                    });
                }
                let mut declarators = vec![VariableDeclarator {
                    target,
                    init,
                    span: Span::new(id_span.start, self.last_span_end()),
                }];
                while matches!(self.current_kind(), TokenKind::Punct(Punct::Comma)) {
                    self.bump()?;
                    let d_start = self.lookahead_span().start;

                    let target = if matches!(
                        self.current_kind(),
                        TokenKind::Punct(Punct::LBracket) | TokenKind::Punct(Punct::LBrace)
                    ) {
                        self.parse_binding_target()?
                    } else if let TokenKind::Ident(nn) = self.current_kind().clone() {
                        let nn_span = self.lookahead_span();
                        if crate::parser::is_unconditional_reserved_word(&nn) {
                            return Err(ParseError {
                                span: nn_span,
                                message: format!(
                                    "`{}` is a reserved word and cannot be used as a binding identifier",
                                    nn
                                ),
                            });
                        }
                        if self.strict_mode && crate::parser::is_strict_reserved_word(&nn) {
                            return Err(ParseError {
                                span: nn_span,
                                message: format!(
                                    "`{}` is a reserved word in strict mode and cannot be used as a binding identifier",
                                    nn
                                ),
                            });
                        }
                        if self.strict_mode && (nn == "eval" || nn == "arguments") {
                            return Err(ParseError {
                                span: nn_span,
                                message: format!(
                                    "Binding identifier '{}' is not allowed in strict mode",
                                    nn
                                ),
                            });
                        }
                        if (self.in_generator || self.strict_mode) && nn == "yield" {
                            return Err(ParseError {
                                span: nn_span,
                                message: "`yield` is not a valid binding in this context".into(),
                            });
                        }
                        self.bump()?;
                        BindingPattern::Identifier(BindingIdentifier {
                            name: nn,
                            span: nn_span,
                        })
                    } else {
                        break;
                    };
                    let init = if matches!(self.current_kind(), TokenKind::Punct(Punct::Assign)) {
                        self.bump()?;
                        Some(self.parse_assignment_expression_no_in()?)
                    } else {
                        None
                    };
                    declarators.push(VariableDeclarator {
                        target,
                        init,
                        span: Span::new(d_start, self.last_span_end()),
                    });
                }
                self.expect_punct(Punct::Semicolon)?;
                let test = if !matches!(self.current_kind(), TokenKind::Punct(Punct::Semicolon)) {
                    Some(self.parse_expression()?)
                } else {
                    None
                };
                self.expect_punct(Punct::Semicolon)?;
                let update = if !matches!(self.current_kind(), TokenKind::Punct(Punct::RParen)) {
                    Some(self.parse_expression()?)
                } else {
                    None
                };
                self.consume_cond_rparen()?;
                let body = self.parse_substatement()?;
                self.err_if_labelled_function(&body, "`for` statement")?;
                let end = self.last_span_end();
                Self::check_c_style_for_lexical_declarators(kind, &declarators)?;

                if matches!(kind, VariableKind::Let | VariableKind::Const) {
                    let mut head_names = Vec::new();
                    for d in &declarators {
                        head_names.extend(d.target.collect_names().iter().map(|n| n.name.clone()));
                    }
                    let mut body_vars = Vec::new();
                    self.collect_for_body_var_names(std::slice::from_ref(&body), &mut body_vars);
                    if let Some(dup) = head_names
                        .iter()
                        .find(|h| body_vars.iter().any(|v| v == *h))
                    {
                        return Err(ParseError {
                            span: Span::new(start, end),
                            message: format!(
                                "for-statement lexical binding `{dup}` conflicts with a var declaration in the body"
                            ),
                        });
                    }
                }
                let init_vs = VariableStatement {
                    kind,
                    declarators,
                    span: Span::new(kw_span.start, kw_span.end),
                };
                return Ok(desugar_using_for_statement(
                    init_vs,
                    test,
                    update,
                    body,
                    Span::new(start, end),
                ));
            }

        }

        if head_is_empty {
            self.bump()?;
        }

        let mut init_expr: Option<Expr> = None;
        if !head_is_empty && !matches!(self.current_kind(), TokenKind::Punct(Punct::Semicolon)) {

            let e = self.parse_expression_no_in()?;

            if self.is_ident("in") || self.is_contextual_keyword("of") {
                let is_of = self.is_contextual_keyword("of");
                self.reject_escaped_contextual_keyword(if is_of { "of" } else { "in" })?;

                if is_of && !await_form {
                    if let Expr::Identifier { name, span } = &e {
                        let raw = &self.source()[span.start..span.end];
                        if name == "async" && raw == "async" {
                            return Err(ParseError {
                                span: *span,
                                message: "`async` cannot be the for-of LHS (grammar lookahead restriction)".into(),
                            });
                        }
                    }
                }
                self.bump()?;
                if is_of {
                    self.current_lex_goal = LexerGoal::RegExp;
                }
                let right = if is_of {
                    self.parse_assignment_expression()?
                } else {
                    self.parse_expression()?
                };
                self.consume_cond_rparen()?;
                let body = self.parse_substatement()?;
                self.err_if_labelled_function(&body, "`for` statement")?;
                let end = self.last_span_end();
                let left = {
                    let span_fallback = e.span();

                    let is_pattern_literal = matches!(&e, Expr::Array { .. } | Expr::Object { .. });

                    let mut probe = &e;
                    while let Expr::Parenthesized { expr, .. } = probe {
                        probe = expr;
                    }
                    if matches!(probe, Expr::This { .. } | Expr::Super { .. }) {
                        return Err(ParseError {
                            span: e.span(),
                            message: "Invalid left-hand side in for-in/for-of head".into(),
                        });
                    }
                    if is_valid_for_assignment_target(&e) {
                        ForBinding::AssignmentTarget(e.clone())
                    } else if !self.strict_mode
                        && Self::is_call_assignment_target(&e)
                        && !self.is_tagged_template_call(&e)
                    {

                        ForBinding::AssignmentTarget(e.clone())
                    } else {
                        match expr_to_binding_pattern(e.clone()) {
                            Some(pat) => {

                                self.check_pattern_binding_ids(&pat, span_fallback)?;
                                ForBinding::Pattern(pat)
                            }
                            None if is_pattern_literal && is_valid_assignment_pattern_expr(&e) => {
                                ForBinding::AssignmentTarget(e)
                            }
                            None if is_pattern_literal => {
                                return Err(ParseError {
                                    span: span_fallback,
                                    message:
                                        "Invalid destructuring assignment target in for-in/for-of head"
                                            .into(),
                                });
                            }
                            None => {
                                return Err(ParseError {
                                    span: span_fallback,
                                    message: "Invalid left-hand side in for-in/for-of head".into(),
                                });
                            }
                        }
                    }
                };
                return if is_of {
                    Ok(Stmt::ForOf {
                        left,
                        right,
                        body: Box::new(body),
                        await_: await_form,
                        span: Span::new(start, end),
                    })
                } else {
                    if await_form {
                        return Err(ParseError {
                            span: Span::new(start, end),
                            message: "`for await` requires a for-of head".into(),
                        });
                    }
                    Ok(Stmt::ForIn {
                        left,
                        right,
                        body: Box::new(body),
                        span: Span::new(start, end),
                    })
                };
            }
            init_expr = Some(e);
        }
        if !head_is_empty {
            self.expect_punct(Punct::Semicolon)?;
        }
        if await_form {
            return Err(ParseError {
                span: Span::new(start, self.last_span_end()),
                message: "`for await` requires a for-of head".into(),
            });
        }
        let test = if !matches!(self.current_kind(), TokenKind::Punct(Punct::Semicolon)) {
            Some(self.parse_expression()?)
        } else {
            None
        };
        self.expect_punct(Punct::Semicolon)?;
        let update = if !matches!(self.current_kind(), TokenKind::Punct(Punct::RParen)) {
            Some(self.parse_expression()?)
        } else {
            None
        };
        self.consume_cond_rparen()?;
        let body = self.parse_substatement()?;
        self.err_if_labelled_function(&body, "`for` statement")?;
        let end = self.last_span_end();
        let init = init_expr.map(ForInit::Expression);
        Ok(Stmt::For {
            init,
            test,
            update,
            body: Box::new(body),
            span: Span::new(start, end),
        })
    }

    fn parse_while_statement(&mut self) -> Result<Stmt, ParseError> {
        let start = self.lookahead_span().start;
        self.expect_keyword("while")?;
        self.expect_punct(Punct::LParen)?;
        let test = self.parse_expression()?;
        self.consume_cond_rparen()?;
        let body = self.parse_substatement()?;
        self.err_if_labelled_function(&body, "`while` statement")?;
        let end = self.last_span_end();
        Ok(Stmt::While {
            test,
            body: Box::new(body),
            span: Span::new(start, end),
        })
    }

    fn parse_with_statement(&mut self) -> Result<Stmt, ParseError> {
        let start = self.lookahead_span().start;

        if self.strict_mode {
            return Err(self.err_here("`with` statements are not allowed in strict mode".into()));
        }
        self.expect_keyword("with")?;
        self.expect_punct(Punct::LParen)?;
        let object = self.parse_expression()?;
        self.consume_cond_rparen()?;
        let body = self.parse_substatement()?;
        self.err_if_labelled_function(&body, "`with` statement")?;
        let end = self.last_span_end();
        Ok(Stmt::With {
            object,
            body: Box::new(body),
            span: Span::new(start, end),
        })
    }

    fn parse_do_while_statement(&mut self) -> Result<Stmt, ParseError> {
        let start = self.lookahead_span().start;
        self.expect_keyword("do")?;
        let body = self.parse_substatement()?;
        self.err_if_labelled_function(&body, "`do`-`while` statement")?;
        self.expect_keyword("while")?;
        self.expect_punct(Punct::LParen)?;
        let test = self.parse_expression()?;

        self.consume_cond_rparen()?;
        if self.is_punct(Punct::Semicolon) {
            self.consume_semicolon_pub()?;
        }
        let end = self.last_span_end();
        Ok(Stmt::DoWhile {
            body: Box::new(body),
            test,
            span: Span::new(start, end),
        })
    }

    fn parse_switch_statement(&mut self) -> Result<Stmt, ParseError> {
        let start = self.lookahead_span().start;
        self.expect_keyword("switch")?;
        self.expect_punct(Punct::LParen)?;
        let discriminant = self.parse_expression()?;
        self.expect_punct(Punct::RParen)?;
        self.expect_punct(Punct::LBrace)?;
        let mut cases = Vec::new();
        let mut case_block_body = Vec::new();
        while !matches!(self.current_kind(), TokenKind::Punct(Punct::RBrace))
            && !self.at_eof_internal()
        {
            let case_start = self.lookahead_span().start;
            let test = if self.is_ident("case") {
                self.bump()?;
                let t = self.parse_expression()?;
                self.expect_punct(Punct::Colon)?;
                Some(t)
            } else if self.is_ident("default") {
                self.bump()?;
                self.expect_punct(Punct::Colon)?;
                None
            } else {
                return Err(self.err_here("expected `case` or `default` in switch body".into()));
            };
            let mut consequent = Vec::new();
            while !self.is_ident("case")
                && !self.is_ident("default")
                && !matches!(self.current_kind(), TokenKind::Punct(Punct::RBrace))
                && !self.at_eof_internal()
            {
                if self.is_ident("using")
                    && self.using_starts_declaration(self.lookahead_span().end)
                {
                    return Err(self
                        .err_here("`using` declaration is not allowed in switch clauses".into()));
                }
                if self.in_async && self.is_ident("await") && self.await_using_starts_declaration()
                {
                    return Err(self.err_here(
                        "`await using` declaration is not allowed in switch clauses".into(),
                    ));
                }
                consequent.push(self.parse_statement()?);
            }
            case_block_body.extend(consequent.iter().cloned());
            let case_end = self.last_span_end();
            cases.push(SwitchCase {
                test,
                consequent,
                span: Span::new(case_start, case_end),
            });
        }
        self.expect_punct(Punct::RBrace)?;
        self.check_block_bound_names(&case_block_body)?;
        let end = self.last_span_end();
        Ok(Stmt::Switch {
            discriminant,
            cases,
            span: Span::new(start, end),
        })
    }

    fn parse_try_statement(&mut self) -> Result<Stmt, ParseError> {
        let start = self.lookahead_span().start;
        self.expect_keyword("try")?;
        let block = self.parse_block_statement_public()?;
        let handler = if self.is_ident("catch") {
            let h_start = self.lookahead_span().start;
            self.bump()?;
            let param = if matches!(self.current_kind(), TokenKind::Punct(Punct::LParen)) {
                self.bump()?;
                let p = Some(self.parse_binding_target()?);
                self.expect_punct(Punct::RParen)?;
                p
            } else {
                None
            };
            let body = self.parse_block_statement_public()?;
            let h_end = self.last_span_end();

            if let Some(p) = &param {
                use std::collections::HashSet;
                let names: Vec<(String, Span)> = p
                    .collect_names()
                    .iter()
                    .map(|id| (id.name.clone(), id.span))
                    .collect();
                let mut seen: HashSet<&str> = HashSet::new();
                for (n, sp) in &names {
                    if !seen.insert(n.as_str()) {
                        return Err(self
                            .err_at(*sp, format!("catch parameter has duplicate binding `{n}`")));
                    }
                }
                if let Stmt::Block { body: stmts, .. } = &body {
                    let mut entries: Vec<(String, Span, u32, bool, bool, bool, bool)> = Vec::new();
                    let mut next_id: u32 = 0;
                    self.collect_block_entries(stmts, false, false, &mut entries, &mut next_id);
                    let lex: HashSet<&str> = entries
                        .iter()
                        .filter(|e| e.3)
                        .map(|e| e.0.as_str())
                        .collect();
                    for (n, sp) in &names {
                        if lex.contains(n.as_str()) {
                            return Err(self.err_at(
                                *sp,
                                format!(
                                    "catch parameter `{n}` is redeclared as a lexical binding in the catch block"
                                ),
                            ));
                        }
                    }
                }
            }
            Some(CatchClause {
                param,
                body: Box::new(body),
                span: Span::new(h_start, h_end),
            })
        } else {
            None
        };
        let finalizer = if self.is_ident("finally") {
            self.bump()?;
            Some(Box::new(self.parse_block_statement_public()?))
        } else {
            None
        };
        let end = self.last_span_end();
        Ok(Stmt::Try {
            disposal: false,
            block: Box::new(block),
            handler,
            finalizer,
            span: Span::new(start, end),
        })
    }

    fn parse_return_statement(&mut self) -> Result<Stmt, ParseError> {
        let start = self.lookahead_span().start;
        if self.function_body_depth == 0 {
            return Err(self.err_here("Illegal return statement".into()));
        }
        self.expect_keyword("return")?;

        let argument = if matches!(self.current_kind(), TokenKind::Punct(Punct::Semicolon))
            || matches!(self.current_kind(), TokenKind::Punct(Punct::RBrace))
            || matches!(self.current_kind(), TokenKind::Eof)
            || self.lookahead_preceded_by_lt()
        {
            None
        } else {
            Some(self.parse_expression()?)
        };
        self.consume_semicolon_pub()?;
        let end = self.last_span_end();
        Ok(Stmt::Return {
            argument,
            span: Span::new(start, end),
        })
    }

    fn parse_throw_statement(&mut self) -> Result<Stmt, ParseError> {
        let start = self.lookahead_span().start;
        self.expect_keyword("throw")?;
        if self.lookahead_preceded_by_lt() {
            return Err(self
                .err_here("no line terminator permitted between `throw` and its argument".into()));
        }
        let argument = self.parse_expression()?;
        self.consume_semicolon_pub()?;
        let end = self.last_span_end();
        Ok(Stmt::Throw {
            argument,
            span: Span::new(start, end),
        })
    }

    fn parse_break_statement(&mut self) -> Result<Stmt, ParseError> {
        let start = self.lookahead_span().start;
        self.expect_keyword("break")?;
        let label = self.parse_optional_label()?;
        self.consume_semicolon_pub()?;
        let end = self.last_span_end();
        Ok(Stmt::Break {
            label,
            span: Span::new(start, end),
        })
    }

    fn parse_continue_statement(&mut self) -> Result<Stmt, ParseError> {
        let start = self.lookahead_span().start;
        self.expect_keyword("continue")?;
        let label = self.parse_optional_label()?;
        self.consume_semicolon_pub()?;
        let end = self.last_span_end();
        Ok(Stmt::Continue {
            label,
            span: Span::new(start, end),
        })
    }

    fn parse_optional_label(&mut self) -> Result<Option<BindingIdentifier>, ParseError> {

        if self.lookahead_preceded_by_lt() {
            return Ok(None);
        }
        if let TokenKind::Ident(n) = self.current_kind().clone() {

            if !matches!(n.as_str(), "else") {
                let span = self.lookahead_span();
                self.bump()?;
                return Ok(Some(BindingIdentifier { name: n, span }));
            }
        }
        Ok(None)
    }

    fn parse_block_statement_public(&mut self) -> Result<Stmt, ParseError> {
        self.parse_block_statement()
    }

    pub(crate) fn check_block_bound_names(&self, body: &[Stmt]) -> Result<(), ParseError> {

        let mut entries: Vec<(String, Span, u32, bool, bool, bool, bool)> = Vec::new();
        let mut next_id: u32 = 0;
        self.collect_block_entries(body, false, false, &mut entries, &mut next_id);
        self.detect_dup_bound_names(&entries)
    }

    pub(crate) fn check_function_body_bound_names(&self, body: &[Stmt]) -> Result<(), ParseError> {
        let mut entries: Vec<(String, Span, u32, bool, bool, bool, bool)> = Vec::new();
        let mut next_id: u32 = 0;
        self.collect_block_entries(body, false, true, &mut entries, &mut next_id);
        self.detect_dup_bound_names(&entries)
    }

    pub(crate) fn check_top_level_bound_names(
        &self,
        items: &[rusty_js_ast::ModuleItem],
    ) -> Result<(), ParseError> {
        use rusty_js_ast::ModuleItem;
        let mut entries: Vec<(String, Span, u32, bool, bool, bool, bool)> = Vec::new();
        let mut next_id: u32 = 0;
        for it in items {
            if let ModuleItem::Statement(s) = it {
                self.collect_block_entries(
                    std::slice::from_ref(s),
                    false,
                    true,
                    &mut entries,
                    &mut next_id,
                );
            }
        }
        self.detect_dup_bound_names(&entries)
    }

    fn detect_dup_bound_names(
        &self,
        entries: &[(String, Span, u32, bool, bool, bool, bool)],
    ) -> Result<(), ParseError> {
        use std::collections::HashMap;
        let mut by_name: HashMap<&str, Vec<&(String, Span, u32, bool, bool, bool, bool)>> =
            HashMap::new();
        for e in entries {
            by_name.entry(&e.0).or_default().push(e);
        }
        for (_, es) in by_name {

            let mut lex_ids: Vec<(u32, bool)> = Vec::new();
            for e in &es {
                if e.3 {
                    if !lex_ids.iter().any(|(id, _)| *id == e.2) {
                        lex_ids.push((e.2, e.5));
                    }
                }
            }
            if lex_ids.len() >= 2 {
                let all_plain_func = lex_ids.iter().all(|(_, pfn)| *pfn);
                if !all_plain_func {
                    let bad = es.iter().find(|e| e.3).unwrap();
                    return Err(ParseError {
                        span: bad.1,
                        message: format!(
                            "Identifier `{}` has already been declared in this block",
                            bad.0
                        ),
                    });
                }
            }

            let lex_pairs: Vec<(u32, bool)> =
                es.iter().filter(|e| e.3).map(|e| (e.2, e.5)).collect();
            let var_pairs: Vec<(u32, bool, bool)> =
                es.iter().filter(|e| e.4).map(|e| (e.2, e.5, e.6)).collect();

            let cross = lex_pairs.iter().any(|(li, _lpf)| {
                var_pairs
                    .iter()
                    .any(|(vi, vpf, _nested_var)| li != vi && !*vpf)
            });
            if cross {
                let bad = es.iter().find(|e| e.3).unwrap();
                return Err(ParseError {
                    span: bad.1,
                    message: format!(
                        "Identifier `{}` cannot be redeclared (lexical/var conflict)",
                        bad.0
                    ),
                });
            }
        }
        Ok(())
    }

    fn collect_block_entries(
        &self,
        body: &[Stmt],
        nested: bool,
        body_scope: bool,
        out: &mut Vec<(String, Span, u32, bool, bool, bool, bool)>,
        next_id: &mut u32,
    ) {
        use rusty_js_ast::{Stmt as S, VariableKind};
        for s in body {
            match s {
                S::Variable(vs) => {
                    let id = *next_id;
                    *next_id += 1;
                    let (is_lex, is_var) = match vs.kind {

                        VariableKind::Let
                        | VariableKind::Const
                        | VariableKind::Using
                        | VariableKind::AwaitUsing => (!nested, false),
                        VariableKind::Var => (false, true),
                    };
                    if !is_lex && !is_var {
                        continue;
                    }
                    for d in &vs.declarators {
                        for nm in d.target.collect_names() {
                            out.push((
                                nm.name.clone(),
                                nm.span,
                                id,
                                is_lex,
                                is_var,
                                false,
                                nested && is_var,
                            ));
                        }
                    }
                }
                S::FunctionDecl {
                    name: Some(n),
                    is_async,
                    is_generator,
                    ..
                } => {
                    let id = *next_id;
                    *next_id += 1;
                    let is_plain = !is_async && !is_generator;
                    let plain_func_nonstrict = is_plain && !self.strict_mode;

                    if nested {
                        if plain_func_nonstrict {
                            out.push((n.name.clone(), n.span, id, false, true, true, false));
                        }
                    } else if body_scope {

                        out.push((
                            n.name.clone(),
                            n.span,
                            id,
                            false,
                            true,
                            plain_func_nonstrict,
                            false,
                        ));
                    } else {
                        let is_lex = true;
                        let is_var = plain_func_nonstrict;
                        out.push((
                            n.name.clone(),
                            n.span,
                            id,
                            is_lex,
                            is_var,
                            plain_func_nonstrict,
                            false,
                        ));
                    }
                }
                S::ClassDecl { name: Some(n), .. } => {
                    if !nested {
                        let id = *next_id;
                        *next_id += 1;
                        out.push((n.name.clone(), n.span, id, true, false, false, false));
                    }
                }

                S::Block { body: inner, .. } => {
                    self.collect_block_entries(inner, true, false, out, next_id);
                }
                S::If {
                    consequent,
                    alternate,
                    ..
                } => {
                    self.collect_stmt_entries(consequent, true, false, out, next_id);
                    if let Some(a) = alternate {
                        self.collect_stmt_entries(a, true, false, out, next_id);
                    }
                }
                S::For { body: b, .. }
                | S::ForIn { body: b, .. }
                | S::ForOf { body: b, .. }
                | S::While { body: b, .. }
                | S::DoWhile { body: b, .. } => {
                    self.collect_stmt_entries(b, true, false, out, next_id);
                }
                S::Switch { cases, .. } => {
                    for c in cases {
                        self.collect_block_entries(&c.consequent, true, false, out, next_id);
                    }
                }
                S::Try {
                    block,
                    handler,
                    finalizer,
                    ..
                } => {
                    self.collect_stmt_entries(block, true, false, out, next_id);
                    if let Some(h) = handler {
                        self.collect_stmt_entries(&h.body, true, false, out, next_id);
                    }
                    if let Some(f) = finalizer {
                        self.collect_stmt_entries(f, true, false, out, next_id);
                    }
                }
                S::Labelled { body: b, .. } => {
                    self.collect_stmt_entries(b, true, false, out, next_id);
                }
                _ => {}
            }
        }
    }

    fn collect_stmt_entries(
        &self,
        s: &Stmt,
        nested: bool,
        body_scope: bool,
        out: &mut Vec<(String, Span, u32, bool, bool, bool, bool)>,
        next_id: &mut u32,
    ) {
        let slice = std::slice::from_ref(s);
        self.collect_block_entries(slice, nested, body_scope, out, next_id);
    }

    pub(crate) fn check_pattern_binding_ids(
        &self,
        pat: &BindingPattern,
        span: Span,
    ) -> Result<(), ParseError> {
        match pat {
            BindingPattern::Identifier(id) => {
                let n = &id.name;
                if self.strict_mode && (n == "eval" || n == "arguments") {
                    return Err(ParseError {
                        span: id.span,
                        message: format!("`{}` is not a valid binding in strict mode", n),
                    });
                }
                if (self.in_generator || self.strict_mode) && n == "yield" {
                    return Err(ParseError {
                        span: id.span,
                        message: "`yield` is not a valid binding in this context".into(),
                    });
                }
                Ok(())
            }
            BindingPattern::Array(ap) => {
                for el in &ap.elements {
                    if let Some(be) = el {
                        self.check_pattern_binding_ids(&be.target, span)?;
                    }
                }
                if let Some(r) = &ap.rest {
                    self.check_pattern_binding_ids(r, span)?;
                }
                Ok(())
            }
            BindingPattern::Object(op) => {
                for prop in &op.properties {
                    self.check_pattern_binding_ids(&prop.value.target, span)?;
                }
                if let Some(r) = &op.rest {
                    let n = &r.name;
                    if self.strict_mode && (n == "eval" || n == "arguments") {
                        return Err(ParseError {
                            span: r.span,
                            message: format!("`{}` is not a valid binding in strict mode", n),
                        });
                    }
                    if (self.in_generator || self.strict_mode) && n == "yield" {
                        return Err(ParseError {
                            span: r.span,
                            message: "`yield` is not a valid binding in this context".into(),
                        });
                    }
                }
                Ok(())
            }
        }
    }
}
