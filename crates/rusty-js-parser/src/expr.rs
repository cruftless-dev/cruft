
use crate::parser::{parse_profile, ParseError, Parser};
use crate::token::{Punct, TokenKind};
use rusty_js_ast::{
    Argument, ArrayElement, ArrowBody, AssignOp, BinaryOp, Expr, MemberProperty, ObjectKey,
    ObjectProperty, ObjectPropertyKind, Span, UnaryOp, UpdateOp,
};

impl<'src> Parser<'src> {

    pub fn parse_assignment_expression(&mut self) -> Result<Expr, ParseError> {
        let _top_var_init_ident_assign_frame =
            parse_profile::TopVarInitIdentAssignFrameGuard::new();

        if self.in_generator {
            if matches!(self.current_kind(), TokenKind::Ident(s) if s == "yield") {
                return self.parse_yield_expression();
            }
        }

        if self.is_ident("async") {
            let _profile =
                parse_profile::nested_phase_guard(parse_profile::Kind::NestedAssignAsync);
            let top_profile_start = parse_profile::top_var_init_ident_assign_phase_start();
            let top_arrow_expr_start =
                parse_profile::top_var_init_ident_arrow_expr_assign_phase_start();
            let async_start = self.lookahead_span().start;
            let pos = self.lookahead_span().end;
            let bytes = self.source().as_bytes();
            let Some(p) = Self::skip_ws_and_comments(bytes, pos, false) else {
                parse_profile::record_top_var_init_ident_assign_phase(
                    parse_profile::TopVarInitIdentAssignPhase::AsyncDisambig,
                    top_profile_start,
                );
                parse_profile::record_top_var_init_ident_arrow_expr_assign_phase(
                    parse_profile::TopVarInitIdentArrowExprAssignPhase::AsyncDisambig,
                    top_arrow_expr_start,
                );
                return self.parse_conditional_expression();
            };
            if Self::bytes_at_identifier_keyword(bytes, p, "function") {
                self.reject_escaped_contextual_keyword("async")?;
                self.bump()?;
                let f = self.parse_function_expression(true, Some(async_start))?;
                parse_profile::record_top_var_init_ident_assign_phase(
                    parse_profile::TopVarInitIdentAssignPhase::AsyncDisambig,
                    top_profile_start,
                );
                parse_profile::record_top_var_init_ident_arrow_expr_assign_phase(
                    parse_profile::TopVarInitIdentArrowExprAssignPhase::AsyncDisambig,
                    top_arrow_expr_start,
                );
                return self.continue_lhs_continuation(f);
            }
            let starts_paren = bytes.get(p) == Some(&b'(');
            let starts_ident = p < bytes.len()
                && (bytes[p].is_ascii_alphabetic() || bytes[p] == b'_' || bytes[p] == b'$');

            if starts_paren || starts_ident {
                let mut q = p;
                if bytes.get(q) == Some(&b'(') {
                    let mut depth = 1i32;
                    q += 1;
                    while q < bytes.len() && depth > 0 {
                        match bytes[q] {
                            b'(' => depth += 1,
                            b')' => depth -= 1,
                            _ => {}
                        }
                        q += 1;
                    }
                } else {
                    while q < bytes.len()
                        && (bytes[q].is_ascii_alphanumeric()
                            || bytes[q] == b'_'
                            || bytes[q] == b'$')
                    {
                        q += 1;
                    }
                }
                let Some(q) = Self::skip_ws_and_comments(bytes, q, false) else {
                    parse_profile::record_top_var_init_ident_assign_phase(
                        parse_profile::TopVarInitIdentAssignPhase::AsyncDisambig,
                        top_profile_start,
                    );
                    parse_profile::record_top_var_init_ident_arrow_expr_assign_phase(
                        parse_profile::TopVarInitIdentArrowExprAssignPhase::AsyncDisambig,
                        top_arrow_expr_start,
                    );
                    return self.parse_conditional_expression();
                };
                if bytes.get(q) == Some(&b'=') && bytes.get(q + 1) == Some(&b'>') {
                    self.reject_escaped_contextual_keyword("async")?;
                    self.bump()?;
                    parse_profile::record_top_var_init_ident_assign_phase(
                        parse_profile::TopVarInitIdentAssignPhase::AsyncDisambig,
                        top_profile_start,
                    );
                    parse_profile::record_top_var_init_ident_arrow_expr_assign_phase(
                        parse_profile::TopVarInitIdentArrowExprAssignPhase::AsyncDisambig,
                        top_arrow_expr_start,
                    );
                    let top_arrow_start = parse_profile::top_var_init_ident_assign_phase_start();
                    let parsed = self.parse_arrow_function(true, Some(async_start));
                    parse_profile::record_top_var_init_ident_assign_phase(
                        parse_profile::TopVarInitIdentAssignPhase::ArrowParse,
                        top_arrow_start,
                    );
                    return parsed;
                }
            }
            parse_profile::record_top_var_init_ident_assign_phase(
                parse_profile::TopVarInitIdentAssignPhase::AsyncDisambig,
                top_profile_start,
            );
            parse_profile::record_top_var_init_ident_arrow_expr_assign_phase(
                parse_profile::TopVarInitIdentArrowExprAssignPhase::AsyncDisambig,
                top_arrow_expr_start,
            );
        }

        {
            let _profile =
                parse_profile::nested_phase_guard(parse_profile::Kind::NestedAssignArrow);
            let top_profile_start = parse_profile::top_var_init_ident_assign_phase_start();
            let top_arrow_expr_start =
                parse_profile::top_var_init_ident_arrow_expr_assign_phase_start();
            if self.looks_like_arrow_function_head() {
                parse_profile::record_top_var_init_ident_assign_phase(
                    parse_profile::TopVarInitIdentAssignPhase::ArrowProbe,
                    top_profile_start,
                );
                parse_profile::record_top_var_init_ident_arrow_expr_assign_phase(
                    parse_profile::TopVarInitIdentArrowExprAssignPhase::ArrowProbe,
                    top_arrow_expr_start,
                );
                let top_arrow_start = parse_profile::top_var_init_ident_assign_phase_start();
                let parsed = self.parse_arrow_function(false, None);
                parse_profile::record_top_var_init_ident_assign_phase(
                    parse_profile::TopVarInitIdentAssignPhase::ArrowParse,
                    top_arrow_start,
                );
                return parsed;
            }
            parse_profile::record_top_var_init_ident_assign_phase(
                parse_profile::TopVarInitIdentAssignPhase::ArrowProbe,
                top_profile_start,
            );
            parse_profile::record_top_var_init_ident_arrow_expr_assign_phase(
                parse_profile::TopVarInitIdentArrowExprAssignPhase::ArrowProbe,
                top_arrow_expr_start,
            );
        }

        let saved_allow_cover = self.allow_cover_initialized_name_in_for_head;
        self.allow_cover_initialized_name_in_for_head = true;
        let left = {
            let _profile =
                parse_profile::nested_phase_guard(parse_profile::Kind::NestedAssignConditional);
            let top_profile_start = parse_profile::top_var_init_ident_assign_phase_start();
            let top_arrow_expr_start =
                parse_profile::top_var_init_ident_arrow_expr_assign_phase_start();
            let parsed = self.parse_conditional_expression();
            parse_profile::record_top_var_init_ident_assign_phase(
                parse_profile::TopVarInitIdentAssignPhase::Conditional,
                top_profile_start,
            );
            parse_profile::record_top_var_init_ident_arrow_expr_assign_phase(
                parse_profile::TopVarInitIdentArrowExprAssignPhase::Conditional,
                top_arrow_expr_start,
            );
            parsed
        };
        self.allow_cover_initialized_name_in_for_head = saved_allow_cover;
        let left = left?;
        if let Some(op) = self.peek_assign_op() {
            if !Self::is_valid_assignment_target(&left, op) {

                let logical = matches!(
                    op,
                    AssignOp::LogicalAndAssign
                        | AssignOp::LogicalOrAssign
                        | AssignOp::NullishAssign
                );
                if self.strict_mode
                    || logical
                    || self.is_tagged_template_call(&left)
                    || !Self::is_call_assignment_target(&left)
                {
                    return Err(ParseError {
                        span: left.span(),
                        message: "invalid assignment target".into(),
                    });
                }
            }

            if self.strict_mode {
                if let Some((name, id_span)) = Self::strict_eval_arguments_target(&left) {
                    return Err(ParseError {
                        span: id_span,
                        message: format!(
                            "`{}` is not a valid assignment target in strict mode",
                            name
                        ),
                    });
                }
            }

            if matches!(op, AssignOp::Assign) {
                if matches!(left, Expr::Array { .. } | Expr::Object { .. }) {
                    if let Some(span) = Self::invalid_destructuring_assignment_pattern_span(&left) {
                        return Err(ParseError {
                            span,
                            message: "invalid destructuring assignment target".into(),
                        });
                    }
                }
                if let Expr::Array {
                    elements,
                    trailing_comma_after_spread,
                    ..
                } = &left
                {
                    if let Some(pos) = elements
                        .iter()
                        .position(|e| matches!(e, ArrayElement::Spread { .. }))
                    {
                        if pos != elements.len() - 1 || *trailing_comma_after_spread {
                            let sp = match &elements[pos] {
                                ArrayElement::Spread { span, .. } => *span,
                                _ => left.span(),
                            };
                            return Err(ParseError {
                                span: sp,
                                message:
                                    "rest element must be last in a destructuring assignment target"
                                        .into(),
                            });
                        }
                    }
                }
                if let Expr::Object { properties, .. } = &left {
                    if let Some(pos) = properties
                        .iter()
                        .position(|p| matches!(p, ObjectProperty::Spread { .. }))
                    {
                        if pos != properties.len() - 1 {
                            let sp = match &properties[pos] {
                                ObjectProperty::Spread { span, .. } => *span,
                                _ => left.span(),
                            };
                            return Err(ParseError {
                                span: sp,
                                message:
                                    "rest property must be last in a destructuring assignment target"
                                        .into(),
                            });
                        }
                    }

                }
            }
            self.bump()?;
            let value = {
                let _profile =
                    parse_profile::nested_phase_guard(parse_profile::Kind::NestedAssignRhs);
                let top_profile_start = parse_profile::top_var_init_ident_assign_phase_start();
                let top_arrow_expr_start =
                    parse_profile::top_var_init_ident_arrow_expr_assign_phase_start();
                let value = self.parse_assignment_expression()?;
                parse_profile::record_top_var_init_ident_assign_phase(
                    parse_profile::TopVarInitIdentAssignPhase::Rhs,
                    top_profile_start,
                );
                parse_profile::record_top_var_init_ident_arrow_expr_assign_phase(
                    parse_profile::TopVarInitIdentArrowExprAssignPhase::Rhs,
                    top_arrow_expr_start,
                );
                value
            };
            let span = Span::new(left.span().start, value.span().end);
            return Ok(Expr::Assign {
                operator: op,
                target: Box::new(left),
                value: Box::new(value),
                span,
            });
        }
        if !self.allow_cover_initialized_name_in_for_head {
            let _profile =
                parse_profile::nested_phase_guard(parse_profile::Kind::NestedAssignCover);
            let top_profile_start = parse_profile::top_var_init_ident_assign_phase_start();
            let top_arrow_expr_start =
                parse_profile::top_var_init_ident_arrow_expr_assign_phase_start();
            if let Some(span) = Self::cover_initialized_name_span(&left) {
                parse_profile::record_top_var_init_ident_assign_phase(
                    parse_profile::TopVarInitIdentAssignPhase::Cover,
                    top_profile_start,
                );
                parse_profile::record_top_var_init_ident_arrow_expr_assign_phase(
                    parse_profile::TopVarInitIdentArrowExprAssignPhase::Cover,
                    top_arrow_expr_start,
                );
                return Err(ParseError {
                    span,
                    message: "CoverInitializedName is not allowed in object initializers".into(),
                });
            }
            parse_profile::record_top_var_init_ident_assign_phase(
                parse_profile::TopVarInitIdentAssignPhase::Cover,
                top_profile_start,
            );
            parse_profile::record_top_var_init_ident_arrow_expr_assign_phase(
                parse_profile::TopVarInitIdentArrowExprAssignPhase::Cover,
                top_arrow_expr_start,
            );

            if let Some(span) = Self::duplicate_proto_span(&left) {
                return Err(ParseError {
                    span,
                    message: "duplicate __proto__ fields are not allowed in object literals".into(),
                });
            }
        }
        Ok(left)
    }

    fn cover_initialized_name_span(expr: &Expr) -> Option<Span> {
        match expr {
            Expr::Object { properties, .. } => properties.iter().find_map(|prop| match prop {
                ObjectProperty::Property {
                    value,
                    shorthand: true,
                    ..
                } if matches!(
                    value,
                    Expr::Assign {
                        operator: AssignOp::Assign,
                        ..
                    }
                ) =>
                {
                    Some(value.span())
                }
                ObjectProperty::Property { value, .. } => Self::cover_initialized_name_span(value),
                ObjectProperty::Spread { expr, .. } => Self::cover_initialized_name_span(expr),
            }),
            Expr::Array { elements, .. } => elements.iter().find_map(|element| match element {
                ArrayElement::Expr(expr) | ArrayElement::Spread { expr, .. } => {
                    Self::cover_initialized_name_span(expr)
                }
                ArrayElement::Elision { .. } => None,
            }),

            Expr::Assign { value, .. } => Self::cover_initialized_name_span(value),
            Expr::Conditional {
                test,
                consequent,
                alternate,
                ..
            } => Self::cover_initialized_name_span(test)
                .or_else(|| Self::cover_initialized_name_span(consequent))
                .or_else(|| Self::cover_initialized_name_span(alternate)),
            Expr::Binary { left, right, .. } => Self::cover_initialized_name_span(left)
                .or_else(|| Self::cover_initialized_name_span(right)),
            Expr::Unary { argument, .. } | Expr::Update { argument, .. } => {
                Self::cover_initialized_name_span(argument)
            }
            Expr::Call {
                callee, arguments, ..
            } => Self::cover_initialized_name_span(callee).or_else(|| {
                arguments.iter().find_map(|arg| match arg {
                    Argument::Expr(expr) | Argument::Spread { expr, .. } => {
                        Self::cover_initialized_name_span(expr)
                    }
                })
            }),
            Expr::Parenthesized { expr, .. } => Self::cover_initialized_name_span(expr),
            _ => None,
        }
    }

    fn duplicate_proto_span(expr: &Expr) -> Option<Span> {
        match expr {
            Expr::Object { properties, .. } => {
                let mut seen = false;
                for prop in properties {
                    if let ObjectProperty::Property {
                        key,
                        value,
                        shorthand: false,
                        kind: ObjectPropertyKind::Init,
                        ..
                    } = prop
                    {
                        let is_method_value = matches!(
                            value,
                            Expr::Function {
                                is_method: true,
                                ..
                            }
                        );
                        if !is_method_value
                            && !matches!(key, ObjectKey::Computed { .. })
                            && object_key_static_name(key).as_deref() == Some("__proto__")
                        {
                            if seen {
                                return Some(object_key_span(key));
                            }
                            seen = true;
                        }
                    }
                }
                properties.iter().find_map(|prop| match prop {
                    ObjectProperty::Property { value, .. } => Self::duplicate_proto_span(value),
                    ObjectProperty::Spread { expr, .. } => Self::duplicate_proto_span(expr),
                })
            }
            Expr::Array { elements, .. } => elements.iter().find_map(|element| match element {
                ArrayElement::Expr(expr) | ArrayElement::Spread { expr, .. } => {
                    Self::duplicate_proto_span(expr)
                }
                ArrayElement::Elision { .. } => None,
            }),
            Expr::Assign { value, .. } => Self::duplicate_proto_span(value),
            Expr::Conditional {
                test,
                consequent,
                alternate,
                ..
            } => Self::duplicate_proto_span(test)
                .or_else(|| Self::duplicate_proto_span(consequent))
                .or_else(|| Self::duplicate_proto_span(alternate)),
            Expr::Binary { left, right, .. } => {
                Self::duplicate_proto_span(left).or_else(|| Self::duplicate_proto_span(right))
            }
            Expr::Unary { argument, .. } | Expr::Update { argument, .. } => {
                Self::duplicate_proto_span(argument)
            }
            Expr::Call {
                callee, arguments, ..
            } => Self::duplicate_proto_span(callee).or_else(|| {
                arguments.iter().find_map(|arg| match arg {
                    Argument::Expr(expr) | Argument::Spread { expr, .. } => {
                        Self::duplicate_proto_span(expr)
                    }
                })
            }),
            Expr::Parenthesized { expr, .. } => Self::duplicate_proto_span(expr),
            _ => None,
        }
    }

    fn peek_assign_op(&self) -> Option<AssignOp> {
        match self.current_kind() {
            TokenKind::Punct(Punct::Assign) => Some(AssignOp::Assign),
            TokenKind::Punct(Punct::PlusAssign) => Some(AssignOp::AddAssign),
            TokenKind::Punct(Punct::MinusAssign) => Some(AssignOp::SubAssign),
            TokenKind::Punct(Punct::StarAssign) => Some(AssignOp::MulAssign),
            TokenKind::Punct(Punct::SlashAssign) => Some(AssignOp::DivAssign),
            TokenKind::Punct(Punct::PercentAssign) => Some(AssignOp::ModAssign),
            TokenKind::Punct(Punct::StarStarAssign) => Some(AssignOp::PowAssign),
            TokenKind::Punct(Punct::ShlAssign) => Some(AssignOp::ShlAssign),
            TokenKind::Punct(Punct::ShrAssign) => Some(AssignOp::ShrAssign),
            TokenKind::Punct(Punct::UShrAssign) => Some(AssignOp::UShrAssign),
            TokenKind::Punct(Punct::BitAndAssign) => Some(AssignOp::BitAndAssign),
            TokenKind::Punct(Punct::BitOrAssign) => Some(AssignOp::BitOrAssign),
            TokenKind::Punct(Punct::BitXorAssign) => Some(AssignOp::BitXorAssign),
            TokenKind::Punct(Punct::LogicalAndAssign) => Some(AssignOp::LogicalAndAssign),
            TokenKind::Punct(Punct::LogicalOrAssign) => Some(AssignOp::LogicalOrAssign),
            TokenKind::Punct(Punct::NullishAssign) => Some(AssignOp::NullishAssign),
            _ => None,
        }
    }

