
use crate::types::PgError;
use sql_core::SqlValue;

fn bad_path(input: &str) -> PgError {
    PgError::InvalidInputSyntax {
        typ: "jsonpath",
        input: input.to_string(),
    }
}

fn does_not_exist(name: &str) -> PgError {
    PgError::InvalidInputSyntax {
        typ: "expression",
        input: format!("function {name}(...) does not exist"),
    }
}

pub fn call(name: &str, args: &[SqlValue]) -> Option<Result<SqlValue, PgError>> {
    match name {
        "jsonb_path_exists" => Some(exists_fn(name, args)),

        "jsonb_path_query" | "jsonb_path_query_first" => Some(query_first_fn(name, args)),

        "jsonb_path_query_array" => Some(query_array_fn(name, args)),
        "jsonb_path_match" => Some(match_fn(name, args)),
        _ => None,
    }
}

pub fn query_all(target: &str, path: &str) -> Result<Vec<String>, PgError> {
    let root = parse_json(target).ok_or_else(|| bad_path(path))?;
    let items = match parse_program(path).map_err(|()| bad_path(path))? {
        Program::Path(p) => eval_path(&p, &root, &root),
        Program::Pred(pred) => vec![Json::Bool(eval_pred(&pred, &root, &root))],
    };
    Ok(items
        .iter()
        .map(|j| {
            let mut out = String::new();
            serialize(j, &mut out);
            out
        })
        .collect())
}

fn query_array_fn(name: &str, args: &[SqlValue]) -> Result<SqlValue, PgError> {
    let (target, path) = match two_text(name, args)? {
        None => return Ok(SqlValue::Null),
        Some(t) => t,
    };
    let matches = query_all(target, path)?;
    let mut out = String::from("[");
    for (i, m) in matches.iter().enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        out.push_str(m);
    }
    out.push(']');
    Ok(SqlValue::Text(out))
}

fn two_text<'a>(name: &str, args: &'a [SqlValue]) -> Result<Option<(&'a str, &'a str)>, PgError> {
    if args.len() != 2 {
        return Err(does_not_exist(name));
    }
    let target = match &args[0] {
        SqlValue::Null => return Ok(None),
        SqlValue::Text(s) => s.as_str(),
        _ => return Err(does_not_exist(name)),
    };
    let path = match &args[1] {
        SqlValue::Null => return Ok(None),
        SqlValue::Text(s) => s.as_str(),
        _ => return Err(does_not_exist(name)),
    };
    Ok(Some((target, path)))
}

fn exists_fn(name: &str, args: &[SqlValue]) -> Result<SqlValue, PgError> {
    let (target, path) = match two_text(name, args)? {
        None => return Ok(SqlValue::Null),
        Some(t) => t,
    };
    let b = path_exists(target, path)?;
    Ok(SqlValue::Int(if b { 1 } else { 0 }))
}

fn query_first_fn(name: &str, args: &[SqlValue]) -> Result<SqlValue, PgError> {
    let (target, path) = match two_text(name, args)? {
        None => return Ok(SqlValue::Null),
        Some(t) => t,
    };
    match path_query_first(target, path)? {
        Some(j) => {
            let mut out = String::new();
            serialize(&j, &mut out);
            Ok(SqlValue::Text(out))
        }
        None => Ok(SqlValue::Null),
    }
}

fn match_fn(name: &str, args: &[SqlValue]) -> Result<SqlValue, PgError> {
    let (target, path) = match two_text(name, args)? {
        None => return Ok(SqlValue::Null),
        Some(t) => t,
    };
    match path_match(target, path)? {
        Some(b) => Ok(SqlValue::Int(if b { 1 } else { 0 })),
        None => Ok(SqlValue::Null),
    }
}

pub fn op_exists(l: &SqlValue, r: &SqlValue) -> Result<SqlValue, PgError> {
    let (target, path) = match (l, r) {
        (SqlValue::Text(t), SqlValue::Text(p)) => (t.as_str(), p.as_str()),
        _ => return Ok(SqlValue::Null),
    };
    Ok(SqlValue::Int(if path_exists(target, path)? {
        1
    } else {
        0
    }))
}

pub fn is_json_target(v: &SqlValue) -> bool {
    matches!(v, SqlValue::Text(t) if parse_json(t).is_some())
}

pub fn looks_like_jsonpath(v: &SqlValue) -> bool {
    let s = match v {
        SqlValue::Text(t) => t.as_str(),
        _ => return false,
    };
    let rest = s.trim_start();

    let rest = ["strict", "lax"]
        .iter()
        .find_map(|w| {
            rest.strip_prefix(w)
                .filter(|r| r.starts_with(char::is_whitespace))
        })
        .map(|r| r.trim_start())
        .unwrap_or(rest);
    rest.starts_with('$')
}

