
use super::PgError;
use sql_core::SqlValue;

#[derive(Debug, Clone, PartialEq)]
pub enum TsQuery {
    Val {
        lexeme: String,
        prefix: bool,
    },
    Not(Box<TsQuery>),
    And(Box<TsQuery>, Box<TsQuery>),
    Or(Box<TsQuery>, Box<TsQuery>),

    Phrase(Box<TsQuery>, Box<TsQuery>, u32),
}

fn syntax(input: &str) -> PgError {
    PgError::TsquerySyntax {
        input: input.to_string(),
    }
}

#[derive(Debug, Clone, PartialEq)]
enum QTok {
    Val { lexeme: String, prefix: bool },
    And,
    Or,
    Not,
    Phrase(u32),
    LParen,
    RParen,
}

fn lex(input: &str) -> Result<Vec<QTok>, PgError> {
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;
    let mut out = Vec::new();
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        match c {
            '&' => {
                out.push(QTok::And);
                i += 1;
            }
            '|' => {
                out.push(QTok::Or);
                i += 1;
            }
            '!' => {
                out.push(QTok::Not);
                i += 1;
            }
            '(' => {
                out.push(QTok::LParen);
                i += 1;
            }
            ')' => {
                out.push(QTok::RParen);
                i += 1;
            }
            '<' => {

                i += 1;
                let start = i;
                while i < chars.len() && chars[i] != '>' {
                    i += 1;
                }
                if i >= chars.len() {
                    return Err(syntax(input));
                }
                let body: String = chars[start..i].iter().collect();
                i += 1;
                let dist = if body == "-" {
                    1
                } else {
                    body.parse::<u32>().map_err(|_| syntax(input))?
                };
                out.push(QTok::Phrase(dist));
            }
            '\'' => {
                i += 1;
                let mut s = String::new();
                loop {
                    match chars.get(i) {
                        None => return Err(syntax(input)),
                        Some('\'') => {
                            if chars.get(i + 1) == Some(&'\'') {
                                s.push('\'');
                                i += 2;
                            } else {
                                i += 1;
                                break;
                            }
                        }
                        Some(&ch) => {
                            s.push(ch);
                            i += 1;
                        }
                    }
                }
                let prefix = consume_prefix(&chars, &mut i);
                out.push(QTok::Val {
                    lexeme: s.to_lowercase(),
                    prefix,
                });
            }
            _ if c.is_alphanumeric() => {
                let start = i;
                while i < chars.len() && chars[i].is_alphanumeric() {
                    i += 1;
                }
                let word: String = chars[start..i].iter().collect();
                let prefix = consume_prefix(&chars, &mut i);
                out.push(QTok::Val {
                    lexeme: word.to_lowercase(),
                    prefix,
                });
            }
            _ => return Err(syntax(input)),
        }
    }
    Ok(out)
}

fn consume_prefix(chars: &[char], i: &mut usize) -> bool {
    if chars.get(*i) == Some(&':') {
        *i += 1;
        let mut prefix = false;
        while matches!(chars.get(*i), Some('*') | Some('A'..='D') | Some('a'..='d')) {
            if chars[*i] == '*' {
                prefix = true;
            }
            *i += 1;
        }
        prefix
    } else {
        false
    }
}

struct Parser<'a> {
    toks: &'a [QTok],
    pos: usize,
    src: &'a str,
}

