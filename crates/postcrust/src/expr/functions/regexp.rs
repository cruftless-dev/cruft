
use crate::types::PgError;
use sql_core::SqlValue;

type Caps = Vec<Option<(usize, usize)>>;

#[derive(Clone)]
enum ClassItem {
    Ch(char),
    Range(char, char),
    Digit,
    NotDigit,
    Word,
    NotWord,
    Space,
    NotSpace,
    Posix(&'static str),
}

#[derive(Clone)]
enum Atom {
    Char(char),
    Any,
    Start,
    End,
    Class {
        negated: bool,
        items: Vec<ClassItem>,
    },
    Group {
        idx: Option<usize>,
        alts: Vec<Vec<Piece>>,
    },
}

#[derive(Clone)]
struct Piece {
    atom: Atom,
    min: usize,
    max: Option<usize>,
    greedy: bool,
}

struct Regex {
    alts: Vec<Vec<Piece>>,
    ngroups: usize,
}

struct Parser {
    src: Vec<char>,
    pos: usize,
    ngroups: usize,
}

fn re_err(input: String) -> PgError {
    PgError::InvalidInputSyntax {
        typ: "regular expression",
        input,
    }
}

impl Parser {
    fn peek(&self) -> Option<char> {
        self.src.get(self.pos).copied()
    }
    fn bump(&mut self) -> Option<char> {
        let c = self.src.get(self.pos).copied();
        if c.is_some() {
            self.pos += 1;
        }
        c
    }

    fn parse_alts(&mut self) -> Result<Vec<Vec<Piece>>, PgError> {
        let mut alts = vec![self.parse_seq()?];
        while self.peek() == Some('|') {
            self.bump();
            alts.push(self.parse_seq()?);
        }
        Ok(alts)
    }

    fn parse_seq(&mut self) -> Result<Vec<Piece>, PgError> {
        let mut seq = Vec::new();
        while let Some(c) = self.peek() {
            if c == '|' || c == ')' {
                break;
            }
            let atom = self.parse_atom()?;
            let piece = self.parse_quantifier(atom)?;
            seq.push(piece);
        }
        Ok(seq)
    }

    fn parse_quantifier(&mut self, atom: Atom) -> Result<Piece, PgError> {
        let (min, max) = match self.peek() {
            Some('*') => {
                self.bump();
                (0, None)
            }
            Some('+') => {
                self.bump();
                (1, None)
            }
            Some('?') => {
                self.bump();
                (0, Some(1))
            }
            Some('{') => self.parse_brace()?,
            _ => {
                return Ok(Piece {
                    atom,
                    min: 1,
                    max: Some(1),
                    greedy: true,
                })
            }
        };

        let greedy = if self.peek() == Some('?') {
            self.bump();
            false
        } else {
            true
        };
        Ok(Piece {
            atom,
            min,
            max,
            greedy,
        })
    }

    fn parse_brace(&mut self) -> Result<(usize, Option<usize>), PgError> {

        self.bump();
        let mut lo = String::new();
        while let Some(c) = self.peek() {
            if c.is_ascii_digit() {
                lo.push(c);
                self.bump();
            } else {
                break;
            }
        }
        let min: usize = lo
            .parse()
            .map_err(|_| re_err(format!("invalid repetition count {{{lo}")))?;
        let max = match self.peek() {
            Some('}') => {
                self.bump();
                Some(min)
            }
            Some(',') => {
                self.bump();
                if self.peek() == Some('}') {
                    self.bump();
                    None
                } else {
                    let mut hi = String::new();
                    while let Some(c) = self.peek() {
                        if c.is_ascii_digit() {
                            hi.push(c);
                            self.bump();
                        } else {
                            break;
                        }
                    }
                    if self.peek() != Some('}') {
                        return Err(re_err("invalid repetition {n,m}".into()));
                    }
                    self.bump();
                    Some(
                        hi.parse()
                            .map_err(|_| re_err("invalid repetition count".into()))?,
                    )
                }
            }
            _ => return Err(re_err("invalid repetition {".into())),
        };
        if let Some(m) = max {
            if m < min {
                return Err(re_err("repetition m < n".into()));
            }
        }
        Ok((min, max))
    }

    fn parse_atom(&mut self) -> Result<Atom, PgError> {
        let c = self.peek().ok_or_else(|| re_err("unexpected end".into()))?;
        match c {
            '.' => {
                self.bump();
                Ok(Atom::Any)
            }
            '^' => {
                self.bump();
                Ok(Atom::Start)
            }
            '$' => {
                self.bump();
                Ok(Atom::End)
            }
            '(' => self.parse_group(),
            '[' => self.parse_class(),
            '\\' => {
                self.bump();
                self.parse_escape()
            }
            _ => {
                self.bump();
                Ok(Atom::Char(c))
            }
        }
    }