pub fn op_match(l: &SqlValue, r: &SqlValue) -> Result<SqlValue, PgError> {
    let (target, path) = match (l, r) {
        (SqlValue::Text(t), SqlValue::Text(p)) => (t.as_str(), p.as_str()),
        _ => return Ok(SqlValue::Null),
    };
    match path_match(target, path)? {
        Some(b) => Ok(SqlValue::Int(if b { 1 } else { 0 })),
        None => Ok(SqlValue::Null),
    }
}

fn path_exists(target: &str, path: &str) -> Result<bool, PgError> {
    let root = parse_json(target).ok_or_else(|| bad_path(path))?;
    match parse_program(path).map_err(|()| bad_path(path))? {
        Program::Path(p) => Ok(!eval_path(&p, &root, &root).is_empty()),
        Program::Pred(pred) => Ok(eval_pred(&pred, &root, &root)),
    }
}

fn path_query_first(target: &str, path: &str) -> Result<Option<Json>, PgError> {
    let root = parse_json(target).ok_or_else(|| bad_path(path))?;
    match parse_program(path).map_err(|()| bad_path(path))? {
        Program::Path(p) => Ok(eval_path(&p, &root, &root).into_iter().next()),
        Program::Pred(pred) => Ok(Some(Json::Bool(eval_pred(&pred, &root, &root)))),
    }
}

fn path_match(target: &str, path: &str) -> Result<Option<bool>, PgError> {
    let root = parse_json(target).ok_or_else(|| bad_path(path))?;
    match parse_program(path).map_err(|()| bad_path(path))? {
        Program::Pred(pred) => Ok(Some(eval_pred(&pred, &root, &root))),
        Program::Path(p) => {
            let mut items = eval_path(&p, &root, &root).into_iter();
            match (items.next(), items.next()) {
                (Some(Json::Bool(b)), None) => Ok(Some(b)),
                _ => Ok(None),
            }
        }
    }
}

fn eval_path(p: &PathExpr, root: &Json, current: &Json) -> Vec<Json> {
    let start = match p.origin {
        Origin::Root => root.clone(),
        Origin::Current => current.clone(),
    };
    let mut stream = vec![start];
    for acc in &p.accs {
        let mut next: Vec<Json> = Vec::new();
        for v in &stream {
            match acc {
                Accessor::Key(k) => {
                    if let Json::Obj(members) = v {
                        for (mk, mv) in members {
                            if mk == k {
                                next.push(mv.clone());
                            }
                        }
                    }
                }
                Accessor::WildKey => {
                    if let Json::Obj(members) = v {
                        for (_, mv) in members {
                            next.push(mv.clone());
                        }
                    }
                }
                Accessor::Index(b) => {
                    if let Json::Arr(items) = v {
                        let i = b.resolve(items.len());
                        if i >= 0 {
                            if let Some(item) = items.get(i as usize) {
                                next.push(item.clone());
                            }
                        }
                    }
                }
                Accessor::Range(lo, hi) => {
                    if let Json::Arr(items) = v {
                        let n = items.len();
                        let a = lo.resolve(n).max(0);
                        let b = hi.resolve(n);
                        let mut i = a;
                        while i <= b {
                            if let Some(item) = items.get(i as usize) {
                                next.push(item.clone());
                            }
                            i += 1;
                        }
                    }
                }
                Accessor::WildIndex => {
                    if let Json::Arr(items) = v {
                        for item in items {
                            next.push(item.clone());
                        }
                    }
                }
                Accessor::Recursive => descend(v, &mut next),
                Accessor::Method(m) => {
                    if let Some(r) = apply_method(*m, v) {
                        next.push(r);
                    }
                }
                Accessor::Filter(pred) => {
                    if eval_pred(pred, root, v) {
                        next.push(v.clone());
                    }
                }
            }
        }
        stream = next;
    }
    stream
}

fn descend(v: &Json, out: &mut Vec<Json>) {
    out.push(v.clone());
    match v {
        Json::Arr(items) => {
            for item in items {
                descend(item, out);
            }
        }
        Json::Obj(members) => {
            for (_, mv) in members {
                descend(mv, out);
            }
        }
        _ => {}
    }
}

fn apply_method(m: Method, v: &Json) -> Option<Json> {
    match m {

        Method::Size => Some(Json::Num(match v {
            Json::Arr(items) => items.len().to_string(),
            _ => "1".to_string(),
        })),
        Method::Type => Some(Json::Str(json_type(v).to_string())),
        Method::Double => num_of(v).map(|x| Json::Num(fmt_num(x))),
        Method::Floor => num_of(v).map(|x| Json::Num(fmt_num(x.floor()))),
        Method::Ceiling => num_of(v).map(|x| Json::Num(fmt_num(x.ceil()))),
        Method::Abs => num_of(v).map(|x| Json::Num(fmt_num(x.abs()))),
    }
}

fn json_type(v: &Json) -> &'static str {
    match v {
        Json::Null => "null",
        Json::Bool(_) => "boolean",
        Json::Num(_) => "number",
        Json::Str(_) => "string",
        Json::Arr(_) => "array",
        Json::Obj(_) => "object",
    }
}

