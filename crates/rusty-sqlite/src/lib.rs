
use std::collections::BTreeMap;

pub mod crizzle;
mod fileformat;
mod functions;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Int(i64),
    Real(f64),
    Text(String),
    Blob(Vec<u8>),
}

impl Value {
    fn type_rank(&self) -> u8 {
        match self {
            Value::Null => 0,
            Value::Int(_) | Value::Real(_) => 1,
            Value::Text(_) => 2,
            Value::Blob(_) => 3,
        }
    }
    fn as_f64(&self) -> Option<f64> {
        match self {
            Value::Int(i) => Some(*i as f64),
            Value::Real(r) => Some(*r),
            Value::Text(s) => s.trim().parse::<f64>().ok(),
            _ => None,
        }
    }

    fn truthy(&self) -> bool {
        match self {
            Value::Null => false,
            Value::Int(i) => *i != 0,
            Value::Real(r) => *r != 0.0,
            Value::Text(s) => s.trim().parse::<f64>().map(|f| f != 0.0).unwrap_or(false),
            Value::Blob(b) => !b.is_empty(),
        }
    }
    fn compare(&self, other: &Value) -> std::cmp::Ordering {
        use std::cmp::Ordering;
        let (ra, rb) = (self.type_rank(), other.type_rank());
        if ra != rb {
            return ra.cmp(&rb);
        }
        match (self, other) {
            (Value::Null, Value::Null) => Ordering::Equal,
            (Value::Text(a), Value::Text(b)) => a.cmp(b),
            (Value::Blob(a), Value::Blob(b)) => a.cmp(b),
            _ => {
                let (a, b) = (self.as_f64().unwrap_or(0.0), other.as_f64().unwrap_or(0.0));
                a.partial_cmp(&b).unwrap_or(Ordering::Equal)
            }
        }
    }
}

#[derive(PartialEq, Eq, Hash)]
enum GKey {
    Null,
    Num(u64),
    Text(String),
    Blob(Vec<u8>),
}

fn gkey(v: &Value) -> Option<GKey> {
    match v {
        Value::Null => Some(GKey::Null),
        Value::Text(s) => Some(GKey::Text(s.clone())),
        Value::Blob(b) => Some(GKey::Blob(b.clone())),
        Value::Int(_) | Value::Real(_) => {
            let f = v.as_f64().unwrap_or(0.0);
            if f.is_nan() {
                return None;
            }

            let bits = (if f == 0.0 { 0.0 } else { f }).to_bits();
            Some(GKey::Num(bits))
        }
    }
}

#[derive(Default)]
struct DistinctSet {
    keyed: std::collections::HashSet<GKey>,
    nan: Vec<Value>,
}
impl DistinctSet {

    fn insert(&mut self, v: &Value) -> bool {
        match gkey(v) {
            Some(k) => self.keyed.insert(k),
            None => {
                if self
                    .nan
                    .iter()
                    .any(|s| s.compare(v) == std::cmp::Ordering::Equal)
                {
                    false
                } else {
                    self.nan.push(v.clone());
                    true
                }
            }
        }
    }
}

pub type SqlResult<T> = Result<T, String>;

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Ident(String),
    Keyword(String),
    Int(i64),
    Real(f64),
    Str(String),
    Blob(Vec<u8>),
    ParamPos(Option<usize>),
    ParamName(String),
    Punct(char),
    Op(String),
}

fn is_kw(s: &str) -> bool {
    matches!(
        s,
        "CREATE"
            | "TABLE"
            | "IF"
            | "NOT"
            | "EXISTS"
            | "DROP"
            | "INSERT"
            | "INTO"
            | "VALUES"
            | "SELECT"
            | "FROM"
            | "WHERE"
            | "ORDER"
            | "BY"
            | "ASC"
            | "DESC"
            | "LIMIT"
            | "OFFSET"
            | "UPDATE"
            | "SET"
            | "DELETE"
            | "AND"
            | "OR"
            | "IS"
            | "NULL"
            | "PRIMARY"
            | "AUTOINCREMENT"
            | "DEFAULT"
            | "UNIQUE"
            | "AS"
            | "DISTINCT"
            | "LIKE"
            | "IN"
            | "BEGIN"
            | "COMMIT"
            | "BETWEEN"
            | "GLOB"
            | "CASE"
            | "WHEN"
            | "THEN"
            | "ELSE"
            | "END"
            | "ROLLBACK"
            | "TRANSACTION"
            | "INTEGER"
            | "INT"
            | "REAL"
            | "TEXT"
            | "BLOB"
            | "NUMERIC"
            | "FLOAT"
            | "DOUBLE"
            | "BOOLEAN"
            | "TRUE"
            | "FALSE"
            | "ALTER"
            | "PRAGMA"
            | "WITH"
            | "RECURSIVE"
            | "WINDOW"
            | "SAVEPOINT"
            | "RELEASE"
            | "ATTACH"
            | "DETACH"
    )
}

fn is_nonreserved_type_name(word: &str) -> bool {
    matches!(
        word,
        "INTEGER" | "INT" | "REAL" | "TEXT" | "BLOB" | "NUMERIC" | "FLOAT" | "DOUBLE"
    )
}

type KeywordSpellings = std::collections::HashMap<usize, String>;

