
use super::ast::{BinOp, Expr, Quantifier, UnOp};
use super::lexer::Tok;
use super::PgError;

fn err(input: &str) -> PgError {
    PgError::InvalidInputSyntax {
        typ: "expression",
        input: input.to_string(),
    }
}

fn validate_token_nesting(toks: &[Tok], src: &str) -> Result<(), PgError> {
    let mut paren_depth = 0usize;
    let mut not_depth = 0usize;
    for tok in toks {
        match tok {
            Tok::LParen => {
                paren_depth += 1;
                not_depth = 0;
                if paren_depth > Parser::MAX_DEPTH {
                    return Err(err(src));
                }
            }
            Tok::RParen => {
                paren_depth = paren_depth.saturating_sub(1);
                not_depth = 0;
            }
            Tok::Ident(s) if s == "not" => {
                not_depth += 1;
                if not_depth > Parser::MAX_DEPTH {
                    return Err(err(src));
                }
            }
            _ => not_depth = 0,
        }
    }
    Ok(())
}

fn is_pred_kw(s: &str) -> bool {
    matches!(s, "between" | "in" | "like" | "ilike")
}

fn is_comparison(op: BinOp) -> bool {
    matches!(
        op,
        BinOp::Lt | BinOp::Gt | BinOp::Eq | BinOp::LtEq | BinOp::GtEq | BinOp::NotEq
    )
}

fn bin_expr(op: BinOp, l: Expr, r: Expr) -> Expr {
    Expr::Binary {
        op,
        left: Box::new(l),
        right: Box::new(r),
    }
}

struct Parser<'a> {
    toks: &'a [Tok],
    pos: usize,
    src: &'a str,
    depth: usize,
}

impl<'a> Parser<'a> {
    const MAX_DEPTH: usize = 256;

    fn peek(&self) -> &Tok {
        &self.toks[self.pos]
    }

    fn next(&mut self) -> Tok {
        let t = self.toks[self.pos].clone();
        if self.pos + 1 < self.toks.len() {
            self.pos += 1;
        }
        t
    }

    fn ident_is(t: &Tok, w: &str) -> bool {
        matches!(t, Tok::Ident(s) if s == w)
    }

    fn quantifier(t: &Tok) -> Option<Quantifier> {
        match t {
            Tok::Ident(s) if s == "any" || s == "some" => Some(Quantifier::Any),
            Tok::Ident(s) if s == "all" => Some(Quantifier::All),
            _ => None,
        }
    }

    fn peek2(&self) -> &Tok {
        self.toks
            .get(self.pos + 1)
            .unwrap_or_else(|| &self.toks[self.toks.len() - 1])
    }

    fn infix_bp(t: &Tok) -> Option<(u8, u8, BinOp)> {
        Some(match t {
            Tok::Ident(s) if s == "or" => (10, 11, BinOp::Or),
            Tok::Ident(s) if s == "and" => (20, 21, BinOp::And),
            Tok::Lt => (40, 41, BinOp::Lt),
            Tok::Gt => (40, 41, BinOp::Gt),
            Tok::Eq => (40, 41, BinOp::Eq),
            Tok::LtEq => (40, 41, BinOp::LtEq),
            Tok::GtEq => (40, 41, BinOp::GtEq),
            Tok::NotEq => (40, 41, BinOp::NotEq),
            Tok::Plus => (60, 61, BinOp::Add),
            Tok::Minus => (60, 61, BinOp::Sub),
            Tok::Star => (70, 71, BinOp::Mul),
            Tok::Slash => (70, 71, BinOp::Div),
            Tok::Percent => (70, 71, BinOp::Mod),
            Tok::Caret => (80, 81, BinOp::Pow),
            _ => return None,
        })
    }