fn num_of(v: &Json) -> Option<f64> {
    match v {
        Json::Num(n) => n.parse::<f64>().ok(),
        Json::Str(s) => s.trim().parse::<f64>().ok(),
        _ => None,
    }
}

fn fmt_num(x: f64) -> String {
    format!("{x}")
}

fn eval_pred(pred: &Pred, root: &Json, current: &Json) -> bool {
    match pred {
        Pred::And(a, b) => eval_pred(a, root, current) && eval_pred(b, root, current),
        Pred::Or(a, b) => eval_pred(a, root, current) || eval_pred(b, root, current),
        Pred::Exists(p) => !eval_path(p, root, current).is_empty(),
        Pred::LikeRegex(operand, pattern, ci) => {

            eval_operand(operand, root, current)
                .iter()
                .any(|v| match v {
                    Json::Str(s) => {
                        crate::expr::functions::regexp::like_regex_match(s, pattern, *ci)
                            .unwrap_or(false)
                    }
                    _ => false,
                })
        }
        Pred::Cmp(op, l, r) => {
            let ls = eval_operand(l, root, current);
            let rs = eval_operand(r, root, current);

            ls.iter()
                .any(|lv| rs.iter().any(|rv| cmp_scalar(*op, lv, rv)))
        }
    }
}

fn eval_operand(o: &Operand, root: &Json, current: &Json) -> Vec<Json> {
    match o {
        Operand::Lit(j) => vec![j.clone()],
        Operand::Path(p) => eval_path(p, root, current),
    }
}

fn cmp_scalar(op: CmpOp, l: &Json, r: &Json) -> bool {
    use std::cmp::Ordering;
    let ord: Option<Ordering> = match (l, r) {
        (Json::Num(a), Json::Num(b)) => match (a.parse::<f64>(), b.parse::<f64>()) {
            (Ok(x), Ok(y)) => x.partial_cmp(&y),
            _ => None,
        },
        (Json::Str(a), Json::Str(b)) => Some(a.cmp(b)),
        (Json::Bool(a), Json::Bool(b)) => Some(a.cmp(b)),
        (Json::Null, Json::Null) => Some(Ordering::Equal),
        _ => None,
    };
    match ord {
        None => false,
        Some(o) => match op {
            CmpOp::Eq => o == Ordering::Equal,
            CmpOp::Ne => o != Ordering::Equal,
            CmpOp::Lt => o == Ordering::Less,
            CmpOp::Le => o != Ordering::Greater,
            CmpOp::Gt => o == Ordering::Greater,
            CmpOp::Ge => o != Ordering::Less,
        },
    }
}

#[derive(Clone, Copy, PartialEq)]
enum CmpOp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

#[derive(Clone, Copy)]
enum Origin {
    Root,
    Current,
}

enum Accessor {
    Key(String),
    WildKey,

    Index(IdxBound),

    Range(IdxBound, IdxBound),
    WildIndex,

    Recursive,

    Method(Method),
    Filter(Pred),
}

#[derive(Clone, Copy)]
enum IdxBound {
    Num(i64),
    Last,
}

impl IdxBound {

    fn resolve(self, len: usize) -> i64 {
        match self {
            IdxBound::Num(n) => n,
            IdxBound::Last => len as i64 - 1,
        }
    }
}

#[derive(Clone, Copy)]
enum Method {
    Size,
    Type,
    Double,
    Floor,
    Ceiling,
    Abs,
}

struct PathExpr {
    origin: Origin,
    accs: Vec<Accessor>,
}

enum Operand {
    Path(PathExpr),
    Lit(Json),
}

enum Pred {
    Cmp(CmpOp, Operand, Operand),
    Exists(PathExpr),

    LikeRegex(Operand, String, bool),
    And(Box<Pred>, Box<Pred>),
    Or(Box<Pred>, Box<Pred>),
}

enum Program {
    Path(PathExpr),
    Pred(Pred),
}

fn parse_program(src: &str) -> Result<Program, ()> {
    let chars: Vec<char> = src.chars().collect();
    let mut p = PParser { c: &chars, pos: 0 };
    p.ws();
    p.mode_word();
    p.ws();

    let first = p.operand()?;
    p.ws();
    if p.at_pred_continuation() {
        let pred = p.pred_from(first)?;
        p.ws();
        if p.pos != p.c.len() {
            return Err(());
        }
        Ok(Program::Pred(pred))
    } else {
        p.ws();
        if p.pos != p.c.len() {
            return Err(());
        }
        match first {
            Operand::Path(path) => Ok(Program::Path(path)),

            Operand::Lit(_) => Err(()),
        }
    }
}

struct PParser<'a> {
    c: &'a [char],
    pos: usize,
}

impl<'a> PParser<'a> {
    fn peek(&self) -> Option<char> {
        self.c.get(self.pos).copied()
    }
    fn peek2(&self) -> Option<char> {
        self.c.get(self.pos + 1).copied()
    }
    fn ws(&mut self) {
        while matches!(self.peek(), Some(c) if c.is_whitespace()) {
            self.pos += 1;
        }
    }