    fn parse_conditional_expression(&mut self) -> Result<Expr, ParseError> {
        let _profile = parse_profile::Guard::new(parse_profile::Kind::ExprConditional);
        let _paren_profile = parse_profile::paren_guard(parse_profile::Kind::ParenConditional);
        let cond_test_start = parse_profile::top_var_init_ident_arrow_expr_cond_phase_start();
        let test = self.parse_binary_expression(0)?;
        parse_profile::record_top_var_init_ident_arrow_expr_cond_phase(
            parse_profile::TopVarInitIdentArrowExprCondPhase::Test,
            cond_test_start,
        );
        if matches!(self.current_kind(), TokenKind::Punct(Punct::Question)) {
            let cond_question_start =
                parse_profile::top_var_init_ident_arrow_expr_cond_phase_start();
            self.bump()?;
            parse_profile::record_top_var_init_ident_arrow_expr_cond_phase(
                parse_profile::TopVarInitIdentArrowExprCondPhase::Question,
                cond_question_start,
            );

            let saved_in_disallowed = self.in_disallowed;
            self.in_disallowed = false;
            let cond_consequent_start =
                parse_profile::top_var_init_ident_arrow_expr_cond_phase_start();
            let consequent = self.parse_assignment_expression()?;
            parse_profile::record_top_var_init_ident_arrow_expr_cond_phase(
                parse_profile::TopVarInitIdentArrowExprCondPhase::Consequent,
                cond_consequent_start,
            );
            self.in_disallowed = saved_in_disallowed;
            self.expect_punct(Punct::Colon)?;
            let cond_alternate_start =
                parse_profile::top_var_init_ident_arrow_expr_cond_phase_start();
            let alternate = self.parse_assignment_expression()?;
            parse_profile::record_top_var_init_ident_arrow_expr_cond_phase(
                parse_profile::TopVarInitIdentArrowExprCondPhase::Alternate,
                cond_alternate_start,
            );
            let span = Span::new(test.span().start, alternate.span().end);
            return Ok(Expr::Conditional {
                test: Box::new(test),
                consequent: Box::new(consequent),
                alternate: Box::new(alternate),
                span,
            });
        }
        Ok(test)
    }

    fn parse_binary_expression(&mut self, min_prec: u8) -> Result<Expr, ParseError> {
        let _profile = parse_profile::Guard::new(parse_profile::Kind::ExprBinary);
        let _paren_profile = parse_profile::paren_guard(parse_profile::Kind::ParenBinary);
        let left_unary_start =
            parse_profile::top_var_init_ident_arrow_expr_cond_binary_phase_start(min_prec);
        let mut left = self.parse_unary_expression()?;
        parse_profile::record_top_var_init_ident_arrow_expr_cond_binary_phase(
            parse_profile::TopVarInitIdentArrowExprCondBinaryPhase::LeftUnary,
            left_unary_start,
        );

        let private_check_start =
            parse_profile::top_var_init_ident_arrow_expr_cond_binary_phase_start(min_prec);
        if self.is_lowered_private_name(&left) {
            let consumed_by_in = matches!(
                self.peek_binary_op(),
                Some((BinaryOp::In, prec, _)) if prec >= min_prec
            );
            if !consumed_by_in {
                return Err(ParseError {
                    span: left.span(),
                    message: "a private name is only valid as the left operand of `in`".into(),
                });
            }
        }
        parse_profile::record_top_var_init_ident_arrow_expr_cond_binary_phase(
            parse_profile::TopVarInitIdentArrowExprCondBinaryPhase::PrivateCheck,
            private_check_start,
        );
        loop {
            let op_scan_start =
                parse_profile::top_var_init_ident_arrow_expr_cond_binary_phase_start(min_prec);
            let Some((op, prec, right_assoc)) = self.peek_binary_op() else {
                parse_profile::record_top_var_init_ident_arrow_expr_cond_binary_phase(
                    parse_profile::TopVarInitIdentArrowExprCondBinaryPhase::OpScan,
                    op_scan_start,
                );
                break;
            };
            if prec < min_prec {
                parse_profile::record_top_var_init_ident_arrow_expr_cond_binary_phase(
                    parse_profile::TopVarInitIdentArrowExprCondBinaryPhase::OpScan,
                    op_scan_start,
                );
                break;
            }
            self.bump()?;
            let next_min = if right_assoc { prec } else { prec + 1 };
            parse_profile::record_top_var_init_ident_arrow_expr_cond_binary_phase(
                parse_profile::TopVarInitIdentArrowExprCondBinaryPhase::OpScan,
                op_scan_start,
            );
            let rhs_start =
                parse_profile::top_var_init_ident_arrow_expr_cond_binary_phase_start(min_prec);
            let right = self.parse_binary_expression(next_min)?;
            parse_profile::record_top_var_init_ident_arrow_expr_cond_binary_phase(
                parse_profile::TopVarInitIdentArrowExprCondBinaryPhase::Rhs,
                rhs_start,
            );
            let assemble_start =
                parse_profile::top_var_init_ident_arrow_expr_cond_binary_phase_start(min_prec);
            let span = Span::new(left.span().start, right.span().end);
            if Self::mixes_nullish_with_logical(op, &left, &right) {
                return Err(ParseError {
                    span,
                    message: "nullish coalescing cannot be mixed with && or || without parentheses"
                        .into(),
                });
            }
            left = Expr::Binary {
                operator: op,
                left: Box::new(left),
                right: Box::new(right),
                span,
            };
            parse_profile::record_top_var_init_ident_arrow_expr_cond_binary_phase(
                parse_profile::TopVarInitIdentArrowExprCondBinaryPhase::Assemble,
                assemble_start,
            );
        }
        Ok(left)
    }

    fn is_lowered_private_name(&self, e: &Expr) -> bool {
        if let Expr::StringLiteral { value, span } = e {
            return value.starts_with('#')
                && self.source().as_bytes().get(span.start) == Some(&b'#');
        }
        false
    }

    fn peek_binary_op(&self) -> Option<(BinaryOp, u8, bool)> {
        match self.current_kind() {

            TokenKind::Punct(Punct::NullishCoalesce) => Some((BinaryOp::NullishCoalesce, 3, false)),
            TokenKind::Punct(Punct::LogicalOr) => Some((BinaryOp::LogicalOr, 4, false)),
            TokenKind::Punct(Punct::LogicalAnd) => Some((BinaryOp::LogicalAnd, 5, false)),

            TokenKind::Punct(Punct::BitOr) => Some((BinaryOp::BitOr, 6, false)),
            TokenKind::Punct(Punct::BitXor) => Some((BinaryOp::BitXor, 7, false)),
            TokenKind::Punct(Punct::BitAnd) => Some((BinaryOp::BitAnd, 8, false)),

            TokenKind::Punct(Punct::Eq) => Some((BinaryOp::Eq, 9, false)),
            TokenKind::Punct(Punct::Ne) => Some((BinaryOp::Ne, 9, false)),
            TokenKind::Punct(Punct::StrictEq) => Some((BinaryOp::StrictEq, 9, false)),
            TokenKind::Punct(Punct::StrictNe) => Some((BinaryOp::StrictNe, 9, false)),

            TokenKind::Punct(Punct::Lt) => Some((BinaryOp::Lt, 10, false)),
            TokenKind::Punct(Punct::Gt) => Some((BinaryOp::Gt, 10, false)),
            TokenKind::Punct(Punct::Le) => Some((BinaryOp::Le, 10, false)),
            TokenKind::Punct(Punct::Ge) => Some((BinaryOp::Ge, 10, false)),

            TokenKind::Ident(s) if s == "instanceof" => Some((BinaryOp::Instanceof, 10, false)),

            TokenKind::Ident(s) if s == "in" && !self.in_disallowed => {
                Some((BinaryOp::In, 10, false))
            }

            TokenKind::Punct(Punct::Shl) => Some((BinaryOp::Shl, 11, false)),
            TokenKind::Punct(Punct::Shr) => Some((BinaryOp::Shr, 11, false)),
            TokenKind::Punct(Punct::UShr) => Some((BinaryOp::UShr, 11, false)),

            TokenKind::Punct(Punct::Plus) => Some((BinaryOp::Add, 12, false)),
            TokenKind::Punct(Punct::Minus) => Some((BinaryOp::Sub, 12, false)),

            TokenKind::Punct(Punct::Star) => Some((BinaryOp::Mul, 13, false)),
            TokenKind::Punct(Punct::Slash) => Some((BinaryOp::Div, 13, false)),
            TokenKind::Punct(Punct::Percent) => Some((BinaryOp::Mod, 13, false)),

            TokenKind::Punct(Punct::StarStar) => Some((BinaryOp::Pow, 14, true)),
            _ => None,
        }
    }

    fn parse_yield_expression(&mut self) -> Result<Expr, ParseError> {
        let start = self.lookahead_span().start;

        if self.in_function_params {
            return Err(ParseError {
                span: self.lookahead_span(),
                message: "YieldExpression is not allowed in formal parameters".into(),
            });
        }
        self.bump()?;
        let yield_had_line_terminator = self.lookahead_preceded_by_lt();
        if yield_had_line_terminator && matches!(self.current_kind(), TokenKind::Punct(Punct::Star))
        {
            return Err(ParseError {
                span: self.lookahead_span(),
                message: "line terminator is not allowed between `yield` and `*`".into(),
            });
        }
        let delegate = matches!(self.current_kind(), TokenKind::Punct(Punct::Star));
        if delegate {
            self.bump()?;
        }

        let arg = match self.current_kind() {
            _ if yield_had_line_terminator => Expr::Identifier {
                name: "undefined".into(),
                span: Span::new(start, start),
            },
            TokenKind::Punct(Punct::Semicolon)
            | TokenKind::Punct(Punct::RParen)
            | TokenKind::Punct(Punct::RBrace)
            | TokenKind::Punct(Punct::RBracket)
            | TokenKind::Punct(Punct::Colon)
            | TokenKind::Punct(Punct::Comma)
            | TokenKind::Eof => Expr::Identifier {
                name: "undefined".into(),
                span: Span::new(start, start),
            },
            _ => self.parse_assignment_expression()?,
        };
        let op = if delegate {
            UnaryOp::YieldDelegate
        } else {
            UnaryOp::Yield
        };
        let span = Span::new(start, arg.span().end);
        Ok(Expr::Unary {
            operator: op,
            argument: Box::new(arg),
            span,
        })
    }

    fn reject_unary_exponentiation_base(&self) -> Result<(), ParseError> {
        if matches!(self.current_kind(), TokenKind::Punct(Punct::StarStar)) {
            return Err(self.err_here(
                "unary operator is not allowed immediately before `**`; parenthesize the operand"
                    .into(),
            ));
        }
        Ok(())
    }

    fn parse_unary_expression(&mut self) -> Result<Expr, ParseError> {
        let start = self.lookahead_span().start;
        let unary_profile_phase = match self.current_kind() {
            TokenKind::Punct(Punct::Plus)
            | TokenKind::Punct(Punct::Minus)
            | TokenKind::Punct(Punct::BitNot)
            | TokenKind::Punct(Punct::LogicalNot) => {
                parse_profile::TopVarInitIdentArrowExprCondUnaryPhase::PrefixUnary
            }
            TokenKind::Ident(s) if s == "typeof" || s == "void" || s == "delete" => {
                parse_profile::TopVarInitIdentArrowExprCondUnaryPhase::PrefixUnary
            }
            TokenKind::Punct(Punct::Inc) | TokenKind::Punct(Punct::Dec) => {
                parse_profile::TopVarInitIdentArrowExprCondUnaryPhase::PrefixUpdate
            }
            TokenKind::Ident(s)
                if self.is_contextual_keyword("await")
                    && s == "await"
                    && (self.in_async
                        || (self.function_body_depth == 0
                            && self.goal_allows_top_level_await())) =>
            {
                parse_profile::TopVarInitIdentArrowExprCondUnaryPhase::AwaitExpr
            }
            TokenKind::Ident(s) if s == "yield" && (self.in_generator || self.strict_mode) => {
                parse_profile::TopVarInitIdentArrowExprCondUnaryPhase::Reserved
            }
            TokenKind::Ident(s) if s == "await" && self.is_module_goal() => {
                parse_profile::TopVarInitIdentArrowExprCondUnaryPhase::Reserved
            }
            _ => parse_profile::TopVarInitIdentArrowExprCondUnaryPhase::Postfix,
        };
        let unary_profile_start =
            parse_profile::top_var_init_ident_arrow_expr_cond_unary_phase_start();
        let parsed = match self.current_kind() {
            TokenKind::Punct(Punct::Plus) => {
                self.bump()?;
                let arg = self.parse_unary_expression()?;
                self.reject_bare_private_unary_operand(&arg)?;
                self.reject_unary_exponentiation_base()?;
                let span = Span::new(start, arg.span().end);
                Ok(Expr::Unary {
                    operator: UnaryOp::Plus,
                    argument: Box::new(arg),
                    span,
                })
            }
            TokenKind::Punct(Punct::Minus) => {
                self.bump()?;
                let arg = self.parse_unary_expression()?;
                self.reject_bare_private_unary_operand(&arg)?;
                self.reject_unary_exponentiation_base()?;
                let span = Span::new(start, arg.span().end);
                Ok(Expr::Unary {
                    operator: UnaryOp::Minus,
                    argument: Box::new(arg),
                    span,
                })
            }
            TokenKind::Punct(Punct::BitNot) => {
                self.bump()?;
                let arg = self.parse_unary_expression()?;
                self.reject_bare_private_unary_operand(&arg)?;
                self.reject_unary_exponentiation_base()?;
                let span = Span::new(start, arg.span().end);
                Ok(Expr::Unary {
                    operator: UnaryOp::BitNot,
                    argument: Box::new(arg),
                    span,
                })
            }
            TokenKind::Punct(Punct::LogicalNot) => {
                self.bump()?;
                let arg = self.parse_unary_expression()?;
                self.reject_bare_private_unary_operand(&arg)?;
                self.reject_unary_exponentiation_base()?;
                let span = Span::new(start, arg.span().end);
                Ok(Expr::Unary {
                    operator: UnaryOp::LogicalNot,
                    argument: Box::new(arg),
                    span,
                })
            }
            TokenKind::Ident(s) if s == "typeof" => {
                self.bump()?;
                let arg = self.parse_unary_expression()?;
                self.reject_bare_private_unary_operand(&arg)?;
                self.reject_unary_exponentiation_base()?;
                let span = Span::new(start, arg.span().end);
                Ok(Expr::Unary {
                    operator: UnaryOp::Typeof,
                    argument: Box::new(arg),
                    span,
                })
            }
            TokenKind::Ident(s) if s == "void" => {
                self.bump()?;
                let arg = self.parse_unary_expression()?;
                self.reject_bare_private_unary_operand(&arg)?;
                self.reject_unary_exponentiation_base()?;
                let span = Span::new(start, arg.span().end);
                Ok(Expr::Unary {
                    operator: UnaryOp::Void,
                    argument: Box::new(arg),
                    span,
                })
            }
            TokenKind::Ident(s) if s == "delete" => {
                self.bump()?;
                let arg = self.parse_unary_expression()?;
                self.reject_bare_private_unary_operand(&arg)?;
                self.reject_unary_exponentiation_base()?;
                let span = Span::new(start, arg.span().end);
                Ok(Expr::Unary {
                    operator: UnaryOp::Delete,
                    argument: Box::new(arg),
                    span,
                })
            }
            TokenKind::Ident(s) if s == "yield" && (self.in_generator || self.strict_mode) => {

                let message = if self.strict_mode && !self.in_generator {
                    "'yield' is a reserved word in strict mode and may not appear outside a generator function"
                } else {
                    "'yield' may not be used as an identifier reference in a generator body"
                };
                Err(ParseError {
                    span: self.lookahead_span(),
                    message: message.into(),
                })
            }

            TokenKind::Ident(s)
                if s == "await"
                    && self.is_contextual_keyword("await")
                    && (self.in_async
                        || (self.function_body_depth == 0
                            && self.goal_allows_top_level_await())) =>
            {
                if self.in_function_params {
                    return Err(ParseError {
                        span: self.lookahead_span(),
                        message: "AwaitExpression is not allowed in formal parameters".into(),
                    });
                }
                self.bump()?;
                let arg = self.parse_unary_expression()?;
                self.reject_bare_private_unary_operand(&arg)?;
                self.reject_unary_exponentiation_base()?;
                let span = Span::new(start, arg.span().end);
                Ok(Expr::Unary {
                    operator: UnaryOp::Await,
                    argument: Box::new(arg),
                    span,
                })
            }
            TokenKind::Ident(s) if s == "await" && self.is_module_goal() => Err(ParseError {
                span: self.lookahead_span(),
                message: "`await` is reserved in module code".into(),
            }),
            TokenKind::Punct(Punct::Inc) => {
                self.bump()?;
                let arg = self.parse_unary_expression()?;
                self.reject_bare_private_unary_operand(&arg)?;
                if Self::expr_is_optional_chain(&arg) {
                    return Err(ParseError {
                        span: arg.span(),
                        message: "optional chain is not a valid target of an update expression"
                            .into(),
                    });
                }
                self.reject_strict_update_target(&arg)?;
                let span = Span::new(start, arg.span().end);
                Ok(Expr::Update {
                    operator: UpdateOp::Inc,
                    argument: Box::new(arg),
                    prefix: true,
                    span,
                })
            }
            TokenKind::Punct(Punct::Dec) => {
                self.bump()?;
                let arg = self.parse_unary_expression()?;
                self.reject_bare_private_unary_operand(&arg)?;
                if Self::expr_is_optional_chain(&arg) {
                    return Err(ParseError {
                        span: arg.span(),
                        message: "optional chain is not a valid target of an update expression"
                            .into(),
                    });
                }
                self.reject_strict_update_target(&arg)?;
                let span = Span::new(start, arg.span().end);
                Ok(Expr::Update {
                    operator: UpdateOp::Dec,
                    argument: Box::new(arg),
                    prefix: true,
                    span,
                })
            }
            _ => self.parse_postfix_expression(),
        };
        parse_profile::record_top_var_init_ident_arrow_expr_cond_unary_phase(
            unary_profile_phase,
            unary_profile_start,
        );
        parsed
    }

    fn reject_bare_private_unary_operand(&self, arg: &Expr) -> Result<(), ParseError> {
        if self.is_lowered_private_name(arg) {
            return Err(ParseError {
                span: arg.span(),
                message: "a private name is only valid as the left operand of `in`".into(),
            });
        }
        Ok(())
    }

    fn expr_is_optional_chain(e: &Expr) -> bool {
        match e {
            Expr::Member {
                object, optional, ..
            } => *optional || Self::expr_is_optional_chain(object),
            Expr::Call {
                callee, optional, ..
            } => *optional || Self::expr_is_optional_chain(callee),
            Expr::Parenthesized { expr, .. } => Self::expr_is_optional_chain(expr),
            _ => false,
        }
    }