impl<'a> Parser<'a> {
    fn peek(&self) -> Option<&QTok> {
        self.toks.get(self.pos)
    }
    fn bump(&mut self) -> Option<&QTok> {
        let t = self.toks.get(self.pos);
        if t.is_some() {
            self.pos += 1;
        }
        t
    }
    fn or(&mut self) -> Result<TsQuery, PgError> {
        let mut lhs = self.and()?;
        while matches!(self.peek(), Some(QTok::Or)) {
            self.bump();
            let rhs = self.and()?;
            lhs = TsQuery::Or(Box::new(lhs), Box::new(rhs));
        }
        Ok(lhs)
    }
    fn and(&mut self) -> Result<TsQuery, PgError> {
        let mut lhs = self.phrase()?;
        while matches!(self.peek(), Some(QTok::And)) {
            self.bump();
            let rhs = self.phrase()?;
            lhs = TsQuery::And(Box::new(lhs), Box::new(rhs));
        }
        Ok(lhs)
    }
    fn phrase(&mut self) -> Result<TsQuery, PgError> {
        let mut lhs = self.not()?;
        while let Some(QTok::Phrase(d)) = self.peek() {
            let d = *d;
            self.bump();
            let rhs = self.not()?;
            lhs = TsQuery::Phrase(Box::new(lhs), Box::new(rhs), d);
        }
        Ok(lhs)
    }
    fn not(&mut self) -> Result<TsQuery, PgError> {
        if matches!(self.peek(), Some(QTok::Not)) {
            self.bump();
            Ok(TsQuery::Not(Box::new(self.not()?)))
        } else {
            self.primary()
        }
    }
    fn primary(&mut self) -> Result<TsQuery, PgError> {
        match self.bump() {
            Some(QTok::LParen) => {
                let inner = self.or()?;
                match self.bump() {
                    Some(QTok::RParen) => Ok(inner),
                    _ => Err(syntax(self.src)),
                }
            }
            Some(QTok::Val { lexeme, prefix }) => Ok(TsQuery::Val {
                lexeme: lexeme.clone(),
                prefix: *prefix,
            }),
            _ => Err(syntax(self.src)),
        }
    }
}

pub fn parse(input: &str) -> Result<TsQuery, PgError> {
    let toks = lex(input)?;
    if toks.is_empty() {
        return Err(syntax(input));
    }
    let mut p = Parser {
        toks: &toks,
        pos: 0,
        src: input,
    };
    let q = p.or()?;
    if p.pos != toks.len() {
        return Err(syntax(input));
    }
    Ok(q)
}

fn priority(q: &TsQuery) -> i32 {
    match q {
        TsQuery::Val { .. } => 5,
        TsQuery::Not(_) => 4,
        TsQuery::Phrase(..) => 3,
        TsQuery::And(..) => 2,
        TsQuery::Or(..) => 1,
    }
}

fn push_val(lexeme: &str, prefix: bool, out: &mut String) {
    out.push('\'');
    for c in lexeme.chars() {
        match c {
            '\'' => out.push_str("''"),
            '\\' => out.push_str("\\\\"),
            _ => out.push(c),
        }
    }
    out.push('\'');
    if prefix {
        out.push_str(":*");
    }
}

fn infix(q: &TsQuery, parent_priority: i32, right_phrase: bool, out: &mut String) {
    match q {
        TsQuery::Val { lexeme, prefix } => push_val(lexeme, *prefix, out),
        TsQuery::Not(a) => {
            let pr = priority(q);
            let paren = pr < parent_priority;
            if paren {
                out.push_str("( ");
            }
            out.push('!');
            infix(a, pr, false, out);
            if paren {
                out.push_str(" )");
            }
        }
        TsQuery::And(..) | TsQuery::Or(..) | TsQuery::Phrase(..) => {
            let pr = priority(q);
            let is_phrase = matches!(q, TsQuery::Phrase(..));
            let paren =
                pr < parent_priority || (is_phrase && pr == parent_priority && right_phrase);
            if paren {
                out.push_str("( ");
            }
            let (l, r, opstr) = match q {
                TsQuery::And(l, r) => (l, r, " & ".to_string()),
                TsQuery::Or(l, r) => (l, r, " | ".to_string()),
                TsQuery::Phrase(l, r, d) => (
                    l,
                    r,
                    if *d == 1 {
                        " <-> ".to_string()
                    } else {
                        format!(" <{d}> ")
                    },
                ),
                _ => unreachable!(),
            };
            infix(l, pr, false, out);
            out.push_str(&opstr);
            infix(r, pr, is_phrase, out);
            if paren {
                out.push_str(" )");
            }
        }
    }
}

pub fn render(q: &TsQuery) -> String {
    let mut out = String::new();
    infix(q, 0, false, &mut out);
    out
}

pub fn numnode(q: &TsQuery) -> i64 {
    match q {
        TsQuery::Val { .. } => 1,
        TsQuery::Not(a) => 1 + numnode(a),
        TsQuery::And(a, b) | TsQuery::Or(a, b) | TsQuery::Phrase(a, b, _) => {
            1 + numnode(a) + numnode(b)
        }
    }
}