    fn mode_word(&mut self) {
        for word in ["strict", "lax"] {
            let w: Vec<char> = word.chars().collect();
            if self.c[self.pos..].starts_with(&w[..]) {

                let after = self.c.get(self.pos + w.len()).copied();
                if matches!(after, Some(c) if c.is_whitespace()) || after == Some('$') {
                    self.pos += w.len();
                    return;
                }
            }
        }
    }

    fn at_pred_continuation(&self) -> bool {
        match self.peek() {
            Some('=') => self.peek2() == Some('='),
            Some('!') => self.peek2() == Some('='),
            Some('<') | Some('>') => true,
            Some('&') => self.peek2() == Some('&'),
            Some('|') => self.peek2() == Some('|'),

            Some('l') => self.c[self.pos..].starts_with(&['l', 'i', 'k', 'e', '_']),
            _ => false,
        }
    }

    fn pred_from(&mut self, first: Operand) -> Result<Pred, ()> {
        let lhs = self.cmp_from(first)?;
        self.logical_chain(lhs)
    }

    fn cmp_from(&mut self, first: Operand) -> Result<Pred, ()> {
        self.ws();
        if self.kw("like_regex") {
            return self.like_regex_from(first);
        }
        if let Some(op) = self.cmp_op() {
            self.ws();
            let right = self.operand()?;
            Ok(Pred::Cmp(op, first, right))
        } else {

            match first {
                Operand::Path(p) => Ok(Pred::Exists(p)),
                Operand::Lit(_) => Err(()),
            }
        }
    }

    fn like_regex_from(&mut self, first: Operand) -> Result<Pred, ()> {
        self.ws();
        if self.peek() != Some('"') {
            return Err(());
        }
        let pattern = self.quoted()?;
        let mut ci = false;
        self.ws();
        if self.kw("flag") {
            self.ws();
            if self.peek() != Some('"') {
                return Err(());
            }
            let flags = self.quoted()?;
            for f in flags.chars() {
                match f {
                    'i' => ci = true,
                    'c' => ci = false,

                    's' | 'm' | 'x' | 'q' => {}
                    _ => return Err(()),
                }
            }
        }

        crate::expr::functions::regexp::like_regex_match("", &pattern, ci).map_err(|_| ())?;
        Ok(Pred::LikeRegex(first, pattern, ci))
    }

    fn logical_chain(&mut self, lhs: Pred) -> Result<Pred, ()> {
        let mut acc = lhs;
        loop {
            self.ws();
            if self.peek() == Some('&') && self.peek2() == Some('&') {
                self.pos += 2;
                self.ws();
                let rhs = self.pred_atom()?;
                acc = Pred::And(Box::new(acc), Box::new(rhs));
            } else if self.peek() == Some('|') && self.peek2() == Some('|') {
                self.pos += 2;
                self.ws();
                let rhs = self.pred_atom()?;
                acc = Pred::Or(Box::new(acc), Box::new(rhs));
            } else {
                break;
            }
        }
        Ok(acc)
    }

    fn pred_atom(&mut self) -> Result<Pred, ()> {
        self.ws();
        if self.peek() == Some('(') {
            self.pos += 1;
            let inner = self.predicate()?;
            self.ws();
            if self.peek() != Some(')') {
                return Err(());
            }
            self.pos += 1;
            return Ok(inner);
        }
        let first = self.operand()?;
        self.cmp_from(first)
    }

    fn predicate(&mut self) -> Result<Pred, ()> {
        let atom = self.pred_atom()?;
        self.logical_chain(atom)
    }

    fn cmp_op(&mut self) -> Option<CmpOp> {
        match self.peek() {
            Some('=') if self.peek2() == Some('=') => {
                self.pos += 2;
                Some(CmpOp::Eq)
            }
            Some('!') if self.peek2() == Some('=') => {
                self.pos += 2;
                Some(CmpOp::Ne)
            }
            Some('<') if self.peek2() == Some('=') => {
                self.pos += 2;
                Some(CmpOp::Le)
            }
            Some('>') if self.peek2() == Some('=') => {
                self.pos += 2;
                Some(CmpOp::Ge)
            }
            Some('<') => {
                self.pos += 1;
                Some(CmpOp::Lt)
            }
            Some('>') => {
                self.pos += 1;
                Some(CmpOp::Gt)
            }
            _ => None,
        }
    }

    fn operand(&mut self) -> Result<Operand, ()> {
        self.ws();
        match self.peek() {
            Some('$') => Ok(Operand::Path(self.path(Origin::Root)?)),
            Some('@') => Ok(Operand::Path(self.path(Origin::Current)?)),
            Some('"') => Ok(Operand::Lit(Json::Str(self.quoted()?))),
            Some(c) if c == '-' || c.is_ascii_digit() => Ok(Operand::Lit(self.number()?)),
            Some('t') | Some('f') | Some('n') => Ok(Operand::Lit(self.keyword()?)),
            _ => Err(()),
        }
    }

