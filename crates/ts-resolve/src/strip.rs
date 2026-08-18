
use rusty_js_parser::{Lexer, LexerGoal, Punct, Span, TemplatePart, TokenKind};

fn is_overload_blocked_name(name: &str) -> bool {

    matches!(
        name,
        "if" | "for" | "while" | "switch" | "catch" | "with"
        | "return" | "yield" | "await" | "throw" | "new" | "typeof"
        | "delete" | "void"
        | "let" | "const" | "var" | "function" | "class"
        | "import" | "export" | "default"

        | "true" | "false" | "null" | "undefined" | "this"
        | "super" | "async" | "static"
    )
}

fn expr_or_div_goal(prev: Option<&TokenKind>) -> LexerGoal {
    let prev = match prev {
        Some(p) => p,
        None => return LexerGoal::RegExp,
    };
    match prev {

        TokenKind::Ident(name)
            if !matches!(
                name.as_str(),
                "return"
                    | "typeof"
                    | "delete"
                    | "void"
                    | "await"
                    | "yield"
                    | "throw"
                    | "new"
                    | "in"
                    | "of"
                    | "instanceof"
                    | "case"
            ) =>
        {
            LexerGoal::Div
        }
        TokenKind::Number(_, _)
        | TokenKind::BigInt(_, _)
        | TokenKind::String(_)
        | TokenKind::Template { .. }
        | TokenKind::Punct(Punct::RParen)
        | TokenKind::Punct(Punct::RBracket)
        | TokenKind::Punct(Punct::RBrace)
        | TokenKind::Punct(Punct::Inc)
        | TokenKind::Punct(Punct::Dec) => LexerGoal::Div,

        _ => LexerGoal::RegExp,
    }
}
use crate::ts_ast::{TsTypeRef, TypeWitness, TypeWitnessKind};

#[derive(Debug, Clone, Copy, Default)]
pub struct StripOptions {

    pub compat_angle_assertions: bool,
}

impl StripOptions {
    pub fn compat_erasure() -> Self {
        StripOptions {
            compat_angle_assertions: true,
        }
    }
}

pub fn compat_erasure_enabled() -> bool {
    matches!(
        std::env::var("CRUFT_TS_COMPAT").as_deref(),
        Ok("1") | Ok("true") | Ok("TRUE") | Ok("yes") | Ok("on")
    ) || matches!(
        std::env::var("CRUFT_TS_COMPAT_ERASURE").as_deref(),
        Ok("1") | Ok("true") | Ok("TRUE") | Ok("yes") | Ok("on")
    )
}

pub fn options_for_path(path: &str) -> StripOptions {
    let tsx = path.ends_with(".tsx");
    StripOptions {
        compat_angle_assertions: !tsx && compat_erasure_enabled(),
    }
}

#[derive(Debug)]
pub struct StripError {
    pub message: String,
    pub pos: usize,
}

impl std::fmt::Display for StripError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ts-strip @{}: {}", self.pos, self.message)
    }
}

pub fn strip_ts(src: &str) -> Result<(String, Vec<TypeWitness>), StripError> {
    strip_ts_with_options(src, StripOptions::default())
}

pub fn strip_ts_with_options(
    src: &str,
    options: StripOptions,
) -> Result<(String, Vec<TypeWitness>), StripError> {
    let mut scanner = Scanner::new(src, options);
    scanner.run()?;

    let mut out = src.as_bytes().to_vec();
    for (start, end) in &scanner.strips {

        for i in *start..*end {
            if out[i] != b'\n' && out[i] != b'\r' {
                out[i] = b' ';
            }
        }
    }
    let stripped = String::from_utf8(out).map_err(|e| StripError {
        message: format!("utf-8 corruption: {}", e),
        pos: 0,
    })?;
    Ok((stripped, scanner.witnesses))
}

struct ScanTok {
    kind: TokenKind,
    span: Span,
    preceded_by_line_terminator: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum BraceCtx {

    Block,

    ClassBody,

    ObjectLit,

    ObjectType,
}

struct Scanner<'src> {
    src: &'src str,
    options: StripOptions,
    toks: Vec<ScanTok>,

    strips: Vec<(usize, usize)>,
    witnesses: Vec<TypeWitness>,

    brace_stack: Vec<(BraceCtx, i32)>,

    paren_depth: i32,

    ternary_stack: Vec<(i32, usize)>,

    pending_class_body: bool,
}

impl<'src> Scanner<'src> {
    fn new(src: &'src str, options: StripOptions) -> Self {
        Scanner {
            src,
            options,
            toks: Vec::new(),
            strips: Vec::new(),
            witnesses: Vec::new(),
            brace_stack: Vec::new(),
            paren_depth: 0,
            ternary_stack: Vec::new(),
            pending_class_body: false,
        }
    }

    fn in_class_body(&self) -> bool {

        true
    }

    fn classify_brace(&self, i: usize) -> BraceCtx {
        if i == 0 {
            return BraceCtx::Block;
        }
        match &self.toks[i - 1].kind {

            TokenKind::Punct(Punct::Assign)
            | TokenKind::Punct(Punct::LParen)
            | TokenKind::Punct(Punct::LBracket)
            | TokenKind::Punct(Punct::Comma)
            | TokenKind::Punct(Punct::Colon)
            | TokenKind::Punct(Punct::Question)
            | TokenKind::Punct(Punct::LogicalAnd)
            | TokenKind::Punct(Punct::LogicalOr)
            | TokenKind::Punct(Punct::NullishCoalesce)
            | TokenKind::Punct(Punct::Spread) => BraceCtx::ObjectLit,

            TokenKind::Ident(n)
                if n == "return"
                    || n == "yield"
                    || n == "throw"
                    || n == "in"
                    || n == "of"
                    || n == "delete"
                    || n == "new" =>
            {
                BraceCtx::ObjectLit
            }

            _ => BraceCtx::Block,
        }
    }