    fn parse_group(&mut self) -> Result<Atom, PgError> {
        self.bump();
        let idx = if self.peek() == Some('?') {

            self.bump();
            if self.peek() == Some(':') {
                self.bump();
                None
            } else {
                return Err(re_err("unsupported group extension (?...)".into()));
            }
        } else {
            self.ngroups += 1;
            Some(self.ngroups)
        };
        let alts = self.parse_alts()?;
        if self.peek() != Some(')') {
            return Err(re_err("unbalanced parenthesis".into()));
        }
        self.bump();
        Ok(Atom::Group { idx, alts })
    }

    fn parse_escape(&mut self) -> Result<Atom, PgError> {
        let c = self
            .bump()
            .ok_or_else(|| re_err("trailing backslash".into()))?;
        let item = match c {
            'd' => Some(ClassItem::Digit),
            'D' => Some(ClassItem::NotDigit),
            'w' => Some(ClassItem::Word),
            'W' => Some(ClassItem::NotWord),
            's' => Some(ClassItem::Space),
            'S' => Some(ClassItem::NotSpace),
            _ => None,
        };
        if let Some(it) = item {

            return Ok(Atom::Class {
                negated: false,
                items: vec![it],
            });
        }

        let lit = match c {
            'n' => '\n',
            't' => '\t',
            'r' => '\r',
            other => other,
        };
        Ok(Atom::Char(lit))
    }

    fn parse_class(&mut self) -> Result<Atom, PgError> {
        self.bump();
        let negated = if self.peek() == Some('^') {
            self.bump();
            true
        } else {
            false
        };
        let mut items: Vec<ClassItem> = Vec::new();

        let mut first = true;
        loop {
            let c = match self.peek() {
                None => return Err(re_err("unterminated character class".into())),
                Some(']') if !first => {
                    self.bump();
                    break;
                }
                Some(c) => c,
            };
            first = false;

            if c == '[' && self.src.get(self.pos + 1) == Some(&':') {
                self.bump();
                self.bump();
                let mut name = String::new();
                while let Some(ch) = self.peek() {
                    if ch == ':' {
                        break;
                    }
                    name.push(ch);
                    self.bump();
                }

                if self.peek() != Some(':') {
                    return Err(re_err("malformed [: :] class".into()));
                }
                self.bump();
                if self.peek() != Some(']') {
                    return Err(re_err("malformed [: :] class".into()));
                }
                self.bump();
                let posix: &'static str = match name.as_str() {
                    "alpha" => "alpha",
                    "digit" => "digit",
                    "alnum" => "alnum",
                    "space" => "space",
                    "upper" => "upper",
                    "lower" => "lower",
                    other => return Err(re_err(format!("unsupported POSIX class [:{other}:]"))),
                };
                items.push(ClassItem::Posix(posix));
                continue;
            }

            let ch = if c == '\\' {
                self.bump();
                let e = self
                    .bump()
                    .ok_or_else(|| re_err("trailing backslash in class".into()))?;
                match e {
                    'd' => {
                        items.push(ClassItem::Digit);
                        continue;
                    }
                    'w' => {
                        items.push(ClassItem::Word);
                        continue;
                    }
                    's' => {
                        items.push(ClassItem::Space);
                        continue;
                    }
                    'n' => '\n',
                    't' => '\t',
                    'r' => '\r',
                    other => other,
                }
            } else {
                self.bump();
                c
            };

            if self.peek() == Some('-') && self.src.get(self.pos + 1).is_some_and(|&n| n != ']') {
                self.bump();
                let hi = self.bump().unwrap();
                items.push(ClassItem::Range(ch, hi));
            } else {
                items.push(ClassItem::Ch(ch));
            }
        }
        Ok(Atom::Class { negated, items })
    }
}

pub(crate) fn like_regex_match(text: &str, pattern: &str, ci: bool) -> Result<bool, PgError> {
    let re = compile(pattern)?;
    let chars: Vec<char> = text.chars().collect();
    Ok(find_at(&re, &chars, 0, ci).is_some())
}

fn compile(pattern: &str) -> Result<Regex, PgError> {
    let mut p = Parser {
        src: pattern.chars().collect(),
        pos: 0,
        ngroups: 0,
    };
    let alts = p.parse_alts()?;
    if p.pos != p.src.len() {
        return Err(re_err("unbalanced parenthesis".into()));
    }
    Ok(Regex {
        alts,
        ngroups: p.ngroups,
    })
}

fn ascii_fold(c: char) -> char {
    c.to_ascii_lowercase()
}
fn ceq(a: char, b: char, ci: bool) -> bool {
    a == b || (ci && ascii_fold(a) == ascii_fold(b))
}