    pub(crate) fn is_tagged_template_call(&self, e: &Expr) -> bool {
        match e {
            Expr::Call { span, .. } => self.tagged_template_call_ends.contains(&span.end),
            Expr::Parenthesized { expr, .. } => self.is_tagged_template_call(expr),
            _ => false,
        }
    }

    pub(crate) fn is_call_assignment_target(e: &Expr) -> bool {
        match e {
            Expr::Call {
                callee, optional, ..
            } => {

                !*optional
                    && !Self::expr_is_optional_chain(e)
                    && !matches!(&**callee, Expr::Identifier { name, .. } if name == "__dynamic_import")
            }
            Expr::Parenthesized { expr, .. } => Self::is_call_assignment_target(expr),
            _ => false,
        }
    }

    fn is_valid_assignment_target(e: &Expr, op: AssignOp) -> bool {
        match e {
            Expr::Identifier { .. } => true,
            Expr::Member { optional, .. } => !optional && !Self::expr_is_optional_chain(e),
            Expr::Parenthesized { expr, .. } => match &**expr {
                Expr::Array { .. } | Expr::Object { .. } => false,
                other => Self::is_valid_assignment_target(other, op),
            },
            Expr::Array { .. } | Expr::Object { .. } => matches!(op, AssignOp::Assign),
            _ => false,
        }
    }

    fn invalid_destructuring_assignment_pattern_span(e: &Expr) -> Option<Span> {
        match e {
            Expr::Identifier { .. } => None,
            Expr::Member {
                object, optional, ..
            } => {
                if *optional || Self::expr_is_optional_chain(e) {
                    return Some(e.span());
                }
                Self::cover_initialized_name_span(object)
            }
            Expr::Parenthesized { expr, .. } => match &**expr {
                Expr::Array { .. } | Expr::Object { .. } | Expr::Assign { .. } => Some(e.span()),
                other => Self::invalid_destructuring_assignment_pattern_span(other),
            },
            Expr::Assign {
                operator, target, ..
            } if matches!(operator, AssignOp::Assign) => {
                Self::invalid_destructuring_assignment_pattern_span(target)
            }
            Expr::Array {
                elements,
                trailing_comma_after_spread,
                ..
            } => {
                if *trailing_comma_after_spread {
                    return Some(e.span());
                }
                let n = elements.len();
                for (i, el) in elements.iter().enumerate() {
                    match el {
                        ArrayElement::Elision { .. } => {}
                        ArrayElement::Expr(expr) => {
                            if let Some(span) =
                                Self::invalid_destructuring_assignment_pattern_span(expr)
                            {
                                return Some(span);
                            }
                        }
                        ArrayElement::Spread { expr, span } => {
                            if i + 1 != n {
                                return Some(*span);
                            }
                            if let Some(span) =
                                Self::invalid_destructuring_assignment_pattern_span(expr)
                            {
                                return Some(span);
                            }
                        }
                    }
                }
                None
            }
            Expr::Object {
                properties,
                trailing_comma_after_spread,
                ..
            } => {

                if *trailing_comma_after_spread {
                    return Some(e.span());
                }
                let n = properties.len();
                for (i, prop) in properties.iter().enumerate() {
                    match prop {
                        ObjectProperty::Property { value, .. } => {
                            if let Some(span) =
                                Self::invalid_destructuring_assignment_pattern_span(value)
                            {
                                return Some(span);
                            }
                        }
                        ObjectProperty::Spread { expr, span } => {
                            if i + 1 != n {
                                return Some(*span);
                            }
                            if let Some(span) =
                                Self::invalid_destructuring_assignment_pattern_span(expr)
                            {
                                return Some(span);
                            }
                        }
                    }
                }
                None
            }
            _ => Some(e.span()),
        }
    }

    fn strict_eval_arguments_target(e: &Expr) -> Option<(String, Span)> {
        match e {
            Expr::Identifier { name, span } if name == "eval" || name == "arguments" => {
                Some((name.clone(), *span))
            }
            Expr::Parenthesized { expr, .. } => Self::strict_eval_arguments_target(expr),
            Expr::Array { elements, .. } => elements.iter().find_map(|element| match element {
                ArrayElement::Expr(expr) | ArrayElement::Spread { expr, .. } => {
                    Self::strict_eval_arguments_target(expr)
                }
                ArrayElement::Elision { .. } => None,
            }),
            Expr::Object { properties, .. } => properties.iter().find_map(|prop| match prop {
                ObjectProperty::Property { value, .. } => Self::strict_eval_arguments_target(value),
                ObjectProperty::Spread { expr, .. } => Self::strict_eval_arguments_target(expr),
            }),
            Expr::Assign { target, .. } => Self::strict_eval_arguments_target(target),
            _ => None,
        }
    }

    fn mixes_nullish_with_logical(op: BinaryOp, left: &Expr, right: &Expr) -> bool {
        match op {
            BinaryOp::NullishCoalesce => {
                Self::contains_unparenthesized_logical(left)
                    || Self::contains_unparenthesized_logical(right)
            }
            BinaryOp::LogicalAnd | BinaryOp::LogicalOr => {
                Self::contains_unparenthesized_nullish(left)
                    || Self::contains_unparenthesized_nullish(right)
            }
            _ => false,
        }
    }

    fn contains_unparenthesized_logical(e: &Expr) -> bool {
        match e {
            Expr::Parenthesized { .. } => false,
            Expr::Binary {
                operator,
                left,
                right,
                ..
            } => {
                matches!(operator, BinaryOp::LogicalAnd | BinaryOp::LogicalOr)
                    || Self::contains_unparenthesized_logical(left)
                    || Self::contains_unparenthesized_logical(right)
            }
            _ => false,
        }
    }

    fn contains_unparenthesized_nullish(e: &Expr) -> bool {
        match e {
            Expr::Parenthesized { .. } => false,
            Expr::Binary {
                operator,
                left,
                right,
                ..
            } => {
                matches!(operator, BinaryOp::NullishCoalesce)
                    || Self::contains_unparenthesized_nullish(left)
                    || Self::contains_unparenthesized_nullish(right)
            }
            _ => false,
        }
    }

    fn reject_strict_update_target(&self, e: &Expr) -> Result<(), ParseError> {
        let Expr::Identifier { name, span } = (match e {
            Expr::Parenthesized { expr, .. } => return self.reject_strict_update_target(expr),
            other => other,
        }) else {
            return Ok(());
        };
        if self.strict_mode && (name == "eval" || name == "arguments") {
            return Err(ParseError {
                span: *span,
                message: format!(
                    "Identifier '{}' is not a valid update target in strict mode",
                    name
                ),
            });
        }
        Ok(())
    }

    fn parse_postfix_expression(&mut self) -> Result<Expr, ParseError> {
        let lhs_start = parse_profile::top_var_init_ident_arrow_expr_cond_postfix_phase_start();
        let expr = self.parse_left_hand_side_expression()?;
        parse_profile::record_top_var_init_ident_arrow_expr_cond_postfix_phase(
            parse_profile::TopVarInitIdentArrowExprCondPostfixPhase::Lhs,
            lhs_start,
        );

        let update_check_start =
            parse_profile::top_var_init_ident_arrow_expr_cond_postfix_phase_start();
        if !self.lookahead_preceded_by_lt() {
            let start = expr.span().start;
            if matches!(
                self.current_kind(),
                TokenKind::Punct(Punct::Inc) | TokenKind::Punct(Punct::Dec)
            ) && Self::expr_is_optional_chain(&expr)
            {
                return Err(ParseError {
                    span: expr.span(),
                    message: "optional chain is not a valid target of an update expression".into(),
                });
            }
            if matches!(self.current_kind(), TokenKind::Punct(Punct::Inc)) {
                parse_profile::record_top_var_init_ident_arrow_expr_cond_postfix_phase(
                    parse_profile::TopVarInitIdentArrowExprCondPostfixPhase::UpdateCheck,
                    update_check_start,
                );
                let update_emit_start =
                    parse_profile::top_var_init_ident_arrow_expr_cond_postfix_phase_start();
                let end = self.lookahead_span().end;
                self.bump()?;
                self.reject_strict_update_target(&expr)?;
                let parsed = Expr::Update {
                    operator: UpdateOp::Inc,
                    argument: Box::new(expr),
                    prefix: false,
                    span: Span::new(start, end),
                };
                parse_profile::record_top_var_init_ident_arrow_expr_cond_postfix_phase(
                    parse_profile::TopVarInitIdentArrowExprCondPostfixPhase::UpdateEmit,
                    update_emit_start,
                );
                return Ok(parsed);
            }
            if matches!(self.current_kind(), TokenKind::Punct(Punct::Dec)) {
                parse_profile::record_top_var_init_ident_arrow_expr_cond_postfix_phase(
                    parse_profile::TopVarInitIdentArrowExprCondPostfixPhase::UpdateCheck,
                    update_check_start,
                );
                let update_emit_start =
                    parse_profile::top_var_init_ident_arrow_expr_cond_postfix_phase_start();
                let end = self.lookahead_span().end;
                self.bump()?;
                self.reject_strict_update_target(&expr)?;
                let parsed = Expr::Update {
                    operator: UpdateOp::Dec,
                    argument: Box::new(expr),
                    prefix: false,
                    span: Span::new(start, end),
                };
                parse_profile::record_top_var_init_ident_arrow_expr_cond_postfix_phase(
                    parse_profile::TopVarInitIdentArrowExprCondPostfixPhase::UpdateEmit,
                    update_emit_start,
                );
                return Ok(parsed);
            }
        }
        parse_profile::record_top_var_init_ident_arrow_expr_cond_postfix_phase(
            parse_profile::TopVarInitIdentArrowExprCondPostfixPhase::UpdateCheck,
            update_check_start,
        );
        Ok(expr)
    }

    pub(crate) fn parse_left_hand_side_expression(&mut self) -> Result<Expr, ParseError> {
        let _profile = parse_profile::Guard::new(parse_profile::Kind::ExprLhs);
        let _paren_profile = parse_profile::paren_guard(parse_profile::Kind::ParenLhs);
        let expr = {
            let _profile = parse_profile::Guard::new(parse_profile::Kind::LhsBase);
            if self.is_ident("new") {
                let start = parse_profile::top_var_init_ident_arrow_expr_cond_lhs_phase_start();
                let parsed = self.parse_new_expression()?;
                parse_profile::record_top_var_init_ident_arrow_expr_cond_lhs_phase(
                    parse_profile::TopVarInitIdentArrowExprCondLhsPhase::NewBase,
                    start,
                );
                parsed
            } else {
                let start = parse_profile::top_var_init_ident_arrow_expr_cond_lhs_phase_start();
                let parsed = self.parse_primary_expression()?;
                parse_profile::record_top_var_init_ident_arrow_expr_cond_lhs_phase(
                    parse_profile::TopVarInitIdentArrowExprCondLhsPhase::PrimaryBase,
                    start,
                );
                parsed
            }
        };
        let cont_start = parse_profile::top_var_init_ident_arrow_expr_cond_lhs_phase_start();
        let parsed = self.continue_lhs_continuation(expr);
        parse_profile::record_top_var_init_ident_arrow_expr_cond_lhs_phase(
            parse_profile::TopVarInitIdentArrowExprCondLhsPhase::Continuation,
            cont_start,
        );
        parsed
    }

    pub(crate) fn continue_lhs_continuation(&mut self, mut expr: Expr) -> Result<Expr, ParseError> {
        let _profile = parse_profile::Guard::new(parse_profile::Kind::LhsCont);
        loop {
            match self.current_kind() {
                TokenKind::Punct(Punct::Dot) => {
                    let _profile = parse_profile::Guard::new(parse_profile::Kind::LhsMember);
                    self.bump()?;
                    expr = self.consume_member_property(expr, false)?;
                }
                TokenKind::Punct(Punct::OptionalChain) => {
                    self.bump()?;
                    if matches!(expr, Expr::Super { .. }) {
                        return Err(
                            self.err_here("optional chaining cannot be applied to `super`".into())
                        );
                    }
                    if matches!(expr, Expr::New { ref arguments, .. } if arguments.is_empty()) {
                        return Err(self.err_here(
                            "optional chaining cannot follow unparenthesized `new`".into(),
                        ));
                    }

                    if matches!(self.current_kind(), TokenKind::Punct(Punct::LParen)) {
                        let _profile = parse_profile::Guard::new(parse_profile::Kind::LhsCall);
                        let start = expr.span().start;
                        let arguments = self.parse_arguments()?;
                        let end = self.last_span_end();
                        expr = Expr::Call {
                            callee: Box::new(expr),
                            arguments,
                            optional: true,
                            span: Span::new(start, end),
                        };
                    } else if matches!(self.current_kind(), TokenKind::Punct(Punct::LBracket)) {
                        let _profile = parse_profile::Guard::new(parse_profile::Kind::LhsComputed);
                        expr = self.consume_computed_member(expr, true)?;
                    } else {
                        let _profile = parse_profile::Guard::new(parse_profile::Kind::LhsMember);
                        expr = self.consume_member_property(expr, true)?;
                    }
                }
                TokenKind::Punct(Punct::LBracket) => {
                    let _profile = parse_profile::Guard::new(parse_profile::Kind::LhsComputed);
                    expr = self.consume_computed_member(expr, false)?;
                }
                TokenKind::Punct(Punct::LParen) => {
                    let _profile = parse_profile::Guard::new(parse_profile::Kind::LhsCall);
                    let start = expr.span().start;
                    let arguments = self.parse_arguments()?;
                    let end = self.last_span_end();
                    expr = Expr::Call {
                        callee: Box::new(expr),
                        arguments,
                        optional: false,
                        span: Span::new(start, end),
                    };
                }
                TokenKind::Template { .. } => {
                    let _profile = parse_profile::Guard::new(parse_profile::Kind::LhsTemplate);

                    if Self::expr_is_optional_chain(&expr) {
                        return Err(
                            self.err_here("tagged template cannot follow an optional chain".into())
                        );
                    }
                    let start = expr.span().start;
                    expr = self.parse_tagged_template(expr, start)?;

                    self.tagged_template_call_ends.insert(expr.span().end);
                }
                _ => break,
            }
        }
        Ok(expr)
    }

    fn parse_new_expression(&mut self) -> Result<Expr, ParseError> {
        let new_span = self.lookahead_span();
        let start = new_span.start;
        self.expect_keyword("new")?;

        if self.ident_source_has_escape(new_span) {
            return Err(ParseError {
                span: new_span,
                message: "the reserved word `new` must not contain a unicode escape".into(),
            });
        }

        if matches!(self.current_kind(), TokenKind::Punct(Punct::Dot)) {
            self.bump()?;
            if let TokenKind::Ident(p) = self.current_kind() {
                if p == "target" {
                    let target_span = self.lookahead_span();

                    if self.ident_source_has_escape(target_span) {
                        return Err(ParseError {
                            span: target_span,
                            message: "`target` in new.target must not contain a unicode escape"
                                .into(),
                        });
                    }
                    let end = self.lookahead_span().end;
                    let p_clone = p.to_string();
                    self.bump()?;
                    return Ok(Expr::MetaProperty {
                        meta: "new".into(),
                        property: p_clone,
                        span: Span::new(start, end),
                    });
                }
            }
            return Err(self.err_here("expected `target` after `new.`".into()));
        }

        if self.is_ident("import") && !self.next_after_current_is_dot_meta() {

            return Err(self.err_here("ImportCall cannot be used as a NewExpression callee".into()));
        }
        if self.is_module_goal() && self.is_ident("await") {
            return Err(self.err_here("`await` cannot be used as a NewExpression callee".into()));
        }
        let mut callee = if self.is_ident("new") {
            self.parse_new_expression()?
        } else {
            self.parse_primary_expression()?
        };

        loop {
            match self.current_kind() {
                TokenKind::Punct(Punct::Dot) => {
                    self.bump()?;
                    callee = self.consume_member_property(callee, false)?;
                }
                TokenKind::Punct(Punct::LBracket) => {
                    callee = self.consume_computed_member(callee, false)?;
                }
                TokenKind::Template { .. } => {

                    if Self::expr_is_optional_chain(&callee) {
                        return Err(
                            self.err_here("tagged template cannot follow an optional chain".into())
                        );
                    }
                    let tag_start = callee.span().start;
                    callee = self.parse_tagged_template(callee, tag_start)?;
                }
                _ => break,
            }
        }

        let arguments = if matches!(self.current_kind(), TokenKind::Punct(Punct::LParen)) {
            self.parse_arguments()?
        } else {
            vec![]
        };
        let end = self.last_span_end();
        Ok(Expr::New {
            callee: Box::new(callee),
            arguments,
            span: Span::new(start, end),
        })
    }

    fn consume_member_property(
        &mut self,
        object: Expr,
        optional: bool,
    ) -> Result<Expr, ParseError> {
        let start = object.span().start;
        let prop = match self.current_kind().clone() {
            TokenKind::Ident(name) => {
                let span = self.lookahead_span();
                self.bump()?;
                MemberProperty::Identifier { name, span }
            }
            TokenKind::PrivateIdent(name) => {
                let span = self.lookahead_span();
                self.bump()?;

                if matches!(object, Expr::Super { .. }) {
                    return Err(self.err_at(
                        span,
                        "private members cannot be accessed through `super`".into(),
                    ));
                }
                MemberProperty::Private { name, span }
            }
            _ => return Err(self.err_here("expected property name".into())),
        };
        let end = match &prop {
            MemberProperty::Identifier { span, .. } => span.end,
            MemberProperty::Private { span, .. } => span.end,
            MemberProperty::Computed { span, .. } => span.end,
        };
        Ok(Expr::Member {
            object: Box::new(object),
            property: Box::new(prop),
            optional,
            span: Span::new(start, end),
        })
    }

    fn consume_computed_member(
        &mut self,
        object: Expr,
        optional: bool,
    ) -> Result<Expr, ParseError> {
        let start = object.span().start;
        self.expect_punct(Punct::LBracket)?;
        let computed = self.parse_expression()?;
        let computed_span = computed.span();
        self.expect_punct(Punct::RBracket)?;
        let end = self.last_span_end();
        Ok(Expr::Member {
            object: Box::new(object),
            property: Box::new(MemberProperty::Computed {
                expr: computed,
                span: computed_span,
            }),
            optional,
            span: Span::new(start, end),
        })
    }

    fn parse_arguments(&mut self) -> Result<Vec<Argument>, ParseError> {

        let saved_in_disallowed = std::mem::take(&mut self.in_disallowed);
        let result = self.parse_arguments_in_allowed();
        self.in_disallowed = saved_in_disallowed;
        result
    }