    fn path(&mut self, origin: Origin) -> Result<PathExpr, ()> {
        self.pos += 1;
        let mut accs = Vec::new();
        loop {

            self.ws();
            match self.peek() {
                Some('.') => {
                    self.pos += 1;
                    match self.peek() {

                        Some('*') if self.peek2() == Some('*') => {
                            self.pos += 2;
                            accs.push(Accessor::Recursive);
                        }
                        Some('*') => {
                            self.pos += 1;
                            accs.push(Accessor::WildKey);
                        }
                        Some('"') => accs.push(Accessor::Key(self.quoted()?)),
                        Some(c) if is_ident_start(c) => {
                            let name = self.ident();

                            self.ws();
                            if self.peek() == Some('(') {
                                self.pos += 1;
                                self.ws();
                                if self.peek() != Some(')') {
                                    return Err(());
                                }
                                self.pos += 1;
                                accs.push(Accessor::Method(method_from(&name)?));
                            } else {
                                accs.push(Accessor::Key(name));
                            }
                        }
                        _ => return Err(()),
                    }
                }
                Some('[') => {
                    self.pos += 1;
                    self.ws();
                    if self.peek() == Some('*') {
                        self.pos += 1;
                        self.ws();
                        if self.peek() != Some(']') {
                            return Err(());
                        }
                        self.pos += 1;
                        accs.push(Accessor::WildIndex);
                    } else {
                        let lo = self.subscript()?;
                        self.ws();

                        if self.kw("to") {
                            self.ws();
                            let hi = self.subscript()?;
                            self.ws();
                            if self.peek() != Some(']') {
                                return Err(());
                            }
                            self.pos += 1;
                            accs.push(Accessor::Range(lo, hi));
                        } else {
                            if self.peek() != Some(']') {
                                return Err(());
                            }
                            self.pos += 1;
                            accs.push(Accessor::Index(lo));
                        }
                    }
                }
                Some('?') => {
                    self.pos += 1;
                    self.ws();
                    if self.peek() != Some('(') {
                        return Err(());
                    }
                    self.pos += 1;
                    let pred = self.predicate()?;
                    self.ws();
                    if self.peek() != Some(')') {
                        return Err(());
                    }
                    self.pos += 1;
                    accs.push(Accessor::Filter(pred));
                }
                _ => break,
            }
        }
        Ok(PathExpr { origin, accs })
    }

    fn ident(&mut self) -> String {
        let start = self.pos;
        while matches!(self.peek(), Some(c) if is_ident_part(c)) {
            self.pos += 1;
        }
        self.c[start..self.pos].iter().collect()
    }

    fn subscript(&mut self) -> Result<IdxBound, ()> {
        if self.kw("last") {
            return Ok(IdxBound::Last);
        }
        let start = self.pos;
        while matches!(self.peek(), Some(c) if c.is_ascii_digit()) {
            self.pos += 1;
        }
        if self.pos == start {
            return Err(());
        }
        let s: String = self.c[start..self.pos].iter().collect();
        s.parse::<i64>().map(IdxBound::Num).map_err(|_| ())
    }

    fn kw(&mut self, w: &str) -> bool {
        let wc: Vec<char> = w.chars().collect();
        if self.c[self.pos..].starts_with(&wc[..]) {
            let after = self.c.get(self.pos + wc.len()).copied();
            if !matches!(after, Some(c) if is_ident_part(c)) {
                self.pos += wc.len();
                return true;
            }
        }
        false
    }

    fn quoted(&mut self) -> Result<String, ()> {
        self.pos += 1;
        let mut buf = String::new();
        loop {
            match self.peek() {
                None => return Err(()),
                Some('"') => {
                    self.pos += 1;
                    break;
                }
                Some('\\') => {
                    self.pos += 1;
                    match self.peek() {
                        Some('"') => buf.push('"'),
                        Some('\\') => buf.push('\\'),
                        Some('/') => buf.push('/'),
                        Some('n') => buf.push('\n'),
                        Some('t') => buf.push('\t'),
                        Some('r') => buf.push('\r'),
                        Some(c) => buf.push(c),
                        None => return Err(()),
                    }
                    self.pos += 1;
                }
                Some(c) => {
                    buf.push(c);
                    self.pos += 1;
                }
            }
        }
        Ok(buf)
    }

    fn number(&mut self) -> Result<Json, ()> {
        let start = self.pos;
        if self.peek() == Some('-') {
            self.pos += 1;
        }
        let mut saw_digit = false;
        while matches!(self.peek(), Some(c) if c.is_ascii_digit()) {
            self.pos += 1;
            saw_digit = true;
        }
        if self.peek() == Some('.') {
            self.pos += 1;
            while matches!(self.peek(), Some(c) if c.is_ascii_digit()) {
                self.pos += 1;
                saw_digit = true;
            }
        }
        if matches!(self.peek(), Some('e') | Some('E')) {
            self.pos += 1;
            if matches!(self.peek(), Some('+') | Some('-')) {
                self.pos += 1;
            }
            while matches!(self.peek(), Some(c) if c.is_ascii_digit()) {
                self.pos += 1;
            }
        }
        if !saw_digit {
            return Err(());
        }
        let tok: String = self.c[start..self.pos].iter().collect();
        Ok(Json::Num(tok))
    }