fn class_matches(items: &[ClassItem], negated: bool, c: char, ci: bool) -> bool {
    let mut hit = false;
    for it in items {
        let m = match it {
            ClassItem::Ch(x) => ceq(*x, c, ci),
            ClassItem::Range(lo, hi) => {
                (*lo <= c && c <= *hi)
                    || (ci && {
                        let f = ascii_fold(c);
                        (ascii_fold(*lo) <= f && f <= ascii_fold(*hi))
                            && lo.is_ascii()
                            && hi.is_ascii()
                    })
            }
            ClassItem::Digit => c.is_ascii_digit(),
            ClassItem::NotDigit => !c.is_ascii_digit(),
            ClassItem::Word => c.is_alphanumeric() || c == '_',
            ClassItem::NotWord => !(c.is_alphanumeric() || c == '_'),
            ClassItem::Space => c.is_whitespace(),
            ClassItem::NotSpace => !c.is_whitespace(),
            ClassItem::Posix("alpha") => c.is_alphabetic(),
            ClassItem::Posix("digit") => c.is_ascii_digit(),
            ClassItem::Posix("alnum") => c.is_alphanumeric(),
            ClassItem::Posix("space") => c.is_whitespace(),
            ClassItem::Posix("upper") => c.is_uppercase(),
            ClassItem::Posix("lower") => c.is_lowercase(),
            ClassItem::Posix(_) => false,
        };
        if m {
            hit = true;
            break;
        }
    }
    hit ^ negated
}

fn match_atom(
    atom: &Atom,
    text: &[char],
    pos: usize,
    caps: &mut Caps,
    ci: bool,
    k: &mut dyn FnMut(usize, &mut Caps) -> bool,
) -> bool {
    match atom {
        Atom::Char(c) => pos < text.len() && ceq(*c, text[pos], ci) && k(pos + 1, caps),
        Atom::Any => pos < text.len() && k(pos + 1, caps),
        Atom::Start => pos == 0 && k(pos, caps),
        Atom::End => pos == text.len() && k(pos, caps),
        Atom::Class { negated, items } => {
            pos < text.len() && class_matches(items, *negated, text[pos], ci) && k(pos + 1, caps)
        }
        Atom::Group { idx, alts } => {
            let start = pos;
            for alt in alts {
                let matched = match_seq(alt, text, pos, caps, ci, &mut |end, caps| {
                    let saved = idx.map(|i| caps[i]);
                    if let Some(i) = idx {
                        caps[*i] = Some((start, end));
                    }
                    if k(end, caps) {
                        true
                    } else {
                        if let Some(i) = idx {
                            caps[*i] = saved.unwrap();
                        }
                        false
                    }
                });
                if matched {
                    return true;
                }
            }
            false
        }
    }
}

fn match_piece(
    piece: &Piece,
    rest: &[Piece],
    text: &[char],
    pos: usize,
    caps: &mut Caps,
    ci: bool,
    k: &mut dyn FnMut(usize, &mut Caps) -> bool,
) -> bool {

    fn rep(
        piece: &Piece,
        rest: &[Piece],
        text: &[char],
        pos: usize,
        count: usize,
        caps: &mut Caps,
        ci: bool,
        k: &mut dyn FnMut(usize, &mut Caps) -> bool,
    ) -> bool {
        let can_more = piece.max.map(|m| count < m).unwrap_or(true);

        let more = |npos: usize, caps: &mut Caps, k: &mut dyn FnMut(usize, &mut Caps) -> bool| {
            if npos == pos {
                (count + 1 >= piece.min) && match_seq(rest, text, npos, caps, ci, k)
            } else {
                rep(piece, rest, text, npos, count + 1, caps, ci, k)
            }
        };
        if piece.greedy {

            if can_more
                && match_atom(&piece.atom, text, pos, caps, ci, &mut |npos, caps| {
                    more(npos, caps, k)
                })
            {
                return true;
            }
            count >= piece.min && match_seq(rest, text, pos, caps, ci, k)
        } else {

            if count >= piece.min && match_seq(rest, text, pos, caps, ci, k) {
                return true;
            }
            can_more
                && match_atom(&piece.atom, text, pos, caps, ci, &mut |npos, caps| {
                    more(npos, caps, k)
                })
        }
    }
    rep(piece, rest, text, pos, 0, caps, ci, k)
}

fn match_seq(
    seq: &[Piece],
    text: &[char],
    pos: usize,
    caps: &mut Caps,
    ci: bool,
    k: &mut dyn FnMut(usize, &mut Caps) -> bool,
) -> bool {
    match seq.split_first() {
        None => k(pos, caps),
        Some((first, rest)) => match_piece(first, rest, text, pos, caps, ci, k),
    }
}