    fn parse_expr(&mut self, min_bp: u8) -> Result<Expr, PgError> {
        if self.depth >= Self::MAX_DEPTH {
            return Err(err(self.src));
        }
        self.depth += 1;
        let mut lhs = self.parse_prefix()?;

        loop {

            if matches!(self.peek(), Tok::Dot) {
                if 100 < min_bp {
                    break;
                }
                self.next();
                match self.next() {
                    Tok::Ident(field) => {
                        lhs = Expr::FieldAccess {
                            base: Box::new(lhs),
                            field,
                            comp_oid: 0,
                            field_oid: 0,
                        };
                        continue;
                    }
                    _ => return Err(err(self.src)),
                }
            }

            if matches!(self.peek(), Tok::Cast) {
                if 100 < min_bp {
                    break;
                }
                self.next();
                let type_name = self.parse_type_name()?;
                lhs = Expr::Cast {
                    expr: Box::new(lhs),
                    type_name,
                };
                continue;
            }

            if Self::ident_is(self.peek(), "collate") {
                if 100 < min_bp {
                    break;
                }
                self.next();
                let mut collation = match self.peek() {
                    Tok::Ident(s) => s.clone(),
                    _ => return Err(err(self.src)),
                };
                self.next();

                if matches!(self.peek(), Tok::Dot) {
                    self.next();
                    collation = match self.peek() {
                        Tok::Ident(s) => s.clone(),
                        _ => return Err(err(self.src)),
                    };
                    self.next();
                }
                lhs = Expr::Collate {
                    expr: Box::new(lhs),
                    collation,
                };
                continue;
            }

            if Self::ident_is(self.peek(), "is") {
                self.next();
                let negated = Self::ident_is(self.peek(), "not");
                if negated {
                    self.next();
                }
                if !Self::ident_is(self.peek(), "null") {
                    return Err(err(self.src));
                }
                self.next();
                lhs = Expr::IsNull {
                    expr: Box::new(lhs),
                    negated,
                };
                continue;
            }

            if min_bp < 40 {
                let pred = match self.peek() {
                    Tok::Ident(s) if is_pred_kw(s) => Some((false, s.clone())),
                    Tok::Ident(s) if s == "not" => match self.peek2() {
                        Tok::Ident(s2) if is_pred_kw(s2) => Some((true, s2.clone())),
                        _ => None,
                    },
                    _ => None,
                };
                if let Some((negated, kw)) = pred {
                    if negated {
                        self.next();
                    }
                    self.next();
                    lhs = self.parse_predicate(lhs, &kw, negated)?;
                    continue;
                }
            }

            if let Tok::Op(spelling) = self.peek() {
                if 50 < min_bp {
                    break;
                }
                let op = spelling.clone();
                self.next();
                let rhs = self.parse_expr(51)?;
                lhs = Expr::GenBinary {
                    op,
                    left: Box::new(lhs),
                    right: Box::new(rhs),
                };
                continue;
            }

            let (l_bp, r_bp, op) = match Self::infix_bp(self.peek()) {
                Some(t) => t,
                None => break,
            };
            if l_bp < min_bp {
                break;
            }
            self.next();

            if is_comparison(op) {
                if let Some(quantifier) = Self::quantifier(self.peek()) {
                    self.next();
                    if !matches!(self.peek(), Tok::LParen) {
                        return Err(err(self.src));
                    }
                    self.next();
                    if !Self::ident_is(self.peek(), "select") {

                        return Err(err(self.src));
                    }
                    let (q, next) = crate::stmt::parser::parse_query_at_depth(
                        self.toks, self.pos, self.src, self.depth,
                    )?;
                    self.pos = next;
                    if !matches!(self.next(), Tok::RParen) {
                        return Err(err(self.src));
                    }
                    lhs = Expr::Quantified {
                        expr: Box::new(lhs),
                        op,
                        quantifier,
                        query: Box::new(q),
                    };
                    continue;
                }
            }
            let rhs = self.parse_expr(r_bp)?;
            lhs = Expr::Binary {
                op,
                left: Box::new(lhs),
                right: Box::new(rhs),
            };
        }

        self.depth = self.depth.saturating_sub(1);
        Ok(lhs)
    }

    fn parse_prefix(&mut self) -> Result<Expr, PgError> {
        match self.peek() {
            Tok::Ident(s) if s == "not" => {
                self.next();
                let expr = self.parse_expr(30)?;
                Ok(Expr::Unary {
                    op: UnOp::Not,
                    expr: Box::new(expr),
                })
            }
            Tok::Minus => {
                self.next();
                let expr = self.parse_expr(90)?;
                Ok(Expr::Unary {
                    op: UnOp::Neg,
                    expr: Box::new(expr),
                })
            }
            Tok::Plus => {
                self.next();
                let expr = self.parse_expr(90)?;
                Ok(Expr::Unary {
                    op: UnOp::Plus,
                    expr: Box::new(expr),
                })
            }

            Tok::Op(s) => {
                let op = s.clone();
                self.next();
                let expr = self.parse_expr(90)?;
                Ok(Expr::GenUnary {
                    op,
                    expr: Box::new(expr),
                })
            }
            _ => self.parse_primary(),
        }
    }