    fn keyword(&mut self) -> Result<Json, ()> {
        for (word, val) in [
            ("true", Json::Bool(true)),
            ("false", Json::Bool(false)),
            ("null", Json::Null),
        ] {
            let w: Vec<char> = word.chars().collect();
            if self.c[self.pos..].starts_with(&w[..]) {
                self.pos += w.len();
                return Ok(val);
            }
        }
        Err(())
    }
}

fn method_from(name: &str) -> Result<Method, ()> {
    match name {
        "size" => Ok(Method::Size),
        "type" => Ok(Method::Type),
        "double" => Ok(Method::Double),
        "floor" => Ok(Method::Floor),
        "ceiling" => Ok(Method::Ceiling),
        "abs" => Ok(Method::Abs),
        _ => Err(()),
    }
}

fn is_ident_start(c: char) -> bool {
    c.is_alphabetic() || c == '_'
}
fn is_ident_part(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '$'
}

#[derive(Clone)]
enum Json {
    Null,
    Bool(bool),
    Num(String),
    Str(String),
    Arr(Vec<Json>),
    Obj(Vec<(String, Json)>),
}

fn parse_json(text: &str) -> Option<Json> {
    let mut p = JParser {
        b: text.as_bytes(),
        pos: 0,
    };
    p.skip_ws();
    let v = p.value()?;
    p.skip_ws();
    if p.pos != p.b.len() {
        return None;
    }
    Some(v)
}

struct JParser<'a> {
    b: &'a [u8],
    pos: usize,
}

impl<'a> JParser<'a> {
    fn peek(&self) -> Option<u8> {
        self.b.get(self.pos).copied()
    }
    fn bump(&mut self) -> Option<u8> {
        let c = self.peek();
        if c.is_some() {
            self.pos += 1;
        }
        c
    }
    fn skip_ws(&mut self) {
        while matches!(
            self.peek(),
            Some(b' ') | Some(b'\t') | Some(b'\n') | Some(b'\r')
        ) {
            self.pos += 1;
        }
    }
    fn value(&mut self) -> Option<Json> {
        match self.peek()? {
            b'{' => self.object(),
            b'[' => self.array(),
            b'"' => Some(Json::Str(self.string()?)),
            b't' => {
                self.literal(b"true")?;
                Some(Json::Bool(true))
            }
            b'f' => {
                self.literal(b"false")?;
                Some(Json::Bool(false))
            }
            b'n' => {
                self.literal(b"null")?;
                Some(Json::Null)
            }
            b'-' | b'0'..=b'9' => self.number(),
            _ => None,
        }
    }
    fn literal(&mut self, word: &[u8]) -> Option<()> {
        if self.b[self.pos..].starts_with(word) {
            self.pos += word.len();
            Some(())
        } else {
            None
        }
    }
    fn object(&mut self) -> Option<Json> {
        self.bump();
        let mut members: Vec<(String, Json)> = Vec::new();
        self.skip_ws();
        if self.peek() == Some(b'}') {
            self.bump();
            return Some(Json::Obj(members));
        }
        loop {
            self.skip_ws();
            if self.peek() != Some(b'"') {
                return None;
            }
            let key = self.string()?;
            self.skip_ws();
            if self.bump() != Some(b':') {
                return None;
            }
            self.skip_ws();
            let val = self.value()?;
            members.push((key, val));
            self.skip_ws();
            match self.bump() {
                Some(b',') => continue,
                Some(b'}') => break,
                _ => return None,
            }
        }
        Some(Json::Obj(members))
    }
    fn array(&mut self) -> Option<Json> {
        self.bump();
        let mut items: Vec<Json> = Vec::new();
        self.skip_ws();
        if self.peek() == Some(b']') {
            self.bump();
            return Some(Json::Arr(items));
        }
        loop {
            self.skip_ws();
            items.push(self.value()?);
            self.skip_ws();
            match self.bump() {
                Some(b',') => continue,
                Some(b']') => break,
                _ => return None,
            }
        }
        Some(Json::Arr(items))
    }
    fn string(&mut self) -> Option<String> {
        self.bump();
        let mut buf: Vec<u8> = Vec::new();
        loop {
            match self.bump()? {
                b'"' => break,
                b'\\' => match self.bump()? {
                    b'"' => buf.push(b'"'),
                    b'\\' => buf.push(b'\\'),
                    b'/' => buf.push(b'/'),
                    b'b' => buf.push(0x08),
                    b'f' => buf.push(0x0c),
                    b'n' => buf.push(b'\n'),
                    b'r' => buf.push(b'\r'),
                    b't' => buf.push(b'\t'),
                    b'u' => {
                        let cp = self.hex4()?;
                        let scalar = if (0xD800..=0xDBFF).contains(&cp) {
                            if self.bump() != Some(b'\\') || self.bump() != Some(b'u') {
                                return None;
                            }
                            let lo = self.hex4()?;
                            if !(0xDC00..=0xDFFF).contains(&lo) {
                                return None;
                            }
                            0x10000 + ((cp - 0xD800) << 10) + (lo - 0xDC00)
                        } else if (0xDC00..=0xDFFF).contains(&cp) {
                            return None;
                        } else {
                            cp
                        };
                        let ch = char::from_u32(scalar)?;
                        let mut tmp = [0u8; 4];
                        buf.extend_from_slice(ch.encode_utf8(&mut tmp).as_bytes());
                    }
                    _ => return None,
                },
                c if c < 0x20 => return None,
                c => buf.push(c),
            }
        }
        String::from_utf8(buf).ok()
    }
    fn hex4(&mut self) -> Option<u32> {
        let mut v: u32 = 0;
        for _ in 0..4 {
            let h = self.bump()?;
            let d = match h {
                b'0'..=b'9' => (h - b'0') as u32,
                b'a'..=b'f' => (h - b'a' + 10) as u32,
                b'A'..=b'F' => (h - b'A' + 10) as u32,
                _ => return None,
            };
            v = v * 16 + d;
        }
        Some(v)
    }
    fn number(&mut self) -> Option<Json> {
        let start = self.pos;
        if self.peek() == Some(b'-') {
            self.bump();
        }
        match self.peek()? {
            b'0' => {
                self.bump();
            }
            b'1'..=b'9' => {
                self.bump();
                self.skip_digits();
            }
            _ => return None,
        }
        if self.peek() == Some(b'.') {
            self.bump();
            if !self.at_digit() {
                return None;
            }
            self.skip_digits();
        }
        if matches!(self.peek(), Some(b'e') | Some(b'E')) {
            self.bump();
            if matches!(self.peek(), Some(b'+') | Some(b'-')) {
                self.bump();
            }
            if !self.at_digit() {
                return None;
            }
            self.skip_digits();
        }
        let tok = core::str::from_utf8(&self.b[start..self.pos]).ok()?;
        Some(Json::Num(tok.to_string()))
    }
    fn at_digit(&self) -> bool {
        matches!(self.peek(), Some(b'0'..=b'9'))
    }
    fn skip_digits(&mut self) {
        while self.at_digit() {
            self.pos += 1;
        }
    }
}