fn find_at(re: &Regex, text: &[char], from: usize, ci: bool) -> Option<Caps> {
    for start in from..=text.len() {
        let mut caps: Caps = vec![None; re.ngroups + 1];
        let mut hit_end: Option<usize> = None;
        for alt in &re.alts {
            let found = match_seq(alt, text, start, &mut caps, ci, &mut |end, _caps| {
                hit_end = Some(end);
                true
            });
            if found {
                break;
            }
        }
        if let Some(end) = hit_end {
            caps[0] = Some((start, end));
            return Some(caps);
        }
    }
    None
}

struct Flags {
    global: bool,
    ci: bool,
}

fn parse_flags(s: &str) -> Result<Flags, PgError> {
    let mut f = Flags {
        global: false,
        ci: false,
    };
    for c in s.chars() {
        match c {
            'g' => f.global = true,
            'i' => f.ci = true,
            'c' => f.ci = false,
            'p' | 'w' | 'n' | 'm' | 'x' | 's' | 't' | 'b' | 'e' | 'q' => {

            }
            other => {
                return Err(re_err(format!(
                    "invalid regular expression option: \"{other}\""
                )))
            }
        }
    }
    Ok(f)
}

fn as_text(v: &SqlValue) -> Option<String> {
    match v {
        SqlValue::Text(s) => Some(s.clone()),
        SqlValue::Int(n) => Some(n.to_string()),
        SqlValue::Real(f) => Some(f.to_string()),
        _ => None,
    }
}

fn as_int(v: &SqlValue) -> Option<i64> {
    match v {
        SqlValue::Int(n) => Some(*n),
        SqlValue::Text(s) => s.trim().parse().ok(),
        _ => None,
    }
}

pub fn call(name: &str, args: &[SqlValue]) -> Option<Result<SqlValue, PgError>> {
    if !matches!(
        name,
        "regexp_replace"
            | "regexp_like"
            | "regexp_count"
            | "regexp_substr"
            | "regexp_instr"
            | "regexp_match"
            | "regexp_split_to_array"
    ) {
        return None;
    }

    if args.iter().any(|a| matches!(a, SqlValue::Null)) {
        return Some(Ok(SqlValue::Null));
    }
    Some(match name {
        "regexp_replace" => regexp_replace(args),
        "regexp_like" => regexp_like(args),
        "regexp_count" => regexp_count(args),
        "regexp_substr" => regexp_substr(args),
        "regexp_instr" => regexp_instr(args),
        "regexp_match" => regexp_match(args),
        "regexp_split_to_array" => regexp_split_to_array(args),
        _ => unreachable!(),
    })
}

fn is_pg_space(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\u{0B}' | '\u{0C}' | '\r')
}

fn requote(s: &str) -> String {
    let needs = s.is_empty()
        || s.eq_ignore_ascii_case("NULL")
        || s.chars()
            .any(|c| matches!(c, '{' | '}' | ',' | '"' | '\\') || is_pg_space(c));
    if !needs {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        if c == '"' || c == '\\' {
            out.push('\\');
        }
        out.push(c);
    }
    out.push('"');
    out
}

fn build_text_array(elems: &[String]) -> String {
    if elems.is_empty() {
        return "{}".to_string();
    }
    let parts: Vec<String> = elems.iter().map(|s| requote(s)).collect();
    format!("{{{}}}", parts.join(","))
}

fn regexp_match(args: &[SqlValue]) -> Result<SqlValue, PgError> {
    if args.len() < 2 || args.len() > 3 {
        return Err(arity_err("regexp_match"));
    }
    let source = as_text(&args[0]).ok_or_else(|| arity_err("regexp_match"))?;
    let pattern = as_text(&args[1]).ok_or_else(|| arity_err("regexp_match"))?;
    let flags = if args.len() == 3 {
        parse_flags(&as_text(&args[2]).ok_or_else(|| arity_err("regexp_match"))?)?
    } else {
        Flags {
            global: false,
            ci: false,
        }
    };
    let re = compile(&pattern)?;
    let text: Vec<char> = source.chars().collect();
    let Some(caps) = find_at(&re, &text, 0, flags.ci) else {
        return Ok(SqlValue::Null);
    };
    let elems: Vec<String> = if re.ngroups == 0 {

        let (s, e) = caps[0].unwrap();
        vec![text[s..e].iter().collect()]
    } else {

        (1..=re.ngroups)
            .map(|i| match caps[i] {
                Some((s, e)) => text[s..e].iter().collect(),
                None => String::new(),
            })
            .collect()
    };
    Ok(SqlValue::Text(build_text_array(&elems)))
}