    fn parse_arguments_in_allowed(&mut self) -> Result<Vec<Argument>, ParseError> {
        let _profile = parse_profile::Guard::new(parse_profile::Kind::CallArgs);
        self.expect_punct(Punct::LParen)?;
        let mut out = Vec::new();
        while !matches!(self.current_kind(), TokenKind::Punct(Punct::RParen)) {
            if matches!(self.current_kind(), TokenKind::Punct(Punct::Spread)) {
                let start = self.lookahead_span().start;
                self.bump()?;
                let expr = self.parse_assignment_expression()?;
                let end = expr.span().end;
                out.push(Argument::Spread {
                    expr,
                    span: Span::new(start, end),
                });
            } else {
                out.push(Argument::Expr(self.parse_assignment_expression()?));
            }
            if matches!(self.current_kind(), TokenKind::Punct(Punct::Comma)) {
                self.bump()?;
            } else {
                break;
            }
        }
        self.expect_punct(Punct::RParen)?;
        Ok(out)
    }

    fn parse_primary_expression(&mut self) -> Result<Expr, ParseError> {
        let _profile = parse_profile::Guard::new(parse_profile::Kind::ExprPrimary);
        let _paren_profile = parse_profile::paren_guard(parse_profile::Kind::ParenPrimary);
        let primary_kind = match self.current_kind() {
            TokenKind::Ident(name) if name == "function" || name == "class" => {
                parse_profile::Kind::PrimaryFnClass
            }
            TokenKind::Ident(_) | TokenKind::PrivateIdent(_) => parse_profile::Kind::PrimaryIdent,
            TokenKind::Number(..)
            | TokenKind::BigInt(..)
            | TokenKind::String(_)
            | TokenKind::WtfString(_)
            | TokenKind::Regex { .. } => parse_profile::Kind::PrimaryLiteral,
            TokenKind::Template { .. } | TokenKind::Punct(Punct::LParen) => {
                parse_profile::Kind::PrimaryParenTemplate
            }
            TokenKind::Punct(Punct::LBracket) | TokenKind::Punct(Punct::LBrace) => {
                parse_profile::Kind::PrimaryObjectArray
            }
            _ => parse_profile::Kind::PrimaryOther,
        };
        let primary_phase = match primary_kind {
            parse_profile::Kind::PrimaryFnClass => {
                parse_profile::TopVarInitIdentArrowExprCondPrimaryPhase::FnClass
            }
            parse_profile::Kind::PrimaryParenTemplate => {
                parse_profile::TopVarInitIdentArrowExprCondPrimaryPhase::ParenTemplate
            }
            parse_profile::Kind::PrimaryObjectArray => {
                parse_profile::TopVarInitIdentArrowExprCondPrimaryPhase::ObjectArray
            }
            parse_profile::Kind::PrimaryIdent => {
                parse_profile::TopVarInitIdentArrowExprCondPrimaryPhase::Ident
            }
            parse_profile::Kind::PrimaryLiteral => {
                parse_profile::TopVarInitIdentArrowExprCondPrimaryPhase::Literal
            }
            _ => parse_profile::TopVarInitIdentArrowExprCondPrimaryPhase::Other,
        };
        let _profile = parse_profile::Guard::new(primary_kind);
        let _scoped_primary_profile =
            parse_profile::top_var_init_ident_arrow_expr_cond_primary_guard(primary_phase);
        let span = self.lookahead_span();
        match self.current_kind().clone() {
            TokenKind::Ident(name) => {

                if matches!(name.as_str(), "null" | "true" | "false" | "this" | "super")
                    && self.ident_source_has_escape(span)
                {
                    return Err(ParseError {
                        span,
                        message: format!(
                            "the reserved word `{name}` must not contain a unicode escape"
                        ),
                    });
                }
                match name.as_str() {
                    "null" => {
                        self.bump()?;
                        Ok(Expr::NullLiteral { span })
                    }
                    "true" => {
                        self.bump()?;
                        Ok(Expr::BoolLiteral { value: true, span })
                    }
                    "false" => {
                        self.bump()?;
                        Ok(Expr::BoolLiteral { value: false, span })
                    }
                    "this" => {
                        self.bump()?;
                        Ok(Expr::This { span })
                    }
                    "super" => {
                        self.bump()?;
                        Ok(Expr::Super { span })
                    }
                    "import" => {
                        if &self.source()[span.start..span.end] != "import" {
                            return Err(self.err_here(
                                "`import` terminal cannot contain escape sequences".into(),
                            ));
                        }

                        let look_end = span.end;
                        let bytes = self.source().as_bytes();
                        let p = Self::skip_ws_and_comments_allow_lt(bytes, look_end);
                        if bytes.get(p) == Some(&b'.') {
                            self.bump()?;
                            self.bump()?;
                            if let TokenKind::Ident(p) = self.current_kind() {
                                if p == "meta" {

                                    let meta_span = self.lookahead_span();
                                    if &self.source()[meta_span.start..meta_span.end] != "meta" {
                                        return Err(self.err_here(
                                            "`meta` terminal cannot contain escape sequences"
                                                .into(),
                                        ));
                                    }

                                    if !matches!(self.parse_goal, crate::parser::ParseGoal::Module)
                                    {
                                        return Err(self.err_here(
                                            "import.meta is only valid in module code".into(),
                                        ));
                                    }
                                    let prop = p.clone();
                                    let end = self.lookahead_span().end;
                                    self.bump()?;
                                    return Ok(Expr::MetaProperty {
                                        meta: "import".into(),
                                        property: prop,
                                        span: Span::new(span.start, end),
                                    });
                                }
                                if p == "source" {
                                    self.bump()?;
                                    if !matches!(
                                        self.current_kind(),
                                        TokenKind::Punct(Punct::LParen)
                                    ) {
                                        return Err(self.err_here(
                                            "expected '(' after `import.source`".into(),
                                        ));
                                    }
                                    self.bump()?;
                                    let arg = self.parse_assignment_expression()?;
                                    let arguments = vec![rusty_js_ast::Argument::Expr(arg)];
                                    if matches!(self.current_kind(), TokenKind::Punct(Punct::Comma))
                                    {
                                        self.bump()?;
                                    }
                                    if !matches!(
                                        self.current_kind(),
                                        TokenKind::Punct(Punct::RParen)
                                    ) {
                                        return Err(
                                            self.err_here("expected ')' in import.source()".into())
                                        );
                                    }
                                    let end = self.lookahead_span().end;
                                    self.bump()?;
                                    let span = Span::new(span.start, end);
                                    return Ok(Expr::Call {
                                        callee: Box::new(Expr::Identifier {
                                            name: "__source_import".into(),
                                            span,
                                        }),
                                        arguments,
                                        optional: false,
                                        span,
                                    });
                                }
                                if p == "defer" {
                                    self.bump()?;
                                    if !matches!(
                                        self.current_kind(),
                                        TokenKind::Punct(Punct::LParen)
                                    ) {
                                        return Err(self
                                            .err_here("expected '(' after `import.defer`".into()));
                                    }
                                    self.bump()?;
                                    let arg = self.parse_assignment_expression()?;
                                    let arguments = vec![
                                        rusty_js_ast::Argument::Expr(arg),
                                        rusty_js_ast::Argument::Expr(Expr::StringLiteral {
                                            value: "__cruft_import_defer".into(),
                                            span,
                                        }),
                                    ];
                                    if matches!(self.current_kind(), TokenKind::Punct(Punct::Comma))
                                    {
                                        self.bump()?;
                                    }
                                    if !matches!(
                                        self.current_kind(),
                                        TokenKind::Punct(Punct::RParen)
                                    ) {
                                        return Err(
                                            self.err_here("expected ')' in import.defer()".into())
                                        );
                                    }
                                    let end = self.lookahead_span().end;
                                    self.bump()?;
                                    let span = Span::new(span.start, end);
                                    return Ok(Expr::Call {
                                        callee: Box::new(Expr::Identifier {
                                            name: "__dynamic_import".into(),
                                            span,
                                        }),
                                        arguments,
                                        optional: false,
                                        span,
                                    });
                                }
                            }
                            return Err(self.err_here(
                                "expected `meta`, `source`, or `defer` after `import.`".into(),
                            ));
                        }

                        self.bump()?;
                        if !matches!(self.current_kind(), TokenKind::Punct(Punct::LParen)) {
                            return Err(self.err_here("expected '(' after `import`".into()));
                        }
                        self.bump()?;
                        let saved_in_disallowed = self.in_disallowed;
                        self.in_disallowed = false;
                        let arg = self.parse_assignment_expression();
                        self.in_disallowed = saved_in_disallowed;
                        let arg = arg?;
                        let mut arguments = vec![rusty_js_ast::Argument::Expr(arg)];
                        let mut has_attrs = false;

                        if matches!(self.current_kind(), TokenKind::Punct(Punct::Comma)) {
                            self.bump()?;
                            if !matches!(self.current_kind(), TokenKind::Punct(Punct::RParen)) {
                                let saved_in_disallowed = self.in_disallowed;
                                self.in_disallowed = false;
                                let attrs = self.parse_assignment_expression();
                                self.in_disallowed = saved_in_disallowed;
                                let attrs = attrs?;
                                arguments.push(rusty_js_ast::Argument::Expr(attrs));
                                has_attrs = true;

                                if matches!(self.current_kind(), TokenKind::Punct(Punct::Comma)) {
                                    self.bump()?;
                                }
                            }
                        }
                        if !matches!(self.current_kind(), TokenKind::Punct(Punct::RParen)) {
                            return Err(self.err_here("expected ')' in dynamic import()".into()));
                        }
                        let end = self.lookahead_span().end;
                        self.bump()?;
                        let span = Span::new(span.start, end);
                        if !has_attrs {
                            arguments.push(rusty_js_ast::Argument::Expr(Expr::Identifier {
                                name: "undefined".into(),
                                span,
                            }));
                        }
                        arguments.push(rusty_js_ast::Argument::Expr(Expr::MetaProperty {
                            meta: "import".into(),
                            property: "meta".into(),
                            span,
                        }));
                        Ok(Expr::Call {
                            callee: Box::new(Expr::Identifier {
                                name: "__dynamic_import".into(),
                                span,
                            }),
                            arguments,
                            optional: false,
                            span,
                        })
                    }

                    "function" => {
                        let _profile =
                            parse_profile::Guard::new(parse_profile::Kind::PrimaryFunction);
                        self.parse_function_expression(false, None)
                    }
                    "class" => {
                        let _profile = parse_profile::Guard::new(parse_profile::Kind::PrimaryClass);
                        self.parse_class_expression()
                    }
                    "async" => {

                        let bytes = self.source().as_bytes();
                        let async_function_after_trivia =
                            Self::skip_ws_and_comments(bytes, span.end, false).map_or(false, |p| {
                                Self::bytes_at_identifier_keyword(bytes, p, "function")
                            });
                        if async_function_after_trivia {
                            let async_start = span.start;
                            self.reject_escaped_contextual_keyword("async")?;
                            self.bump()?;
                            self.parse_function_expression(true, Some(async_start))
                        } else {
                            self.bump()?;
                            Ok(Expr::Identifier { name, span })
                        }
                    }
                    _ => {

                        if crate::parser::is_unconditional_reserved_word(&name)
                            || (self.strict_mode && crate::parser::is_strict_reserved_word(&name))
                            || (self.in_async && name == "await")
                        {
                            return Err(ParseError {
                                span,
                                message: format!(
                                    "`{}` is a reserved word and cannot be used as an identifier reference",
                                    name
                                ),
                            });
                        }
                        self.bump()?;
                        Ok(Expr::Identifier { name, span })
                    }
                }
            }
            TokenKind::PrivateIdent(_name) => {

                let name = match self.current_kind().clone() {
                    TokenKind::PrivateIdent(name) => name,
                    _ => unreachable!(),
                };
                self.bump()?;
                Ok(Expr::StringLiteral {
                    value: format!("#{}", name),
                    span,
                })
            }
            TokenKind::Punct(Punct::At) => {
                let decorators = self.parse_class_decorators()?;
                if !self.is_ident("class") {
                    return Err(
                        self.err_here("decorators are only supported on class expressions".into())
                    );
                }
                self.parse_class_expression_with_decorators(decorators)
            }
            TokenKind::Number(value, _) => {
                self.bump()?;
                Ok(Expr::NumberLiteral { value, span })
            }
            TokenKind::BigInt(digits, kind) => {
                self.bump()?;

                use crate::token::NumberKind;
                let normalized = match kind {
                    NumberKind::Hex => format!("0x{}", digits),
                    NumberKind::Octal | NumberKind::LegacyOctal => format!("0o{}", digits),
                    NumberKind::Binary => format!("0b{}", digits),
                    NumberKind::Decimal => digits,
                };
                Ok(Expr::BigIntLiteral {
                    digits: normalized,
                    span,
                })
            }
            TokenKind::String(value) => {
                self.bump()?;
                Ok(Expr::StringLiteral { value, span })
            }
            TokenKind::WtfString(units) => {
                self.bump()?;
                Ok(Expr::WtfStringLiteral { units, span })
            }
            TokenKind::Template { cooked, part, .. } => {
                use crate::token::TemplatePart;
                match part {
                    TemplatePart::NoSubstitution => {
                        let _profile =
                            parse_profile::Guard::new(parse_profile::Kind::PrimaryTemplate);
                        let value = self.untagged_template_tv(cooked)?;
                        self.bump()?;
                        Ok(Expr::StringLiteral { value, span })
                    }
                    TemplatePart::Head => {
                        let _profile =
                            parse_profile::Guard::new(parse_profile::Kind::PrimaryTemplate);
                        self.parse_template_with_substitutions(span.start)
                    }
                    TemplatePart::Middle | TemplatePart::Tail => Err(self
                        .err_here("unexpected template middle/tail in expression position".into())),
                }
            }
            TokenKind::Regex { body, flags } => {
                validate_regexp_flags(&flags, span)?;
                validate_named_group_specifiers(&body, &flags, span)?;
                validate_quantifiers(&body, &flags, span)?;
                validate_unicode_mode_escapes(&body, &flags, span)?;
                validate_unicode_property_escapes(&body, &flags, span)?;
                validate_character_class_ranges(&body, &flags, span)?;
                validate_inline_modifiers(&body, span)?;
                validate_named_group_refs(&body, &flags, span)?;
                validate_string_properties(&body, &flags, span)?;
                validate_v_mode_class_syntax(&body, &flags, span)?;
                let pattern = std::rc::Rc::new(body.clone());
                let flags = std::rc::Rc::new(flags.clone());
                self.bump()?;
                Ok(Expr::RegExp {
                    pattern,
                    flags,
                    span,
                })
            }
            TokenKind::Punct(Punct::LBracket) => self.parse_array_literal(),
            TokenKind::Punct(Punct::LBrace) => self.parse_object_literal(),
            TokenKind::Punct(Punct::LParen) => {
                let _profile = parse_profile::Guard::new(parse_profile::Kind::PrimaryParen);
                self.parse_parenthesized()
            }
            _ => Err(self.err_here(format!(
                "unexpected token in expression: {:?}",
                self.current_kind()
            ))),
        }
    }

    fn parse_array_literal(&mut self) -> Result<Expr, ParseError> {
        let start = self.lookahead_span().start;
        self.expect_punct(Punct::LBracket)?;
        let mut elements = Vec::new();

        let mut trailing_comma_after_spread = false;
        while !matches!(self.current_kind(), TokenKind::Punct(Punct::RBracket)) {
            if matches!(self.current_kind(), TokenKind::Punct(Punct::Comma)) {
                let span = self.lookahead_span();
                elements.push(ArrayElement::Elision { span });
                self.bump()?;
                trailing_comma_after_spread = false;
                continue;
            }
            let was_spread = matches!(self.current_kind(), TokenKind::Punct(Punct::Spread));
            if was_spread {
                let sp_start = self.lookahead_span().start;
                self.bump()?;
                let expr = self.parse_cover_for_head_pattern_assignment_expression()?;
                let end = expr.span().end;
                elements.push(ArrayElement::Spread {
                    expr,
                    span: Span::new(sp_start, end),
                });
            } else {
                elements.push(ArrayElement::Expr(
                    self.parse_cover_for_head_pattern_assignment_expression()?,
                ));
            }
            if matches!(self.current_kind(), TokenKind::Punct(Punct::Comma)) {
                self.bump()?;
                trailing_comma_after_spread = was_spread;
            } else {
                trailing_comma_after_spread = false;
                break;
            }
        }
        self.expect_punct(Punct::RBracket)?;
        let end = self.last_span_end();
        Ok(Expr::Array {
            elements,
            trailing_comma_after_spread,
            span: Span::new(start, end),
        })
    }

    fn parse_cover_for_head_pattern_assignment_expression(&mut self) -> Result<Expr, ParseError> {
        if !self.allow_cover_initialized_name_in_for_head {
            return self.parse_assignment_expression();
        }
        let saved_in_disallowed = self.in_disallowed;
        self.in_disallowed = false;
        let result = self.parse_assignment_expression();
        self.in_disallowed = saved_in_disallowed;
        result
    }