fn serialize(v: &Json, out: &mut String) {
    match v {
        Json::Null => out.push_str("null"),
        Json::Bool(true) => out.push_str("true"),
        Json::Bool(false) => out.push_str("false"),
        Json::Num(n) => out.push_str(n),
        Json::Str(s) => serialize_string(s, out),
        Json::Arr(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                serialize(item, out);
            }
            out.push(']');
        }
        Json::Obj(members) => {
            out.push('{');
            for (i, (k, val)) in members.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                serialize_string(k, out);
                out.push_str(": ");
                serialize(val, out);
            }
            out.push('}');
        }
    }
}

fn serialize_string(s: &str, out: &mut String) {
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

#[cfg(test)]
mod tests {
    use super::*;

    fn q(target: &str, path: &str) -> Option<String> {
        match path_query_first(target, path).expect("valid path") {
            Some(j) => {
                let mut s = String::new();
                serialize(&j, &mut s);
                Some(s)
            }
            None => None,
        }
    }

    #[test]
    fn member_and_nested_access() {
        assert_eq!(q(r#"{"a": 5}"#, "$.a").as_deref(), Some("5"));
        assert_eq!(q(r#"{"a": {"b": 7}}"#, "$.a.b").as_deref(), Some("7"));
        assert_eq!(q(r#"{"a": 5}"#, "$.z"), None);
    }

    #[test]
    fn array_index_and_wildcards() {
        assert_eq!(q(r#"{"a": [10, 20, 30]}"#, "$.a[0]").as_deref(), Some("10"));
        assert_eq!(q(r#"{"a": [10, 20, 30]}"#, "$.a[1]").as_deref(), Some("20"));

        assert_eq!(q(r#"{"a": [10, 20]}"#, "$.a[*]").as_deref(), Some("10"));
        assert_eq!(q(r#"{"a": 1, "b": 2}"#, "$.*").as_deref(), Some("1"));
    }

    #[test]
    fn exists_true_false_and_missing() {
        assert!(path_exists(r#"{"a": 1}"#, "$.a").unwrap());
        assert!(!path_exists(r#"{"a": 1}"#, "$.b").unwrap());
        assert_eq!(q(r#"{"a": 1}"#, "$.b"), None);
    }

    #[test]
    fn filter_comparison() {
        let doc = r#"{"items": [{"price": 5}, {"price": 15}, {"price": 25}]}"#;
        assert_eq!(
            q(doc, "$.items[*] ? (@.price > 10)").as_deref(),
            Some(r#"{"price": 15}"#)
        );
        assert!(path_exists(doc, "$.items[*] ? (@.price > 10)").unwrap());
        assert!(!path_exists(doc, "$.items[*] ? (@.price > 100)").unwrap());
    }

    #[test]
    fn filter_logical() {
        let doc = r#"{"items": [{"p": 5, "q": 1}, {"p": 15, "q": 9}]}"#;
        assert_eq!(
            q(doc, "$.items[*] ? (@.p > 10 && @.q > 5)").as_deref(),
            Some(r#"{"p": 15, "q": 9}"#)
        );
    }

    #[test]
    fn predicate_match() {
        assert_eq!(path_match(r#"{"a": 1}"#, "$.a == 1").unwrap(), Some(true));
        assert_eq!(path_match(r#"{"a": 1}"#, "$.a == 2").unwrap(), Some(false));

        assert_eq!(path_match(r#"{"a": 1}"#, "$.a").unwrap(), None);
    }

    #[test]
    fn string_comparison_and_literals() {
        assert_eq!(
            path_match(r#"{"n": "abc"}"#, r#"$.n == "abc""#).unwrap(),
            Some(true)
        );
        assert_eq!(
            path_match(r#"{"b": true}"#, "$.b == true").unwrap(),
            Some(true)
        );
    }

    #[test]
    fn malformed_paths_rejected() {
        for bad in [
            "",
            "a",
            "$.",
            "$[",
            "$.a[",
            "$ ? (@.a",
            "$..a",
            "$.a[-1]",
            "$.a.bogus()",
            "$.a[1 to]",
            "$.a[last",
            "$.**{2}",
        ] {
            assert!(
                path_exists(r#"{"a": 1}"#, bad).is_err(),
                "should reject {bad:?}"
            );
        }
    }

    fn qa(target: &str, path: &str) -> Vec<String> {
        query_all(target, path).expect("valid path")
    }

    #[test]
    fn recursive_descent() {
        let doc = r#"{"price": 10, "sub": {"price": 20}}"#;

        assert_eq!(
            qa(doc, "$.**"),
            vec![
                r#"{"price": 10, "sub": {"price": 20}}"#.to_string(),
                "10".to_string(),
                r#"{"price": 20}"#.to_string(),
                "20".to_string(),
            ]
        );

        assert_eq!(
            qa(doc, "$.**.price"),
            vec!["10".to_string(), "20".to_string()]
        );
    }

    #[test]
    fn item_methods() {
        assert_eq!(q(r#"{"a": [1, 2, 3]}"#, "$.a.size()").as_deref(), Some("3"));
        assert_eq!(q(r#"{"a": 5}"#, "$.a.size()").as_deref(), Some("1"));
        assert_eq!(
            q(r#"{"a": [1]}"#, "$.a.type()").as_deref(),
            Some(r#""array""#)
        );
        assert_eq!(
            q(r#"{"a": 5}"#, "$.a.type()").as_deref(),
            Some(r#""number""#)
        );
        assert_eq!(
            q(r#"{"a": "s"}"#, "$.a.type()").as_deref(),
            Some(r#""string""#)
        );
        assert_eq!(
            q(r#"{"a": null}"#, "$.a.type()").as_deref(),
            Some(r#""null""#)
        );
        assert_eq!(q(r#"{"x": 1.7}"#, "$.x.floor()").as_deref(), Some("1"));
        assert_eq!(q(r#"{"x": 1.2}"#, "$.x.ceiling()").as_deref(), Some("2"));
        assert_eq!(q(r#"{"x": -5}"#, "$.x.abs()").as_deref(), Some("5"));
        assert_eq!(q(r#"{"x": "3.5"}"#, "$.x.double()").as_deref(), Some("3.5"));
    }

    #[test]
    fn last_and_ranges() {
        let doc = r#"{"a": [10, 20, 30]}"#;
        assert_eq!(q(doc, "$.a[last]").as_deref(), Some("30"));
        assert_eq!(
            qa(doc, "$.a[1 to 2]"),
            vec!["20".to_string(), "30".to_string()]
        );
        assert_eq!(qa(doc, "$.a[0 to last]").len(), 3);
    }

    #[test]
    fn like_regex_filter() {
        let doc = r#"{"names": ["alice", "bob", "carol"]}"#;
        assert_eq!(
            qa(doc, r#"$.names[*] ? (@ like_regex "^a")"#),
            vec![r#""alice""#.to_string()]
        );
        assert_eq!(
            qa(doc, r#"$.names[*] ? (@ like_regex "^B" flag "i")"#),
            vec![r#""bob""#.to_string()]
        );
    }

    #[test]
    fn call_dispatch_and_null() {

        let r = call(
            "jsonb_path_exists",
            &[SqlValue::Null, SqlValue::Text("$".into())],
        );
        assert_eq!(r.unwrap().unwrap(), SqlValue::Null);

        assert!(call("lpad", &[]).is_none());

        let r = call(
            "jsonb_path_exists",
            &[
                SqlValue::Text(r#"{"a":1}"#.into()),
                SqlValue::Text("$.a".into()),
            ],
        );
        assert_eq!(r.unwrap().unwrap(), SqlValue::Int(1));
    }
}
