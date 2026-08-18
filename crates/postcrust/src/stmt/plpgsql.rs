
use crate::catalog::{Catalog, FunctionDef, Lang};
use crate::expr::ast::Expr;
use crate::expr::eval::{eval_ctx, EvalCtx};
use crate::types::registry::TypeRegistries;
use crate::types::{self, PgError};
use sql_core::SqlValue;
use std::cell::Cell;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq)]
pub struct PlBlock {
    pub decls: Vec<PlDecl>,
    pub body: Vec<PlStmt>,

    pub handlers: Vec<Handler>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Handler {
    pub conditions: Vec<Cond>,
    pub body: Vec<PlStmt>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Cond {
    Named(String),
    SqlState(String),
}

fn cond_sqlstate(name: &str) -> Option<&'static str> {
    Some(match name {
        "unique_violation" => "23505",
        "foreign_key_violation" => "23503",
        "not_null_violation" => "23502",
        "check_violation" => "23514",
        "integrity_constraint_violation" => "23000",
        "restrict_violation" => "23001",
        "division_by_zero" => "22012",
        "numeric_value_out_of_range" => "22003",
        "string_data_right_truncation" => "22001",
        "invalid_text_representation" => "22P02",
        "case_not_found" => "20000",
        "no_data_found" => "P0002",
        "too_many_rows" => "P0003",
        "raise_exception" => "P0001",
        "cannot_coerce" => "42846",
        "syntax_error" => "42601",
        "undefined_column" => "42703",
        "undefined_table" => "42P01",
        "undefined_object" => "42704",
        "undefined_function" => "42883",
        "duplicate_table" => "42P07",
        "serialization_failure" => "40001",
        "lock_not_available" => "55P03",
        "object_not_in_prerequisite_state" => "55000",
        "generated_always" => "428C9",
        "in_failed_sql_transaction" => "25P02",
        "active_sql_transaction" => "25001",
        _ => return None,
    })
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlDecl {
    pub name: String,

    pub typ: String,
    pub default: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AssignTarget {
    Var(String),
    NewField(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RaiseLevel {
    Exception,
    Notice,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PlStmt {

    Assign { target: AssignTarget, expr: String },

    If {
        arms: Vec<(String, Vec<PlStmt>)>,
        els: Option<Vec<PlStmt>>,
    },

    Return(String),

    Raise {
        level: RaiseLevel,
        fmt: String,
        args: Vec<String>,
    },

    Sql(String),

    SelectInto { query: String, targets: Vec<String> },

    Loop {
        label: Option<String>,
        body: Vec<PlStmt>,
    },

    While {
        label: Option<String>,
        cond: String,
        body: Vec<PlStmt>,
    },

    ForRange {
        label: Option<String>,
        var: String,
        reverse: bool,
        lo: String,
        hi: String,
        step: Option<String>,
        body: Vec<PlStmt>,
    },

    Exit {
        label: Option<String>,
        when: Option<String>,
    },

    Continue {
        label: Option<String>,
        when: Option<String>,
    },

    Case {
        operand: Option<String>,
        arms: Vec<(Vec<String>, Vec<PlStmt>)>,
        els: Option<Vec<PlStmt>>,
    },

    Block(PlBlock),
}

#[derive(Debug, Clone, PartialEq)]
enum L {
    Word(String),
    Str(String),
    Assign,
    Semi,
    LParen,
    RParen,
    Comma,
    Other,
    Eof,
}

struct Lex {
    tok: L,
    start: usize,
}

fn err(msg: impl Into<String>) -> PgError {
    PgError::InvalidInputSyntax {
        typ: "query",
        input: msg.into(),
    }
}

fn lex(chars: &[char]) -> Result<Vec<Lex>, PgError> {
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
            continue;
        }

        if c == '-' && i + 1 < chars.len() && chars[i + 1] == '-' {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
            continue;
        }
        let start = i;
        if c == '\'' {
            let mut s = String::new();
            i += 1;
            loop {
                if i >= chars.len() {
                    return Err(err("unterminated string in PL/pgSQL body"));
                }
                if chars[i] == '\'' {
                    if i + 1 < chars.len() && chars[i + 1] == '\'' {
                        s.push('\'');
                        i += 2;
                        continue;
                    }
                    i += 1;
                    break;
                }
                s.push(chars[i]);
                i += 1;
            }
            out.push(Lex {
                tok: L::Str(s),
                start,
            });
            continue;
        }
        if c.is_alphabetic() || c == '_' {
            while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            let word: String = chars[start..i].iter().collect();
            out.push(Lex {
                tok: L::Word(word.to_ascii_lowercase()),
                start,
            });
            continue;
        }
        if c == ':' && i + 1 < chars.len() && chars[i + 1] == '=' {
            i += 2;
            out.push(Lex {
                tok: L::Assign,
                start,
            });
            continue;
        }
        match c {
            ';' => {
                i += 1;
                out.push(Lex {
                    tok: L::Semi,
                    start,
                });
            }
            '(' => {
                i += 1;
                out.push(Lex {
                    tok: L::LParen,
                    start,
                });
            }
            ')' => {
                i += 1;
                out.push(Lex {
                    tok: L::RParen,
                    start,
                });
            }
            ',' => {
                i += 1;
                out.push(Lex {
                    tok: L::Comma,
                    start,
                });
            }
            _ => {

                i += 1;
                out.push(Lex {
                    tok: L::Other,
                    start,
                });
            }
        }
    }
    out.push(Lex {
        tok: L::Eof,
        start: chars.len(),
    });
    Ok(out)
}

struct P<'a> {
    chars: &'a [char],
    toks: Vec<Lex>,
    pos: usize,
}

impl<'a> P<'a> {
    fn peek(&self) -> &L {
        &self.toks[self.pos].tok
    }
    fn at_word(&self, w: &str) -> bool {
        matches!(self.peek(), L::Word(s) if s == w)
    }
    fn eat_word(&mut self, w: &str) -> bool {
        if self.at_word(w) {
            self.pos += 1;
            true
        } else {
            false
        }
    }
    fn expect_word(&mut self, w: &str) -> Result<(), PgError> {
        if self.eat_word(w) {
            Ok(())
        } else {
            Err(err(format!("PL/pgSQL: expected `{w}`")))
        }
    }
    fn start_of(&self, pos: usize) -> usize {
        self.toks[pos].start
    }

    fn slice(&self, from: usize, to: usize) -> String {
        let a = self.start_of(from);
        let b = self.start_of(to);
        self.chars[a..b]
            .iter()
            .collect::<String>()
            .trim()
            .to_string()
    }
}

pub fn parse_block(src: &str) -> Result<PlBlock, PgError> {
    let chars: Vec<char> = src.chars().collect();
    let toks = lex(&chars)?;
    let mut p = P {
        chars: &chars,
        toks,
        pos: 0,
    };

    let block = p.parse_block_body()?;

    let _ = matches!(p.peek(), L::Semi);
    Ok(block)
}

impl<'a> P<'a> {
    fn expect_semi(&mut self) -> Result<(), PgError> {
        if matches!(self.peek(), L::Semi) {
            self.pos += 1;
            Ok(())
        } else {
            Err(err("PL/pgSQL: expected `;`"))
        }
    }

    fn parse_block_body(&mut self) -> Result<PlBlock, PgError> {
        let mut decls = Vec::new();
        if self.eat_word("declare") {
            while !self.at_word("begin") {
                if matches!(self.peek(), L::Eof) {
                    return Err(err("PL/pgSQL: DECLARE without BEGIN"));
                }

                let name = match self.peek() {
                    L::Word(s) => {
                        let n = s.clone();
                        self.pos += 1;
                        n
                    }
                    _ => return Err(err("PL/pgSQL: expected declaration name")),
                };

                let type_from = self.pos;
                while !matches!(self.peek(), L::Assign | L::Semi | L::Eof)
                    && !self.at_word("default")
                {
                    self.pos += 1;
                }
                let typ = self.slice(type_from, self.pos).to_ascii_lowercase();
                let typ = typ.split_whitespace().collect::<Vec<_>>().join(" ");
                let mut default = None;
                if self.eat_word("default") || matches!(self.peek(), L::Assign) {

                    if matches!(self.peek(), L::Assign) {
                        self.pos += 1;
                    }
                    let from = self.pos;
                    while !matches!(self.peek(), L::Semi | L::Eof) {
                        self.pos += 1;
                    }
                    default = Some(self.slice(from, self.pos));
                }
                self.expect_semi()?;
                decls.push(PlDecl { name, typ, default });
            }
        }
        self.expect_word("begin")?;
        let body = self.parse_stmts(&["exception", "end"])?;
        let handlers = if self.eat_word("exception") {
            self.parse_handlers()?
        } else {
            Vec::new()
        };
        self.expect_word("end")?;

        if let L::Word(w) = self.peek() {

            let _ = w;
            self.pos += 1;
        }
        Ok(PlBlock {
            decls,
            body,
            handlers,
        })
    }

    fn parse_handlers(&mut self) -> Result<Vec<Handler>, PgError> {
        let mut handlers = Vec::new();
        while self.eat_word("when") {
            let mut conditions = Vec::new();
            loop {
                if self.eat_word("sqlstate") {
                    match self.peek() {
                        L::Str(s) => {
                            conditions.push(Cond::SqlState(s.clone()));
                            self.pos += 1;
                        }
                        _ => return Err(err("PL/pgSQL: SQLSTATE requires a string literal")),
                    }
                } else if let L::Word(w) = self.peek() {
                    conditions.push(Cond::Named(w.clone()));
                    self.pos += 1;
                } else {
                    return Err(err(
                        "PL/pgSQL: expected a condition name in EXCEPTION handler",
                    ));
                }
                if !self.eat_word("or") {
                    break;
                }
            }
            self.expect_word("then")?;
            let body = self.parse_stmts(&["when", "end"])?;
            handlers.push(Handler { conditions, body });
        }
        if handlers.is_empty() {
            return Err(err("PL/pgSQL: EXCEPTION with no WHEN handler"));
        }
        Ok(handlers)
    }

    fn parse_label(&mut self) -> Option<String> {

        let save = self.pos;
        if self.is_other_char('<') && self.next_is_other_char('<') {
            self.pos += 2;
            let name = match self.peek() {
                L::Word(s) => {
                    let n = s.clone();
                    self.pos += 1;
                    n
                }
                _ => {
                    self.pos = save;
                    return None;
                }
            };
            if self.is_other_char('>') && self.next_is_other_char('>') {
                self.pos += 2;
                return Some(name);
            }
            self.pos = save;
        }
        None
    }

    fn is_other_char(&self, c: char) -> bool {
        matches!(self.peek(), L::Other) && self.chars.get(self.start_of(self.pos)) == Some(&c)
    }
    fn next_is_other_char(&self, c: char) -> bool {
        matches!(self.toks.get(self.pos + 1).map(|t| &t.tok), Some(L::Other))
            && self.chars.get(self.start_of(self.pos + 1)) == Some(&c)
    }

    fn parse_stmts(&mut self, stops: &[&str]) -> Result<Vec<PlStmt>, PgError> {
        let mut out = Vec::new();
        loop {
            if matches!(self.peek(), L::Eof) {
                break;
            }
            if let L::Word(w) = self.peek() {
                if stops.contains(&w.as_str()) {
                    break;
                }
            }
            out.push(self.parse_stmt()?);
        }
        Ok(out)
    }

    fn parse_stmt(&mut self) -> Result<PlStmt, PgError> {

        let label = self.parse_label();
        if self.at_word("loop") {
            return self.parse_loop(label);
        }
        if self.at_word("while") {
            return self.parse_while(label);
        }
        if self.at_word("for") {
            return self.parse_for(label);
        }
        if label.is_some() {
            return Err(err("PL/pgSQL: a label may only precede LOOP/WHILE/FOR"));
        }

        if self.at_word("declare") || self.at_word("begin") {
            let block = self.parse_block_body()?;
            self.expect_semi()?;
            return Ok(PlStmt::Block(block));
        }
        if self.at_word("case") {
            return self.parse_case();
        }
        if self.at_word("exit") || self.at_word("continue") {
            return self.parse_exit_continue();
        }
        if self.at_word("if") {
            return self.parse_if();
        }
        if self.eat_word("return") {
            let from = self.pos;
            while !matches!(self.peek(), L::Semi | L::Eof) {
                self.pos += 1;
            }
            let expr = self.slice(from, self.pos);
            self.expect_semi()?;
            return Ok(PlStmt::Return(expr));
        }
        if self.eat_word("raise") {
            return self.parse_raise();
        }

        if let L::Word(w) = self.peek() {
            let w = w.clone();
            match w.as_str() {
                "perform" => {

                    let from = self.pos;
                    self.pos += 1;
                    while !matches!(self.peek(), L::Semi | L::Eof) {
                        self.pos += 1;
                    }
                    let raw = self.slice(from, self.pos);
                    self.expect_semi()?;

                    let rest = raw
                        .get(
                            raw.char_indices()
                                .nth(7)
                                .map(|(b, _)| b)
                                .unwrap_or(raw.len())..,
                        )
                        .unwrap_or("");
                    return Ok(PlStmt::Sql(format!("select {}", rest.trim())));
                }
                "insert" | "update" | "delete" => {
                    let from = self.pos;
                    while !matches!(self.peek(), L::Semi | L::Eof) {
                        self.pos += 1;
                    }
                    let sql = self.slice(from, self.pos);
                    self.expect_semi()?;
                    return Ok(PlStmt::Sql(sql));
                }
                "select" => {
                    return self.parse_select_into();
                }
                _ => {}
            }
        }

        self.parse_assign()
    }

    fn parse_if(&mut self) -> Result<PlStmt, PgError> {
        self.expect_word("if")?;
        let mut arms = Vec::new();

        let cond = self.take_until_word("then")?;
        self.expect_word("then")?;
        let body = self.parse_stmts(&["elsif", "elseif", "else", "end"])?;
        arms.push((cond, body));
        loop {
            if self.eat_word("elsif") || self.eat_word("elseif") {
                let cond = self.take_until_word("then")?;
                self.expect_word("then")?;
                let body = self.parse_stmts(&["elsif", "elseif", "else", "end"])?;
                arms.push((cond, body));
            } else {
                break;
            }
        }
        let els = if self.eat_word("else") {
            Some(self.parse_stmts(&["end"])?)
        } else {
            None
        };
        self.expect_word("end")?;
        self.expect_word("if")?;
        self.expect_semi()?;
        Ok(PlStmt::If { arms, els })
    }

    fn parse_loop(&mut self, label: Option<String>) -> Result<PlStmt, PgError> {
        self.expect_word("loop")?;
        let body = self.parse_stmts(&["end"])?;
        self.expect_word("end")?;
        self.expect_word("loop")?;
        self.skip_trailing_label();
        self.expect_semi()?;
        Ok(PlStmt::Loop { label, body })
    }

    fn parse_while(&mut self, label: Option<String>) -> Result<PlStmt, PgError> {
        self.expect_word("while")?;
        let cond = self.take_until_word("loop")?;
        self.expect_word("loop")?;
        let body = self.parse_stmts(&["end"])?;
        self.expect_word("end")?;
        self.expect_word("loop")?;
        self.skip_trailing_label();
        self.expect_semi()?;
        Ok(PlStmt::While { label, cond, body })
    }

    fn parse_for(&mut self, label: Option<String>) -> Result<PlStmt, PgError> {
        self.expect_word("for")?;
        let var = match self.peek() {
            L::Word(s) => {
                let n = s.clone();
                self.pos += 1;
                n
            }
            _ => return Err(err("PL/pgSQL: expected FOR loop variable")),
        };
        self.expect_word("in")?;
        let reverse = self.eat_word("reverse");

        let lo = self.take_until_dotdot()?;

        self.pos += 2;
        let hi = self.take_until_word_any(&["by", "loop"])?;
        let step = if self.eat_word("by") {
            Some(self.take_until_word("loop")?)
        } else {
            None
        };
        self.expect_word("loop")?;
        let body = self.parse_stmts(&["end"])?;
        self.expect_word("end")?;
        self.expect_word("loop")?;
        self.skip_trailing_label();
        self.expect_semi()?;
        Ok(PlStmt::ForRange {
            label,
            var,
            reverse,
            lo,
            hi,
            step,
            body,
        })
    }

    fn parse_exit_continue(&mut self) -> Result<PlStmt, PgError> {
        let is_exit = self.eat_word("exit");
        if !is_exit {
            self.expect_word("continue")?;
        }

        let label = match self.peek() {
            L::Word(w) if w != "when" => {
                let n = w.clone();
                self.pos += 1;
                Some(n)
            }
            _ => None,
        };
        let when = if self.eat_word("when") {
            Some(self.take_until_semi())
        } else {
            None
        };
        self.expect_semi()?;
        if is_exit {
            Ok(PlStmt::Exit { label, when })
        } else {
            Ok(PlStmt::Continue { label, when })
        }
    }

    fn parse_case(&mut self) -> Result<PlStmt, PgError> {
        self.expect_word("case")?;

        let operand = if self.at_word("when") {
            None
        } else {
            Some(self.take_until_word("when")?)
        };
        let mut arms = Vec::new();
        while self.eat_word("when") {
            let tests = if operand.is_some() {
                self.take_exprs_until_word("then")?
            } else {
                vec![self.take_until_word("then")?]
            };
            self.expect_word("then")?;
            let body = self.parse_stmts(&["when", "else", "end"])?;
            arms.push((tests, body));
        }
        let els = if self.eat_word("else") {
            Some(self.parse_stmts(&["end"])?)
        } else {
            None
        };
        self.expect_word("end")?;
        self.expect_word("case")?;
        self.expect_semi()?;
        Ok(PlStmt::Case { operand, arms, els })
    }

    fn skip_trailing_label(&mut self) {
        if matches!(self.peek(), L::Word(_)) {
            self.pos += 1;
        }
    }

    fn take_until_semi(&mut self) -> String {
        let from = self.pos;
        while !matches!(self.peek(), L::Semi | L::Eof) {
            self.pos += 1;
        }
        self.slice(from, self.pos)
    }

    fn take_until_dotdot(&mut self) -> Result<String, PgError> {
        let from = self.pos;
        let mut depth = 0i32;
        loop {
            match self.peek() {
                L::Eof => return Err(err("PL/pgSQL: expected `..` in FOR range")),
                L::LParen => depth += 1,
                L::RParen if depth > 0 => depth -= 1,
                _ if depth == 0 && self.is_other_char('.') && self.next_is_other_char('.') => break,
                _ => {}
            }
            self.pos += 1;
        }
        Ok(self.slice(from, self.pos))
    }

    fn take_until_word_any(&mut self, words: &[&str]) -> Result<String, PgError> {
        let from = self.pos;
        let mut depth = 0i32;
        loop {
            match self.peek() {
                L::Eof => return Err(err(format!("PL/pgSQL: expected one of {words:?}"))),
                L::LParen => depth += 1,
                L::RParen if depth > 0 => depth -= 1,
                L::Word(w) if depth == 0 && words.contains(&w.as_str()) => break,
                _ => {}
            }
            self.pos += 1;
        }
        Ok(self.slice(from, self.pos))
    }

    fn take_exprs_until_word(&mut self, word: &str) -> Result<Vec<String>, PgError> {
        let mut out = Vec::new();
        let mut from = self.pos;
        let mut depth = 0i32;
        loop {
            match self.peek() {
                L::Eof => return Err(err(format!("PL/pgSQL: expected `{word}`"))),
                L::LParen => depth += 1,
                L::RParen if depth > 0 => depth -= 1,
                L::Comma if depth == 0 => {
                    out.push(self.slice(from, self.pos));
                    self.pos += 1;
                    from = self.pos;
                    continue;
                }
                L::Word(w) if depth == 0 && w == word => {
                    out.push(self.slice(from, self.pos));
                    break;
                }
                _ => {}
            }
            self.pos += 1;
        }
        Ok(out)
    }

    fn take_until_word(&mut self, word: &str) -> Result<String, PgError> {
        let from = self.pos;
        let mut depth = 0i32;
        loop {
            match self.peek() {
                L::Eof => return Err(err(format!("PL/pgSQL: expected `{word}`"))),
                L::LParen => depth += 1,
                L::RParen if depth > 0 => depth -= 1,
                L::Word(w) if depth == 0 && w == word => break,
                _ => {}
            }
            self.pos += 1;
        }
        Ok(self.slice(from, self.pos))
    }

    fn parse_raise(&mut self) -> Result<PlStmt, PgError> {

        let level = if self.eat_word("exception") {
            RaiseLevel::Exception
        } else if self.eat_word("notice")
            || self.eat_word("warning")
            || self.eat_word("info")
            || self.eat_word("log")
            || self.eat_word("debug")
        {
            RaiseLevel::Notice
        } else {

            RaiseLevel::Exception
        };
        let fmt = match self.peek() {
            L::Str(s) => {
                let s = s.clone();
                self.pos += 1;
                s
            }
            _ => return Err(err("PL/pgSQL: RAISE requires a format string")),
        };
        let mut args = Vec::new();
        while matches!(self.peek(), L::Comma) {
            self.pos += 1;
            let from = self.pos;
            let mut depth = 0i32;
            loop {
                match self.peek() {
                    L::Eof | L::Semi => break,
                    L::Comma if depth == 0 => break,
                    L::Word(w) if depth == 0 && w == "using" => break,
                    L::LParen => depth += 1,
                    L::RParen if depth > 0 => depth -= 1,
                    _ => {}
                }
                self.pos += 1;
            }
            args.push(self.slice(from, self.pos));
        }

        while !matches!(self.peek(), L::Semi | L::Eof) {
            self.pos += 1;
        }
        self.expect_semi()?;
        Ok(PlStmt::Raise { level, fmt, args })
    }

    fn parse_assign(&mut self) -> Result<PlStmt, PgError> {

        let first = match self.peek() {
            L::Word(s) => {
                let n = s.clone();
                self.pos += 1;
                n
            }
            _ => return Err(err("PL/pgSQL: unrecognized statement")),
        };

        let target = if matches!(self.peek(), L::Other)
            && self.chars.get(self.start_of(self.pos)) == Some(&'.')
        {

            self.pos += 1;
            let field = match self.peek() {
                L::Word(s) => {
                    let n = s.clone();
                    self.pos += 1;
                    n
                }
                _ => return Err(err("PL/pgSQL: expected field name after `.`")),
            };
            if first == "new" {
                AssignTarget::NewField(field)
            } else {
                return Err(err(format!(
                    "PL/pgSQL: cannot assign to `{first}.{field}` (only NEW.col is writable)"
                )));
            }
        } else {
            AssignTarget::Var(first)
        };
        if !matches!(self.peek(), L::Assign) {
            return Err(err("PL/pgSQL: expected `:=` in assignment"));
        }
        self.pos += 1;
        let from = self.pos;
        while !matches!(self.peek(), L::Semi | L::Eof) {
            self.pos += 1;
        }
        let expr = self.slice(from, self.pos);
        self.expect_semi()?;
        Ok(PlStmt::Assign { target, expr })
    }

    fn parse_select_into(&mut self) -> Result<PlStmt, PgError> {

        let from = self.pos;
        self.pos += 1;

        let sel_start = self.pos;
        let mut depth = 0i32;
        let mut into_pos = None;
        while !matches!(self.peek(), L::Semi | L::Eof) {
            match self.peek() {
                L::LParen => depth += 1,
                L::RParen if depth > 0 => depth -= 1,
                L::Word(w) if depth == 0 && w == "into" => {
                    into_pos = Some(self.pos);
                    break;
                }
                _ => {}
            }
            self.pos += 1;
        }
        let Some(into_pos) = into_pos else {

            while !matches!(self.peek(), L::Semi | L::Eof) {
                self.pos += 1;
            }
            let sql = self.slice(from, self.pos);
            self.expect_semi()?;
            return Ok(PlStmt::Sql(sql));
        };
        let sel_list = self.slice(sel_start, into_pos);

        self.pos = into_pos + 1;
        let mut targets = Vec::new();
        loop {
            match self.peek() {
                L::Word(s) => {
                    targets.push(s.clone());
                    self.pos += 1;
                }
                _ => break,
            }
            if matches!(self.peek(), L::Comma) {
                self.pos += 1;
            } else {
                break;
            }
        }

        let rest_start = self.pos;
        while !matches!(self.peek(), L::Semi | L::Eof) {
            self.pos += 1;
        }
        let rest = self.slice(rest_start, self.pos);
        self.expect_semi()?;
        let query = format!("select {} {}", sel_list.trim(), rest.trim());
        Ok(PlStmt::SelectInto {
            query: query.trim().to_string(),
            targets,
        })
    }
}

pub struct PlEnv {
    pub col_names: Vec<String>,
    pub col_oids: Vec<u32>,
    pub col_typmods: Vec<i32>,
    pub new: Option<Vec<SqlValue>>,
    pub old: Option<Vec<SqlValue>>,
    pub tg: Vec<(String, String)>,
    pub vars: HashMap<String, SqlValue>,

    pub var_types: HashMap<String, (u32, i32)>,

    pub is_function: bool,
}

impl PlEnv {
    fn col_index(&self, name: &str) -> Option<usize> {
        self.col_names.iter().position(|n| n == name)
    }
}

const LOOP_CAP: u64 = 10_000_000;

enum Flow {
    Normal,
    Return(Option<Vec<SqlValue>>),
    Exit(Option<String>),
    Continue(Option<String>),
}

pub enum PlOutcome {
    Return(Option<Vec<SqlValue>>),
    Fell,
}

pub fn exec_block(
    block: &PlBlock,
    env: &mut PlEnv,
    catalog: &mut Catalog,
    regs: &Arc<TypeRegistries>,
) -> Result<PlOutcome, PgError> {
    match run_block(block, env, catalog, regs)? {
        Flow::Return(row) => Ok(PlOutcome::Return(row)),

        _ => Ok(PlOutcome::Fell),
    }
}

fn run_block(
    block: &PlBlock,
    env: &mut PlEnv,
    catalog: &mut Catalog,
    regs: &Arc<TypeRegistries>,
) -> Result<Flow, PgError> {

    let mut saved: Vec<(String, Option<SqlValue>, Option<(u32, i32)>)> = Vec::new();
    for d in &block.decls {
        let oid = crate::types::oid_for_type_name(&d.typ);
        let tm = types::typmod::NONE;
        let raw = match &d.default {
            Some(src) => eval_expr(src, env, regs)?,
            None => SqlValue::Null,
        };
        let v = match oid {
            Some(o) => crate::stmt::ddl::coerce(raw, Some(o), tm, regs)?,
            None => raw,
        };
        saved.push((
            d.name.clone(),
            env.vars.remove(&d.name),
            env.var_types.remove(&d.name),
        ));
        if let Some(o) = oid {
            env.var_types.insert(d.name.clone(), (o, tm));
        }
        env.vars.insert(d.name.clone(), v);
    }
    let flow = run_body_with_handlers(block, env, catalog, regs);

    for (name, oldv, oldt) in saved.into_iter().rev() {
        match oldv {
            Some(v) => {
                env.vars.insert(name.clone(), v);
            }
            None => {
                env.vars.remove(&name);
            }
        }
        match oldt {
            Some(t) => {
                env.var_types.insert(name, t);
            }
            None => {
                env.var_types.remove(&name);
            }
        }
    }
    flow
}

fn run_body_with_handlers(
    block: &PlBlock,
    env: &mut PlEnv,
    catalog: &mut Catalog,
    regs: &Arc<TypeRegistries>,
) -> Result<Flow, PgError> {
    if block.handlers.is_empty() {
        return exec_stmts(&block.body, env, catalog, regs);
    }
    let snap = catalog.stmt_snapshot();
    match exec_stmts(&block.body, env, catalog, regs) {
        Ok(flow) => Ok(flow),
        Err(e) => {
            let state = e.sqlstate();
            let Some(handler) = match_handler(&block.handlers, state) else {

                return Err(e);
            };

            catalog.stmt_restore(&snap);

            let saved_state = env
                .vars
                .insert("sqlstate".to_string(), SqlValue::Text(state.to_string()));
            let saved_errm = env
                .vars
                .insert("sqlerrm".to_string(), SqlValue::Text(e.message()));
            let hflow = exec_stmts(&handler.body, env, catalog, regs);

            match saved_state {
                Some(v) => {
                    env.vars.insert("sqlstate".to_string(), v);
                }
                None => {
                    env.vars.remove("sqlstate");
                }
            }
            match saved_errm {
                Some(v) => {
                    env.vars.insert("sqlerrm".to_string(), v);
                }
                None => {
                    env.vars.remove("sqlerrm");
                }
            }
            hflow
        }
    }
}

fn match_handler<'a>(handlers: &'a [Handler], state: &str) -> Option<&'a Handler> {
    handlers.iter().find(|h| {
        h.conditions.iter().any(|c| match c {
            Cond::SqlState(code) => code.eq_ignore_ascii_case(state),
            Cond::Named(name) if name == "others" => true,
            Cond::Named(name) => cond_sqlstate(name) == Some(state),
        })
    })
}

fn exec_stmts(
    stmts: &[PlStmt],
    env: &mut PlEnv,
    catalog: &mut Catalog,
    regs: &Arc<TypeRegistries>,
) -> Result<Flow, PgError> {
    for s in stmts {
        match exec_stmt(s, env, catalog, regs)? {
            Flow::Normal => {}
            flow => return Ok(flow),
        }
    }
    Ok(Flow::Normal)
}

fn targets(label: &Option<String>, own: &Option<String>) -> bool {
    match label {
        None => true,
        Some(l) => own.as_deref() == Some(l.as_str()),
    }
}

fn assign_local(
    env: &mut PlEnv,
    name: &str,
    v: SqlValue,
    regs: &Arc<TypeRegistries>,
) -> Result<(), PgError> {
    let coerced = match env.var_types.get(name).copied() {
        Some((oid, tm)) => crate::stmt::ddl::coerce(v, Some(oid), tm, regs)?,
        None => v,
    };
    env.vars.insert(name.to_string(), coerced);
    Ok(())
}

fn exec_stmt(
    s: &PlStmt,
    env: &mut PlEnv,
    catalog: &mut Catalog,
    regs: &Arc<TypeRegistries>,
) -> Result<Flow, PgError> {
    match s {
        PlStmt::Assign { target, expr } => {
            let v = eval_expr(expr, env, regs)?;
            match target {
                AssignTarget::Var(name) => {
                    assign_local(env, name, v, regs)?;
                }
                AssignTarget::NewField(col) => {
                    let idx = env
                        .col_index(col)
                        .ok_or_else(|| err(format!("record \"new\" has no field \"{col}\"")))?;
                    let oid = env.col_oids.get(idx).copied();
                    let tm = env
                        .col_typmods
                        .get(idx)
                        .copied()
                        .unwrap_or(types::typmod::NONE);
                    let coerced = crate::stmt::ddl::coerce(v, oid, tm, regs)?;
                    if let Some(row) = env.new.as_mut() {
                        if idx < row.len() {
                            row[idx] = coerced;
                        }
                    }
                }
            }
            Ok(Flow::Normal)
        }
        PlStmt::If { arms, els } => {
            for (cond, body) in arms {
                if truthy(&eval_expr(cond, env, regs)?) {
                    return exec_stmts(body, env, catalog, regs);
                }
            }
            if let Some(body) = els {
                return exec_stmts(body, env, catalog, regs);
            }
            Ok(Flow::Normal)
        }
        PlStmt::Return(src) => {
            let t = src.trim().to_ascii_lowercase();
            let row = if t.is_empty() || t == "null" {
                None
            } else if !env.is_function && t == "new" {
                env.new.clone()
            } else if !env.is_function && t == "old" {
                env.old.clone()
            } else if env.is_function {

                match eval_expr_mut(src, env, catalog, regs)? {
                    SqlValue::Null => None,
                    v => Some(vec![v]),
                }
            } else {

                match eval_expr(src, env, regs)? {
                    SqlValue::Null => None,
                    _ => {
                        return Err(err(
                            "PL/pgSQL: only RETURN NEW|OLD|NULL is supported in a trigger",
                        ))
                    }
                }
            };
            Ok(Flow::Return(row))
        }
        PlStmt::Raise { level, fmt, args } => {
            let msg = format_raise(fmt, args, env, regs)?;
            match level {
                RaiseLevel::Exception => Err(PgError::RaiseException { message: msg }),
                RaiseLevel::Notice => Ok(Flow::Normal),
            }
        }
        PlStmt::Sql(src) => {
            let sql = substitute_vars(src, env);
            crate::stmt::run_mut(&sql, catalog)?;
            Ok(Flow::Normal)
        }
        PlStmt::SelectInto { query, targets } => {
            let sql = substitute_vars(query, env);
            let res = crate::stmt::lower::run(&sql, catalog)?;
            let first = res.rows.first();
            for (i, t) in targets.iter().enumerate() {
                let v = first
                    .and_then(|r| r.get(i))
                    .cloned()
                    .unwrap_or(SqlValue::Null);
                assign_local(env, t, v, regs)?;
            }
            Ok(Flow::Normal)
        }
        PlStmt::Block(block) => run_block(block, env, catalog, regs),
        PlStmt::Loop { label, body } => {
            let mut n = 0u64;
            loop {
                loop_guard(&mut n)?;
                match exec_stmts(body, env, catalog, regs)? {
                    Flow::Normal => {}
                    Flow::Return(r) => return Ok(Flow::Return(r)),
                    Flow::Exit(l) => {
                        if targets(&l, label) {
                            break;
                        }
                        return Ok(Flow::Exit(l));
                    }
                    Flow::Continue(l) => {
                        if targets(&l, label) {
                            continue;
                        }
                        return Ok(Flow::Continue(l));
                    }
                }
            }
            Ok(Flow::Normal)
        }
        PlStmt::While { label, cond, body } => {
            let mut n = 0u64;
            while truthy(&eval_expr(cond, env, regs)?) {
                loop_guard(&mut n)?;
                match exec_stmts(body, env, catalog, regs)? {
                    Flow::Normal => {}
                    Flow::Return(r) => return Ok(Flow::Return(r)),
                    Flow::Exit(l) => {
                        if targets(&l, label) {
                            break;
                        }
                        return Ok(Flow::Exit(l));
                    }
                    Flow::Continue(l) => {
                        if targets(&l, label) {
                            continue;
                        }
                        return Ok(Flow::Continue(l));
                    }
                }
            }
            Ok(Flow::Normal)
        }
        PlStmt::ForRange {
            label,
            var,
            reverse,
            lo,
            hi,
            step,
            body,
        } => {
            let first = as_int(&eval_expr(lo, env, regs)?, "FOR loop lower bound")?;
            let second = as_int(&eval_expr(hi, env, regs)?, "FOR loop upper bound")?;
            let step = match step {
                Some(s) => {
                    let st = as_int(&eval_expr(s, env, regs)?, "FOR loop BY step")?;
                    if st <= 0 {
                        return Err(err("PL/pgSQL: FOR loop BY step must be positive"));
                    }
                    st
                }
                None => 1,
            };

            let saved_v = env.vars.remove(var);
            let saved_t = env.var_types.remove(var);
            env.var_types
                .insert(var.clone(), (crate::types::oid::INT4, types::typmod::NONE));
            let flow = run_for_range(
                *reverse, first, second, step, var, body, env, catalog, regs, label,
            );
            match saved_v {
                Some(v) => {
                    env.vars.insert(var.clone(), v);
                }
                None => {
                    env.vars.remove(var);
                }
            }
            match saved_t {
                Some(t) => {
                    env.var_types.insert(var.clone(), t);
                }
                None => {
                    env.var_types.remove(var);
                }
            }
            flow
        }
        PlStmt::Exit { label, when } => {
            if let Some(c) = when {
                if !truthy(&eval_expr(c, env, regs)?) {
                    return Ok(Flow::Normal);
                }
            }
            Ok(Flow::Exit(label.clone()))
        }
        PlStmt::Continue { label, when } => {
            if let Some(c) = when {
                if !truthy(&eval_expr(c, env, regs)?) {
                    return Ok(Flow::Normal);
                }
            }
            Ok(Flow::Continue(label.clone()))
        }
        PlStmt::Case { operand, arms, els } => {
            for (tests, body) in arms {
                for t in tests {
                    let hit = match operand {

                        Some(op) => truthy(&eval_expr(&format!("({op}) = ({t})"), env, regs)?),

                        None => truthy(&eval_expr(t, env, regs)?),
                    };
                    if hit {
                        return exec_stmts(body, env, catalog, regs);
                    }
                }
            }
            if let Some(body) = els {
                return exec_stmts(body, env, catalog, regs);
            }

            Err(PgError::RaiseException {
                message: "case not found".to_string(),
            })
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn run_for_range(
    reverse: bool,
    first: i64,
    second: i64,
    step: i64,
    var: &str,
    body: &[PlStmt],
    env: &mut PlEnv,
    catalog: &mut Catalog,
    regs: &Arc<TypeRegistries>,
    label: &Option<String>,
) -> Result<Flow, PgError> {
    let mut n = 0u64;
    let mut i = first;
    loop {
        if reverse {
            if i < second {
                break;
            }
        } else if i > second {
            break;
        }
        loop_guard(&mut n)?;
        env.vars.insert(var.to_string(), SqlValue::Int(i));
        match exec_stmts(body, env, catalog, regs)? {
            Flow::Normal => {}
            Flow::Return(r) => return Ok(Flow::Return(r)),
            Flow::Exit(l) => {
                if targets(&l, label) {
                    break;
                }
                return Ok(Flow::Exit(l));
            }
            Flow::Continue(l) => {
                if !targets(&l, label) {
                    return Ok(Flow::Continue(l));
                }
            }
        }
        if reverse {
            i -= step;
        } else {
            i += step;
        }
    }
    Ok(Flow::Normal)
}

fn loop_guard(n: &mut u64) -> Result<(), PgError> {
    *n += 1;
    if *n > LOOP_CAP {
        return Err(err(format!(
            "PL/pgSQL: loop exceeded {LOOP_CAP} iterations (runaway-loop guard)"
        )));
    }
    Ok(())
}

fn as_int(v: &SqlValue, what: &str) -> Result<i64, PgError> {
    match v {
        SqlValue::Int(n) => Ok(*n),
        SqlValue::Real(r) => Ok(*r as i64),
        SqlValue::Text(t) => t
            .trim()
            .parse::<i64>()
            .map_err(|_| err(format!("PL/pgSQL: {what} is not an integer"))),
        _ => Err(err(format!("PL/pgSQL: {what} is not an integer"))),
    }
}

pub fn truthy(v: &SqlValue) -> bool {
    match v {
        SqlValue::Int(n) => *n != 0,
        SqlValue::Real(r) => *r != 0.0,
        SqlValue::Text(t) => {
            let t = t.trim();
            t.eq_ignore_ascii_case("t") || t.eq_ignore_ascii_case("true") || t == "1"
        }
        _ => false,
    }
}

fn subst_list(env: &PlEnv) -> Vec<(String, crate::expr::ast::Expr)> {
    use crate::expr::ast::Expr;
    let mut subst: Vec<(String, Expr)> = Vec::new();

    for (k, v) in &env.vars {
        subst.push((k.clone(), Expr::Lit(v.clone())));
    }

    for (which, rowopt) in [("new", &env.new), ("old", &env.old)] {
        match rowopt {
            Some(row) => {
                for (i, name) in env.col_names.iter().enumerate() {
                    let v = row.get(i).cloned().unwrap_or(SqlValue::Null);
                    subst.push((format!("{which}.{name}"), Expr::Lit(v)));
                }
                let texts: Vec<Option<String>> = row
                    .iter()
                    .enumerate()
                    .map(|(i, v)| render_field(env.col_oids.get(i).copied().unwrap_or(0), v))
                    .collect();
                subst.push((
                    which.to_string(),
                    Expr::Lit(SqlValue::Text(crate::types::composite::encode(&texts))),
                ));
            }
            None => {
                subst.push((which.to_string(), Expr::Null));
            }
        }
    }

    for (k, v) in &env.tg {
        subst.push((k.clone(), Expr::Lit(SqlValue::Text(v.clone()))));
    }
    subst
}

fn render_field(oid: u32, v: &SqlValue) -> Option<String> {
    match v {
        SqlValue::Null => None,
        _ => {
            let s = if oid != 0 {
                let t = types::output(oid, v);
                if t.is_empty() {
                    structural(v)
                } else {
                    t
                }
            } else {
                structural(v)
            };
            Some(s)
        }
    }
}

fn structural(v: &SqlValue) -> String {
    match v {
        SqlValue::Null => String::new(),
        SqlValue::Int(n) => n.to_string(),
        SqlValue::Real(f) => format!("{f}"),
        SqlValue::Text(s) => s.clone(),
        SqlValue::Blob(b) => {
            let mut s = String::from("\\x");
            for byte in b {
                s.push_str(&format!("{byte:02x}"));
            }
            s
        }
    }
}

fn eval_expr(src: &str, env: &PlEnv, regs: &Arc<TypeRegistries>) -> Result<SqlValue, PgError> {
    let e = crate::expr::parser::parse(src)
        .map_err(|_| err(format!("PL/pgSQL: could not parse expression `{src}`")))?;
    eval_ast(&e, env, regs)
}

pub fn eval_ast(
    e: &crate::expr::ast::Expr,
    env: &PlEnv,
    regs: &Arc<TypeRegistries>,
) -> Result<SqlValue, PgError> {
    let subst = subst_list(env);
    let substituted = crate::stmt::func_inline::subst_expr_pub(e, &subst);
    eval_ctx(&substituted, EvalCtx::new(regs))
}

fn format_raise(
    fmt: &str,
    args: &[String],
    env: &PlEnv,
    regs: &Arc<TypeRegistries>,
) -> Result<String, PgError> {
    let mut out = String::new();
    let mut ai = 0usize;
    let mut chars = fmt.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '%' {
            if chars.peek() == Some(&'%') {
                chars.next();
                out.push('%');
            } else if let Some(a) = args.get(ai) {
                let v = eval_expr(a, env, regs)?;
                out.push_str(&structural(&v));
                ai += 1;
            }
        } else {
            out.push(c);
        }
    }
    Ok(out)
}

fn substitute_vars(sql: &str, env: &PlEnv) -> String {
    let chars: Vec<char> = sql.chars().collect();
    let mut out = String::new();
    let mut i = 0usize;
    while i < chars.len() {
        let c = chars[i];

        if c == '\'' {
            out.push(c);
            i += 1;
            while i < chars.len() {
                out.push(chars[i]);
                if chars[i] == '\'' {
                    if i + 1 < chars.len() && chars[i + 1] == '\'' {
                        out.push(chars[i + 1]);
                        i += 2;
                        continue;
                    }
                    i += 1;
                    break;
                }
                i += 1;
            }
            continue;
        }
        if c.is_alphabetic() || c == '_' {
            let start = i;
            while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            let word: String = chars[start..i].iter().collect();
            let lw = word.to_ascii_lowercase();

            if (lw == "new" || lw == "old") && chars.get(i) == Some(&'.') {

                let mut j = i + 1;
                let fstart = j;
                while j < chars.len() && (chars[j].is_alphanumeric() || chars[j] == '_') {
                    j += 1;
                }
                let field: String = chars[fstart..j]
                    .iter()
                    .collect::<String>()
                    .to_ascii_lowercase();
                if let Some(lit) = env_field_literal(env, &lw, &field) {
                    out.push_str(&lit);
                    i = j;
                    continue;
                }
            }

            if let Some(lit) = env_var_literal(env, &lw) {
                out.push_str(&lit);
                continue;
            }
            out.push_str(&word);
            continue;
        }
        out.push(c);
        i += 1;
    }
    out
}

fn env_field_literal(env: &PlEnv, which: &str, field: &str) -> Option<String> {
    let row = if which == "new" {
        env.new.as_ref()
    } else {
        env.old.as_ref()
    }?;
    let idx = env.col_index(field)?;
    let v = row.get(idx)?;
    Some(sql_literal(v, env.col_oids.get(idx).copied().unwrap_or(0)))
}

fn env_var_literal(env: &PlEnv, name: &str) -> Option<String> {
    if let Some(v) = env.vars.get(name) {
        return Some(sql_literal(v, 0));
    }
    for (k, v) in &env.tg {
        if k == name {
            return Some(sql_literal(&SqlValue::Text(v.clone()), 0));
        }
    }
    None
}

fn sql_literal(v: &SqlValue, oid: u32) -> String {
    match v {
        SqlValue::Null => "NULL".to_string(),
        _ => {
            let text = if oid != 0 {
                let t = types::output(oid, v);
                if t.is_empty() {
                    structural(v)
                } else {
                    t
                }
            } else {
                structural(v)
            };
            let quoted = format!("'{}'", text.replace('\'', "''"));
            let tname = if oid != 0 { types::type_name(oid) } else { "" };
            if tname.is_empty() {
                quoted
            } else {
                format!("{quoted}::{tname}")
            }
        }
    }
}

const MAX_CALL_DEPTH: u32 = 64;

thread_local! {

    static CALL_DEPTH: Cell<u32> = const { Cell::new(0) };
}

pub fn run_function(
    fdef: &FunctionDef,
    arg_values: &[SqlValue],
    catalog: &mut Catalog,
    regs: &Arc<TypeRegistries>,
) -> Result<SqlValue, PgError> {

    if arg_values.len() != fdef.args.len() {
        return Err(err(format!(
            "function {}(...) with {} argument(s) does not exist",
            fdef.name,
            arg_values.len()
        )));
    }
    let Some(block) = fdef.pl_body.as_ref() else {
        return Err(err(format!(
            "function \"{}\" is not a plpgsql function",
            fdef.name
        )));
    };

    let depth = CALL_DEPTH.with(|d| {
        let v = d.get() + 1;
        d.set(v);
        v
    });
    let result = run_function_body(fdef, block, arg_values, catalog, regs, depth);
    CALL_DEPTH.with(|d| d.set(d.get().saturating_sub(1)));
    result
}

fn run_function_body(
    fdef: &FunctionDef,
    block: &PlBlock,
    arg_values: &[SqlValue],
    catalog: &mut Catalog,
    regs: &Arc<TypeRegistries>,
    depth: u32,
) -> Result<SqlValue, PgError> {
    if depth > MAX_CALL_DEPTH {
        return Err(err(format!(
            "plpgsql function \"{}\" exceeded max call depth {MAX_CALL_DEPTH} \
             (recursion is not supported)",
            fdef.name
        )));
    }

    let mut vars: HashMap<String, SqlValue> = HashMap::new();
    let mut var_types: HashMap<String, (u32, i32)> = HashMap::new();
    for (i, ((argname, oid, typmod), raw)) in fdef.args.iter().zip(arg_values).enumerate() {
        let coerced = if *oid != 0 {
            crate::stmt::ddl::coerce(raw.clone(), Some(*oid), *typmod, regs)?
        } else {
            raw.clone()
        };
        let pos = format!("${}", i + 1);
        vars.insert(pos.clone(), coerced.clone());
        if *oid != 0 {
            var_types.insert(pos, (*oid, *typmod));
        }
        if let Some(n) = argname {
            let n = n.to_ascii_lowercase();
            vars.insert(n.clone(), coerced);
            if *oid != 0 {
                var_types.insert(n, (*oid, *typmod));
            }
        }
    }
    let mut env = PlEnv {
        col_names: Vec::new(),
        col_oids: Vec::new(),
        col_typmods: Vec::new(),
        new: None,
        old: None,
        tg: Vec::new(),
        vars,
        var_types,
        is_function: true,
    };
    let outcome = exec_block(block, &mut env, catalog, regs)?;
    let ret = match outcome {
        PlOutcome::Return(Some(row)) => row.into_iter().next().unwrap_or(SqlValue::Null),

        PlOutcome::Return(None) | PlOutcome::Fell => SqlValue::Null,
    };
    if fdef.ret_oid != 0 {
        crate::stmt::ddl::coerce(ret, Some(fdef.ret_oid), fdef.ret_typmod, regs)
    } else {
        Ok(ret)
    }
}

fn eval_expr_mut(
    src: &str,
    env: &PlEnv,
    catalog: &mut Catalog,
    regs: &Arc<TypeRegistries>,
) -> Result<SqlValue, PgError> {
    let e = crate::expr::parser::parse(src)
        .map_err(|_| err(format!("PL/pgSQL: could not parse expression `{src}`")))?;
    let subst = subst_list(env);
    let substituted = crate::stmt::func_inline::subst_expr_pub(&e, &subst);
    let folded = fold_const_plpgsql(&substituted, catalog, regs)?;
    eval_ctx(&folded, EvalCtx::new(regs))
}

fn is_plpgsql(name: &str, regs: &TypeRegistries) -> bool {
    regs.functions
        .get(name)
        .map(|f| f.language == Lang::PlPgSql)
        .unwrap_or(false)
}

pub(crate) fn expr_has_column(e: &Expr) -> bool {
    match e {
        Expr::Column(_) | Expr::ColumnRef(_) => true,
        Expr::ScalarSubquery(_)
        | Expr::Exists { .. }
        | Expr::InSubquery { .. }
        | Expr::Quantified { .. }
        | Expr::Window { .. } => true,
        _ => scalar_children(e).iter().any(|c| expr_has_column(c)),
    }
}

pub(crate) fn expr_calls_plpgsql(e: &Expr, regs: &TypeRegistries) -> bool {
    if let Expr::Func { name, .. } = e {
        if is_plpgsql(name, regs) {
            return true;
        }
    }
    scalar_children(e)
        .iter()
        .any(|c| expr_calls_plpgsql(c, regs))
}

pub(crate) fn collect_plpgsql_calls(e: &Expr, regs: &TypeRegistries) -> Vec<Expr> {
    let mut out = Vec::new();
    collect_calls_into(e, regs, &mut out);
    out
}

fn collect_calls_into(e: &Expr, regs: &TypeRegistries, out: &mut Vec<Expr>) {
    if let Expr::Func { name, .. } = e {
        if is_plpgsql(name, regs) {
            out.push(e.clone());
            return;
        }
    }
    for c in scalar_children(e) {
        collect_calls_into(c, regs, out);
    }
}

pub(crate) fn subst_plpgsql_calls(
    e: &Expr,
    results: &[SqlValue],
    regs: &TypeRegistries,
    idx: &mut usize,
) -> Expr {
    if let Expr::Func { name, .. } = e {
        if is_plpgsql(name, regs) {
            let oid = regs.functions[name].ret_oid;
            let v = results.get(*idx).cloned().unwrap_or(SqlValue::Null);
            *idx += 1;
            return lit_cast(v, oid);
        }
    }
    map_scalar_children(e, &mut |c| subst_plpgsql_calls(c, results, regs, idx))
}

pub(crate) fn fold_const_plpgsql(
    e: &Expr,
    catalog: &mut Catalog,
    regs: &Arc<TypeRegistries>,
) -> Result<Expr, PgError> {

    let folded = map_scalar_children_res(e, &mut |c| fold_const_plpgsql(c, catalog, regs))?;
    if let Expr::Func {
        name,
        args,
        distinct,
        filter,
        order_by,
    } = &folded
    {
        if !*distinct
            && filter.is_none()
            && order_by.is_empty()
            && is_plpgsql(name, regs)
            && args.iter().all(|a| !expr_has_column(a))
        {
            let fdef = regs.functions[name].clone();
            let mut vals = Vec::with_capacity(args.len());
            for a in args {
                vals.push(eval_ctx(a, EvalCtx::new(regs))?);
            }
            let v = run_function(&fdef, &vals, catalog, regs)?;
            return Ok(lit_cast(v, fdef.ret_oid));
        }
    }
    Ok(folded)
}

fn lit_cast(v: SqlValue, oid: u32) -> Expr {
    if oid == 0 {
        return Expr::Lit(v);
    }
    let name = types::type_name(oid);
    if name.is_empty() {
        Expr::Lit(v)
    } else {
        Expr::Cast {
            expr: Box::new(Expr::Lit(v)),
            type_name: name.to_string(),
        }
    }
}

fn scalar_children(e: &Expr) -> Vec<&Expr> {
    match e {
        Expr::Unary { expr, .. }
        | Expr::GenUnary { expr, .. }
        | Expr::Cast { expr, .. }
        | Expr::IsNull { expr, .. } => vec![expr],
        Expr::Binary { left, right, .. } | Expr::GenBinary { left, right, .. } => {
            vec![left, right]
        }
        Expr::Func { args, .. } => args.iter().collect(),
        Expr::Row(xs) => xs.iter().collect(),
        Expr::FieldAccess { base, .. } => vec![base],
        Expr::Case {
            operand,
            whens,
            else_,
        } => {
            let mut v: Vec<&Expr> = Vec::new();
            if let Some(o) = operand {
                v.push(o);
            }
            for (c, r) in whens {
                v.push(c);
                v.push(r);
            }
            if let Some(x) = else_ {
                v.push(x);
            }
            v
        }
        Expr::InSubquery { expr, .. } | Expr::Quantified { expr, .. } => vec![expr],
        _ => Vec::new(),
    }
}

fn map_scalar_children(e: &Expr, f: &mut dyn FnMut(&Expr) -> Expr) -> Expr {
    match e {
        Expr::Unary { op, expr } => Expr::Unary {
            op: *op,
            expr: Box::new(f(expr)),
        },
        Expr::GenUnary { op, expr } => Expr::GenUnary {
            op: op.clone(),
            expr: Box::new(f(expr)),
        },
        Expr::Cast { expr, type_name } => Expr::Cast {
            expr: Box::new(f(expr)),
            type_name: type_name.clone(),
        },
        Expr::IsNull { expr, negated } => Expr::IsNull {
            expr: Box::new(f(expr)),
            negated: *negated,
        },
        Expr::Binary { op, left, right } => Expr::Binary {
            op: *op,
            left: Box::new(f(left)),
            right: Box::new(f(right)),
        },
        Expr::GenBinary { op, left, right } => Expr::GenBinary {
            op: op.clone(),
            left: Box::new(f(left)),
            right: Box::new(f(right)),
        },
        Expr::Func {
            name,
            args,
            distinct,
            filter,
            order_by,
        } => Expr::Func {
            name: name.clone(),
            args: args.iter().map(|a| f(a)).collect(),
            distinct: *distinct,
            filter: filter.clone(),
            order_by: order_by.clone(),
        },
        Expr::Row(xs) => Expr::Row(xs.iter().map(|x| f(x)).collect()),
        Expr::FieldAccess {
            base,
            field,
            comp_oid,
            field_oid,
        } => Expr::FieldAccess {
            base: Box::new(f(base)),
            field: field.clone(),
            comp_oid: *comp_oid,
            field_oid: *field_oid,
        },
        Expr::Case {
            operand,
            whens,
            else_,
        } => Expr::Case {
            operand: operand.as_deref().map(&mut *f).map(Box::new),
            whens: whens.iter().map(|(c, r)| (f(c), f(r))).collect(),
            else_: else_.as_deref().map(&mut *f).map(Box::new),
        },
        Expr::InSubquery {
            expr,
            query,
            negated,
        } => Expr::InSubquery {
            expr: Box::new(f(expr)),
            query: query.clone(),
            negated: *negated,
        },
        Expr::Quantified {
            expr,
            op,
            quantifier,
            query,
        } => Expr::Quantified {
            expr: Box::new(f(expr)),
            op: *op,
            quantifier: *quantifier,
            query: query.clone(),
        },
        _ => e.clone(),
    }
}

fn map_scalar_children_res(
    e: &Expr,
    f: &mut dyn FnMut(&Expr) -> Result<Expr, PgError>,
) -> Result<Expr, PgError> {

    let mut err_slot: Option<PgError> = None;
    let out = map_scalar_children(e, &mut |c| match f(c) {
        Ok(v) => v,
        Err(e) => {
            if err_slot.is_none() {
                err_slot = Some(e);
            }
            c.clone()
        }
    });
    match err_slot {
        Some(e) => Err(e),
        None => Ok(out),
    }
}