fn regexp_split_to_array(args: &[SqlValue]) -> Result<SqlValue, PgError> {
    if args.len() < 2 || args.len() > 3 {
        return Err(arity_err("regexp_split_to_array"));
    }
    let source = as_text(&args[0]).ok_or_else(|| arity_err("regexp_split_to_array"))?;
    let pattern = as_text(&args[1]).ok_or_else(|| arity_err("regexp_split_to_array"))?;
    let flags = if args.len() == 3 {
        parse_flags(&as_text(&args[2]).ok_or_else(|| arity_err("regexp_split_to_array"))?)?
    } else {
        Flags {
            global: false,
            ci: false,
        }
    };
    let re = compile(&pattern)?;
    let text: Vec<char> = source.chars().collect();
    let spans = all_matches(&re, &text, 0, flags.ci);

    if !spans.is_empty() && spans.iter().all(|(s, e)| s == e) {
        let pieces: Vec<String> = text.iter().map(|c| c.to_string()).collect();
        return Ok(SqlValue::Text(build_text_array(&pieces)));
    }

    let mut pieces: Vec<String> = Vec::new();
    let mut prev = 0usize;
    for (ms, me) in spans {
        pieces.push(text[prev..ms].iter().collect());
        prev = me;
    }
    pieces.push(text[prev..].iter().collect());
    Ok(SqlValue::Text(build_text_array(&pieces)))
}

pub(crate) fn regexp_matches_rows(args: &[SqlValue]) -> Result<Vec<SqlValue>, PgError> {
    if args.iter().any(|a| matches!(a, SqlValue::Null)) {
        return Ok(Vec::new());
    }
    if args.len() < 2 || args.len() > 3 {
        return Err(arity_err("regexp_matches"));
    }
    let source = as_text(&args[0]).ok_or_else(|| arity_err("regexp_matches"))?;
    let pattern = as_text(&args[1]).ok_or_else(|| arity_err("regexp_matches"))?;
    let flags = if args.len() == 3 {
        parse_flags(&as_text(&args[2]).ok_or_else(|| arity_err("regexp_matches"))?)?
    } else {
        Flags {
            global: false,
            ci: false,
        }
    };
    let re = compile(&pattern)?;
    let text: Vec<char> = source.chars().collect();
    let mut out = Vec::new();
    let mut pos = 0usize;
    while pos <= text.len() {
        let Some(caps) = find_at(&re, &text, pos, flags.ci) else {
            break;
        };
        let (ms, me) = caps[0].unwrap();
        let elems: Vec<String> = if re.ngroups == 0 {
            vec![text[ms..me].iter().collect()]
        } else {
            (1..=re.ngroups)
                .map(|i| match caps[i] {
                    Some((s, e)) => text[s..e].iter().collect(),
                    None => String::new(),
                })
                .collect()
        };
        out.push(SqlValue::Text(build_text_array(&elems)));
        if !flags.global {
            break;
        }
        pos = if me > ms { me } else { me + 1 };
    }
    Ok(out)
}

pub(crate) fn regexp_split_to_table_rows(args: &[SqlValue]) -> Result<Vec<SqlValue>, PgError> {
    if args.iter().any(|a| matches!(a, SqlValue::Null)) {
        return Ok(Vec::new());
    }
    if args.len() < 2 || args.len() > 3 {
        return Err(arity_err("regexp_split_to_table"));
    }
    let source = as_text(&args[0]).ok_or_else(|| arity_err("regexp_split_to_table"))?;
    let pattern = as_text(&args[1]).ok_or_else(|| arity_err("regexp_split_to_table"))?;
    let flags = if args.len() == 3 {
        parse_flags(&as_text(&args[2]).ok_or_else(|| arity_err("regexp_split_to_table"))?)?
    } else {
        Flags {
            global: false,
            ci: false,
        }
    };
    let re = compile(&pattern)?;
    let text: Vec<char> = source.chars().collect();
    let spans = all_matches(&re, &text, 0, flags.ci);

    let pieces: Vec<String> = if !spans.is_empty() && spans.iter().all(|(s, e)| s == e) {
        text.iter().map(|c| c.to_string()).collect()
    } else {
        let mut pieces = Vec::new();
        let mut prev = 0usize;
        for (ms, me) in spans {
            pieces.push(text[prev..ms].iter().collect());
            prev = me;
        }
        pieces.push(text[prev..].iter().collect());
        pieces
    };
    Ok(pieces.into_iter().map(SqlValue::Text).collect())
}

fn arity_err(name: &str) -> PgError {
    PgError::InvalidInputSyntax {
        typ: "expression",
        input: format!("function {name}(...) does not exist"),
    }
}

fn start_index(start: i64, _name: &str) -> Result<usize, PgError> {
    if start < 1 {
        return Err(re_err("argument 'start' must be a positive integer".into()));
    }
    Ok((start - 1) as usize)
}

