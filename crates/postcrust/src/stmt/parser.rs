
use super::ast::{
    FromItem, JoinKind, LockClause, LockStrength, LockWait, NamedWindow, OrderKey, SelectItem,
    SelectStmt, Stmt,
};
use crate::expr::ast::Expr;
use crate::expr::lexer::{lex, Tok};
use crate::expr::parser::parse_expr_at_depth;
use crate::types::PgError;

const CLAUSE_KW: &[&str] = &[
    "from",
    "where",
    "order",
    "group",
    "having",
    "limit",
    "offset",
    "union",
    "intersect",
    "except",
    "fetch",
    "for",
    "window",
    "as",
];

const JOIN_KW: &[&str] = &[
    "join", "inner", "left", "right", "full", "cross", "outer", "on", "natural", "using",
];

struct P<'a> {
    toks: &'a [Tok],
    pos: usize,
    src: &'a str,
    query_depth: usize,
}

fn err(src: &str) -> PgError {
    PgError::InvalidInputSyntax {
        typ: "query",
        input: src.to_string(),
    }
}

fn validate_query_token_nesting(toks: &[Tok], src: &str) -> Result<(), PgError> {
    let mut paren_depth = 0usize;
    for tok in toks {
        match tok {
            Tok::LParen => {
                paren_depth += 1;
                if paren_depth > P::MAX_QUERY_DEPTH {
                    return Err(err(src));
                }
            }
            Tok::RParen => paren_depth = paren_depth.saturating_sub(1),
            _ => {}
        }
    }
    Ok(())
}

fn rollup_sets(cols: &[Expr]) -> Vec<Vec<Expr>> {
    (0..=cols.len()).rev().map(|k| cols[..k].to_vec()).collect()
}

fn cube_sets(cols: &[Expr]) -> Vec<Vec<Expr>> {
    let n = cols.len();
    (0..(1usize << n))
        .map(|t| {
            (0..n)
                .filter(|&i| (t >> (n - 1 - i)) & 1 == 0)
                .map(|i| cols[i].clone())
                .collect()
        })
        .collect()
}

fn cross_product(elements: &[Vec<Vec<Expr>>]) -> Vec<Vec<Expr>> {
    let mut acc: Vec<Vec<Expr>> = vec![Vec::new()];
    for elem in elements {
        let mut next = Vec::with_capacity(acc.len() * elem.len());
        for prefix in &acc {
            for set in elem {
                let mut combined = prefix.clone();
                combined.extend(set.iter().cloned());
                next.push(combined);
            }
        }
        acc = next;
    }
    acc
}

impl<'a> P<'a> {
    const MAX_QUERY_DEPTH: usize = 256;

    fn peek(&self) -> &Tok {
        &self.toks[self.pos]
    }

    fn at_kw(&self, kw: &str) -> bool {
        matches!(self.peek(), Tok::Ident(s) if s == kw)
    }