fn tokenize(src: &str) -> SqlResult<(Vec<Tok>, Vec<(usize, usize)>, KeywordSpellings)> {
    let b = src.as_bytes();
    let mut i = 0;
    let mut out = Vec::new();

    let mut spans: Vec<(usize, usize)> = Vec::new();
    let mut tok_start = 0usize;

    macro_rules! push {
        ($t:expr) => {{
            out.push($t);
            spans.push((tok_start, i));
        }};
    }
    let mut spellings: KeywordSpellings = std::collections::HashMap::new();
    while i < b.len() {
        let c = b[i] as char;
        if c.is_whitespace() {
            i += 1;
            continue;
        }

        if c == '-' && i + 1 < b.len() && b[i + 1] == b'-' {
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        if c == '/' && i + 1 < b.len() && b[i + 1] == b'*' {
            i += 2;
            while i + 1 < b.len() && !(b[i] == b'*' && b[i + 1] == b'/') {
                i += 1;
            }
            i += 2;
            continue;
        }
        tok_start = i;

        if c.is_alphabetic() || c == '_' {
            let start = i;
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                i += 1;
            }
            let word = &src[start..i];
            let up = word.to_ascii_uppercase();

            if (up == "X") && i < b.len() && b[i] == b'\'' {
                i += 1;
                let hs = i;
                while i < b.len() && b[i] != b'\'' {
                    i += 1;
                }
                let hex = &src[hs..i];
                i += 1;
                let mut bytes = Vec::new();
                let hb = hex.as_bytes();
                let mut k = 0;
                while k + 1 < hb.len() {
                    let hi = (hb[k] as char).to_digit(16);
                    let lo = (hb[k + 1] as char).to_digit(16);
                    match (hi, lo) {
                        (Some(h), Some(l)) => bytes.push((h * 16 + l) as u8),
                        _ => return Err("bad hex in blob literal".into()),
                    }
                    k += 2;
                }
                push!(Tok::Blob(bytes));
                continue;
            }
            if is_kw(&up) {
                spellings.insert(out.len(), word.to_string());
                push!(Tok::Keyword(up));
            } else {
                push!(Tok::Ident(word.to_string()));
            }
            continue;
        }

        if c == '"' || c == '`' || c == '[' {
            let close = if c == '[' { ']' } else { c };
            i += 1;
            let start = i;
            while i < b.len() && b[i] as char != close {
                i += 1;
            }
            let ident = src[start..i].to_string();
            i += 1;
            push!(Tok::Ident(ident));
            continue;
        }

        if c == '\'' {
            i += 1;
            let mut s = String::new();
            loop {
                if i >= b.len() {
                    return Err("unterminated string".into());
                }
                if b[i] == b'\'' {
                    if i + 1 < b.len() && b[i + 1] == b'\'' {
                        s.push('\'');
                        i += 2;
                        continue;
                    }
                    i += 1;
                    break;
                }

                let ch = src[i..].chars().next().unwrap_or('\u{FFFD}');
                s.push(ch);
                i += ch.len_utf8();
            }
            push!(Tok::Str(s));
            continue;
        }

        if c.is_ascii_digit()
            || (c == '.' && i + 1 < b.len() && (b[i + 1] as char).is_ascii_digit())
        {
            let start = i;
            let mut is_real = false;
            while i < b.len()
                && ((b[i] as char).is_ascii_digit()
                    || b[i] == b'.'
                    || b[i] == b'e'
                    || b[i] == b'E'
                    || ((b[i] == b'+' || b[i] == b'-')
                        && i > start
                        && (b[i - 1] == b'e' || b[i - 1] == b'E')))
            {
                if b[i] == b'.' || b[i] == b'e' || b[i] == b'E' {
                    is_real = true;
                }
                i += 1;
            }
            let num = &src[start..i];
            if is_real {
                push!(Tok::Real(num.parse().map_err(|_| "bad number")?));
            } else {
                match num.parse::<i64>() {
                    Ok(n) => push!(Tok::Int(n)),
                    Err(_) => push!(Tok::Real(num.parse().map_err(|_| "bad number")?)),
                }
            }
            continue;
        }

        if c == '?' {
            i += 1;
            let start = i;
            while i < b.len() && (b[i] as char).is_ascii_digit() {
                i += 1;
            }
            if i > start {
                push!(Tok::ParamPos(Some(
                    src[start..i].parse().map_err(|_| "bad param")?
                )));
            } else {
                push!(Tok::ParamPos(None));
            }
            continue;
        }
        if c == ':' || c == '$' || c == '@' {
            i += 1;
            let start = i;
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                i += 1;
            }
            push!(Tok::ParamName(src[start..i].to_string()));
            continue;
        }

        let three = if i + 2 < b.len() { &src[i..i + 3] } else { "" };
        if three == "->>" {
            i += 3;
            push!(Tok::Op("->>".to_string()));
            continue;
        }
        let two = if i + 1 < b.len() { &src[i..i + 2] } else { "" };
        match two {
            "==" | "!=" | "<>" | "<=" | ">=" | "||" | "->" => {
                let op = two.to_string();
                i += 2;
                push!(Tok::Op(op));
                continue;
            }
            _ => {}
        }
        match c {
            '=' | '<' | '>' => {
                i += 1;
                push!(Tok::Op(c.to_string()));
            }
            '(' | ')' | ',' | '.' | '*' | '+' | '-' | '/' | '%' | ';' => {
                i += 1;
                push!(Tok::Punct(c));
            }
            _ => return Err(format!("unexpected char '{c}'")),
        }
    }
    Ok((out, spans, spellings))
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Affinity {
    Integer,
    Real,
    Text,
    Blob,
    Numeric,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Collation {
    Binary,
    NoCase,
    RTrim,
}
impl Collation {
    fn to_sql_core(self) -> sql_core::TextCollation {
        match self {
            Self::Binary => sql_core::TextCollation::Binary,
            Self::NoCase => sql_core::TextCollation::AsciiNoCase,
            Self::RTrim => sql_core::TextCollation::RTrim,
        }
    }
}
fn collation_from_name(s: &str) -> Collation {
    if s.eq_ignore_ascii_case("NOCASE") {
        Collation::NoCase
    } else if s.eq_ignore_ascii_case("RTRIM") {
        Collation::RTrim
    } else {
        Collation::Binary
    }
}

fn compare_coll(a: &Value, b: &Value, coll: Collation) -> std::cmp::Ordering {
    match (a, b, coll) {
        (Value::Text(x), Value::Text(y), Collation::NoCase) => {
            x.to_ascii_lowercase().cmp(&y.to_ascii_lowercase())
        }
        (Value::Text(x), Value::Text(y), Collation::RTrim) => {
            x.trim_end_matches(' ').cmp(y.trim_end_matches(' '))
        }
        _ => a.compare(b),
    }
}

#[derive(Debug, Clone)]
pub struct ColumnDef {
    pub name: String,
    pub affinity: Affinity,

    pub decl_type: Option<String>,
    pub pk: bool,
    pub autoincrement: bool,
    pub not_null: bool,
    pub unique: bool,
    pub default: Option<Expr>,

    pub generated: Option<Expr>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum FkAction {
    NoAction,
    Restrict,
    Cascade,
    SetNull,
    SetDefault,
}

#[derive(Debug, Clone)]
struct ForeignKey {
    col: String,
    parent_table: String,
    parent_col: String,
    on_delete: FkAction,
}

#[derive(Debug, Clone)]
enum ParamRef {
    Pos(Option<usize>),
    Name(String),
}

#[derive(Debug, Clone)]
enum Expr {
    Lit(Value),
    Col(String),
    Param(ParamRef),
    Unary(String, Box<Expr>),
    Binary(String, Box<Expr>, Box<Expr>),
    Func(String, Vec<Expr>),

    Window {
        func: String,
        args: Vec<Expr>,
        partition: Vec<Expr>,
        order: Vec<(Expr, bool)>,
        frame: Option<Frame>,

        filter: Option<Box<Expr>>,

        window_ref: Option<String>,
    },

    Collate(Box<Expr>, String),

    AggFilter {
        func: String,
        args: Vec<Expr>,
        filter: Box<Expr>,
    },

    AggOrder {
        func: String,
        args: Vec<Expr>,
        order: Vec<(Expr, bool)>,
        filter: Option<Box<Expr>>,
    },

    Distinct(Box<Expr>),
    Case {
        operand: Option<Box<Expr>>,
        arms: Vec<(Expr, Expr)>,
        els: Option<Box<Expr>>,
    },
    Star,

    Subquery(Box<Stmt>),

    InSelect(Box<Expr>, Box<Stmt>),

    Exists(Box<Stmt>),
}

#[derive(Debug, Clone)]
struct SelectItem {
    expr: Expr,
    alias: Option<String>,

    source: Option<String>,
}

fn resolve_alias_refs(e: &Expr, items: &[SelectItem], schema: &[String]) -> Expr {
    let is_real_col = |name: &str| {
        schema.iter().any(|s| {
            s.eq_ignore_ascii_case(name)
                || s.rsplit_once('.')
                    .map(|(_, c)| c.eq_ignore_ascii_case(name))
                    .unwrap_or(false)
        })
    };
    match e {
        Expr::Col(name) if !is_real_col(name) => {
            for it in items {
                if let Some(a) = &it.alias {
                    if a.eq_ignore_ascii_case(name) {
                        return it.expr.clone();
                    }
                }
            }
            e.clone()
        }
        Expr::Unary(o, x) => Expr::Unary(o.clone(), Box::new(resolve_alias_refs(x, items, schema))),
        Expr::Binary(o, l, r) => Expr::Binary(
            o.clone(),
            Box::new(resolve_alias_refs(l, items, schema)),
            Box::new(resolve_alias_refs(r, items, schema)),
        ),
        Expr::Collate(x, c) => {
            Expr::Collate(Box::new(resolve_alias_refs(x, items, schema)), c.clone())
        }
        Expr::Distinct(x) => Expr::Distinct(Box::new(resolve_alias_refs(x, items, schema))),
        Expr::Func(n, a) => Expr::Func(
            n.clone(),
            a.iter()
                .map(|x| resolve_alias_refs(x, items, schema))
                .collect(),
        ),
        Expr::Case { operand, arms, els } => Expr::Case {
            operand: operand
                .as_ref()
                .map(|o| Box::new(resolve_alias_refs(o, items, schema))),
            arms: arms
                .iter()
                .map(|(c, r)| {
                    (
                        resolve_alias_refs(c, items, schema),
                        resolve_alias_refs(r, items, schema),
                    )
                })
                .collect(),
            els: els
                .as_ref()
                .map(|x| Box::new(resolve_alias_refs(x, items, schema))),
        },
        other => other.clone(),
    }
}

fn resolve_order_key(key: &Expr, items: &[SelectItem], star_names: &[String]) -> Expr {
    match key {
        Expr::Col(name) => {
            for it in items {
                if let Some(a) = &it.alias {
                    if a.eq_ignore_ascii_case(name) {
                        return it.expr.clone();
                    }
                }
            }
            key.clone()
        }
        Expr::Lit(Value::Int(n)) => {
            let idx = *n;
            if idx >= 1 && (idx as usize) <= items.len() {
                return items[(idx as usize) - 1].expr.clone();
            }

            if idx >= 1 && (idx as usize) <= star_names.len() {
                return Expr::Col(star_names[(idx as usize) - 1].clone());
            }
            key.clone()
        }
        _ => key.clone(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum CompoundOp {
    Union,
    UnionAll,
    Intersect,
    Except,
}

#[derive(Debug, Clone, PartialEq)]
enum FrameBound {
    UnboundedPreceding,
    Preceding(i64),
    CurrentRow,
    Following(i64),
    UnboundedFollowing,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum FrameUnit {
    Rows,
    Range,
    Groups,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
enum FrameExclude {
    #[default]
    NoOthers,
    CurrentRow,
    Group,
    Ties,
}

#[derive(Debug, Clone, PartialEq)]
struct Frame {
    unit: FrameUnit,
    start: FrameBound,
    end: FrameBound,
    exclude: FrameExclude,
}

#[derive(Debug, Clone)]
struct JoinClause {
    kind: sql_core::JoinKind,
    table: String,
    alias: Option<String>,
    on: Option<Expr>,

    using: Vec<String>,

    sub: Option<Box<Stmt>>,
}

#[derive(Debug, Clone)]
struct CteDef {
    name: String,
    columns: Option<Vec<String>>,
    select: Box<Stmt>,
}

#[derive(Debug, Clone)]
enum Stmt {
    CreateTable {
        if_not_exists: bool,
        name: String,
        columns: Vec<ColumnDef>,
        table_uniques: Vec<Vec<usize>>,
        checks: Vec<(Expr, String)>,
        foreign_keys: Vec<ForeignKey>,
    },
    DropTable {
        if_exists: bool,
        name: String,
    },
    CompoundSelect {
        first: Box<Stmt>,
        rest: Vec<(CompoundOp, Stmt)>,
        order_by: Vec<(Expr, bool)>,
        limit: Option<Expr>,
        offset: Option<Expr>,
    },

    With {
        recursive: bool,
        ctes: Vec<CteDef>,
        body: Box<Stmt>,
    },
    Insert {
        table: String,
        columns: Option<Vec<String>>,
        rows: Vec<Vec<Expr>>,
        or_action: InsertOr,
        on_conflict: Option<OnConflict>,
        returning: Option<ReturningClause>,
    },
    Select {
        items: Vec<SelectItem>,
        star: bool,
        from: Option<String>,

        from_sub: Option<Box<Stmt>>,
        from_alias: Option<String>,
        joins: Vec<JoinClause>,
        where_: Option<Expr>,
        group_by: Vec<Expr>,
        having: Option<Expr>,
        distinct: bool,
        order_by: Vec<(Expr, bool)>,
        limit: Option<Expr>,
        offset: Option<Expr>,
    },
    Update {
        table: String,
        sets: Vec<(String, Expr)>,
        where_: Option<Expr>,
        returning: Option<ReturningClause>,
    },
    Delete {
        table: String,
        where_: Option<Expr>,
        returning: Option<ReturningClause>,
    },
    Begin,
    Commit,
    Rollback,

    Values {
        rows: Vec<Vec<Expr>>,
    },

    Savepoint(String),
    Release(String),
    RollbackTo(String),

    TableFunc {
        name: String,
        args: Vec<Expr>,
    },

    Attach {
        path: String,
        schema: String,
    },
    Detach(String),
    AlterAddColumn {
        table: String,
        column: ColumnDef,
    },
    AlterRenameTable {
        table: String,
        new_name: String,
    },
    AlterRenameColumn {
        table: String,
        old: String,
        new: String,
    },
    CreateView {
        if_not_exists: bool,
        name: String,
        select: Box<Stmt>,
    },
    DropView {
        if_exists: bool,
        name: String,
    },
    CreateTrigger {
        name: String,
        timing: String,
        event: String,
        update_cols: Vec<String>,
        table: String,
        when: Option<Expr>,
        body: Vec<Stmt>,
    },
    DropTrigger {
        if_exists: bool,
        name: String,
    },

    CreateIndex {
        if_not_exists: bool,
        name: String,
        table: String,
        unique: bool,
        columns: Vec<String>,
        enforce: bool,
    },
    DropIndex {
        if_exists: bool,
        name: String,
    },
    Pragma {
        name: String,
        arg: Option<String>,
        value: Option<Expr>,
    },
}

#[derive(Debug, Clone, PartialEq)]
enum InsertOr {
    None,
    Ignore,
    Replace,
}

#[derive(Debug, Clone)]
struct OnConflict {
    target: Vec<String>,
    action: ConflictAction,
}

#[derive(Debug, Clone)]
enum ConflictAction {
    Nothing,
    Update {
        sets: Vec<(String, Expr)>,
        where_: Option<Expr>,
    },
}

#[derive(Debug, Clone)]
struct ReturningClause {
    star: bool,
    items: Vec<SelectItem>,
}

#[derive(Debug, Clone)]
struct IndexDef {
    #[allow(dead_code)]
    name: String,
    unique: bool,
    cols: Vec<usize>,
    enforce: bool,
}

struct Parser {
    toks: Vec<Tok>,
    pos: usize,
    anon: usize,
    expr_depth: usize,
    spellings: KeywordSpellings,
    src: String,
    spans: Vec<(usize, usize)>,
    index_hints: Vec<(String, String)>,
}

impl Parser {
    const MAX_EXPR_DEPTH: usize = 256;

    fn source_slice(&self, start: usize, end: usize) -> String {
        if end == 0 || start >= self.spans.len() || end > self.spans.len() {
            return String::new();
        }
        let lo = self.spans[start].0;
        let hi = self.spans[end - 1].1;
        self.src.get(lo..hi).unwrap_or("").trim().to_string()
    }

    fn near_error(&self) -> String {
        let tok = self.source_slice(self.pos, self.pos + 1);
        format!("near \"{tok}\": syntax error")
    }

    fn enter_expr_depth(&mut self) -> SqlResult<()> {
        if self.expr_depth >= Self::MAX_EXPR_DEPTH {
            return Err("SQL expression nesting depth exceeded".into());
        }
        self.expr_depth += 1;
        Ok(())
    }

    fn leave_expr_depth(&mut self) {
        self.expr_depth = self.expr_depth.saturating_sub(1);
    }
}

fn validate_sql_nesting(toks: &[Tok]) -> SqlResult<()> {
    let mut paren_depth = 0usize;
    let mut not_depth = 0usize;
    for tok in toks {
        match tok {
            Tok::Punct('(') => {
                paren_depth += 1;
                not_depth = 0;
                if paren_depth > Parser::MAX_EXPR_DEPTH {
                    return Err("SQL expression nesting depth exceeded".into());
                }
            }
            Tok::Punct(')') => {
                paren_depth = paren_depth.saturating_sub(1);
                not_depth = 0;
            }
            Tok::Keyword(k) if k == "NOT" => {
                not_depth += 1;
                if not_depth > Parser::MAX_EXPR_DEPTH {
                    return Err("SQL expression nesting depth exceeded".into());
                }
            }
            _ => not_depth = 0,
        }
    }
    Ok(())
}

impl Parser {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.pos)
    }
    fn next(&mut self) -> Option<Tok> {
        let t = self.toks.get(self.pos).cloned();
        self.pos += 1;
        t
    }
    fn eat_kw(&mut self, kw: &str) -> bool {
        if matches!(self.peek(), Some(Tok::Keyword(k)) if k == kw) {
            self.pos += 1;
            true
        } else {
            false
        }
    }
    fn expect_kw(&mut self, kw: &str) -> SqlResult<()> {
        if self.eat_kw(kw) {
            Ok(())
        } else {
            Err(format!("expected {kw}, got {:?}", self.peek()))
        }
    }
    fn eat_punct(&mut self, c: char) -> bool {
        if matches!(self.peek(), Some(Tok::Punct(p)) if *p == c) {
            self.pos += 1;
            true
        } else {
            false
        }
    }
    fn expect_punct(&mut self, c: char) -> SqlResult<()> {
        if self.eat_punct(c) {
            Ok(())
        } else {
            Err(format!("expected '{c}', got {:?}", self.peek()))
        }
    }
    fn ident(&mut self) -> SqlResult<String> {
        let idx = self.pos;
        match self.next() {
            Some(Tok::Ident(s)) => Ok(s),

            Some(Tok::Keyword(k)) => Ok(self.spellings.get(&idx).cloned().unwrap_or(k)),

            Some(Tok::Str(s)) => Ok(s),
            other => Err(format!("expected identifier, got {other:?}")),
        }
    }

    fn ident_alias(&mut self) -> SqlResult<String> {
        let idx = self.pos;
        match self.next() {
            Some(Tok::Ident(s)) => Ok(s),
            Some(Tok::Keyword(k)) => Ok(self.spellings.get(&idx).cloned().unwrap_or(k)),
            other => Err(format!("expected identifier, got {other:?}")),
        }
    }

    fn word_is(&self, kw: &str) -> bool {
        match self.peek() {
            Some(Tok::Keyword(k)) => k.eq_ignore_ascii_case(kw),
            Some(Tok::Ident(k)) => k.eq_ignore_ascii_case(kw),
            _ => false,
        }
    }
    fn eat_word(&mut self, kw: &str) -> bool {
        if self.word_is(kw) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn parse_stmt(&mut self) -> SqlResult<Stmt> {
        let kw = match self.peek() {
            Some(Tok::Keyword(k)) => k.clone(),
            other => return Err(format!("expected statement, got {other:?}")),
        };
        match kw.as_str() {
            "CREATE" => self.parse_create(),
            "DROP" => self.parse_drop(),
            "INSERT" => self.parse_insert(),
            "SELECT" => self.parse_select(),
            "WITH" => self.parse_with(),
            "UPDATE" => self.parse_update(),
            "DELETE" => self.parse_delete(),
            "ALTER" => self.parse_alter(),
            "PRAGMA" => self.parse_pragma(),
            "BEGIN" => {
                self.pos += 1;
                self.eat_kw("TRANSACTION");
                Ok(Stmt::Begin)
            }
            "COMMIT" => {
                self.pos += 1;
                Ok(Stmt::Commit)
            }
            "ROLLBACK" => {
                self.pos += 1;
                self.eat_kw("TRANSACTION");

                if self.eat_ident_kw("TO") {
                    self.eat_ident_kw("SAVEPOINT");
                    Ok(Stmt::RollbackTo(self.ident()?))
                } else {
                    Ok(Stmt::Rollback)
                }
            }
            "SAVEPOINT" => {
                self.pos += 1;
                Ok(Stmt::Savepoint(self.ident()?))
            }
            "RELEASE" => {
                self.pos += 1;
                self.eat_ident_kw("SAVEPOINT");
                Ok(Stmt::Release(self.ident()?))
            }

            "VALUES" => self.parse_select(),
            "ATTACH" => {
                self.pos += 1;
                self.eat_ident_kw("DATABASE");
                let path = match self.next() {
                    Some(Tok::Str(s)) => s,
                    other => return Err(format!("expected database path string, got {other:?}")),
                };
                self.expect_kw("AS")?;
                let schema = self.ident()?;
                Ok(Stmt::Attach { path, schema })
            }
            "DETACH" => {
                self.pos += 1;
                self.eat_ident_kw("DATABASE");
                Ok(Stmt::Detach(self.ident()?))
            }
            _ => Err(format!("unsupported statement: {kw}")),
        }
    }

    fn parse_table_name(&mut self) -> SqlResult<String> {
        let first = self.ident()?;
        if self.eat_punct('.') {
            Ok(format!("{first}.{}", self.ident()?))
        } else {
            Ok(first)
        }
    }

    fn parse_derived_body(&mut self) -> SqlResult<Stmt> {
        self.parse_select()
    }

    fn parse_values(&mut self) -> SqlResult<Stmt> {
        self.expect_kw("VALUES")?;
        let mut rows = Vec::new();
        loop {
            self.expect_punct('(')?;
            let mut row = Vec::new();
            loop {
                row.push(self.parse_expr()?);
                if self.eat_punct(',') {
                    continue;
                }
                break;
            }
            self.expect_punct(')')?;
            rows.push(row);
            if self.eat_punct(',') {
                continue;
            }
            break;
        }
        Ok(Stmt::Values { rows })
    }

    fn parse_affinity(&mut self) -> (Affinity, Option<String>) {

        let is_constraint = |s: &str| {
            matches!(
                s.to_ascii_uppercase().as_str(),
                "PRIMARY"
                    | "NOT"
                    | "NULL"
                    | "UNIQUE"
                    | "DEFAULT"
                    | "CHECK"
                    | "REFERENCES"
                    | "GENERATED"
                    | "AS"
                    | "COLLATE"
                    | "CONSTRAINT"
            )
        };
        let mut ty = String::new();
        let mut decl = String::new();
        loop {
            let word = match self.peek() {
                Some(Tok::Keyword(k)) | Some(Tok::Ident(k)) if !is_constraint(k) => k.clone(),
                _ => break,
            };
            ty.push_str(&word);
            ty.push(' ');

            let spelled = self
                .spellings
                .get(&self.pos)
                .cloned()
                .unwrap_or_else(|| word.clone());
            if !decl.is_empty() {
                decl.push(' ');
            }
            decl.push_str(&spelled);
            self.pos += 1;

            if self.eat_punct('(') {
                decl.push('(');
                let mut first = true;
                while !matches!(self.peek(), Some(Tok::Punct(')')) | None) {
                    match self.next() {
                        Some(Tok::Int(n)) => {
                            decl.push_str(&n.to_string());
                            first = false;
                        }
                        Some(Tok::Real(r)) => {
                            decl.push_str(&r.to_string());
                            first = false;
                        }
                        Some(Tok::Punct(',')) => {
                            decl.push(',');
                        }
                        Some(Tok::Ident(s)) => {
                            if !first {
                                decl.push(' ');
                            }
                            decl.push_str(&s);
                            first = false;
                        }
                        _ => {}
                    }
                }
                self.eat_punct(')');
                decl.push(')');
            }
        }

        let u = ty.to_ascii_uppercase();
        let affinity = if u.contains("INT") {
            Affinity::Integer
        } else if u.contains("CHAR") || u.contains("CLOB") || u.contains("TEXT") {
            Affinity::Text
        } else if u.trim().is_empty() || u.contains("BLOB") {
            Affinity::Blob
        } else if u.contains("REAL") || u.contains("FLOA") || u.contains("DOUB") {
            Affinity::Real
        } else {
            Affinity::Numeric
        };
        (affinity, if decl.is_empty() { None } else { Some(decl) })
    }

    fn parse_create(&mut self) -> SqlResult<Stmt> {
        self.expect_kw("CREATE")?;

        self.eat_word("TEMP");
        self.eat_word("TEMPORARY");
        if self.eat_word("VIEW") {
            return self.parse_view();
        }
        if self.eat_word("TRIGGER") {
            return self.parse_trigger();
        }
        if self.eat_kw("UNIQUE") {
            self.expect_word("INDEX")?;
            return self.parse_index(true);
        }
        if self.eat_word("INDEX") {
            return self.parse_index(false);
        }
        self.expect_kw("TABLE")?;
        let if_not_exists = self.eat_kw("IF") && {
            self.expect_kw("NOT")?;
            self.expect_kw("EXISTS")?;
            true
        };
        let name = self.parse_table_name()?;
        self.expect_punct('(')?;
        let mut columns = Vec::new();
        let mut table_uniques: Vec<Vec<usize>> = Vec::new();
        let mut checks: Vec<(Expr, String)> = Vec::new();
        let mut foreign_keys: Vec<ForeignKey> = Vec::new();
        loop {
            let cname = self.ident()?;
            let (affinity, decl_type) = self.parse_affinity();
            let mut col = ColumnDef {
                name: cname,
                affinity,
                decl_type,
                pk: false,
                autoincrement: false,
                not_null: false,
                unique: false,
                default: None,
                generated: None,
            };

            loop {
                if self.eat_kw("PRIMARY") {
                    if !self.eat_word("KEY") {
                        return Err("expected KEY".into());
                    }
                    col.pk = true;
                    if self.eat_kw("AUTOINCREMENT") {
                        col.autoincrement = true;
                    }
                } else if self.eat_kw("NOT") {
                    self.expect_kw("NULL")?;
                    col.not_null = true;
                } else if self.eat_kw("UNIQUE") {
                    col.unique = true;

                } else if self.eat_kw("DEFAULT") {
                    col.default = Some(self.parse_expr()?);
                } else if self.eat_ident_kw("CHECK") {
                    self.expect_punct('(')?;
                    let e = self.parse_expr()?;
                    self.expect_punct(')')?;
                    let src = expr_to_sql(&e);
                    checks.push((e, src));
                } else if self.eat_ident_kw("REFERENCES") {

                    let parent_table = self.ident()?;
                    let parent_col = if matches!(self.peek(), Some(Tok::Punct('('))) {
                        self.parse_paren_cols()?
                            .into_iter()
                            .next()
                            .unwrap_or_default()
                    } else {
                        String::new()
                    };
                    let on_delete = self.parse_fk_actions();
                    foreign_keys.push(ForeignKey {
                        col: col.name.clone(),
                        parent_table,
                        parent_col,
                        on_delete,
                    });
                } else if self.eat_ident_kw("GENERATED") {

                    self.eat_ident_kw("ALWAYS");
                    self.expect_kw("AS")?;
                    self.expect_punct('(')?;
                    col.generated = Some(self.parse_expr()?);
                    self.expect_punct(')')?;
                    self.eat_ident_kw("STORED");
                    self.eat_ident_kw("VIRTUAL");
                } else if self.eat_kw("AS") {

                    self.expect_punct('(')?;
                    col.generated = Some(self.parse_expr()?);
                    self.expect_punct(')')?;
                    self.eat_ident_kw("STORED");
                    self.eat_ident_kw("VIRTUAL");
                } else {
                    break;
                }
            }
            columns.push(col);
            if self.eat_punct(',') {
                let is_table_con = matches!(self.peek(), Some(Tok::Keyword(k)) if k == "PRIMARY" || k == "UNIQUE")
                    || matches!(self.peek(), Some(Tok::Ident(s)) if matches!(s.to_ascii_uppercase().as_str(), "CHECK" | "CONSTRAINT" | "FOREIGN"));
                if is_table_con {
                    loop {
                        self.parse_table_con(
                            &columns,
                            &mut checks,
                            &mut table_uniques,
                            &mut foreign_keys,
                        )?;
                        if self.eat_punct(',') {
                            continue;
                        }
                        break;
                    }
                    break;
                }
                continue;
            }
            break;
        }
        self.expect_punct(')')?;

        loop {
            if self.eat_kw("WITHOUT") || self.eat_ident_kw("WITHOUT") {
                self.eat_ident_kw("ROWID");
            } else if self.eat_ident_kw("STRICT") {

            } else {
                break;
            }
            if self.eat_punct(',') {
                continue;
            }
            break;
        }
        Ok(Stmt::CreateTable {
            if_not_exists,
            name,
            columns,
            table_uniques,
            checks,
            foreign_keys,
        })
    }

    fn parse_opt_alias(&mut self) -> Option<String> {
        if self.eat_kw("AS") {
            return self.ident().ok();
        }
        if matches!(self.peek(), Some(Tok::Ident(s)) if !matches!(s.to_ascii_uppercase().as_str(), "INNER" | "LEFT" | "RIGHT" | "FULL" | "CROSS" | "JOIN" | "ON" | "USING" | "UNION" | "INTERSECT" | "EXCEPT" | "GROUP" | "HAVING" | "ORDER" | "LIMIT" | "INDEXED" | "NOT"))
        {
            return self.ident().ok();
        }
        None
    }

    fn parse_index_hint(&mut self) -> Option<String> {
        if self.eat_ident_kw("INDEXED") {
            if self.eat_kw("BY") {
                return self.ident().ok();
            }
        } else if self.eat_kw("NOT") || self.eat_ident_kw("NOT") {
            self.eat_ident_kw("INDEXED");
        }
        None
    }
    fn eat_ident_kw(&mut self, kw: &str) -> bool {
        if matches!(self.peek(), Some(Tok::Ident(s)) if s.eq_ignore_ascii_case(kw)) {
            self.pos += 1;
            true
        } else {
            false
        }
    }
    fn parse_paren_cols(&mut self) -> SqlResult<Vec<String>> {
        self.expect_punct('(')?;
        let mut cols = Vec::new();
        loop {
            cols.push(self.ident()?);
            if self.eat_punct(',') {
                continue;
            }
            break;
        }
        self.expect_punct(')')?;
        Ok(cols)
    }

    fn parse_call_arg_list(&mut self) -> SqlResult<Vec<Expr>> {
        self.expect_punct('(')?;
        let mut args = Vec::new();
        if !matches!(self.peek(), Some(Tok::Punct(')'))) {
            loop {
                args.push(self.parse_expr()?);
                if self.eat_punct(',') {
                    continue;
                }
                break;
            }
        }
        self.expect_punct(')')?;
        Ok(args)
    }

    fn parse_fk_actions(&mut self) -> FkAction {
        let mut on_delete = FkAction::NoAction;
        while self.eat_ident_kw("ON") {
            let is_delete = self.eat_kw("DELETE");
            if !is_delete {
                self.eat_kw("UPDATE");
            }
            let action = if self.eat_ident_kw("CASCADE") {
                FkAction::Cascade
            } else if self.eat_ident_kw("RESTRICT") {
                FkAction::Restrict
            } else if self.eat_kw("SET") {
                if self.eat_kw("NULL") {
                    FkAction::SetNull
                } else {
                    self.eat_kw("DEFAULT");
                    FkAction::SetDefault
                }
            } else if self.eat_ident_kw("NO") {
                self.eat_ident_kw("ACTION");
                FkAction::NoAction
            } else {
                FkAction::NoAction
            };
            if is_delete {
                on_delete = action;
            }
        }
        on_delete
    }
    fn parse_table_con(
        &mut self,
        columns: &[ColumnDef],
        checks: &mut Vec<(Expr, String)>,
        table_uniques: &mut Vec<Vec<usize>>,
        foreign_keys: &mut Vec<ForeignKey>,
    ) -> SqlResult<()> {
        if self.eat_ident_kw("CONSTRAINT") {
            let _ = self.ident();
        }
        if self.eat_kw("PRIMARY") {
            if !self.eat_word("KEY") {
                return Err("expected KEY".into());
            }
            let cols = self.parse_paren_cols()?;
            table_uniques.push(col_indices(columns, &cols)?);
        } else if self.eat_kw("UNIQUE") {
            let cols = self.parse_paren_cols()?;
            table_uniques.push(col_indices(columns, &cols)?);
        } else if self.eat_ident_kw("CHECK") {
            self.expect_punct('(')?;
            let e = self.parse_expr()?;
            self.expect_punct(')')?;
            let src = expr_to_sql(&e);
            checks.push((e, src));
        } else if self.eat_ident_kw("FOREIGN") {

            if !self.eat_word("KEY") {
                return Err("expected KEY".into());
            }
            let child_cols = self.parse_paren_cols()?;
            self.eat_ident_kw("REFERENCES");
            let parent_table = self.ident()?;
            let parent_cols = if matches!(self.peek(), Some(Tok::Punct('('))) {
                self.parse_paren_cols()?
            } else {
                Vec::new()
            };
            let on_delete = self.parse_fk_actions();
            for (i, c) in child_cols.iter().enumerate() {
                foreign_keys.push(ForeignKey {
                    col: c.clone(),
                    parent_table: parent_table.clone(),
                    parent_col: parent_cols.get(i).cloned().unwrap_or_default(),
                    on_delete,
                });
            }
        } else {
            while !matches!(
                self.peek(),
                Some(Tok::Punct(',')) | Some(Tok::Punct(')')) | None
            ) {
                self.pos += 1;
            }
        }
        Ok(())
    }

    fn parse_drop(&mut self) -> SqlResult<Stmt> {
        self.expect_kw("DROP")?;
        let is_view = self.eat_word("VIEW");
        let is_trigger = !is_view && self.eat_word("TRIGGER");
        let is_index = !is_view && !is_trigger && self.eat_word("INDEX");
        if !is_view && !is_trigger && !is_index {
            self.expect_kw("TABLE")?;
        }
        let if_exists = self.eat_kw("IF") && {
            self.expect_kw("EXISTS")?;
            true
        };
        let name = self.parse_table_name()?;
        if is_view {
            Ok(Stmt::DropView { if_exists, name })
        } else if is_trigger {
            Ok(Stmt::DropTrigger { if_exists, name })
        } else if is_index {
            Ok(Stmt::DropIndex { if_exists, name })
        } else {
            Ok(Stmt::DropTable { if_exists, name })
        }
    }

    fn parse_index(&mut self, unique: bool) -> SqlResult<Stmt> {
        let if_not_exists = self.eat_kw("IF") && {
            self.expect_kw("NOT")?;
            self.expect_kw("EXISTS")?;
            true
        };
        let name = self.ident()?;
        self.expect_word("ON")?;
        let table = self.ident()?;
        self.expect_punct('(')?;
        let mut columns: Vec<String> = Vec::new();
        let mut enforce = true;
        loop {
            let e = self.parse_expr()?;
            match e {
                Expr::Col(c) => columns.push(c),

                _ => enforce = false,
            }

            self.eat_kw("ASC");
            self.eat_kw("DESC");
            if self.eat_word("COLLATE") {
                let _ = self.ident();
            }
            if self.eat_punct(',') {
                continue;
            }
            break;
        }
        self.expect_punct(')')?;

        if self.eat_kw("WHERE") {
            let _ = self.parse_expr()?;
            enforce = false;
        }
        Ok(Stmt::CreateIndex {
            if_not_exists,
            name,
            table,
            unique,
            columns,
            enforce,
        })
    }

    fn parse_view(&mut self) -> SqlResult<Stmt> {
        let if_not_exists = self.eat_kw("IF") && {
            self.expect_kw("NOT")?;
            self.expect_kw("EXISTS")?;
            true
        };
        let name = self.ident()?;

        if self.eat_punct('(') {
            while !self.eat_punct(')') && self.peek().is_some() {
                self.pos += 1;
            }
        }
        self.expect_kw("AS")?;
        let select = self.parse_select()?;
        Ok(Stmt::CreateView {
            if_not_exists,
            name,
            select: Box::new(select),
        })
    }

    fn parse_trigger(&mut self) -> SqlResult<Stmt> {
        let if_not_exists = self.eat_kw("IF") && {
            self.expect_kw("NOT")?;
            self.expect_kw("EXISTS")?;
            true
        };
        let _ = if_not_exists;
        let name = self.ident()?;

        let timing = if self.eat_word("BEFORE") {
            "BEFORE".to_string()
        } else if self.eat_word("INSTEAD") {
            self.eat_word("OF");
            "INSTEAD OF".to_string()
        } else {
            self.eat_word("AFTER");
            "AFTER".to_string()
        };

        let mut update_cols = Vec::new();
        let event = if self.eat_kw("INSERT") {
            "INSERT".to_string()
        } else if self.eat_kw("UPDATE") {

            if self.eat_word("OF") {
                loop {
                    update_cols.push(self.ident()?);
                    if self.eat_punct(',') {
                        continue;
                    }
                    break;
                }
            }
            "UPDATE".to_string()
        } else if self.eat_kw("DELETE") {
            "DELETE".to_string()
        } else {
            return Err("expected trigger event".into());
        };
        self.expect_word("ON")?;
        let table = self.ident()?;

        if self.eat_word("FOR") {
            self.eat_word("EACH");
            self.eat_word("ROW");
        }

        let when = if self.eat_word("WHEN") {
            Some(self.parse_expr()?)
        } else {
            None
        };
        self.expect_kw("BEGIN")?;
        let mut body = Vec::new();
        loop {
            while self.eat_punct(';') {}
            if self.word_is("END") || self.peek().is_none() {
                break;
            }
            body.push(self.parse_stmt()?);
            while self.eat_punct(';') {}
        }
        self.eat_word("END");
        Ok(Stmt::CreateTrigger {
            name,
            timing,
            event,
            update_cols,
            table,
            when,
            body,
        })
    }

    fn expect_word(&mut self, kw: &str) -> SqlResult<()> {
        if self.eat_word(kw) {
            Ok(())
        } else {
            Err(format!("expected {kw}, got {:?}", self.peek()))
        }
    }

    fn parse_alter(&mut self) -> SqlResult<Stmt> {
        self.expect_kw("ALTER")?;
        self.expect_word("TABLE")?;
        let table = self.ident()?;
        if self.eat_word("RENAME") {
            if self.eat_word("TO") {
                let new_name = self.ident()?;
                return Ok(Stmt::AlterRenameTable { table, new_name });
            }

            self.eat_word("COLUMN");
            let old = self.ident()?;
            self.expect_word("TO")?;
            let new = self.ident()?;
            return Ok(Stmt::AlterRenameColumn { table, old, new });
        }
        if self.eat_word("ADD") {
            self.eat_word("COLUMN");
            let cname = self.ident()?;
            let (affinity, decl_type) = self.parse_affinity();
            let mut col = ColumnDef {
                name: cname,
                affinity,
                decl_type,
                pk: false,
                autoincrement: false,
                not_null: false,
                unique: false,
                default: None,
                generated: None,
            };
            loop {
                if self.eat_kw("PRIMARY") {
                    if !self.eat_word("KEY") {
                        return Err("expected KEY".into());
                    }
                    col.pk = true;
                } else if self.eat_kw("NOT") {
                    self.expect_kw("NULL")?;
                    col.not_null = true;
                } else if self.eat_kw("UNIQUE") {
                    col.unique = true;
                } else if self.eat_kw("DEFAULT") {
                    col.default = Some(self.parse_expr()?);
                } else {
                    break;
                }
            }
            return Ok(Stmt::AlterAddColumn { table, column: col });
        }
        Err("unsupported ALTER TABLE".into())
    }

    fn parse_pragma(&mut self) -> SqlResult<Stmt> {
        self.expect_kw("PRAGMA")?;
        let name = self.ident()?;
        let mut arg = None;
        let mut value = None;
        if self.eat_punct('(') {
            arg = Some(self.ident()?);
            self.expect_punct(')')?;
        } else if let Some(Tok::Op(o)) = self.peek() {
            if o == "=" {
                self.pos += 1;
                value = Some(self.parse_expr()?);
            }
        }
        Ok(Stmt::Pragma { name, arg, value })
    }

    fn parse_insert(&mut self) -> SqlResult<Stmt> {
        self.expect_kw("INSERT")?;

        let mut or_action = InsertOr::None;
        if self.eat_kw("OR") {
            if self.eat_ident_kw("IGNORE") {
                or_action = InsertOr::Ignore;
            } else if self.eat_ident_kw("REPLACE") {
                or_action = InsertOr::Replace;
            } else if self.eat_ident_kw("ROLLBACK")
                || self.eat_ident_kw("ABORT")
                || self.eat_ident_kw("FAIL")
            {

            } else {
                return Err("expected conflict action after INSERT OR".into());
            }
        }
        self.expect_kw("INTO")?;
        let table = self.parse_table_name()?;
        let mut columns = None;
        if self.eat_punct('(') {
            let mut cols = Vec::new();
            loop {
                cols.push(self.ident()?);
                if self.eat_punct(',') {
                    continue;
                }
                break;
            }
            self.expect_punct(')')?;
            columns = Some(cols);
        }
        self.expect_kw("VALUES")?;
        let mut rows = Vec::new();
        loop {
            self.expect_punct('(')?;
            let mut vals = Vec::new();
            loop {
                vals.push(self.parse_expr()?);
                if self.eat_punct(',') {
                    continue;
                }
                break;
            }
            self.expect_punct(')')?;
            rows.push(vals);
            if self.eat_punct(',') {
                continue;
            }
            break;
        }

        let mut on_conflict = None;
        if self.eat_ident_kw("ON") {
            self.expect_word("CONFLICT")?;
            let mut target = Vec::new();
            if matches!(self.peek(), Some(Tok::Punct('('))) {
                target = self.parse_paren_cols()?;
            }
            self.expect_word("DO")?;
            let action = if self.eat_ident_kw("NOTHING") {
                ConflictAction::Nothing
            } else if self.eat_kw("UPDATE") || self.eat_ident_kw("UPDATE") {
                self.expect_kw("SET")?;
                let mut sets = Vec::new();
                loop {
                    let col = self.ident()?;
                    match self.next() {
                        Some(Tok::Op(o)) if o == "=" => {}
                        other => return Err(format!("expected = in SET, got {other:?}")),
                    }
                    let e = self.parse_expr()?;
                    sets.push((col, e));
                    if self.eat_punct(',') {
                        continue;
                    }
                    break;
                }
                let where_ = if self.eat_kw("WHERE") {
                    Some(self.parse_expr()?)
                } else {
                    None
                };
                ConflictAction::Update { sets, where_ }
            } else {
                return Err("expected NOTHING or UPDATE after ON CONFLICT ... DO".into());
            };
            on_conflict = Some(OnConflict { target, action });
        }

        let returning = if self.eat_ident_kw("RETURNING") {
            Some(self.parse_returning()?)
        } else {
            None
        };
        Ok(Stmt::Insert {
            table,
            columns,
            rows,
            or_action,
            on_conflict,
            returning,
        })
    }

    fn parse_returning(&mut self) -> SqlResult<ReturningClause> {
        let mut star = false;
        let mut items = Vec::new();
        loop {
            if self.eat_punct('*') {
                star = true;
            } else {
                let start_tok = self.pos;
                let expr = self.parse_expr()?;
                let source = self.source_slice(start_tok, self.pos);
                let alias = if self.eat_kw("AS") {
                    Some(self.ident_alias()?)
                } else if matches!(self.peek(), Some(Tok::Ident(_))) {
                    Some(self.ident_alias()?)
                } else {
                    None
                };
                items.push(SelectItem {
                    expr,
                    alias,
                    source: Some(source),
                });
            }
            if self.eat_punct(',') {
                continue;
            }
            break;
        }
        Ok(ReturningClause { star, items })
    }

    fn parse_select_core(&mut self) -> SqlResult<Stmt> {

        if matches!(self.peek(), Some(Tok::Keyword(k)) if k == "VALUES") {
            return self.parse_values();
        }
        self.expect_kw("SELECT")?;
        let distinct = self.eat_kw("DISTINCT");
        let mut items = Vec::new();
        let mut star = false;
        loop {
            if self.eat_punct('*') {
                star = true;
            } else {
                let start_tok = self.pos;
                let expr = self.parse_expr()?;
                let source = self.source_slice(start_tok, self.pos);
                let alias = if self.eat_kw("AS") {
                    Some(self.ident_alias()?)
                } else if matches!(self.peek(), Some(Tok::Ident(s)) if !matches!(s.to_ascii_uppercase().as_str(), "UNION" | "INTERSECT" | "EXCEPT" | "ALL"))
                {
                    Some(self.ident_alias()?)
                } else {
                    None
                };
                items.push(SelectItem {
                    expr,
                    alias,
                    source: Some(source),
                });
            }
            if self.eat_punct(',') {
                continue;
            }
            break;
        }
        let (from, from_sub, from_alias, joins) = if self.eat_kw("FROM") {

            let (t, from_sub) = if matches!(self.peek(), Some(Tok::Punct('(')))
                && matches!(self.peek2(), Some(Tok::Keyword(k)) if k == "SELECT" || k == "VALUES")
            {
                self.expect_punct('(')?;
                let sub = self.parse_derived_body()?;
                self.expect_punct(')')?;
                (String::new(), Some(Box::new(sub)))
            } else if matches!(self.peek(), Some(Tok::Ident(_)))
                && matches!(self.peek2(), Some(Tok::Punct('(')))
            {

                let name = self.ident()?.to_ascii_uppercase();
                let args = self.parse_call_arg_list()?;
                (
                    String::new(),
                    Some(Box::new(Stmt::TableFunc { name, args })),
                )
            } else {
                (self.parse_table_name()?, None)
            };
            let a = self.parse_opt_alias();
            if let Some(idx) = self.parse_index_hint() {
                self.index_hints.push((t.clone(), idx));
            }
            let mut joins = Vec::new();
            loop {
                let kind = if self.eat_ident_kw("CROSS") {
                    self.eat_ident_kw("JOIN");
                    Some(sql_core::JoinKind::Cross)
                } else if self.eat_ident_kw("INNER") {
                    self.eat_ident_kw("JOIN");
                    Some(sql_core::JoinKind::Inner)
                } else if self.eat_ident_kw("LEFT") {
                    self.eat_ident_kw("OUTER");
                    self.eat_ident_kw("JOIN");
                    Some(sql_core::JoinKind::Left)
                } else if self.eat_ident_kw("JOIN") {
                    Some(sql_core::JoinKind::Inner)
                } else if self.eat_punct(',') {
                    Some(sql_core::JoinKind::Cross)
                } else {
                    None
                };
                match kind {
                    Some(k) => {

                        let (jt, jsub) = if matches!(self.peek(), Some(Tok::Punct('(')))
                            && matches!(self.peek2(), Some(Tok::Keyword(kw)) if kw == "SELECT" || kw == "VALUES")
                        {
                            self.expect_punct('(')?;
                            let sub = self.parse_derived_body()?;
                            self.expect_punct(')')?;
                            (String::new(), Some(Box::new(sub)))
                        } else if matches!(self.peek(), Some(Tok::Ident(_)))
                            && matches!(self.peek2(), Some(Tok::Punct('(')))
                        {

                            let name = self.ident()?.to_ascii_uppercase();
                            let args = self.parse_call_arg_list()?;
                            (
                                String::new(),
                                Some(Box::new(Stmt::TableFunc { name, args })),
                            )
                        } else {
                            (self.parse_table_name()?, None)
                        };
                        let ja = self.parse_opt_alias();
                        if let Some(idx) = self.parse_index_hint() {
                            self.index_hints.push((jt.clone(), idx));
                        }
                        let (on, using) = if self.eat_ident_kw("ON") {
                            (Some(self.parse_expr()?), Vec::new())
                        } else if self.eat_ident_kw("USING") {
                            self.expect_punct('(')?;
                            let mut cols = Vec::new();
                            loop {
                                cols.push(self.ident()?);
                                if !self.eat_punct(',') {
                                    break;
                                }
                            }
                            self.expect_punct(')')?;
                            (None, cols)
                        } else {
                            (None, Vec::new())
                        };
                        joins.push(JoinClause {
                            kind: k,
                            table: jt,
                            alias: ja,
                            on,
                            using,
                            sub: jsub,
                        });
                    }
                    None => break,
                }
            }
            (Some(t), from_sub, a, joins)
        } else {
            (None, None, None, Vec::new())
        };
        let where_ = if self.eat_kw("WHERE") {
            Some(self.parse_expr()?)
        } else {
            None
        };
        let mut group_by = Vec::new();
        let mut having = None;
        if self.eat_word("GROUP") {
            self.expect_word("BY")?;
            loop {
                group_by.push(self.parse_expr()?);
                if self.eat_punct(',') {
                    continue;
                }
                break;
            }

            if self.eat_word("HAVING") {
                having = Some(self.parse_expr()?);
            }
        }

        if self.eat_word("WINDOW") {
            let mut defs: Vec<(String, Expr)> = Vec::new();
            loop {
                let wname = self.ident()?;
                self.expect_kw("AS")?;

                let spec = self.parse_window(String::new(), Vec::new(), None)?;
                defs.push((wname, spec));
                if self.eat_punct(',') {
                    continue;
                }
                break;
            }
            for it in &mut items {
                it.expr = resolve_window_refs(&it.expr, &defs);
            }
        }
        Ok(Stmt::Select {
            items,
            star,
            from,
            from_sub,
            from_alias,
            joins,
            where_,
            group_by,
            having,
            distinct,
            order_by: Vec::new(),
            limit: None,
            offset: None,
        })
    }

    fn parse_order_limit(&mut self) -> SqlResult<(Vec<(Expr, bool)>, Option<Expr>, Option<Expr>)> {
        let mut order_by = Vec::new();
        if self.eat_kw("ORDER") {
            self.expect_kw("BY")?;
            loop {
                let key = self.parse_expr()?;
                let desc = if self.eat_kw("DESC") {
                    true
                } else {
                    self.eat_kw("ASC");
                    false
                };
                order_by.push((key, desc));
                if self.eat_punct(',') {
                    continue;
                }
                break;
            }
        }
        let mut limit = None;
        let mut offset = None;
        if self.eat_kw("LIMIT") {
            limit = Some(self.parse_expr()?);
            if self.eat_kw("OFFSET") {
                offset = Some(self.parse_expr()?);
            } else if self.eat_punct(',') {
                offset = limit.take();
                limit = Some(self.parse_expr()?);
            }
        }
        Ok((order_by, limit, offset))
    }

    fn eat_compound_op(&mut self) -> Option<CompoundOp> {
        let word = match self.peek() {
            Some(Tok::Ident(s)) => s.to_ascii_uppercase(),
            _ => return None,
        };
        match word.as_str() {
            "UNION" => {
                self.pos += 1;
                if matches!(self.peek(), Some(Tok::Ident(s)) if s.eq_ignore_ascii_case("ALL")) {
                    self.pos += 1;
                    Some(CompoundOp::UnionAll)
                } else {
                    Some(CompoundOp::Union)
                }
            }
            "INTERSECT" => {
                self.pos += 1;
                Some(CompoundOp::Intersect)
            }
            "EXCEPT" => {
                self.pos += 1;
                Some(CompoundOp::Except)
            }
            _ => None,
        }
    }

    fn parse_window(
        &mut self,
        func: String,
        args: Vec<Expr>,
        filter: Option<Box<Expr>>,
    ) -> SqlResult<Expr> {

        if !matches!(self.peek(), Some(Tok::Punct('('))) {
            let name = self.ident()?;
            return Ok(Expr::Window {
                func,
                args,
                partition: Vec::new(),
                order: Vec::new(),
                frame: None,
                filter,
                window_ref: Some(name),
            });
        }
        self.expect_punct('(')?;
        let mut partition = Vec::new();
        if matches!(self.peek(), Some(Tok::Ident(s)) if s.eq_ignore_ascii_case("PARTITION")) {
            self.pos += 1;
            self.expect_kw("BY")?;
            loop {
                partition.push(self.parse_expr()?);
                if self.eat_punct(',') {
                    continue;
                }
                break;
            }
        }
        let mut order = Vec::new();
        if self.eat_kw("ORDER") {
            self.expect_kw("BY")?;
            loop {
                let k = self.parse_expr()?;
                let desc = if self.eat_kw("DESC") {
                    true
                } else {
                    self.eat_kw("ASC");
                    false
                };
                order.push((k, desc));
                if self.eat_punct(',') {
                    continue;
                }
                break;
            }
        }

        let frame = self.parse_frame()?;
        if !self.eat_punct(')') {
            return Err("expected ) to close OVER window spec".into());
        }
        Ok(Expr::Window {
            func,
            args,
            partition,
            order,
            frame,
            filter,
            window_ref: None,
        })
    }

    fn ident_is(&self, word: &str) -> bool {
        matches!(self.peek(), Some(Tok::Ident(s)) if s.eq_ignore_ascii_case(word))
    }

    fn parse_frame(&mut self) -> SqlResult<Option<Frame>> {
        let unit = if self.ident_is("ROWS") {
            self.pos += 1;
            FrameUnit::Rows
        } else if self.ident_is("RANGE") {
            self.pos += 1;
            FrameUnit::Range
        } else if self.ident_is("GROUPS") {
            self.pos += 1;
            FrameUnit::Groups
        } else {
            return Ok(None);
        };
        if self.eat_kw("BETWEEN") {
            let start = self.parse_frame_bound()?;
            self.expect_kw("AND")?;
            let end = self.parse_frame_bound()?;
            let exclude = self.parse_frame_exclude()?;
            Ok(Some(Frame {
                unit,
                start,
                end,
                exclude,
            }))
        } else {
            let start = self.parse_frame_bound()?;
            let exclude = self.parse_frame_exclude()?;
            Ok(Some(Frame {
                unit,
                start,
                end: FrameBound::CurrentRow,
                exclude,
            }))
        }
    }

    fn parse_frame_exclude(&mut self) -> SqlResult<FrameExclude> {
        if !self.ident_is("EXCLUDE") {
            return Ok(FrameExclude::NoOthers);
        }
        self.pos += 1;
        if self.ident_is("NO") {
            self.pos += 1;
            if !self.ident_is("OTHERS") {
                return Err("expected OTHERS after EXCLUDE NO".into());
            }
            self.pos += 1;
            Ok(FrameExclude::NoOthers)
        } else if self.ident_is("CURRENT") {
            self.pos += 1;
            if !self.ident_is("ROW") {
                return Err("expected ROW after EXCLUDE CURRENT".into());
            }
            self.pos += 1;
            Ok(FrameExclude::CurrentRow)
        } else if self.eat_kw("GROUP") {

            Ok(FrameExclude::Group)
        } else if self.ident_is("GROUP") {
            self.pos += 1;
            Ok(FrameExclude::Group)
        } else if self.ident_is("TIES") {
            self.pos += 1;
            Ok(FrameExclude::Ties)
        } else {
            Err("expected NO OTHERS | CURRENT ROW | GROUP | TIES after EXCLUDE".into())
        }
    }

    fn parse_frame_bound(&mut self) -> SqlResult<FrameBound> {
        if self.ident_is("UNBOUNDED") {
            self.pos += 1;
            if self.ident_is("PRECEDING") {
                self.pos += 1;
                return Ok(FrameBound::UnboundedPreceding);
            }
            if self.ident_is("FOLLOWING") {
                self.pos += 1;
                return Ok(FrameBound::UnboundedFollowing);
            }
            return Err("expected PRECEDING/FOLLOWING after UNBOUNDED".into());
        }
        if self.ident_is("CURRENT") {
            self.pos += 1;
            if self.ident_is("ROW") {
                self.pos += 1;
                return Ok(FrameBound::CurrentRow);
            }
            return Err("expected ROW after CURRENT".into());
        }

        let n = match self.next() {
            Some(Tok::Int(i)) => i,
            other => return Err(format!("expected frame bound, got {other:?}")),
        };
        if self.ident_is("PRECEDING") {
            self.pos += 1;
            Ok(FrameBound::Preceding(n))
        } else if self.ident_is("FOLLOWING") {
            self.pos += 1;
            Ok(FrameBound::Following(n))
        } else {
            Err("expected PRECEDING/FOLLOWING after frame offset".into())
        }
    }

    fn parse_with(&mut self) -> SqlResult<Stmt> {
        self.expect_kw("WITH")?;
        let recursive = self.eat_kw("RECURSIVE");
        let mut ctes = Vec::new();
        loop {
            let name = self.ident()?;
            let columns = if self.eat_punct('(') {
                let mut cols = Vec::new();
                loop {
                    cols.push(self.ident()?);
                    if self.eat_punct(',') {
                        continue;
                    }
                    break;
                }
                if !self.eat_punct(')') {
                    return Err("expected ) after CTE column list".into());
                }
                Some(cols)
            } else {
                None
            };
            self.expect_kw("AS")?;

            let _ = self.eat_kw("NOT") || self.eat_ident_kw("NOT");
            self.eat_ident_kw("MATERIALIZED");
            if !self.eat_punct('(') {
                return Err("expected ( before CTE body".into());
            }
            let select = self.parse_select()?;
            if !self.eat_punct(')') {
                return Err("expected ) after CTE body".into());
            }
            ctes.push(CteDef {
                name,
                columns,
                select: Box::new(select),
            });
            if self.eat_punct(',') {
                continue;
            }
            break;
        }
        let body = self.parse_stmt()?;
        Ok(Stmt::With {
            recursive,
            ctes,
            body: Box::new(body),
        })
    }

    fn parse_select(&mut self) -> SqlResult<Stmt> {
        let first = self.parse_select_core()?;
        let mut rest = Vec::new();
        while let Some(op) = self.eat_compound_op() {
            rest.push((op, self.parse_select_core()?));
        }
        self.assemble_select(first, rest)
    }

    fn assemble_select(&mut self, first: Stmt, rest: Vec<(CompoundOp, Stmt)>) -> SqlResult<Stmt> {
        let last = rest.last().map(|(_, s)| s).unwrap_or(&first);
        if matches!(last, Stmt::Values { .. }) {
            if let Some(Tok::Keyword(k)) = self.peek() {
                if k == "ORDER" || k == "LIMIT" {
                    return Err(format!("near \"{k}\": syntax error"));
                }
            }
        }
        let (order_by, limit, offset) = self.parse_order_limit()?;
        if rest.is_empty() {
            match first {
                Stmt::Select {
                    items,
                    star,
                    from,
                    from_sub,
                    from_alias,
                    joins,
                    where_,
                    group_by,
                    having,
                    distinct,
                    ..
                } => Ok(Stmt::Select {
                    items,
                    star,
                    from,
                    from_sub,
                    from_alias,
                    joins,
                    where_,
                    group_by,
                    having,
                    distinct,
                    order_by,
                    limit,
                    offset,
                }),

                other => Ok(other),
            }
        } else {
            Ok(Stmt::CompoundSelect {
                first: Box::new(first),
                rest,
                order_by,
                limit,
                offset,
            })
        }
    }

    fn parse_update(&mut self) -> SqlResult<Stmt> {
        self.expect_kw("UPDATE")?;
        let table = self.parse_table_name()?;
        self.expect_kw("SET")?;
        let mut sets = Vec::new();
        loop {
            let col = self.ident()?;
            match self.next() {
                Some(Tok::Op(o)) if o == "=" => {}
                other => return Err(format!("expected = in SET, got {other:?}")),
            }
            let val = self.parse_expr()?;
            sets.push((col, val));
            if self.eat_punct(',') {
                continue;
            }
            break;
        }
        let where_ = if self.eat_kw("WHERE") {
            Some(self.parse_expr()?)
        } else {
            None
        };
        let returning = if self.eat_ident_kw("RETURNING") {
            Some(self.parse_returning()?)
        } else {
            None
        };
        Ok(Stmt::Update {
            table,
            sets,
            where_,
            returning,
        })
    }

    fn parse_delete(&mut self) -> SqlResult<Stmt> {
        self.expect_kw("DELETE")?;
        self.expect_kw("FROM")?;
        let table = self.parse_table_name()?;
        let where_ = if self.eat_kw("WHERE") {
            Some(self.parse_expr()?)
        } else {
            None
        };
        let returning = if self.eat_ident_kw("RETURNING") {
            Some(self.parse_returning()?)
        } else {
            None
        };
        Ok(Stmt::Delete {
            table,
            where_,
            returning,
        })
    }

    fn parse_expr(&mut self) -> SqlResult<Expr> {
        self.enter_expr_depth()?;
        let result = self.parse_or();
        self.leave_expr_depth();
        result
    }
    fn parse_or(&mut self) -> SqlResult<Expr> {
        let mut left = self.parse_and()?;
        while self.eat_kw("OR") {
            let right = self.parse_and()?;
            left = Expr::Binary("OR".into(), Box::new(left), Box::new(right));
        }
        Ok(left)
    }
    fn parse_and(&mut self) -> SqlResult<Expr> {
        let mut left = self.parse_not()?;
        while self.eat_kw("AND") {
            let right = self.parse_not()?;
            left = Expr::Binary("AND".into(), Box::new(left), Box::new(right));
        }
        Ok(left)
    }
    fn parse_not(&mut self) -> SqlResult<Expr> {
        self.enter_expr_depth()?;
        if self.eat_kw("NOT") {
            let e = self.parse_not();
            self.leave_expr_depth();
            return e.map(|e| Expr::Unary("NOT".into(), Box::new(e)));
        }
        let result = self.parse_cmp();
        self.leave_expr_depth();
        result
    }
    fn peek2(&self) -> Option<&Tok> {
        self.toks.get(self.pos + 1)
    }

    fn parse_case(&mut self) -> SqlResult<Expr> {
        let operand = if matches!(self.peek(), Some(Tok::Keyword(k)) if k == "WHEN") {
            None
        } else {
            Some(Box::new(self.parse_expr()?))
        };
        let mut arms = Vec::new();
        while self.eat_kw("WHEN") {
            let cond = self.parse_expr()?;
            self.expect_kw("THEN")?;
            let res = self.parse_expr()?;
            arms.push((cond, res));
        }
        let els = if self.eat_kw("ELSE") {
            Some(Box::new(self.parse_expr()?))
        } else {
            None
        };
        self.expect_kw("END")?;
        Ok(Expr::Case { operand, arms, els })
    }
    fn parse_cmp(&mut self) -> SqlResult<Expr> {
        let left = self.parse_add()?;

        if self.eat_kw("IS") {
            let neg = self.eat_kw("NOT");
            let op = if neg { "ISNOT" } else { "IS" };

            let right = if self.eat_kw("NULL") {
                Expr::Lit(Value::Null)
            } else {
                self.parse_add()?
            };
            return Ok(Expr::Binary(op.into(), Box::new(left), Box::new(right)));
        }

        let negated = matches!(self.peek(), Some(Tok::Keyword(k)) if k == "NOT")
            && matches!(self.peek2(), Some(Tok::Keyword(k)) if matches!(k.as_str(), "IN" | "BETWEEN" | "LIKE" | "GLOB"));
        if negated {
            self.pos += 1;
        }
        let wrap = |e: Expr| {
            if negated {
                Expr::Unary("NOT".into(), Box::new(e))
            } else {
                e
            }
        };
        if self.eat_kw("IN") {
            self.expect_punct('(')?;
            if matches!(self.peek(), Some(Tok::Keyword(k)) if k == "SELECT") {
                let sub = self.parse_select()?;
                self.expect_punct(')')?;
                return Ok(wrap(Expr::InSelect(Box::new(left), Box::new(sub))));
            }
            let mut items = Vec::new();
            if !matches!(self.peek(), Some(Tok::Punct(')'))) {
                loop {
                    items.push(self.parse_expr()?);
                    if self.eat_punct(',') {
                        continue;
                    }
                    break;
                }
            }
            self.expect_punct(')')?;

            let e = if items.is_empty() {
                Expr::Lit(Value::Int(0))
            } else {
                let mut it = items.into_iter();
                let mut acc = Expr::Binary(
                    "=".into(),
                    Box::new(left.clone()),
                    Box::new(it.next().unwrap()),
                );
                for item in it {
                    acc = Expr::Binary(
                        "OR".into(),
                        Box::new(acc),
                        Box::new(Expr::Binary(
                            "=".into(),
                            Box::new(left.clone()),
                            Box::new(item),
                        )),
                    );
                }
                acc
            };
            return Ok(wrap(e));
        }
        if self.eat_kw("BETWEEN") {
            let lo = self.parse_add()?;
            self.expect_kw("AND")?;
            let hi = self.parse_add()?;
            let e = Expr::Binary(
                "AND".into(),
                Box::new(Expr::Binary(
                    ">=".into(),
                    Box::new(left.clone()),
                    Box::new(lo),
                )),
                Box::new(Expr::Binary(
                    "<=".into(),
                    Box::new(left.clone()),
                    Box::new(hi),
                )),
            );
            return Ok(wrap(e));
        }
        if self.eat_kw("LIKE") {
            let right = self.parse_add()?;

            if self.eat_word("ESCAPE") {
                let esc = self.parse_add()?;
                return Ok(wrap(Expr::Func("LIKE".into(), vec![right, left, esc])));
            }
            return Ok(wrap(Expr::Binary(
                "LIKE".into(),
                Box::new(left),
                Box::new(right),
            )));
        }
        if self.eat_kw("GLOB") {
            let right = self.parse_add()?;
            return Ok(wrap(Expr::Binary(
                "GLOB".into(),
                Box::new(left),
                Box::new(right),
            )));
        }
        if let Some(Tok::Op(o)) = self.peek() {
            let op = o.clone();
            if matches!(
                op.as_str(),
                "=" | "==" | "!=" | "<>" | "<" | "<=" | ">" | ">="
            ) {
                self.pos += 1;
                let right = self.parse_add()?;
                return Ok(Expr::Binary(op, Box::new(left), Box::new(right)));
            }
        }
        Ok(left)
    }
    fn parse_add(&mut self) -> SqlResult<Expr> {
        let mut left = self.parse_mul()?;
        loop {
            let op = match self.peek() {
                Some(Tok::Punct('+')) => "+",
                Some(Tok::Punct('-')) => "-",
                Some(Tok::Op(o)) if o == "||" => "||",

                Some(Tok::Op(o)) if o == "->" => "->",
                Some(Tok::Op(o)) if o == "->>" => "->>",
                _ => break,
            };
            self.pos += 1;
            let right = self.parse_mul()?;
            left = Expr::Binary(op.into(), Box::new(left), Box::new(right));
        }
        Ok(left)
    }
    fn parse_mul(&mut self) -> SqlResult<Expr> {
        let mut left = self.parse_unary()?;
        loop {
            let op = match self.peek() {
                Some(Tok::Punct('*')) => "*",
                Some(Tok::Punct('/')) => "/",
                Some(Tok::Punct('%')) => "%",
                _ => break,
            };
            self.pos += 1;
            let right = self.parse_unary()?;
            left = Expr::Binary(op.into(), Box::new(left), Box::new(right));
        }
        Ok(left)
    }
    fn parse_unary(&mut self) -> SqlResult<Expr> {
        if self.eat_punct('-') {
            let e = self.parse_unary()?;
            return Ok(Expr::Unary("-".into(), Box::new(e)));
        }
        if self.eat_punct('+') {
            return self.parse_unary();
        }

        let mut e = self.parse_primary()?;
        while self.eat_word("COLLATE") {
            let name = self.ident()?;
            e = Expr::Collate(Box::new(e), name);
        }
        Ok(e)
    }
    fn parse_primary(&mut self) -> SqlResult<Expr> {

        let rewrite_spelling = match self.toks.get(self.pos) {
            Some(Tok::Keyword(k)) if is_nonreserved_type_name(k) => Some(
                self.spellings
                    .get(&self.pos)
                    .cloned()
                    .unwrap_or_else(|| k.clone()),
            ),
            _ => None,
        };
        if let Some(spelling) = rewrite_spelling {
            self.toks[self.pos] = Tok::Ident(spelling);
        }
        match self.next() {
            Some(Tok::Int(n)) => Ok(Expr::Lit(Value::Int(n))),
            Some(Tok::Real(r)) => Ok(Expr::Lit(Value::Real(r))),
            Some(Tok::Str(s)) => Ok(Expr::Lit(Value::Text(s))),
            Some(Tok::Blob(b)) => Ok(Expr::Lit(Value::Blob(b))),
            Some(Tok::ParamPos(n)) => {

                let idx = n.unwrap_or_else(|| {
                    self.anon += 1;
                    self.anon
                });
                Ok(Expr::Param(ParamRef::Pos(Some(idx))))
            }
            Some(Tok::ParamName(n)) => Ok(Expr::Param(ParamRef::Name(n))),
            Some(Tok::Keyword(k)) if k == "NULL" => Ok(Expr::Lit(Value::Null)),
            Some(Tok::Keyword(k)) if k == "TRUE" => Ok(Expr::Lit(Value::Int(1))),
            Some(Tok::Keyword(k)) if k == "FALSE" => Ok(Expr::Lit(Value::Int(0))),
            Some(Tok::Keyword(k)) if k == "CASE" => self.parse_case(),

            Some(Tok::Keyword(k)) if k == "EXISTS" => {
                self.expect_punct('(')?;
                let sub = self.parse_select()?;
                self.expect_punct(')')?;
                Ok(Expr::Exists(Box::new(sub)))
            }
            Some(Tok::Punct('(')) => {

                if matches!(self.peek(), Some(Tok::Keyword(k)) if k == "SELECT") {
                    let sub = self.parse_select()?;
                    self.expect_punct(')')?;
                    return Ok(Expr::Subquery(Box::new(sub)));
                }
                let e = self.parse_expr()?;
                self.expect_punct(')')?;
                Ok(e)
            }
            Some(Tok::Punct('*')) => Ok(Expr::Star),

            Some(Tok::Ident(name))
                if name.eq_ignore_ascii_case("CAST")
                    && matches!(self.peek(), Some(Tok::Punct('('))) =>
            {
                self.expect_punct('(')?;
                let inner = self.parse_expr()?;
                self.expect_kw("AS")?;
                let (ty, _) = self.parse_affinity();
                let ty_name = match ty {
                    Affinity::Integer => "INTEGER",
                    Affinity::Real => "REAL",
                    Affinity::Text => "TEXT",
                    Affinity::Blob => "BLOB",
                    Affinity::Numeric => "NUMERIC",
                };
                self.expect_punct(')')?;
                Ok(Expr::Func(
                    "CAST".into(),
                    vec![inner, Expr::Lit(Value::Text(ty_name.into()))],
                ))
            }

            Some(Tok::Ident(name))
                if name.eq_ignore_ascii_case("RAISE")
                    && matches!(self.peek(), Some(Tok::Punct('('))) =>
            {
                self.expect_punct('(')?;
                let action = self.ident()?.to_ascii_uppercase();
                let mut args = vec![Expr::Lit(Value::Text(action.clone()))];
                if self.eat_punct(',') {
                    args.push(self.parse_expr()?);
                }
                self.expect_punct(')')?;
                Ok(Expr::Func("RAISE".into(), args))
            }
            Some(Tok::Ident(name)) => {

                if self.eat_punct('(') {
                    let mut args = Vec::new();

                    let distinct = self.eat_kw("DISTINCT");
                    if self.eat_punct('*') {
                        args.push(Expr::Star);
                    } else if !matches!(self.peek(), Some(Tok::Punct(')'))) {
                        loop {
                            let a = self.parse_expr()?;
                            if distinct && args.is_empty() {
                                args.push(Expr::Distinct(Box::new(a)));
                            } else {
                                args.push(a);
                            }
                            if self.eat_punct(',') {
                                continue;
                            }
                            break;
                        }
                    }

                    let mut agg_order: Vec<(Expr, bool)> = Vec::new();
                    if self.eat_kw("ORDER") {
                        self.expect_kw("BY")?;
                        loop {
                            let key = self.parse_expr()?;
                            let desc = if self.eat_kw("DESC") {
                                true
                            } else {
                                self.eat_kw("ASC");
                                false
                            };
                            agg_order.push((key, desc));
                            if self.eat_punct(',') {
                                continue;
                            }
                            break;
                        }
                    }
                    self.expect_punct(')')?;

                    let filter = if self.ident_is("FILTER") {
                        self.pos += 1;
                        if !self.eat_punct('(') {
                            return Err("expected ( after FILTER".into());
                        }
                        self.expect_kw("WHERE")?;
                        let cond = self.parse_expr()?;
                        if !self.eat_punct(')') {
                            return Err("expected ) after FILTER (WHERE ...)".into());
                        }
                        Some(Box::new(cond))
                    } else {
                        None
                    };

                    if self.ident_is("OVER") {
                        self.pos += 1;
                        return self.parse_window(name.to_ascii_uppercase(), args, filter);
                    }
                    if !agg_order.is_empty() {

                        return Ok(Expr::AggOrder {
                            func: name.to_ascii_uppercase(),
                            args,
                            order: agg_order,
                            filter,
                        });
                    }
                    if let Some(f) = filter {

                        return Ok(Expr::AggFilter {
                            func: name.to_ascii_uppercase(),
                            args,
                            filter: f,
                        });
                    }
                    Ok(Expr::Func(name.to_ascii_uppercase(), args))
                } else if self.eat_punct('.') {

                    let col = self.ident()?;
                    if self.eat_punct('.') {
                        let col3 = self.ident()?;
                        Ok(Expr::Col(format!("{name}.{col}.{col3}")))
                    } else {
                        Ok(Expr::Col(format!("{name}.{col}")))
                    }
                } else {
                    Ok(Expr::Col(name))
                }
            }

            Some(Tok::Keyword(k))
                if matches!(k.as_str(), "LIKE" | "GLOB")
                    && matches!(self.peek(), Some(Tok::Punct('('))) =>
            {
                let name = k.clone();
                let args = self.parse_call_arg_list()?;
                Ok(Expr::Func(name, args))
            }
            other => Err(format!("unexpected token in expression: {other:?}")),
        }
    }
}

pub fn parse_statement(sql: &str) -> SqlResult<Statement> {
    let (stmt, index_hints) = parse_one(sql)?;
    Ok(Statement {
        stmt,
        sql: sql.to_string(),
        index_hints,
    })
}

fn parse_one(sql: &str) -> SqlResult<(Stmt, Vec<(String, String)>)> {
    let (toks, spans, spellings) = tokenize(sql)?;
    validate_sql_nesting(&toks)?;
    let ntok = toks.len();
    let mut p = Parser {
        toks,
        pos: 0,
        anon: 0,
        expr_depth: 0,
        spellings,
        src: sql.to_string(),
        spans,
        index_hints: Vec::new(),
    };
    match p.parse_stmt() {
        Ok(stmt) => {
            p.eat_punct(';');

            if p.pos < ntok {
                return Err(p.near_error());
            }
            Ok((stmt, p.index_hints))
        }

        Err(e) => {
            if p.pos >= ntok {
                Err("incomplete input".into())
            } else if e.starts_with("near \"") || e == "incomplete input" {
                Err(e)
            } else {
                Err(p.near_error())
            }
        }
    }
}

fn split_statements(sql: &str) -> SqlResult<Vec<Stmt>> {
    let (toks, spans, spellings) = tokenize(sql)?;
    validate_sql_nesting(&toks)?;
    let mut p = Parser {
        toks,
        pos: 0,
        anon: 0,
        expr_depth: 0,
        spellings,
        src: sql.to_string(),
        spans,
        index_hints: Vec::new(),
    };
    let mut out = Vec::new();
    while p.peek().is_some() {
        while p.eat_punct(';') {}
        if p.peek().is_none() {
            break;
        }
        p.anon = 0;
        out.push(p.parse_stmt()?);
        while p.eat_punct(';') {}
    }
    Ok(out)
}

#[derive(Debug, Clone)]
struct Table {
    columns: Vec<ColumnDef>,
    rows: Vec<Vec<Value>>,
    row_ids: Vec<i64>,
    next_rowid: i64,

    max_rowid: i64,
    checks: Vec<(Expr, String)>,
    table_uniques: Vec<Vec<usize>>,
    #[allow(dead_code)]
    indexes: Vec<IndexDef>,
    foreign_keys: Vec<ForeignKey>,

    eq_indexes: std::collections::HashMap<usize, sql_core::EqIndex>,
}

pub struct Statement {
    stmt: Stmt,
    pub sql: String,

    index_hints: Vec<(String, String)>,
}

#[derive(Default, Clone)]
pub struct Bindings {
    pub positional: Vec<Value>,
    pub named: BTreeMap<String, Value>,
}

impl Bindings {
    pub fn new() -> Self {
        Self::default()
    }
}

pub enum Outcome {
    Rows {
        columns: Vec<String>,
        rows: Vec<Vec<Value>>,
    },
    Mutation {
        changes: i64,
        last_insert_rowid: i64,
    },
}

pub struct SelectRowCursor<'db> {
    rows: &'db [Vec<Value>],
    columns: Vec<String>,
    source_names: Vec<String>,
    ncols: usize,
    items: Vec<SelectItem>,
    star: bool,
    where_: Option<Expr>,
    offset: usize,
    limit: Option<usize>,
    seen: usize,
    emitted: usize,
    pos: usize,
    binds: Bindings,
    col_aff: std::collections::HashMap<String, Affinity>,
    closed: bool,
}

pub struct SelectCursorSession {
    table: String,
    columns: Vec<String>,
    source_names: Vec<String>,
    ncols: usize,
    items: Vec<SelectItem>,
    star: bool,
    where_: Option<Expr>,
    offset: usize,
    limit: Option<usize>,
    seen: usize,
    emitted: usize,
    pos: usize,
    binds: Bindings,
    col_aff: std::collections::HashMap<String, Affinity>,
    closed: bool,
}

impl<'db> SelectRowCursor<'db> {
    pub fn columns(&self) -> &[String] {
        &self.columns
    }

    pub fn close(&mut self) {
        self.closed = true;
    }

    pub fn next_row(&mut self) -> SqlResult<Option<Vec<Value>>> {
        if self.closed {
            return Ok(None);
        }
        if self.limit.is_some_and(|limit| self.emitted >= limit) {
            self.close();
            return Ok(None);
        }
        while let Some(row) = self.rows.get(self.pos) {
            self.pos += 1;
            let mut ctx = ParamCtx {
                b: &self.binds,
                next_pos: 0,
                db: None,
                scopes: Vec::new(),
                cur_alias: None,
                col_aff: self.col_aff.clone(),
                subq_cache: std::collections::HashMap::new(),
            };
            if let Some(e) = &self.where_ {
                if !eval(e, row, &self.source_names, &mut ctx)?.truthy() {
                    continue;
                }
            }
            if self.seen < self.offset {
                self.seen += 1;
                continue;
            }
            self.seen += 1;
            let projected = if self.star {
                row[..self.ncols].to_vec()
            } else {
                let mut out = Vec::with_capacity(self.items.len());
                for item in &self.items {
                    out.push(eval(&item.expr, row, &self.source_names, &mut ctx)?);
                }
                out
            };
            self.emitted += 1;
            return Ok(Some(projected));
        }
        self.close();
        Ok(None)
    }
}

impl SelectCursorSession {
    pub fn columns(&self) -> &[String] {
        &self.columns
    }

    pub fn close(&mut self) {
        self.closed = true;
    }

    pub fn is_closed(&self) -> bool {
        self.closed
    }
}

#[derive(Debug, Clone)]
struct TriggerDef {
    name: String,
    timing: String,
    event: String,
    update_cols: Vec<String>,
    table: String,
    when: Option<Expr>,
    body: Vec<Stmt>,
}

#[derive(Clone)]
pub struct Database {
    tables: BTreeMap<String, Table>,
    path: Option<String>,
    changes: i64,
    total_changes: i64,
    last_insert_rowid: i64,

    savepoints: Vec<(String, BTreeMap<String, Table>)>,

    attached: BTreeMap<String, BTreeMap<String, Table>>,
    in_txn: bool,
    txn_snapshot: Option<BTreeMap<String, Table>>,

    views: Vec<(String, Stmt)>,
    triggers: Vec<TriggerDef>,
    user_version: i64,
    foreign_keys_on: bool,

    cte_scopes: Vec<Vec<(String, (Vec<ColumnDef>, Vec<Vec<Value>>))>>,
}

impl Database {
    pub fn open_memory() -> Self {
        Database {
            tables: BTreeMap::new(),
            path: None,
            changes: 0,
            total_changes: 0,
            last_insert_rowid: 0,
            savepoints: Vec::new(),
            attached: BTreeMap::new(),
            in_txn: false,
            txn_snapshot: None,
            views: Vec::new(),
            triggers: Vec::new(),
            user_version: 0,
            foreign_keys_on: false,
            cte_scopes: Vec::new(),
        }
    }

    pub fn open_file(path: &str) -> SqlResult<Self> {
        let mut db = Database::open_memory();
        db.path = Some(path.to_string());
        if let Ok(bytes) = std::fs::read(path) {
            if !bytes.is_empty() {
                db.tables = if fileformat::is_sqlite_file(&bytes) {

                    match std::fs::read(format!("{path}-wal")) {
                        Ok(wal) if !wal.is_empty() => {
                            fileformat::read_sqlite_file(&fileformat::apply_wal(&bytes, &wal))?
                        }
                        _ => fileformat::read_sqlite_file(&bytes)?,
                    }
                } else {
                    deserialize(&bytes)?
                };
            }
        }
        Ok(db)
    }

    pub fn serialize_bytes(&self) -> Vec<u8> {
        fileformat::write_sqlite_file(&self.tables).unwrap_or_else(|| serialize(&self.tables))
    }

    pub fn deserialize_bytes(bytes: &[u8]) -> SqlResult<Self> {
        let mut db = Database::open_memory();
        if !bytes.is_empty() {
            db.tables = if fileformat::is_sqlite_file(bytes) {
                fileformat::read_sqlite_file(bytes)?
            } else {
                deserialize(bytes)?
            };
        }
        Ok(db)
    }

    pub fn changes(&self) -> i64 {
        self.changes
    }
    pub fn last_insert_rowid(&self) -> i64 {
        self.last_insert_rowid
    }

    fn tbl(&self, qname: &str) -> Option<&Table> {
        let (schema, bare) = split_schema(qname);
        match schema {
            None | Some("main") | Some("temp") => self.tables.get(bare),
            Some(s) => self.attached.get(s).and_then(|m| m.get(bare)),
        }
    }
    fn tbl_mut(&mut self, qname: &str) -> Option<&mut Table> {
        let (schema, bare) = split_schema(qname);
        match schema {
            None | Some("main") | Some("temp") => self.tables.get_mut(bare),
            Some(s) => self.attached.get_mut(s).and_then(|m| m.get_mut(bare)),
        }
    }
    fn has_tbl(&self, qname: &str) -> bool {
        self.tbl(qname).is_some()
    }
    fn insert_tbl(&mut self, qname: &str, t: Table) {
        let (schema, bare) = split_schema(qname);
        let bare = bare.to_string();
        match schema {
            None | Some("main") | Some("temp") => {
                self.tables.insert(bare, t);
            }
            Some(s) => {
                self.attached
                    .entry(s.to_string())
                    .or_default()
                    .insert(bare, t);
            }
        }
    }
    fn remove_tbl(&mut self, qname: &str) -> Option<Table> {
        let (schema, bare) = split_schema(qname);
        match schema {
            None | Some("main") | Some("temp") => self.tables.remove(bare),
            Some(s) => self.attached.get_mut(s).and_then(|m| m.remove(bare)),
        }
    }
    pub fn is_file_backed(&self) -> bool {
        self.path.is_some()
    }

    pub fn persist(&self) -> SqlResult<()> {

        if let Some(p) = &self.path {
            let bytes = fileformat::write_sqlite_file(&self.tables)
                .unwrap_or_else(|| serialize(&self.tables));
            std::fs::write(p, bytes).map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    pub fn prepare(&self, sql: &str) -> SqlResult<Statement> {
        parse_statement(sql)
    }

    fn subquery_hoistable(&self, sub: &Stmt) -> bool {
        let Stmt::Select {
            from,
            from_sub,
            from_alias,
            joins,
            ..
        } = sub
        else {
            return false;
        };
        if from_sub.is_some() {
            return false;
        }
        let mut own_cols: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut own_aliases: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut add = |name: &str,
                       alias: Option<&str>,
                       own_cols: &mut std::collections::HashSet<String>,
                       own_aliases: &mut std::collections::HashSet<String>|
         -> bool {
            match self.tbl(name) {
                Some(t) => {
                    for c in &t.columns {
                        own_cols.insert(c.name.to_ascii_lowercase());
                    }
                    own_cols.insert("rowid".into());
                    own_aliases.insert(name.to_ascii_lowercase());
                    if let Some(a) = alias {
                        own_aliases.insert(a.to_ascii_lowercase());
                    }
                    true
                }
                None => false,
            }
        };
        if let Some(t) = from {
            if !t.is_empty() && !add(t, from_alias.as_deref(), &mut own_cols, &mut own_aliases) {
                return false;
            }
        }
        for j in joins {
            if j.sub.is_some()
                || !add(
                    &j.table,
                    j.alias.as_deref(),
                    &mut own_cols,
                    &mut own_aliases,
                )
            {
                return false;
            }
        }
        let ok = std::cell::Cell::new(true);
        collect_select_colrefs(
            sub,
            &mut |name: &str| {
                match name.rsplit_once('.') {

                    Some((q, _)) => {
                        if !own_aliases.contains(&q.to_ascii_lowercase()) {
                            ok.set(false);
                        }
                    }

                    None => {
                        if !own_cols.contains(&name.to_ascii_lowercase()) {
                            ok.set(false);
                        }
                    }
                }
            },
            &mut || ok.set(false),
        );
        ok.get()
    }

    pub fn run(&mut self, stmt: &Statement, b: &Bindings) -> SqlResult<Outcome> {

        for (table, index) in &stmt.index_hints {
            let has = self.tbl(table).is_some_and(|t| {
                t.indexes
                    .iter()
                    .any(|ix| ix.name.eq_ignore_ascii_case(index))
            });
            if !has {
                return Err(format!("no such index: {index}"));
            }
        }
        let mut ctx = ParamCtx {
            b,
            next_pos: 0,
            db: Some(self.clone()),
            scopes: Vec::new(),
            cur_alias: None,
            col_aff: std::collections::HashMap::new(),
            subq_cache: std::collections::HashMap::new(),
        };
        self.exec(&stmt.stmt, &mut ctx)
    }

    pub fn insert_values_no_returning(
        &mut self,
        table: &str,
        columns: &[String],
        rows: &[Vec<sql_core::SqlValue>],
    ) -> SqlResult<Outcome> {
        let expr_rows: Vec<Vec<Expr>> = rows
            .iter()
            .map(|row| row.iter().map(|v| Expr::Lit(sql_to_val(v))).collect())
            .collect();
        self.insert_expr_values_no_returning(table, columns, &expr_rows)
    }

    pub fn insert_values_no_returning_owned(
        &mut self,
        table: &str,
        columns: &[String],
        rows: Vec<Vec<sql_core::SqlValue>>,
    ) -> SqlResult<Outcome> {
        let expr_rows: Vec<Vec<Expr>> = rows
            .into_iter()
            .map(|row| {
                row.into_iter()
                    .map(|v| Expr::Lit(sql_to_val_owned(v)))
                    .collect()
            })
            .collect();
        self.insert_expr_values_no_returning(table, columns, &expr_rows)
    }

    fn insert_expr_values_no_returning(
        &mut self,
        table: &str,
        columns: &[String],
        expr_rows: &[Vec<Expr>],
    ) -> SqlResult<Outcome> {
        let b = Bindings::new();
        let mut ctx = ParamCtx {
            b: &b,
            next_pos: 0,
            db: Some(self.clone()),
            scopes: Vec::new(),
            cur_alias: None,
            col_aff: std::collections::HashMap::new(),
            subq_cache: std::collections::HashMap::new(),
        };
        self.exec_insert(
            table,
            &Some(columns.to_vec()),
            expr_rows,
            &InsertOr::None,
            &None,
            &None,
            &mut ctx,
        )
    }

    pub fn select_cursor<'db>(
        &'db self,
        stmt: &Statement,
        b: &Bindings,
    ) -> SqlResult<SelectRowCursor<'db>> {
        let Stmt::Select {
            items,
            star,
            from,
            from_sub,
            from_alias: _,
            joins,
            where_,
            group_by,
            having,
            distinct,
            order_by,
            limit,
            offset,
        } = &stmt.stmt
        else {
            return Err("cursor requires SELECT".into());
        };
        if from_sub.is_some() || !joins.is_empty() {
            return Err("cursor does not yet support joins or derived SELECT sources".into());
        }
        if from.is_none() {
            return Err("cursor requires a base table".into());
        }
        if *distinct
            || !order_by.is_empty()
            || !group_by.is_empty()
            || having.is_some()
            || items
                .iter()
                .any(|it| expr_contains_aggregate(&it.expr) || expr_has_window(&it.expr))
        {
            return Err("cursor does not yet support materializing SELECT clauses".into());
        }
        let refs_subquery_or_rowid = items.iter().any(|it| expr_refs_rowid(&it.expr))
            || where_.as_ref().is_some_and(expr_refs_rowid);
        if refs_subquery_or_rowid {
            return Err("cursor does not yet support rowid or subquery expressions".into());
        }
        let table = from.as_ref().unwrap();
        let tbl = self
            .tables
            .get(table)
            .ok_or_else(|| format!("no such table: {table}"))?;
        let source_names: Vec<String> = tbl.columns.iter().map(|c| c.name.clone()).collect();
        if !*star {
            for item in items {
                validate_cursor_expr(&item.expr, &source_names)?;
            }
        }
        if let Some(w) = where_ {
            validate_cursor_expr(w, &source_names)?;
        }
        let mut ctx = ParamCtx {
            b,
            next_pos: 0,
            db: None,
            scopes: Vec::new(),
            cur_alias: None,
            col_aff: std::collections::HashMap::new(),
            subq_cache: std::collections::HashMap::new(),
        };
        let off = match offset {
            Some(e) => {
                validate_cursor_expr(e, &[])?;
                eval(e, &[], &[], &mut ctx)?.as_f64().unwrap_or(0.0) as usize
            }
            None => 0,
        };
        let lim = match limit {
            Some(e) => {
                validate_cursor_expr(e, &[])?;
                Some(eval(e, &[], &[], &mut ctx)?.as_f64().unwrap_or(0.0) as usize)
            }
            None => None,
        };
        let columns = if *star {
            source_names.clone()
        } else {
            items
                .iter()
                .enumerate()
                .map(|(i, it)| select_item_name(it, i))
                .collect()
        };
        let mut col_aff = std::collections::HashMap::new();
        for cd in &tbl.columns {
            col_aff.insert(cd.name.clone(), cd.affinity);
        }
        Ok(SelectRowCursor {
            rows: &tbl.rows,
            columns,
            source_names,
            ncols: tbl.columns.len(),
            items: items.clone(),
            star: *star,
            where_: where_.clone(),
            offset: off,
            limit: lim,
            seen: 0,
            emitted: 0,
            pos: 0,
            binds: b.clone(),
            col_aff,
            closed: false,
        })
    }

    pub fn select_cursor_session(
        &self,
        stmt: &Statement,
        b: &Bindings,
    ) -> SqlResult<SelectCursorSession> {
        let cur = self.select_cursor(stmt, b)?;
        Ok(SelectCursorSession {
            table: match &stmt.stmt {
                Stmt::Select {
                    from: Some(table), ..
                } => table.clone(),
                _ => return Err("cursor requires SELECT".into()),
            },
            columns: cur.columns.clone(),
            source_names: cur.source_names.clone(),
            ncols: cur.ncols,
            items: cur.items.clone(),
            star: cur.star,
            where_: cur.where_.clone(),
            offset: cur.offset,
            limit: cur.limit,
            seen: cur.seen,
            emitted: cur.emitted,
            pos: cur.pos,
            binds: cur.binds.clone(),
            col_aff: cur.col_aff.clone(),
            closed: cur.closed,
        })
    }

    pub fn step_select_cursor(
        &self,
        cur: &mut SelectCursorSession,
    ) -> SqlResult<Option<Vec<Value>>> {
        if cur.closed {
            return Ok(None);
        }
        if cur.limit.is_some_and(|limit| cur.emitted >= limit) {
            cur.close();
            return Ok(None);
        }
        let tbl = self
            .tables
            .get(&cur.table)
            .ok_or_else(|| format!("no such table: {}", cur.table))?;
        while let Some(row) = tbl.rows.get(cur.pos) {
            cur.pos += 1;
            let mut ctx = ParamCtx {
                b: &cur.binds,
                next_pos: 0,
                db: None,
                scopes: Vec::new(),
                cur_alias: None,
                col_aff: cur.col_aff.clone(),
                subq_cache: std::collections::HashMap::new(),
            };
            if let Some(e) = &cur.where_ {
                if !eval(e, row, &cur.source_names, &mut ctx)?.truthy() {
                    continue;
                }
            }
            if cur.seen < cur.offset {
                cur.seen += 1;
                continue;
            }
            cur.seen += 1;
            let projected = if cur.star {
                row[..cur.ncols].to_vec()
            } else {
                let mut out = Vec::with_capacity(cur.items.len());
                for item in &cur.items {
                    out.push(eval(&item.expr, row, &cur.source_names, &mut ctx)?);
                }
                out
            };
            cur.emitted += 1;
            return Ok(Some(projected));
        }
        cur.close();
        Ok(None)
    }

    pub fn exec_script(&mut self, sql: &str) -> SqlResult<Outcome> {
        let stmts = split_statements(sql)?;
        let mut last = Outcome::Mutation {
            changes: 0,
            last_insert_rowid: self.last_insert_rowid,
        };
        for s in stmts {
            let b = Bindings::new();
            let mut ctx = ParamCtx {
                b: &b,
                next_pos: 0,
                db: Some(self.clone()),
                scopes: Vec::new(),
                cur_alias: None,
                col_aff: std::collections::HashMap::new(),
                subq_cache: std::collections::HashMap::new(),
            };
            last = self.exec(&s, &mut ctx)?;
        }
        Ok(last)
    }

    fn exec(&mut self, stmt: &Stmt, ctx: &mut ParamCtx) -> SqlResult<Outcome> {
        match stmt {
            Stmt::Begin => {
                self.in_txn = true;
                self.txn_snapshot = Some(self.tables.clone());
                Ok(self.mutation(0))
            }
            Stmt::Commit => {
                self.in_txn = false;
                self.txn_snapshot = None;
                self.persist()?;
                Ok(self.mutation(0))
            }
            Stmt::Rollback => {
                if let Some(snap) = self.txn_snapshot.take() {
                    self.tables = snap;
                }
                self.savepoints.clear();
                self.in_txn = false;
                Ok(self.mutation(0))
            }
            Stmt::Savepoint(name) => {
                self.savepoints.push((name.clone(), self.tables.clone()));
                Ok(self.mutation(0))
            }
            Stmt::Release(name) => {

                if let Some(idx) = self
                    .savepoints
                    .iter()
                    .rposition(|(n, _)| n.eq_ignore_ascii_case(name))
                {
                    self.savepoints.truncate(idx);
                    Ok(self.mutation(0))
                } else {
                    Err(format!("no such savepoint: {name}"))
                }
            }
            Stmt::RollbackTo(name) => {

                if let Some(idx) = self
                    .savepoints
                    .iter()
                    .rposition(|(n, _)| n.eq_ignore_ascii_case(name))
                {
                    self.tables = self.savepoints[idx].1.clone();
                    self.savepoints.truncate(idx + 1);
                    Ok(self.mutation(0))
                } else {
                    Err(format!("no such savepoint: {name}"))
                }
            }
            Stmt::Values { rows } => {
                let mut out_rows = Vec::new();
                let mut ncols = 0;
                for r in rows {
                    let mut vr = Vec::new();
                    for e in r {
                        vr.push(eval(e, &[], &[], ctx)?);
                    }
                    ncols = ncols.max(vr.len());
                    out_rows.push(vr);
                }
                let columns = (1..=ncols).map(|i| format!("column{i}")).collect();
                Ok(Outcome::Rows {
                    columns,
                    rows: out_rows,
                })
            }
            Stmt::Attach { path, schema } => {
                if schema.eq_ignore_ascii_case("main") || schema.eq_ignore_ascii_case("temp") {
                    return Err(format!("cannot ATTACH to reserved schema {schema}"));
                }
                if self.attached.contains_key(schema) {
                    return Err(format!("database {schema} is already in use"));
                }

                let tables = if path == ":memory:" || path.is_empty() {
                    BTreeMap::new()
                } else {
                    Database::open_file(path)
                        .map(|d| d.tables)
                        .unwrap_or_default()
                };
                self.attached.insert(schema.clone(), tables);
                Ok(self.mutation(0))
            }
            Stmt::Detach(schema) => {
                if self.attached.remove(schema).is_none() {
                    return Err(format!("no such database: {schema}"));
                }
                Ok(self.mutation(0))
            }
            Stmt::TableFunc { name, args } => {
                let arg0 = match args.first() {
                    Some(e) => eval(e, &[], &[], ctx)?,
                    None => Value::Null,
                };

                if let Some(res) = self.pragma_tvf(name, &text_of(&arg0)) {
                    return res;
                }
                match functions::json::table_valued(name, &arg0) {
                    Some(Ok((columns, rows))) => Ok(Outcome::Rows { columns, rows }),
                    Some(Err(e)) => Err(e),
                    None => Err(format!("no such table-valued function: {name}")),
                }
            }
            Stmt::CreateTable {
                if_not_exists,
                name,
                columns,
                table_uniques,
                checks,
                foreign_keys,
            } => {
                if self.has_tbl(name) {
                    if *if_not_exists {
                        return Ok(self.mutation(0));
                    }
                    return Err(format!("table {name} already exists"));
                }
                self.insert_tbl(
                    name,
                    Table {
                        columns: columns.clone(),
                        rows: Vec::new(),
                        row_ids: Vec::new(),
                        next_rowid: 1,
                        max_rowid: 0,
                        checks: checks.clone(),
                        table_uniques: table_uniques.clone(),
                        indexes: Vec::new(),
                        foreign_keys: foreign_keys.clone(),
                        eq_indexes: std::collections::HashMap::new(),
                    },
                );
                self.maybe_persist()?;
                Ok(self.mutation(0))
            }
            Stmt::DropTable { if_exists, name } => {
                if self.remove_tbl(name).is_none() && !*if_exists {
                    return Err(format!("no such table: {name}"));
                }
                self.maybe_persist()?;
                Ok(self.mutation(0))
            }
            Stmt::Insert {
                table,
                columns,
                rows,
                or_action,
                on_conflict,
                returning,
            } => self.exec_insert(table, columns, rows, or_action, on_conflict, returning, ctx),
            Stmt::Select { .. } => self.exec_select(stmt, ctx),
            Stmt::CompoundSelect {
                first,
                rest,
                order_by,
                limit,
                offset,
            } => self.exec_compound(first, rest, order_by, limit, offset, ctx),
            Stmt::With {
                recursive,
                ctes,
                body,
            } => self.exec_with(*recursive, ctes, body, ctx),
            Stmt::Update {
                table,
                sets,
                where_,
                returning,
            } => self.exec_update(table, sets, where_, returning, ctx),
            Stmt::Delete {
                table,
                where_,
                returning,
            } => self.exec_delete(table, where_, returning, ctx),
            Stmt::AlterAddColumn { table, column } => {
                let tbl = self
                    .tbl_mut(table)
                    .ok_or_else(|| format!("no such table: {table}"))?;
                let default = match &column.default {
                    Some(e) => {
                        let mut c = ParamCtx {
                            b: ctx.b,
                            next_pos: 0,
                            db: None,
                            scopes: Vec::new(),
                            cur_alias: None,
                            col_aff: std::collections::HashMap::new(),
                            subq_cache: std::collections::HashMap::new(),
                        };
                        coerce(
                            eval(e, &[], &[], &mut c).unwrap_or(Value::Null),
                            column.affinity,
                        )
                    }
                    None => Value::Null,
                };
                tbl.columns.push(column.clone());
                for row in tbl.rows.iter_mut() {
                    row.push(default.clone());
                }
                self.maybe_persist()?;
                Ok(self.mutation(0))
            }
            Stmt::AlterRenameTable { table, new_name } => {
                let t = self
                    .tables
                    .remove(table)
                    .ok_or_else(|| format!("no such table: {table}"))?;
                self.tables.insert(new_name.clone(), t);
                for tr in self.triggers.iter_mut() {
                    if tr.table.eq_ignore_ascii_case(table) {
                        tr.table = new_name.clone();
                    }
                }
                self.maybe_persist()?;
                Ok(self.mutation(0))
            }
            Stmt::AlterRenameColumn { table, old, new } => {
                let tbl = self
                    .tbl_mut(table)
                    .ok_or_else(|| format!("no such table: {table}"))?;
                let col = tbl
                    .columns
                    .iter_mut()
                    .find(|c| c.name.eq_ignore_ascii_case(old))
                    .ok_or_else(|| format!("no such column: {old}"))?;
                col.name = new.clone();
                self.maybe_persist()?;
                Ok(self.mutation(0))
            }
            Stmt::CreateView {
                if_not_exists,
                name,
                select,
            } => {
                let exists = self.views.iter().any(|(n, _)| n.eq_ignore_ascii_case(name))
                    || self.tables.contains_key(name);
                if exists {
                    if *if_not_exists {
                        return Ok(self.mutation(0));
                    }
                    return Err(format!("table {name} already exists"));
                }
                self.views.push((name.clone(), (**select).clone()));
                Ok(self.mutation(0))
            }
            Stmt::DropView { if_exists, name } => {
                let before = self.views.len();
                self.views.retain(|(n, _)| !n.eq_ignore_ascii_case(name));
                if self.views.len() == before && !*if_exists {
                    return Err(format!("no such view: {name}"));
                }
                Ok(self.mutation(0))
            }
            Stmt::CreateTrigger {
                name,
                timing,
                event,
                update_cols,
                table,
                when,
                body,
            } => {
                self.triggers.push(TriggerDef {
                    name: name.clone(),
                    timing: timing.clone(),
                    event: event.clone(),
                    update_cols: update_cols.clone(),
                    table: table.clone(),
                    when: when.clone(),
                    body: body.clone(),
                });
                Ok(self.mutation(0))
            }
            Stmt::DropTrigger { if_exists, name } => {
                let before = self.triggers.len();
                self.triggers.retain(|t| !t.name.eq_ignore_ascii_case(name));
                if self.triggers.len() == before && !*if_exists {
                    return Err(format!("no such trigger: {name}"));
                }
                Ok(self.mutation(0))
            }
            Stmt::CreateIndex {
                if_not_exists,
                name,
                table,
                unique,
                columns,
                enforce,
            } => {
                let tbl = self
                    .tbl_mut(table)
                    .ok_or_else(|| format!("no such table: {table}"))?;
                if tbl
                    .indexes
                    .iter()
                    .any(|ix| ix.name.eq_ignore_ascii_case(name))
                {
                    if *if_not_exists {
                        return Ok(self.mutation(0));
                    }
                    return Err(format!("index {name} already exists"));
                }

                let mut cols: Vec<usize> = Vec::new();
                let mut enforce = *enforce;
                for c in columns {
                    match tbl
                        .columns
                        .iter()
                        .position(|cd| cd.name.eq_ignore_ascii_case(c))
                    {
                        Some(i) => cols.push(i),
                        None => return Err(format!("no such column: {c}")),
                    }
                }
                if columns.is_empty() {
                    enforce = false;
                }
                tbl.indexes.push(IndexDef {
                    name: name.clone(),
                    unique: *unique,
                    cols,
                    enforce,
                });
                self.maybe_persist()?;
                Ok(self.mutation(0))
            }
            Stmt::DropIndex { if_exists, name } => {
                let mut found = false;
                for tbl in self.tables.values_mut() {
                    let before = tbl.indexes.len();
                    tbl.indexes.retain(|ix| !ix.name.eq_ignore_ascii_case(name));
                    if tbl.indexes.len() != before {
                        found = true;
                    }
                }
                if !found && !*if_exists {
                    return Err(format!("no such index: {name}"));
                }
                self.maybe_persist()?;
                Ok(self.mutation(0))
            }
            Stmt::Pragma { name, arg, value } => self.exec_pragma(name, arg, value, ctx),
        }
    }

    fn pragma_tvf(&self, name: &str, arg: &str) -> Option<SqlResult<Outcome>> {
        match name.to_ascii_uppercase().as_str() {
            "PRAGMA_TABLE_INFO" => Some(self.table_info_rows(arg)),
            "PRAGMA_INDEX_LIST" => Some(self.index_list_rows(arg)),
            "PRAGMA_INDEX_INFO" => Some(self.index_info_rows(arg)),
            "PRAGMA_FOREIGN_KEY_LIST" => Some(self.foreign_key_list_rows(arg)),
            "PRAGMA_DATABASE_LIST" => Some(Ok(self.database_list_rows())),
            "PRAGMA_COLLATION_LIST" => Some(Ok(Self::collation_list_rows())),
            _ => None,
        }
    }

    fn database_list_rows(&self) -> Outcome {
        let mut rows = vec![vec![
            Value::Int(0),
            Value::Text("main".into()),
            Value::Text(self.path.clone().unwrap_or_default()),
        ]];
        for (seq, name) in self.attached.keys().enumerate() {
            rows.push(vec![
                Value::Int(seq as i64 + 2),
                Value::Text(name.clone()),
                Value::Text(String::new()),
            ]);
        }
        Outcome::Rows {
            columns: vec!["seq".into(), "name".into(), "file".into()],
            rows,
        }
    }

    fn collation_list_rows() -> Outcome {
        let rows = ["RTRIM", "NOCASE", "BINARY"]
            .iter()
            .enumerate()
            .map(|(seq, name)| vec![Value::Int(seq as i64), Value::Text((*name).into())])
            .collect();
        Outcome::Rows {
            columns: vec!["seq".into(), "name".into()],
            rows,
        }
    }

    fn index_list_rows(&self, tname: &str) -> SqlResult<Outcome> {
        let tbl = self
            .tbl(tname)
            .ok_or_else(|| format!("no such table: {tname}"))?;
        let mut entries: Vec<(String, bool, &str)> = Vec::new();
        for idx in tbl.indexes.iter().rev() {
            entries.push((idx.name.clone(), idx.unique, "c"));
        }

        let mut n = 0;
        for c in &tbl.columns {
            if c.unique {
                n += 1;
                entries.push((format!("sqlite_autoindex_{tname}_{n}"), true, "u"));
            }
        }
        for _ in &tbl.table_uniques {
            n += 1;
            entries.push((format!("sqlite_autoindex_{tname}_{n}"), true, "u"));
        }
        let rows = entries
            .into_iter()
            .enumerate()
            .map(|(seq, (name, uniq, origin))| {
                vec![
                    Value::Int(seq as i64),
                    Value::Text(name),
                    Value::Int(uniq as i64),
                    Value::Text(origin.into()),
                    Value::Int(0),
                ]
            })
            .collect();
        Ok(Outcome::Rows {
            columns: vec![
                "seq".into(),
                "name".into(),
                "unique".into(),
                "origin".into(),
                "partial".into(),
            ],
            rows,
        })
    }

    fn index_info_rows(&self, iname: &str) -> SqlResult<Outcome> {
        let mut rows = Vec::new();
        for tbl in self.tables.values() {
            if let Some(idx) = tbl
                .indexes
                .iter()
                .find(|i| i.name.eq_ignore_ascii_case(iname))
            {
                for (seqno, &cid) in idx.cols.iter().enumerate() {
                    let cname = tbl
                        .columns
                        .get(cid)
                        .map(|c| c.name.clone())
                        .unwrap_or_default();
                    rows.push(vec![
                        Value::Int(seqno as i64),
                        Value::Int(cid as i64),
                        Value::Text(cname),
                    ]);
                }
                break;
            }
        }
        Ok(Outcome::Rows {
            columns: vec!["seqno".into(), "cid".into(), "name".into()],
            rows,
        })
    }

    fn foreign_key_list_rows(&self, tname: &str) -> SqlResult<Outcome> {
        let tbl = self
            .tbl(tname)
            .ok_or_else(|| format!("no such table: {tname}"))?;
        let mut rows = Vec::new();
        for (id, fk) in tbl.foreign_keys.iter().rev().enumerate() {
            let to = if fk.parent_col.is_empty() {
                self.tbl(&fk.parent_table)
                    .and_then(|pt| pt.columns.iter().find(|c| c.pk).map(|c| c.name.clone()))
                    .unwrap_or_default()
            } else {
                fk.parent_col.clone()
            };
            rows.push(vec![
                Value::Int(id as i64),
                Value::Int(0),
                Value::Text(fk.parent_table.clone()),
                Value::Text(fk.col.clone()),
                Value::Text(to),
                Value::Text("NO ACTION".into()),
                Value::Text(fk_action_text(fk.on_delete).into()),
                Value::Text("NONE".into()),
            ]);
        }
        Ok(Outcome::Rows {
            columns: vec![
                "id".into(),
                "seq".into(),
                "table".into(),
                "from".into(),
                "to".into(),
                "on_update".into(),
                "on_delete".into(),
                "match".into(),
            ],
            rows,
        })
    }

    fn table_info_rows(&self, tname: &str) -> SqlResult<Outcome> {
        let tbl = self
            .tbl(tname)
            .ok_or_else(|| format!("no such table: {tname}"))?;
        let mut rows = Vec::new();
        for (i, c) in tbl.columns.iter().enumerate() {
            if c.generated.is_some() {
                continue;
            }
            let dflt = match &c.default {
                Some(e) => Value::Text(render_default(e)),
                None => Value::Null,
            };
            rows.push(vec![
                Value::Int(i as i64),
                Value::Text(c.name.clone()),

                Value::Text(
                    c.decl_type
                        .clone()
                        .unwrap_or_else(|| affinity_type_name(c.affinity).to_string()),
                ),
                Value::Int(if c.not_null { 1 } else { 0 }),
                dflt,
                Value::Int(if c.pk { 1 } else { 0 }),
            ]);
        }
        Ok(Outcome::Rows {
            columns: vec![
                "cid".into(),
                "name".into(),
                "type".into(),
                "notnull".into(),
                "dflt_value".into(),
                "pk".into(),
            ],
            rows,
        })
    }

    fn exec_pragma(
        &mut self,
        name: &str,
        arg: &Option<String>,
        value: &Option<Expr>,
        ctx: &mut ParamCtx,
    ) -> SqlResult<Outcome> {
        let lname = name.to_ascii_lowercase();
        match lname.as_str() {
            "foreign_keys" => {

                let text = value.as_ref().map(expr_to_sql).or_else(|| arg.clone());
                match text {
                    Some(t) => {
                        let t = t.trim().trim_matches('\'').to_ascii_lowercase();
                        self.foreign_keys_on = matches!(t.as_str(), "on" | "1" | "true" | "yes");
                        Ok(self.mutation(0))
                    }
                    None => Ok(Outcome::Rows {
                        columns: vec!["foreign_keys".into()],
                        rows: vec![vec![Value::Int(self.foreign_keys_on as i64)]],
                    }),
                }
            }
            "user_version" => {
                if let Some(e) = value {
                    let v = eval(e, &[], &[], ctx)?.as_f64().unwrap_or(0.0) as i64;
                    self.user_version = v;
                    Ok(self.mutation(0))
                } else {
                    Ok(Outcome::Rows {
                        columns: vec!["user_version".into()],
                        rows: vec![vec![Value::Int(self.user_version)]],
                    })
                }
            }
            "table_info" => self.table_info_rows(&arg.clone().unwrap_or_default()),
            "index_list" => self.index_list_rows(&arg.clone().unwrap_or_default()),
            "index_info" => self.index_info_rows(&arg.clone().unwrap_or_default()),
            "foreign_key_list" => self.foreign_key_list_rows(&arg.clone().unwrap_or_default()),
            "database_list" => Ok(self.database_list_rows()),
            "collation_list" => Ok(Self::collation_list_rows()),

            "integrity_check" => Ok(Outcome::Rows {
                columns: vec!["integrity_check".into()],
                rows: vec![vec![Value::Text("ok".into())]],
            }),
            "quick_check" => Ok(Outcome::Rows {
                columns: vec!["quick_check".into()],
                rows: vec![vec![Value::Text("ok".into())]],
            }),

            "journal_mode" => {
                if value.is_some() || arg.is_some() {
                    Ok(self.mutation(0))
                } else {
                    Ok(Outcome::Rows {
                        columns: vec!["journal_mode".into()],
                        rows: vec![vec![Value::Text("memory".into())]],
                    })
                }
            }
            "synchronous" => {
                if value.is_some() || arg.is_some() {
                    Ok(self.mutation(0))
                } else {
                    Ok(Outcome::Rows {
                        columns: vec!["synchronous".into()],
                        rows: vec![vec![Value::Int(2)]],
                    })
                }
            }
            "encoding" => {
                if value.is_some() || arg.is_some() {
                    Ok(self.mutation(0))
                } else {
                    Ok(Outcome::Rows {
                        columns: vec!["encoding".into()],
                        rows: vec![vec![Value::Text("UTF-8".into())]],
                    })
                }
            }
            "page_size" => {
                if value.is_some() || arg.is_some() {
                    Ok(self.mutation(0))
                } else {
                    Ok(Outcome::Rows {
                        columns: vec!["page_size".into()],
                        rows: vec![vec![Value::Int(4096)]],
                    })
                }
            }
            "cache_size" => {
                if value.is_some() || arg.is_some() {
                    Ok(self.mutation(0))
                } else {
                    Ok(Outcome::Rows {
                        columns: vec!["cache_size".into()],
                        rows: vec![vec![Value::Int(-2000)]],
                    })
                }
            }

            _ => Ok(Outcome::Rows {
                columns: Vec::new(),
                rows: Vec::new(),
            }),
        }
    }

    fn sqlite_master_rows(&self) -> (Vec<ColumnDef>, Vec<Vec<Value>>) {
        let cols = ["type", "name", "tbl_name", "rootpage", "sql"];
        let coldefs: Vec<ColumnDef> = cols
            .iter()
            .map(|n| ColumnDef {
                name: (*n).to_string(),
                affinity: Affinity::Text,
                decl_type: None,
                pk: false,
                autoincrement: false,
                not_null: false,
                unique: false,
                default: None,
                generated: None,
            })
            .collect();
        let mut rows = Vec::new();
        let mut page = 2i64;
        for (name, _t) in self.tables.iter() {
            rows.push(vec![
                Value::Text("table".into()),
                Value::Text(name.clone()),
                Value::Text(name.clone()),
                Value::Int(page),
                Value::Text(format!("CREATE TABLE {name}")),
            ]);
            page += 1;
        }
        for (name, _s) in self.views.iter() {
            rows.push(vec![
                Value::Text("view".into()),
                Value::Text(name.clone()),
                Value::Text(name.clone()),
                Value::Int(0),
                Value::Text(format!("CREATE VIEW {name}")),
            ]);
        }
        for t in self.triggers.iter() {
            rows.push(vec![
                Value::Text("trigger".into()),
                Value::Text(t.name.clone()),
                Value::Text(t.table.clone()),
                Value::Int(0),
                Value::Text(format!("CREATE TRIGGER {}", t.name)),
            ]);
        }
        (coldefs, rows)
    }

    fn index_eq_source(&mut self, table: &str, col: usize, key: &Value) -> Vec<Vec<Value>> {
        let tbl = self.tbl_mut(table).unwrap();

        let coerced = coerce(key.clone(), tbl.columns[col].affinity);
        if !tbl.eq_indexes.contains_key(&col) {
            let idx = sql_core::EqIndex::build(tbl.rows.iter().map(|r| val_to_sql(&r[col])));
            tbl.eq_indexes.insert(col, idx);
        }
        let positions = tbl.eq_indexes[&col].probe(&val_to_sql(&coerced)).to_vec();
        let rows: Vec<Vec<Value>> = positions.iter().map(|&p| tbl.rows[p].clone()).collect();
        if std::env::var_os("CRUFT_SQL_PLAN").is_some() {
            eprintln!(
                "[sql-plan] IndexScan {table}.{} -> {} row(s)",
                tbl.columns[col].name,
                rows.len()
            );
        }
        rows
    }

    fn resolve_from(
        &mut self,
        name: &str,
        ctx: &mut ParamCtx,
    ) -> SqlResult<(Vec<ColumnDef>, Vec<Vec<Value>>)> {

        for scope in self.cte_scopes.iter().rev() {
            if let Some((_, rel)) = scope
                .iter()
                .rev()
                .find(|(n, _)| n.eq_ignore_ascii_case(name))
            {
                return Ok(rel.clone());
            }
        }
        if let Some(tbl) = self.tbl(name) {
            return Ok((tbl.columns.clone(), tbl.rows.clone()));
        }
        if name.eq_ignore_ascii_case("sqlite_master") || name.eq_ignore_ascii_case("sqlite_schema")
        {
            return Ok(self.sqlite_master_rows());
        }
        if let Some((_, sel)) = self
            .views
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .cloned()
        {
            let out = self.exec(&sel, ctx)?;
            if let Outcome::Rows { columns, rows } = out {
                let cds = columns
                    .iter()
                    .map(|n| ColumnDef {
                        name: n.clone(),
                        affinity: Affinity::Blob,
                        decl_type: None,
                        pk: false,
                        autoincrement: false,
                        not_null: false,
                        unique: false,
                        default: None,
                        generated: None,
                    })
                    .collect();
                return Ok((cds, rows));
            }
        }

        if matches!(
            name.to_ascii_uppercase().as_str(),
            "PRAGMA_DATABASE_LIST" | "PRAGMA_COLLATION_LIST"
        ) {
            if let Some(res) = self.pragma_tvf(name, "") {
                if let Outcome::Rows { columns, rows } = res? {
                    let cds = columns
                        .iter()
                        .map(|n| ColumnDef {
                            name: n.clone(),
                            affinity: Affinity::Blob,
                            decl_type: None,
                            pk: false,
                            autoincrement: false,
                            not_null: false,
                            unique: false,
                            default: None,
                            generated: None,
                        })
                        .collect();
                    return Ok((cds, rows));
                }
            }
        }
        Err(format!("no such table: {name}"))
    }

    fn exec_with(
        &mut self,
        recursive: bool,
        ctes: &[CteDef],
        body: &Stmt,
        ctx: &mut ParamCtx,
    ) -> SqlResult<Outcome> {
        self.cte_scopes.push(Vec::new());
        let result = self.exec_with_inner(recursive, ctes, body, ctx);
        self.cte_scopes.pop();
        result
    }

    fn exec_with_inner(
        &mut self,
        recursive: bool,
        ctes: &[CteDef],
        body: &Stmt,
        ctx: &mut ParamCtx,
    ) -> SqlResult<Outcome> {

        let pushdown: Option<usize> = if recursive && ctes.len() == 1 {
            passthrough_limit(body, &ctes[0].name, ctx)
        } else {
            None
        };
        for cte in ctes {

            self.cte_scopes
                .last_mut()
                .unwrap()
                .push((cte.name.clone(), (Vec::new(), Vec::new())));
            let idx = self.cte_scopes.last().unwrap().len() - 1;
            let rel = if recursive && stmt_refers_table(&cte.select, &cte.name) {
                self.materialize_recursive_cte(cte, pushdown, ctx)?
            } else {
                let out = self.exec(&cte.select, ctx)?;
                let (columns, rows) = match out {
                    Outcome::Rows { columns, rows } => (columns, rows),
                    _ => (Vec::new(), Vec::new()),
                };
                self.cte_rel(cte, columns, rows)
            };
            self.cte_scopes.last_mut().unwrap()[idx] = (cte.name.clone(), rel);
        }
        self.exec(body, ctx)
    }

    fn cte_rel(
        &self,
        cte: &CteDef,
        columns: Vec<String>,
        rows: Vec<Vec<Value>>,
    ) -> (Vec<ColumnDef>, Vec<Vec<Value>>) {
        let names: Vec<String> = match &cte.columns {
            Some(cols) if cols.len() == columns.len() => cols.clone(),
            _ => columns,
        };
        let cds = names
            .into_iter()
            .map(|n| ColumnDef {
                name: n,
                affinity: Affinity::Blob,
                decl_type: None,
                pk: false,
                autoincrement: false,
                not_null: false,
                unique: false,
                default: None,
                generated: None,
            })
            .collect();
        (cds, rows)
    }

    fn materialize_recursive_cte(
        &mut self,
        cte: &CteDef,
        pushdown: Option<usize>,
        ctx: &mut ParamCtx,
    ) -> SqlResult<(Vec<ColumnDef>, Vec<Vec<Value>>)> {
        let (first, rest) = match &*cte.select {
            Stmt::CompoundSelect { first, rest, .. } if !rest.is_empty() => (first, rest),
            _ => return Err("recursive CTE body must be `seed UNION [ALL] recursive-term`".into()),
        };
        let seed_out = self.exec(first, ctx)?;
        let (seed_cols, seed_rows) = match seed_out {
            Outcome::Rows { columns, rows } => (columns, rows),
            _ => (Vec::new(), Vec::new()),
        };
        let (cds, _) = self.cte_rel(cte, seed_cols, Vec::new());
        let union_all = matches!(rest[0].0, CompoundOp::UnionAll);
        let rec_term = &rest[0].1;

        let mut result: Vec<Vec<Value>> = Vec::new();
        let mut working: Vec<Vec<Value>> = Vec::new();
        for r in seed_rows {
            if union_all || !result.iter().any(|x| rows_equal(x, &r)) {
                result.push(r.clone());
                working.push(r);
            }
        }
        if pushdown.is_some_and(|n| result.len() >= n) {
            return Ok((cds, result));
        }
        const CAP: usize = 100_000;
        while !working.is_empty() {

            if let Some(scope) = self.cte_scopes.last_mut() {
                if let Some(e) = scope
                    .iter_mut()
                    .rev()
                    .find(|(n, _)| n.eq_ignore_ascii_case(&cte.name))
                {
                    e.1 = (cds.clone(), std::mem::take(&mut working));
                }
            }
            let out = self.exec(rec_term, ctx)?;
            let new_rows = match out {
                Outcome::Rows { rows, .. } => rows,
                _ => Vec::new(),
            };
            let mut next: Vec<Vec<Value>> = Vec::new();
            for r in new_rows {
                if union_all || !result.iter().any(|x| rows_equal(x, &r)) {
                    result.push(r.clone());
                    next.push(r);
                }
            }
            working = next;
            if pushdown.is_some_and(|n| result.len() >= n) {
                break;
            }
            if result.len() > CAP {
                return Err(
                    "recursive CTE exceeded the safety cap (non-terminating without an outer LIMIT to push down)".into(),
                );
            }
        }
        Ok((cds, result))
    }

    fn derived_source(
        &mut self,
        sub: &Stmt,
        ctx: &mut ParamCtx,
    ) -> SqlResult<(Vec<ColumnDef>, Vec<Vec<Value>>)> {

        match self.exec(sub, ctx)? {
            Outcome::Rows { columns, rows } => {
                let cds = columns
                    .iter()
                    .map(|n| ColumnDef {
                        name: n.clone(),
                        affinity: Affinity::Blob,
                        decl_type: None,
                        pk: false,
                        autoincrement: false,
                        not_null: false,
                        unique: false,
                        default: None,
                        generated: None,
                    })
                    .collect();
                Ok((cds, rows))
            }
            _ => Err("derived table is not a SELECT".into()),
        }
    }

    fn eval_tvf_row(
        &mut self,
        name: &str,
        args: &[Expr],
        schema: &[String],
        row: &[Value],
        ctx: &mut ParamCtx,
    ) -> SqlResult<(Vec<String>, Vec<Vec<Value>>)> {
        let arg0 = match args.first() {

            Some(e) => eval(&bind_cols(e, schema), row, schema, ctx)?,
            None => Value::Null,
        };
        if let Some(res) = self.pragma_tvf(name, &text_of(&arg0)) {
            return match res? {
                Outcome::Rows { columns, rows } => Ok((columns, rows)),
                _ => Ok((Vec::new(), Vec::new())),
            };
        }
        match functions::json::table_valued(name, &arg0) {
            Some(Ok((columns, rows))) => Ok((columns, rows)),
            Some(Err(e)) => Err(e),
            None => Err(format!("no such table-valued function: {name}")),
        }
    }

    fn lateral_tvf_join(
        &mut self,
        left: &sql_core::Plan,
        schema: &[String],
        name: &str,
        args: &[Expr],
        j: &JoinClause,
        ctx: &mut ParamCtx,
    ) -> SqlResult<(Vec<ColumnDef>, Vec<Vec<Value>>, Vec<String>)> {
        let left_rows = left.clone().execute()?;
        let jalias = j.alias.clone().unwrap_or_default();
        let mut jcds: Vec<ColumnDef> = Vec::new();
        let mut combined_schema: Vec<String> = Vec::new();
        let mut out: Vec<Vec<Value>> = Vec::new();
        for lr in &left_rows {
            let lvals: Vec<Value> = lr.iter().map(sql_to_val).collect();
            let (cols, trows) = self.eval_tvf_row(name, args, schema, &lvals, ctx)?;
            if jcds.is_empty() && !cols.is_empty() {
                jcds = cols
                    .iter()
                    .map(|n| ColumnDef {
                        name: n.clone(),
                        affinity: Affinity::Blob,
                        decl_type: None,
                        pk: false,
                        autoincrement: false,
                        not_null: false,
                        unique: false,
                        default: None,
                        generated: None,
                    })
                    .collect();
                combined_schema = schema.to_vec();
                combined_schema.extend(cols.iter().map(|c| format!("{jalias}.{c}")));
            }
            if trows.is_empty() {
                if matches!(j.kind, sql_core::JoinKind::Left) && !jcds.is_empty() {
                    let mut r = lvals.clone();
                    r.extend(std::iter::repeat(Value::Null).take(jcds.len()));
                    out.push(r);
                }
                continue;
            }
            for tr in trows {
                let mut r = lvals.clone();
                r.extend(tr);
                out.push(r);
            }
        }

        if combined_schema.is_empty() {
            let (cols, _) = self
                .eval_tvf_row(name, args, schema, &vec![Value::Null; schema.len()], ctx)
                .unwrap_or_default();
            jcds = cols
                .iter()
                .map(|n| ColumnDef {
                    name: n.clone(),
                    affinity: Affinity::Blob,
                    decl_type: None,
                    pk: false,
                    autoincrement: false,
                    not_null: false,
                    unique: false,
                    default: None,
                    generated: None,
                })
                .collect();
            combined_schema = schema.to_vec();
            combined_schema.extend(cols.iter().map(|c| format!("{jalias}.{c}")));
        }
        Ok((jcds, out, combined_schema))
    }

    fn has_triggers(&self, table: &str, event: &str) -> bool {
        self.triggers
            .iter()
            .any(|t| t.event.eq_ignore_ascii_case(event) && t.table.eq_ignore_ascii_case(table))
    }

    fn fire_triggers(
        &mut self,
        table: &str,
        event: &str,
        timing: &str,
        coldefs: &[ColumnDef],
        events: &[(Option<Vec<Value>>, Option<Vec<Value>>)],
        changed: &[String],
    ) -> SqlResult<Vec<bool>> {
        let matching: Vec<TriggerDef> = self
            .triggers
            .iter()
            .filter(|t| {
                t.event.eq_ignore_ascii_case(event)
                    && t.table.eq_ignore_ascii_case(table)
                    && t.timing.eq_ignore_ascii_case(timing)
            })
            .cloned()
            .collect();
        let mut skip = vec![false; events.len()];
        if matching.is_empty() {
            return Ok(skip);
        }
        let names: Vec<String> = coldefs.iter().map(|c| c.name.clone()).collect();
        for (i, (old, new)) in events.iter().enumerate() {
            'trig: for tr in &matching {

                if !tr.update_cols.is_empty()
                    && !tr
                        .update_cols
                        .iter()
                        .any(|c| changed.iter().any(|ch| ch.eq_ignore_ascii_case(c)))
                {
                    continue;
                }

                if let Some(w) = &tr.when {
                    let wsub = subst_expr(w, &names, old.as_deref(), new.as_deref());
                    let b = Bindings::new();
                    let mut c = ParamCtx {
                        b: &b,
                        next_pos: 0,
                        db: Some(self.clone()),
                        scopes: Vec::new(),
                        cur_alias: None,
                        col_aff: std::collections::HashMap::new(),
                        subq_cache: std::collections::HashMap::new(),
                    };
                    if !eval(&wsub, &[], &[], &mut c)?.truthy() {
                        continue;
                    }
                }
                for stmt in &tr.body {
                    let substituted = subst_stmt(stmt, &names, old.as_deref(), new.as_deref());
                    let b = Bindings::new();
                    let mut c = ParamCtx {
                        b: &b,
                        next_pos: 0,
                        db: Some(self.clone()),
                        scopes: Vec::new(),
                        cur_alias: None,
                        col_aff: std::collections::HashMap::new(),
                        subq_cache: std::collections::HashMap::new(),
                    };
                    match self.exec(&substituted, &mut c) {
                        Ok(_) => {}
                        Err(e) if e == RAISE_IGNORE => {
                            skip[i] = true;
                            break 'trig;
                        }
                        Err(e) => return Err(e),
                    }
                }
            }
        }
        Ok(skip)
    }

    fn mutation(&self, changes: i64) -> Outcome {
        Outcome::Mutation {
            changes,
            last_insert_rowid: self.last_insert_rowid,
        }
    }
    fn maybe_persist(&self) -> SqlResult<()> {
        if !self.in_txn {
            self.persist()?;
        }
        Ok(())
    }

    pub fn update_where_in_values_no_returning(
        &mut self,
        table: &str,
        sets: &[(String, sql_core::SqlValue)],
        col: &str,
        values: &[sql_core::SqlValue],
    ) -> SqlResult<usize> {
        if let Some(t) = self.tbl_mut(table) {
            t.eq_indexes.clear();
        }
        let coldefs = self
            .tbl(table)
            .ok_or_else(|| format!("no such table: {table}"))?
            .columns
            .clone();
        let names: Vec<String> = coldefs.iter().map(|c| c.name.clone()).collect();
        let col_idx = coldefs
            .iter()
            .position(|c| c.name.eq_ignore_ascii_case(col))
            .ok_or_else(|| format!("no such column: {col}"))?;
        let set_idx: Vec<(usize, Value)> = sets
            .iter()
            .map(|(c, v)| {
                coldefs
                    .iter()
                    .position(|d| d.name.eq_ignore_ascii_case(c))
                    .map(|i| (i, coerce(sql_to_val(v), coldefs[i].affinity)))
                    .ok_or_else(|| format!("no such column: {c}"))
            })
            .collect::<SqlResult<_>>()?;
        let affinity = coldefs[col_idx].affinity;
        let mut keyed = std::collections::HashSet::new();
        let mut fallback = Vec::new();
        for value in values {
            let v = coerce(sql_to_val(value), affinity);
            if matches!(v, Value::Null) {
                continue;
            }
            match gkey(&v) {
                Some(k) => {
                    keyed.insert(k);
                }
                None => fallback.push(v),
            }
        }
        let matches_in = |v: &Value| {
            if matches!(v, Value::Null) {
                return false;
            }
            let coerced = coerce(v.clone(), affinity);
            match gkey(&coerced) {
                Some(k) => keyed.contains(&k),
                None => fallback
                    .iter()
                    .any(|candidate| candidate.compare(&coerced) == std::cmp::Ordering::Equal),
            }
        };
        let changed: Vec<String> = sets.iter().map(|(c, _)| c.clone()).collect();
        let checks = self.tbl(table).unwrap().checks.clone();
        let mut snap = Some((self.tables.clone(), self.attached.clone()));
        let has_before_update = self.triggers.iter().any(|t| {
            t.timing.eq_ignore_ascii_case("BEFORE")
                && t.event.eq_ignore_ascii_case("UPDATE")
                && t.table.eq_ignore_ascii_case(table)
        });
        let order: Vec<usize> = {
            let tbl = self.tbl(table).unwrap();
            let mut o: Vec<usize> = (0..tbl.rows.len()).collect();
            o.sort_by_key(|&i| tbl.row_ids[i]);
            o
        };
        let b = Bindings::new();
        let mut ctx = ParamCtx {
            b: &b,
            next_pos: 0,
            db: Some(self.clone()),
            scopes: Vec::new(),
            cur_alias: None,
            col_aff: std::collections::HashMap::new(),
            subq_cache: std::collections::HashMap::new(),
        };
        for cd in &coldefs {
            ctx.col_aff.insert(cd.name.clone(), cd.affinity);
        }
        let mut changes = 0i64;
        let mut fired: Vec<(Option<Vec<Value>>, Option<Vec<Value>>)> = Vec::new();
        for idx in order {
            let old = self.tbl(table).unwrap().rows[idx].clone();
            if !old.get(col_idx).is_some_and(matches_in) {
                continue;
            }
            let mut newrow = old.clone();
            for (i, v) in &set_idx {
                newrow[*i] = v.clone();
            }
            if coldefs.iter().any(|c| c.generated.is_some()) {
                for i in 0..coldefs.len() {
                    if let Some(gen) = coldefs[i].generated.clone() {
                        newrow[i] =
                            coerce(eval(&gen, &newrow, &names, &mut ctx)?, coldefs[i].affinity);
                    }
                }
            }
            if let Some(err) = coldefs.iter().enumerate().find_map(|(i, cd)| {
                if cd.not_null && matches!(newrow[i], Value::Null) {
                    Some(format!(
                        "NOTNULL\x1fNOT NULL constraint failed: {table}.{}",
                        cd.name
                    ))
                } else {
                    None
                }
            }) {
                if let Some((t, a)) = snap.take() {
                    self.tables = t;
                    self.attached = a;
                }
                return Err(err);
            }
            for (e, src) in &checks {
                let v = eval(e, &newrow, &names, &mut ctx)?;
                if !matches!(v, Value::Null) && !v.truthy() {
                    if let Some((t, a)) = snap.take() {
                        self.tables = t;
                        self.attached = a;
                    }
                    return Err(format!("CHECK\x1fCHECK constraint failed: {src}"));
                }
            }
            if has_before_update {
                let ev = [(Some(old.clone()), Some(newrow.clone()))];
                match self.fire_triggers(table, "UPDATE", "BEFORE", &coldefs, &ev, &changed) {
                    Ok(f) => {
                        if f[0] {
                            continue;
                        }
                    }
                    Err(e) => {
                        if let Some((t, a)) = snap.take() {
                            self.tables = t;
                            self.attached = a;
                        }
                        return Err(e);
                    }
                }
            }
            self.tbl_mut(table).unwrap().rows[idx] = newrow.clone();
            fired.push((Some(old), Some(newrow)));
            changes += 1;
        }
        self.changes = changes;
        self.total_changes += changes;
        if let Err(e) = self.fire_triggers(table, "UPDATE", "AFTER", &coldefs, &fired, &changed) {
            if let Some((t, a)) = snap.take() {
                self.tables = t;
                self.attached = a;
            }
            return Err(e);
        }
        self.maybe_persist()?;
        Ok(changes.max(0) as usize)
    }

    pub fn delete_where_in_values_no_returning(
        &mut self,
        table: &str,
        col: &str,
        values: &[sql_core::SqlValue],
    ) -> SqlResult<usize> {
        if let Some(t) = self.tbl_mut(table) {
            t.eq_indexes.clear();
        }
        let coldefs = self
            .tbl(table)
            .ok_or_else(|| format!("no such table: {table}"))?
            .columns
            .clone();
        let col_idx = coldefs
            .iter()
            .position(|c| c.name.eq_ignore_ascii_case(col))
            .ok_or_else(|| format!("no such column: {col}"))?;
        let affinity = coldefs[col_idx].affinity;
        let mut keyed = std::collections::HashSet::new();
        let mut fallback = Vec::new();
        for value in values {
            let v = coerce(sql_to_val(value), affinity);
            if matches!(v, Value::Null) {
                continue;
            }
            match gkey(&v) {
                Some(k) => {
                    keyed.insert(k);
                }
                None => fallback.push(v),
            }
        }
        let matches_in = |v: &Value| {
            if matches!(v, Value::Null) {
                return false;
            }
            let coerced = coerce(v.clone(), affinity);
            match gkey(&coerced) {
                Some(k) => keyed.contains(&k),
                None => fallback
                    .iter()
                    .any(|candidate| candidate.compare(&coerced) == std::cmp::Ordering::Equal),
            }
        };
        let mut snap = if self.has_triggers(table, "DELETE") {
            Some((self.tables.clone(), self.attached.clone()))
        } else {
            None
        };
        let has_before_delete = self.triggers.iter().any(|t| {
            t.timing.eq_ignore_ascii_case("BEFORE")
                && t.event.eq_ignore_ascii_case("DELETE")
                && t.table.eq_ignore_ascii_case(table)
        });
        let b = Bindings::new();
        let mut ctx = ParamCtx {
            b: &b,
            next_pos: 0,
            db: Some(self.clone()),
            scopes: Vec::new(),
            cur_alias: None,
            col_aff: std::collections::HashMap::new(),
            subq_cache: std::collections::HashMap::new(),
        };
        let tbl = self.tbl_mut(table).unwrap();
        let before = tbl.rows.len();
        let old_rows = std::mem::take(&mut tbl.rows);
        let old_ids = std::mem::take(&mut tbl.row_ids);
        let mut kept = Vec::with_capacity(before);
        let mut kept_ids = Vec::with_capacity(before);
        let mut deleted_pairs: Vec<(i64, Vec<Value>)> = Vec::new();
        for (row, rid) in old_rows.into_iter().zip(old_ids) {
            if row.get(col_idx).is_some_and(matches_in) {
                deleted_pairs.push((rid, row));
            } else {
                kept.push(row);
                kept_ids.push(rid);
            }
        }
        deleted_pairs.sort_by_key(|(rid, _)| *rid);
        let deleted: Vec<Vec<Value>> = if has_before_delete {
            let mut final_deleted = Vec::new();
            for (rid, row) in deleted_pairs {
                let ev = [(Some(row.clone()), None)];
                match self.fire_triggers(table, "DELETE", "BEFORE", &coldefs, &ev, &[]) {
                    Ok(f) => {
                        if f[0] {
                            kept.push(row);
                            kept_ids.push(rid);
                            continue;
                        }
                    }
                    Err(e) => {
                        if let Some((t, a)) = snap.take() {
                            self.tables = t;
                            self.attached = a;
                        }
                        return Err(e);
                    }
                }
                final_deleted.push(row);
            }
            final_deleted
        } else {
            deleted_pairs.into_iter().map(|(_, r)| r).collect()
        };
        let tbl = self.tbl_mut(table).unwrap();
        tbl.rows = kept;
        tbl.row_ids = kept_ids;
        tbl.max_rowid = tbl.row_ids.iter().copied().max().unwrap_or(0);
        if self.foreign_keys_on && !deleted.is_empty() {
            self.cascade_deletes(table, &coldefs, &deleted, &mut ctx)?;
        }
        let changes = (before - self.tbl(table).unwrap().rows.len()) as i64;
        self.changes = changes;
        self.total_changes += changes;
        if self.has_triggers(table, "DELETE") {
            let events: Vec<(Option<Vec<Value>>, Option<Vec<Value>>)> =
                deleted.iter().map(|r| (Some(r.clone()), None)).collect();
            if let Err(e) = self.fire_triggers(table, "DELETE", "AFTER", &coldefs, &events, &[]) {
                if let Some((t, a)) = snap.take() {
                    self.tables = t;
                    self.attached = a;
                }
                return Err(e);
            }
        }
        self.maybe_persist()?;
        Ok(changes.max(0) as usize)
    }

    fn exec_insert(
        &mut self,
        table: &str,
        columns: &Option<Vec<String>>,
        rows: &[Vec<Expr>],
        or_action: &InsertOr,
        on_conflict: &Option<OnConflict>,
        returning: &Option<ReturningClause>,
        ctx: &mut ParamCtx,
    ) -> SqlResult<Outcome> {

        let coldefs = self
            .tbl(table)
            .ok_or_else(|| format!("no such table: {table}"))?
            .columns
            .clone();
        let names: Vec<String> = coldefs.iter().map(|c| c.name.clone()).collect();

        let target: Vec<usize> = match columns {
            Some(names) => names
                .iter()
                .map(|n| {
                    coldefs
                        .iter()
                        .position(|c| c.name.eq_ignore_ascii_case(n))
                        .ok_or_else(|| format!("no such column: {n}"))
                })
                .collect::<SqlResult<_>>()?,
            None => (0..coldefs.len()).collect(),
        };
        let mut changes = 0i64;
        let mut last_id = self.last_insert_rowid;

        let has_insert_trigger = self
            .triggers
            .iter()
            .any(|t| t.event.eq_ignore_ascii_case("INSERT") && t.table.eq_ignore_ascii_case(table));

        let has_update_trigger = self.has_triggers(table, "UPDATE");
        let has_before_update = self.triggers.iter().any(|t| {
            t.timing.eq_ignore_ascii_case("BEFORE")
                && t.event.eq_ignore_ascii_case("UPDATE")
                && t.table.eq_ignore_ascii_case(table)
        });

        let has_before_insert = self.triggers.iter().any(|t| {
            t.timing.eq_ignore_ascii_case("BEFORE")
                && t.event.eq_ignore_ascii_case("INSERT")
                && t.table.eq_ignore_ascii_case(table)
        });

        let mut snap = if has_insert_trigger || has_update_trigger {
            Some((self.tables.clone(), self.attached.clone()))
        } else {
            None
        };
        let want_returning = returning.is_some();
        let mut inserted: Vec<(Option<Vec<Value>>, Option<Vec<Value>>)> = Vec::new();

        let mut upsert_updated: Vec<(Option<Vec<Value>>, Option<Vec<Value>>)> = Vec::new();
        let mut upsert_changed: Vec<String> = Vec::new();

        let mut upsert_pending: Vec<(usize, Vec<Value>, Vec<Value>)> = Vec::new();

        let mut returned_rows: Vec<Vec<Value>> = Vec::new();
        for value_row in rows {
            if value_row.len() != target.len() {
                return Err("INSERT column/value count mismatch".into());
            }

            let mut full: Vec<Value> = coldefs
                .iter()
                .map(|c| match &c.default {
                    Some(e) => eval(e, &[], &[], ctx).unwrap_or(Value::Null),
                    None => Value::Null,
                })
                .collect();
            for (slot, expr) in target.iter().zip(value_row) {
                full[*slot] = coerce(eval(expr, &[], &[], ctx)?, coldefs[*slot].affinity);
            }

            if coldefs.iter().any(|c| c.generated.is_some()) {
                let gnames: Vec<String> = coldefs.iter().map(|c| c.name.clone()).collect();
                for i in 0..coldefs.len() {
                    if let Some(gen) = coldefs[i].generated.clone() {
                        let v = coerce(eval(&gen, &full, &gnames, ctx)?, coldefs[i].affinity);
                        full[i] = v;
                    }
                }
            }

            if has_before_insert {
                let ev = [(None, Some(full.clone()))];
                match self.fire_triggers(table, "INSERT", "BEFORE", &coldefs, &ev, &[]) {
                    Ok(f) => {
                        if f[0] {
                            continue;
                        }
                    }
                    Err(e) => {
                        if let Some((t, a)) = snap.take() {
                            self.tables = t;
                            self.attached = a;
                        }
                        return Err(e);
                    }
                }
            }

            if self.foreign_keys_on {
                let fks = self
                    .tbl(table)
                    .map(|t| t.foreign_keys.clone())
                    .unwrap_or_default();
                for fk in &fks {
                    let ci = match coldefs
                        .iter()
                        .position(|c| c.name.eq_ignore_ascii_case(&fk.col))
                    {
                        Some(i) => i,
                        None => continue,
                    };
                    if matches!(full[ci], Value::Null) {
                        continue;
                    }
                    let parent = match self.tbl(&fk.parent_table) {
                        Some(p) => p,
                        None => continue,
                    };
                    let pi = if fk.parent_col.is_empty() {
                        parent.columns.iter().position(|c| c.pk)
                    } else {
                        parent
                            .columns
                            .iter()
                            .position(|c| c.name.eq_ignore_ascii_case(&fk.parent_col))
                    };
                    let pi = match pi {
                        Some(i) => i,
                        None => continue,
                    };
                    let found = parent
                        .rows
                        .iter()
                        .any(|r| r[pi].compare(&full[ci]) == std::cmp::Ordering::Equal);
                    if !found {
                        return Err("FOREIGNKEY\x1fFOREIGN KEY constraint failed".into());
                    }
                }
            }

            let tbl = self.tbl_mut(table).unwrap();
            let mut rowid = tbl.next_rowid;

            let mut pk_auto_col: Option<usize> = None;
            if let Some(pk) = coldefs
                .iter()
                .position(|c| c.pk && c.affinity == Affinity::Integer)
            {
                match &full[pk] {
                    Value::Null => {

                        rowid = if coldefs[pk].autoincrement {
                            tbl.next_rowid
                        } else {

                            tbl.max_rowid + 1
                        };
                        full[pk] = Value::Int(rowid);
                        pk_auto_col = Some(pk);
                    }
                    Value::Int(v) => rowid = *v,
                    other => {
                        if let Some(f) = other.as_f64() {
                            rowid = f as i64;
                            full[pk] = Value::Int(rowid);
                        }
                    }
                }
            }
            if rowid >= tbl.next_rowid {
                tbl.next_rowid = rowid + 1;
            }

            let notnull_err = coldefs.iter().enumerate().find_map(|(i, cd)| {
                if cd.not_null && matches!(full[i], Value::Null) {
                    Some(format!(
                        "NOTNULL\x1fNOT NULL constraint failed: {table}.{}",
                        cd.name
                    ))
                } else {
                    None
                }
            });

            let mut unique_rows: Vec<usize> = Vec::new();
            let mut first_unique_err: Option<String> = None;
            for (i, cd) in coldefs.iter().enumerate() {

                if pk_auto_col == Some(i) {
                    continue;
                }
                if (cd.pk || cd.unique) && !matches!(full[i], Value::Null) {

                    if !tbl.eq_indexes.contains_key(&i) {
                        let idx =
                            sql_core::EqIndex::build(tbl.rows.iter().map(|r| val_to_sql(&r[i])));
                        tbl.eq_indexes.insert(i, idx);
                    }
                    let hits = tbl.eq_indexes[&i].probe(&val_to_sql(&full[i])).to_vec();
                    for ri in hits {
                        if !unique_rows.contains(&ri) {
                            unique_rows.push(ri);
                        }

                        let code = if cd.pk { "PRIMARYKEY" } else { "UNIQUE" };
                        first_unique_err.get_or_insert_with(|| {
                            format!("{code}\x1fUNIQUE constraint failed: {table}.{}", cd.name)
                        });
                    }
                }
            }
            for group in &tbl.table_uniques {
                if group.iter().any(|&i| matches!(full[i], Value::Null)) {
                    continue;
                }
                for (ri, r) in tbl.rows.iter().enumerate() {
                    if group
                        .iter()
                        .all(|&i| r[i].compare(&full[i]) == std::cmp::Ordering::Equal)
                    {
                        if !unique_rows.contains(&ri) {
                            unique_rows.push(ri);
                        }
                        let cols: Vec<String> = group
                            .iter()
                            .map(|&i| format!("{table}.{}", coldefs[i].name))
                            .collect();
                        first_unique_err.get_or_insert_with(|| {
                            format!("UNIQUE\x1fUNIQUE constraint failed: {}", cols.join(", "))
                        });
                    }
                }
            }

            for ix in &tbl.indexes {
                if !ix.enforce || !ix.unique || ix.cols.is_empty() {
                    continue;
                }
                if ix.cols.iter().any(|&i| matches!(full[i], Value::Null)) {
                    continue;
                }
                for (ri, r) in tbl.rows.iter().enumerate() {
                    if ix
                        .cols
                        .iter()
                        .all(|&i| r[i].compare(&full[i]) == std::cmp::Ordering::Equal)
                    {
                        if !unique_rows.contains(&ri) {
                            unique_rows.push(ri);
                        }
                        let cols: Vec<String> = ix
                            .cols
                            .iter()
                            .map(|&i| format!("{table}.{}", coldefs[i].name))
                            .collect();
                        first_unique_err.get_or_insert_with(|| {
                            format!("UNIQUE\x1fUNIQUE constraint failed: {}", cols.join(", "))
                        });
                    }
                }
            }

            let mut check_err: Option<String> = None;
            for (e, src) in &tbl.checks {
                let v = eval(e, &full, &names, ctx)?;
                if !matches!(v, Value::Null) && !v.truthy() {
                    check_err = Some(format!("CHECK\x1fCHECK constraint failed: {src}"));
                    break;
                }
            }
            let has_unique = !unique_rows.is_empty();

            match (or_action, on_conflict) {
                (InsertOr::Ignore, _) => {

                    if notnull_err.is_some() || has_unique || check_err.is_some() {
                        continue;
                    }
                }
                (InsertOr::Replace, _) => {

                    if let Some(e) = notnull_err {
                        return Err(e);
                    }
                    if let Some(e) = check_err {
                        return Err(e);
                    }
                    if has_unique {
                        let mut idxs = unique_rows.clone();
                        idxs.sort_unstable();
                        for ri in idxs.into_iter().rev() {
                            tbl.rows.remove(ri);
                            tbl.row_ids.remove(ri);

                            tbl.eq_indexes.clear();
                        }

                        tbl.max_rowid = tbl.row_ids.iter().copied().max().unwrap_or(0);
                    }
                }
                (_, Some(oc)) => {

                    if has_unique {
                        match &oc.action {
                            ConflictAction::Nothing => {
                                continue;
                            }
                            ConflictAction::Update { sets, where_ } => {

                                let mut set_idx: Vec<(usize, &Expr)> = Vec::new();
                                for (c, e) in sets {
                                    let i = coldefs
                                        .iter()
                                        .position(|d| d.name.eq_ignore_ascii_case(c))
                                        .ok_or_else(|| format!("no such column: {c}"))?;
                                    set_idx.push((i, e));
                                }

                                let mut ev_names = names.clone();
                                for cn in &names {
                                    ev_names.push(format!("excluded.{cn}"));
                                }
                                for ri in unique_rows.clone() {
                                    let existing = tbl.rows[ri].clone();
                                    let mut ev_row = existing.clone();
                                    ev_row.extend(full.iter().cloned());
                                    if let Some(w) = where_ {
                                        if !eval(w, &ev_row, &ev_names, ctx)?.truthy() {
                                            continue;
                                        }
                                    }
                                    let mut newrow = existing.clone();
                                    for (i, e) in &set_idx {
                                        newrow[*i] = coerce(
                                            eval(e, &ev_row, &ev_names, ctx)?,
                                            coldefs[*i].affinity,
                                        );
                                    }

                                    if coldefs.iter().any(|c| c.generated.is_some()) {
                                        for i in 0..coldefs.len() {
                                            if let Some(gen) = coldefs[i].generated.clone() {
                                                newrow[i] = coerce(
                                                    eval(&gen, &newrow, &names, ctx)?,
                                                    coldefs[i].affinity,
                                                );
                                            }
                                        }
                                    }

                                    upsert_pending.push((ri, existing, newrow));
                                }
                                if upsert_changed.is_empty() {
                                    upsert_changed = sets.iter().map(|(c, _)| c.clone()).collect();
                                }
                                continue;
                            }
                        }
                    } else {

                        if let Some(e) = notnull_err {
                            return Err(e);
                        }
                        if let Some(e) = check_err {
                            return Err(e);
                        }
                    }
                }
                (InsertOr::None, None) => {

                    if let Some(e) = notnull_err {
                        return Err(e);
                    }
                    if let Some(e) = first_unique_err {
                        return Err(e);
                    }
                    if let Some(e) = check_err {
                        return Err(e);
                    }
                }
            }

            if has_insert_trigger {
                inserted.push((None, Some(full.clone())));
            }
            if want_returning {
                returned_rows.push(full.clone());
            }
            let new_pos = tbl.rows.len();
            tbl.rows.push(full);
            tbl.row_ids.push(rowid);

            if rowid > tbl.max_rowid {
                tbl.max_rowid = rowid;
            }

            if !tbl.eq_indexes.is_empty() {
                let cols: Vec<usize> = tbl.eq_indexes.keys().copied().collect();
                for c in cols {
                    let key = val_to_sql(&tbl.rows[new_pos][c]);
                    tbl.eq_indexes.get_mut(&c).unwrap().insert(key, new_pos);
                }
            }
            last_id = rowid;
            changes += 1;
        }

        for (ri, existing, newrow) in upsert_pending {
            if has_before_update {
                let ev = [(Some(existing.clone()), Some(newrow.clone()))];
                match self.fire_triggers(table, "UPDATE", "BEFORE", &coldefs, &ev, &upsert_changed)
                {
                    Ok(f) => {
                        if f[0] {
                            continue;
                        }
                    }
                    Err(e) => {
                        if let Some((t, a)) = snap.take() {
                            self.tables = t;
                            self.attached = a;
                        }
                        return Err(e);
                    }
                }
            }
            self.tbl_mut(table).unwrap().rows[ri] = newrow.clone();
            returned_rows.push(newrow.clone());
            upsert_updated.push((Some(existing), Some(newrow)));
            changes += 1;
        }
        self.changes = changes;
        self.total_changes += changes;
        self.last_insert_rowid = last_id;

        if let Err(e) = self
            .fire_triggers(table, "INSERT", "AFTER", &coldefs, &inserted, &[])
            .and_then(|_| {
                self.fire_triggers(
                    table,
                    "UPDATE",
                    "AFTER",
                    &coldefs,
                    &upsert_updated,
                    &upsert_changed,
                )
            })
        {
            if let Some((t, a)) = snap.take() {
                self.tables = t;
                self.attached = a;
            }
            return Err(e);
        }
        self.maybe_persist()?;
        if let Some(rc) = returning {
            return project_returning(rc, &returned_rows, &names, ctx);
        }
        Ok(Outcome::Mutation {
            changes,
            last_insert_rowid: last_id,
        })
    }

    fn aggregate_project(
        &self,
        filtered: &[&Vec<Value>],
        names: &[String],
        items: &[SelectItem],
        star: bool,
        group_by: &[Expr],
        having: &Option<Expr>,
        order_by: &[(Expr, bool)],
        distinct: bool,
        limit: &Option<Expr>,
        offset: &Option<Expr>,
        ctx: &mut ParamCtx,
    ) -> SqlResult<Option<Outcome>> {
        use std::cmp::Ordering::Equal;

        let off = match offset {
            Some(e) => eval(e, &[], &[], ctx)?.as_f64().unwrap_or(0.0) as usize,
            None => 0,
        };
        let lim = match limit {
            Some(e) => Some(eval(e, &[], &[], ctx)?.as_f64().unwrap_or(0.0) as usize),
            None => None,
        };
        let apply_lim = |mut rows: Vec<Vec<Value>>| -> Vec<Vec<Value>> {
            if off > 0 {
                rows = rows.split_off(off.min(rows.len()));
            }
            if let Some(l) = lim {
                rows.truncate(l);
            }
            rows
        };
        if !group_by.is_empty() {
            let resolved_group_by: Vec<Expr> = group_by
                .iter()
                .map(|g| resolve_alias_refs(g, items, names))
                .collect();
            let mut groups: Vec<(Vec<Value>, Vec<&Vec<Value>>)> = Vec::new();

            let mut index: std::collections::HashMap<Vec<GKey>, usize> =
                std::collections::HashMap::new();
            let mut fell_back = false;
            for row in filtered {
                let mut key = Vec::with_capacity(resolved_group_by.len());
                for g in &resolved_group_by {
                    key.push(eval(g, row, names, ctx)?);
                }
                let gk: Option<Vec<GKey>> = key.iter().map(gkey).collect();
                let Some(gk) = gk else {
                    fell_back = true;
                    break;
                };
                if let Some(&idx) = index.get(&gk) {
                    groups[idx].1.push(row);
                } else {
                    index.insert(gk, groups.len());
                    groups.push((key, vec![*row]));
                }
            }
            if fell_back {

                groups.clear();
                for row in filtered {
                    let mut key = Vec::with_capacity(resolved_group_by.len());
                    for g in &resolved_group_by {
                        key.push(eval(g, row, names, ctx)?);
                    }
                    if let Some(slot) = groups.iter_mut().find(|(k, _)| {
                        k.len() == key.len()
                            && k.iter().zip(&key).all(|(a, b)| a.compare(b) == Equal)
                    }) {
                        slot.1.push(row);
                    } else {
                        groups.push((key, vec![*row]));
                    }
                }
            }

            if let Some(h) = having {

                let h = resolve_alias_refs(h, items, names);
                let mut kept = Vec::new();
                for grp in groups {
                    if eval_agg_expr(&h, &grp.1, names, ctx)?.truthy() {
                        kept.push(grp);
                    }
                }
                groups = kept;
            }

            if !order_by.is_empty() {
                let mut keyed: Vec<(Vec<Value>, &(Vec<Value>, Vec<&Vec<Value>>))> = Vec::new();
                for grp in &groups {
                    let mut keys = Vec::new();
                    for (k, _) in order_by {
                        let rk = resolve_order_key(k, items, &[]);
                        keys.push(eval_agg_expr(&rk, &grp.1, names, ctx)?);
                    }
                    keyed.push((keys, grp));
                }
                keyed.sort_by(|a, b| {
                    for (i, (_, desc)) in order_by.iter().enumerate() {
                        let ord = a.0[i].compare(&b.0[i]);
                        let ord = if *desc { ord.reverse() } else { ord };
                        if ord != Equal {
                            return ord;
                        }
                    }
                    Equal
                });
                groups = keyed.into_iter().map(|(_, g)| g.clone()).collect();
            }
            let out_names: Vec<String> = items
                .iter()
                .enumerate()
                .map(|(i, it)| select_item_name(it, i))
                .collect();
            let mut out_rows = Vec::new();
            for (_, grp) in &groups {
                let mut r = Vec::new();
                for it in items {

                    r.push(eval_agg_expr(&it.expr, grp, names, ctx)?);
                }
                out_rows.push(r);
            }
            if distinct {
                out_rows = dedup_rows(out_rows);
            }
            return Ok(Some(Outcome::Rows {
                columns: out_names,
                rows: apply_lim(out_rows),
            }));
        }

        let has_agg = !star && items.iter().any(|it| expr_contains_aggregate(&it.expr));
        if has_agg {
            let mut out_names = Vec::new();
            let mut out_row = Vec::new();
            for (idx, it) in items.iter().enumerate() {
                out_names.push(select_item_name(it, idx));
                out_row.push(eval_agg_expr(&it.expr, filtered, names, ctx)?);
            }
            return Ok(Some(Outcome::Rows {
                columns: out_names,
                rows: apply_lim(vec![out_row]),
            }));
        }
        Ok(None)
    }

    #[allow(clippy::too_many_arguments)]
    fn window_project(
        &self,
        filtered: &[&Vec<Value>],
        names: &[String],
        items: &[SelectItem],
        order_by: &[(Expr, bool)],
        limit: &Option<Expr>,
        offset: &Option<Expr>,
        distinct: bool,
        ctx: &mut ParamCtx,
    ) -> SqlResult<Outcome> {
        let mut specs: Vec<WinSpec> = Vec::new();
        let rewritten: Vec<SelectItem> = items
            .iter()
            .map(|it| SelectItem {
                expr: extract_windows(&it.expr, &mut specs),
                alias: it.alias.clone(),
                source: it.source.clone(),
            })
            .collect();

        let mut ext_names: Vec<String> = names.to_vec();
        for i in 0..specs.len() {
            ext_names.push(format!("__win{i}"));
        }
        let mut rows: Vec<Vec<Value>> = filtered.iter().map(|r| (*r).clone()).collect();
        for spec in &specs {
            let vals = self.compute_window(spec, filtered, names, ctx)?;
            for (ri, v) in vals.into_iter().enumerate() {
                rows[ri].push(v);
            }
        }

        let mut order: Vec<usize> = (0..rows.len()).collect();
        if !order_by.is_empty() {
            let keys: Vec<Expr> = order_by
                .iter()
                .map(|(k, _)| resolve_order_key(k, &rewritten, &[]))
                .collect();
            let mut keyed: Vec<(Vec<Value>, usize)> = Vec::new();
            for (i, row) in rows.iter().enumerate() {
                let mut kv = Vec::new();
                for k in &keys {
                    kv.push(eval(k, row, &ext_names, ctx)?);
                }
                keyed.push((kv, i));
            }
            keyed.sort_by(|a, b| {
                for (col, (_, desc)) in order_by.iter().enumerate() {
                    let ord = a.0[col].compare(&b.0[col]);
                    let ord = if *desc { ord.reverse() } else { ord };
                    if ord != std::cmp::Ordering::Equal {
                        return ord;
                    }
                }
                std::cmp::Ordering::Equal
            });
            order = keyed.into_iter().map(|(_, i)| i).collect();
        } else if let Some(spec) = specs.first() {

            if !spec.partition.is_empty() || !spec.order.is_empty() {
                let mut sort_exprs: Vec<(Expr, bool)> =
                    spec.partition.iter().map(|e| (e.clone(), false)).collect();
                sort_exprs.extend(spec.order.iter().cloned());
                let mut keyed: Vec<(Vec<Value>, usize)> = Vec::new();
                for (i, row) in rows.iter().enumerate() {
                    let mut kv = Vec::new();
                    for (k, _) in &sort_exprs {
                        kv.push(eval(k, row, &ext_names, ctx)?);
                    }
                    keyed.push((kv, i));
                }
                keyed.sort_by(|a, b| {
                    for (col, (_, desc)) in sort_exprs.iter().enumerate() {
                        let ord = a.0[col].compare(&b.0[col]);
                        let ord = if *desc { ord.reverse() } else { ord };
                        if ord != std::cmp::Ordering::Equal {
                            return ord;
                        }
                    }
                    std::cmp::Ordering::Equal
                });
                order = keyed.into_iter().map(|(_, i)| i).collect();
            }
        }

        let off = match offset {
            Some(e) => eval(e, &[], &[], ctx)?.as_f64().unwrap_or(0.0) as usize,
            None => 0,
        };
        let lim = match limit {
            Some(e) => Some(eval(e, &[], &[], ctx)?.as_f64().unwrap_or(0.0) as i64),
            None => None,
        };

        let out_names: Vec<String> = items
            .iter()
            .enumerate()
            .map(|(i, it)| select_item_name(it, i))
            .collect();

        let mut out_rows: Vec<Vec<Value>> = Vec::new();
        for (n, &i) in order.iter().enumerate() {
            if n < off {
                continue;
            }
            if let Some(l) = lim {
                if (out_rows.len() as i64) >= l {
                    break;
                }
            }
            let mut v = Vec::new();
            for it in &rewritten {
                v.push(eval(&it.expr, &rows[i], &ext_names, ctx)?);
            }
            out_rows.push(v);
        }
        if distinct {
            out_rows = dedup_rows(out_rows);
        }
        Ok(Outcome::Rows {
            columns: out_names,
            rows: out_rows,
        })
    }

    fn compute_window(
        &self,
        spec: &WinSpec,
        rows: &[&Vec<Value>],
        names: &[String],
        ctx: &mut ParamCtx,
    ) -> SqlResult<Vec<Value>> {
        use std::cmp::Ordering::Equal;
        let n = rows.len();
        let mut result = vec![Value::Null; n];

        let mut parts: Vec<(Vec<Value>, Vec<usize>)> = Vec::new();
        for i in 0..n {
            let key: Vec<Value> = spec
                .partition
                .iter()
                .map(|p| eval(p, rows[i], names, ctx))
                .collect::<SqlResult<_>>()?;
            if let Some(slot) = parts.iter_mut().find(|(k, _)| {
                k.len() == key.len() && k.iter().zip(&key).all(|(a, b)| a.compare(b) == Equal)
            }) {
                slot.1.push(i);
            } else {
                parts.push((key, vec![i]));
            }
        }

        for (_, members) in &parts {

            let mut ordered = members.clone();
            let mut okeys: Vec<Vec<Value>> = vec![Vec::new(); members.len()];
            if !spec.order.is_empty() {
                let mut keyed: Vec<(Vec<Value>, usize)> = Vec::new();
                for &i in members {
                    let k: Vec<Value> = spec
                        .order
                        .iter()
                        .map(|(e, _)| eval(e, rows[i], names, ctx))
                        .collect::<SqlResult<_>>()?;
                    keyed.push((k, i));
                }
                keyed.sort_by(|a, b| {
                    for (col, (_, desc)) in spec.order.iter().enumerate() {
                        let ord = a.0[col].compare(&b.0[col]);
                        let ord = if *desc { ord.reverse() } else { ord };
                        if ord != Equal {
                            return ord;
                        }
                    }
                    Equal
                });
                ordered = keyed.iter().map(|(_, i)| *i).collect();
                okeys = keyed.into_iter().map(|(k, _)| k).collect();
            }

            let mut group_ids = vec![0usize; ordered.len()];
            for p in 1..ordered.len() {
                group_ids[p] = group_ids[p - 1]
                    + if okey_eq(&okeys[p], &okeys[p - 1]) {
                        0
                    } else {
                        1
                    };
            }
            let desc = spec.order.first().map(|(_, d)| *d).unwrap_or(false);

            match spec.func.as_str() {
                "ROW_NUMBER" => {
                    for (pos, &i) in ordered.iter().enumerate() {
                        result[i] = Value::Int(pos as i64 + 1);
                    }
                }
                "RANK" | "DENSE_RANK" => {
                    let dense = spec.func == "DENSE_RANK";
                    let mut rank = 0i64;
                    for pos in 0..ordered.len() {
                        let same = pos > 0 && okey_eq(&okeys[pos], &okeys[pos - 1]);
                        if !same {
                            rank = if dense { rank + 1 } else { pos as i64 + 1 };
                        }
                        result[ordered[pos]] = Value::Int(rank);
                    }
                }
                "NTILE" => {
                    let buckets = match spec.args.first() {
                        Some(e) => eval(e, &[], &[], ctx)?.as_f64().unwrap_or(1.0) as i64,
                        None => 1,
                    }
                    .max(1);
                    let total = ordered.len() as i64;
                    let base = total / buckets;
                    let rem = total % buckets;
                    let mut p = 0i64;
                    for b in 0..buckets {
                        let size = base + if b < rem { 1 } else { 0 };
                        for _ in 0..size {
                            if p < total {
                                result[ordered[p as usize]] = Value::Int(b + 1);
                                p += 1;
                            }
                        }
                    }
                }
                "FIRST_VALUE" | "LAST_VALUE" | "NTH_VALUE" => {
                    let ordered_by = !spec.order.is_empty();
                    let exclude = spec.frame.as_ref().map(|f| f.exclude).unwrap_or_default();
                    for pos in 0..ordered.len() {
                        let (s, e) = frame_range(
                            pos,
                            ordered.len(),
                            &okeys,
                            ordered_by,
                            &spec.frame,
                            desc,
                            &group_ids,
                        );

                        let incl: Vec<usize> = (s..e)
                            .filter(|&oi| !frame_excluded(oi, pos, exclude, &okeys))
                            .collect();
                        let target: Option<usize> = if incl.is_empty() {
                            None
                        } else {
                            match spec.func.as_str() {
                                "FIRST_VALUE" => Some(incl[0]),
                                "LAST_VALUE" => Some(*incl.last().unwrap()),
                                _ => {
                                    let nn = match spec.args.get(1) {
                                        Some(ex) => {
                                            eval(ex, &[], &[], ctx)?.as_f64().unwrap_or(0.0) as i64
                                        }
                                        None => 0,
                                    };
                                    if nn >= 1 && (nn as usize) <= incl.len() {
                                        Some(incl[(nn as usize) - 1])
                                    } else {
                                        None
                                    }
                                }
                            }
                        };
                        result[ordered[pos]] = match (target, spec.args.first()) {
                            (Some(t), Some(a)) => eval(a, rows[ordered[t]], names, ctx)?,
                            _ => Value::Null,
                        };
                    }
                }
                "LAG" | "LEAD" => {
                    let lead = spec.func == "LEAD";
                    let off = match spec.args.get(1) {
                        Some(e) => eval(e, &[], &[], ctx)?.as_f64().unwrap_or(1.0) as i64,
                        None => 1,
                    };
                    let default = match spec.args.get(2) {
                        Some(e) => eval(e, &[], &[], ctx)?,
                        None => Value::Null,
                    };
                    for pos in 0..ordered.len() {
                        let target = if lead {
                            pos as i64 + off
                        } else {
                            pos as i64 - off
                        };
                        let val = if target >= 0 && (target as usize) < ordered.len() {
                            match spec.args.first() {
                                Some(a) => eval(a, rows[ordered[target as usize]], names, ctx)?,
                                None => Value::Null,
                            }
                        } else {
                            default.clone()
                        };
                        result[ordered[pos]] = val;
                    }
                }
                _ => {

                    let agg = Expr::Func(spec.func.clone(), spec.args.clone());

                    let keep = |i: usize, ctx: &mut ParamCtx| -> SqlResult<bool> {
                        match &spec.filter {
                            Some(f) => Ok(eval(f, rows[i], names, ctx)?.truthy()),
                            None => Ok(true),
                        }
                    };
                    if spec.frame.is_none() && spec.order.is_empty() {

                        let mut subset: Vec<&Vec<Value>> = Vec::new();
                        for &i in members {
                            if keep(i, ctx)? {
                                subset.push(rows[i]);
                            }
                        }
                        let v = eval_aggregate(&agg, &subset, names, ctx)?;
                        for &i in members {
                            result[i] = v.clone();
                        }
                    } else {
                        let ordered_by = !spec.order.is_empty();
                        let exclude = spec.frame.as_ref().map(|f| f.exclude).unwrap_or_default();
                        for pos in 0..ordered.len() {
                            let (s, e) = frame_range(
                                pos,
                                ordered.len(),
                                &okeys,
                                ordered_by,
                                &spec.frame,
                                desc,
                                &group_ids,
                            );
                            let mut subset: Vec<&Vec<Value>> = Vec::new();
                            for oi in s..e {
                                if frame_excluded(oi, pos, exclude, &okeys) {
                                    continue;
                                }
                                let i = ordered[oi];
                                if keep(i, ctx)? {
                                    subset.push(rows[i]);
                                }
                            }
                            result[ordered[pos]] = eval_aggregate(&agg, &subset, names, ctx)?;
                        }
                    }
                }
            }
        }
        Ok(result)
    }

    fn exec_select(&mut self, stmt: &Stmt, ctx: &mut ParamCtx) -> SqlResult<Outcome> {
        let Stmt::Select {
            items,
            star,
            from,
            from_sub,
            from_alias,
            joins,
            where_,
            group_by,
            having,
            distinct,
            order_by,
            limit,
            offset,
        } = stmt
        else {
            unreachable!()
        };

        ctx.cur_alias = from_alias
            .clone()
            .or_else(|| from.clone().filter(|s| !s.is_empty()));
        if !joins.is_empty() {
            return self.exec_join_select(
                items, *star, from, from_sub, from_alias, joins, where_, group_by, having,
                *distinct, order_by, limit, offset, ctx,
            );
        }

        let needs_rowid = items.iter().any(|it| expr_refs_rowid(&it.expr))
            || where_.as_ref().is_some_and(expr_refs_rowid)
            || order_by.iter().any(|(k, _)| expr_refs_rowid(k))
            || group_by.iter().any(expr_refs_rowid)
            || having.as_ref().is_some_and(expr_refs_rowid);

        let single_table: Option<&str> = match (from_sub, from) {
            (None, Some(t)) if self.tables.contains_key(t) => Some(t.as_str()),
            _ => None,
        };
        let idx_eq = match single_table {
            Some(t) if !needs_rowid => {
                let cds = self.tables[t].columns.clone();
                find_indexable_eq(where_, &cds, ctx)?
            }
            _ => None,
        };

        let coldefs: Vec<ColumnDef>;
        let names: Vec<String>;
        let ncols: usize;
        let owned: Vec<Vec<Value>>;
        let source: &[Vec<Value>];
        if let (Some(t), Some((ci, key))) = (single_table, idx_eq) {
            coldefs = self.tables[t].columns.clone();
            names = coldefs.iter().map(|c| c.name.clone()).collect();
            ncols = coldefs.len();
            owned = self.index_eq_source(t, ci, &key);
            source = &owned;
        } else if let Some(t) = single_table.filter(|_| !needs_rowid) {
            let tbl = &self.tables[t];
            coldefs = tbl.columns.clone();
            names = coldefs.iter().map(|c| c.name.clone()).collect();
            ncols = coldefs.len();
            owned = Vec::new();
            source = &tbl.rows;
            if std::env::var_os("CRUFT_SQL_PLAN").is_some() {
                eprintln!("[sql-plan] SeqScan(ref) {t} ({} rows)", source.len());
            }
        } else {
            let (cds, src) = match (from_sub, from) {
                (Some(sub), _) => self.derived_source(sub, ctx)?,
                (None, Some(t)) => self.resolve_from(t, ctx)?,
                (None, None) => (Vec::new(), vec![Vec::new()]),
            };
            coldefs = cds;
            let base_names: Vec<String> = coldefs.iter().map(|c| c.name.clone()).collect();
            ncols = base_names.len();
            if needs_rowid {

                let row_ids: Vec<i64> = match (from_sub, from) {
                    (None, Some(t)) => self
                        .tbl(t)
                        .map(|tb| tb.row_ids.clone())
                        .unwrap_or_else(|| (1..=src.len() as i64).collect()),
                    _ => (1..=src.len() as i64).collect(),
                };
                names = base_names
                    .iter()
                    .cloned()
                    .chain(std::iter::once("rowid".to_string()))
                    .collect();
                owned = src
                    .into_iter()
                    .enumerate()
                    .map(|(i, mut r)| {
                        r.push(Value::Int(row_ids.get(i).copied().unwrap_or(i as i64 + 1)));
                        r
                    })
                    .collect();
            } else {
                names = base_names;
                owned = src;
            }
            source = &owned;
        }

        ctx.col_aff.clear();
        for (cd, nm) in coldefs.iter().zip(&names) {
            ctx.col_aff.insert(nm.clone(), cd.affinity);
            ctx.col_aff.insert(cd.name.clone(), cd.affinity);
        }

        let mut filtered: Vec<&Vec<Value>> = Vec::new();
        for row in source {
            match where_ {
                Some(e) => {
                    if eval(e, row, &names, ctx)?.truthy() {
                        filtered.push(row);
                    }
                }
                None => filtered.push(row),
            }
        }

        if let Some(out) = self.aggregate_project(
            &filtered, &names, items, *star, group_by, having, order_by, *distinct, limit, offset,
            ctx,
        )? {
            return Ok(out);
        }

        if items.iter().any(|it| expr_has_window(&it.expr)) {
            return self.window_project(
                &filtered, &names, items, order_by, limit, offset, *distinct, ctx,
            );
        }

        if !order_by.is_empty() {

            let star_names: &[String] = if *star { &names[..ncols] } else { &[] };
            let resolved_keys: Vec<Expr> = order_by
                .iter()
                .map(|(k, _)| resolve_order_key(k, items, star_names))
                .collect();
            let key_colls: Vec<Collation> = resolved_keys
                .iter()
                .map(|k| expr_collation(k).unwrap_or(Collation::Binary))
                .collect();
            let mut keyed: Vec<(Vec<Value>, &Vec<Value>)> = Vec::new();
            for row in &filtered {
                let mut keys = Vec::new();
                for k in &resolved_keys {
                    keys.push(eval(k, row, &names, ctx)?);
                }
                keyed.push((keys, row));
            }
            keyed.sort_by(|a, b| {
                for (i, (_, desc)) in order_by.iter().enumerate() {
                    let ord = compare_coll(&a.0[i], &b.0[i], key_colls[i]);
                    let ord = if *desc { ord.reverse() } else { ord };
                    if ord != std::cmp::Ordering::Equal {
                        return ord;
                    }
                }
                std::cmp::Ordering::Equal
            });
            filtered = keyed.into_iter().map(|(_, r)| r).collect();
        }

        let off = match offset {
            Some(e) => eval(e, &[], &[], ctx)?.as_f64().unwrap_or(0.0) as usize,
            None => 0,
        };
        let lim = match limit {
            Some(e) => Some(eval(e, &[], &[], ctx)?.as_f64().unwrap_or(0.0) as i64),
            None => None,
        };

        let out_names: Vec<String> = if *star {
            names[..ncols].to_vec()
        } else {
            items
                .iter()
                .enumerate()
                .map(|(i, it)| select_item_name(it, i))
                .collect()
        };
        let mut out_rows = Vec::new();
        for (n, row) in filtered.iter().enumerate() {
            if n < off {
                continue;
            }
            if let Some(l) = lim {
                if (out_rows.len() as i64) >= l {
                    break;
                }
            }
            let projected: Vec<Value> = if *star {
                (*row)[..ncols].to_vec()
            } else {
                let mut v = Vec::new();
                for it in items {
                    v.push(eval(&it.expr, row, &names, ctx)?);
                }
                v
            };
            out_rows.push(projected);
        }
        if *distinct {
            out_rows = dedup_rows(out_rows);
        }
        Ok(Outcome::Rows {
            columns: out_names,
            rows: out_rows,
        })
    }

    fn exec_compound(
        &mut self,
        first: &Stmt,
        rest: &[(CompoundOp, Stmt)],
        order_by: &[(Expr, bool)],
        limit: &Option<Expr>,
        offset: &Option<Expr>,
        ctx: &mut ParamCtx,
    ) -> SqlResult<Outcome> {

        let (columns, mut rows) = match self.exec(first, ctx)? {
            Outcome::Rows { columns, rows } => (columns, rows),
            _ => return Err("compound arm is not a SELECT".into()),
        };
        for (op, arm) in rest {
            let arm_rows = match self.exec(arm, ctx)? {
                Outcome::Rows { rows, .. } => rows,
                _ => return Err("compound arm is not a SELECT".into()),
            };
            rows = combine_compound(*op, rows, arm_rows);
        }

        let has_distinct_op = rest
            .iter()
            .any(|(op, _)| !matches!(op, CompoundOp::UnionAll));
        if order_by.is_empty() && has_distinct_op {
            rows.sort_by(|a, b| {
                for (x, y) in a.iter().zip(b.iter()) {
                    let c = x.compare(y);
                    if c != std::cmp::Ordering::Equal {
                        return c;
                    }
                }
                std::cmp::Ordering::Equal
            });
        }

        if !order_by.is_empty() {
            let mut keyed: Vec<(Vec<Value>, Vec<Value>)> = Vec::new();
            for row in rows.into_iter() {
                let mut keys = Vec::new();
                for (k, _) in order_by {
                    let kv = match k {
                        Expr::Lit(Value::Int(n)) if *n >= 1 && (*n as usize) <= row.len() => {
                            row[(*n as usize) - 1].clone()
                        }
                        _ => eval(k, &row, &columns, ctx)?,
                    };
                    keys.push(kv);
                }
                keyed.push((keys, row));
            }
            keyed.sort_by(|a, b| {
                for (i, (_, desc)) in order_by.iter().enumerate() {
                    let ord = a.0[i].compare(&b.0[i]);
                    let ord = if *desc { ord.reverse() } else { ord };
                    if ord != std::cmp::Ordering::Equal {
                        return ord;
                    }
                }
                std::cmp::Ordering::Equal
            });
            rows = keyed.into_iter().map(|(_, r)| r).collect();
        }

        let off = match offset {
            Some(e) => eval(e, &[], &[], ctx)?.as_f64().unwrap_or(0.0) as usize,
            None => 0,
        };
        let lim = match limit {
            Some(e) => Some(eval(e, &[], &[], ctx)?.as_f64().unwrap_or(0.0) as i64),
            None => None,
        };
        let mut out_rows = Vec::new();
        for (n, row) in rows.into_iter().enumerate() {
            if n < off {
                continue;
            }
            if let Some(l) = lim {
                if (out_rows.len() as i64) >= l {
                    break;
                }
            }
            out_rows.push(row);
        }
        Ok(Outcome::Rows {
            columns,
            rows: out_rows,
        })
    }

    fn exec_join_select(
        &mut self,
        items: &[SelectItem],
        star: bool,
        from: &Option<String>,
        from_sub: &Option<Box<Stmt>>,
        from_alias: &Option<String>,
        joins: &[JoinClause],
        where_: &Option<Expr>,
        group_by: &[Expr],
        having: &Option<Expr>,
        distinct: bool,
        order_by: &[(Expr, bool)],
        limit: &Option<Expr>,
        offset: &Option<Expr>,
        ctx: &mut ParamCtx,
    ) -> SqlResult<Outcome> {
        use sql_core::Plan;

        let mut col_aff: std::collections::HashMap<String, Affinity> =
            std::collections::HashMap::new();

        let (fcds, frows): (Vec<ColumnDef>, Vec<Vec<Value>>) = match from_sub {
            Some(sub) => self.derived_source(sub, ctx)?,
            None => {
                let ft = from.as_ref().ok_or("JOIN requires a FROM table")?;

                self.resolve_from(ft, ctx)?
            }
        };
        let falias = from_alias
            .clone()
            .or_else(|| from.clone().filter(|s| !s.is_empty()))
            .unwrap_or_default();
        let mut schema: Vec<String> = fcds
            .iter()
            .map(|c| format!("{falias}.{}", c.name))
            .collect();
        for c in &fcds {
            col_aff.insert(format!("{falias}.{}", c.name), c.affinity);
            col_aff.entry(c.name.clone()).or_insert(c.affinity);
        }
        let mut plan = Plan::Scan(rows_to_sql(&frows));
        for j in joins {

            if let Some(sub) = &j.sub {
                if let Stmt::TableFunc { name, args } = &**sub {
                    if args.iter().any(expr_has_col) {
                        let (jcds, combined_rows, combined_schema) =
                            self.lateral_tvf_join(&plan, &schema, name, args, j, ctx)?;

                        let no_using = std::collections::HashSet::new();
                        for a in args {
                            if let Some(nm) = first_ambiguous_col(a, &combined_schema, &no_using) {
                                return Err(format!("ambiguous column name: {nm}"));
                            }
                        }
                        let jalias = j.alias.clone().unwrap_or_default();
                        for c in &jcds {
                            col_aff.insert(format!("{jalias}.{}", c.name), c.affinity);
                            col_aff.entry(c.name.clone()).or_insert(c.affinity);
                        }
                        plan = Plan::Scan(rows_to_sql(&combined_rows));
                        schema = combined_schema;
                        continue;
                    }
                }
            }
            let (jcds, jrows): (Vec<ColumnDef>, Vec<Vec<Value>>) = match &j.sub {
                Some(sub) => self.derived_source(sub, ctx)?,
                None => {

                    self.resolve_from(&j.table, ctx)?
                }
            };
            let jalias = j.alias.clone().unwrap_or_else(|| j.table.clone());
            let right_cols: Vec<String> = jcds
                .iter()
                .map(|c| format!("{jalias}.{}", c.name))
                .collect();
            for c in &jcds {
                col_aff.insert(format!("{jalias}.{}", c.name), c.affinity);
                col_aff.entry(c.name.clone()).or_insert(c.affinity);
            }
            let right_width = jcds.len();
            let right = Plan::Scan(rows_to_sql(&jrows));
            let combined: Vec<String> = schema.iter().chain(right_cols.iter()).cloned().collect();
            let left_width = schema.len();

            let mut equi: Vec<(usize, usize)> = Vec::new();
            let pred = if !j.using.is_empty() {

                let mut pairs: Vec<(usize, usize)> = Vec::new();
                for col in &j.using {
                    let li = schema
                        .iter()
                        .position(|s| s.rsplit_once('.').map(|(_, c)| c) == Some(col.as_str()))
                        .ok_or_else(|| {
                            format!(
                                "cannot join using column {col} - column not present in left table"
                            )
                        })?;
                    let rp = right_cols.iter().position(|s| s.rsplit_once('.').map(|(_, c)| c) == Some(col.as_str()))
                        .ok_or_else(|| format!("cannot join using column {col} - column not present in right table"))?;
                    if same_aff_class(
                        col_aff.get(&schema[li]).copied(),
                        col_aff.get(&right_cols[rp]).copied(),
                    ) {
                        equi.push((li, rp));
                    }
                    pairs.push((li, left_width + rp));
                }
                Some(Box::new(move |r: &sql_core::Row| {
                    Ok(pairs
                        .iter()
                        .all(|&(l, rr)| r[l].cmp(&r[rr]) == std::cmp::Ordering::Equal))
                }) as sql_core::Pred)
            } else {
                if let Some(on) = &j.on {
                    equi = extract_equi(on, &combined, left_width, &col_aff);
                }
                j.on.as_ref().map(|on| {
                    make_pred(
                        bind_cols(on, &combined),
                        combined.clone(),
                        ctx.b.clone(),
                        col_aff.clone(),
                    )
                })
            };

            if !equi.is_empty() {
                let left_keys: Vec<sql_core::Scalar> = equi
                    .iter()
                    .map(|&(li, _)| {
                        Box::new(move |r: &sql_core::Row| Ok(r[li].clone())) as sql_core::Scalar
                    })
                    .collect();
                let right_keys: Vec<sql_core::Scalar> = equi
                    .iter()
                    .map(|&(_, ri)| {
                        Box::new(move |r: &sql_core::Row| Ok(r[ri].clone())) as sql_core::Scalar
                    })
                    .collect();
                plan = Plan::HashJoin {
                    left: Box::new(plan),
                    right: Box::new(right),
                    left_width,
                    right_width,
                    kind: j.kind,
                    left_keys,
                    right_keys,
                    extra: pred,
                };
            } else {
                plan = Plan::NestedLoopJoin {
                    left: Box::new(plan),
                    right: Box::new(right),
                    left_width,
                    right_width,
                    kind: j.kind,
                    pred,
                };
            }
            schema = combined;
        }

        let using_cols: std::collections::HashSet<String> = joins
            .iter()
            .flat_map(|j| j.using.iter().map(|c| c.to_ascii_lowercase()))
            .collect();

        let resolved_order: Vec<Expr> = order_by
            .iter()
            .map(|(e, _)| resolve_order_key(e, items, &[]))
            .collect();
        let mut check_exprs: Vec<&Expr> = Vec::new();
        if let Some(w) = where_ {
            check_exprs.push(w);
        }
        if let Some(h) = having {
            check_exprs.push(h);
        }
        for it in items {
            check_exprs.push(&it.expr);
        }
        check_exprs.extend(group_by.iter());
        check_exprs.extend(resolved_order.iter());
        for j in joins {
            if let Some(on) = &j.on {
                check_exprs.push(on);
            }
        }
        for e in &check_exprs {
            if let Some(name) = first_ambiguous_col(e, &schema, &using_cols) {
                return Err(format!("ambiguous column name: {name}"));
            }
        }
        if let Some(w) = where_ {
            plan = Plan::Filter {
                input: Box::new(plan),
                pred: make_pred(
                    bind_cols(w, &schema),
                    schema.clone(),
                    ctx.b.clone(),
                    col_aff.clone(),
                ),
            };
        }

        let need_agg = !group_by.is_empty()
            || (!star && items.iter().any(|it| expr_contains_aggregate(&it.expr)));
        if need_agg {
            let sql_rows = plan.execute()?;
            let vrows: Vec<Vec<Value>> = sql_rows
                .iter()
                .map(|r| r.iter().map(sql_to_val).collect())
                .collect();
            let refs: Vec<&Vec<Value>> = vrows.iter().collect();
            if let Some(out) = self.aggregate_project(
                &refs, &schema, items, star, group_by, having, order_by, distinct, limit, offset,
                ctx,
            )? {
                return Ok(out);
            }
        }

        if !star && items.iter().any(|it| expr_has_window(&it.expr)) {
            let sql_rows = plan.execute()?;
            let vrows: Vec<Vec<Value>> = sql_rows
                .iter()
                .map(|r| r.iter().map(sql_to_val).collect())
                .collect();
            let refs: Vec<&Vec<Value>> = vrows.iter().collect();
            return self.window_project(
                &refs, &schema, items, order_by, limit, offset, distinct, ctx,
            );
        }
        if !order_by.is_empty() {

            let star_names: &[String] = if star { &schema } else { &[] };
            let keys = order_by
                .iter()
                .map(|(k, desc)| {
                    let rk = resolve_order_key(k, items, star_names);
                    let collation = expr_collation(&rk)
                        .unwrap_or(Collation::Binary)
                        .to_sql_core();
                    (
                        make_scalar(
                            bind_cols(&rk, &schema),
                            schema.clone(),
                            ctx.b.clone(),
                            col_aff.clone(),
                        ),
                        sql_core::SortOptions::with_default(
                            *desc,
                            None,
                            sql_core::NullsDefault::Sqlite,
                            collation,
                        ),
                    )
                })
                .collect();
            plan = Plan::Sort {
                input: Box::new(plan),
                keys,
            };
        }
        let out_names: Vec<String> = if star {
            schema
                .iter()
                .map(|s| {
                    s.rsplit_once('.')
                        .map(|(_, c)| c.to_string())
                        .unwrap_or_else(|| s.clone())
                })
                .collect()
        } else {
            items
                .iter()
                .enumerate()
                .map(|(i, it)| select_item_name(it, i))
                .collect()
        };
        if !star {
            let cols = items
                .iter()
                .map(|it| {
                    make_scalar(
                        bind_cols(&it.expr, &schema),
                        schema.clone(),
                        ctx.b.clone(),
                        col_aff.clone(),
                    )
                })
                .collect();
            plan = Plan::Project {
                input: Box::new(plan),
                cols,
            };
        }
        let off = match offset {
            Some(e) => eval(e, &[], &[], ctx)?.as_f64().unwrap_or(0.0) as usize,
            None => 0,
        };
        let lim = match limit {
            Some(e) => Some(eval(e, &[], &[], ctx)?.as_f64().unwrap_or(0.0) as usize),
            None => None,
        };
        plan = Plan::Limit {
            input: Box::new(plan),
            limit: lim,
            offset: off,
        };
        let rows_sql = plan.execute()?;
        let rows: Vec<Vec<Value>> = rows_sql
            .iter()
            .map(|r| r.iter().map(sql_to_val).collect())
            .collect();
        Ok(Outcome::Rows {
            columns: out_names,
            rows,
        })
    }

    fn exec_update(
        &mut self,
        table: &str,
        sets: &[(String, Expr)],
        where_: &Option<Expr>,
        returning: &Option<ReturningClause>,
        ctx: &mut ParamCtx,
    ) -> SqlResult<Outcome> {
        if let Some(t) = self.tbl_mut(table) {
            t.eq_indexes.clear();
        }
        let coldefs = self
            .tbl(table)
            .ok_or_else(|| format!("no such table: {table}"))?
            .columns
            .clone();
        let names: Vec<String> = coldefs.iter().map(|c| c.name.clone()).collect();

        let changed: Vec<String> = sets.iter().map(|(c, _)| c.clone()).collect();
        let mut snap = if self.has_triggers(table, "UPDATE") {
            Some((self.tables.clone(), self.attached.clone()))
        } else {
            None
        };

        let has_before_update = self.triggers.iter().any(|t| {
            t.timing.eq_ignore_ascii_case("BEFORE")
                && t.event.eq_ignore_ascii_case("UPDATE")
                && t.table.eq_ignore_ascii_case(table)
        });
        let set_idx: Vec<(usize, &Expr)> = sets
            .iter()
            .map(|(c, e)| {
                coldefs
                    .iter()
                    .position(|d| d.name.eq_ignore_ascii_case(c))
                    .map(|i| (i, e))
                    .ok_or_else(|| format!("no such column: {c}"))
            })
            .collect::<SqlResult<_>>()?;

        ctx.col_aff.clear();
        for cd in &coldefs {
            ctx.col_aff.insert(cd.name.clone(), cd.affinity);
        }
        let mut changes = 0i64;
        let mut fired: Vec<(Option<Vec<Value>>, Option<Vec<Value>>)> = Vec::new();
        let mut updated: Vec<Vec<Value>> = Vec::new();
        let want_returning = returning.is_some();

        let order: Vec<usize> = {
            let tr = self.tbl(table).unwrap();
            let mut o: Vec<usize> = (0..tr.rows.len()).collect();
            o.sort_by_key(|&i| tr.row_ids[i]);
            o
        };
        if has_before_update {

            for idx in order {
                let old = self.tbl(table).unwrap().rows[idx].clone();
                if let Some(e) = where_ {
                    if !eval(e, &old, &names, ctx)?.truthy() {
                        continue;
                    }
                }
                let mut newrow = old.clone();
                for (i, e) in &set_idx {
                    newrow[*i] = coerce(eval(e, &newrow, &names, ctx)?, coldefs[*i].affinity);
                }
                if coldefs.iter().any(|c| c.generated.is_some()) {
                    for i in 0..coldefs.len() {
                        if let Some(gen) = coldefs[i].generated.clone() {
                            newrow[i] =
                                coerce(eval(&gen, &newrow, &names, ctx)?, coldefs[i].affinity);
                        }
                    }
                }
                let ev = [(Some(old.clone()), Some(newrow.clone()))];
                match self.fire_triggers(table, "UPDATE", "BEFORE", &coldefs, &ev, &changed) {
                    Ok(f) => {
                        if f[0] {
                            continue;
                        }
                    }
                    Err(e) => {
                        if let Some((t, a)) = snap.take() {
                            self.tables = t;
                            self.attached = a;
                        }
                        return Err(e);
                    }
                }
                self.tbl_mut(table).unwrap().rows[idx] = newrow.clone();
                fired.push((Some(old), Some(newrow.clone())));
                if want_returning {
                    updated.push(newrow);
                }
                changes += 1;
            }
        } else {

            let rows = &mut self.tbl_mut(table).unwrap().rows;
            for idx in order {
                let row = &mut rows[idx];
                let matched = match where_ {
                    Some(e) => eval(e, row, &names, ctx)?.truthy(),
                    None => true,
                };
                if !matched {
                    continue;
                }
                let old = row.clone();
                for (i, e) in &set_idx {
                    let v = coerce(eval(e, row, &names, ctx)?, coldefs[*i].affinity);
                    row[*i] = v;
                }
                if coldefs.iter().any(|c| c.generated.is_some()) {
                    for i in 0..coldefs.len() {
                        if let Some(gen) = coldefs[i].generated.clone() {
                            let v = coerce(eval(&gen, row, &names, ctx)?, coldefs[i].affinity);
                            row[i] = v;
                        }
                    }
                }
                fired.push((Some(old), Some(row.clone())));
                if want_returning {
                    updated.push(row.clone());
                }
                changes += 1;
            }
        }
        self.changes = changes;
        self.total_changes += changes;

        if let Err(e) = self.fire_triggers(table, "UPDATE", "AFTER", &coldefs, &fired, &changed) {
            if let Some((t, a)) = snap.take() {
                self.tables = t;
                self.attached = a;
            }
            return Err(e);
        }
        self.maybe_persist()?;
        if let Some(rc) = returning {
            return project_returning(rc, &updated, &names, ctx);
        }
        Ok(Outcome::Mutation {
            changes,
            last_insert_rowid: self.last_insert_rowid,
        })
    }

    fn exec_delete(
        &mut self,
        table: &str,
        where_: &Option<Expr>,
        returning: &Option<ReturningClause>,
        ctx: &mut ParamCtx,
    ) -> SqlResult<Outcome> {
        if let Some(t) = self.tbl_mut(table) {
            t.eq_indexes.clear();
        }
        let coldefs = self
            .tbl(table)
            .ok_or_else(|| format!("no such table: {table}"))?
            .columns
            .clone();
        let names: Vec<String> = coldefs.iter().map(|c| c.name.clone()).collect();
        let mut snap = if self.has_triggers(table, "DELETE") {
            Some((self.tables.clone(), self.attached.clone()))
        } else {
            None
        };
        let has_before_delete = self.triggers.iter().any(|t| {
            t.timing.eq_ignore_ascii_case("BEFORE")
                && t.event.eq_ignore_ascii_case("DELETE")
                && t.table.eq_ignore_ascii_case(table)
        });

        ctx.col_aff.clear();
        for cd in &coldefs {
            ctx.col_aff.insert(cd.name.clone(), cd.affinity);
        }
        let tbl = self.tbl_mut(table).unwrap();
        let before = tbl.rows.len();
        let old_rows = std::mem::take(&mut tbl.rows);
        let old_ids = std::mem::take(&mut tbl.row_ids);
        let mut kept = Vec::with_capacity(before);
        let mut kept_ids = Vec::with_capacity(before);
        let mut deleted_pairs: Vec<(i64, Vec<Value>)> = Vec::new();
        for (row, rid) in old_rows.into_iter().zip(old_ids) {
            let matched = match where_ {
                Some(e) => eval(e, &row, &names, ctx)?.truthy(),
                None => true,
            };
            if !matched {
                kept.push(row);
                kept_ids.push(rid);
            } else {
                deleted_pairs.push((rid, row));
            }
        }

        deleted_pairs.sort_by_key(|(rid, _)| *rid);

        let deleted: Vec<Vec<Value>> = if has_before_delete {
            let mut final_deleted = Vec::new();
            for (rid, row) in deleted_pairs {
                let ev = [(Some(row.clone()), None)];
                match self.fire_triggers(table, "DELETE", "BEFORE", &coldefs, &ev, &[]) {
                    Ok(f) => {
                        if f[0] {
                            kept.push(row);
                            kept_ids.push(rid);
                            continue;
                        }
                    }
                    Err(e) => {
                        if let Some((t, a)) = snap.take() {
                            self.tables = t;
                            self.attached = a;
                        }
                        return Err(e);
                    }
                }
                final_deleted.push(row);
            }
            final_deleted
        } else {
            deleted_pairs.into_iter().map(|(_, r)| r).collect()
        };
        let tbl = self.tbl_mut(table).unwrap();
        tbl.rows = kept;
        tbl.row_ids = kept_ids;

        tbl.max_rowid = tbl.row_ids.iter().copied().max().unwrap_or(0);

        if self.foreign_keys_on && !deleted.is_empty() {
            self.cascade_deletes(table, &coldefs, &deleted, ctx)?;
        }
        let changes = (before - self.tbl(table).unwrap().rows.len()) as i64;
        self.changes = changes;
        self.total_changes += changes;

        if self.has_triggers(table, "DELETE") {
            let events: Vec<(Option<Vec<Value>>, Option<Vec<Value>>)> =
                deleted.iter().map(|r| (Some(r.clone()), None)).collect();
            if let Err(e) = self.fire_triggers(table, "DELETE", "AFTER", &coldefs, &events, &[]) {
                if let Some((t, a)) = snap.take() {
                    self.tables = t;
                    self.attached = a;
                }
                return Err(e);
            }
        }
        self.maybe_persist()?;
        if let Some(rc) = returning {
            return project_returning(rc, &deleted, &names, ctx);
        }
        Ok(Outcome::Mutation {
            changes,
            last_insert_rowid: self.last_insert_rowid,
        })
    }

    fn cascade_deletes(
        &mut self,
        parent_table: &str,
        parent_cols: &[ColumnDef],
        deleted: &[Vec<Value>],
        ctx: &mut ParamCtx,
    ) -> SqlResult<()> {
        use std::cmp::Ordering::Equal;
        let mut refs: Vec<(String, ForeignKey)> = Vec::new();
        for (tname, t) in self.tables.iter() {
            for fk in &t.foreign_keys {
                if fk.parent_table.eq_ignore_ascii_case(parent_table)
                    && matches!(
                        fk.on_delete,
                        FkAction::Cascade
                            | FkAction::SetNull
                            | FkAction::SetDefault
                            | FkAction::Restrict
                    )
                {
                    refs.push((tname.clone(), fk.clone()));
                }
            }
        }
        for (child_name, fk) in refs {
            let pidx = if fk.parent_col.is_empty() {
                parent_cols.iter().position(|c| c.pk)
            } else {
                parent_cols
                    .iter()
                    .position(|c| c.name.eq_ignore_ascii_case(&fk.parent_col))
            };
            let pidx = match pidx {
                Some(i) => i,
                None => continue,
            };
            let del_vals: Vec<Value> = deleted
                .iter()
                .filter_map(|r| r.get(pidx).cloned())
                .filter(|v| !matches!(v, Value::Null))
                .collect();
            if del_vals.is_empty() {
                continue;
            }
            let child_cols = match self.tbl(&child_name) {
                Some(t) => t.columns.clone(),
                None => continue,
            };
            let cidx = match child_cols
                .iter()
                .position(|c| c.name.eq_ignore_ascii_case(&fk.col))
            {
                Some(i) => i,
                None => continue,
            };
            let hits = |row: &[Value]| {
                row.get(cidx)
                    .is_some_and(|v| del_vals.iter().any(|d| d.compare(v) == Equal))
            };
            match fk.on_delete {
                FkAction::Cascade => {
                    let t = self.tbl_mut(&child_name).unwrap();
                    t.eq_indexes.clear();
                    let rows = std::mem::take(&mut t.rows);
                    let ids = std::mem::take(&mut t.row_ids);
                    let (mut kept, mut kept_ids, mut removed) =
                        (Vec::new(), Vec::new(), Vec::new());
                    for (r, id) in rows.into_iter().zip(ids) {
                        if hits(&r) {
                            removed.push(r);
                        } else {
                            kept.push(r);
                            kept_ids.push(id);
                        }
                    }
                    let t = self.tbl_mut(&child_name).unwrap();
                    t.rows = kept;
                    t.row_ids = kept_ids;
                    t.max_rowid = t.row_ids.iter().copied().max().unwrap_or(0);
                    if !removed.is_empty() {
                        self.cascade_deletes(&child_name, &child_cols, &removed, ctx)?;
                    }
                }
                FkAction::SetNull | FkAction::SetDefault => {
                    let t = self.tbl_mut(&child_name).unwrap();
                    t.eq_indexes.clear();
                    for r in t.rows.iter_mut() {
                        if hits(r) && cidx < r.len() {
                            r[cidx] = Value::Null;
                        }
                    }
                }
                FkAction::Restrict => {
                    if self.tbl(&child_name).unwrap().rows.iter().any(|r| hits(r)) {
                        return Err("FOREIGN KEY constraint failed".into());
                    }
                }
                FkAction::NoAction => {}
            }
        }
        Ok(())
    }
}

struct ParamCtx<'a> {
    b: &'a Bindings,
    next_pos: usize,

    db: Option<Database>,

    scopes: Vec<Scope>,

    cur_alias: Option<String>,

    col_aff: std::collections::HashMap<String, Affinity>,

    subq_cache: std::collections::HashMap<usize, std::rc::Rc<(Vec<String>, Vec<Vec<Value>>)>>,
}

#[derive(Clone)]
struct Scope {
    alias: Option<String>,
    cols: Vec<String>,
    row: Vec<Value>,
}

fn lookup_col(name: &str, row: &[Value], cols: &[String], ctx: &ParamCtx) -> Option<Value> {

    if let Some(i) = cols.iter().position(|c| c.eq_ignore_ascii_case(name)) {
        return row.get(i).cloned();
    }
    if let Some((qual, col)) = name.split_once('.') {

        for s in ctx.scopes.iter().rev() {
            if s.alias
                .as_deref()
                .map(|a| a.eq_ignore_ascii_case(qual))
                .unwrap_or(false)
            {
                if let Some(i) = s.cols.iter().position(|c| c.eq_ignore_ascii_case(col)) {
                    return s.row.get(i).cloned();
                }
            }
        }
        if let Some(i) = cols.iter().position(|c| c.eq_ignore_ascii_case(col)) {
            return row.get(i).cloned();
        }
        for s in ctx.scopes.iter().rev() {
            if let Some(i) = s.cols.iter().position(|c| c.eq_ignore_ascii_case(col)) {
                return s.row.get(i).cloned();
            }
        }
        return None;
    }

    for s in ctx.scopes.iter().rev() {
        if let Some(i) = s.cols.iter().position(|c| c.eq_ignore_ascii_case(name)) {
            return s.row.get(i).cloned();
        }
    }
    None
}

fn collect_select_colrefs(sub: &Stmt, on_col: &mut dyn FnMut(&str), on_nested: &mut dyn FnMut()) {
    let Stmt::Select {
        items,
        star: _,
        from: _,
        from_sub: _,
        from_alias: _,
        joins,
        where_,
        group_by,
        having,
        distinct: _,
        order_by,
        limit,
        offset,
    } = sub
    else {
        return;
    };
    fn walk(e: &Expr, on_col: &mut dyn FnMut(&str), on_nested: &mut dyn FnMut()) {
        match e {
            Expr::Col(n) => on_col(n),
            Expr::Subquery(_) | Expr::InSelect(_, _) | Expr::Exists(_) => on_nested(),
            Expr::Unary(_, a) | Expr::Distinct(a) | Expr::Collate(a, _) => {
                walk(a, on_col, on_nested)
            }
            Expr::Binary(_, a, b) => {
                walk(a, on_col, on_nested);
                walk(b, on_col, on_nested);
            }
            Expr::Func(_, args) => args.iter().for_each(|a| walk(a, on_col, on_nested)),
            Expr::AggFilter { args, filter, .. } => {
                args.iter().for_each(|a| walk(a, on_col, on_nested));
                walk(filter, on_col, on_nested);
            }
            Expr::AggOrder {
                args,
                order,
                filter,
                ..
            } => {
                args.iter().for_each(|a| walk(a, on_col, on_nested));
                order.iter().for_each(|(a, _)| walk(a, on_col, on_nested));
                if let Some(f) = filter {
                    walk(f, on_col, on_nested);
                }
            }
            Expr::Window {
                args,
                partition,
                order,
                ..
            } => {
                args.iter().for_each(|a| walk(a, on_col, on_nested));
                partition.iter().for_each(|a| walk(a, on_col, on_nested));
                order.iter().for_each(|(a, _)| walk(a, on_col, on_nested));
            }
            Expr::Case { operand, arms, els } => {
                if let Some(o) = operand {
                    walk(o, on_col, on_nested);
                }
                for (c, r) in arms {
                    walk(c, on_col, on_nested);
                    walk(r, on_col, on_nested);
                }
                if let Some(x) = els {
                    walk(x, on_col, on_nested);
                }
            }
            _ => {}
        }
    }
    for it in items {
        walk(&it.expr, on_col, on_nested);
    }
    if let Some(w) = where_ {
        walk(w, on_col, on_nested);
    }
    if let Some(h) = having {
        walk(h, on_col, on_nested);
    }
    for g in group_by {
        walk(g, on_col, on_nested);
    }
    for (e, _) in order_by {
        walk(e, on_col, on_nested);
    }
    for j in joins {
        if let Some(on) = &j.on {
            walk(on, on_col, on_nested);
        }
    }
    if let Some(l) = limit {
        walk(l, on_col, on_nested);
    }
    if let Some(o) = offset {
        walk(o, on_col, on_nested);
    }
}

fn run_subquery(
    sub: &Stmt,
    row: &[Value],
    cols: &[String],
    ctx: &mut ParamCtx,
) -> SqlResult<(Vec<String>, Vec<Vec<Value>>)> {
    let mut db = ctx
        .db
        .clone()
        .ok_or("subquery requires a database context")?;
    let key = sub as *const Stmt as usize;

    if let Some(cached) = ctx.subq_cache.get(&key) {
        return Ok(((**cached).0.clone(), (**cached).1.clone()));
    }
    let hoistable = db.subquery_hoistable(sub);
    ctx.scopes.push(Scope {
        alias: ctx.cur_alias.clone(),
        cols: cols.to_vec(),
        row: row.to_vec(),
    });
    let saved_alias = ctx.cur_alias.clone();
    let saved_pos = ctx.next_pos;
    let res = db.exec_select(sub, ctx);
    ctx.next_pos = saved_pos;
    ctx.cur_alias = saved_alias;
    ctx.scopes.pop();
    match res? {
        Outcome::Rows { columns, rows } => {
            if hoistable {
                ctx.subq_cache
                    .insert(key, std::rc::Rc::new((columns.clone(), rows.clone())));
            }
            Ok((columns, rows))
        }
        _ => Err("subquery is not a SELECT".into()),
    }
}

impl ParamCtx<'_> {
    fn resolve(&mut self, p: &ParamRef) -> SqlResult<Value> {
        match p {
            ParamRef::Pos(Some(n)) => self
                .b
                .positional
                .get(n - 1)
                .cloned()
                .ok_or_else(|| format!("missing bound parameter ?{n}")),
            ParamRef::Pos(None) => {
                let idx = self.next_pos;
                self.next_pos += 1;
                self.b
                    .positional
                    .get(idx)
                    .cloned()
                    .ok_or_else(|| "missing positional parameter".to_string())
            }
            ParamRef::Name(name) => self
                .b
                .named
                .get(name)
                .cloned()
                .ok_or_else(|| format!("missing bound parameter :{name}")),
        }
    }
}

fn expr_refs_rowid(e: &Expr) -> bool {
    match e {
        Expr::Collate(inner, _) => expr_refs_rowid(inner),
        Expr::AggFilter { args, filter, .. } => {
            args.iter().any(expr_refs_rowid) || expr_refs_rowid(filter)
        }
        Expr::AggOrder {
            args,
            order,
            filter,
            ..
        } => {
            args.iter().any(expr_refs_rowid)
                || order.iter().any(|(k, _)| expr_refs_rowid(k))
                || filter.as_deref().is_some_and(expr_refs_rowid)
        }
        Expr::Window {
            args,
            partition,
            order,
            ..
        } => {
            args.iter().any(expr_refs_rowid)
                || partition.iter().any(expr_refs_rowid)
                || order.iter().any(|(k, _)| expr_refs_rowid(k))
        }
        Expr::Col(name) => {
            let seg = name.rsplit('.').next().unwrap_or(name);
            matches!(
                seg.to_ascii_lowercase().as_str(),
                "rowid" | "_rowid_" | "oid"
            )
        }
        Expr::Lit(_) | Expr::Param(_) | Expr::Star => false,
        Expr::Unary(_, x) | Expr::Distinct(x) => expr_refs_rowid(x),
        Expr::Binary(_, a, b) => expr_refs_rowid(a) || expr_refs_rowid(b),
        Expr::Func(_, args) => args.iter().any(expr_refs_rowid),
        Expr::Case { operand, arms, els } => {
            operand.as_deref().is_some_and(expr_refs_rowid)
                || arms
                    .iter()
                    .any(|(w, t)| expr_refs_rowid(w) || expr_refs_rowid(t))
                || els.as_deref().is_some_and(expr_refs_rowid)
        }

        Expr::Subquery(_) | Expr::InSelect(_, _) | Expr::Exists(_) => true,
    }
}

fn find_indexable_eq(
    where_: &Option<Expr>,
    coldefs: &[ColumnDef],
    ctx: &mut ParamCtx,
) -> SqlResult<Option<(usize, Value)>> {
    let Some(root) = where_ else { return Ok(None) };
    let mut stack = vec![root];
    while let Some(cur) = stack.pop() {
        match cur {
            Expr::Binary(op, a, b) if op == "AND" => {
                stack.push(a);
                stack.push(b);
            }
            Expr::Binary(op, a, b) if op == "=" || op == "==" => {
                for (col_side, val_side) in [(a.as_ref(), b.as_ref()), (b.as_ref(), a.as_ref())] {
                    if let (Expr::Col(name), Expr::Lit(_) | Expr::Param(_)) = (col_side, val_side) {
                        let seg = name.rsplit('.').next().unwrap_or(name);
                        if let Some(ci) = coldefs
                            .iter()
                            .position(|c| c.name.eq_ignore_ascii_case(seg))
                        {
                            let v = eval(val_side, &[], &[], ctx)?;
                            if !matches!(v, Value::Null) {
                                return Ok(Some((ci, v)));
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
    Ok(None)
}

struct WinSpec {
    func: String,
    args: Vec<Expr>,
    partition: Vec<Expr>,
    order: Vec<(Expr, bool)>,
    frame: Option<Frame>,
    filter: Option<Box<Expr>>,
}

fn expr_has_window(e: &Expr) -> bool {
    match e {
        Expr::Window { .. } => true,
        Expr::Unary(_, a) | Expr::Distinct(a) | Expr::Collate(a, _) => expr_has_window(a),
        Expr::Binary(_, a, b) => expr_has_window(a) || expr_has_window(b),
        Expr::Func(_, args) => args.iter().any(expr_has_window),
        Expr::Case { operand, arms, els } => {
            operand.as_deref().is_some_and(expr_has_window)
                || arms
                    .iter()
                    .any(|(c, r)| expr_has_window(c) || expr_has_window(r))
                || els.as_deref().is_some_and(expr_has_window)
        }
        _ => false,
    }
}

fn expr_has_col(e: &Expr) -> bool {
    match e {
        Expr::Col(_) => true,
        Expr::Unary(_, a) | Expr::Distinct(a) | Expr::Collate(a, _) => expr_has_col(a),
        Expr::Binary(_, a, b) => expr_has_col(a) || expr_has_col(b),
        Expr::Func(_, args) => args.iter().any(expr_has_col),
        Expr::Case { operand, arms, els } => {
            operand.as_deref().is_some_and(expr_has_col)
                || arms.iter().any(|(c, r)| expr_has_col(c) || expr_has_col(r))
                || els.as_deref().is_some_and(expr_has_col)
        }
        _ => false,
    }
}

fn first_ambiguous_col<'a>(
    e: &'a Expr,
    schema: &[String],
    using_cols: &std::collections::HashSet<String>,
) -> Option<String> {
    match e {
        Expr::Col(name) => {

            if name.contains('.') || using_cols.contains(&name.to_ascii_lowercase()) {
                return None;
            }
            let mut count = 0;
            for s in schema {
                let tail = s.rsplit_once('.').map(|(_, c)| c).unwrap_or(s.as_str());
                if tail.eq_ignore_ascii_case(name) {
                    count += 1;
                }
            }
            (count > 1).then(|| name.clone())
        }
        Expr::Unary(_, a) | Expr::Distinct(a) | Expr::Collate(a, _) => {
            first_ambiguous_col(a, schema, using_cols)
        }
        Expr::Binary(_, a, b) => first_ambiguous_col(a, schema, using_cols)
            .or_else(|| first_ambiguous_col(b, schema, using_cols)),
        Expr::Func(_, args) => args
            .iter()
            .find_map(|a| first_ambiguous_col(a, schema, using_cols)),
        Expr::Case { operand, arms, els } => operand
            .as_deref()
            .and_then(|o| first_ambiguous_col(o, schema, using_cols))
            .or_else(|| {
                arms.iter().find_map(|(c, r)| {
                    first_ambiguous_col(c, schema, using_cols)
                        .or_else(|| first_ambiguous_col(r, schema, using_cols))
                })
            })
            .or_else(|| {
                els.as_deref()
                    .and_then(|x| first_ambiguous_col(x, schema, using_cols))
            }),
        _ => None,
    }
}

fn validate_cursor_expr(e: &Expr, names: &[String]) -> SqlResult<()> {
    match e {
        Expr::Col(name) => {
            if names.iter().any(|n| n.eq_ignore_ascii_case(name)) {
                Ok(())
            } else {
                Err(format!("no such column: {name}"))
            }
        }
        Expr::Lit(_) | Expr::Param(_) | Expr::Star => Ok(()),
        Expr::Unary(_, x) | Expr::Distinct(x) | Expr::Collate(x, _) => {
            validate_cursor_expr(x, names)
        }
        Expr::Binary(_, l, r) => {
            validate_cursor_expr(l, names)?;
            validate_cursor_expr(r, names)
        }
        Expr::Func(name, args) if !is_agg_name(name) => {
            for arg in args {
                validate_cursor_expr(arg, names)?;
            }
            Ok(())
        }
        Expr::Case { operand, arms, els } => {
            if let Some(o) = operand {
                validate_cursor_expr(o, names)?;
            }
            for (c, r) in arms {
                validate_cursor_expr(c, names)?;
                validate_cursor_expr(r, names)?;
            }
            if let Some(e) = els {
                validate_cursor_expr(e, names)?;
            }
            Ok(())
        }
        Expr::Subquery(_) | Expr::InSelect(_, _) | Expr::Exists(_) => {
            Err("cursor does not yet support subquery expressions".into())
        }
        Expr::AggFilter { .. } | Expr::AggOrder { .. } | Expr::Window { .. } | Expr::Func(_, _) => {
            Err("cursor does not yet support aggregate/window expressions".into())
        }
    }
}

fn same_aff_class(a: Option<Affinity>, b: Option<Affinity>) -> bool {
    let class = |x: Option<Affinity>| match x {
        Some(Affinity::Integer) | Some(Affinity::Real) | Some(Affinity::Numeric) => 0u8,
        Some(Affinity::Text) => 1,
        Some(Affinity::Blob) | None => 2,
    };
    class(a) == class(b)
}

fn extract_equi(
    on: &Expr,
    combined: &[String],
    left_width: usize,
    col_aff: &std::collections::HashMap<String, Affinity>,
) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    fn walk(
        on: &Expr,
        combined: &[String],
        left_width: usize,
        col_aff: &std::collections::HashMap<String, Affinity>,
        out: &mut Vec<(usize, usize)>,
    ) {
        match on {
            Expr::Binary(op, l, r) if op == "=" || op == "==" => {
                let (Expr::Col(a), Expr::Col(b)) = (l.as_ref(), r.as_ref()) else {
                    return;
                };
                let na = resolve_qualified(a, combined);
                let nb = resolve_qualified(b, combined);
                let (Some(pa), Some(pb)) = (
                    combined.iter().position(|s| s.eq_ignore_ascii_case(&na)),
                    combined.iter().position(|s| s.eq_ignore_ascii_case(&nb)),
                ) else {
                    return;
                };
                let (lp, rp) = if pa < left_width && pb >= left_width {
                    (pa, pb)
                } else if pb < left_width && pa >= left_width {
                    (pb, pa)
                } else {
                    return;
                };
                if same_aff_class(
                    col_aff.get(&combined[lp]).copied(),
                    col_aff.get(&combined[rp]).copied(),
                ) {
                    out.push((lp, rp - left_width));
                }
            }
            Expr::Binary(op, l, r) if op == "AND" => {
                walk(l, combined, left_width, col_aff, out);
                walk(r, combined, left_width, col_aff, out);
            }
            _ => {}
        }
    }
    walk(on, combined, left_width, col_aff, &mut out);
    out
}

fn extract_windows(e: &Expr, specs: &mut Vec<WinSpec>) -> Expr {
    match e {
        Expr::Window {
            func,
            args,
            partition,
            order,
            frame,
            filter,
            ..
        } => {
            let idx = specs.len();
            specs.push(WinSpec {
                func: func.clone(),
                args: args.clone(),
                partition: partition.clone(),
                order: order.clone(),
                frame: frame.clone(),
                filter: filter.clone(),
            });
            Expr::Col(format!("__win{idx}"))
        }
        Expr::Unary(op, a) => Expr::Unary(op.clone(), Box::new(extract_windows(a, specs))),
        Expr::Binary(op, a, b) => Expr::Binary(
            op.clone(),
            Box::new(extract_windows(a, specs)),
            Box::new(extract_windows(b, specs)),
        ),
        Expr::Func(n, args) => Expr::Func(
            n.clone(),
            args.iter().map(|a| extract_windows(a, specs)).collect(),
        ),
        Expr::Distinct(a) => Expr::Distinct(Box::new(extract_windows(a, specs))),
        Expr::Case { operand, arms, els } => Expr::Case {
            operand: operand
                .as_ref()
                .map(|o| Box::new(extract_windows(o, specs))),
            arms: arms
                .iter()
                .map(|(c, r)| (extract_windows(c, specs), extract_windows(r, specs)))
                .collect(),
            els: els.as_ref().map(|o| Box::new(extract_windows(o, specs))),
        },
        other => other.clone(),
    }
}

fn resolve_window_refs(e: &Expr, defs: &[(String, Expr)]) -> Expr {
    match e {
        Expr::Window {
            func,
            args,
            filter,
            window_ref: Some(name),
            ..
        } => {
            if let Some((
                _,
                Expr::Window {
                    partition,
                    order,
                    frame,
                    ..
                },
            )) = defs.iter().find(|(n, _)| n.eq_ignore_ascii_case(name))
            {
                Expr::Window {
                    func: func.clone(),
                    args: args.clone(),
                    partition: partition.clone(),
                    order: order.clone(),
                    frame: frame.clone(),
                    filter: filter.clone(),
                    window_ref: None,
                }
            } else {
                e.clone()
            }
        }
        Expr::Unary(op, a) => Expr::Unary(op.clone(), Box::new(resolve_window_refs(a, defs))),
        Expr::Binary(op, a, b) => Expr::Binary(
            op.clone(),
            Box::new(resolve_window_refs(a, defs)),
            Box::new(resolve_window_refs(b, defs)),
        ),
        Expr::Func(n, args) => Expr::Func(
            n.clone(),
            args.iter().map(|a| resolve_window_refs(a, defs)).collect(),
        ),
        Expr::Distinct(a) => Expr::Distinct(Box::new(resolve_window_refs(a, defs))),
        Expr::Case { operand, arms, els } => Expr::Case {
            operand: operand
                .as_ref()
                .map(|o| Box::new(resolve_window_refs(o, defs))),
            arms: arms
                .iter()
                .map(|(c, r)| (resolve_window_refs(c, defs), resolve_window_refs(r, defs)))
                .collect(),
            els: els.as_ref().map(|o| Box::new(resolve_window_refs(o, defs))),
        },
        other => other.clone(),
    }
}

fn select_item_name(it: &SelectItem, idx: usize) -> String {
    if let Some(a) = &it.alias {
        return a.clone();
    }
    if let Expr::Col(c) = &it.expr {
        return c
            .rsplit_once('.')
            .map(|(_, x)| x.to_string())
            .unwrap_or_else(|| c.clone());
    }
    match &it.source {
        Some(s) if !s.is_empty() => s.clone(),
        _ => format!("column{}", idx + 1),
    }
}

fn is_agg_name(name: &str) -> bool {
    matches!(
        name,
        "COUNT"
            | "SUM"
            | "AVG"
            | "MIN"
            | "MAX"
            | "TOTAL"
            | "GROUP_CONCAT"
            | "JSON_GROUP_ARRAY"
            | "JSON_GROUP_OBJECT"
    )
}
fn expr_is_aggregate(e: &Expr) -> bool {
    match e {
        Expr::Func(name, _) => is_agg_name(name),
        Expr::AggFilter { .. } | Expr::AggOrder { .. } => true,
        _ => false,
    }
}

fn expr_contains_aggregate(e: &Expr) -> bool {
    match e {
        Expr::Func(name, args) => is_agg_name(name) || args.iter().any(expr_contains_aggregate),
        Expr::AggFilter { .. } | Expr::AggOrder { .. } => true,
        Expr::Window { .. } => false,
        Expr::Binary(_, a, b) => expr_contains_aggregate(a) || expr_contains_aggregate(b),
        Expr::Unary(_, a) | Expr::Distinct(a) | Expr::Collate(a, _) => expr_contains_aggregate(a),
        Expr::Case { operand, arms, els } => {
            operand.as_deref().is_some_and(expr_contains_aggregate)
                || arms
                    .iter()
                    .any(|(c, r)| expr_contains_aggregate(c) || expr_contains_aggregate(r))
                || els.as_deref().is_some_and(expr_contains_aggregate)
        }
        _ => false,
    }
}

fn fk_action_text(a: FkAction) -> &'static str {
    match a {
        FkAction::NoAction => "NO ACTION",
        FkAction::Restrict => "RESTRICT",
        FkAction::Cascade => "CASCADE",
        FkAction::SetNull => "SET NULL",
        FkAction::SetDefault => "SET DEFAULT",
    }
}

fn affinity_type_name(a: Affinity) -> &'static str {
    match a {
        Affinity::Integer => "INTEGER",
        Affinity::Real => "REAL",
        Affinity::Text => "TEXT",
        Affinity::Blob => "BLOB",
        Affinity::Numeric => "NUMERIC",
    }
}

fn render_default(e: &Expr) -> String {
    match e {
        Expr::Lit(Value::Int(i)) => i.to_string(),
        Expr::Lit(Value::Real(r)) => r.to_string(),
        Expr::Lit(Value::Text(s)) => format!("'{s}'"),
        Expr::Lit(Value::Null) => "NULL".to_string(),
        Expr::Unary(op, inner) => format!("{op}{}", render_default(inner)),
        _ => String::new(),
    }
}

fn subst_stmt(stmt: &Stmt, names: &[String], old: Option<&[Value]>, new: Option<&[Value]>) -> Stmt {
    let f = |e: &Expr| subst_expr(e, names, old, new);
    match stmt {
        Stmt::Insert {
            table,
            columns,
            rows,
            or_action,
            on_conflict,
            returning,
        } => Stmt::Insert {
            table: table.clone(),
            columns: columns.clone(),
            rows: rows.iter().map(|r| r.iter().map(&f).collect()).collect(),
            or_action: or_action.clone(),
            on_conflict: on_conflict.clone(),
            returning: returning.clone(),
        },
        Stmt::Update {
            table,
            sets,
            where_,
            returning,
        } => Stmt::Update {
            table: table.clone(),
            sets: sets.iter().map(|(c, e)| (c.clone(), f(e))).collect(),
            where_: where_.as_ref().map(&f),
            returning: returning.clone(),
        },
        Stmt::Delete {
            table,
            where_,
            returning,
        } => Stmt::Delete {
            table: table.clone(),
            where_: where_.as_ref().map(&f),
            returning: returning.clone(),
        },
        other => other.clone(),
    }
}

fn subst_expr(e: &Expr, names: &[String], old: Option<&[Value]>, new: Option<&[Value]>) -> Expr {
    match e {
        Expr::Col(name) => {
            if let Some((qual, col)) = name.split_once('.') {
                let src = if qual.eq_ignore_ascii_case("NEW") {
                    new
                } else if qual.eq_ignore_ascii_case("OLD") {
                    old
                } else {
                    None
                };
                if let Some(vals) = src {
                    if let Some(i) = names.iter().position(|c| c.eq_ignore_ascii_case(col)) {
                        if let Some(v) = vals.get(i) {
                            return Expr::Lit(v.clone());
                        }
                    }
                    return Expr::Lit(Value::Null);
                }
            }
            e.clone()
        }
        Expr::Unary(op, inner) => {
            Expr::Unary(op.clone(), Box::new(subst_expr(inner, names, old, new)))
        }
        Expr::Binary(op, l, r) => Expr::Binary(
            op.clone(),
            Box::new(subst_expr(l, names, old, new)),
            Box::new(subst_expr(r, names, old, new)),
        ),
        Expr::Func(n, args) => Expr::Func(
            n.clone(),
            args.iter()
                .map(|a| subst_expr(a, names, old, new))
                .collect(),
        ),
        Expr::Case { operand, arms, els } => Expr::Case {
            operand: operand
                .as_ref()
                .map(|o| Box::new(subst_expr(o, names, old, new))),
            arms: arms
                .iter()
                .map(|(c, r)| {
                    (
                        subst_expr(c, names, old, new),
                        subst_expr(r, names, old, new),
                    )
                })
                .collect(),
            els: els
                .as_ref()
                .map(|o| Box::new(subst_expr(o, names, old, new))),
        },
        _ => e.clone(),
    }
}

fn eval(e: &Expr, row: &[Value], cols: &[String], ctx: &mut ParamCtx) -> SqlResult<Value> {
    match e {

        Expr::AggFilter { .. } | Expr::AggOrder { .. } => {
            Err("misuse of aggregate function".into())
        }

        Expr::Window { .. } => Err("misuse of window function".into()),
        Expr::Lit(v) => Ok(v.clone()),
        Expr::Star => Ok(Value::Int(1)),
        Expr::Param(p) => ctx.resolve(p),
        Expr::Col(name) => {
            lookup_col(name, row, cols, ctx).ok_or_else(|| format!("no such column: {name}"))
        }
        Expr::Unary(op, inner) => {
            let v = eval(inner, row, cols, ctx)?;
            match op.as_str() {
                "-" => Ok(match v {
                    Value::Int(i) => Value::Int(-i),
                    Value::Real(r) => Value::Real(-r),
                    Value::Null => Value::Null,

                    Value::Text(s) => match text_to_num(&s) {
                        Value::Int(i) => Value::Int(-i),
                        other => Value::Real(-other.as_f64().unwrap_or(0.0)),
                    },
                    o => o.as_f64().map(|f| Value::Real(-f)).unwrap_or(Value::Null),
                }),
                "NOT" => Ok(match v {
                    Value::Null => Value::Null,
                    o => Value::Int(if o.truthy() { 0 } else { 1 }),
                }),
                _ => Err(format!("bad unary op {op}")),
            }
        }
        Expr::Binary(op, l, r) => {
            let a = eval(l, row, cols, ctx)?;
            let b = eval(r, row, cols, ctx)?;

            if matches!(
                op.as_str(),
                "=" | "==" | "!=" | "<>" | "<" | "<=" | ">" | ">="
            ) {
                let (a, b) = apply_cmp_affinity(a, expr_affinity(l, ctx), b, expr_affinity(r, ctx));
                match expr_collation(l).or_else(|| expr_collation(r)) {
                    Some(coll) => {
                        if matches!(a, Value::Null) || matches!(b, Value::Null) {
                            Ok(Value::Null)
                        } else {
                            Ok(Value::Int(
                                cmp_to_bool(op, compare_coll(&a, &b, coll)) as i64
                            ))
                        }
                    }
                    None => eval_binary(op, a, b),
                }
            } else {
                eval_binary(op, a, b)
            }
        }
        Expr::Collate(inner, _) => eval(inner, row, cols, ctx),
        Expr::Func(name, args) => eval_scalar_func(name, args, row, cols, ctx),

        Expr::Distinct(inner) => eval(inner, row, cols, ctx),

        Expr::Subquery(sub) => {
            let (_cols, rows) = run_subquery(sub, row, cols, ctx)?;
            Ok(rows
                .into_iter()
                .next()
                .and_then(|r| r.into_iter().next())
                .unwrap_or(Value::Null))
        }

        Expr::InSelect(lhs, sub) => {
            let lv = eval(lhs, row, cols, ctx)?;
            let (_cols, rows) = run_subquery(sub, row, cols, ctx)?;
            if matches!(lv, Value::Null) {
                return Ok(if rows.is_empty() {
                    Value::Int(0)
                } else {
                    Value::Null
                });
            }
            let mut saw_null = false;
            for r in &rows {
                match r.first() {
                    Some(Value::Null) | None => saw_null = true,
                    Some(v) => {
                        if v.compare(&lv) == std::cmp::Ordering::Equal {
                            return Ok(Value::Int(1));
                        }
                    }
                }
            }
            Ok(if saw_null { Value::Null } else { Value::Int(0) })
        }

        Expr::Exists(sub) => {
            let (_cols, rows) = run_subquery(sub, row, cols, ctx)?;
            Ok(Value::Int(if rows.is_empty() { 0 } else { 1 }))
        }
        Expr::Case { operand, arms, els } => {
            let op_val = match operand {
                Some(o) => Some(eval(o, row, cols, ctx)?),
                None => None,
            };
            for (cond, res) in arms {
                let matched = match &op_val {

                    Some(ov) => {
                        let cv = eval(cond, row, cols, ctx)?;
                        !matches!(ov, Value::Null)
                            && !matches!(cv, Value::Null)
                            && ov.compare(&cv) == std::cmp::Ordering::Equal
                    }

                    None => eval(cond, row, cols, ctx)?.truthy(),
                };
                if matched {
                    return eval(res, row, cols, ctx);
                }
            }
            match els {
                Some(e) => eval(e, row, cols, ctx),
                None => Ok(Value::Null),
            }
        }
    }
}

fn expr_affinity(e: &Expr, ctx: &ParamCtx) -> Option<Affinity> {
    match e {
        Expr::Col(name) => ctx.col_aff.get(name).copied().or_else(|| {
            let bare = name.rsplit('.').next().unwrap_or(name);
            ctx.col_aff.get(bare).copied()
        }),
        Expr::Func(f, args) if f.eq_ignore_ascii_case("CAST") && args.len() == 2 => {
            if let Expr::Lit(Value::Text(ty)) = &args[1] {
                Some(affinity_from_type(ty))
            } else {
                None
            }
        }
        _ => None,
    }
}

fn affinity_from_type(ty: &str) -> Affinity {
    let u = ty.to_ascii_uppercase();
    if u.contains("INT") {
        Affinity::Integer
    } else if u.contains("CHAR") || u.contains("CLOB") || u.contains("TEXT") {
        Affinity::Text
    } else if u.contains("BLOB") || u.is_empty() {
        Affinity::Blob
    } else if u.contains("REAL") || u.contains("FLOA") || u.contains("DOUB") {
        Affinity::Real
    } else {
        Affinity::Numeric
    }
}

fn is_int_operand(v: &Value) -> bool {
    match v {
        Value::Int(_) => true,
        Value::Text(s) => matches!(text_to_num(s), Value::Int(_)),
        _ => false,
    }
}
fn text_to_num(s: &str) -> Value {
    let f = numeric_prefix(s);
    let t = s.trim_start();
    let b = t.as_bytes();
    let mut i = 0;
    if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
        i += 1;
    }
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
    }
    let has_dot = i < b.len() && b[i] == b'.';
    if !has_dot && f.is_finite() && f == f.trunc() && f.abs() < 9.223e18 {
        Value::Int(f as i64)
    } else {
        Value::Real(f)
    }
}

fn numeric_prefix(s: &str) -> f64 {
    let t = s.trim_start();
    let b = t.as_bytes();
    let mut i = 0;
    if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
        i += 1;
    }
    let mut seen = false;
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
        seen = true;
    }
    if i < b.len() && b[i] == b'.' {
        i += 1;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
            seen = true;
        }
    }
    if !seen {
        return 0.0;
    }
    if i < b.len() && (b[i] == b'e' || b[i] == b'E') {
        let save = i;
        i += 1;
        if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
            i += 1;
        }
        let ed = i;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        if i == ed {
            i = save;
        }
    }
    t[..i].parse::<f64>().unwrap_or(0.0)
}