fn regexp_replace(args: &[SqlValue]) -> Result<SqlValue, PgError> {
    if args.len() < 3 || args.len() > 4 {
        return Err(arity_err("regexp_replace"));
    }
    let source = as_text(&args[0]).ok_or_else(|| arity_err("regexp_replace"))?;
    let pattern = as_text(&args[1]).ok_or_else(|| arity_err("regexp_replace"))?;
    let replacement = as_text(&args[2]).ok_or_else(|| arity_err("regexp_replace"))?;
    let flags = if args.len() == 4 {
        parse_flags(&as_text(&args[3]).ok_or_else(|| arity_err("regexp_replace"))?)?
    } else {
        Flags {
            global: false,
            ci: false,
        }
    };
    let re = compile(&pattern)?;
    let text: Vec<char> = source.chars().collect();

    let mut out = String::new();
    let mut pos = 0usize;
    let mut replaced = false;
    while pos <= text.len() {
        if !flags.global && replaced {
            break;
        }
        let Some(caps) = find_at(&re, &text, pos, flags.ci) else {
            break;
        };
        let (ms, me) = caps[0].unwrap();

        out.extend(&text[pos..ms]);
        expand_replacement(&replacement, &text, &caps, &mut out);
        replaced = true;
        if me > ms {
            pos = me;
        } else {

            if me < text.len() {
                out.push(text[me]);
            }
            pos = me + 1;
        }
        if !flags.global {

            if pos <= text.len() {
                out.extend(&text[pos..]);
            }
            return Ok(SqlValue::Text(out));
        }
    }
    if pos < text.len() {
        out.extend(&text[pos..]);
    }
    Ok(SqlValue::Text(out))
}

fn expand_replacement(repl: &str, text: &[char], caps: &Caps, out: &mut String) {
    let rc: Vec<char> = repl.chars().collect();
    let mut i = 0;
    while i < rc.len() {
        let c = rc[i];
        if c == '\\' && i + 1 < rc.len() {
            let n = rc[i + 1];
            if n.is_ascii_digit() {
                let g = n as usize - '0' as usize;
                if let Some(Some((s, e))) = caps.get(g) {
                    out.extend(&text[*s..*e]);
                }
                i += 2;
                continue;
            } else if n == '&' {
                if let Some((s, e)) = caps[0] {
                    out.extend(&text[s..e]);
                }
                i += 2;
                continue;
            } else if n == '\\' {
                out.push('\\');
                i += 2;
                continue;
            }
        }
        out.push(c);
        i += 1;
    }
}

fn regexp_like(args: &[SqlValue]) -> Result<SqlValue, PgError> {
    if args.len() < 2 || args.len() > 3 {
        return Err(arity_err("regexp_like"));
    }
    let source = as_text(&args[0]).ok_or_else(|| arity_err("regexp_like"))?;
    let pattern = as_text(&args[1]).ok_or_else(|| arity_err("regexp_like"))?;
    let flags = if args.len() == 3 {
        parse_flags(&as_text(&args[2]).ok_or_else(|| arity_err("regexp_like"))?)?
    } else {
        Flags {
            global: false,
            ci: false,
        }
    };
    let re = compile(&pattern)?;
    let text: Vec<char> = source.chars().collect();
    Ok(SqlValue::Int(
        find_at(&re, &text, 0, flags.ci).is_some() as i64
    ))
}

fn regexp_count(args: &[SqlValue]) -> Result<SqlValue, PgError> {
    if args.len() < 2 || args.len() > 4 {
        return Err(arity_err("regexp_count"));
    }
    let source = as_text(&args[0]).ok_or_else(|| arity_err("regexp_count"))?;
    let pattern = as_text(&args[1]).ok_or_else(|| arity_err("regexp_count"))?;
    let start = if args.len() >= 3 {
        start_index(
            as_int(&args[2]).ok_or_else(|| arity_err("regexp_count"))?,
            "regexp_count",
        )?
    } else {
        0
    };
    let flags = if args.len() == 4 {
        parse_flags(&as_text(&args[3]).ok_or_else(|| arity_err("regexp_count"))?)?
    } else {
        Flags {
            global: false,
            ci: false,
        }
    };
    let re = compile(&pattern)?;
    let text: Vec<char> = source.chars().collect();
    let mut pos = start.min(text.len());
    let mut count = 0i64;
    while pos <= text.len() {
        let Some(caps) = find_at(&re, &text, pos, flags.ci) else {
            break;
        };
        let (ms, me) = caps[0].unwrap();
        count += 1;
        pos = if me > ms { me } else { me + 1 };
        let _ = ms;
    }
    Ok(SqlValue::Int(count))
}