    fn lex_all(&mut self) -> Result<(), StripError> {

        let mut lx = Lexer::new(self.src);
        let mut tmpl_brace_depths: Vec<i32> = Vec::new();
        let mut brace_depth: i32 = 0;
        let mut prev_kind: Option<TokenKind> = None;
        let mut prev_was_postfix_bang: bool = false;
        loop {

            let goal = if let Some(&entry_depth) = tmpl_brace_depths.last() {
                if brace_depth == entry_depth {
                    LexerGoal::TemplateTail
                } else if prev_was_postfix_bang {
                    LexerGoal::Div
                } else {
                    expr_or_div_goal(prev_kind.as_ref())
                }
            } else if prev_was_postfix_bang {
                LexerGoal::Div
            } else {
                expr_or_div_goal(prev_kind.as_ref())
            };
            let t = lx.next_token(goal).map_err(|e| StripError {
                message: format!("lex: {:?}", e),
                pos: lx.pos(),
            })?;

            match &t.kind {
                TokenKind::Punct(Punct::LBrace) => brace_depth += 1,
                TokenKind::Punct(Punct::RBrace) => brace_depth -= 1,
                TokenKind::Template { part, .. } => {
                    match part {
                        TemplatePart::Head => {

                            tmpl_brace_depths.push(brace_depth);
                        }
                        TemplatePart::Middle => {

                        }
                        TemplatePart::Tail => {

                            tmpl_brace_depths.pop();
                        }
                        TemplatePart::NoSubstitution => {

                        }
                    }
                }
                _ => {}
            }

            prev_was_postfix_bang = matches!(t.kind, TokenKind::Punct(Punct::LogicalNot))
                && match prev_kind.as_ref() {
                    Some(TokenKind::Ident(n)) => !matches!(
                        n.as_str(),
                        "return"
                            | "yield"
                            | "delete"
                            | "typeof"
                            | "void"
                            | "throw"
                            | "await"
                            | "new"
                            | "in"
                            | "of"
                            | "instanceof"
                            | "case"
                    ),
                    Some(TokenKind::Number(_, _))
                    | Some(TokenKind::BigInt(_, _))
                    | Some(TokenKind::String(_))
                    | Some(TokenKind::Template { .. })
                    | Some(TokenKind::Punct(Punct::RParen))
                    | Some(TokenKind::Punct(Punct::RBracket))
                    | Some(TokenKind::Punct(Punct::RBrace))
                    | Some(TokenKind::Punct(Punct::Inc))
                    | Some(TokenKind::Punct(Punct::Dec)) => true,
                    _ => false,
                };
            prev_kind = Some(t.kind.clone());
            let done = matches!(t.kind, TokenKind::Eof);
            self.toks.push(ScanTok {
                kind: t.kind,
                span: t.span,
                preceded_by_line_terminator: t.preceded_by_line_terminator,
            });
            if done {
                return Ok(());
            }
        }
    }

    fn elide_unused_imports(&mut self) {

        let mut in_strip: Vec<bool> = vec![false; self.toks.len()];
        for (start, end) in &self.strips {
            for (idx, tok) in self.toks.iter().enumerate() {
                if tok.span.start >= *start && tok.span.end <= *end {
                    in_strip[idx] = true;
                }
            }
        }

        let mut new_strips: Vec<(usize, usize)> = Vec::new();
        let mut i = 0;
        while i < self.toks.len() {
            if in_strip[i] {
                i += 1;
                continue;
            }
            let is_import_stmt = matches!(&self.toks[i].kind,
                TokenKind::Ident(n) if n == "import")
                && self.is_stmt_start(i);
            if !is_import_stmt {
                i += 1;
                continue;
            }

            let stmt_end_idx = match self.find_stmt_end(i + 1) {
                Some(idx) => idx,
                None => {
                    i += 1;
                    continue;
                }
            };

            let mut name_idxs: Vec<usize> = Vec::new();
            let mut j = i + 1;
            let mut side_effect_only = true;

            while j <= stmt_end_idx {
                match &self.toks[j].kind {
                    TokenKind::Ident(name) if name == "from" => break,
                    TokenKind::Ident(name) if name == "as" => {

                        name_idxs.pop();
                        side_effect_only = false;
                        j += 1;
                        continue;
                    }
                    TokenKind::Ident(name) if name == "type" && j == i + 1 => {

                        return;

                    }
                    TokenKind::Ident(_) => {
                        name_idxs.push(j);
                        side_effect_only = false;
                    }
                    TokenKind::Punct(Punct::Star) => {
                        side_effect_only = false;
                    }
                    TokenKind::Punct(Punct::LBrace)
                    | TokenKind::Punct(Punct::RBrace)
                    | TokenKind::Punct(Punct::Comma)
                    | TokenKind::Punct(Punct::Colon) => {}
                    TokenKind::String(_) => {

                        break;
                    }
                    _ => {}
                }
                j += 1;
            }
            if side_effect_only || name_idxs.is_empty() {
                i = stmt_end_idx + 1;
                continue;
            }

            let mut all_unused = true;
            for &nidx in &name_idxs {
                let target_name = match &self.toks[nidx].kind {
                    TokenKind::Ident(n) => n.clone(),
                    _ => continue,
                };
                let mut count = 0usize;
                for (idx, tok) in self.toks.iter().enumerate() {
                    if idx >= i && idx <= stmt_end_idx {
                        continue;
                    }
                    if in_strip[idx] {
                        continue;
                    }
                    if let TokenKind::Ident(n) = &tok.kind {
                        if n == &target_name {
                            count += 1;
                        }
                    }
                }
                if count > 0 {
                    all_unused = false;
                    break;
                }
            }
            if all_unused {
                let start = self.toks[i].span.start;
                let end = self.toks[stmt_end_idx].span.end;
                new_strips.push((start, end));
            }
            i = stmt_end_idx + 1;
        }
        if !new_strips.is_empty() {
            self.strips.extend(new_strips);

            self.strips.sort_by_key(|r| r.0);
            let mut merged: Vec<(usize, usize)> = Vec::with_capacity(self.strips.len());
            for r in self.strips.drain(..) {
                if let Some(last) = merged.last_mut() {
                    if r.0 <= last.1 {
                        last.1 = last.1.max(r.1);
                        continue;
                    }
                }
                merged.push(r);
            }
            self.strips = merged;
        }
    }