fn num_coerce(v: &Value) -> f64 {
    match v {
        Value::Int(i) => *i as f64,
        Value::Real(r) => *r,
        Value::Text(s) => numeric_prefix(s),
        _ => 0.0,
    }
}

fn is_numeric_aff(a: Option<Affinity>) -> bool {
    matches!(
        a,
        Some(Affinity::Integer | Affinity::Real | Affinity::Numeric)
    )
}

fn to_numeric_aff(v: Value) -> Value {
    if let Value::Text(ref s) = v {
        let t = s.trim();
        if let Ok(i) = t.parse::<i64>() {
            return Value::Int(i);
        }
        if let Ok(f) = t.parse::<f64>() {
            if f.is_finite() {
                return Value::Real(f);
            }
        }
    }
    v
}
fn to_text_aff(v: Value) -> Value {
    match v {
        Value::Int(_) | Value::Real(_) => Value::Text(text_of(&v)),
        other => other,
    }
}

fn apply_cmp_affinity(
    a: Value,
    la: Option<Affinity>,
    b: Value,
    ra: Option<Affinity>,
) -> (Value, Value) {
    if matches!(a, Value::Null) || matches!(b, Value::Null) {
        return (a, b);
    }
    let text = |o: Option<Affinity>| matches!(o, Some(Affinity::Text));
    if is_numeric_aff(la) && (text(ra) || ra.is_none()) {
        return (a, to_numeric_aff(b));
    }
    if is_numeric_aff(ra) && (text(la) || la.is_none()) {
        return (to_numeric_aff(a), b);
    }
    if text(la) && ra.is_none() {
        return (a, to_text_aff(b));
    }
    if text(ra) && la.is_none() {
        return (to_text_aff(a), b);
    }
    (a, b)
}