pub fn matches(q: &TsQuery, entries: &[(String, Vec<u32>)]) -> bool {
    match q {
        TsQuery::Val { lexeme, prefix } => has_lexeme(entries, lexeme, *prefix),
        TsQuery::Not(a) => !matches(a, entries),
        TsQuery::And(a, b) => matches(a, entries) && matches(b, entries),
        TsQuery::Or(a, b) => matches(a, entries) || matches(b, entries),
        TsQuery::Phrase(..) => !phrase_positions(q, entries).is_empty(),
    }
}

fn has_lexeme(entries: &[(String, Vec<u32>)], lexeme: &str, prefix: bool) -> bool {
    entries.iter().any(|(l, _)| {
        if prefix {
            l.starts_with(lexeme)
        } else {
            l == lexeme
        }
    })
}

fn phrase_positions(q: &TsQuery, entries: &[(String, Vec<u32>)]) -> Vec<u32> {
    match q {
        TsQuery::Val { lexeme, prefix } => {
            let mut ps: Vec<u32> = Vec::new();
            for (l, positions) in entries {
                let hit = if *prefix {
                    l.starts_with(lexeme)
                } else {
                    l == lexeme
                };
                if hit {
                    ps.extend(positions.iter().copied());
                }
            }
            ps.sort_unstable();
            ps.dedup();
            ps
        }
        TsQuery::Phrase(a, b, n) => {
            let left = phrase_positions(a, entries);
            let right = phrase_positions(b, entries);
            right
                .into_iter()
                .filter(|p| {
                    p.checked_sub(*n)
                        .map(|q| left.contains(&q))
                        .unwrap_or(false)
                })
                .collect()
        }
        TsQuery::And(a, b) => {
            let left = phrase_positions(a, entries);
            let right = phrase_positions(b, entries);
            left.into_iter().filter(|p| right.contains(p)).collect()
        }
        TsQuery::Or(a, b) => {
            let mut ps = phrase_positions(a, entries);
            ps.extend(phrase_positions(b, entries));
            ps.sort_unstable();
            ps.dedup();
            ps
        }
        TsQuery::Not(_) => Vec::new(),
    }
}

pub fn input(_oid: u32, text: &str) -> Result<SqlValue, PgError> {
    Ok(SqlValue::Text(render(&parse(text)?)))
}

pub fn output(_oid: u32, v: &SqlValue) -> String {
    match v {
        SqlValue::Text(s) => s.clone(),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::tsvector;

    fn c(s: &str) -> String {
        render(&parse(s).unwrap())
    }

    #[test]
    fn canonical_output() {
        assert_eq!(c("fat & rat"), "'fat' & 'rat'");
        assert_eq!(c("fat | rat"), "'fat' | 'rat'");
        assert_eq!(c("!fat"), "!'fat'");
        assert_eq!(c("fat <-> rat"), "'fat' <-> 'rat'");
        assert_eq!(c("fat <2> rat"), "'fat' <2> 'rat'");

        assert_eq!(c("a & b | c"), "'a' & 'b' | 'c'");

        assert_eq!(c("a & (b | c)"), "'a' & ( 'b' | 'c' )");
        assert_eq!(c("super:*"), "'super':*");
    }

    #[test]
    fn numnode_counts() {
        assert_eq!(numnode(&parse("foo").unwrap()), 1);
        assert_eq!(numnode(&parse("foo & bar").unwrap()), 3);
        assert_eq!(numnode(&parse("foo & (bar | baz)").unwrap()), 5);
        assert_eq!(numnode(&parse("!foo").unwrap()), 2);
    }

    #[test]
    fn matching() {
        let v = tsvector::tokenize("a fat cat sat on a mat");
        assert!(matches(&parse("cat & fat").unwrap(), &v));
        assert!(!matches(&parse("cat & dog").unwrap(), &v));
        assert!(matches(&parse("cat | dog").unwrap(), &v));
        assert!(matches(&parse("dog | cat").unwrap(), &v));
        assert!(!matches(&parse("!cat").unwrap(), &v));
        assert!(matches(&parse("!dog").unwrap(), &v));

        assert!(matches(&parse("fat <-> cat").unwrap(), &v));
        assert!(!matches(&parse("cat <-> fat").unwrap(), &v));

        assert!(matches(&parse("a <3> sat").unwrap(), &v));
    }

    #[test]
    fn malformed_errors() {
        for bad in ["& foo", "foo &", "!", "( foo", "foo )", ""] {
            assert!(parse(bad).is_err(), "expected error for {bad:?}");
        }
    }
}