fn all_matches(re: &Regex, text: &[char], start: usize, ci: bool) -> Vec<(usize, usize)> {
    let mut spans = Vec::new();
    let mut pos = start.min(text.len());
    while pos <= text.len() {
        let Some(caps) = find_at(re, text, pos, ci) else {
            break;
        };
        let (ms, me) = caps[0].unwrap();
        spans.push((ms, me));
        pos = if me > ms { me } else { me + 1 };
    }
    spans
}

fn regexp_substr(args: &[SqlValue]) -> Result<SqlValue, PgError> {
    if args.len() < 2 || args.len() > 5 {
        return Err(arity_err("regexp_substr"));
    }
    let source = as_text(&args[0]).ok_or_else(|| arity_err("regexp_substr"))?;
    let pattern = as_text(&args[1]).ok_or_else(|| arity_err("regexp_substr"))?;
    let start = if args.len() >= 3 {
        start_index(
            as_int(&args[2]).ok_or_else(|| arity_err("regexp_substr"))?,
            "regexp_substr",
        )?
    } else {
        0
    };
    let nth = if args.len() >= 4 {
        let n = as_int(&args[3]).ok_or_else(|| arity_err("regexp_substr"))?;
        if n < 1 {
            return Err(re_err("argument 'n' must be a positive integer".into()));
        }
        n as usize
    } else {
        1
    };
    let flags = if args.len() == 5 {
        parse_flags(&as_text(&args[4]).ok_or_else(|| arity_err("regexp_substr"))?)?
    } else {
        Flags {
            global: false,
            ci: false,
        }
    };
    let re = compile(&pattern)?;
    let text: Vec<char> = source.chars().collect();
    let spans = all_matches(&re, &text, start, flags.ci);
    match spans.get(nth - 1) {
        Some((s, e)) => Ok(SqlValue::Text(text[*s..*e].iter().collect())),
        None => Ok(SqlValue::Null),
    }
}