fn expr_collation(e: &Expr) -> Option<Collation> {
    match e {
        Expr::Collate(_, name) => Some(collation_from_name(name)),
        _ => None,
    }
}
fn cmp_to_bool(op: &str, ord: std::cmp::Ordering) -> bool {
    use std::cmp::Ordering::{Equal, Greater, Less};
    match op {
        "=" | "==" => ord == Equal,
        "!=" | "<>" => ord != Equal,
        "<" => ord == Less,
        "<=" => ord != Greater,
        ">" => ord == Greater,
        ">=" => ord != Less,
        _ => false,
    }
}

fn is_eq(a: &Value, b: &Value) -> bool {
    match (matches!(a, Value::Null), matches!(b, Value::Null)) {
        (true, true) => true,
        (true, false) | (false, true) => false,
        (false, false) => a.compare(b) == std::cmp::Ordering::Equal,
    }
}

fn project_returning(
    rc: &ReturningClause,
    rows: &[Vec<Value>],
    names: &[String],
    ctx: &mut ParamCtx,
) -> SqlResult<Outcome> {
    let out_cols: Vec<String> = if rc.star {
        names.to_vec()
    } else {
        rc.items
            .iter()
            .enumerate()
            .map(|(i, it)| select_item_name(it, i))
            .collect()
    };
    let mut out_rows = Vec::new();
    for r in rows {
        if rc.star {
            out_rows.push(r.clone());
        } else {
            let mut orow = Vec::new();
            for it in &rc.items {
                orow.push(eval(&it.expr, r, names, ctx)?);
            }
            out_rows.push(orow);
        }
    }
    Ok(Outcome::Rows {
        columns: out_cols,
        rows: out_rows,
    })
}