    fn run(&mut self) -> Result<(), StripError> {
        self.lex_all()?;
        let n = self.toks.len();
        let mut i = 0;
        while i < n {
            i = self.step(i)?;
        }

        self.strips.sort_by_key(|r| r.0);
        let mut merged: Vec<(usize, usize)> = Vec::with_capacity(self.strips.len());
        for r in self.strips.drain(..) {
            if let Some(last) = merged.last_mut() {
                if r.0 <= last.1 {
                    last.1 = last.1.max(r.1);
                    continue;
                }
            }
            merged.push(r);
        }
        self.strips = merged;

        self.elide_unused_imports();
        Ok(())
    }

    fn step(&mut self, i: usize) -> Result<usize, StripError> {
        let t = &self.toks[i];
        match &t.kind {
            TokenKind::Ident(name) => {

                if name == "class" {
                    self.pending_class_body = true;
                }
                if (name == "function" || name == "class") && self.next_is_ident(i + 1) {
                    let after_name = i + 2;
                    if after_name < self.toks.len()
                        && matches!(self.toks[after_name].kind, TokenKind::Punct(Punct::Lt))
                    {
                        if let Some(close) = self.match_angle(after_name) {
                            let start = self.toks[after_name].span.start;
                            let end = self.toks[close].span.end;
                            self.strips.push((start, end));

                        }
                    }
                }

                if name == "extends" && self.next_is_ident(i + 1) {
                    let after_name = i + 2;
                    if after_name < self.toks.len()
                        && matches!(self.toks[after_name].kind, TokenKind::Punct(Punct::Lt))
                    {
                        if let Some(close) = self.match_angle(after_name) {
                            let start = self.toks[after_name].span.start;
                            let end = self.toks[close].span.end;
                            self.strips.push((start, end));
                        }
                    }
                }

                if name == "implements" {

                    let mut j = i + 1;
                    let mut depth = 0i32;
                    while j < self.toks.len() {
                        match &self.toks[j].kind {
                            TokenKind::Punct(Punct::LBrace) if depth == 0 => break,
                            TokenKind::Punct(Punct::LParen) | TokenKind::Punct(Punct::LBracket) => {
                                depth += 1
                            }
                            TokenKind::Punct(Punct::RParen) | TokenKind::Punct(Punct::RBracket)
                                if depth > 0 =>
                            {
                                depth -= 1
                            }
                            TokenKind::Punct(Punct::Lt) => depth += 1,
                            TokenKind::Punct(Punct::Gt) if depth > 0 => depth -= 1,
                            TokenKind::Eof | TokenKind::Punct(Punct::Semicolon) => break,
                            _ => {}
                        }
                        j += 1;
                    }
                    if j < self.toks.len()
                        && matches!(self.toks[j].kind, TokenKind::Punct(Punct::LBrace))
                        && j > i
                    {
                        let start = t.span.start;
                        let end = self.toks[j - 1].span.end;
                        self.strips.push((start, end));
                    }
                }

                let is_ts_class_modifier = matches!(
                    name.as_str(),
                    "public" | "private" | "protected" | "readonly" | "abstract" | "override"
                );
                if is_ts_class_modifier && self.in_class_body() {

                    if i + 1 < self.toks.len()
                        && matches!(self.toks[i + 1].kind, TokenKind::Ident(_))
                    {
                        if self.paren_depth > 0 {

                            return Err(StripError {
                                message: "TypeScript parameter property is not supported: \
                                     constructor(public/private/protected/readonly x) \
                                     requires synthesizing `this.x = x`, which the \
                                     type-eraser does not emit"
                                    .to_string(),
                                pos: t.span.start,
                            });
                        }
                        self.strips.push((t.span.start, t.span.end));
                    }
                }

                let in_block_or_module = matches!(
                    self.brace_stack.last(),
                    Some((BraceCtx::ClassBody, _)) | None
                );
                let stmt_start_prev = i == 0
                    || t.preceded_by_line_terminator
                    || matches!(
                        self.toks[i - 1].kind,
                        TokenKind::Punct(Punct::LBrace) | TokenKind::Punct(Punct::Semicolon)
                    )
                    || matches!(&self.toks[i - 1].kind,
                        TokenKind::Ident(prev_name) if prev_name == "function"
                            || prev_name == "abstract"
                            || prev_name == "public"
                            || prev_name == "private"
                            || prev_name == "protected"
                            || prev_name == "override"
                            || prev_name == "static");

                let prev_is_function_kw = i > 0
                    && matches!(&self.toks[i - 1].kind,
                        TokenKind::Ident(n) if n == "function");
                let at_class_member_start = in_block_or_module
                    && stmt_start_prev
                    && (matches!(self.brace_stack.last(), Some((BraceCtx::ClassBody, _)))
                        || prev_is_function_kw);
                if at_class_member_start && !is_overload_blocked_name(name) {

                    let lparen_search_pos = if i + 1 < self.toks.len()
                        && matches!(self.toks[i + 1].kind, TokenKind::Punct(Punct::Lt))
                    {
                        if let Some(close) = self.match_angle(i + 1) {
                            close + 1
                        } else {
                            i + 1
                        }
                    } else {
                        i + 1
                    };
                    if let Some(lparen) =
                        self.next_punct_immediate(lparen_search_pos, Punct::LParen)
                    {
                        if let Some(rparen) = self.match_parens(lparen) {

                            let after_rparen = rparen + 1;
                            let after_kind = self.toks.get(after_rparen).map(|t| &t.kind);
                            let is_method_decl_shape = matches!(
                                after_kind,
                                Some(TokenKind::Punct(Punct::Colon))
                                    | Some(TokenKind::Punct(Punct::LBrace))
                                    | Some(TokenKind::Punct(Punct::Semicolon))
                            );
                            if !is_method_decl_shape {

                            } else {

                                let mut k = after_rparen;
                                let mut depth = 0i32;
                                let mut found_overload = false;
                                let in_class_body = matches!(
                                    self.brace_stack.last(),
                                    Some((BraceCtx::ClassBody, _))
                                );
                                while k < self.toks.len() {

                                    if in_class_body
                                        && depth == 0
                                        && k > after_rparen
                                        && self.toks[k].preceded_by_line_terminator
                                        && matches!(
                                            self.toks[k].kind,
                                            TokenKind::Ident(_) | TokenKind::Punct(Punct::RBrace)
                                        )
                                    {
                                        found_overload = true;

                                        k = if k > 0 { k - 1 } else { k };

                                        break;
                                    }
                                    match &self.toks[k].kind {
                                        TokenKind::Punct(Punct::LBrace) if depth == 0 => break,
                                        TokenKind::Punct(Punct::LParen)
                                        | TokenKind::Punct(Punct::LBracket)
                                        | TokenKind::Punct(Punct::Lt) => depth += 1,
                                        TokenKind::Punct(Punct::RParen)
                                        | TokenKind::Punct(Punct::RBracket)
                                        | TokenKind::Punct(Punct::Gt)
                                            if depth > 0 =>
                                        {
                                            depth -= 1
                                        }
                                        TokenKind::Punct(Punct::Shr) if depth > 0 => {
                                            depth = (depth - 2).max(0)
                                        }
                                        TokenKind::Punct(Punct::UShr) if depth > 0 => {
                                            depth = (depth - 3).max(0)
                                        }
                                        TokenKind::Punct(Punct::Semicolon) if depth == 0 => {
                                            found_overload = true;
                                            break;
                                        }
                                        TokenKind::Eof => break,
                                        _ => {}
                                    }
                                    k += 1;
                                }
                                if found_overload {

                                    let start = if i > 0
                                        && matches!(&self.toks[i - 1].kind,
                                        TokenKind::Ident(n) if n == "function")
                                    {
                                        self.toks[i - 1].span.start
                                    } else {
                                        t.span.start
                                    };
                                    let end = self.toks[k].span.end;
                                    self.strips.push((start, end));

                                    return Ok(k + 1);
                                }
                            }
                        }
                    }
                }

                if name == "interface" && self.next_is_ident(i + 1) {
                    if let Some(brace_open) = self.find_punct(i + 2, Punct::LBrace) {
                        if let Some(brace_close) = self.match_braces(brace_open) {
                            let start = t.span.start;
                            let end = self.toks[brace_close].span.end;
                            self.strips.push((start, end));
                            return Ok(brace_close + 1);
                        }
                    }
                }

                if name == "type" && self.is_stmt_start(i) && self.next_is_ident(i + 1) {
                    if let Some(end_idx) = self.find_stmt_end(i + 2) {
                        let start = t.span.start;
                        let end = self.toks[end_idx].span.end;
                        self.strips.push((start, end));
                        return Ok(end_idx + 1);
                    }
                }

                if name == "import"
                    && self.is_stmt_start(i)
                    && i + 2 < self.toks.len()
                    && matches!(&self.toks[i + 1].kind, TokenKind::Ident(_))
                    && matches!(&self.toks[i + 2].kind, TokenKind::Punct(Punct::Assign))
                {
                    if let Some(end_idx) = self.find_stmt_end(i + 1) {
                        let start = t.span.start;
                        let end = self.toks[end_idx].span.end;
                        self.strips.push((start, end));
                        return Ok(end_idx + 1);
                    }
                }

                if (name == "import" || name == "export")
                    && self.is_stmt_start(i)
                    && i + 1 < self.toks.len()
                    && matches!(&self.toks[i + 1].kind, TokenKind::Ident(n) if n == "type")

                    && (name == "import" || (i + 2 < self.toks.len()
                        && (matches!(self.toks[i + 2].kind, TokenKind::Punct(Punct::LBrace))
                            || (matches!(self.toks[i + 2].kind, TokenKind::Ident(_))
                                && i + 3 < self.toks.len()
                                && matches!(&self.toks[i + 3].kind, TokenKind::Ident(n) if n == "from")))))
                {
                    if let Some(end_idx) = self.find_stmt_end(i + 1) {
                        let start = t.span.start;
                        let end = self.toks[end_idx].span.end;
                        self.strips.push((start, end));
                        return Ok(end_idx + 1);
                    }
                }

                if name == "enum" && self.next_is_ident(i + 1) {
                    if let Some(brace_open) = self.find_punct(i + 2, Punct::LBrace) {
                        if let Some(brace_close) = self.match_braces(brace_open) {

                            let mut start_idx = i;
                            let mut is_ambient = false;
                            while start_idx > 0 {
                                if let TokenKind::Ident(n) = &self.toks[start_idx - 1].kind {
                                    if n == "declare" {
                                        is_ambient = true;
                                    }
                                    if n == "export"
                                        || n == "declare"
                                        || n == "const"
                                        || n == "default"
                                    {
                                        start_idx -= 1;
                                        continue;
                                    }
                                }
                                break;
                            }
                            let start = self.toks[start_idx].span.start;
                            let end = self.toks[brace_close].span.end;
                            if is_ambient {

                                self.strips.push((start, end));
                                return Ok(brace_close + 1);
                            }

                            return Err(StripError {
                                message: "TypeScript enum is not supported: a runtime enum \
                                     requires lowering to a runtime object, which the \
                                     type-eraser does not emit"
                                    .to_string(),
                                pos: start,
                            });
                        }
                    }
                }

                if name == "namespace" && self.next_is_ident(i + 1) {
                    if let Some(brace_open) = self.find_punct(i + 2, Punct::LBrace) {
                        if self.match_braces(brace_open).is_some() {
                            let is_ambient = i > 0
                                && matches!(&self.toks[i - 1].kind,
                                    TokenKind::Ident(n) if n == "declare");
                            if !is_ambient {
                                return Err(StripError {
                                    message: "TypeScript namespace is not supported: a runtime \
                                         namespace requires lowering to a runtime object, \
                                         which the type-eraser does not emit"
                                        .to_string(),
                                    pos: t.span.start,
                                });
                            }
                        }
                    }
                }

                if name == "declare" && self.is_stmt_start(i) {
                    let next_is_enum = matches!(self.toks.get(i + 1),
                        Some(tk) if matches!(&tk.kind, TokenKind::Ident(n) if n == "enum"));
                    if !next_is_enum {
                        if let Some(end_idx) = self.find_stmt_end(i + 1) {
                            let start = t.span.start;
                            let end = self.toks[end_idx].span.end;
                            self.strips.push((start, end));
                            return Ok(end_idx + 1);
                        }
                    }
                }

                if name == "as"
                    && i > 0
                    && self.is_expr_terminator(i - 1)
                    && !self.in_module_specifier_alias(i)
                {
                    let type_start = t.span.start;
                    let after = self.skip_type(i + 1);
                    let type_end = if after > i + 1 {
                        self.toks[after - 1].span.end
                    } else {
                        t.span.end
                    };
                    self.strips.push((type_start, type_end));
                    return Ok(after);
                }

                if name == "satisfies" && i > 0 && self.is_expr_terminator(i - 1) {
                    let type_start = t.span.start;
                    let after = self.skip_type(i + 1);
                    let type_end = if after > i + 1 {
                        self.toks[after - 1].span.end
                    } else {
                        t.span.end
                    };
                    self.strips.push((type_start, type_end));
                    return Ok(after);
                }
                Ok(i + 1)
            }
            TokenKind::Punct(Punct::Colon) => {

                let at_obj_key = i > 0
                    && matches!(self.toks[i - 1].kind, TokenKind::Ident(_))
                    && matches!(self.brace_stack.last(),
                        Some((BraceCtx::ObjectLit, push_pd)) if *push_pd == self.paren_depth);
                if !at_obj_key {
                    if let Some(&(top_pd, top_brace_len)) = self.ternary_stack.last() {
                        if top_pd == self.paren_depth && top_brace_len == self.brace_stack.len() {
                            self.ternary_stack.pop();
                            return Ok(i + 1);
                        }
                    }
                }

                if self.is_annotation_colon(i) {
                    let start = t.span.start;
                    let after = self.skip_type(i + 1);
                    let end = if after > i + 1 {
                        self.toks[after - 1].span.end
                    } else {
                        t.span.end
                    };
                    self.strips.push((start, end));

                    if i > 0 {
                        if let TokenKind::Ident(nm) = &self.toks[i - 1].kind {
                            let ty_text = &self.src[t.span.end..end];
                            self.witnesses.push(TypeWitness {
                                kind: TypeWitnessKind::LocalBinding {
                                    name: nm.clone(),
                                    ty: TsTypeRef::Named {
                                        name: ty_text.trim().to_string(),
                                        type_args: vec![],
                                    },
                                },
                                span: Span::new(start, end),
                            });
                        }
                    }
                    return Ok(after);
                }
                Ok(i + 1)
            }
            TokenKind::Punct(Punct::Question) => {

                if i + 1 < self.toks.len()
                    && matches!(self.toks[i + 1].kind, TokenKind::Punct(Punct::Colon))
                    && i > 0
                    && matches!(self.toks[i - 1].kind, TokenKind::Ident(_))
                {
                    self.strips.push((t.span.start, t.span.end));
                    return Ok(i + 1);
                }

                let next_is_colon = i + 1 < self.toks.len()
                    && matches!(self.toks[i + 1].kind, TokenKind::Punct(Punct::Colon));
                let next_is_dot = i + 1 < self.toks.len()
                    && matches!(self.toks[i + 1].kind, TokenKind::Punct(Punct::Dot));
                if !next_is_colon && !next_is_dot && i > 0 && self.is_expr_terminator(i - 1) {
                    self.ternary_stack
                        .push((self.paren_depth, self.brace_stack.len()));
                }
                Ok(i + 1)
            }
            TokenKind::Punct(Punct::LogicalNot) => {

                if i > 0 && self.is_expr_terminator(i - 1) && self.next_is_postfix_context(i + 1) {
                    self.strips.push((t.span.start, t.span.end));
                    return Ok(i + 1);
                }
                Ok(i + 1)
            }
            TokenKind::Punct(Punct::LBrace) => {
                let ctx = if self.pending_class_body {
                    self.pending_class_body = false;
                    BraceCtx::ClassBody
                } else {
                    self.classify_brace(i)
                };
                self.brace_stack.push((ctx, self.paren_depth));
                Ok(i + 1)
            }
            TokenKind::Punct(Punct::RBrace) => {
                self.brace_stack.pop();
                Ok(i + 1)
            }
            TokenKind::Punct(Punct::LParen) => {
                self.paren_depth += 1;
                Ok(i + 1)
            }
            TokenKind::Punct(Punct::RParen) => {
                if self.paren_depth > 0 {
                    self.paren_depth -= 1;
                }
                Ok(i + 1)
            }
            TokenKind::Punct(Punct::Lt) => {

                if let Some(close) = self.match_angle(i) {
                    let after = close + 1;
                    if after < self.toks.len()
                        && matches!(self.toks[after].kind, TokenKind::Punct(Punct::LParen))
                    {

                        let prev_is_expr_terminator = i > 0 && self.is_expr_terminator(i - 1);
                        let _ = prev_is_expr_terminator;
                        let start = self.toks[i].span.start;
                        let end = self.toks[close].span.end;
                        self.strips.push((start, end));
                    }

                    if self.options.compat_angle_assertions
                        && self.prev_allows_angle_assertion(i)
                        && after < self.toks.len()
                        && !matches!(self.toks[after].kind, TokenKind::Punct(Punct::LParen))
                        && self.token_can_start_angle_asserted_expr(after)
                    {
                        let start = self.toks[i].span.start;
                        let end = self.toks[close].span.end;
                        self.strips.push((start, end));
                    }
                }
                Ok(i + 1)
            }
            _ => Ok(i + 1),
        }
    }