fn regexp_instr(args: &[SqlValue]) -> Result<SqlValue, PgError> {
    if args.len() < 2 || args.len() > 6 {
        return Err(arity_err("regexp_instr"));
    }
    let source = as_text(&args[0]).ok_or_else(|| arity_err("regexp_instr"))?;
    let pattern = as_text(&args[1]).ok_or_else(|| arity_err("regexp_instr"))?;
    let start = if args.len() >= 3 {
        start_index(
            as_int(&args[2]).ok_or_else(|| arity_err("regexp_instr"))?,
            "regexp_instr",
        )?
    } else {
        0
    };
    let nth = if args.len() >= 4 {
        let n = as_int(&args[3]).ok_or_else(|| arity_err("regexp_instr"))?;
        if n < 1 {
            return Err(re_err("argument 'n' must be a positive integer".into()));
        }
        n as usize
    } else {
        1
    };
    let endoption = if args.len() >= 5 {
        as_int(&args[4]).ok_or_else(|| arity_err("regexp_instr"))?
    } else {
        0
    };
    let flags = if args.len() == 6 {
        parse_flags(&as_text(&args[5]).ok_or_else(|| arity_err("regexp_instr"))?)?
    } else {
        Flags {
            global: false,
            ci: false,
        }
    };
    let re = compile(&pattern)?;
    let text: Vec<char> = source.chars().collect();
    let spans = all_matches(&re, &text, start, flags.ci);
    match spans.get(nth - 1) {

        Some((s, e)) => Ok(SqlValue::Int(if endoption == 0 {
            *s as i64 + 1
        } else {
            *e as i64 + 1
        })),
        None => Ok(SqlValue::Int(0)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(name: &str, args: &[SqlValue]) -> SqlValue {
        super::call(name, args).unwrap().unwrap()
    }
    fn txt(s: &str) -> SqlValue {
        SqlValue::Text(s.to_string())
    }

    #[test]
    fn replace_first_and_global() {
        assert_eq!(
            t("regexp_replace", &[txt("foobarbaz"), txt("b.."), txt("X")]),
            txt("fooXbaz")
        );
        assert_eq!(
            t(
                "regexp_replace",
                &[txt("foobarbaz"), txt("b.."), txt("X"), txt("g")]
            ),
            txt("fooXX")
        );
    }

    #[test]
    fn replace_backrefs() {
        assert_eq!(
            t(
                "regexp_replace",
                &[txt("A PostgreSQL"), txt(r"(\w+)\s(\w+)"), txt(r"\2 \1")]
            ),
            txt("PostgreSQL A")
        );
        assert_eq!(
            t("regexp_replace", &[txt("abc"), txt("b"), txt(r"[\&]")]),
            txt("a[b]c")
        );
    }

    #[test]
    fn like_and_ci() {
        assert_eq!(t("regexp_like", &[txt("abc"), txt("^a")]), SqlValue::Int(1));
        assert_eq!(t("regexp_like", &[txt("ABC"), txt("^a")]), SqlValue::Int(0));
        assert_eq!(
            t("regexp_like", &[txt("ABC"), txt("^a"), txt("i")]),
            SqlValue::Int(1)
        );
        assert_eq!(
            t("regexp_like", &[txt("xyz"), txt("[[:digit:]]")]),
            SqlValue::Int(0)
        );
        assert_eq!(
            t("regexp_like", &[txt("x1z"), txt(r"\d")]),
            SqlValue::Int(1)
        );
    }

    #[test]
    fn count_substr_instr() {
        assert_eq!(
            t("regexp_count", &[txt("ababab"), txt("ab")]),
            SqlValue::Int(3)
        );
        assert_eq!(
            t(
                "regexp_count",
                &[txt("ababab"), txt("ab"), SqlValue::Int(3)]
            ),
            SqlValue::Int(2)
        );
        assert_eq!(
            t(
                "regexp_substr",
                &[
                    txt("foobarbaz"),
                    txt("b(..)"),
                    SqlValue::Int(1),
                    SqlValue::Int(2)
                ]
            ),
            txt("baz")
        );
        assert_eq!(
            t(
                "regexp_substr",
                &[
                    txt("foobarbaz"),
                    txt("q.."),
                    SqlValue::Int(1),
                    SqlValue::Int(1)
                ]
            ),
            SqlValue::Null
        );
        assert_eq!(
            t(
                "regexp_instr",
                &[
                    txt("foobarbaz"),
                    txt("b.."),
                    SqlValue::Int(1),
                    SqlValue::Int(2)
                ]
            ),
            SqlValue::Int(7)
        );
        assert_eq!(
            t(
                "regexp_instr",
                &[
                    txt("foobarbaz"),
                    txt("b.."),
                    SqlValue::Int(1),
                    SqlValue::Int(1),
                    SqlValue::Int(1)
                ]
            ),
            SqlValue::Int(7)
        );
    }

    #[test]
    fn quantifiers_and_alternation() {
        assert_eq!(
            t("regexp_like", &[txt("aaa"), txt("^a{2,3}$")]),
            SqlValue::Int(1)
        );
        assert_eq!(
            t("regexp_like", &[txt("aaaa"), txt("^a{2,3}$")]),
            SqlValue::Int(0)
        );
        assert_eq!(
            t("regexp_like", &[txt("cat"), txt("^(cat|dog)$")]),
            SqlValue::Int(1)
        );
        assert_eq!(
            t(
                "regexp_replace",
                &[txt("a.b.c"), txt(r"\."), txt("-"), txt("g")]
            ),
            txt("a-b-c")
        );
    }

    #[test]
    fn null_is_null() {
        assert_eq!(
            t("regexp_like", &[SqlValue::Null, txt("a")]),
            SqlValue::Null
        );
        assert_eq!(
            t("regexp_count", &[txt("aa"), SqlValue::Null]),
            SqlValue::Null
        );
    }

    #[test]
    fn regexp_match_groups_and_whole() {

        assert_eq!(
            t(
                "regexp_match",
                &[txt("foobarbequebaz"), txt("(bar)(beque)")]
            ),
            txt("{bar,beque}")
        );

        assert_eq!(
            t("regexp_match", &[txt("foobarbequebaz"), txt("barbeque")]),
            txt("{barbeque}")
        );

        assert_eq!(
            t("regexp_match", &[txt("foobar"), txt("(zzz)")]),
            SqlValue::Null
        );

        assert_eq!(
            t("regexp_match", &[txt("FOOBAR"), txt("(bar)"), txt("i")]),
            txt("{BAR}")
        );

        assert_eq!(
            t("regexp_match", &[txt("a foo b"), txt("(foo b)")]),
            txt("{\"foo b\"}")
        );
    }

    #[test]
    fn regexp_split_to_array_cases() {
        assert_eq!(
            t(
                "regexp_split_to_array",
                &[txt("the quick brown fox"), txt(r"\s+")]
            ),
            txt("{the,quick,brown,fox}")
        );

        assert_eq!(
            t("regexp_split_to_array", &[txt("123"), txt("")]),
            txt("{1,2,3}")
        );

        assert_eq!(
            t("regexp_split_to_array", &[txt("aXXbXXc"), txt("XX")]),
            txt("{a,b,c}")
        );

        assert_eq!(
            t("regexp_split_to_array", &[txt(",a,"), txt(",")]),
            txt("{\"\",a,\"\"}")
        );

        assert_eq!(
            t("regexp_split_to_array", &[txt("hello"), txt("zzz")]),
            txt("{hello}")
        );
    }
}