fn eval_binary(op: &str, a: Value, b: Value) -> SqlResult<Value> {
    let null = matches!(a, Value::Null) || matches!(b, Value::Null);
    match op {

        "AND" => Ok({

            let fa = !matches!(a, Value::Null) && !a.truthy();
            let fb = !matches!(b, Value::Null) && !b.truthy();
            if fa || fb {
                Value::Int(0)
            } else if null {
                Value::Null
            } else {
                Value::Int(1)
            }
        }),
        "OR" => Ok({

            let ta = !matches!(a, Value::Null) && a.truthy();
            let tb = !matches!(b, Value::Null) && b.truthy();
            if ta || tb {
                Value::Int(1)
            } else if null {
                Value::Null
            } else {
                Value::Int(0)
            }
        }),

        "IS" => Ok(Value::Int(is_eq(&a, &b) as i64)),
        "ISNOT" => Ok(Value::Int(!is_eq(&a, &b) as i64)),
        "=" | "==" | "!=" | "<>" | "<" | "<=" | ">" | ">=" => {
            if null {
                return Ok(Value::Null);
            }
            let ord = a.compare(&b);
            use std::cmp::Ordering::*;
            let res = match op {
                "=" | "==" => ord == Equal,
                "!=" | "<>" => ord != Equal,
                "<" => ord == Less,
                "<=" => ord != Greater,
                ">" => ord == Greater,
                ">=" => ord != Less,
                _ => unreachable!(),
            };
            Ok(Value::Int(res as i64))
        }
        "LIKE" => {
            if null {
                return Ok(Value::Null);
            }
            let (s, pat) = (text_of(&a), text_of(&b));
            Ok(Value::Int(like_match(&pat, &s) as i64))
        }
        "GLOB" => {
            if null {
                return Ok(Value::Null);
            }
            let (s, pat) = (text_of(&a), text_of(&b));
            Ok(Value::Int(glob_match(&pat, &s) as i64))
        }
        "||" => {
            if null {
                return Ok(Value::Null);
            }
            Ok(Value::Text(format!("{}{}", text_of(&a), text_of(&b))))
        }
        "+" | "-" | "*" | "/" | "%" => {
            if null {
                return Ok(Value::Null);
            }
            let (x, y) = (num_coerce(&a), num_coerce(&b));

            let both_int = is_int_operand(&a) && is_int_operand(&b);
            let res = match op {
                "+" => x + y,
                "-" => x - y,
                "*" => x * y,
                "/" => {
                    if y == 0.0 {
                        return Ok(Value::Null);
                    }
                    if both_int {
                        return Ok(Value::Int((x as i64) / (y as i64)));
                    }
                    x / y
                }
                "%" => {

                    let (xi, yi) = (x as i64, y as i64);
                    if yi == 0 {
                        return Ok(Value::Null);
                    }
                    let m = xi % yi;
                    return Ok(if both_int {
                        Value::Int(m)
                    } else {
                        Value::Real(m as f64)
                    });
                }
                _ => unreachable!(),
            };
            if both_int && op != "/" {
                Ok(Value::Int(res as i64))
            } else {
                Ok(Value::Real(res))
            }
        }

        "->" | "->>" => functions::dispatch(op, &[a, b])
            .unwrap_or_else(|| Err(format!("no such operator: {op}"))),
        _ => Err(format!("bad binary op {op}")),
    }
}