    fn next_is_ident(&self, i: usize) -> bool {
        i < self.toks.len() && matches!(self.toks[i].kind, TokenKind::Ident(_))
    }

    fn is_stmt_start(&self, i: usize) -> bool {
        if i == 0 {
            return true;
        }
        let prev = &self.toks[i - 1];
        if prev.preceded_by_line_terminator || self.toks[i].preceded_by_line_terminator {
            return true;
        }
        matches!(
            prev.kind,
            TokenKind::Punct(Punct::Semicolon)
                | TokenKind::Punct(Punct::LBrace)
                | TokenKind::Punct(Punct::RBrace)
        )
    }

    fn in_module_specifier_alias(&self, i: usize) -> bool {
        let mut depth_brace = 0i32;
        let mut depth_paren = 0i32;
        let mut depth_brack = 0i32;
        let mut saw_lbrace = false;
        let mut j = i;
        while j > 0 {
            j -= 1;
            match &self.toks[j].kind {
                TokenKind::Punct(Punct::RBrace) => depth_brace += 1,
                TokenKind::Punct(Punct::LBrace) => {
                    if depth_brace == 0 {
                        saw_lbrace = true;
                    } else {
                        depth_brace -= 1;
                    }
                }
                TokenKind::Punct(Punct::RParen) => depth_paren += 1,
                TokenKind::Punct(Punct::LParen) if depth_paren > 0 => depth_paren -= 1,
                TokenKind::Punct(Punct::RBracket) => depth_brack += 1,
                TokenKind::Punct(Punct::LBracket) if depth_brack > 0 => depth_brack -= 1,
                TokenKind::Punct(Punct::Semicolon)
                    if depth_brace == 0 && depth_paren == 0 && depth_brack == 0 =>
                {
                    return false;
                }
                TokenKind::Ident(name)
                    if depth_brace == 0
                        && depth_paren == 0
                        && depth_brack == 0
                        && (name == "import" || name == "export")
                        && self.is_stmt_start(j) =>
                {

                    if !saw_lbrace {
                        return false;
                    }
                    let mut k = j + 1;
                    if matches!(self.toks.get(k).map(|t| &t.kind),
                        Some(TokenKind::Ident(n)) if n == "type")
                    {
                        k += 1;
                    }
                    return matches!(
                        self.toks.get(k).map(|t| &t.kind),
                        Some(TokenKind::Punct(Punct::LBrace))
                    );
                }
                _ => {}
            }
        }
        false
    }