    fn parse_object_literal(&mut self) -> Result<Expr, ParseError> {
        let start = self.lookahead_span().start;
        self.expect_punct(Punct::LBrace)?;
        let mut properties = Vec::new();

        let mut trailing_comma_after_spread = false;
        while !matches!(self.current_kind(), TokenKind::Punct(Punct::RBrace)) {
            let iter_is_spread = matches!(self.current_kind(), TokenKind::Punct(Punct::Spread));
            let mut depth3_colon_property = false;
            if matches!(self.current_kind(), TokenKind::Punct(Punct::Spread)) {
                let prop_profile_start = crate::parser::parse_profile::depth3_object_prop_start();
                let sp_start = self.lookahead_span().start;
                self.bump()?;
                let expr = self.parse_cover_for_head_pattern_assignment_expression()?;
                let end = expr.span().end;
                properties.push(ObjectProperty::Spread {
                    expr,
                    span: Span::new(sp_start, end),
                });
                crate::parser::parse_profile::record_depth3_object_prop(
                    crate::parser::parse_profile::ObjectPropPhase::Spread,
                    prop_profile_start,
                );
            } else if matches!(self.current_kind(), TokenKind::Punct(Punct::Star)) {
                let prop_profile_start = crate::parser::parse_profile::depth3_object_prop_start();

                let prop_start = self.lookahead_span().start;
                self.bump()?;
                let key = self.parse_object_key()?;

                let params = self.parse_unique_formal_parameters_ga(true, false)?;
                let body = self.parse_function_body_gs(
                    Some(true),
                    Some(false),
                    Self::is_simple_param_list(&params),
                )?;
                let end = self.last_span_end();
                let func = Expr::Function {
                    name: None,
                    is_async: false,
                    is_generator: true,
                    is_method: true,
                    params,
                    body,
                    span: Span::new(prop_start, end),
                };
                properties.push(ObjectProperty::Property {
                    key,
                    value: func,
                    shorthand: false,
                    kind: ObjectPropertyKind::Init,
                    span: Span::new(prop_start, end),
                });
                crate::parser::parse_profile::record_depth3_object_prop(
                    crate::parser::parse_profile::ObjectPropPhase::GeneratorMethod,
                    prop_profile_start,
                );
            } else if self.looks_like_async_method_shorthand() {
                let prop_profile_start = crate::parser::parse_profile::depth3_object_prop_start();

                let prop_start = self.lookahead_span().start;
                self.reject_escaped_contextual_keyword("async")?;
                self.bump()?;
                let is_generator = if matches!(self.current_kind(), TokenKind::Punct(Punct::Star)) {
                    self.bump()?;
                    true
                } else {
                    false
                };
                let key = self.parse_object_key()?;

                let params = self.parse_unique_formal_parameters_ga(is_generator, true)?;
                let body = self.parse_function_body_gs(
                    Some(is_generator),
                    Some(true),
                    Self::is_simple_param_list(&params),
                )?;
                let end = self.last_span_end();
                let func = Expr::Function {
                    name: None,
                    is_async: true,
                    is_generator,
                    is_method: true,
                    params,
                    body,
                    span: Span::new(prop_start, end),
                };
                properties.push(ObjectProperty::Property {
                    key,
                    value: func,
                    shorthand: false,
                    kind: ObjectPropertyKind::Init,
                    span: Span::new(prop_start, end),
                });
                crate::parser::parse_profile::record_depth3_object_prop(
                    crate::parser::parse_profile::ObjectPropPhase::AsyncMethod,
                    prop_profile_start,
                );
            } else if self.looks_like_accessor_shorthand() {
                let prop_profile_start = crate::parser::parse_profile::depth3_object_prop_start();

                let prop_start = self.lookahead_span().start;
                let (kind, kw) = match self.current_kind() {
                    TokenKind::Ident(n) if n == "get" => (ObjectPropertyKind::Get, "get"),
                    TokenKind::Ident(n) if n == "set" => (ObjectPropertyKind::Set, "set"),
                    _ => unreachable!(
                        "looks_like_accessor_shorthand returned true without get/set token"
                    ),
                };
                self.reject_escaped_contextual_keyword(kw)?;
                self.bump()?;
                let key = self.parse_object_key()?;
                let params = self.parse_function_parameters()?;
                let body = self.parse_function_body_gs(
                    Some(false),
                    Some(false),
                    Self::is_simple_param_list(&params),
                )?;
                let method_kind = match kind {
                    ObjectPropertyKind::Get => rusty_js_ast::MethodKind::Getter,
                    ObjectPropertyKind::Set => rusty_js_ast::MethodKind::Setter,
                    ObjectPropertyKind::Init => rusty_js_ast::MethodKind::Method,
                };
                self.validate_accessor_parameters(method_kind, &params)?;
                if self.last_body_became_strict {
                    self.revalidate_params_after_strict_promotion(&params, None)?;
                }
                let end = self.last_span_end();
                let func = Expr::Function {
                    name: None,
                    is_async: false,
                    is_generator: false,
                    is_method: true,
                    params,
                    body,
                    span: Span::new(prop_start, end),
                };
                properties.push(ObjectProperty::Property {
                    key,
                    value: func,
                    shorthand: false,
                    kind,
                    span: Span::new(prop_start, end),
                });
                crate::parser::parse_profile::record_depth3_object_prop(
                    crate::parser::parse_profile::ObjectPropPhase::Accessor,
                    prop_profile_start,
                );
            } else {
                let prop_start = self.lookahead_span().start;
                let colon_key_profile_start =
                    crate::parser::parse_profile::depth3_object_prop_start();
                let key = self.parse_object_key()?;
                if matches!(self.current_kind(), TokenKind::Punct(Punct::Colon)) {
                    depth3_colon_property = true;
                    let prop_profile_start =
                        crate::parser::parse_profile::depth3_object_prop_start();
                    crate::parser::parse_profile::record_depth3_object_colon_phase(
                        crate::parser::parse_profile::ObjectColonPhase::Key,
                        colon_key_profile_start,
                    );

                    self.bump()?;
                    let colon_value_kind = match self.current_kind() {
                        TokenKind::Ident(s) if s == "function" || s == "class" => {
                            parse_profile::Kind::NestedVarInitFnClass
                        }
                        TokenKind::Punct(Punct::LParen) => parse_profile::Kind::NestedVarInitParen,
                        TokenKind::Punct(Punct::LBrace) | TokenKind::Punct(Punct::LBracket) => {
                            parse_profile::Kind::NestedVarInitObjectArray
                        }
                        TokenKind::Ident(_) => parse_profile::Kind::NestedVarInitIdent,
                        TokenKind::Number(..)
                        | TokenKind::BigInt(..)
                        | TokenKind::String(_)
                        | TokenKind::WtfString(_)
                        | TokenKind::Template { .. }
                        | TokenKind::Regex { .. } => parse_profile::Kind::NestedVarInitLiteral,
                        _ => parse_profile::Kind::NestedVarInitOther,
                    };
                    let colon_value_profile_start =
                        crate::parser::parse_profile::depth3_object_prop_start();
                    let value = self.parse_cover_for_head_pattern_assignment_expression()?;
                    crate::parser::parse_profile::record_depth3_object_colon_phase(
                        crate::parser::parse_profile::ObjectColonPhase::Value,
                        colon_value_profile_start,
                    );
                    crate::parser::parse_profile::record_depth3_object_colon_value(
                        colon_value_kind,
                        colon_value_profile_start,
                    );
                    let end = value.span().end;
                    properties.push(ObjectProperty::Property {
                        key,
                        value,
                        shorthand: false,
                        kind: ObjectPropertyKind::Init,
                        span: Span::new(prop_start, end),
                    });
                    crate::parser::parse_profile::record_depth3_object_prop(
                        crate::parser::parse_profile::ObjectPropPhase::Colon,
                        prop_profile_start,
                    );
                } else if matches!(self.current_kind(), TokenKind::Punct(Punct::LParen)) {
                    let prop_profile_start =
                        crate::parser::parse_profile::depth3_object_prop_start();

                    let params = self.parse_unique_formal_parameters_ga(false, false)?;
                    let body = self.parse_function_body_gs(
                        Some(false),
                        Some(false),
                        Self::is_simple_param_list(&params),
                    )?;
                    let end = self.last_span_end();
                    let func = Expr::Function {
                        name: None,
                        is_async: false,
                        is_generator: false,
                        is_method: true,
                        params,
                        body,
                        span: Span::new(prop_start, end),
                    };
                    properties.push(ObjectProperty::Property {
                        key,
                        value: func,
                        shorthand: false,
                        kind: ObjectPropertyKind::Init,
                        span: Span::new(prop_start, end),
                    });
                    crate::parser::parse_profile::record_depth3_object_prop(
                        crate::parser::parse_profile::ObjectPropPhase::Method,
                        prop_profile_start,
                    );
                } else {
                    let prop_profile_start =
                        crate::parser::parse_profile::depth3_object_prop_start();

                    let (name, key_span) = match &key {
                        ObjectKey::Identifier { name, span } => (name.clone(), *span),
                        _ => {
                            return Err(
                                self.err_here("only identifier keys support shorthand".into())
                            )
                        }
                    };
                    if (self.strict_mode || self.in_generator) && name == "yield" {
                        return Err(ParseError {
                            span: key_span,
                            message: "`yield` is not a valid shorthand identifier in this context"
                                .into(),
                        });
                    }

                    if crate::parser::is_unconditional_reserved_word(&name)
                        || (self.strict_mode && crate::parser::is_strict_reserved_word(&name))
                    {
                        return Err(ParseError {
                            span: key_span,
                            message: format!(
                                "`{}` is a reserved word and cannot be used as a shorthand identifier",
                                name
                            ),
                        });
                    }
                    let ident = Expr::Identifier {
                        name: name.clone(),
                        span: key_span,
                    };
                    let value = if matches!(self.current_kind(), TokenKind::Punct(Punct::Assign)) {
                        self.bump()?;
                        let default = self.parse_cover_for_head_pattern_assignment_expression()?;
                        let end = default.span().end;
                        Expr::Assign {
                            operator: rusty_js_ast::AssignOp::Assign,
                            target: Box::new(ident),
                            value: Box::new(default),
                            span: Span::new(key_span.start, end),
                        }
                    } else {
                        ident
                    };
                    let val_end = value.span().end;
                    properties.push(ObjectProperty::Property {
                        key,
                        value,
                        shorthand: true,
                        kind: ObjectPropertyKind::Init,
                        span: Span::new(prop_start, val_end),
                    });
                    crate::parser::parse_profile::record_depth3_object_prop(
                        crate::parser::parse_profile::ObjectPropPhase::Shorthand,
                        prop_profile_start,
                    );
                }
            }
            if matches!(self.current_kind(), TokenKind::Punct(Punct::Comma)) {
                let colon_comma_profile_start = depth3_colon_property
                    .then(crate::parser::parse_profile::depth3_object_prop_start)
                    .flatten();
                self.bump()?;

                trailing_comma_after_spread = iter_is_spread;
                crate::parser::parse_profile::record_depth3_object_colon_phase(
                    crate::parser::parse_profile::ObjectColonPhase::Comma,
                    colon_comma_profile_start,
                );
            } else {
                trailing_comma_after_spread = false;
                break;
            }
        }
        self.expect_punct(Punct::RBrace)?;
        let end = self.last_span_end();
        Ok(Expr::Object {
            properties,
            trailing_comma_after_spread,
            span: Span::new(start, end),
        })
    }