const RAISE_IGNORE: &str = "\u{0}__cruft_raise_ignore__";

fn eval_scalar_func(
    name: &str,
    args: &[Expr],
    row: &[Value],
    cols: &[String],
    ctx: &mut ParamCtx,
) -> SqlResult<Value> {

    if name.eq_ignore_ascii_case("RAISE") {
        let action = match args.first() {
            Some(Expr::Lit(Value::Text(a))) => a.clone(),
            _ => String::new(),
        };
        if action == "IGNORE" {
            return Err(RAISE_IGNORE.to_string());
        }
        let msg = match args.get(1) {
            Some(e) => text_of(&eval(e, row, cols, ctx)?),
            None => action,
        };
        return Err(msg);
    }
    let mut vals = Vec::new();
    for a in args {
        vals.push(eval(a, row, cols, ctx)?);
    }

    if let Some(res) = functions::dispatch(name, &vals) {
        return res;
    }
    match name {

        "LIKE" => {
            if vals.len() < 2 || vals[..2].iter().any(|v| matches!(v, Value::Null)) {
                return Ok(Value::Null);
            }
            let esc = vals.get(2).and_then(|v| text_of(v).chars().next());
            Ok(Value::Int(
                like_match_esc(&text_of(&vals[0]), &text_of(&vals[1]), esc) as i64,
            ))
        }

        "GLOB" => {
            if vals.len() < 2 || vals[..2].iter().any(|v| matches!(v, Value::Null)) {
                return Ok(Value::Null);
            }
            Ok(Value::Int(
                glob_match(&text_of(&vals[0]), &text_of(&vals[1])) as i64,
            ))
        }
        "LENGTH" => Ok(match vals.first() {
            Some(Value::Null) | None => Value::Null,
            Some(Value::Blob(b)) => Value::Int(b.len() as i64),
            Some(v) => Value::Int(text_of(v).chars().count() as i64),
        }),
        "UPPER" => Ok(match vals.first() {
            Some(Value::Null) | None => Value::Null,
            Some(v) => Value::Text(text_of(v).to_uppercase()),
        }),
        "LOWER" => Ok(match vals.first() {
            Some(Value::Null) | None => Value::Null,
            Some(v) => Value::Text(text_of(v).to_lowercase()),
        }),
        "ABS" => Ok(match vals.first() {
            Some(Value::Int(i)) => Value::Int(i.abs()),
            Some(Value::Null) | None => Value::Null,
            Some(v) => v
                .as_f64()
                .map(|f| Value::Real(f.abs()))
                .unwrap_or(Value::Null),
        }),
        "COALESCE" | "IFNULL" => Ok(vals
            .into_iter()
            .find(|v| !matches!(v, Value::Null))
            .unwrap_or(Value::Null)),
        "TYPEOF" => Ok(Value::Text(
            match vals.first() {
                Some(Value::Null) | None => "null",
                Some(Value::Int(_)) => "integer",
                Some(Value::Real(_)) => "real",
                Some(Value::Text(_)) => "text",
                Some(Value::Blob(_)) => "blob",
            }
            .into(),
        )),

        "LAST_INSERT_ROWID" => Ok(Value::Int(match &ctx.db {
            Some(db) => db.last_insert_rowid,
            None => 0,
        })),
        "CHANGES" => Ok(Value::Int(match &ctx.db {
            Some(db) => db.changes,
            None => 0,
        })),
        "TOTAL_CHANGES" => Ok(Value::Int(match &ctx.db {
            Some(db) => db.total_changes,
            None => 0,
        })),

        "RANDOM" => Ok(Value::Int(next_rand() as i64)),
        "RANDOMBLOB" => {
            let n = vals
                .first()
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0)
                .max(0.0) as usize;
            let mut b = Vec::with_capacity(n);
            while b.len() < n {
                b.extend_from_slice(&next_rand().to_le_bytes());
            }
            b.truncate(n);
            Ok(Value::Blob(b))
        }
        _ => Err(format!("no such function: {name}")),
    }
}