    fn eat_kw(&mut self, kw: &str) -> bool {
        if self.at_kw(kw) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn expect_kw(&mut self, kw: &str) -> Result<(), PgError> {
        if self.eat_kw(kw) {
            Ok(())
        } else {
            Err(err(self.src))
        }
    }

    fn expr(&mut self) -> Result<crate::expr::ast::Expr, PgError> {
        let (e, next) = parse_expr_at_depth(&self.toks, self.pos, self.src, self.query_depth)?;
        self.pos = next;
        Ok(e)
    }

    fn parse_select(&mut self) -> Result<SelectStmt, PgError> {
        if self.query_depth >= Self::MAX_QUERY_DEPTH {
            return Err(err(self.src));
        }
        self.query_depth += 1;
        self.expect_kw("select")?;

        let (distinct, distinct_on) = if self.eat_kw("distinct") {
            if self.eat_kw("on") {
                if !matches!(self.peek(), Tok::LParen) {
                    return Err(err(self.src));
                }
                self.pos += 1;
                let mut keys = vec![self.expr()?];
                while matches!(self.peek(), Tok::Comma) {
                    self.pos += 1;
                    keys.push(self.expr()?);
                }
                if !matches!(self.peek(), Tok::RParen) {
                    return Err(err(self.src));
                }
                self.pos += 1;
                (false, keys)
            } else {
                (true, Vec::new())
            }
        } else {
            (false, Vec::new())
        };

        let projection = self.parse_select_list()?;

        let from = if self.eat_kw("from") {
            Some(self.parse_from_item()?)
        } else {
            None
        };

        let filter = if self.eat_kw("where") {
            Some(self.expr()?)
        } else {
            None
        };

        let (group_by, grouping_sets) = if self.eat_kw("group") {
            self.expect_kw("by")?;
            self.parse_group_by()?
        } else {
            (Vec::new(), Vec::new())
        };

        let having = if self.eat_kw("having") {
            Some(self.expr()?)
        } else {
            None
        };

        let windows = if self.eat_kw("window") {
            self.parse_window_defs()?
        } else {
            Vec::new()
        };

        let order_by = if self.eat_kw("order") {
            self.expect_kw("by")?;
            self.parse_order_keys()?
        } else {
            Vec::new()
        };

        let (mut limit, mut offset) = (None, 0i64);
        loop {
            if self.at_kw("limit") && limit.is_none() {
                self.pos += 1;
                if self.eat_kw("all") {
                    limit = None;
                } else {
                    limit = Some(self.expect_int()?);
                }
            } else if self.at_kw("offset") && offset == 0 {
                self.pos += 1;
                offset = self.expect_int()?;
            } else {
                break;
            }
        }

        let mut locking = Vec::new();
        while self.at_kw("for") {
            locking.push(self.parse_lock_clause()?);
        }

        let stmt = SelectStmt {
            distinct,
            distinct_on,
            projection,
            from,
            filter,
            group_by,
            grouping_sets,
            having,
            order_by,
            limit,
            offset,
            windows,
            tail: Vec::new(),
            locking,
        };
        self.query_depth = self.query_depth.saturating_sub(1);
        Ok(stmt)
    }

    fn parse_lock_clause(&mut self) -> Result<LockClause, PgError> {
        self.expect_kw("for")?;
        let strength = if self.eat_kw("update") {
            LockStrength::Update
        } else if self.eat_kw("no") {
            self.expect_kw("key")?;
            self.expect_kw("update")?;
            LockStrength::NoKeyUpdate
        } else if self.eat_kw("share") {
            LockStrength::Share
        } else if self.eat_kw("key") {
            self.expect_kw("share")?;
            LockStrength::KeyShare
        } else {
            return Err(err(self.src));
        };

        let mut of = Vec::new();
        if self.eat_kw("of") {
            loop {
                match self.peek() {
                    Tok::Ident(n) if !CLAUSE_KW.contains(&n.as_str()) => {
                        let n = n.clone();
                        self.pos += 1;
                        of.push(n);
                    }
                    _ => return Err(err(self.src)),
                }
                if matches!(self.peek(), Tok::Comma) {
                    self.pos += 1;
                } else {
                    break;
                }
            }
        }

        let wait = if self.eat_kw("nowait") {
            LockWait::NoWait
        } else if self.eat_kw("skip") {
            self.expect_kw("locked")?;
            LockWait::SkipLocked
        } else {
            LockWait::Wait
        };
        Ok(LockClause { strength, of, wait })
    }

    fn parse_group_by(&mut self) -> Result<(Vec<Expr>, Vec<Vec<Expr>>), PgError> {
        let mut plain: Vec<Expr> = Vec::new();
        let mut elements: Vec<Vec<Vec<Expr>>> = Vec::new();
        let mut advanced = false;
        loop {
            if self.at_kw("rollup") || self.at_kw("cube") {
                advanced = true;
                let is_cube = self.at_kw("cube");
                self.pos += 1;
                let cols = self.parse_paren_expr_list()?;
                elements.push(if is_cube {
                    cube_sets(&cols)
                } else {
                    rollup_sets(&cols)
                });
            } else if self.at_kw("grouping")
                && matches!(self.toks.get(self.pos + 1), Some(Tok::Ident(s)) if s == "sets")
            {
                advanced = true;
                self.pos += 2;
                elements.push(self.parse_grouping_set_specs()?);
            } else {
                let e = self.expr()?;
                plain.push(e.clone());
                elements.push(vec![vec![e]]);
            }
            if matches!(self.peek(), Tok::Comma) {
                self.pos += 1;
            } else {
                break;
            }
        }
        if advanced {
            Ok((Vec::new(), cross_product(&elements)))
        } else {
            Ok((plain, Vec::new()))
        }
    }

    fn parse_paren_expr_list(&mut self) -> Result<Vec<Expr>, PgError> {
        if !matches!(self.peek(), Tok::LParen) {
            return Err(err(self.src));
        }
        self.pos += 1;
        let mut cols = vec![self.expr()?];
        while matches!(self.peek(), Tok::Comma) {
            self.pos += 1;
            cols.push(self.expr()?);
        }
        if !matches!(self.peek(), Tok::RParen) {
            return Err(err(self.src));
        }
        self.pos += 1;
        Ok(cols)
    }

    fn parse_grouping_set_specs(&mut self) -> Result<Vec<Vec<Expr>>, PgError> {
        if !matches!(self.peek(), Tok::LParen) {
            return Err(err(self.src));
        }
        self.pos += 1;
        let mut sets: Vec<Vec<Expr>> = Vec::new();
        loop {
            if matches!(self.peek(), Tok::LParen) {
                self.pos += 1;
                let mut cols: Vec<Expr> = Vec::new();
                if !matches!(self.peek(), Tok::RParen) {
                    cols.push(self.expr()?);
                    while matches!(self.peek(), Tok::Comma) {
                        self.pos += 1;
                        cols.push(self.expr()?);
                    }
                }
                if !matches!(self.peek(), Tok::RParen) {
                    return Err(err(self.src));
                }
                self.pos += 1;
                sets.push(cols);
            } else if self.at_kw("rollup") || self.at_kw("cube") || self.at_kw("grouping") {

                return Err(err(self.src));
            } else {
                sets.push(vec![self.expr()?]);
            }
            if matches!(self.peek(), Tok::Comma) {
                self.pos += 1;
            } else {
                break;
            }
        }
        if !matches!(self.peek(), Tok::RParen) {
            return Err(err(self.src));
        }
        self.pos += 1;
        Ok(sets)
    }

    fn parse_window_defs(&mut self) -> Result<Vec<NamedWindow>, PgError> {
        let mut defs = Vec::new();
        loop {
            let name = match self.peek() {
                Tok::Ident(n) if !CLAUSE_KW.contains(&n.as_str()) => {
                    let n = n.clone();
                    self.pos += 1;
                    n
                }
                _ => return Err(err(self.src)),
            };
            self.expect_kw("as")?;
            if !matches!(self.peek(), Tok::LParen) {
                return Err(err(self.src));
            }
            self.pos += 1;
            let mut partition_by = Vec::new();
            if self.eat_kw("partition") {
                self.expect_kw("by")?;
                partition_by.push(self.expr()?);
                while matches!(self.peek(), Tok::Comma) {
                    self.pos += 1;
                    partition_by.push(self.expr()?);
                }
            }
            let order_by = if self.eat_kw("order") {
                self.expect_kw("by")?;
                self.parse_order_keys()?
            } else {
                Vec::new()
            };
            if !matches!(self.peek(), Tok::RParen) {
                return Err(err(self.src));
            }
            self.pos += 1;
            defs.push(NamedWindow {
                name,
                partition_by,
                order_by,
            });
            if matches!(self.peek(), Tok::Comma) {
                self.pos += 1;
            } else {
                break;
            }
        }
        Ok(defs)
    }

    fn parse_from_item(&mut self) -> Result<FromItem, PgError> {
        let mut left = self.parse_table_primary()?;
        loop {
            if matches!(self.peek(), Tok::Comma) {
                self.pos += 1;
                let right = self.parse_table_primary()?;
                left = FromItem::Join {
                    left: Box::new(left),
                    right: Box::new(right),
                    kind: JoinKind::Cross,
                    on: None,
                    using: Vec::new(),
                    natural: false,
                };
            } else if let Some((kind, natural)) = self.parse_join_kw()? {
                let right = self.parse_table_primary()?;

                let (on, using) = if natural {
                    (None, Vec::new())
                } else if self.eat_kw("on") {
                    (Some(self.expr()?), Vec::new())
                } else if self.eat_kw("using") {
                    (None, self.parse_using_cols()?)
                } else {
                    (None, Vec::new())
                };
                left = FromItem::Join {
                    left: Box::new(left),
                    right: Box::new(right),
                    kind,
                    on,
                    using,
                    natural,
                };
            } else {
                break;
            }
        }
        Ok(left)
    }

    fn parse_join_kw(&mut self) -> Result<Option<(JoinKind, bool)>, PgError> {
        let natural = self.eat_kw("natural");

        if !natural && self.eat_kw("cross") {
            self.expect_kw("join")?;
            return Ok(Some((JoinKind::Cross, false)));
        }
        if self.eat_kw("inner") {
            self.expect_kw("join")?;
            return Ok(Some((JoinKind::Inner, natural)));
        }
        if self.eat_kw("left") {
            self.eat_kw("outer");
            self.expect_kw("join")?;
            return Ok(Some((JoinKind::Left, natural)));
        }
        if self.eat_kw("right") {
            self.eat_kw("outer");
            self.expect_kw("join")?;
            return Ok(Some((JoinKind::Right, natural)));
        }
        if self.eat_kw("full") {
            self.eat_kw("outer");
            self.expect_kw("join")?;
            return Ok(Some((JoinKind::Full, natural)));
        }

        if self.eat_kw("join") {
            return Ok(Some((JoinKind::Inner, natural)));
        }

        if natural {
            return Err(err(self.src));
        }
        Ok(None)
    }

    fn parse_using_cols(&mut self) -> Result<Vec<String>, PgError> {
        if !matches!(self.peek(), Tok::LParen) {
            return Err(err(self.src));
        }
        self.pos += 1;
        let mut cols = Vec::new();
        loop {
            match self.peek() {
                Tok::Ident(n) => {
                    let n = n.clone();
                    self.pos += 1;
                    cols.push(n);
                }
                _ => return Err(err(self.src)),
            }
            if matches!(self.peek(), Tok::Comma) {
                self.pos += 1;
            } else {
                break;
            }
        }
        if !matches!(self.peek(), Tok::RParen) {
            return Err(err(self.src));
        }
        self.pos += 1;
        if cols.is_empty() {
            return Err(err(self.src));
        }
        Ok(cols)
    }

    fn parse_table_primary(&mut self) -> Result<FromItem, PgError> {

        let lateral = self.eat_kw("lateral");
        if matches!(self.peek(), Tok::LParen) {
            self.pos += 1;

            if self.at_kw("values") {
                return self.parse_values_from(lateral);
            }
            if !self.at_kw("select") {
                return Err(err(self.src));
            }

            let (q, next) = parse_query_at_depth(self.toks, self.pos, self.src, self.query_depth)?;
            self.pos = next;
            if !matches!(self.peek(), Tok::RParen) {
                return Err(err(self.src));
            }
            self.pos += 1;
            let alias = self.parse_required_alias()?;
            return Ok(FromItem::Subquery {
                query: Box::new(q),
                alias,
                lateral,
            });
        }
        let name = match self.peek() {
            Tok::Ident(n) if !CLAUSE_KW.contains(&n.as_str()) && !JOIN_KW.contains(&n.as_str()) => {
                let n = n.clone();
                self.pos += 1;
                n
            }
            _ => return Err(err(self.src)),
        };

        let name = if matches!(self.peek(), Tok::Dot) {
            self.pos += 1;
            match self.peek() {
                Tok::Ident(t) => {
                    let t = t.clone();
                    self.pos += 1;
                    format!("{name}.{t}")
                }
                _ => return Err(err(self.src)),
            }
        } else {
            name
        };

        if matches!(self.peek(), Tok::LParen) {
            self.pos += 1;
            let mut args = Vec::new();
            if !matches!(self.peek(), Tok::RParen) {
                loop {
                    args.push(self.expr()?);
                    if matches!(self.peek(), Tok::Comma) {
                        self.pos += 1;
                    } else {
                        break;
                    }
                }
            }
            if !matches!(self.peek(), Tok::RParen) {
                return Err(err(self.src));
            }
            self.pos += 1;
            let alias = self.parse_alias()?;
            return Ok(FromItem::Function {
                name,
                args,
                alias,
                lateral,
            });
        }

        if lateral {
            return Err(err(self.src));
        }
        let alias = if self.eat_kw("as") {
            match self.peek() {
                Tok::Ident(a) => {
                    let a = a.clone();
                    self.pos += 1;
                    Some(a)
                }
                _ => return Err(err(self.src)),
            }
        } else if let Tok::Ident(a) = self.peek() {
            if !CLAUSE_KW.contains(&a.as_str()) && !JOIN_KW.contains(&a.as_str()) {
                let a = a.clone();
                self.pos += 1;
                Some(a)
            } else {
                None
            }
        } else {
            None
        };
        Ok(FromItem::Table { name, alias })
    }

    fn parse_values_from(&mut self, lateral: bool) -> Result<FromItem, PgError> {
        self.eat_kw("values");
        let mut rows: Vec<Vec<Expr>> = Vec::new();
        loop {
            if !matches!(self.peek(), Tok::LParen) {
                return Err(err(self.src));
            }
            self.pos += 1;
            let mut exprs = Vec::new();
            loop {
                exprs.push(self.expr()?);
                if matches!(self.peek(), Tok::Comma) {
                    self.pos += 1;
                } else {
                    break;
                }
            }
            if !matches!(self.peek(), Tok::RParen) {
                return Err(err(self.src));
            }
            self.pos += 1;
            rows.push(exprs);
            if matches!(self.peek(), Tok::Comma) {
                self.pos += 1;
            } else {
                break;
            }
        }

        if !matches!(self.peek(), Tok::RParen) {
            return Err(err(self.src));
        }
        self.pos += 1;

        self.eat_kw("as");
        let alias = match self.peek() {
            Tok::Ident(a) if !CLAUSE_KW.contains(&a.as_str()) && !JOIN_KW.contains(&a.as_str()) => {
                let a = a.clone();
                self.pos += 1;
                a
            }
            _ => return Err(err(self.src)),
        };
        let mut col_aliases: Vec<String> = Vec::new();
        if matches!(self.peek(), Tok::LParen) {
            self.pos += 1;
            loop {
                match self.peek() {
                    Tok::Ident(c) => {
                        col_aliases.push(c.clone());
                        self.pos += 1;
                    }
                    _ => return Err(err(self.src)),
                }
                match self.peek() {
                    Tok::Comma => self.pos += 1,
                    Tok::RParen => {
                        self.pos += 1;
                        break;
                    }
                    _ => return Err(err(self.src)),
                }
            }
        }
        let query = values_to_select(rows, &col_aliases);
        Ok(FromItem::Subquery {
            query: Box::new(query),
            alias,
            lateral,
        })
    }

    fn parse_required_alias(&mut self) -> Result<String, PgError> {
        self.eat_kw("as");
        match self.peek() {
            Tok::Ident(a) if !CLAUSE_KW.contains(&a.as_str()) && !JOIN_KW.contains(&a.as_str()) => {
                let a = a.clone();
                self.pos += 1;
                Ok(a)
            }
            _ => Err(err(self.src)),
        }
    }

    fn parse_select_list(&mut self) -> Result<Vec<SelectItem>, PgError> {
        let mut items = Vec::new();
        loop {
            if matches!(self.peek(), Tok::Star) {

                self.pos += 1;
                items.push(SelectItem::Star);
            } else {
                let expr = self.expr()?;
                let alias = self.parse_alias()?;
                items.push(SelectItem::Expr { expr, alias });
            }
            if matches!(self.peek(), Tok::Comma) {
                self.pos += 1;
            } else {
                break;
            }
        }
        Ok(items)
    }

    fn parse_alias(&mut self) -> Result<Option<String>, PgError> {
        if self.eat_kw("as") {
            return match self.peek() {
                Tok::Ident(a) => {
                    let a = a.clone();
                    self.pos += 1;
                    Ok(Some(a))
                }
                _ => Err(err(self.src)),
            };
        }
        if let Tok::Ident(a) = self.peek() {
            if !CLAUSE_KW.contains(&a.as_str()) {
                let a = a.clone();
                self.pos += 1;
                return Ok(Some(a));
            }
        }
        Ok(None)
    }

    fn parse_order_keys(&mut self) -> Result<Vec<OrderKey>, PgError> {
        let mut keys = Vec::new();
        loop {
            let expr = self.expr()?;
            let descending = if self.eat_kw("desc") {
                true
            } else {
                self.eat_kw("asc");
                false
            };

            let nulls_first = if self.eat_kw("nulls") {
                if self.eat_kw("first") {
                    Some(true)
                } else if self.eat_kw("last") {
                    Some(false)
                } else {
                    return Err(err(self.src));
                }
            } else {
                None
            };
            keys.push(OrderKey {
                expr,
                descending,
                nulls_first,
                comp_oid: None,
            });
            if matches!(self.peek(), Tok::Comma) {
                self.pos += 1;
            } else {
                break;
            }
        }
        Ok(keys)
    }

    fn expect_int(&mut self) -> Result<i64, PgError> {
        match self.peek() {
            Tok::Int(n) => {
                let n = *n;
                self.pos += 1;
                Ok(n)
            }
            _ => Err(err(self.src)),
        }
    }
}

pub fn parse(sql: &str) -> Result<Stmt, PgError> {
    let toks = lex(sql)?;
    validate_query_token_nesting(&toks, sql)?;
    let mut p = P {
        toks: &toks,
        pos: 0,
        src: sql,
        query_depth: 0,
    };
    if !p.at_kw("select") {
        return Err(err(sql));
    }
    let s = p.parse_select()?;
    match p.peek() {
        Tok::Eof => Ok(Stmt::Select(s)),
        _ => Err(err(sql)),
    }
}

fn bare_select(projection: Vec<SelectItem>) -> SelectStmt {
    SelectStmt {
        distinct: false,
        distinct_on: Vec::new(),
        projection,
        from: None,
        filter: None,
        group_by: Vec::new(),
        grouping_sets: Vec::new(),
        having: None,
        order_by: Vec::new(),
        limit: None,
        offset: 0,
        windows: Vec::new(),
        tail: Vec::new(),
        locking: Vec::new(),
    }
}

fn values_to_select(rows: Vec<Vec<Expr>>, col_aliases: &[String]) -> SelectStmt {
    let proj = |exprs: Vec<Expr>, named: bool| -> Vec<SelectItem> {
        exprs
            .into_iter()
            .enumerate()
            .map(|(i, e)| {
                let alias = named.then(|| {
                    col_aliases
                        .get(i)
                        .cloned()
                        .unwrap_or_else(|| format!("column{}", i + 1))
                });
                SelectItem::Expr { expr: e, alias }
            })
            .collect()
    };
    let mut iter = rows.into_iter();
    let mut head = bare_select(proj(iter.next().unwrap_or_default(), true));
    head.tail = iter
        .map(|exprs| super::ast::SetOpArm {
            op: super::ast::SetOp::Union,
            all: true,
            arm: bare_select(proj(exprs, false)),
        })
        .collect();
    head
}

pub fn parse_select_at(
    toks: &[Tok],
    pos: usize,
    src: &str,
) -> Result<(SelectStmt, usize), PgError> {
    parse_select_at_depth(toks, pos, src, 0)
}

pub(crate) fn parse_select_at_depth(
    toks: &[Tok],
    pos: usize,
    src: &str,
    query_depth: usize,
) -> Result<(SelectStmt, usize), PgError> {
    validate_query_token_nesting(toks.get(pos..).unwrap_or(&[]), src)?;
    let mut p = P {
        toks,
        pos,
        src,
        query_depth,
    };
    if !p.at_kw("select") {
        return Err(err(src));
    }
    let s = p.parse_select()?;
    Ok((s, p.pos))
}

pub fn parse_query_at(toks: &[Tok], pos: usize, src: &str) -> Result<(SelectStmt, usize), PgError> {
    parse_query_at_depth(toks, pos, src, 0)
}

pub(crate) fn parse_query_at_depth(
    toks: &[Tok],
    pos: usize,
    src: &str,
    query_depth: usize,
) -> Result<(SelectStmt, usize), PgError> {
    let (mut first, mut cur) = parse_select_at_depth(toks, pos, src, query_depth)?;
    let mut tail: Vec<super::ast::SetOpArm> = Vec::new();
    while let Some(Tok::Ident(s)) = toks.get(cur) {
        let Some(op) = super::ast::SetOp::from_kw(s) else {
            break;
        };
        cur += 1;
        let all = matches!(toks.get(cur), Some(Tok::Ident(a)) if a == "all");
        if all {
            cur += 1;
        }
        if !matches!(toks.get(cur), Some(Tok::Ident(a)) if a == "select") {
            return Err(err(src));
        }
        let (arm, next) = parse_select_at_depth(toks, cur, src, query_depth)?;
        cur = next;
        tail.push(super::ast::SetOpArm { op, all, arm });
    }
    first.tail = tail;
    Ok((first, cur))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::expr::ast::{BinOp, Expr};

    fn sel(sql: &str) -> SelectStmt {
        match parse(sql).expect("parse ok") {
            Stmt::Select(s) => s,
        }
    }

    #[test]
    fn star_from() {
        let s = sel("SELECT * FROM t");
        assert_eq!(s.projection, vec![SelectItem::Star]);
        assert_eq!(
            s.from,
            Some(FromItem::Table {
                name: "t".into(),
                alias: None
            })
        );
        assert!(!s.distinct);
    }

    #[test]
    fn projection_with_aliases() {
        let s = sel("SELECT a, b AS x, c d FROM t");
        assert_eq!(
            s.projection,
            vec![
                SelectItem::Expr {
                    expr: Expr::Column("a".into()),
                    alias: None
                },
                SelectItem::Expr {
                    expr: Expr::Column("b".into()),
                    alias: Some("x".into())
                },
                SelectItem::Expr {
                    expr: Expr::Column("c".into()),
                    alias: Some("d".into())
                },
            ]
        );
    }

    #[test]
    fn where_order_limit_offset() {
        let s = sel("SELECT v FROM t WHERE v >= 2 ORDER BY v DESC, id ASC LIMIT 5 OFFSET 3");
        assert!(matches!(
            s.filter,
            Some(Expr::Binary {
                op: BinOp::GtEq,
                ..
            })
        ));
        assert_eq!(s.order_by.len(), 2);
        assert!(s.order_by[0].descending);
        assert!(!s.order_by[1].descending);
        assert_eq!(s.limit, Some(5));
        assert_eq!(s.offset, 3);
    }

    #[test]
    fn limit_all_and_offset_before_limit() {
        assert_eq!(sel("SELECT a FROM t LIMIT ALL").limit, None);
        let s = sel("SELECT a FROM t OFFSET 2 LIMIT 4");
        assert_eq!((s.limit, s.offset), (Some(4), 2));
    }

    #[test]
    fn distinct_and_no_from() {
        assert!(sel("SELECT DISTINCT a FROM t").distinct);
        let s = sel("SELECT 1 + 2");
        assert_eq!(s.from, None);
    }

    #[test]
    fn computed_projection_and_function() {
        let s = sel("SELECT upper(name), v * 2 AS dbl FROM t");
        assert_eq!(s.projection.len(), 2);
        assert!(matches!(&s.projection[1], SelectItem::Expr { alias: Some(a), .. } if a == "dbl"));
    }

    #[test]
    fn named_window_clause_and_bare_over_ref() {
        let s = sel(
            "SELECT sum(v) OVER w, rank() OVER w FROM t WINDOW w AS (PARTITION BY g ORDER BY id)",
        );
        assert_eq!(s.windows.len(), 1);
        assert_eq!(s.windows[0].name, "w");
        assert_eq!(s.windows[0].partition_by, vec![Expr::Column("g".into())]);
        assert_eq!(s.windows[0].order_by.len(), 1);

        for it in &s.projection {
            match it {
                SelectItem::Expr {
                    expr:
                        Expr::Window {
                            window_ref,
                            partition_by,
                            ..
                        },
                    ..
                } => {
                    assert_eq!(window_ref.as_deref(), Some("w"));
                    assert!(partition_by.is_empty());
                }
                _ => panic!("expected a window ref"),
            }
        }
    }

    #[test]
    fn multiple_named_windows_and_over_paren_ref() {
        let s = sel(
            "SELECT sum(v) OVER (w ORDER BY id) FROM t WINDOW w AS (PARTITION BY g), x AS (ORDER BY v)",
        );
        assert_eq!(s.windows.len(), 2);
        assert_eq!(s.windows[1].name, "x");

        match &s.projection[0] {
            SelectItem::Expr {
                expr:
                    Expr::Window {
                        window_ref,
                        order_by,
                        ..
                    },
                ..
            } => {
                assert_eq!(window_ref.as_deref(), Some("w"));
                assert_eq!(order_by.len(), 1);
            }
            _ => panic!("expected a window ref"),
        }
    }

    #[test]
    fn rejects_garbage() {
        assert!(parse("SELECT a FROM t WHERE").is_err());
        assert!(parse("DELETE FROM t").is_err());
        assert!(parse("SELECT a FROM t t2 t3").is_err());
    }

    #[test]
    fn rejects_excessive_query_nesting_without_stack_overflow() {
        let mut sql = String::from("SELECT 1");
        for _ in 0..1100 {
            sql = format!("SELECT * FROM ({sql}) AS q");
        }
        assert!(parse(&sql).is_err());
    }
}