    fn parse_object_key(&mut self) -> Result<ObjectKey, ParseError> {
        let span = self.lookahead_span();
        match self.current_kind().clone() {
            TokenKind::Ident(name) => {
                self.bump()?;
                Ok(ObjectKey::Identifier { name, span })
            }
            TokenKind::String(value) => {
                self.bump()?;
                Ok(ObjectKey::String { value, span })
            }
            TokenKind::WtfString(units) => {
                self.bump()?;
                Ok(ObjectKey::String {
                    value: String::from_utf16_lossy(&units),
                    span,
                })
            }
            TokenKind::Number(value, _) => {
                self.bump()?;
                Ok(ObjectKey::Number { value, span })
            }
            TokenKind::BigInt(digits, kind) => {
                self.bump()?;
                Ok(ObjectKey::String {
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
                let end = self.last_span_end();
                Ok(ObjectKey::Computed {
                    expr,
                    span: Span::new(span.start, end),
                })
            }
            _ => Err(self.err_here("expected object key".into())),
        }
    }

    pub(crate) fn bigint_literal_property_name(
        digits: &str,
        kind: crate::token::NumberKind,
    ) -> String {
        use crate::token::NumberKind;
        let radix: u16 = match kind {
            NumberKind::Decimal => return digits.to_string(),
            NumberKind::Binary => 2,
            NumberKind::Octal | NumberKind::LegacyOctal => 8,
            NumberKind::Hex => 16,
        };
        let mut out = vec![0u8];
        for b in digits.bytes() {
            let d = match b {
                b'0'..=b'9' => b - b'0',
                b'a'..=b'f' => b - b'a' + 10,
                b'A'..=b'F' => b - b'A' + 10,
                _ => 0,
            };
            let mut carry = d as u16;
            for digit in out.iter_mut() {
                let v = (*digit as u16) * radix + carry;
                *digit = (v % 10) as u8;
                carry = v / 10;
            }
            while carry != 0 {
                out.push((carry % 10) as u8);
                carry /= 10;
            }
        }
        while out.len() > 1 && out.last() == Some(&0) {
            out.pop();
        }
        out.iter().rev().map(|d| char::from(b'0' + *d)).collect()
    }

    fn parse_parenthesized(&mut self) -> Result<Expr, ParseError> {
        self.enter_parse_depth()?;
        let start = self.lookahead_span().start;
        if let Err(err) = self.expect_punct(Punct::LParen) {
            self.leave_parse_depth();
            return Err(err);
        }

        let saved_in_disallowed = self.in_disallowed;
        self.in_disallowed = false;
        let expr = {
            let _paren_depth = parse_profile::ParenDepthGuard::new();
            let _profile = parse_profile::Guard::new(parse_profile::Kind::ParenInner);
            let inner_family = match self.current_kind() {
                TokenKind::Ident(name) if name == "function" => {
                    parse_profile::ParenInnerFamily::Function
                }
                TokenKind::Ident(name) if name == "class" => parse_profile::ParenInnerFamily::Class,
                TokenKind::Punct(Punct::LParen) => parse_profile::ParenInnerFamily::Paren,
                TokenKind::Punct(Punct::LBrace) | TokenKind::Punct(Punct::LBracket) => {
                    parse_profile::ParenInnerFamily::ObjectArray
                }
                TokenKind::Ident(_) | TokenKind::PrivateIdent(_) => {
                    parse_profile::ParenInnerFamily::Ident
                }
                TokenKind::Number(..)
                | TokenKind::BigInt(..)
                | TokenKind::String(_)
                | TokenKind::WtfString(_)
                | TokenKind::Regex { .. }
                | TokenKind::Template { .. } => parse_profile::ParenInnerFamily::Literal,
                _ => parse_profile::ParenInnerFamily::Other,
            };
            let _inner_family_profile = parse_profile::ParenInnerFamilyGuard::new(inner_family);
            self.parse_expression()
        };
        self.in_disallowed = saved_in_disallowed;
        let expr = match expr {
            Ok(expr) => expr,
            Err(err) => {
                self.leave_parse_depth();
                return Err(err);
            }
        };
        if let Err(err) = self.expect_punct(Punct::RParen) {
            self.leave_parse_depth();
            return Err(err);
        }
        let end = self.last_span_end();
        self.leave_parse_depth();
        Ok(Expr::Parenthesized {
            expr: Box::new(expr),
            span: Span::new(start, end),
        })
    }

    pub fn parse_expression(&mut self) -> Result<Expr, ParseError> {
        let _profile = parse_profile::Guard::new(parse_profile::Kind::Expression);
        let first = self.parse_assignment_expression()?;
        if !matches!(self.current_kind(), TokenKind::Punct(Punct::Comma)) {
            return Ok(first);
        }
        let start = first.span().start;
        let mut expressions = vec![first];
        while matches!(self.current_kind(), TokenKind::Punct(Punct::Comma)) {
            self.bump()?;
            expressions.push(self.parse_assignment_expression()?);
        }
        let end = expressions.last().unwrap().span().end;
        Ok(Expr::Sequence {
            expressions,
            span: Span::new(start, end),
        })
    }

    fn parse_tagged_template(&mut self, tag: Expr, start: usize) -> Result<Expr, ParseError> {
        use crate::token::TemplatePart;
        use rusty_js_ast::{Argument, ArrayElement};

        let (cooked_quasis, raw_quasis, expressions, end) = match self.current_kind().clone() {
            TokenKind::Template {
                cooked, raw, part, ..
            } => {
                let tspan = self.lookahead_span();
                match part {
                    TemplatePart::NoSubstitution => {
                        self.bump()?;
                        (vec![cooked], vec![raw], Vec::new(), tspan.end)
                    }
                    TemplatePart::Head => {
                        self.parse_tagged_template_with_substitutions(tspan.start)?
                    }
                    _ => return Err(self.err_here("unexpected template part for tag".into())),
                }
            }
            _ => return Err(self.err_here("expected template after tag".into())),
        };
        let strings_arr = Expr::Array {
            elements: cooked_quasis
                .iter()
                .map(|q| match q {
                    Some(value) => ArrayElement::Expr(Expr::StringLiteral {
                        value: value.clone(),
                        span: Span::new(start, end),
                    }),
                    None => ArrayElement::Expr(Expr::Identifier {
                        name: "undefined".into(),
                        span: Span::new(start, end),
                    }),
                })
                .collect(),
            trailing_comma_after_spread: false,
            span: Span::new(start, end),
        };
        let raw_arr = Expr::Array {
            elements: raw_quasis
                .iter()
                .map(|q| {
                    ArrayElement::Expr(Expr::StringLiteral {
                        value: q.clone(),
                        span: Span::new(start, end),
                    })
                })
                .collect(),
            trailing_comma_after_spread: false,
            span: Span::new(start, end),
        };
        let site_key = Expr::StringLiteral {
            value: format!("{}:{}", start, end),
            span: Span::new(start, end),
        };
        let template_object = Expr::Call {
            callee: Box::new(Expr::Identifier {
                name: "__template_object__".into(),
                span: Span::new(start, end),
            }),
            arguments: vec![
                Argument::Expr(strings_arr),
                Argument::Expr(site_key),
                Argument::Expr(raw_arr),
            ],
            optional: false,
            span: Span::new(start, end),
        };
        let mut arguments: Vec<Argument> = vec![Argument::Expr(template_object)];
        for e in expressions {
            arguments.push(Argument::Expr(e));
        }
        Ok(Expr::Call {
            callee: Box::new(tag),
            arguments,
            optional: false,
            span: Span::new(start, end),
        })
    }

    fn parse_tagged_template_with_substitutions(
        &mut self,
        start: usize,
    ) -> Result<(Vec<Option<String>>, Vec<String>, Vec<Expr>, usize), ParseError> {
        use crate::token::TemplatePart;
        let mut cooked_quasis = Vec::new();
        let mut raw_quasis = Vec::new();
        let mut expressions = Vec::new();

        match self.current_kind().clone() {
            TokenKind::Template {
                cooked,
                raw,
                part: TemplatePart::Head,
                ..
            } => {
                cooked_quasis.push(cooked);
                raw_quasis.push(raw);
            }
            _ => return Err(self.err_here("expected template head".into())),
        }
        self.bump()?;

        loop {
            expressions.push(self.parse_expression()?);
            self.enter_template_tail()?;
            match self.current_kind().clone() {
                TokenKind::Template {
                    cooked,
                    raw,
                    part: TemplatePart::Middle,
                    ..
                } => {
                    cooked_quasis.push(cooked);
                    raw_quasis.push(raw);
                    self.bump()?;
                }
                TokenKind::Template {
                    cooked,
                    raw,
                    part: TemplatePart::Tail,
                    ..
                } => {
                    cooked_quasis.push(cooked);
                    raw_quasis.push(raw);
                    self.bump()?;
                    break;
                }
                _ => {
                    return Err(
                        self.err_here("expected template middle/tail after substitution".into())
                    )
                }
            }
        }

        Ok((
            cooked_quasis,
            raw_quasis,
            expressions,
            self.last_span_end().max(start),
        ))
    }

    fn skip_ws_and_comments(
        bytes: &[u8],
        mut j: usize,
        allow_line_terminator: bool,
    ) -> Option<usize> {
        loop {
            while j < bytes.len() {
                match bytes[j] {
                    b' ' | b'\t' => j += 1,
                    b'\n' | b'\r' if allow_line_terminator => j += 1,
                    b'\n' | b'\r' => return None,
                    _ => break,
                }
            }
            if bytes.get(j) == Some(&b'/') && bytes.get(j + 1) == Some(&b'/') {
                if !allow_line_terminator {
                    return None;
                }
                j += 2;
                while j < bytes.len() && bytes[j] != b'\n' && bytes[j] != b'\r' {
                    j += 1;
                }
                continue;
            }
            if bytes.get(j) == Some(&b'/') && bytes.get(j + 1) == Some(&b'*') {
                j += 2;
                while j + 1 < bytes.len() && !(bytes[j] == b'*' && bytes[j + 1] == b'/') {
                    if !allow_line_terminator && (bytes[j] == b'\n' || bytes[j] == b'\r') {
                        return None;
                    }
                    j += 1;
                }
                j = (j + 2).min(bytes.len());
                continue;
            }
            return Some(j);
        }
    }

    fn looks_like_async_method_shorthand(&self) -> bool {
        let is_async_ident = matches!(self.current_kind(), TokenKind::Ident(n) if n == "async");
        if !is_async_ident {
            return false;
        }
        let src = self.source().as_bytes();
        let span = self.lookahead_span();

        let Some(mut j) = Self::skip_ws_and_comments(src, span.end, false) else {
            return false;
        };

        if let Some(&b'*') = src.get(j) {
            j += 1;
            let Some(next) = Self::skip_ws_and_comments(src, j, false) else {
                return false;
            };
            j = next;
        }
        match src.get(j) {

            Some(&b) if b.is_ascii_alphabetic() || b == b'_' || b == b'$' => true,
            Some(&b) if b >= 0x80 || b == b'\\' => true,
            Some(&b'"') | Some(&b'\'') => true,
            Some(&b'[') => true,
            Some(&b) if b.is_ascii_digit() => true,
            _ => false,
        }
    }

    fn looks_like_accessor_shorthand(&self) -> bool {
        let is_accessor_ident = match self.current_kind() {
            TokenKind::Ident(n) => n == "get" || n == "set",
            _ => false,
        };
        if !is_accessor_ident {
            return false;
        }
        let src = self.source().as_bytes();
        let span = self.lookahead_span();

        let Some(j) = Self::skip_ws_and_comments(src, span.end, true) else {
            return false;
        };
        match src.get(j) {

            Some(&b) if b.is_ascii_alphabetic() || b == b'_' || b == b'$' => true,
            Some(&b) if b >= 0x80 || b == b'\\' => true,
            Some(&b'"') | Some(&b'\'') => true,
            Some(&b'[') => true,
            Some(&b) if b.is_ascii_digit() => true,
            _ => false,
        }
    }

    fn looks_like_arrow_function_head(&self) -> bool {

        let src = self.source().as_bytes();
        let start = self.lookahead_span().start;
        match self.current_kind() {
            TokenKind::Ident(name) => {

                if matches!(
                    name.as_str(),
                    "typeof"
                        | "void"
                        | "delete"
                        | "await"
                        | "new"
                        | "function"
                        | "class"
                        | "this"
                        | "super"
                        | "null"
                        | "true"
                        | "false"
                        | "return"
                        | "throw"
                        | "if"
                        | "else"
                        | "for"
                        | "while"
                        | "do"
                        | "switch"
                        | "case"
                        | "default"
                        | "break"
                        | "continue"
                        | "try"
                        | "catch"
                        | "finally"
                        | "var"
                        | "let"
                        | "const"
                        | "import"
                        | "export"
                ) || (name == "yield" && (self.strict_mode || self.in_generator))
                {
                    return false;
                }

                let mut j = start;
                while j < src.len()
                    && (src[j].is_ascii_alphanumeric() || src[j] == b'_' || src[j] == b'$')
                {
                    j += 1;
                }
                let Some(j) = Self::skip_ws_and_comments(src, j, false) else {
                    return false;
                };
                src.get(j) == Some(&b'=') && src.get(j + 1) == Some(&b'>')
            }
            TokenKind::Punct(Punct::LParen) => {

                let mut j = start + 1;
                let mut depth = 1i32;
                while j < src.len() && depth > 0 {
                    match src[j] {
                        b'/' if src.get(j + 1) == Some(&b'/') => {

                            j += 2;
                            while j < src.len() && src[j] != b'\n' && src[j] != b'\r' {
                                j += 1;
                            }
                            continue;
                        }
                        b'/' if src.get(j + 1) == Some(&b'*') => {

                            j += 2;
                            while j + 1 < src.len() && !(src[j] == b'*' && src[j + 1] == b'/') {
                                j += 1;
                            }
                            j = (j + 2).min(src.len());
                            continue;
                        }
                        b'(' => depth += 1,
                        b')' => depth -= 1,
                        b'\'' | b'"' => {

                            let q = src[j];
                            j += 1;
                            while j < src.len() && src[j] != q {
                                if src[j] == b'\\' && j + 1 < src.len() {
                                    j += 2;
                                    continue;
                                }
                                j += 1;
                            }
                        }
                        b'`' => {

                            j += 1;
                            while j < src.len() && src[j] != b'`' {
                                j += 1;
                            }
                        }
                        _ => {}
                    }
                    j += 1;
                }

                loop {
                    while j < src.len() && matches!(src[j], b' ' | b'\t' | b'\n' | b'\r') {
                        j += 1;
                    }
                    if j + 1 < src.len() && src[j] == b'/' && src[j + 1] == b'/' {
                        j += 2;
                        while j < src.len() && src[j] != b'\n' && src[j] != b'\r' {
                            j += 1;
                        }
                        continue;
                    }
                    if j + 1 < src.len() && src[j] == b'/' && src[j + 1] == b'*' {
                        j += 2;
                        while j + 1 < src.len() && !(src[j] == b'*' && src[j + 1] == b'/') {
                            j += 1;
                        }
                        j = (j + 2).min(src.len());
                        continue;
                    }
                    break;
                }
                src.get(j) == Some(&b'=') && src.get(j + 1) == Some(&b'>')
            }
            _ => false,
        }
    }

    fn opaque_until_top_terminator(&mut self) -> Result<Expr, ParseError> {
        let start = self.lookahead_span().start;
        let mut depth_paren = 0i32;
        let mut depth_brace = 0i32;
        let mut depth_bracket = 0i32;
        while !self.at_eof_internal() {
            let kind = self.current_kind().clone();
            match kind {
                TokenKind::Punct(Punct::LParen) => depth_paren += 1,
                TokenKind::Punct(Punct::RParen) => {
                    if depth_paren == 0 {
                        break;
                    }
                    depth_paren -= 1;
                }
                TokenKind::Punct(Punct::LBrace) => depth_brace += 1,
                TokenKind::Punct(Punct::RBrace) => {
                    if depth_brace == 0 {
                        break;
                    }
                    depth_brace -= 1;
                }
                TokenKind::Punct(Punct::LBracket) => depth_bracket += 1,
                TokenKind::Punct(Punct::RBracket) => {
                    if depth_bracket == 0 {
                        break;
                    }
                    depth_bracket -= 1;
                }
                TokenKind::Punct(Punct::Comma) | TokenKind::Punct(Punct::Semicolon) => {
                    if depth_paren == 0 && depth_brace == 0 && depth_bracket == 0 {
                        break;
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
        let end = self.last_span_end();
        Ok(Expr::Opaque {
            span: Span::new(start, end),
        })
    }

    fn opaque_until_top_terminator_within_braces(&mut self) -> Result<Expr, ParseError> {
        let start = self.lookahead_span().start;
        if matches!(self.current_kind(), TokenKind::Punct(Punct::LParen)) {
            self.skip_balanced_public(Punct::LParen, Punct::RParen)?;
        }
        if matches!(self.current_kind(), TokenKind::Punct(Punct::LBrace)) {
            self.skip_balanced_public(Punct::LBrace, Punct::RBrace)?;
        }
        let end = self.last_span_end();
        Ok(Expr::Opaque {
            span: Span::new(start, end),
        })
    }

    fn parse_function_expression(
        &mut self,
        is_async: bool,
        async_start: Option<usize>,
    ) -> Result<Expr, ParseError> {
        let start = async_start.unwrap_or_else(|| self.lookahead_span().start);
        let fn_head_start = parse_profile::top_var_init_ident_arrow_expr_cond_fnclass_phase_start();
        self.expect_keyword("function")?;
        let is_generator = if matches!(self.current_kind(), TokenKind::Punct(Punct::Star)) {
            self.bump()?;
            true
        } else {
            false
        };
        let name = if let TokenKind::Ident(n) = self.current_kind().clone() {
            if !matches!(n.as_str(), "(") {
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
                if (is_generator || self.strict_mode) && n == "yield" {
                    return Err(ParseError {
                        span,
                        message: "`yield` is not a valid function name in this context".into(),
                    });
                }
                if is_async && n == "await" {
                    return Err(ParseError {
                        span,
                        message: "`await` is not a valid function name in async function code"
                            .into(),
                    });
                }
                self.bump()?;
                Some(rusty_js_ast::BindingIdentifier { name: n, span })
            } else {
                None
            }
        } else {
            None
        };
        parse_profile::record_top_var_init_ident_arrow_expr_cond_fnclass_phase(
            parse_profile::TopVarInitIdentArrowExprCondFnClassPhase::FnHead,
            fn_head_start,
        );
        let params = {
            let _profile = parse_profile::Guard::new(parse_profile::Kind::FnExprParams);
            let phase_start =
                parse_profile::top_var_init_ident_arrow_expr_cond_fnclass_phase_start();
            let parsed = self.parse_function_parameters_ga(is_generator, is_async)?;
            parse_profile::record_top_var_init_ident_arrow_expr_cond_fnclass_phase(
                parse_profile::TopVarInitIdentArrowExprCondFnClassPhase::FnParams,
                phase_start,
            );
            parsed
        };
        self.check_formal_parameter_dups(&params)?;
        let body = {
            let _profile = parse_profile::Guard::new(parse_profile::Kind::FnExprBody);
            let phase_start =
                parse_profile::top_var_init_ident_arrow_expr_cond_fnclass_phase_start();
            let parsed = self.parse_function_body_gs(
                Some(is_generator),
                Some(is_async),
                Self::is_simple_param_list(&params),
            )?;
            parse_profile::record_top_var_init_ident_arrow_expr_cond_fnclass_phase(
                parse_profile::TopVarInitIdentArrowExprCondFnClassPhase::FnBody,
                phase_start,
            );
            parsed
        };
        if self.last_body_became_strict {
            let _profile = parse_profile::Guard::new(parse_profile::Kind::FnExprRevalidate);
            let phase_start =
                parse_profile::top_var_init_ident_arrow_expr_cond_fnclass_phase_start();
            self.revalidate_params_after_strict_promotion(&params, name.as_ref())?;
            parse_profile::record_top_var_init_ident_arrow_expr_cond_fnclass_phase(
                parse_profile::TopVarInitIdentArrowExprCondFnClassPhase::FnRevalidate,
                phase_start,
            );
        }
        let end = self.last_span_end();
        Ok(Expr::Function {
            name,
            is_async,
            is_generator,
            is_method: false,
            params,
            body,
            span: Span::new(start, end),
        })
    }

    fn parse_class_expression(&mut self) -> Result<Expr, ParseError> {
        self.parse_class_expression_with_decorators(Vec::new())
    }

    fn parse_class_expression_with_decorators(
        &mut self,
        decorators: Vec<String>,
    ) -> Result<Expr, ParseError> {
        let start = self.lookahead_span().start;
        let class_head_start =
            parse_profile::top_var_init_ident_arrow_expr_cond_fnclass_phase_start();
        self.expect_keyword("class")?;
        let name = if let TokenKind::Ident(n) = self.current_kind().clone() {
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
                Some(rusty_js_ast::BindingIdentifier { name: n, span })
            } else {
                None
            }
        } else {
            None
        };
        parse_profile::record_top_var_init_ident_arrow_expr_cond_fnclass_phase(
            parse_profile::TopVarInitIdentArrowExprCondFnClassPhase::ClassHead,
            class_head_start,
        );
        let super_class = if self.is_ident("extends") {
            let _profile = parse_profile::Guard::new(parse_profile::Kind::ClassExprSuper);
            let phase_start =
                parse_profile::top_var_init_ident_arrow_expr_cond_fnclass_phase_start();
            self.bump()?;
            let parsed = Some(Box::new(self.parse_left_hand_side_expression()?));
            parse_profile::record_top_var_init_ident_arrow_expr_cond_fnclass_phase(
                parse_profile::TopVarInitIdentArrowExprCondFnClassPhase::ClassSuper,
                phase_start,
            );
            parsed
        } else {
            None
        };
        let members = {
            let _profile = parse_profile::Guard::new(parse_profile::Kind::ClassExprBody);
            let phase_start =
                parse_profile::top_var_init_ident_arrow_expr_cond_fnclass_phase_start();
            let parsed = self.parse_class_body()?;
            parse_profile::record_top_var_init_ident_arrow_expr_cond_fnclass_phase(
                parse_profile::TopVarInitIdentArrowExprCondFnClassPhase::ClassBody,
                phase_start,
            );
            parsed
        };
        let end = self.last_span_end();
        Ok(Expr::Class {
            decorators,
            name,
            super_class,
            members,
            span: Span::new(start, end),
        })
    }

    fn untagged_template_tv(&self, cooked: Option<String>) -> Result<String, ParseError> {
        cooked.ok_or_else(|| {
            self.err_here("invalid escape sequence in untagged template literal".into())
        })
    }

    fn parse_template_with_substitutions(&mut self, start: usize) -> Result<Expr, ParseError> {
        use crate::token::TemplatePart;
        let mut quasis: Vec<std::rc::Rc<String>> = Vec::new();
        let mut raw_quasis: Vec<std::rc::Rc<String>> = Vec::new();
        let mut expressions: Vec<Expr> = Vec::new();
        let (head_cooked, head_raw) = match self.current_kind().clone() {
            TokenKind::Template {
                cooked,
                raw,
                part: TemplatePart::Head,
            } => (self.untagged_template_tv(cooked)?, raw),
            _ => return Err(self.err_here("expected template head".into())),
        };
        quasis.push(std::rc::Rc::new(head_cooked));
        raw_quasis.push(std::rc::Rc::new(head_raw));
        self.bump()?;
        loop {

            let saved_in_disallowed = std::mem::take(&mut self.in_disallowed);
            let expr = self.parse_expression();
            self.in_disallowed = saved_in_disallowed;
            let expr = expr?;
            expressions.push(expr);
            self.enter_template_tail()?;
            match self.current_kind().clone() {
                TokenKind::Template {
                    cooked,
                    raw,
                    part: TemplatePart::Middle,
                } => {
                    quasis.push(std::rc::Rc::new(self.untagged_template_tv(cooked)?));
                    raw_quasis.push(std::rc::Rc::new(raw));
                    self.bump()?;
                    continue;
                }
                TokenKind::Template {
                    cooked,
                    raw,
                    part: TemplatePart::Tail,
                } => {
                    quasis.push(std::rc::Rc::new(self.untagged_template_tv(cooked)?));
                    raw_quasis.push(std::rc::Rc::new(raw));
                    self.bump()?;
                    break;
                }
                _ => {
                    return Err(
                        self.err_here("expected template middle/tail after substitution".into())
                    )
                }
            }
        }
        let end = self.last_span_end();
        Ok(Expr::TemplateLiteral {
            quasis,
            raw_quasis,
            expressions,
            span: Span::new(start, end),
        })
    }

    fn parse_arrow_function(
        &mut self,
        is_async: bool,
        start_override: Option<usize>,
    ) -> Result<Expr, ParseError> {
        let start = start_override.unwrap_or_else(|| self.lookahead_span().start);

        let top_arrow_head_start = parse_profile::top_var_init_ident_arrow_phase_start();
        let params: Vec<rusty_js_ast::Parameter> =
            if let TokenKind::Ident(n) = self.current_kind().clone() {

                let span = self.lookahead_span();
                if is_async && n == "await" {
                    return Err(self.err_at(
                        span,
                        "`await` is not a valid binding in async function code".into(),
                    ));
                }
                self.bump()?;
                vec![rusty_js_ast::Parameter {
                    target: rusty_js_ast::BindingPattern::Identifier(
                        rusty_js_ast::BindingIdentifier { name: n, span },
                    ),
                    default: None,
                    rest: false,
                    span,
                }]
            } else if matches!(self.current_kind(), TokenKind::Punct(Punct::LParen)) {

                self.parse_function_parameters_gai(false, is_async, true)?
            } else {
                return Err(self.err_here("expected arrow head".into()));
            };
        parse_profile::record_top_var_init_ident_arrow_phase(
            parse_profile::TopVarInitIdentArrowPhase::Head,
            top_arrow_head_start,
        );

        if matches!(self.current_kind(), TokenKind::Punct(Punct::Arrow))
            && self.lookahead_preceded_by_lt()
        {
            return Err(
                self.err_here("No line terminator allowed before `=>` in arrow function".into())
            );
        }
        let top_arrow_token_start = parse_profile::top_var_init_ident_arrow_phase_start();
        self.expect_punct(Punct::Arrow)?;
        parse_profile::record_top_var_init_ident_arrow_phase(
            parse_profile::TopVarInitIdentArrowPhase::ArrowToken,
            top_arrow_token_start,
        );

        let top_arrow_validate_start = parse_profile::top_var_init_ident_arrow_phase_start();
        let mut seen: Vec<(String, rusty_js_ast::Span)> = Vec::new();
        for p in &params {
            for id in p.target.collect_names() {
                if seen.iter().any(|(s, _)| s == &id.name) {
                    return Err(self.err_at(
                        id.span,
                        format!("arrow function has duplicate parameter name `{}`", id.name),
                    ));
                }

                let is_await_name = id.name == "await";
                let mode_gate = if is_await_name {
                    is_async || self.in_async || self.is_module_goal()
                } else if self.strict_mode {
                    crate::parser::is_reserved_word(&id.name)
                        || id.name == "eval"
                        || id.name == "arguments"
                } else {
                    crate::parser::is_unconditional_reserved_word(&id.name)
                };
                if mode_gate {
                    let suffix = if is_await_name {
                        " in async or module code"
                    } else if self.strict_mode {
                        " in strict mode"
                    } else {
                        ""
                    };
                    return Err(self.err_at(
                        id.span,
                        format!(
                            "arrow function parameter `{}` is a reserved word{}",
                            id.name, suffix
                        ),
                    ));
                }
                seen.push((id.name.clone(), id.span));
            }
        }
        parse_profile::record_top_var_init_ident_arrow_phase(
            parse_profile::TopVarInitIdentArrowPhase::Validate,
            top_arrow_validate_start,
        );

        let (body, end) = if matches!(self.current_kind(), TokenKind::Punct(Punct::LBrace)) {
            let top_arrow_body_start = parse_profile::top_var_init_ident_arrow_phase_start();
            let body = self.parse_function_body_gs(
                Some(false),
                Some(is_async),
                Self::is_simple_param_list(&params),
            )?;
            parse_profile::record_top_var_init_ident_arrow_phase(
                parse_profile::TopVarInitIdentArrowPhase::BlockBody,
                top_arrow_body_start,
            );
            (ArrowBody::Block(body), self.last_span_end())
        } else {
            self.function_body_depth += 1;
            let prior_gen = self.in_generator;
            let prior_async = self.in_async;

            let prior_in_params = self.in_function_params;
            self.in_generator = false;
            self.in_async = is_async;
            self.in_function_params = false;
            let top_arrow_body_start = parse_profile::top_var_init_ident_arrow_phase_start();
            let e = self.parse_assignment_expression()?;
            parse_profile::record_top_var_init_ident_arrow_phase(
                parse_profile::TopVarInitIdentArrowPhase::ExprBody,
                top_arrow_body_start,
            );
            let end = e.span().end;
            self.in_generator = prior_gen;
            self.in_async = prior_async;
            self.in_function_params = prior_in_params;
            self.function_body_depth = self.function_body_depth.saturating_sub(1);
            (ArrowBody::Expression(Box::new(e)), end)
        };
        Ok(Expr::Arrow {
            is_async,
            params,
            body,
            span: Span::new(start, end),
        })
    }
}

fn object_key_static_name(key: &ObjectKey) -> Option<String> {
    match key {
        ObjectKey::Identifier { name, .. } => Some(name.clone()),
        ObjectKey::String { value, .. } => Some(value.clone()),
        _ => None,
    }
}

fn object_key_span(key: &ObjectKey) -> Span {
    match key {
        ObjectKey::Identifier { span, .. }
        | ObjectKey::String { span, .. }
        | ObjectKey::Number { span, .. }
        | ObjectKey::Computed { span, .. } => *span,
    }
}

fn validate_regexp_flags(flags: &str, span: rusty_js_ast::Span) -> Result<(), ParseError> {
    let mut seen = [false; 128];
    let (mut has_u, mut has_v) = (false, false);
    for c in flags.chars() {
        if !matches!(c, 'd' | 'g' | 'i' | 'm' | 's' | 'u' | 'v' | 'y') {
            return Err(ParseError {
                span,
                message: format!("invalid regular expression flag `{c}`"),
            });
        }
        let idx = c as usize;
        if idx < 128 && seen[idx] {
            return Err(ParseError {
                span,
                message: format!("duplicate regular expression flag `{c}`"),
            });
        }
        if idx < 128 {
            seen[idx] = true;
        }
        has_u |= c == 'u';
        has_v |= c == 'v';
    }
    if has_u && has_v {
        return Err(ParseError {
            span,
            message: "regular expression flags `u` and `v` are mutually exclusive".into(),
        });
    }
    Ok(())
}

fn validate_v_mode_class_syntax(
    pattern: &str,
    flags: &str,
    span: rusty_js_ast::Span,
) -> Result<(), ParseError> {
    if !flags.contains('v') {
        return Ok(());
    }
    const RESERVED: &str = "!#$%*+,.:;<=>?@^`~";
    let chars: Vec<char> = pattern.chars().collect();
    let n = chars.len();
    let mut i = 0;

    let mut stack: Vec<(bool, bool)> = Vec::new();
    let err = |span, msg: String| Err(ParseError { span, message: msg });
    while i < n {
        let c = chars[i];
        if c == '\\' {
            if i + 2 < n && chars[i + 1] == 'u' && chars[i + 2] == '{' {

                i += 3;
                while i < n && chars[i] != '}' {
                    i += 1;
                }
                i = (i + 1).min(n);
            } else if i + 2 < n && matches!(chars[i + 1], 'p' | 'P' | 'q') && chars[i + 2] == '{' {
                if chars[i + 1] == 'q' && stack.is_empty() {
                    return err(
                        span,
                        "`\\q{}` is only valid inside a `v`-mode character class".into(),
                    );
                }

                i += 3;
                let mut depth = 1;
                while i < n && depth > 0 {
                    if chars[i] == '\\' {
                        i += 2;
                        continue;
                    }
                    if chars[i] == '{' {
                        depth += 1;
                    } else if chars[i] == '}' {
                        depth -= 1;
                    }
                    i += 1;
                }
            } else {
                i += 2;
            }

            if let Some(top) = stack.last_mut() {
                top.0 = true;
                top.1 = false;
            }
            continue;
        }
        if c == '[' {
            stack.push((false, false));
            i += 1;
            if i < n && chars[i] == '^' {
                i += 1;
            }
            continue;
        }
        if stack.is_empty() {
            i += 1;
            continue;
        }
        if c == ']' {
            let (_, pending) = *stack.last().unwrap();
            if pending {
                return err(
                    span,
                    "missing operand after `&&`/`--`/`-` in a `v`-mode character class".into(),
                );
            }
            stack.pop();

            if let Some(parent) = stack.last_mut() {
                parent.0 = true;
                parent.1 = false;
            }
            i += 1;
            continue;
        }

        if i + 1 < n && chars[i + 1] == c && RESERVED.contains(c) {
            return err(
                span,
                format!("reserved double punctuator `{c}{c}` in a `v`-mode character class"),
            );
        }

        if matches!(c, '(' | ')' | '{' | '}' | '|' | '/') {
            return err(
                span,
                format!("`{c}` must be escaped in a `v`-mode character class"),
            );
        }

        if (c == '&' && chars.get(i + 1) == Some(&'&'))
            || (c == '-' && chars.get(i + 1) == Some(&'-'))
        {
            let top = stack.last_mut().unwrap();
            if !top.0 {
                return err(
                    span,
                    format!("missing operand before `{c}{c}` in a `v`-mode character class"),
                );
            }
            top.0 = false;
            top.1 = true;
            i += 2;
            continue;
        }

        if c == '-' {
            let top = stack.last_mut().unwrap();
            if !top.0 {
                return err(
                    span,
                    "lone `-` in a `v`-mode character class must be escaped".into(),
                );
            }
            top.0 = false;
            top.1 = true;
            i += 1;
            continue;
        }

        let top = stack.last_mut().unwrap();
        top.0 = true;
        top.1 = false;
        i += 1;
    }
    if !stack.is_empty() {
        return err(span, "unterminated `v`-mode character class".into());
    }
    Ok(())
}

fn validate_inline_modifiers(pattern: &str, span: rusty_js_ast::Span) -> Result<(), ParseError> {
    let chars: Vec<char> = pattern.chars().collect();
    let n = chars.len();
    let mut i = 0;
    let mut in_class = false;
    while i < n {
        let c = chars[i];
        if c == '\\' {
            i += 2;
            continue;
        }
        if in_class {
            if c == ']' {
                in_class = false;
            }
            i += 1;
            continue;
        }
        if c == '[' {
            in_class = true;
            i += 1;
            continue;
        }
        if c == '(' && i + 1 < n && chars[i + 1] == '?' {
            match chars.get(i + 2).copied() {

                Some(':') | Some('=') | Some('!') | Some('<') => {
                    i += 2;
                    continue;
                }
                _ => {

                    let mut j = i + 2;
                    let mut add: Vec<char> = Vec::new();
                    while j < n && !matches!(chars[j], ':' | '-' | ')') {
                        add.push(chars[j]);
                        j += 1;
                    }
                    let mut has_dash = false;
                    let mut remove: Vec<char> = Vec::new();
                    if j < n && chars[j] == '-' {
                        has_dash = true;
                        j += 1;
                        while j < n && !matches!(chars[j], ':' | '-' | ')') {
                            remove.push(chars[j]);
                            j += 1;
                        }
                    }
                    if j >= n || chars[j] != ':' {
                        return Err(ParseError {
                            span,
                            message: "invalid regular expression inline modifier: \
                                      expected `:` after modifier flags"
                                .into(),
                        });
                    }
                    validate_modifier_run(&add, span)?;
                    if has_dash {
                        validate_modifier_run(&remove, span)?;
                        for c in &add {
                            if remove.contains(c) {
                                return Err(ParseError {
                                    span,
                                    message: format!(
                                        "regular expression modifier flag `{c}` \
                                         appears in both the added and removed flag sets"
                                    ),
                                });
                            }
                        }
                        if add.is_empty() && remove.is_empty() {
                            return Err(ParseError {
                                span,
                                message: "regular expression modifier group has \
                                          empty added and removed flag sets"
                                    .into(),
                            });
                        }
                    }
                    i = j + 1;
                    continue;
                }
            }
        }
        i += 1;
    }
    Ok(())
}

fn validate_modifier_run(run: &[char], span: rusty_js_ast::Span) -> Result<(), ParseError> {
    let mut seen: Vec<char> = Vec::new();
    for &c in run {
        if c != 'i' && c != 'm' && c != 's' {
            return Err(ParseError {
                span,
                message: format!("invalid regular expression modifier flag `{c}`"),
            });
        }
        if seen.contains(&c) {
            return Err(ParseError {
                span,
                message: format!("duplicate regular expression modifier flag `{c}`"),
            });
        }
        seen.push(c);
    }
    Ok(())
}

fn validate_named_group_refs(
    pattern: &str,
    flags: &str,
    span: rusty_js_ast::Span,
) -> Result<(), ParseError> {
    let scan = collect_named_groups(pattern);
    if let Err(dup) = &scan.duplicate {
        return Err(ParseError {
            span,
            message: format!("duplicate capture group name `{dup}`"),
        });
    }
    let strict = flags.contains('u') || flags.contains('v') || !scan.all_defined.is_empty();
    if strict {
        for name in &scan.refs {
            if !scan.all_defined.contains(name) {
                return Err(ParseError {
                    span,
                    message: format!("reference to undefined capture group name `{name}`"),
                });
            }
        }
    }
    Ok(())
}

fn count_capturing_groups(chars: &[char]) -> u64 {
    let n = chars.len();
    let mut count = 0u64;
    let mut i = 0;
    let mut in_class = false;
    while i < n {
        match chars[i] {
            '\\' => {
                i += 2;
                continue;
            }
            '[' if !in_class => in_class = true,
            ']' if in_class => in_class = false,
            '(' if !in_class => {
                let capturing = if chars.get(i + 1) == Some(&'?') {
                    chars.get(i + 2) == Some(&'<')
                        && !matches!(chars.get(i + 3), Some('=') | Some('!'))
                } else {
                    true
                };
                if capturing {
                    count += 1;
                }
            }
            _ => {}
        }
        i += 1;
    }
    count
}

fn validate_unicode_mode_escapes(
    pattern: &str,
    flags: &str,
    span: rusty_js_ast::Span,
) -> Result<(), ParseError> {
    if !(flags.contains('u') || flags.contains('v')) {
        return Ok(());
    }
    let chars: Vec<char> = pattern.chars().collect();
    let n = chars.len();
    let group_count = count_capturing_groups(&chars);
    let err = |m: &str| {
        Err(ParseError {
            span,
            message: m.to_string(),
        })
    };
    const VALID_LETTERS: &[char] = &[
        'd', 'D', 's', 'S', 'w', 'W', 'b', 'B', 'f', 'n', 'r', 't', 'v', 'c', 'x', 'u', 'p', 'P',
        'k',
    ];
    let mut i = 0;
    let mut in_class = false;
    while i < n {
        let c = chars[i];
        if c == '[' && !in_class {
            in_class = true;
            i += 1;
            continue;
        }
        if c == ']' && in_class {
            in_class = false;
            i += 1;
            continue;
        }
        if c != '\\' {
            i += 1;
            continue;
        }
        let Some(&e) = chars.get(i + 1) else {
            return err("trailing backslash in regular expression");
        };
        if e == 'c' {
            if !matches!(chars.get(i + 2), Some(ch) if ch.is_ascii_alphabetic()) {
                return err("invalid `\\c` control escape in unicode-mode regular expression");
            }
            i += 3;
            continue;
        }
        if e == 'u' {
            if chars.get(i + 2) == Some(&'{') {
                let start = i + 3;
                let mut j = start;
                while j < n && chars[j] != '}' {
                    j += 1;
                }
                let hex: String = chars[start..j.min(n)].iter().collect();
                let ok = j < n
                    && j > start
                    && hex.chars().all(|h| h.is_ascii_hexdigit())
                    && u32::from_str_radix(&hex, 16)
                        .map(|v| v <= 0x10FFFF)
                        .unwrap_or(false);
                if !ok {
                    return err("invalid `\\u{...}` unicode escape in regular expression");
                }
                i = j + 1;
                continue;
            }
            let ok = i + 6 <= n && chars[i + 2..i + 6].iter().all(|h| h.is_ascii_hexdigit());
            if !ok {
                return err("invalid `\\u` unicode escape in regular expression");
            }
            i += 6;
            continue;
        }
        if e == '0' {
            if matches!(chars.get(i + 2), Some(d) if d.is_ascii_digit()) {
                return err(
                    "legacy octal escape is not allowed in unicode-mode regular expression",
                );
            }
            i += 2;
            continue;
        }
        if e.is_ascii_digit() {
            let mut j = i + 1;
            while j < n && chars[j].is_ascii_digit() {
                j += 1;
            }
            let v: u64 = chars[i + 1..j]
                .iter()
                .collect::<String>()
                .parse()
                .unwrap_or(u64::MAX);
            if v > group_count {
                return err("invalid decimal escape (backreference exceeds capturing groups) in unicode-mode regular expression");
            }
            i = j;
            continue;
        }

        let v_q = flags.contains('v') && e == 'q';
        if e.is_ascii_alphabetic() && !VALID_LETTERS.contains(&e) && !v_q {
            return err("invalid identity escape in unicode-mode regular expression");
        }
        i += 2;
    }
    Ok(())
}

fn decode_regexp_identifier_name(name: &[char]) -> Result<Vec<u32>, ()> {
    let n = name.len();
    let mut out = Vec::new();
    let mut i = 0;
    while i < n {
        if name[i] != '\\' {
            out.push(name[i] as u32);
            i += 1;
            continue;
        }
        if name.get(i + 1) != Some(&'u') {
            return Err(());
        }
        i += 2;
        if name.get(i) == Some(&'{') {
            let start = i + 1;
            let mut j = start;
            while j < n && name[j] != '}' {
                j += 1;
            }
            if j >= n || j == start {
                return Err(());
            }
            let v = u32::from_str_radix(&name[start..j].iter().collect::<String>(), 16)
                .map_err(|_| ())?;
            if v > 0x10FFFF {
                return Err(());
            }
            out.push(v);
            i = j + 1;
        } else {
            if i + 4 > n {
                return Err(());
            }
            let v = u32::from_str_radix(&name[i..i + 4].iter().collect::<String>(), 16)
                .map_err(|_| ())?;
            i += 4;

            if (0xD800..=0xDBFF).contains(&v)
                && name.get(i) == Some(&'\\')
                && name.get(i + 1) == Some(&'u')
            {
                if i + 6 <= n {
                    if let Ok(lo) =
                        u32::from_str_radix(&name[i + 2..i + 6].iter().collect::<String>(), 16)
                    {
                        if (0xDC00..=0xDFFF).contains(&lo) {
                            out.push(0x10000 + ((v - 0xD800) << 10) + (lo - 0xDC00));
                            i += 6;
                            continue;
                        }
                    }
                }
            }
            out.push(v);
        }
    }
    Ok(out)
}

fn validate_quantifiers(
    pattern: &str,
    flags: &str,
    span: rusty_js_ast::Span,
) -> Result<(), ParseError> {
    let chars: Vec<char> = pattern.chars().collect();
    let n = chars.len();
    let unicode = flags.contains('u') || flags.contains('v');

    let mut group_stack: Vec<u8> = Vec::new();
    let mut prev_quantifiable = false;

    let mut prev_assertion: Option<bool> = None;
    let mut prev_was_quantifier = false;
    let mut i = 0;
    let mut in_class = false;

    let has_named_groups = {
        let mut hng = false;
        let mut k = 0;
        while k < n {
            if chars[k] == '\\' {
                k += 2;
                continue;
            }
            if chars[k] == '('
                && chars.get(k + 1) == Some(&'?')
                && chars.get(k + 2) == Some(&'<')
                && !matches!(chars.get(k + 3), Some('=') | Some('!'))
            {
                hng = true;
                break;
            }
            k += 1;
        }
        hng
    };
    let check_target = |prev_q: bool, asrt: Option<bool>| -> Result<(), ParseError> {
        if let Some(is_lb) = asrt {
            if is_lb {
                return Err(ParseError {
                    span,
                    message: "a lookbehind assertion cannot be quantified".into(),
                });
            }
            if unicode {
                return Err(ParseError {
                    span,
                    message: "a lookahead assertion cannot be quantified in unicode mode".into(),
                });
            }
            return Ok(());
        }
        if !prev_q {
            return Err(ParseError {
                span,
                message: "nothing to repeat: quantifier has no preceding atom".into(),
            });
        }
        Ok(())
    };
    while i < n {
        let c = chars[i];
        if c == '\\' {

            if matches!(chars.get(i + 1), Some('u' | 'p' | 'P')) && chars.get(i + 2) == Some(&'{') {
                let mut j = i + 3;
                while j < n && chars[j] != '}' {
                    j += 1;
                }
                i = if j < n { j + 1 } else { j };
                prev_quantifiable = true;
                prev_assertion = None;
                prev_was_quantifier = false;
                continue;
            }

            if chars.get(i + 1) == Some(&'k') && !in_class && (unicode || has_named_groups) {
                if chars.get(i + 2) != Some(&'<') {
                    return Err(ParseError {
                        span,
                        message: "Invalid named reference: `\\k` must be followed by a group name"
                            .into(),
                    });
                }
                let mut j = i + 3;
                let mut name_len = 0usize;
                while j < n {
                    let nc = chars[j];
                    if nc == '>' {
                        break;
                    }
                    if nc == '\\' {

                        j += 2;
                        name_len += 1;
                        continue;
                    }
                    if nc.is_alphanumeric() || nc == '$' || nc == '_' {
                        name_len += 1;
                        j += 1;
                    } else {
                        break;
                    }
                }
                if name_len == 0 || j >= n || chars[j] != '>' {
                    return Err(ParseError {
                        span,
                        message: "Invalid named reference: incomplete or invalid group name".into(),
                    });
                }
                i = j + 1;
                prev_quantifiable = true;
                prev_assertion = None;
                prev_was_quantifier = false;
                continue;
            }
            i += 2;
            prev_quantifiable = true;
            prev_assertion = None;
            prev_was_quantifier = false;
            continue;
        }
        if c == '[' && !in_class {
            in_class = true;
            i += 1;
            continue;
        }
        if c == ']' && in_class {
            in_class = false;
            i += 1;
            prev_quantifiable = true;
            prev_assertion = None;
            prev_was_quantifier = false;
            continue;
        }
        if in_class {
            i += 1;
            continue;
        }
        if c == '(' {

            let (kind, skip) = if chars.get(i + 1) == Some(&'?') {

                let modifier_skip = if matches!(chars.get(i + 2), Some(&('i' | 'm' | 's' | '-'))) {
                    let mut j = i + 2;
                    while j < n && matches!(chars[j], 'i' | 'm' | 's' | '-') {
                        j += 1;
                    }
                    if chars.get(j) == Some(&':') {
                        Some(j - i + 1)
                    } else {
                        None
                    }
                } else {
                    None
                };
                if let Some(s) = modifier_skip {
                    (0u8, s)
                } else {
                    match (chars.get(i + 2), chars.get(i + 3)) {
                        (Some('='), _) => (1u8, 3),
                        (Some('!'), _) => (1u8, 3),
                        (Some('<'), Some('=')) => (2u8, 4),
                        (Some('<'), Some('!')) => (2u8, 4),
                        (Some('<'), _) => (0u8, 3),
                        (Some(':'), _) => (0u8, 3),
                        _ => (0u8, 1),
                    }
                }
            } else {
                (0u8, 1)
            };
            group_stack.push(kind);
            i += skip;
            prev_quantifiable = false;
            prev_assertion = None;
            prev_was_quantifier = false;
            continue;
        }
        if c == ')' {
            let kind = group_stack.pop().unwrap_or(0);
            i += 1;
            prev_quantifiable = true;
            prev_assertion = match kind {
                1 => Some(false),
                2 => Some(true),
                _ => None,
            };
            prev_was_quantifier = false;
            continue;
        }
        if c == '|' || c == '^' {
            i += 1;
            prev_quantifiable = false;
            prev_assertion = None;
            prev_was_quantifier = false;
            continue;
        }
        if matches!(c, '*' | '+' | '?') {

            if c == '?' && prev_was_quantifier {
                i += 1;
                prev_was_quantifier = false;
                prev_quantifiable = false;
                continue;
            }
            check_target(prev_quantifiable, prev_assertion)?;
            i += 1;
            prev_quantifiable = false;
            prev_assertion = None;
            prev_was_quantifier = true;
            continue;
        }
        if c == '{' {

            let mut j = i + 1;
            let lo_start = j;
            while j < n && chars[j].is_ascii_digit() {
                j += 1;
            }
            let mut shape: Option<(u64, Option<u64>, usize)> = None;
            if j > lo_start {
                let lo: u64 = chars[lo_start..j]
                    .iter()
                    .collect::<String>()
                    .parse()
                    .unwrap_or(u64::MAX);
                if j < n && chars[j] == '}' {
                    shape = Some((lo, Some(lo), j));
                } else if j < n && chars[j] == ',' {
                    let hi_start = j + 1;
                    let mut k = hi_start;
                    while k < n && chars[k].is_ascii_digit() {
                        k += 1;
                    }
                    if k < n && chars[k] == '}' {
                        let hi = if k > hi_start {
                            Some(
                                chars[hi_start..k]
                                    .iter()
                                    .collect::<String>()
                                    .parse()
                                    .unwrap_or(0),
                            )
                        } else {
                            None
                        };
                        shape = Some((lo, hi, k));
                    }
                }
            }
            if let Some((lo, hi, end)) = shape {
                check_target(prev_quantifiable, prev_assertion)?;
                if let Some(hi) = hi {
                    if hi < lo {
                        return Err(ParseError {
                            span,
                            message: "regular expression quantifier range out of order (max < min)"
                                .into(),
                        });
                    }
                }
                i = end + 1;
                prev_quantifiable = false;
                prev_assertion = None;
                prev_was_quantifier = true;
                continue;
            }

            if flags.contains('u') {
                return Err(ParseError {
                    span,
                    message: "lone `{` is not a valid atom in unicode-mode RegExp".into(),
                });
            }
            i += 1;
            prev_quantifiable = true;
            prev_assertion = None;
            prev_was_quantifier = false;
            continue;
        }

        if flags.contains('u') && matches!(c, '}' | ']') {
            return Err(ParseError {
                span,
                message: format!("lone `{c}` is not a valid atom in unicode-mode RegExp"),
            });
        }

        i += 1;
        prev_quantifiable = true;
        prev_assertion = None;
        prev_was_quantifier = false;
    }
    Ok(())
}

fn validate_named_group_specifiers(
    pattern: &str,
    flags: &str,
    span: rusty_js_ast::Span,
) -> Result<(), ParseError> {
    let chars: Vec<char> = pattern.chars().collect();
    let n = chars.len();
    let has_named_groups = !collect_named_groups(pattern).all_defined.is_empty();

    let unicode = flags.contains('u') || flags.contains('v');
    let k_is_backref = unicode || has_named_groups;
    let err = |m: &str| {
        Err(ParseError {
            span,
            message: m.to_string(),
        })
    };
    let mut i = 0;
    let mut in_class = false;
    while i < n {
        let c = chars[i];
        if c == '\\' {

            if k_is_backref && i + 1 < n && chars[i + 1] == 'k' {
                if chars.get(i + 2) != Some(&'<') {
                    return err("`\\k` must be followed by a named-capture reference `<name>`");
                }

                let name_start = i + 3;
                let mut j = name_start;
                while j < n && chars[j] != '>' && chars[j] != '\\' {
                    j += 1;
                }
                if j >= n || chars[j] != '>' {
                    return err("unterminated or malformed named backreference");
                }
                if j == name_start {
                    return err("empty named backreference");
                }
                let name = &chars[name_start..j];
                if !crate::lexer::is_id_start(name[0] as u32)
                    || !name[1..]
                        .iter()
                        .all(|&ch| crate::lexer::is_id_continue(ch as u32))
                {
                    return err("invalid named backreference: not a valid identifier name");
                }
                i = j + 1;
                continue;
            }
            i += 2;
            continue;
        }
        if in_class {
            if c == ']' {
                in_class = false;
            }
            i += 1;
            continue;
        }
        if c == '[' {
            in_class = true;
            i += 1;
            continue;
        }

        if c == '('
            && i + 2 < n
            && chars[i + 1] == '?'
            && chars[i + 2] == '<'
            && !matches!(chars.get(i + 3), Some('=') | Some('!'))
        {
            let mut j = i + 3;
            let mut name: Vec<char> = Vec::new();
            while j < n && chars[j] != '>' {
                name.push(chars[j]);
                j += 1;
            }
            if j >= n {
                return err("unterminated group specifier in regular expression");
            }
            if name.is_empty() {
                return err("empty named capture group specifier");
            }
            let cps = match decode_regexp_identifier_name(&name) {
                Ok(c) => c,
                Err(()) => return err("invalid escape in capture group name"),
            };
            if cps.is_empty() {
                return err("empty named capture group specifier");
            }
            if !crate::lexer::is_id_start(cps[0]) {
                return err(
                    "invalid capture group name: does not start with an identifier code point",
                );
            }
            if !cps[1..].iter().all(|&cp| crate::lexer::is_id_continue(cp)) {
                return err("invalid capture group name: contains a non-identifier code point");
            }
            i = j + 1;
            continue;
        }
        i += 1;
    }
    Ok(())
}

pub(crate) struct NamedGroupScan {
    pub duplicate: Result<(), String>,
    pub all_defined: Vec<String>,
    pub refs: Vec<String>,
}

pub(crate) fn collect_named_groups(pattern: &str) -> NamedGroupScan {
    let chars: Vec<char> = pattern.chars().collect();
    let n = chars.len();
    let mut frames: Vec<Vec<String>> = vec![Vec::new()];
    let mut all_defined: Vec<String> = Vec::new();
    let mut refs: Vec<String> = Vec::new();
    let mut duplicate: Result<(), String> = Ok(());
    let mut i = 0;
    let mut in_class = false;
    while i < n {
        let c = chars[i];
        if c == '\\' {
            if i + 2 < n && chars[i + 1] == 'k' && chars[i + 2] == '<' {
                let mut j = i + 3;
                let mut name = String::new();
                while j < n && chars[j] != '>' {
                    name.push(chars[j]);
                    j += 1;
                }
                if j < n && chars[j] == '>' {
                    refs.push(name);
                    i = j + 1;
                    continue;
                }
            }
            i += 2;
            continue;
        }
        if in_class {
            if c == ']' {
                in_class = false;
            }
            i += 1;
            continue;
        }
        if c == '[' {
            in_class = true;
            i += 1;
            continue;
        }
        if c == '|' {
            if let Some(top) = frames.last_mut() {
                top.clear();
            }
            i += 1;
            continue;
        }
        if c == ')' {
            if frames.len() > 1 {
                frames.pop();
            }
            i += 1;
            continue;
        }
        if c == '(' {

            if i + 2 < n
                && chars[i + 1] == '?'
                && chars[i + 2] == '<'
                && !matches!(chars.get(i + 3), Some('=') | Some('!'))
            {
                let mut j = i + 3;
                let mut name = String::new();
                while j < n && chars[j] != '>' {
                    name.push(chars[j]);
                    j += 1;
                }
                if j < n && chars[j] == '>' {
                    if duplicate.is_ok() && frames.iter().any(|f| f.contains(&name)) {
                        duplicate = Err(name.clone());
                    }
                    if let Some(top) = frames.last_mut() {
                        top.push(name.clone());
                    }
                    if !all_defined.contains(&name) {
                        all_defined.push(name);
                    }
                    frames.push(Vec::new());
                    i = j + 1;
                    continue;
                }
            }

            frames.push(Vec::new());
            i += 1;
            continue;
        }
        i += 1;
    }
    NamedGroupScan {
        duplicate,
        all_defined,
        refs,
    }
}

const STRING_PROPERTIES: &[&str] = &[
    "Basic_Emoji",
    "Emoji_Keycap_Sequence",
    "RGI_Emoji",
    "RGI_Emoji_Flag_Sequence",
    "RGI_Emoji_Modifier_Sequence",
    "RGI_Emoji_Tag_Sequence",
    "RGI_Emoji_ZWJ_Sequence",
];

fn validate_string_properties(
    pattern: &str,
    flags: &str,
    span: rusty_js_ast::Span,
) -> Result<(), ParseError> {
    let v_mode = flags.contains('v');
    let bytes = pattern.as_bytes();
    let mut i = 0;

    let mut class_neg: Vec<bool> = Vec::new();
    while i + 1 < bytes.len() {
        if bytes[i] == b'\\' && !matches!(bytes[i + 1], b'p' | b'P') {
            i += 2;
            continue;
        }
        if bytes[i] == b'[' {
            let negated_class = bytes.get(i + 1) == Some(&b'^');
            class_neg.push(negated_class);
            i += if negated_class { 2 } else { 1 };
            continue;
        }
        if bytes[i] == b']' {
            class_neg.pop();
            i += 1;
            continue;
        }
        if bytes[i] == b'\\'
            && matches!(bytes[i + 1], b'p' | b'P')
            && bytes.get(i + 2) == Some(&b'{')
        {
            let negated = bytes[i + 1] == b'P';
            let start = i + 3;
            if let Some(rel_end) = pattern[start..].find('}') {
                let end = start + rel_end;
                let body = &pattern[start..end];
                if STRING_PROPERTIES.contains(&body) {
                    if negated {
                        return Err(ParseError {
                            span,
                            message: format!(
                                "Unicode property of strings `{body}` cannot be negated"
                            ),
                        });
                    }
                    if !v_mode {
                        return Err(ParseError {
                            span,
                            message: format!(
                                "Unicode property of strings `{body}` requires the `v` flag"
                            ),
                        });
                    }
                    if class_neg.iter().any(|&n| n) {
                        return Err(ParseError {
                            span,
                            message: format!(
                                "Unicode property of strings `{body}` cannot appear in a negated class"
                            ),
                        });
                    }
                }
                i = end + 1;
                continue;
            }
        }
        i += 1;
    }
    Ok(())
}

fn validate_unicode_property_escapes(
    pattern: &str,
    flags: &str,
    span: rusty_js_ast::Span,
) -> Result<(), ParseError> {
    if !flags.contains('u') && !flags.contains('v') {
        return Ok(());
    }

    let bytes = pattern.as_bytes();
    let mut i = 0;
    while i + 1 < bytes.len() {
        if bytes[i] == b'\\' && matches!(bytes[i + 1], b'p' | b'P') {
            if bytes.get(i + 2) != Some(&b'{') {
                return Err(ParseError {
                    span,
                    message: "invalid Unicode property escape".into(),
                });
            }
            let mut j = i + 3;
            while j < bytes.len() && bytes[j] != b'}' {
                j += 1;
            }
            if j == bytes.len() {
                return Err(ParseError {
                    span,
                    message: "unterminated Unicode property escape".into(),
                });
            }
            let body = &pattern[i + 3..j];
            if !crate::generated_unicode::property_escape_names::is_known_unicode_property_escape(
                body,
            ) {
                return Err(ParseError {
                    span,
                    message: format!("invalid Unicode property escape `{body}`"),
                });
            }
            i = j + 1;
            continue;
        }
        i += 1;
    }

    Ok(())
}

#[derive(Clone, Copy, Debug)]
enum ClassAtomForRange {
    Single(u32),
    ClassEscape,
}

fn validate_character_class_ranges(
    pattern: &str,
    flags: &str,
    span: rusty_js_ast::Span,
) -> Result<(), ParseError> {

    if flags.contains('v') {
        return Ok(());
    }
    let chars: Vec<char> = pattern.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '\\' {
            i += 2;
            continue;
        }
        if chars[i] != '[' {
            i += 1;
            continue;
        }
        i += 1;
        if chars.get(i) == Some(&'^') {
            i += 1;
        }
        while i < chars.len() && chars[i] != ']' {
            let (left, next) = parse_class_atom_for_range(&chars, i);
            i = next;
            if chars.get(i) != Some(&'-') || chars.get(i + 1) == Some(&']') {
                continue;
            }
            let (right, after_right) = parse_class_atom_for_range(&chars, i + 1);
            match (left, right) {
                (ClassAtomForRange::Single(a), ClassAtomForRange::Single(b)) => {
                    if a > b {

                        return Err(ParseError {
                            span,
                            message: "invalid regular expression character class range".into(),
                        });
                    }
                    i = after_right;
                }
                _ => {

                    if flags.contains('u') {
                        return Err(ParseError {
                            span,
                            message: "invalid regular expression character class range".into(),
                        });
                    }
                    i += 1;
                }
            }
        }
        if i < chars.len() && chars[i] == ']' {
            i += 1;
        }
    }
    Ok(())
}

fn parse_class_atom_for_range(chars: &[char], start: usize) -> (ClassAtomForRange, usize) {
    if start >= chars.len() {
        return (ClassAtomForRange::ClassEscape, start);
    }
    if chars[start] != '\\' {
        return (ClassAtomForRange::Single(chars[start] as u32), start + 1);
    }
    let Some(&esc) = chars.get(start + 1) else {
        return (ClassAtomForRange::Single('\\' as u32), start + 1);
    };
    match esc {
        'd' | 'D' | 's' | 'S' | 'w' | 'W' | 'B' => (ClassAtomForRange::ClassEscape, start + 2),
        'p' | 'P' if chars.get(start + 2) == Some(&'{') => {
            let mut i = start + 3;
            while i < chars.len() && chars[i] != '}' {
                i += 1;
            }
            (ClassAtomForRange::ClassEscape, (i + 1).min(chars.len()))
        }
        'b' => (ClassAtomForRange::Single(0x08), start + 2),
        't' => (ClassAtomForRange::Single(0x09), start + 2),
        'n' => (ClassAtomForRange::Single(0x0A), start + 2),
        'v' => (ClassAtomForRange::Single(0x0B), start + 2),
        'f' => (ClassAtomForRange::Single(0x0C), start + 2),
        'r' => (ClassAtomForRange::Single(0x0D), start + 2),
        '0'..='7' => {
            let mut value = 0u32;
            let mut i = start + 1;
            let mut consumed = 0;
            while consumed < 3 && i < chars.len() {
                let Some(d) = chars[i].to_digit(8) else {
                    break;
                };
                value = value * 8 + d;
                i += 1;
                consumed += 1;
            }
            (ClassAtomForRange::Single(value), i)
        }
        'x' if start + 3 < chars.len() => {
            let hex: String = chars[start + 2..=start + 3].iter().collect();
            let value = u32::from_str_radix(&hex, 16).unwrap_or(esc as u32);
            (ClassAtomForRange::Single(value), start + 4)
        }
        'u' if chars.get(start + 2) == Some(&'{') => {

            let mut i = start + 3;
            let mut hex = String::new();
            while i < chars.len() && chars[i] != '}' {
                hex.push(chars[i]);
                i += 1;
            }
            let value = u32::from_str_radix(&hex, 16).unwrap_or(esc as u32);
            (ClassAtomForRange::Single(value), (i + 1).min(chars.len()))
        }
        'u' if start + 5 < chars.len() => {
            let hex: String = chars[start + 2..=start + 5].iter().collect();
            let value = u32::from_str_radix(&hex, 16).unwrap_or(esc as u32);
            (ClassAtomForRange::Single(value), start + 6)
        }
        'c' if start + 2 < chars.len() => {
            let c = chars[start + 2] as u32;
            (ClassAtomForRange::Single(c & 0x1f), start + 3)
        }
        _ => (ClassAtomForRange::Single(esc as u32), start + 2),
    }
}