thread_local! {
    static RNG_STATE: std::cell::Cell<u64> = const { std::cell::Cell::new(0x2545_F491_4F6C_DD1D) };
}

fn next_rand() -> u64 {
    RNG_STATE.with(|c| {
        let mut x = c.get();
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        c.set(x);
        x
    })
}

fn eval_agg_expr(
    e: &Expr,
    grp: &[&Vec<Value>],
    cols: &[String],
    ctx: &mut ParamCtx,
) -> SqlResult<Value> {
    if expr_is_aggregate(e) {
        return eval_aggregate(e, grp, cols, ctx);
    }
    match e {
        Expr::Binary(op, l, r) => {
            let a = eval_agg_expr(l, grp, cols, ctx)?;
            let b = eval_agg_expr(r, grp, cols, ctx)?;
            eval_binary(op, a, b)
        }
        Expr::Unary(op, inner) => {
            let v = eval_agg_expr(inner, grp, cols, ctx)?;
            match op.as_str() {
                "-" => Ok(match v {
                    Value::Int(i) => Value::Int(-i),
                    Value::Real(r) => Value::Real(-r),
                    Value::Null => Value::Null,

                    Value::Text(s) => match text_to_num(&s) {
                        Value::Int(i) => Value::Int(-i),
                        other => Value::Real(-other.as_f64().unwrap_or(0.0)),
                    },
                    o => o.as_f64().map(|f| Value::Real(-f)).unwrap_or(Value::Null),
                }),
                "NOT" => Ok(match v {
                    Value::Null => Value::Null,
                    o => Value::Int(if o.truthy() { 0 } else { 1 }),
                }),
                _ => Err(format!("bad unary op {op}")),
            }
        }

        Expr::Func(name, args) => {
            let lits: Vec<Expr> = args
                .iter()
                .map(|a| eval_agg_expr(a, grp, cols, ctx).map(Expr::Lit))
                .collect::<SqlResult<_>>()?;
            let rep: &[Value] = grp.first().map(|r| r.as_slice()).unwrap_or(&[]);
            eval_scalar_func(name, &lits, rep, cols, ctx)
        }

        Expr::Case { operand, arms, els } => {
            let op = operand
                .as_ref()
                .map(|o| eval_agg_expr(o, grp, cols, ctx))
                .transpose()?;
            for (cond, res) in arms {
                let c = eval_agg_expr(cond, grp, cols, ctx)?;
                let hit = match &op {
                    Some(o) => o.compare(&c) == std::cmp::Ordering::Equal,
                    None => c.truthy(),
                };
                if hit {
                    return eval_agg_expr(res, grp, cols, ctx);
                }
            }
            match els {
                Some(e) => eval_agg_expr(e, grp, cols, ctx),
                None => Ok(Value::Null),
            }
        }
        _ => {
            let rep: &[Value] = grp.first().map(|r| r.as_slice()).unwrap_or(&[]);
            eval(e, rep, cols, ctx)
        }
    }
}

fn eval_aggregate(
    e: &Expr,
    rows: &[&Vec<Value>],
    cols: &[String],
    ctx: &mut ParamCtx,
) -> SqlResult<Value> {

    if let Expr::AggFilter { func, args, filter } = e {
        let mut kept: Vec<&Vec<Value>> = Vec::new();
        for r in rows {
            if eval(filter, r, cols, ctx)?.truthy() {
                kept.push(r);
            }
        }
        let inner = Expr::Func(func.clone(), args.clone());
        return eval_aggregate(&inner, &kept, cols, ctx);
    }

    if let Expr::AggOrder {
        func,
        args,
        order,
        filter,
    } = e
    {
        let mut kept: Vec<&Vec<Value>> = Vec::new();
        for r in rows {
            if match filter {
                Some(f) => eval(f, r, cols, ctx)?.truthy(),
                None => true,
            } {
                kept.push(r);
            }
        }

        let mut keyed: Vec<(Vec<Value>, &Vec<Value>)> = Vec::with_capacity(kept.len());
        for r in kept {
            let mut ks = Vec::with_capacity(order.len());
            for (ke, _) in order {
                ks.push(eval(ke, r, cols, ctx)?);
            }
            keyed.push((ks, r));
        }
        keyed.sort_by(|a, b| {
            for (i, (_, desc)) in order.iter().enumerate() {
                let c = a.0[i].compare(&b.0[i]);
                let c = if *desc { c.reverse() } else { c };
                if c != std::cmp::Ordering::Equal {
                    return c;
                }
            }
            std::cmp::Ordering::Equal
        });
        let sorted: Vec<&Vec<Value>> = keyed.into_iter().map(|(_, r)| r).collect();
        let inner = Expr::Func(func.clone(), args.clone());
        return eval_aggregate(&inner, &sorted, cols, ctx);
    }
    let Expr::Func(name, args) = e else {
        return Err("expected aggregate".into());
    };

    let (arg, distinct) = match args.first() {
        Some(Expr::Distinct(inner)) => (Some(inner.as_ref()), true),
        other => (other, false),
    };
    match name.as_str() {
        "COUNT" => {
            if matches!(arg, Some(Expr::Star) | None) {
                return Ok(Value::Int(rows.len() as i64));
            }
            let a = arg.unwrap();
            if distinct {
                let mut seen = DistinctSet::default();
                let mut n = 0i64;
                for r in rows {
                    let v = eval(a, r, cols, ctx)?;
                    if matches!(v, Value::Null) {
                        continue;
                    }
                    if seen.insert(&v) {
                        n += 1;
                    }
                }
                return Ok(Value::Int(n));
            }
            let mut n = 0i64;
            for r in rows {
                if !matches!(eval(a, r, cols, ctx)?, Value::Null) {
                    n += 1;
                }
            }
            Ok(Value::Int(n))
        }
        "GROUP_CONCAT" => {
            let a = arg.ok_or("GROUP_CONCAT needs an argument")?;

            let sep = match args.get(1) {
                Some(e) => text_of(&eval(e, &[], &[], ctx)?),
                None => ",".to_string(),
            };
            let mut parts: Vec<String> = Vec::new();
            let mut seen = DistinctSet::default();
            for r in rows {
                let v = eval(a, r, cols, ctx)?;
                if matches!(v, Value::Null) {
                    continue;
                }
                if distinct && !seen.insert(&v) {
                    continue;
                }
                parts.push(text_of(&v));
            }
            if parts.is_empty() {
                Ok(Value::Null)
            } else {
                Ok(Value::Text(parts.join(&sep)))
            }
        }

        "JSON_GROUP_ARRAY" => {
            let a = arg.ok_or("json_group_array needs an argument")?;
            let mut vals = Vec::new();
            for r in rows {
                vals.push(eval(a, r, cols, ctx)?);
            }
            Ok(functions::json::group_array(&vals))
        }
        "JSON_GROUP_OBJECT" => {
            let (ka, va) = (
                args.first().ok_or("json_group_object needs key,value")?,
                args.get(1).ok_or("json_group_object needs key,value")?,
            );
            let mut pairs = Vec::new();
            for r in rows {
                pairs.push((eval(ka, r, cols, ctx)?, eval(va, r, cols, ctx)?));
            }
            Ok(functions::json::group_object(&pairs))
        }
        "SUM" | "TOTAL" | "AVG" => {
            let a = arg.ok_or("aggregate needs an argument")?;

            let mut isum: i64 = 0;
            let mut int_ok = true;
            let mut fsum = 0.0f64;
            let mut ferr = 0.0f64;
            let mut cnt = 0i64;
            let mut kbn = |v: f64| {
                let t = fsum + v;
                if fsum.abs() > v.abs() {
                    ferr += (fsum - t) + v;
                } else {
                    ferr += (v - t) + fsum;
                }
                fsum = t;
            };
            for r in rows {
                match eval(a, r, cols, ctx)? {
                    Value::Null => {}
                    Value::Int(i) => {
                        cnt += 1;
                        kbn(i as f64);
                        match isum.checked_add(i) {
                            Some(s) => isum = s,
                            None => int_ok = false,
                        }
                    }
                    v => {
                        cnt += 1;
                        int_ok = false;
                        if let Some(f) = v.as_f64() {
                            kbn(f);
                        }
                    }
                }
            }
            let ftotal = fsum + ferr;
            if name == "AVG" {
                return Ok(if cnt == 0 {
                    Value::Null
                } else if int_ok {
                    Value::Real(isum as f64 / cnt as f64)
                } else {
                    Value::Real(ftotal / cnt as f64)
                });
            }
            if name == "TOTAL" {
                return Ok(Value::Real(if int_ok { isum as f64 } else { ftotal }));
            }
            if cnt == 0 {
                Ok(Value::Null)
            } else if int_ok {
                Ok(Value::Int(isum))
            } else {
                Ok(Value::Real(ftotal))
            }
        }
        "MIN" | "MAX" => {
            let a = arg.ok_or("aggregate needs an argument")?;
            let mut best: Option<Value> = None;
            for r in rows {
                let v = eval(a, r, cols, ctx)?;
                if matches!(v, Value::Null) {
                    continue;
                }
                best = Some(match best {
                    None => v,
                    Some(b) => {
                        let take = if name == "MIN" {
                            v.compare(&b) == std::cmp::Ordering::Less
                        } else {
                            v.compare(&b) == std::cmp::Ordering::Greater
                        };
                        if take {
                            v
                        } else {
                            b
                        }
                    }
                });
            }
            Ok(best.unwrap_or(Value::Null))
        }
        _ => Err(format!("no such aggregate: {name}")),
    }
}

fn text_of(v: &Value) -> String {
    match v {
        Value::Null => String::new(),
        Value::Int(i) => i.to_string(),
        Value::Real(r) => {
            if r.fract() == 0.0 && r.is_finite() {
                format!("{r:.1}")
            } else {
                r.to_string()
            }
        }
        Value::Text(s) => s.clone(),
        Value::Blob(b) => String::from_utf8_lossy(b).into_owned(),
    }
}

fn rows_equal(a: &[Value], b: &[Value]) -> bool {
    a.len() == b.len()
        && a.iter()
            .zip(b.iter())
            .all(|(x, y)| x.compare(y) == std::cmp::Ordering::Equal)
}

fn stmt_refers_table(stmt: &Stmt, name: &str) -> bool {
    match stmt {
        Stmt::Select {
            from,
            from_sub,
            joins,
            ..
        } => {
            if from
                .as_deref()
                .is_some_and(|t| t.eq_ignore_ascii_case(name))
            {
                return true;
            }
            if joins.iter().any(|j| j.table.eq_ignore_ascii_case(name)) {
                return true;
            }
            if let Some(sub) = from_sub {
                if stmt_refers_table(sub, name) {
                    return true;
                }
            }
            false
        }
        Stmt::CompoundSelect { first, rest, .. } => {
            stmt_refers_table(first, name) || rest.iter().any(|(_, s)| stmt_refers_table(s, name))
        }
        _ => false,
    }
}

fn passthrough_limit(body: &Stmt, cte_name: &str, ctx: &mut ParamCtx) -> Option<usize> {
    if let Stmt::Select {
        from,
        from_sub,
        joins,
        where_,
        group_by,
        having,
        distinct,
        order_by,
        limit,
        offset,
        ..
    } = body
    {
        let bare = from
            .as_deref()
            .is_some_and(|t| t.eq_ignore_ascii_case(cte_name))
            && from_sub.is_none()
            && joins.is_empty()
            && where_.is_none()
            && group_by.is_empty()
            && having.is_none()
            && !*distinct
            && order_by.is_empty()
            && offset.is_none();
        if bare {
            if let Some(e) = limit {
                if let Ok(v) = eval(e, &[], &[], ctx) {
                    let n = v.as_f64().unwrap_or(0.0);
                    if n >= 0.0 {
                        return Some(n as usize);
                    }
                }
            }
        }
    }
    None
}