    fn parse_type_name(&mut self) -> Result<String, PgError> {
        let mut name = match self.next() {
            Tok::Ident(s) => s,
            _ => return Err(err(self.src)),
        };
        if name == "double" && Self::ident_is(self.peek(), "precision") {
            self.next();
            name = "double precision".to_string();
        } else if name == "character" && Self::ident_is(self.peek(), "varying") {
            self.next();
            name = "character varying".to_string();
        } else if name == "bit" && Self::ident_is(self.peek(), "varying") {
            self.next();
            name = "bit varying".to_string();
        } else if (name == "time" || name == "timestamp")
            && (Self::ident_is(self.peek(), "with") || Self::ident_is(self.peek(), "without"))
        {
            let with_tz = Self::ident_is(self.peek(), "with");
            self.next();
            if Self::ident_is(self.peek(), "time") {
                self.next();
            }
            if Self::ident_is(self.peek(), "zone") {
                self.next();
            }
            if with_tz {
                name = format!("{name} with time zone");
            }
        }

        if matches!(self.peek(), Tok::LParen) {
            let mut depth = 0;
            loop {
                match self.next() {
                    Tok::LParen => depth += 1,
                    Tok::RParen => {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                    Tok::Eof => return Err(err(self.src)),
                    _ => {}
                }
            }
        }

        if matches!(self.peek(), Tok::LBracket) {
            self.next();
            if !matches!(self.peek(), Tok::RBracket) {
                return Err(err(self.src));
            }
            self.next();
            name.push_str("[]");
        }
        Ok(name)
    }

    fn parse_predicate(&mut self, lhs: Expr, kw: &str, negated: bool) -> Result<Expr, PgError> {
        match kw {
            "between" => {

                let low = self.parse_expr(41)?;
                if !Self::ident_is(self.peek(), "and") {
                    return Err(err(self.src));
                }
                self.next();
                let high = self.parse_expr(41)?;
                Ok(if negated {
                    bin_expr(
                        BinOp::Or,
                        bin_expr(BinOp::Lt, lhs.clone(), low),
                        bin_expr(BinOp::Gt, lhs, high),
                    )
                } else {
                    bin_expr(
                        BinOp::And,
                        bin_expr(BinOp::GtEq, lhs.clone(), low),
                        bin_expr(BinOp::LtEq, lhs, high),
                    )
                })
            }
            "in" => {

                if !matches!(self.peek(), Tok::LParen) {
                    return Err(err(self.src));
                }
                self.next();
                if Self::ident_is(self.peek(), "select") {
                    let (q, next) = crate::stmt::parser::parse_query_at_depth(
                        self.toks, self.pos, self.src, self.depth,
                    )?;
                    self.pos = next;
                    if !matches!(self.next(), Tok::RParen) {
                        return Err(err(self.src));
                    }
                    return Ok(Expr::InSubquery {
                        expr: Box::new(lhs),
                        query: Box::new(q),
                        negated,
                    });
                }
                let mut items = vec![self.parse_expr(0)?];
                while matches!(self.peek(), Tok::Comma) {
                    self.next();
                    items.push(self.parse_expr(0)?);
                }
                if !matches!(self.peek(), Tok::RParen) {
                    return Err(err(self.src));
                }
                self.next();
                let (cmp, join) = if negated {
                    (BinOp::NotEq, BinOp::And)
                } else {
                    (BinOp::Eq, BinOp::Or)
                };
                let mut it = items.into_iter();
                let mut acc = bin_expr(cmp, lhs.clone(), it.next().unwrap());
                for item in it {
                    acc = bin_expr(join, acc, bin_expr(cmp, lhs.clone(), item));
                }
                Ok(acc)
            }
            "like" | "ilike" => {
                let pat = self.parse_expr(45)?;
                let f = Expr::Func {
                    name: kw.to_string(),
                    args: vec![lhs, pat],
                    distinct: false,
                    filter: None,
                    order_by: Vec::new(),
                };
                Ok(if negated {
                    Expr::Unary {
                        op: super::ast::UnOp::Not,
                        expr: Box::new(f),
                    }
                } else {
                    f
                })
            }
            _ => Err(err(self.src)),
        }
    }

    fn parse_window(&mut self, func: String, args: Vec<Expr>) -> Result<Expr, PgError> {

        if let Tok::Ident(name) = self.peek() {
            let name = name.clone();
            self.next();
            return Ok(Expr::Window {
                func,
                args,
                partition_by: Vec::new(),
                order_by: Vec::new(),
                frame: None,
                window_ref: Some(name),
            });
        }
        if !matches!(self.peek(), Tok::LParen) {
            return Err(err(self.src));
        }
        self.next();

        let mut window_ref = None;
        if let Tok::Ident(name) = self.peek() {
            if name != "partition" && name != "order" {
                window_ref = Some(name.clone());
                self.next();
            }
        }
        let mut partition_by = Vec::new();
        if Self::ident_is(self.peek(), "partition") {

            if window_ref.is_some() {
                return Err(err(self.src));
            }
            self.next();
            if !Self::ident_is(self.peek(), "by") {
                return Err(err(self.src));
            }
            self.next();
            partition_by.push(self.parse_expr(0)?);
            while matches!(self.peek(), Tok::Comma) {
                self.next();
                partition_by.push(self.parse_expr(0)?);
            }
        }
        let mut order_by = Vec::new();
        if Self::ident_is(self.peek(), "order") {
            self.next();
            if !Self::ident_is(self.peek(), "by") {
                return Err(err(self.src));
            }
            self.next();
            loop {
                let expr = self.parse_expr(0)?;
                let descending = if Self::ident_is(self.peek(), "desc") {
                    self.next();
                    true
                } else {
                    if Self::ident_is(self.peek(), "asc") {
                        self.next();
                    }
                    false
                };

                let nulls_first = if Self::ident_is(self.peek(), "nulls") {
                    self.next();
                    if Self::ident_is(self.peek(), "first") {
                        self.next();
                        Some(true)
                    } else if Self::ident_is(self.peek(), "last") {
                        self.next();
                        Some(false)
                    } else {
                        return Err(err(self.src));
                    }
                } else {
                    None
                };
                order_by.push(crate::stmt::ast::OrderKey {
                    expr,
                    descending,
                    nulls_first,
                    comp_oid: None,
                });
                if matches!(self.peek(), Tok::Comma) {
                    self.next();
                } else {
                    break;
                }
            }
        }

        let frame = if Self::ident_is(self.peek(), "rows")
            || Self::ident_is(self.peek(), "range")
            || Self::ident_is(self.peek(), "groups")
        {
            Some(self.parse_frame()?)
        } else {
            None
        };
        if !matches!(self.peek(), Tok::RParen) {
            return Err(err(self.src));
        }
        self.next();
        Ok(Expr::Window {
            func,
            args,
            partition_by,
            order_by,
            frame,
            window_ref,
        })
    }

    fn parse_order_by_keys(&mut self) -> Result<Vec<crate::stmt::ast::OrderKey>, PgError> {
        self.next();
        if !Self::ident_is(self.peek(), "by") {
            return Err(err(self.src));
        }
        self.next();
        let mut keys = Vec::new();
        loop {
            let expr = self.parse_expr(0)?;
            let descending = if Self::ident_is(self.peek(), "desc") {
                self.next();
                true
            } else {
                if Self::ident_is(self.peek(), "asc") {
                    self.next();
                }
                false
            };
            let nulls_first = if Self::ident_is(self.peek(), "nulls") {
                self.next();
                if Self::ident_is(self.peek(), "first") {
                    self.next();
                    Some(true)
                } else if Self::ident_is(self.peek(), "last") {
                    self.next();
                    Some(false)
                } else {
                    return Err(err(self.src));
                }
            } else {
                None
            };
            keys.push(crate::stmt::ast::OrderKey {
                expr,
                descending,
                nulls_first,
                comp_oid: None,
            });
            if matches!(self.peek(), Tok::Comma) {
                self.next();
            } else {
                break;
            }
        }
        Ok(keys)
    }

    fn parse_frame(&mut self) -> Result<super::ast::WindowFrame, PgError> {
        use super::ast::{FrameExclude, FrameMode, WindowFrame};
        let mode = if Self::ident_is(self.peek(), "rows") {
            FrameMode::Rows
        } else if Self::ident_is(self.peek(), "groups") {
            FrameMode::Groups
        } else {
            FrameMode::Range
        };
        self.next();

        let (start, end) = if Self::ident_is(self.peek(), "between") {
            self.next();
            let start = self.parse_frame_bound()?;
            if !Self::ident_is(self.peek(), "and") {
                return Err(err(self.src));
            }
            self.next();
            let end = self.parse_frame_bound()?;
            (start, end)
        } else {

            (
                self.parse_frame_bound()?,
                super::ast::FrameBound::CurrentRow,
            )
        };

        let exclude = if Self::ident_is(self.peek(), "exclude") {
            self.next();
            if Self::ident_is(self.peek(), "current") {
                self.next();
                if !Self::ident_is(self.peek(), "row") {
                    return Err(err(self.src));
                }
                self.next();
                FrameExclude::CurrentRow
            } else if Self::ident_is(self.peek(), "group") {
                self.next();
                FrameExclude::Group
            } else if Self::ident_is(self.peek(), "ties") {
                self.next();
                FrameExclude::Ties
            } else if Self::ident_is(self.peek(), "no") {
                self.next();
                if !Self::ident_is(self.peek(), "others") {
                    return Err(err(self.src));
                }
                self.next();
                FrameExclude::NoOthers
            } else {
                return Err(err(self.src));
            }
        } else {
            FrameExclude::NoOthers
        };

        Ok(WindowFrame {
            mode,
            start,
            end,
            exclude,
        })
    }

    fn parse_frame_bound(&mut self) -> Result<super::ast::FrameBound, PgError> {
        use super::ast::FrameBound;
        if Self::ident_is(self.peek(), "unbounded") {
            self.next();
            if Self::ident_is(self.peek(), "preceding") {
                self.next();
                return Ok(FrameBound::UnboundedPreceding);
            }
            if Self::ident_is(self.peek(), "following") {
                self.next();
                return Ok(FrameBound::UnboundedFollowing);
            }
            return Err(err(self.src));
        }
        if Self::ident_is(self.peek(), "current") {
            self.next();
            if !Self::ident_is(self.peek(), "row") {
                return Err(err(self.src));
            }
            self.next();
            return Ok(FrameBound::CurrentRow);
        }

        let n = match self.next() {
            Tok::Int(n) if n >= 0 => n,
            _ => return Err(err(self.src)),
        };
        if Self::ident_is(self.peek(), "preceding") {
            self.next();
            Ok(FrameBound::Preceding(n))
        } else if Self::ident_is(self.peek(), "following") {
            self.next();
            Ok(FrameBound::Following(n))
        } else {
            Err(err(self.src))
        }
    }

    fn parse_case(&mut self) -> Result<Expr, PgError> {
        let operand = if !Self::ident_is(self.peek(), "when") {
            Some(Box::new(self.parse_expr(0)?))
        } else {
            None
        };
        let mut whens = Vec::new();
        while Self::ident_is(self.peek(), "when") {
            self.next();
            let cond = self.parse_expr(0)?;
            if !Self::ident_is(self.peek(), "then") {
                return Err(err(self.src));
            }
            self.next();
            let res = self.parse_expr(0)?;
            whens.push((cond, res));
        }
        if whens.is_empty() {
            return Err(err(self.src));
        }
        let else_ = if Self::ident_is(self.peek(), "else") {
            self.next();
            Some(Box::new(self.parse_expr(0)?))
        } else {
            None
        };
        if !Self::ident_is(self.peek(), "end") {
            return Err(err(self.src));
        }
        self.next();
        Ok(Expr::Case {
            operand,
            whens,
            else_,
        })
    }

    fn parse_overlay(&mut self) -> Result<Expr, PgError> {
        self.next();
        let string = self.parse_expr(0)?;
        if !Self::ident_is(self.peek(), "placing") {
            return Err(err(self.src));
        }
        self.next();
        let sub = self.parse_expr(0)?;
        if !Self::ident_is(self.peek(), "from") {
            return Err(err(self.src));
        }
        self.next();
        let start = self.parse_expr(0)?;
        let mut args = vec![string, sub, start];
        if Self::ident_is(self.peek(), "for") {
            self.next();
            args.push(self.parse_expr(0)?);
        }
        if !matches!(self.next(), Tok::RParen) {
            return Err(err(self.src));
        }
        Ok(Expr::Func {
            name: "overlay".to_string(),
            args,
            distinct: false,
            filter: None,
            order_by: Vec::new(),
        })
    }

    fn parse_primary(&mut self) -> Result<Expr, PgError> {
        match self.next() {
            Tok::Int(n) => Ok(Expr::Int(n)),
            Tok::Float(f) => Ok(Expr::Float(f)),
            Tok::Str(s) => Ok(Expr::Str(s)),
            Tok::LParen => {

                if Self::ident_is(self.peek(), "select") {
                    let (q, next) = crate::stmt::parser::parse_query_at_depth(
                        self.toks, self.pos, self.src, self.depth,
                    )?;
                    self.pos = next;
                    match self.next() {
                        Tok::RParen => Ok(Expr::ScalarSubquery(Box::new(q))),
                        _ => Err(err(self.src)),
                    }
                } else {
                    let e = self.parse_expr(0)?;
                    match self.next() {
                        Tok::RParen => Ok(e),
                        _ => Err(err(self.src)),
                    }
                }
            }
            Tok::Ident(name) => match name.as_str() {
                "true" => Ok(Expr::Bool(true)),
                "false" => Ok(Expr::Bool(false)),
                "null" => Ok(Expr::Null),
                "case" => self.parse_case(),

                "array" if matches!(self.peek(), Tok::LBracket) => {
                    self.next();
                    let mut elems = Vec::new();
                    if !matches!(self.peek(), Tok::RBracket) {
                        loop {
                            elems.push(self.parse_expr(0)?);
                            match self.peek() {
                                Tok::Comma => {
                                    self.next();
                                }
                                _ => break,
                            }
                        }
                    }
                    match self.next() {
                        Tok::RBracket => Ok(Expr::Array(elems)),
                        _ => Err(err(self.src)),
                    }
                }

                "overlay" if matches!(self.peek(), Tok::LParen) => self.parse_overlay(),

                "row" if matches!(self.peek(), Tok::LParen) => {
                    self.next();
                    let mut elems = Vec::new();
                    if !matches!(self.peek(), Tok::RParen) {
                        loop {
                            elems.push(self.parse_expr(0)?);
                            match self.peek() {
                                Tok::Comma => {
                                    self.next();
                                }
                                _ => break,
                            }
                        }
                    }
                    match self.next() {
                        Tok::RParen => Ok(Expr::Row(elems)),
                        _ => Err(err(self.src)),
                    }
                }
                "exists" => {

                    if !matches!(self.peek(), Tok::LParen) {
                        return Err(err(self.src));
                    }
                    self.next();
                    if !Self::ident_is(self.peek(), "select") {
                        return Err(err(self.src));
                    }
                    let (q, next) = crate::stmt::parser::parse_query_at_depth(
                        self.toks, self.pos, self.src, self.depth,
                    )?;
                    self.pos = next;
                    if !matches!(self.next(), Tok::RParen) {
                        return Err(err(self.src));
                    }
                    Ok(Expr::Exists {
                        query: Box::new(q),
                        negated: false,
                    })
                }
                _ => {

                    if matches!(self.peek(), Tok::LParen) {
                        self.next();
                        let mut args = Vec::new();

                        let distinct = Self::ident_is(self.peek(), "distinct");
                        if distinct {
                            self.next();
                        }

                        if matches!(self.peek(), Tok::Star) {
                            self.next();
                        } else if !matches!(self.peek(), Tok::RParen) {
                            loop {
                                args.push(self.parse_expr(0)?);
                                match self.peek() {
                                    Tok::Comma => {
                                        self.next();
                                    }
                                    _ => break,
                                }
                            }
                        }

                        let mut order_by = if Self::ident_is(self.peek(), "order") {
                            self.parse_order_by_keys()?
                        } else {
                            Vec::new()
                        };
                        match self.next() {
                            Tok::RParen => {

                                if Self::ident_is(self.peek(), "within") {
                                    self.next();
                                    if !Self::ident_is(self.peek(), "group") {
                                        return Err(err(self.src));
                                    }
                                    self.next();
                                    if !matches!(self.next(), Tok::LParen) {
                                        return Err(err(self.src));
                                    }
                                    if !Self::ident_is(self.peek(), "order") {
                                        return Err(err(self.src));
                                    }
                                    order_by = self.parse_order_by_keys()?;
                                    if !matches!(self.next(), Tok::RParen) {
                                        return Err(err(self.src));
                                    }
                                }

                                let filter = if Self::ident_is(self.peek(), "filter") {
                                    self.next();
                                    if !matches!(self.next(), Tok::LParen) {
                                        return Err(err(self.src));
                                    }
                                    if !Self::ident_is(self.peek(), "where") {
                                        return Err(err(self.src));
                                    }
                                    self.next();
                                    let cond = self.parse_expr(0)?;
                                    if !matches!(self.next(), Tok::RParen) {
                                        return Err(err(self.src));
                                    }
                                    Some(Box::new(cond))
                                } else {
                                    None
                                };

                                if Self::ident_is(self.peek(), "over") {
                                    self.next();
                                    self.parse_window(name, args)
                                } else {
                                    Ok(Expr::Func {
                                        name,
                                        args,
                                        distinct,
                                        filter,
                                        order_by,
                                    })
                                }
                            }
                            _ => Err(err(self.src)),
                        }
                    } else if matches!(self.peek(), Tok::Dot) {

                        self.next();
                        match self.next() {
                            Tok::Ident(col) => Ok(Expr::Column(format!("{name}.{col}"))),
                            _ => Err(err(self.src)),
                        }
                    } else {
                        Ok(Expr::Column(name))
                    }
                }
            },
            _ => Err(err(self.src)),
        }
    }
}

pub fn parse(src: &str) -> Result<Expr, PgError> {
    let toks = super::lexer::lex(src)?;
    validate_token_nesting(&toks, src)?;
    let mut p = Parser {
        toks: &toks,
        pos: 0,
        src,
        depth: 0,
    };
    let e = p.parse_expr(0)?;
    match p.peek() {
        Tok::Eof => Ok(e),
        _ => Err(err(src)),
    }
}

pub fn parse_expr_at(toks: &[Tok], pos: usize, src: &str) -> Result<(Expr, usize), PgError> {
    parse_expr_at_depth(toks, pos, src, 0)
}

pub(crate) fn parse_expr_at_depth(
    toks: &[Tok],
    pos: usize,
    src: &str,
    depth: usize,
) -> Result<(Expr, usize), PgError> {
    validate_token_nesting(toks.get(pos..).unwrap_or(&[]), src)?;
    let mut p = Parser {
        toks,
        pos,
        src,
        depth,
    };
    let e = p.parse_expr(0)?;
    Ok((e, p.pos))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> Expr {
        parse(s).expect("parse ok")
    }

    fn bin(op: BinOp, l: Expr, r: Expr) -> Expr {
        Expr::Binary {
            op,
            left: Box::new(l),
            right: Box::new(r),
        }
    }

    #[test]
    fn precedence_mul_over_add() {

        assert_eq!(
            p("1 + 2 * 3"),
            bin(
                BinOp::Add,
                Expr::Int(1),
                bin(BinOp::Mul, Expr::Int(2), Expr::Int(3))
            )
        );
    }

    #[test]
    fn left_assoc_subtraction() {

        assert_eq!(
            p("10 - 2 - 3"),
            bin(
                BinOp::Sub,
                bin(BinOp::Sub, Expr::Int(10), Expr::Int(2)),
                Expr::Int(3)
            )
        );
    }

    #[test]
    fn pow_is_left_assoc_like_pg() {

        assert_eq!(
            p("2 ^ 3 ^ 2"),
            bin(
                BinOp::Pow,
                bin(BinOp::Pow, Expr::Int(2), Expr::Int(3)),
                Expr::Int(2)
            )
        );
    }

    #[test]
    fn parens_override() {
        assert_eq!(
            p("(1 + 2) * 3"),
            bin(
                BinOp::Mul,
                bin(BinOp::Add, Expr::Int(1), Expr::Int(2)),
                Expr::Int(3)
            )
        );
    }

    #[test]
    fn unary_minus_binds_above_pow() {

        assert_eq!(
            p("-2 ^ 2"),
            bin(
                BinOp::Pow,
                Expr::Unary {
                    op: UnOp::Neg,
                    expr: Box::new(Expr::Int(2))
                },
                Expr::Int(2)
            )
        );
    }

    #[test]
    fn not_binds_below_comparison() {

        assert_eq!(
            p("not a = b"),
            Expr::Unary {
                op: UnOp::Not,
                expr: Box::new(bin(
                    BinOp::Eq,
                    Expr::Column("a".into()),
                    Expr::Column("b".into())
                ))
            }
        );
    }

    #[test]
    fn and_binds_tighter_than_or() {

        assert_eq!(
            p("a or b and c"),
            bin(
                BinOp::Or,
                Expr::Column("a".into()),
                bin(
                    BinOp::And,
                    Expr::Column("b".into()),
                    Expr::Column("c".into())
                )
            )
        );
    }

    #[test]
    fn cast_binds_tightest() {

        assert_eq!(
            p("-2::int"),
            Expr::Unary {
                op: UnOp::Neg,
                expr: Box::new(Expr::Cast {
                    expr: Box::new(Expr::Int(2)),
                    type_name: "int".into()
                })
            }
        );
    }

    #[test]
    fn function_call() {
        assert_eq!(
            p("abs(-5)"),
            Expr::Func {
                name: "abs".into(),
                args: vec![Expr::Unary {
                    op: UnOp::Neg,
                    expr: Box::new(Expr::Int(5))
                }],
                distinct: false,
                filter: None,
                order_by: Vec::new(),
            }
        );
        assert_eq!(
            p("f(1, 2, 3)"),
            Expr::Func {
                name: "f".into(),
                args: vec![Expr::Int(1), Expr::Int(2), Expr::Int(3)],
                distinct: false,
                filter: None,
                order_by: Vec::new()
            }
        );
        assert_eq!(
            p("now()"),
            Expr::Func {
                name: "now".into(),
                args: vec![],
                distinct: false,
                filter: None,
                order_by: Vec::new()
            }
        );
    }

    #[test]
    fn aggregate_distinct_and_filter() {
        assert_eq!(
            p("count(DISTINCT x)"),
            Expr::Func {
                name: "count".into(),
                args: vec![Expr::Column("x".into())],
                distinct: true,
                filter: None,
                order_by: Vec::new()
            }
        );
        match p("count(*) FILTER (WHERE x > 0)") {
            Expr::Func {
                name,
                args,
                distinct,
                filter,
                ..
            } => {
                assert_eq!(name, "count");
                assert!(args.is_empty());
                assert!(!distinct);
                assert!(matches!(
                    filter.as_deref(),
                    Some(Expr::Binary { op: BinOp::Gt, .. })
                ));
            }
            other => panic!("expected Func, got {other:?}"),
        }
    }

    #[test]
    fn aggregate_internal_order_by() {
        match p("string_agg(name, ',' ORDER BY amt DESC)") {
            Expr::Func {
                name,
                args,
                order_by,
                ..
            } => {
                assert_eq!(name, "string_agg");
                assert_eq!(args.len(), 2);
                assert_eq!(order_by.len(), 1);
                assert!(order_by[0].descending);
                assert_eq!(order_by[0].expr, Expr::Column("amt".into()));
            }
            other => panic!("expected Func, got {other:?}"),
        }
    }

    #[test]
    fn within_group_ordered_set() {
        match p("percentile_cont(0.5) WITHIN GROUP (ORDER BY n)") {
            Expr::Func {
                name,
                args,
                order_by,
                ..
            } => {
                assert_eq!(name, "percentile_cont");
                assert_eq!(args.len(), 1);
                assert_eq!(order_by.len(), 1);
                assert!(!order_by[0].descending);
                assert_eq!(order_by[0].expr, Expr::Column("n".into()));
            }
            other => panic!("expected Func, got {other:?}"),
        }
        match p("mode() WITHIN GROUP (ORDER BY c)") {
            Expr::Func {
                name,
                args,
                order_by,
                ..
            } => {
                assert_eq!(name, "mode");
                assert!(args.is_empty());
                assert_eq!(order_by.len(), 1);
            }
            other => panic!("expected Func, got {other:?}"),
        }
    }

    #[test]
    fn general_operator_parses_by_spelling() {

        assert_eq!(
            p("a || b"),
            Expr::GenBinary {
                op: "||".into(),
                left: Box::new(Expr::Column("a".into())),
                right: Box::new(Expr::Column("b".into())),
            }
        );
        assert!(matches!(p("@ x"), Expr::GenUnary { .. }));
    }

    #[test]
    fn trailing_garbage_errors() {
        assert!(parse("1 + 2 3").is_err());
        assert!(parse("(1 + 2").is_err());
    }

    #[test]
    fn rejects_excessive_expression_nesting_without_stack_overflow() {
        let parens = format!("{}1{}", "(".repeat(1100), ")".repeat(1100));
        assert!(parse(&parens).is_err());

        let not_chain = format!("{}true", "not ".repeat(1100));
        assert!(parse(&not_chain).is_err());
    }
}