    fn next_punct_immediate(&self, at: usize, p: Punct) -> Option<usize> {
        if at < self.toks.len() {
            if let TokenKind::Punct(pp) = &self.toks[at].kind {
                if *pp == p {
                    return Some(at);
                }
            }
        }
        None
    }

    fn match_parens(&self, lparen: usize) -> Option<usize> {
        let mut depth = 0i32;
        for j in lparen..self.toks.len() {
            match &self.toks[j].kind {
                TokenKind::Punct(Punct::LParen) => depth += 1,
                TokenKind::Punct(Punct::RParen) => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(j);
                    }
                }
                TokenKind::Eof => return None,
                _ => {}
            }
        }
        None
    }

    fn find_punct(&self, from: usize, p: Punct) -> Option<usize> {
        for j in from..self.toks.len() {
            if let TokenKind::Punct(pp) = &self.toks[j].kind {
                if *pp == p {
                    return Some(j);
                }
            }
        }
        None
    }

    fn match_angle(&self, lt: usize) -> Option<usize> {
        let mut depth = 0i32;
        let mut depth_brace = 0i32;
        let mut depth_paren = 0i32;
        let mut depth_brack = 0i32;
        for j in lt..self.toks.len() {
            match &self.toks[j].kind {
                TokenKind::Punct(Punct::Lt)
                    if depth_brace == 0 && depth_paren == 0 && depth_brack == 0 =>
                {
                    depth += 1
                }
                TokenKind::Punct(Punct::Gt)
                    if depth_brace == 0 && depth_paren == 0 && depth_brack == 0 =>
                {
                    depth -= 1;
                    if depth == 0 {
                        return Some(j);
                    }
                }
                TokenKind::Punct(Punct::Shr)
                    if depth_brace == 0 && depth_paren == 0 && depth_brack == 0 =>
                {
                    depth -= 2;
                    if depth <= 0 {
                        return Some(j);
                    }
                }
                TokenKind::Punct(Punct::UShr)
                    if depth_brace == 0 && depth_paren == 0 && depth_brack == 0 =>
                {
                    depth -= 3;
                    if depth <= 0 {
                        return Some(j);
                    }
                }
                TokenKind::Punct(Punct::LBrace) => depth_brace += 1,
                TokenKind::Punct(Punct::RBrace) if depth_brace > 0 => depth_brace -= 1,
                TokenKind::Punct(Punct::RBrace) => return None,
                TokenKind::Punct(Punct::LParen) => depth_paren += 1,
                TokenKind::Punct(Punct::RParen) if depth_paren > 0 => depth_paren -= 1,
                TokenKind::Punct(Punct::LBracket) => depth_brack += 1,
                TokenKind::Punct(Punct::RBracket) if depth_brack > 0 => depth_brack -= 1,
                TokenKind::Eof => return None,
                TokenKind::Punct(Punct::Semicolon)
                    if depth_brace == 0 && depth_paren == 0 && depth_brack == 0 =>
                {
                    return None
                }
                _ => {}
            }
        }
        None
    }

    fn match_braces(&self, lbrace: usize) -> Option<usize> {
        let mut depth = 0i32;
        for j in lbrace..self.toks.len() {
            match &self.toks[j].kind {
                TokenKind::Punct(Punct::LBrace) => depth += 1,
                TokenKind::Punct(Punct::RBrace) => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(j);
                    }
                }
                _ => {}
            }
        }
        None
    }

    fn find_stmt_end(&self, from: usize) -> Option<usize> {
        let mut depth_paren = 0i32;
        let mut depth_brace = 0i32;
        let mut depth_brack = 0i32;
        let mut j = from;
        while j < self.toks.len() {
            let k = &self.toks[j].kind;
            match k {
                TokenKind::Punct(Punct::LParen) => depth_paren += 1,
                TokenKind::Punct(Punct::RParen) => depth_paren -= 1,
                TokenKind::Punct(Punct::LBrace) => depth_brace += 1,
                TokenKind::Punct(Punct::RBrace) => {
                    if depth_brace == 0 {
                        return Some(j.saturating_sub(1).max(from));
                    }
                    depth_brace -= 1;
                }
                TokenKind::Punct(Punct::LBracket) => depth_brack += 1,
                TokenKind::Punct(Punct::RBracket) => depth_brack -= 1,
                TokenKind::Punct(Punct::Semicolon)
                    if depth_paren == 0 && depth_brace == 0 && depth_brack == 0 =>
                {
                    return Some(j);
                }
                TokenKind::Eof => return Some(j.saturating_sub(1).max(from)),
                _ => {}
            }
            j += 1;
        }
        Some(self.toks.len().saturating_sub(1))
    }

    fn is_expr_terminator(&self, i: usize) -> bool {
        match &self.toks[i].kind {
            TokenKind::Ident(_)
            | TokenKind::Number(_, _)
            | TokenKind::BigInt(_, _)
            | TokenKind::String(_)
            | TokenKind::Template { .. }
            | TokenKind::Punct(Punct::RParen)
            | TokenKind::Punct(Punct::RBracket)
            | TokenKind::Punct(Punct::RBrace) => true,
            _ => false,
        }
    }

    fn prev_allows_angle_assertion(&self, i: usize) -> bool {
        if i == 0 {
            return true;
        }
        let prev = &self.toks[i - 1].kind;
        match prev {
            TokenKind::Punct(
                Punct::LParen
                | Punct::LBracket
                | Punct::LBrace
                | Punct::Comma
                | Punct::Semicolon
                | Punct::Assign
                | Punct::Colon
                | Punct::Question
                | Punct::Arrow
                | Punct::LogicalAnd
                | Punct::LogicalOr
                | Punct::NullishCoalesce,
            ) => true,
            TokenKind::Ident(name)
                if matches!(
                    name.as_str(),
                    "return" | "throw" | "yield" | "await" | "case" | "typeof" | "delete" | "void"
                ) =>
            {
                true
            }
            _ => false,
        }
    }

    fn token_can_start_angle_asserted_expr(&self, i: usize) -> bool {
        matches!(
            self.toks[i].kind,
            TokenKind::Ident(_)
                | TokenKind::Number(_, _)
                | TokenKind::BigInt(_, _)
                | TokenKind::String(_)
                | TokenKind::Template { .. }
                | TokenKind::Punct(Punct::LBracket)
                | TokenKind::Punct(Punct::LBrace)
        )
    }

    fn next_is_postfix_context(&self, i: usize) -> bool {
        if i >= self.toks.len() {
            return true;
        }
        match &self.toks[i].kind {
            TokenKind::Eof
            | TokenKind::Punct(Punct::Semicolon)
            | TokenKind::Punct(Punct::Comma)
            | TokenKind::Punct(Punct::RParen)
            | TokenKind::Punct(Punct::RBracket)
            | TokenKind::Punct(Punct::RBrace)
            | TokenKind::Punct(Punct::Dot)
            | TokenKind::Punct(Punct::OptionalChain)

            | TokenKind::Punct(Punct::LParen)
            | TokenKind::Punct(Punct::LBracket)
            | TokenKind::Punct(Punct::Plus) | TokenKind::Punct(Punct::Minus)
            | TokenKind::Punct(Punct::Star) | TokenKind::Punct(Punct::Slash)
            | TokenKind::Punct(Punct::Percent) | TokenKind::Punct(Punct::StarStar)
            | TokenKind::Punct(Punct::Lt) | TokenKind::Punct(Punct::Gt)
            | TokenKind::Punct(Punct::Le) | TokenKind::Punct(Punct::Ge)
            | TokenKind::Punct(Punct::Eq) | TokenKind::Punct(Punct::Ne)
            | TokenKind::Punct(Punct::StrictEq) | TokenKind::Punct(Punct::StrictNe)
            | TokenKind::Punct(Punct::LogicalAnd) | TokenKind::Punct(Punct::LogicalOr)
            | TokenKind::Punct(Punct::NullishCoalesce)
            | TokenKind::Punct(Punct::Question)
            | TokenKind::Punct(Punct::Colon)
            => true,
            _ => false,
        }
    }

    fn is_annotation_colon(&self, i: usize) -> bool {
        if i == 0 {
            return false;
        }

        let in_obj_lit_at_own_level = matches!(self.brace_stack.last(),
            Some((BraceCtx::ObjectLit, push_pd)) if *push_pd == self.paren_depth);
        let in_obj_lit = in_obj_lit_at_own_level;

        let mut anchor = i - 1;

        if matches!(
            self.toks[anchor].kind,
            TokenKind::Punct(Punct::Question | Punct::LogicalNot)
        ) && anchor > 0
        {
            anchor -= 1;
        }
        let prev = &self.toks[anchor];
        let prev_is_close_paren = matches!(prev.kind, TokenKind::Punct(Punct::RParen));
        let prev_is_ident = matches!(prev.kind, TokenKind::Ident(_));
        let prev_is_close_brack = matches!(prev.kind, TokenKind::Punct(Punct::RBracket));
        let prev_is_close_brace = matches!(prev.kind, TokenKind::Punct(Punct::RBrace));
        if !(prev_is_close_paren || prev_is_ident || prev_is_close_brack || prev_is_close_brace) {
            return false;
        }

        if in_obj_lit && prev_is_ident {
            return false;
        }

        if prev_is_ident && matches!(&prev.kind, TokenKind::Ident(n) if n == "default") {
            return false;
        }
        if anchor > 0 {
            if let TokenKind::Ident(n) = &self.toks[anchor - 1].kind {
                if n == "case" {
                    return false;
                }
            }
        }

        if prev_is_close_brace || prev_is_close_brack {

            if matches!(self.brace_stack.last(),
                Some((BraceCtx::ObjectLit, push_pd)) if *push_pd == self.paren_depth)
            {
                return false;
            }
            let after = self.skip_type(i + 1);
            if after < self.toks.len() {
                return matches!(
                    self.toks[after].kind,
                    TokenKind::Punct(Punct::Comma)
                        | TokenKind::Punct(Punct::RParen)
                        | TokenKind::Punct(Punct::Assign)
                        | TokenKind::Eof
                );
            }
            return false;
        }

        if prev_is_close_paren {
            let after = self.skip_type(i + 1);
            if after < self.toks.len() {
                return matches!(
                    self.toks[after].kind,
                    TokenKind::Punct(Punct::LBrace)
                        | TokenKind::Punct(Punct::Arrow)
                        | TokenKind::Punct(Punct::Semicolon)
                        | TokenKind::Punct(Punct::Comma)
                        | TokenKind::Punct(Punct::Assign)
                        | TokenKind::Eof
                );
            }
            return false;
        }

        if prev_is_ident && anchor >= 1 {
            let two_back = &self.toks[anchor - 1].kind;
            if matches!(
                two_back,
                TokenKind::Punct(Punct::LBrace) | TokenKind::Punct(Punct::Comma)
            ) {

                let after = self.skip_type(i + 1);
                if after < self.toks.len() {
                    return matches!(
                        self.toks[after].kind,
                        TokenKind::Punct(Punct::Comma)
                            | TokenKind::Punct(Punct::RParen)
                            | TokenKind::Punct(Punct::RBrace)
                            | TokenKind::Punct(Punct::Semicolon)
                            | TokenKind::Punct(Punct::Assign)
                            | TokenKind::Eof
                    );
                }
                return true;
            }
        }
        true
    }

    fn skip_type(&self, mut i: usize) -> usize {
        let mut depth_angle = 0i32;
        let mut depth_paren = 0i32;
        let mut depth_brace = 0i32;
        let mut depth_brack = 0i32;
        let start = i;

        let mut prev_was_rparen_at_top = false;
        while i < self.toks.len() {
            let at_top =
                depth_angle == 0 && depth_paren == 0 && depth_brace == 0 && depth_brack == 0;

            if matches!(self.toks[i].kind, TokenKind::Eof) {
                break;
            }
            if matches!(self.toks[i].kind, TokenKind::Template { .. }) {
                break;
            }
            if at_top && matches!(self.toks[i].kind, TokenKind::Punct(Punct::Semicolon)) {
                break;
            }
            if at_top {

                if i > start && self.toks[i].preceded_by_line_terminator {
                    break;
                }

                match &self.toks[i].kind {
                    TokenKind::Punct(Punct::Comma)
                    | TokenKind::Punct(Punct::Assign)
                    | TokenKind::Punct(Punct::RParen)
                    | TokenKind::Punct(Punct::RBrace)
                    | TokenKind::Punct(Punct::RBracket) => break,
                    TokenKind::Punct(Punct::Arrow) => {
                        if !prev_was_rparen_at_top {
                            break;
                        }

                    }
                    TokenKind::Punct(Punct::LBrace) => {

                        let prev_is_type_op = i > 0
                            && matches!(
                                self.toks[i - 1].kind,
                                TokenKind::Punct(Punct::BitAnd) | TokenKind::Punct(Punct::BitOr)
                            );
                        if i == start || prev_is_type_op {
                            depth_brace += 1;
                        } else {
                            break;
                        }
                    }
                    _ => {}
                }
            }
            match &self.toks[i].kind {
                TokenKind::Punct(Punct::Lt) => depth_angle += 1,
                TokenKind::Punct(Punct::Gt) => depth_angle = (depth_angle - 1).max(0),
                TokenKind::Punct(Punct::Shr) => depth_angle = (depth_angle - 2).max(0),
                TokenKind::Punct(Punct::UShr) => depth_angle = (depth_angle - 3).max(0),
                TokenKind::Punct(Punct::LParen) => depth_paren += 1,
                TokenKind::Punct(Punct::RParen) if depth_paren > 0 => depth_paren -= 1,
                TokenKind::Punct(Punct::LBrace) if !at_top => depth_brace += 1,
                TokenKind::Punct(Punct::RBrace) if depth_brace > 0 => depth_brace -= 1,
                TokenKind::Punct(Punct::LBracket) => depth_brack += 1,
                TokenKind::Punct(Punct::RBracket) if depth_brack > 0 => depth_brack -= 1,
                _ => {}
            }

            prev_was_rparen_at_top = matches!(self.toks[i].kind, TokenKind::Punct(Punct::RParen))
                && depth_angle == 0
                && depth_paren == 0
                && depth_brace == 0
                && depth_brack == 0;
            i += 1;
        }
        i
    }
}