fn split_schema(name: &str) -> (Option<&str>, &str) {
    match name.split_once('.') {
        Some((s, t)) => (Some(s), t),
        None => (None, name),
    }
}

fn okey_eq(a: &[Value], b: &[Value]) -> bool {
    a.len() == b.len()
        && a.iter()
            .zip(b)
            .all(|(x, y)| x.compare(y) == std::cmp::Ordering::Equal)
}

fn frame_excluded(oi: usize, pos: usize, exclude: FrameExclude, okeys: &[Vec<Value>]) -> bool {
    match exclude {
        FrameExclude::NoOthers => false,
        FrameExclude::CurrentRow => oi == pos,
        FrameExclude::Group => okey_eq(&okeys[oi], &okeys[pos]),
        FrameExclude::Ties => oi != pos && okey_eq(&okeys[oi], &okeys[pos]),
    }
}

fn frame_range(
    pos: usize,
    n: usize,
    okeys: &[Vec<Value>],
    ordered: bool,
    frame: &Option<Frame>,
    desc: bool,
    group_ids: &[usize],
) -> (usize, usize) {
    let peer_end = |p: usize| {
        let mut e = p + 1;
        while e < n && okey_eq(&okeys[e], &okeys[p]) {
            e += 1;
        }
        e
    };
    let peer_start = |p: usize| {
        let mut s = p;
        while s > 0 && okey_eq(&okeys[s - 1], &okeys[p]) {
            s -= 1;
        }
        s
    };
    let valf = |i: usize| {
        okeys
            .get(i)
            .and_then(|k| k.first())
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0)
    };

    let lower = |target: f64| {
        (0..n)
            .find(|&i| {
                if desc {
                    valf(i) <= target
                } else {
                    valf(i) >= target
                }
            })
            .unwrap_or(n)
    };
    let upper = |target: f64| {
        (0..n)
            .find(|&i| {
                if desc {
                    valf(i) < target
                } else {
                    valf(i) > target
                }
            })
            .unwrap_or(n)
    };
    let g_lower = |target: i64| (0..n).find(|&i| group_ids[i] as i64 >= target).unwrap_or(n);
    let g_upper = |target: i64| (0..n).find(|&i| group_ids[i] as i64 > target).unwrap_or(n);
    match frame {
        None => {
            if !ordered {
                (0, n)
            } else {
                (0, peer_end(pos))
            }
        }
        Some(f) if f.unit == FrameUnit::Rows => {
            let start = match f.start {
                FrameBound::UnboundedPreceding => 0,
                FrameBound::Preceding(k) => pos.saturating_sub(k as usize),
                FrameBound::CurrentRow => pos,
                FrameBound::Following(k) => (pos + k as usize).min(n),
                FrameBound::UnboundedFollowing => n,
            };
            let end = match f.end {
                FrameBound::UnboundedPreceding => 0,
                FrameBound::Preceding(k) => pos.saturating_sub(k as usize) + 1,
                FrameBound::CurrentRow => pos + 1,
                FrameBound::Following(k) => (pos + k as usize + 1).min(n),
                FrameBound::UnboundedFollowing => n,
            };
            let start = start.min(n);
            (start, end.min(n).max(start))
        }
        Some(f) if f.unit == FrameUnit::Range => {
            let cur = valf(pos);
            let kf = |k: i64| k as f64;
            let start = match f.start {
                FrameBound::UnboundedPreceding => 0,
                FrameBound::CurrentRow => peer_start(pos),
                FrameBound::Preceding(k) => lower(if desc { cur + kf(k) } else { cur - kf(k) }),
                FrameBound::Following(k) => lower(if desc { cur - kf(k) } else { cur + kf(k) }),
                FrameBound::UnboundedFollowing => n,
            };
            let end = match f.end {
                FrameBound::UnboundedFollowing => n,
                FrameBound::CurrentRow => peer_end(pos),
                FrameBound::Preceding(k) => upper(if desc { cur + kf(k) } else { cur - kf(k) }),
                FrameBound::Following(k) => upper(if desc { cur - kf(k) } else { cur + kf(k) }),
                FrameBound::UnboundedPreceding => 0,
            };
            (start, end.max(start))
        }
        Some(f) => {

            let g = group_ids.get(pos).copied().unwrap_or(0) as i64;
            let start = match f.start {
                FrameBound::UnboundedPreceding => 0,
                FrameBound::CurrentRow => peer_start(pos),
                FrameBound::Preceding(k) => g_lower(g - k),
                FrameBound::Following(k) => g_lower(g + k),
                FrameBound::UnboundedFollowing => n,
            };
            let end = match f.end {
                FrameBound::UnboundedFollowing => n,
                FrameBound::CurrentRow => peer_end(pos),
                FrameBound::Preceding(k) => g_upper(g - k),
                FrameBound::Following(k) => g_upper(g + k),
                FrameBound::UnboundedPreceding => 0,
            };
            (start, end.max(start))
        }
    }
}

fn dedup_rows(rows: Vec<Vec<Value>>) -> Vec<Vec<Value>> {
    let mut out: Vec<Vec<Value>> = Vec::new();
    for r in rows {
        if !out.iter().any(|e| rows_equal(e, &r)) {
            out.push(r);
        }
    }
    out
}

fn combine_compound(
    op: CompoundOp,
    left: Vec<Vec<Value>>,
    right: Vec<Vec<Value>>,
) -> Vec<Vec<Value>> {
    match op {
        CompoundOp::UnionAll => {
            let mut out = left;
            out.extend(right);
            out
        }
        CompoundOp::Union => {
            let mut out = left;
            out.extend(right);
            dedup_rows(out)
        }
        CompoundOp::Intersect => {
            let deduped = dedup_rows(left);
            deduped
                .into_iter()
                .filter(|r| right.iter().any(|o| rows_equal(o, r)))
                .collect()
        }
        CompoundOp::Except => {
            let deduped = dedup_rows(left);
            deduped
                .into_iter()
                .filter(|r| !right.iter().any(|o| rows_equal(o, r)))
                .collect()
        }
    }
}

fn expr_to_sql(e: &Expr) -> String {
    match e {
        Expr::Collate(inner, name) => format!("{} COLLATE {name}", expr_to_sql(inner)),
        Expr::AggFilter { func, .. } => format!("{func}(...) FILTER (...)"),
        Expr::AggOrder { func, .. } => format!("{func}(... ORDER BY ...)"),
        Expr::Window { func, .. } => format!("{func}() OVER (...)"),
        Expr::Lit(Value::Null) => "NULL".into(),
        Expr::Lit(Value::Int(i)) => i.to_string(),
        Expr::Lit(Value::Real(r)) => text_of(&Value::Real(*r)),
        Expr::Lit(Value::Text(s)) => format!("'{}'", s.replace('\'', "''")),
        Expr::Lit(Value::Blob(_)) => "x''".into(),
        Expr::Col(c) => c.clone(),
        Expr::Star => "*".into(),
        Expr::Param(_) => "?".into(),
        Expr::Unary(op, inner) => {
            if op == "NOT" {
                format!("NOT {}", expr_to_sql(inner))
            } else {
                format!("{}{}", op, expr_to_sql(inner))
            }
        }
        Expr::Binary(op, l, r) => format!("{} {} {}", expr_to_sql(l), op, expr_to_sql(r)),
        Expr::Func(name, args) => {
            let a: Vec<String> = args.iter().map(expr_to_sql).collect();
            format!("{}({})", name, a.join(", "))
        }
        Expr::Distinct(inner) => format!("DISTINCT {}", expr_to_sql(inner)),
        Expr::Case { .. } => "CASE".into(),
        Expr::Subquery(_) => "(SELECT ...)".into(),
        Expr::InSelect(l, _) => format!("{} IN (SELECT ...)", expr_to_sql(l)),
        Expr::Exists(_) => "EXISTS (SELECT ...)".into(),
    }
}

fn col_indices(columns: &[ColumnDef], names: &[String]) -> SqlResult<Vec<usize>> {
    names
        .iter()
        .map(|n| {
            columns
                .iter()
                .position(|c| c.name.eq_ignore_ascii_case(n))
                .ok_or_else(|| format!("no such column: {n}"))
        })
        .collect()
}

fn val_to_sql(v: &Value) -> sql_core::SqlValue {
    match v {
        Value::Null => sql_core::SqlValue::Null,
        Value::Int(i) => sql_core::SqlValue::Int(*i),
        Value::Real(r) => sql_core::SqlValue::Real(*r),
        Value::Text(s) => sql_core::SqlValue::Text(s.clone()),
        Value::Blob(b) => sql_core::SqlValue::Blob(b.clone()),
    }
}
fn sql_to_val(v: &sql_core::SqlValue) -> Value {
    match v {
        sql_core::SqlValue::Null => Value::Null,
        sql_core::SqlValue::Int(i) => Value::Int(*i),
        sql_core::SqlValue::Real(r) => Value::Real(*r),
        sql_core::SqlValue::Text(s) => Value::Text(s.clone()),
        sql_core::SqlValue::Blob(b) => Value::Blob(b.clone()),
    }
}
fn sql_to_val_owned(v: sql_core::SqlValue) -> Value {
    match v {
        sql_core::SqlValue::Null => Value::Null,
        sql_core::SqlValue::Int(i) => Value::Int(i),
        sql_core::SqlValue::Real(r) => Value::Real(r),
        sql_core::SqlValue::Text(s) => Value::Text(s),
        sql_core::SqlValue::Blob(b) => Value::Blob(b),
    }
}
fn rows_to_sql(rows: &[Vec<Value>]) -> Vec<Vec<sql_core::SqlValue>> {
    rows.iter()
        .map(|r| r.iter().map(val_to_sql).collect())
        .collect()
}

fn resolve_qualified(name: &str, schema: &[String]) -> String {
    if schema.iter().any(|s| s.eq_ignore_ascii_case(name)) {
        return name.to_string();
    }
    for s in schema {
        if let Some((_, c)) = s.rsplit_once('.') {
            if c.eq_ignore_ascii_case(name) {
                return s.clone();
            }
        }
    }
    name.to_string()
}

fn bind_cols(e: &Expr, schema: &[String]) -> Expr {
    match e {
        Expr::Col(name) => Expr::Col(resolve_qualified(name, schema)),
        Expr::Unary(o, x) => Expr::Unary(o.clone(), Box::new(bind_cols(x, schema))),
        Expr::Binary(o, l, r) => Expr::Binary(
            o.clone(),
            Box::new(bind_cols(l, schema)),
            Box::new(bind_cols(r, schema)),
        ),
        Expr::Func(n, a) => Expr::Func(n.clone(), a.iter().map(|x| bind_cols(x, schema)).collect()),
        Expr::Case { operand, arms, els } => Expr::Case {
            operand: operand.as_ref().map(|o| Box::new(bind_cols(o, schema))),
            arms: arms
                .iter()
                .map(|(c, r)| (bind_cols(c, schema), bind_cols(r, schema)))
                .collect(),
            els: els.as_ref().map(|e| Box::new(bind_cols(e, schema))),
        },
        other => other.clone(),
    }
}
fn make_pred(
    e: Expr,
    names: Vec<String>,
    binds: Bindings,
    col_aff: std::collections::HashMap<String, Affinity>,
) -> sql_core::Pred {
    Box::new(move |row: &Vec<sql_core::SqlValue>| {
        let vrow: Vec<Value> = row.iter().map(sql_to_val).collect();
        let mut c = ParamCtx {
            b: &binds,
            next_pos: 0,
            db: None,
            scopes: Vec::new(),
            cur_alias: None,
            col_aff: col_aff.clone(),
            subq_cache: std::collections::HashMap::new(),
        };
        Ok(eval(&e, &vrow, &names, &mut c)?.truthy())
    })
}
fn make_scalar(
    e: Expr,
    names: Vec<String>,
    binds: Bindings,
    col_aff: std::collections::HashMap<String, Affinity>,
) -> sql_core::Scalar {
    Box::new(move |row: &Vec<sql_core::SqlValue>| {
        let vrow: Vec<Value> = row.iter().map(sql_to_val).collect();
        let mut c = ParamCtx {
            b: &binds,
            next_pos: 0,
            db: None,
            scopes: Vec::new(),
            cur_alias: None,
            col_aff: col_aff.clone(),
            subq_cache: std::collections::HashMap::new(),
        };
        Ok(val_to_sql(&eval(&e, &vrow, &names, &mut c)?))
    })
}

fn like_match(pat: &str, s: &str) -> bool {
    like_match_esc(pat, s, None)
}

fn like_match_esc(pat: &str, s: &str, esc: Option<char>) -> bool {
    let p: Vec<char> = pat.to_lowercase().chars().collect();
    let t: Vec<char> = s.to_lowercase().chars().collect();
    let esc = esc.and_then(|c| c.to_lowercase().next());
    fn go(p: &[char], t: &[char], esc: Option<char>) -> bool {
        if p.is_empty() {
            return t.is_empty();
        }

        if esc == Some(p[0]) && p.len() > 1 {
            return !t.is_empty() && t[0] == p[1] && go(&p[2..], &t[1..], esc);
        }
        match p[0] {
            '%' => go(&p[1..], t, esc) || (!t.is_empty() && go(p, &t[1..], esc)),
            '_' => !t.is_empty() && go(&p[1..], &t[1..], esc),
            c => !t.is_empty() && t[0] == c && go(&p[1..], &t[1..], esc),
        }
    }
    go(&p, &t, esc)
}

fn glob_match(pat: &str, s: &str) -> bool {
    let p: Vec<char> = pat.chars().collect();
    let t: Vec<char> = s.chars().collect();

    fn class_match(class: &[char], ch: char) -> bool {
        let mut i = 0;
        while i < class.len() {
            if i + 2 < class.len() && class[i + 1] == '-' {
                if ch >= class[i] && ch <= class[i + 2] {
                    return true;
                }
                i += 3;
            } else {
                if ch == class[i] {
                    return true;
                }
                i += 1;
            }
        }
        false
    }
    fn go(p: &[char], t: &[char]) -> bool {
        if p.is_empty() {
            return t.is_empty();
        }
        match p[0] {
            '*' => go(&p[1..], t) || (!t.is_empty() && go(p, &t[1..])),
            '?' => !t.is_empty() && go(&p[1..], &t[1..]),
            '[' => {
                if t.is_empty() {
                    return false;
                }

                let neg = p.get(1) == Some(&'^');
                let body_start = if neg { 2 } else { 1 };
                let mut j = body_start;
                if p.get(j) == Some(&']') {
                    j += 1;
                }
                while j < p.len() && p[j] != ']' {
                    j += 1;
                }
                if j >= p.len() {
                    return t[0] == '[' && go(&p[1..], &t[1..]);
                }
                let hit = class_match(&p[body_start..j], t[0]);
                (hit != neg) && go(&p[j + 1..], &t[1..])
            }
            c => !t.is_empty() && t[0] == c && go(&p[1..], &t[1..]),
        }
    }
    go(&p, &t)
}

fn coerce(v: Value, aff: Affinity) -> Value {
    if matches!(v, Value::Null) {
        return Value::Null;
    }
    match aff {
        Affinity::Blob => v,
        Affinity::Text => match v {
            Value::Text(_) | Value::Blob(_) => v,
            other => Value::Text(text_of(&other)),
        },
        Affinity::Integer => match &v {
            Value::Int(_) => v,
            Value::Real(r) if r.fract() == 0.0 => Value::Int(*r as i64),

            Value::Text(s) => {
                let t = s.trim();
                if let Ok(i) = t.parse::<i64>() {
                    Value::Int(i)
                } else if let Ok(f) = t.parse::<f64>() {
                    if f.fract() == 0.0 && f.is_finite() && f.abs() < 9.223e18 {
                        Value::Int(f as i64)
                    } else {
                        Value::Real(f)
                    }
                } else {
                    v
                }
            }
            _ => v,
        },
        Affinity::Real => v.as_f64().map(Value::Real).unwrap_or(v),
        Affinity::Numeric => match &v {
            Value::Int(_) | Value::Real(_) => v,
            Value::Text(s) => {
                let t = s.trim();
                if let Ok(i) = t.parse::<i64>() {
                    Value::Int(i)
                } else if let Ok(f) = t.parse::<f64>() {
                    Value::Real(f)
                } else {
                    v
                }
            }
            _ => v,
        },
    }
}

fn put_u32(out: &mut Vec<u8>, n: u32) {
    out.extend_from_slice(&n.to_le_bytes());
}
fn put_i64(out: &mut Vec<u8>, n: i64) {
    out.extend_from_slice(&n.to_le_bytes());
}
fn put_bytes(out: &mut Vec<u8>, b: &[u8]) {
    put_u32(out, b.len() as u32);
    out.extend_from_slice(b);
}

fn aff_code(a: Affinity) -> u8 {
    match a {
        Affinity::Integer => 0,
        Affinity::Real => 1,
        Affinity::Text => 2,
        Affinity::Blob => 3,
        Affinity::Numeric => 4,
    }
}
fn aff_from(c: u8) -> Affinity {
    match c {
        0 => Affinity::Integer,
        1 => Affinity::Real,
        2 => Affinity::Text,
        3 => Affinity::Blob,
        _ => Affinity::Numeric,
    }
}

fn serialize(tables: &BTreeMap<String, Table>) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(b"CRSQL1\n");
    put_u32(&mut out, tables.len() as u32);
    for (name, tbl) in tables {
        put_bytes(&mut out, name.as_bytes());
        put_u32(&mut out, tbl.columns.len() as u32);
        for c in &tbl.columns {
            put_bytes(&mut out, c.name.as_bytes());
            out.push(aff_code(c.affinity));
            let flags = (c.pk as u8)
                | ((c.autoincrement as u8) << 1)
                | ((c.not_null as u8) << 2)
                | ((c.unique as u8) << 3);
            out.push(flags);
        }
        put_i64(&mut out, tbl.next_rowid);
        put_u32(&mut out, tbl.rows.len() as u32);
        for row in &tbl.rows {
            for v in row {
                match v {
                    Value::Null => out.push(0),
                    Value::Int(i) => {
                        out.push(1);
                        put_i64(&mut out, *i);
                    }
                    Value::Real(r) => {
                        out.push(2);
                        out.extend_from_slice(&r.to_le_bytes());
                    }
                    Value::Text(s) => {
                        out.push(3);
                        put_bytes(&mut out, s.as_bytes());
                    }
                    Value::Blob(b) => {
                        out.push(4);
                        put_bytes(&mut out, b);
                    }
                }
            }
        }
    }
    out
}

struct Reader<'a> {
    b: &'a [u8],
    i: usize,
}
impl<'a> Reader<'a> {
    fn u32(&mut self) -> SqlResult<u32> {
        let end = self.i + 4;
        let s = self.b.get(self.i..end).ok_or("truncated db file")?;
        self.i = end;
        Ok(u32::from_le_bytes(s.try_into().unwrap()))
    }
    fn i64(&mut self) -> SqlResult<i64> {
        let end = self.i + 8;
        let s = self.b.get(self.i..end).ok_or("truncated db file")?;
        self.i = end;
        Ok(i64::from_le_bytes(s.try_into().unwrap()))
    }
    fn f64(&mut self) -> SqlResult<f64> {
        Ok(f64::from_bits(self.i64()? as u64))
    }
    fn u8(&mut self) -> SqlResult<u8> {
        let v = *self.b.get(self.i).ok_or("truncated db file")?;
        self.i += 1;
        Ok(v)
    }
    fn bytes(&mut self) -> SqlResult<Vec<u8>> {
        let n = self.u32()? as usize;
        let end = self.i + n;
        let s = self.b.get(self.i..end).ok_or("truncated db file")?;
        self.i = end;
        Ok(s.to_vec())
    }
}

fn deserialize(bytes: &[u8]) -> SqlResult<BTreeMap<String, Table>> {
    let mut r = Reader { b: bytes, i: 0 };
    let magic = r.b.get(0..7).ok_or("not a cruft db file")?;
    if magic != b"CRSQL1\n" {
        return Err("unrecognized db file format".into());
    }
    r.i = 7;
    let mut tables = BTreeMap::new();
    let tcount = r.u32()?;
    for _ in 0..tcount {
        let name = String::from_utf8_lossy(&r.bytes()?).into_owned();
        let ccount = r.u32()?;
        let mut columns = Vec::new();
        for _ in 0..ccount {
            let cname = String::from_utf8_lossy(&r.bytes()?).into_owned();
            let aff = aff_from(r.u8()?);
            let flags = r.u8()?;
            columns.push(ColumnDef {
                name: cname,
                affinity: aff,
                decl_type: None,
                pk: flags & 1 != 0,
                autoincrement: flags & 2 != 0,
                not_null: flags & 4 != 0,
                unique: flags & 8 != 0,
                default: None,
                generated: None,
            });
        }
        let next_rowid = r.i64()?;
        let rcount = r.u32()?;
        let mut rows = Vec::new();
        for _ in 0..rcount {
            let mut row = Vec::new();
            for _ in 0..ccount {
                let tag = r.u8()?;
                row.push(match tag {
                    0 => Value::Null,
                    1 => Value::Int(r.i64()?),
                    2 => Value::Real(r.f64()?),
                    3 => Value::Text(String::from_utf8_lossy(&r.bytes()?).into_owned()),
                    4 => Value::Blob(r.bytes()?),
                    _ => return Err("bad value tag in db file".into()),
                });
            }
            rows.push(row);
        }
        let row_ids: Vec<i64> = (1..=rows.len() as i64).collect();
        let max_rowid = row_ids.iter().copied().max().unwrap_or(0);
        tables.insert(
            name,
            Table {
                columns,
                rows,
                row_ids,
                next_rowid,
                max_rowid,
                checks: Vec::new(),
                table_uniques: Vec::new(),
                indexes: Vec::new(),
                foreign_keys: Vec::new(),
                eq_indexes: std::collections::HashMap::new(),
            },
        );
    }
    Ok(tables)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn q(db: &mut Database, sql: &str, b: Bindings) -> Outcome {
        let st = db.prepare(sql).unwrap();
        db.run(&st, &b).unwrap()
    }
    fn rows(o: Outcome) -> (Vec<String>, Vec<Vec<Value>>) {
        match o {
            Outcome::Rows { columns, rows } => (columns, rows),
            _ => panic!("expected rows"),
        }
    }

    #[test]
    fn create_insert_select() {
        let mut db = Database::open_memory();
        db.exec_script("CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT, age INTEGER)")
            .unwrap();
        let mut b = Bindings::new();
        b.positional = vec![Value::Text("ada".into()), Value::Int(36)];
        let o = q(&mut db, "INSERT INTO users (name, age) VALUES (?, ?)", b);
        match o {
            Outcome::Mutation {
                changes,
                last_insert_rowid,
            } => {
                assert_eq!(changes, 1);
                assert_eq!(last_insert_rowid, 1);
            }
            _ => panic!(),
        }
        let (cols, r) = rows(q(
            &mut db,
            "SELECT id, name, age FROM users",
            Bindings::new(),
        ));
        assert_eq!(cols, vec!["id", "name", "age"]);
        assert_eq!(r.len(), 1);
        assert!(matches!(r[0][0], Value::Int(1)));
        assert!(matches!(&r[0][1], Value::Text(s) if s == "ada"));
        assert!(matches!(r[0][2], Value::Int(36)));
    }

    #[test]
    fn insert_values_no_returning_reuses_engine_insert_semantics() {
        let mut db = Database::open_memory();
        db.exec_script(
            "CREATE TABLE t (id INTEGER PRIMARY KEY, name TEXT NOT NULL, age INTEGER DEFAULT 7, doubled INTEGER GENERATED ALWAYS AS (age * 2))",
        )
        .unwrap();
        let out = db
            .insert_values_no_returning(
                "t",
                &["id".into(), "name".into(), "age".into()],
                &[
                    vec![
                        sql_core::SqlValue::Int(1),
                        sql_core::SqlValue::Text("ada".into()),
                        sql_core::SqlValue::Int(11),
                    ],
                    vec![
                        sql_core::SqlValue::Int(2),
                        sql_core::SqlValue::Text("bob".into()),
                        sql_core::SqlValue::Null,
                    ],
                ],
            )
            .unwrap();
        assert!(matches!(
            out,
            Outcome::Mutation {
                changes: 2,
                last_insert_rowid: 2
            }
        ));
        let (cols, r) = rows(q(
            &mut db,
            "SELECT id, name, age, doubled FROM t ORDER BY id",
            Bindings::new(),
        ));
        assert_eq!(cols, vec!["id", "name", "age", "doubled"]);
        assert_eq!(
            r,
            vec![
                vec![
                    Value::Int(1),
                    Value::Text("ada".into()),
                    Value::Int(11),
                    Value::Int(22),
                ],
                vec![
                    Value::Int(2),
                    Value::Text("bob".into()),
                    Value::Null,
                    Value::Null,
                ],
            ]
        );
        let err = match db.insert_values_no_returning(
            "t",
            &["id".into(), "name".into()],
            &[vec![sql_core::SqlValue::Int(3), sql_core::SqlValue::Null]],
        ) {
            Ok(_) => panic!("expected NOT NULL failure"),
            Err(e) => e,
        };
        assert!(err.contains("NOT NULL constraint failed: t.name"));
    }

    #[test]
    fn update_where_in_values_no_returning_uses_membership_predicate() {
        let mut db = Database::open_memory();
        db.exec_script(
            "CREATE TABLE t (id INTEGER PRIMARY KEY, name TEXT NOT NULL, age INTEGER CHECK(age >= 0), doubled INTEGER GENERATED ALWAYS AS (age * 2))",
        )
        .unwrap();
        db.exec_script(
            "INSERT INTO t (id, name, age) VALUES (1, 'ada', 10), (2, 'bob', 20), (3, 'cy', 30)",
        )
        .unwrap();
        let affected = db
            .update_where_in_values_no_returning(
                "t",
                &[("age".into(), sql_core::SqlValue::Int(41))],
                "id",
                &[sql_core::SqlValue::Int(1), sql_core::SqlValue::Int(3)],
            )
            .unwrap();
        assert_eq!(affected, 2);
        let (cols, r) = rows(q(
            &mut db,
            "SELECT id, age, doubled FROM t ORDER BY id",
            Bindings::new(),
        ));
        assert_eq!(cols, vec!["id", "age", "doubled"]);
        assert_eq!(
            r,
            vec![
                vec![Value::Int(1), Value::Int(41), Value::Int(82)],
                vec![Value::Int(2), Value::Int(20), Value::Int(40)],
                vec![Value::Int(3), Value::Int(41), Value::Int(82)],
            ]
        );
        let err = match db.update_where_in_values_no_returning(
            "t",
            &[("age".into(), sql_core::SqlValue::Int(-1))],
            "id",
            &[sql_core::SqlValue::Int(1)],
        ) {
            Ok(_) => panic!("expected CHECK failure"),
            Err(e) => e,
        };
        assert!(err.contains("CHECK constraint failed"));
        let (_, r) = rows(q(
            &mut db,
            "SELECT id, age, doubled FROM t ORDER BY id",
            Bindings::new(),
        ));
        assert_eq!(
            r,
            vec![
                vec![Value::Int(1), Value::Int(41), Value::Int(82)],
                vec![Value::Int(2), Value::Int(20), Value::Int(40)],
                vec![Value::Int(3), Value::Int(41), Value::Int(82)],
            ]
        );
    }

    #[test]
    fn where_order_limit_named_params() {
        let mut db = Database::open_memory();
        db.exec_script("CREATE TABLE t (n INTEGER)").unwrap();
        for n in [5, 1, 3, 2, 4] {
            let mut b = Bindings::new();
            b.positional = vec![Value::Int(n)];
            q(&mut db, "INSERT INTO t (n) VALUES (?)", b);
        }
        let mut b = Bindings::new();
        b.named.insert("min".into(), Value::Int(2));
        let (_, r) = rows(q(
            &mut db,
            "SELECT n FROM t WHERE n >= :min ORDER BY n DESC LIMIT 2",
            b,
        ));
        assert_eq!(r.len(), 2);
        assert!(matches!(r[0][0], Value::Int(5)));
        assert!(matches!(r[1][0], Value::Int(4)));
    }

    #[test]
    fn select_cursor_steps_simple_base_table_without_materialized_rowset() {
        let mut db = Database::open_memory();
        db.exec_script("CREATE TABLE t (id INTEGER PRIMARY KEY, name TEXT, active INTEGER)")
            .unwrap();
        db.exec_script(
            "INSERT INTO t (id, name, active) VALUES (1, 'ada', 1), (2, 'bob', 0), (3, 'cy', 1), (4, 'dee', 1)",
        )
        .unwrap();
        let st = db
            .prepare("SELECT id, name FROM t WHERE active = 1 LIMIT 2 OFFSET 1")
            .unwrap();
        let mut cur = db.select_cursor(&st, &Bindings::new()).unwrap();
        assert_eq!(cur.columns(), &["id".to_string(), "name".to_string()]);
        let first = cur.next_row().unwrap().unwrap();
        assert!(matches!(first[0], Value::Int(3)));
        assert!(matches!(&first[1], Value::Text(s) if s == "cy"));
        let second = cur.next_row().unwrap().unwrap();
        assert!(matches!(second[0], Value::Int(4)));
        assert!(matches!(&second[1], Value::Text(s) if s == "dee"));
        assert!(cur.next_row().unwrap().is_none());
    }

    #[test]
    fn select_cursor_can_close_early_and_refuses_materializing_shapes() {
        let mut db = Database::open_memory();
        db.exec_script("CREATE TABLE t (id INTEGER PRIMARY KEY, name TEXT)")
            .unwrap();
        db.exec_script("INSERT INTO t (id, name) VALUES (1, 'ada'), (2, 'bob')")
            .unwrap();
        let st = db.prepare("SELECT * FROM t").unwrap();
        let mut cur = db.select_cursor(&st, &Bindings::new()).unwrap();
        assert!(cur.next_row().unwrap().is_some());
        cur.close();
        assert!(cur.next_row().unwrap().is_none());

        let ordered = db.prepare("SELECT id FROM t ORDER BY id DESC").unwrap();
        assert!(db.select_cursor(&ordered, &Bindings::new()).is_err());
        let aggregate = db.prepare("SELECT COUNT(*) FROM t").unwrap();
        assert!(db.select_cursor(&aggregate, &Bindings::new()).is_err());
    }

    #[test]
    fn select_cursor_session_steps_with_short_database_borrows() {
        let mut db = Database::open_memory();
        db.exec_script("CREATE TABLE t (id INTEGER, v TEXT)")
            .unwrap();
        db.exec_script("INSERT INTO t (id, v) VALUES (1, 'a'), (2, 'b'), (3, 'c')")
            .unwrap();
        let st = db
            .prepare("SELECT id, v FROM t WHERE id >= 2 LIMIT 2")
            .unwrap();
        let mut cur = db.select_cursor_session(&st, &Bindings::new()).unwrap();
        assert_eq!(cur.columns(), &["id".to_string(), "v".to_string()]);
        assert_eq!(
            db.step_select_cursor(&mut cur).unwrap(),
            Some(vec![Value::Int(2), Value::Text("b".into())])
        );
        assert_eq!(
            db.step_select_cursor(&mut cur).unwrap(),
            Some(vec![Value::Int(3), Value::Text("c".into())])
        );
        assert_eq!(db.step_select_cursor(&mut cur).unwrap(), None);
        cur.close();
        assert_eq!(db.step_select_cursor(&mut cur).unwrap(), None);
    }

    #[test]
    fn update_delete_aggregate() {
        let mut db = Database::open_memory();
        db.exec_script("CREATE TABLE t (id INTEGER PRIMARY KEY, v INTEGER)")
            .unwrap();
        db.exec_script("INSERT INTO t (v) VALUES (10), (20), (30)")
            .unwrap();
        let (_, r) = rows(q(
            &mut db,
            "SELECT COUNT(*), SUM(v), AVG(v), MAX(v) FROM t",
            Bindings::new(),
        ));
        assert!(matches!(r[0][0], Value::Int(3)));
        assert!(matches!(r[0][1], Value::Int(60)));
        assert!(matches!(r[0][2], Value::Real(x) if (x - 20.0).abs() < 1e-9));
        assert!(matches!(r[0][3], Value::Int(30)));

        let o = q(&mut db, "UPDATE t SET v = 99 WHERE v = 20", Bindings::new());
        assert!(matches!(o, Outcome::Mutation { changes: 1, .. }));
        let o = q(&mut db, "DELETE FROM t WHERE v < 30", Bindings::new());
        assert!(matches!(o, Outcome::Mutation { changes: 1, .. }));
    }

    #[test]
    fn delete_where_in_values_no_returning_uses_membership_predicate() {
        let mut db = Database::open_memory();
        db.exec_script("CREATE TABLE t (id INTEGER PRIMARY KEY, v TEXT)")
            .unwrap();
        db.exec_script("INSERT INTO t (id, v) VALUES (1, 'a'), (2, 'b'), (3, 'c'), (4, 'd')")
            .unwrap();
        let affected = db
            .delete_where_in_values_no_returning(
                "t",
                "id",
                &[
                    sql_core::SqlValue::Int(1),
                    sql_core::SqlValue::Text("3".into()),
                    sql_core::SqlValue::Null,
                    sql_core::SqlValue::Int(99),
                ],
            )
            .unwrap();
        assert_eq!(affected, 2);
        let (_, r) = rows(q(
            &mut db,
            "SELECT id, v FROM t ORDER BY id",
            Bindings::new(),
        ));
        assert_eq!(r.len(), 2);
        assert!(matches!(r[0][0], Value::Int(2)));
        assert!(matches!(&r[0][1], Value::Text(s) if s == "b"));
        assert!(matches!(r[1][0], Value::Int(4)));
        assert!(matches!(&r[1][1], Value::Text(s) if s == "d"));
    }

    #[test]
    fn expr_no_from() {
        let mut db = Database::open_memory();
        let (_, r) = rows(q(
            &mut db,
            "SELECT 1 + 2 * 3, UPPER('hi'), LENGTH('abcd')",
            Bindings::new(),
        ));
        assert!(matches!(r[0][0], Value::Int(7)));
        assert!(matches!(&r[0][1], Value::Text(s) if s == "HI"));
        assert!(matches!(r[0][2], Value::Int(4)));
    }

    #[test]
    fn subqueries_and_connection_counters_do_not_require_raw_database_pointer() {
        let mut db = Database::open_memory();
        db.exec_script(
            "CREATE TABLE parent (id INTEGER PRIMARY KEY, name TEXT);
             CREATE TABLE child (parent_id INTEGER, value INTEGER);
             INSERT INTO parent (name) VALUES ('a'), ('b');
             INSERT INTO child (parent_id, value) VALUES (1, 7), (1, 9);",
        )
        .unwrap();

        let (_, r) = rows(q(
            &mut db,
            "SELECT p.name,
                    (SELECT MAX(c.value) FROM child c WHERE c.parent_id = p.id)
               FROM parent p
              WHERE EXISTS (SELECT 1 FROM child c WHERE c.parent_id = p.id)
              ORDER BY p.id",
            Bindings::new(),
        ));
        assert_eq!(r.len(), 1);
        assert!(matches!(&r[0][0], Value::Text(s) if s == "a"));
        assert!(matches!(r[0][1], Value::Int(9)));

        let (_, r) = rows(q(
            &mut db,
            "SELECT last_insert_rowid(), changes(), total_changes()",
            Bindings::new(),
        ));
        assert!(matches!(r[0][0], Value::Int(2)));
        assert!(matches!(r[0][1], Value::Int(2)));
        assert!(matches!(r[0][2], Value::Int(4)));
    }

    #[test]
    fn parser_rejects_excessive_expression_nesting_without_stack_overflow() {
        let parens = format!("SELECT {}1{}", "(".repeat(1100), ")".repeat(1100));
        let err = match parse_statement(&parens) {
            Ok(_) => panic!("deep parenthesized SQL must fail"),
            Err(err) => err,
        };
        assert!(err.contains("nesting depth"));

        let not_chain = format!("SELECT {}1", "NOT ".repeat(1100));
        let err = match parse_statement(&not_chain) {
            Ok(_) => panic!("deep NOT SQL must fail"),
            Err(err) => err,
        };
        assert!(err.contains("nesting depth"));
    }

    #[test]
    fn persistence_roundtrip() {
        let dir = std::env::temp_dir();
        let path = dir.join(format!("rusty_sqlite_test_{}.db", std::process::id()));
        let p = path.to_str().unwrap();
        let _ = std::fs::remove_file(p);
        {
            let mut db = Database::open_file(p).unwrap();
            db.exec_script("CREATE TABLE k (id INTEGER PRIMARY KEY, s TEXT)")
                .unwrap();
            db.exec_script("INSERT INTO k (s) VALUES ('persisted')")
                .unwrap();
        }
        {
            let mut db = Database::open_file(p).unwrap();
            let (_, r) = rows(q(&mut db, "SELECT s FROM k", Bindings::new()));
            assert_eq!(r.len(), 1);
            assert!(matches!(&r[0][0], Value::Text(s) if s == "persisted"));
        }
        let _ = std::fs::remove_file(p);
    }

    #[test]
    fn like_and_coercion() {
        let mut db = Database::open_memory();
        db.exec_script("CREATE TABLE t (name TEXT)").unwrap();
        db.exec_script("INSERT INTO t (name) VALUES ('Alice'), ('Bob'), ('Alfred')")
            .unwrap();
        let (_, r) = rows(q(
            &mut db,
            "SELECT name FROM t WHERE name LIKE 'al%' ORDER BY name",
            Bindings::new(),
        ));
        assert_eq!(r.len(), 2);
        assert!(matches!(&r[0][0], Value::Text(s) if s == "Alfred"));
        assert!(matches!(&r[1][0], Value::Text(s) if s == "Alice"));
    }
}
