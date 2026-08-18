
use crate::types::tsquery::TsQuery;
use crate::types::{tsquery, tsvector, PgError};
use sql_core::SqlValue;

fn owns(name: &str) -> bool {
    matches!(
        name,
        "to_tsvector"
            | "to_tsquery"
            | "plainto_tsquery"
            | "phraseto_tsquery"
            | "numnode"
            | "strip"
            | "ts_rank"
            | "ts_rank_cd"
            | "ts_headline"
            | "setweight"
    )
}

fn arity_err(name: &str) -> PgError {
    PgError::InvalidInputSyntax {
        typ: "expression",
        input: format!("function {name}(...) does not exist"),
    }
}

fn text_arg<'a>(name: &str, args: &'a [SqlValue]) -> Result<Option<&'a str>, PgError> {
    let v = match args.len() {
        1 => &args[0],
        2 => &args[1],
        _ => return Err(arity_err(name)),
    };
    match v {
        SqlValue::Null => Ok(None),
        SqlValue::Text(s) => Ok(Some(s.as_str())),
        _ => Err(arity_err(name)),
    }
}

pub fn tsvector_length(s: &str) -> Option<i64> {

    if !s.contains('\'') {
        return None;
    }
    let entries = tsvector::parse(s).ok()?;
    if tsvector::render(&entries) == s {
        Some(entries.len() as i64)
    } else {
        None
    }
}

pub fn call(name: &str, args: &[SqlValue]) -> Option<Result<SqlValue, PgError>> {
    if !owns(name) {
        return None;
    }
    Some(match name {
        "to_tsvector" => match text_arg(name, args) {
            Ok(None) => Ok(SqlValue::Null),
            Ok(Some(t)) => Ok(SqlValue::Text(tsvector::render(&tsvector::tokenize(t)))),
            Err(e) => Err(e),
        },
        "to_tsquery" => match text_arg(name, args) {
            Ok(None) => Ok(SqlValue::Null),
            Ok(Some(t)) => match tsquery::parse(t) {
                Ok(q) => Ok(SqlValue::Text(tsquery::render(&q))),
                Err(e) => Err(e),
            },
            Err(e) => Err(e),
        },
        "plainto_tsquery" => match text_arg(name, args) {
            Ok(None) => Ok(SqlValue::Null),
            Ok(Some(t)) => Ok(SqlValue::Text(join_words(t, false))),
            Err(e) => Err(e),
        },
        "phraseto_tsquery" => match text_arg(name, args) {
            Ok(None) => Ok(SqlValue::Null),
            Ok(Some(t)) => Ok(SqlValue::Text(join_words(t, true))),
            Err(e) => Err(e),
        },
        "numnode" => {
            if args.len() != 1 {
                return Some(Err(arity_err(name)));
            }
            match &args[0] {
                SqlValue::Null => Ok(SqlValue::Null),
                SqlValue::Text(s) => match tsquery::parse(s) {
                    Ok(q) => Ok(SqlValue::Int(tsquery::numnode(&q))),
                    Err(e) => Err(e),
                },
                _ => Err(arity_err(name)),
            }
        }
        "strip" => {
            if args.len() != 1 {
                return Some(Err(arity_err(name)));
            }
            match &args[0] {
                SqlValue::Null => Ok(SqlValue::Null),
                SqlValue::Text(s) => match tsvector::parse(s) {
                    Ok(v) => Ok(SqlValue::Text(tsvector::render(&tsvector::strip(&v)))),
                    Err(e) => Err(e),
                },
                _ => Err(arity_err(name)),
            }
        }
        "ts_rank" | "ts_rank_cd" => match rank_call(name, args) {
            Ok(v) => Ok(v),
            Err(e) => Err(e),
        },
        "setweight" => {
            if args.len() != 2 {
                return Some(Err(arity_err(name)));
            }
            match (&args[0], &args[1]) {
                (SqlValue::Null, _) | (_, SqlValue::Null) => Ok(SqlValue::Null),
                (SqlValue::Text(vec_s), SqlValue::Text(w)) => match weight_letter(w) {
                    Some(letter) => match tsvector::parse(vec_s) {
                        Ok(entries) => Ok(SqlValue::Text(render_weighted(&entries, letter))),
                        Err(e) => Err(e),
                    },
                    None => Err(PgError::InvalidInputSyntax {
                        typ: "\"char\"",
                        input: w.clone(),
                    }),
                },
                _ => Err(arity_err(name)),
            }
        }
        "ts_headline" => {

            let (doc, qy) = match args.len() {
                2 => (&args[0], &args[1]),
                3 => (&args[1], &args[2]),
                _ => return Some(Err(arity_err(name))),
            };
            match (doc, qy) {
                (SqlValue::Null, _) | (_, SqlValue::Null) => Ok(SqlValue::Null),
                (SqlValue::Text(d), SqlValue::Text(q_s)) => match tsquery::parse(q_s) {
                    Ok(q) => Ok(SqlValue::Text(ts_headline(d, &q))),
                    Err(e) => Err(e),
                },
                _ => Err(arity_err(name)),
            }
        }
        _ => unreachable!(),
    })
}

const WEIGHT_D: f32 = 0.1;

const RANK_NORM: f32 = 1.64493406685;

fn word_distance(keylen: i32) -> f32 {
    if keylen > 100 {
        return 1e-30;
    }
    (1.0f32 / 1.05f32).powi(keylen)
}

fn query_operands(q: &TsQuery, out: &mut Vec<(String, bool)>) {
    match q {
        TsQuery::Val { lexeme, prefix } => {
            if !out.iter().any(|(l, _)| l == lexeme) {
                out.push((lexeme.clone(), *prefix));
            }
        }
        TsQuery::Not(a) => query_operands(a, out),
        TsQuery::And(a, b) | TsQuery::Or(a, b) | TsQuery::Phrase(a, b, _) => {
            query_operands(a, out);
            query_operands(b, out);
        }
    }
}

fn has_phrase(q: &TsQuery) -> bool {
    match q {
        TsQuery::Val { .. } => false,
        TsQuery::Not(a) => has_phrase(a),
        TsQuery::Phrase(..) => true,
        TsQuery::And(a, b) | TsQuery::Or(a, b) => has_phrase(a) || has_phrase(b),
    }
}

fn operand_positions(entries: &[tsvector::Entry], lexeme: &str, prefix: bool) -> Option<Vec<u32>> {
    let mut present = false;
    let mut ps: Vec<u32> = Vec::new();
    for (l, positions) in entries {
        let hit = if prefix {
            l.starts_with(lexeme)
        } else {
            l == lexeme
        };
        if hit {
            present = true;
            ps.extend(positions.iter().copied());
        }
    }
    if !present {
        return None;
    }
    ps.sort_unstable();
    ps.dedup();

    if ps.is_empty() {
        ps.push(0);
    }
    Some(ps)
}

fn calc_rank_or(entries: &[tsvector::Entry], ops: &[(String, bool)]) -> f32 {
    let mut res = 0.0f32;
    for (lexeme, prefix) in ops {
        if let Some(post) = operand_positions(entries, lexeme, *prefix) {
            let mut resj = 0.0f32;
            for j in 0..post.len() as i32 {
                resj += WEIGHT_D / (((j + 1) * (j + 1)) as f32);
            }
            res += resj / RANK_NORM;
        }
    }
    if !ops.is_empty() {
        res /= ops.len() as f32;
    }
    res
}

fn calc_rank_and(entries: &[tsvector::Entry], ops: &[(String, bool)]) -> f32 {
    if ops.len() < 2 {
        return calc_rank_or(entries, ops);
    }
    let pos: Vec<Option<Vec<u32>>> = ops
        .iter()
        .map(|(l, p)| operand_positions(entries, l, *p))
        .collect();
    let mut res = 0.0f32;
    for i in 0..ops.len() {
        let Some(posi) = &pos[i] else { continue };
        for posk in pos.iter().take(i).flatten() {
            for &l in posi {
                for &p in posk {
                    let mut dist = (l as i32 - p as i32).abs();
                    if dist == 0 {
                        dist = 16384;
                    }
                    let curw = (WEIGHT_D * WEIGHT_D * word_distance(dist)).sqrt();
                    res = 1.0 - (1.0 - res) * (1.0 - curw);
                }
            }
        }
    }
    res
}

fn ts_rank(entries: &[tsvector::Entry], q: &TsQuery) -> f32 {
    if entries.is_empty() {
        return 0.0;
    }
    let mut ops: Vec<(String, bool)> = Vec::new();
    query_operands(q, &mut ops);
    if ops.is_empty() {
        return 0.0;
    }
    let and = has_phrase(q) || matches!(q, TsQuery::And(..));
    let res = if and {
        calc_rank_and(entries, &ops)
    } else {
        calc_rank_or(entries, &ops)
    };
    if res < 0.0 {
        1e-20
    } else {
        res
    }
}

const NORM_LOGLENGTH: i32 = 0x01;
const NORM_LENGTH: i32 = 0x02;
const NORM_EXTDIST: i32 = 0x04;
const NORM_UNIQ: i32 = 0x08;
const NORM_LOGUNIQ: i32 = 0x10;
const NORM_RDIVRPLUS1: i32 = 0x20;

const DEFAULT_WEIGHTS: [f32; 4] = [0.1, 0.2, 0.4, 1.0];

fn parse_weights(name: &str, s: &str) -> Result<[f32; 4], PgError> {
    let inner = s.trim();
    let inner = inner
        .strip_prefix('{')
        .and_then(|x| x.strip_suffix('}'))
        .ok_or_else(|| arity_err(name))?;
    let parts: Vec<&str> = inner.split(',').map(|p| p.trim()).collect();
    if parts.len() != 4 {
        return Err(arity_err(name));
    }
    let mut w = [0.0f32; 4];
    for (i, p) in parts.iter().enumerate() {
        w[i] = p.parse::<f32>().map_err(|_| arity_err(name))?;
    }
    Ok(w)
}

fn rank_call(name: &str, args: &[SqlValue]) -> Result<SqlValue, PgError> {

    let mut rest = args;
    let mut weights = DEFAULT_WEIGHTS;
    if let Some(SqlValue::Text(s)) = rest.first() {
        if s.starts_with('{') {
            weights = parse_weights(name, s)?;
            rest = &rest[1..];
        }
    }

    let norm = match rest.len() {
        2 => 0i32,
        3 => match &rest[2] {
            SqlValue::Int(n) => *n as i32,
            SqlValue::Null => return Ok(SqlValue::Null),
            _ => return Err(arity_err(name)),
        },
        _ => return Err(arity_err(name)),
    };
    match (&rest[0], &rest[1]) {
        (SqlValue::Null, _) | (_, SqlValue::Null) => Ok(SqlValue::Null),
        (SqlValue::Text(vec_s), SqlValue::Text(q_s)) => {
            let entries = tsvector::parse(vec_s)?;
            let q = tsquery::parse(q_s)?;
            let raw = if name == "ts_rank_cd" {
                calc_rank_cd(&entries, &q, &weights, norm)
            } else {
                ts_rank(&entries, &q)
            };
            let normed = apply_norm(raw, &entries, norm, name == "ts_rank_cd");
            Ok(SqlValue::Real(normed as f64))
        }
        _ => Err(arity_err(name)),
    }
}

fn cnt_length(entries: &[tsvector::Entry]) -> i32 {
    entries
        .iter()
        .map(|(_, p)| if p.is_empty() { 1 } else { p.len() as i32 })
        .sum()
}

fn apply_norm(mut res: f32, entries: &[tsvector::Entry], method: i32, cd: bool) -> f32 {
    let size = entries.len() as f32;
    let len = cnt_length(entries) as f32;
    if method & NORM_LOGLENGTH != 0 && size > 0.0 {
        let denom = if cd {
            ((len + 1.0) as f64).ln() as f32
        } else {
            (((len + 1.0) as f64).ln() / std::f64::consts::LN_2) as f32
        };
        if denom != 0.0 {
            res /= denom;
        }
    }
    if method & NORM_LENGTH != 0 && len > 0.0 {
        res /= len;
    }
    if method & NORM_UNIQ != 0 && size > 0.0 {
        res /= size;
    }
    if method & NORM_LOGUNIQ != 0 && size > 0.0 {
        res /= (((size + 1.0) as f64).ln() / std::f64::consts::LN_2) as f32;
    }
    res
}

struct DocEntry {
    pos: i32,
    wclass: usize,
    ops: Vec<usize>,
}

struct CoverExt {
    scan: usize,
    p: i32,
    q: i32,
    begin: usize,
    end: usize,
}

fn exec_exists(
    q: &TsQuery,
    exists: &[bool],
    operand_of: &dyn Fn(&str, bool) -> usize,
    calc_not: bool,
) -> bool {
    match q {
        TsQuery::Val { lexeme, prefix } => exists[operand_of(lexeme, *prefix)],
        TsQuery::Not(a) => {
            if calc_not {
                !exec_exists(a, exists, operand_of, calc_not)
            } else {
                true
            }
        }
        TsQuery::And(a, b) | TsQuery::Phrase(a, b, _) => {
            exec_exists(a, exists, operand_of, calc_not)
                && exec_exists(b, exists, operand_of, calc_not)
        }
        TsQuery::Or(a, b) => {
            exec_exists(a, exists, operand_of, calc_not)
                || exec_exists(b, exists, operand_of, calc_not)
        }
    }
}

fn cover(
    doc: &[DocEntry],
    nops: usize,
    q: &TsQuery,
    ext: &mut CoverExt,
    operand_of: &dyn Fn(&str, bool) -> usize,
) -> bool {
    loop {

        let mut exists = vec![false; nops];
        let mut found = false;
        let mut lastpos = ext.scan;
        let mut end_idx = 0usize;
        let mut qq = 0i32;
        let mut i = ext.scan;
        while i < doc.len() {
            for &o in &doc[i].ops {
                exists[o] = true;
            }
            if exec_exists(q, &exists, operand_of, false) {
                qq = doc[i].pos;
                end_idx = i;
                lastpos = i;
                found = true;
                break;
            }
            i += 1;
        }
        if !found {
            return false;
        }

        for e in exists.iter_mut() {
            *e = false;
        }
        let mut begin_idx = lastpos;
        let mut p = i32::MAX;
        let mut j = lastpos as isize;
        while j >= ext.scan as isize {
            let ju = j as usize;
            for &o in &doc[ju].ops {
                exists[o] = true;
            }
            if exec_exists(q, &exists, operand_of, true) {
                p = doc[ju].pos;
                begin_idx = ju;
                break;
            }
            j -= 1;
        }
        if p <= qq {
            ext.p = p;
            ext.q = qq;
            ext.begin = begin_idx;
            ext.end = end_idx;
            ext.scan = begin_idx + 1;
            return true;
        }
        ext.scan += 1;
    }
}

fn calc_rank_cd(entries: &[tsvector::Entry], q: &TsQuery, weights: &[f32; 4], method: i32) -> f32 {
    let mut ops: Vec<(String, bool)> = Vec::new();
    query_operands(q, &mut ops);
    if ops.is_empty() || entries.is_empty() {
        return 0.0;
    }

    let ops_ref = ops.clone();
    let operand_of = move |lex: &str, prefix: bool| -> usize {
        ops_ref
            .iter()
            .position(|(l, p)| l == lex && *p == prefix)
            .unwrap_or(0)
    };

    let mut doc: Vec<DocEntry> = Vec::new();
    for (lex, positions) in entries {

        let matched: Vec<usize> = ops
            .iter()
            .enumerate()
            .filter(|(_, (ol, op))| {
                if *op {
                    lex.starts_with(ol.as_str())
                } else {
                    lex == ol
                }
            })
            .map(|(idx, _)| idx)
            .collect();
        if matched.is_empty() {
            continue;
        }
        for &pos in positions {
            doc.push(DocEntry {
                pos: pos as i32,
                wclass: 0,
                ops: matched.clone(),
            });
        }
    }
    if doc.is_empty() {
        return 0.0;
    }
    doc.sort_by_key(|d| d.pos);

    let invws: Vec<f64> = weights.iter().map(|&w| 1.0 / (w as f64)).collect();

    let mut ext = CoverExt {
        scan: 0,
        p: 0,
        q: 0,
        begin: 0,
        end: 0,
    };
    let mut wdoc = 0.0f64;
    let mut sum_dist = 0.0f64;
    let mut prev_ext_pos = 0.0f64;
    let mut n_extent = 0i32;

    while cover(&doc, ops.len(), q, &mut ext, &operand_of) {
        let mut inv_sum = 0.0f64;
        for d in &doc[ext.begin..=ext.end] {
            inv_sum += invws[d.wclass];
        }
        let cpos = (ext.end - ext.begin + 1) as f64 / inv_sum;
        let mut n_noise = (ext.q - ext.p) - (ext.end as i32 - ext.begin as i32);
        if n_noise < 0 {
            n_noise = (ext.end as i32 - ext.begin as i32) / 2;
        }
        wdoc += cpos / (1.0 + n_noise as f64);

        let cur_ext_pos = (ext.q + ext.p) as f64 / 2.0;
        if n_extent > 0 && cur_ext_pos > prev_ext_pos {
            sum_dist += 1.0 / (cur_ext_pos - prev_ext_pos);
        }
        prev_ext_pos = cur_ext_pos;
        n_extent += 1;
    }

    if method & NORM_EXTDIST != 0 && n_extent > 0 && sum_dist > 0.0 {
        wdoc /= n_extent as f64 / sum_dist;
    }
    if method & NORM_RDIVRPLUS1 != 0 {
        wdoc /= n_extent as f64 + 1.0;
    }
    wdoc as f32
}

fn weight_letter(w: &str) -> Option<char> {
    let mut chars = w.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) => match c.to_ascii_uppercase() {
            up @ ('A' | 'B' | 'C' | 'D') => Some(up),
            _ => None,
        },
        _ => None,
    }
}

fn render_weighted(entries: &[tsvector::Entry], letter: char) -> String {
    let mut out = String::new();
    for (i, (lex, positions)) in entries.iter().enumerate() {
        if i > 0 {
            out.push(' ');
        }
        out.push('\'');
        for c in lex.chars() {
            match c {
                '\'' => out.push_str("''"),
                '\\' => out.push_str("\\\\"),
                _ => out.push(c),
            }
        }
        out.push('\'');
        if !positions.is_empty() {
            out.push(':');
            for (j, p) in positions.iter().enumerate() {
                if j > 0 {
                    out.push(',');
                }
                out.push_str(&p.to_string());
                if letter != 'D' {
                    out.push(letter);
                }
            }
        }
    }
    out
}

fn ts_headline(doc: &str, q: &TsQuery) -> String {
    let mut ops: Vec<(String, bool)> = Vec::new();
    positive_operands(q, &mut ops);
    let mut out = String::new();
    let mut word = String::new();
    let flush = |word: &mut String, out: &mut String| {
        if word.is_empty() {
            return;
        }
        let lower = word.to_lowercase();
        let hit = ops.iter().any(|(l, p)| {
            if *p {
                lower.starts_with(l.as_str())
            } else {
                lower == *l
            }
        });
        if hit {
            out.push_str("<b>");
            out.push_str(word);
            out.push_str("</b>");
        } else {
            out.push_str(word);
        }
        word.clear();
    };
    for ch in doc.chars() {
        if ch.is_alphanumeric() {
            word.push(ch);
        } else {
            flush(&mut word, &mut out);
            out.push(ch);
        }
    }
    flush(&mut word, &mut out);
    out
}

fn positive_operands(q: &TsQuery, out: &mut Vec<(String, bool)>) {
    match q {
        TsQuery::Val { lexeme, prefix } => {
            if !out.iter().any(|(l, _)| l == lexeme) {
                out.push((lexeme.clone(), *prefix));
            }
        }
        TsQuery::Not(_) => {}
        TsQuery::And(a, b) | TsQuery::Or(a, b) | TsQuery::Phrase(a, b, _) => {
            positive_operands(a, out);
            positive_operands(b, out);
        }
    }
}

fn join_words(text: &str, phrase: bool) -> String {
    let entries = tsvector::tokenize(text);

    let mut occ: Vec<(u32, String)> = Vec::new();
    for (lex, positions) in &entries {
        for p in positions {
            occ.push((*p, lex.clone()));
        }
    }
    occ.sort_by_key(|(p, _)| *p);
    let words: Vec<String> = occ.into_iter().map(|(_, l)| l).collect();
    if words.is_empty() {
        return String::new();
    }
    let mut q = tsquery::TsQuery::Val {
        lexeme: words[0].clone(),
        prefix: false,
    };
    for w in &words[1..] {
        let rhs = tsquery::TsQuery::Val {
            lexeme: w.clone(),
            prefix: false,
        };
        q = if phrase {
            tsquery::TsQuery::Phrase(Box::new(q), Box::new(rhs), 1)
        } else {
            tsquery::TsQuery::And(Box::new(q), Box::new(rhs))
        };
    }
    tsquery::render(&q)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(name: &str, arg: &str) -> String {
        match call(name, &[SqlValue::Text(arg.into())]) {
            Some(Ok(SqlValue::Text(t))) => t,
            other => panic!("{name}({arg:?}) => {other:?}"),
        }
    }

    #[test]
    fn tsvector_and_query() {
        assert_eq!(s("to_tsvector", "a fat cat"), "'a':1 'cat':3 'fat':2");
        assert_eq!(s("to_tsquery", "fat & cat"), "'fat' & 'cat'");
        assert_eq!(
            s("plainto_tsquery", "The Fat Cats"),
            "'the' & 'fat' & 'cats'"
        );
        assert_eq!(
            s("phraseto_tsquery", "The Fat Cats"),
            "'the' <-> 'fat' <-> 'cats'"
        );
    }

    #[test]
    fn numnode_and_strip() {
        assert_eq!(
            call("numnode", &[SqlValue::Text("foo & bar".into())]),
            Some(Ok(SqlValue::Int(3)))
        );
        assert_eq!(s("strip", "'a':1 'cat':3 'fat':2"), "'a' 'cat' 'fat'");
    }

    #[test]
    fn config_arg_ignored() {
        assert_eq!(
            call(
                "to_tsvector",
                &[
                    SqlValue::Text("simple".into()),
                    SqlValue::Text("a fat".into())
                ]
            ),
            Some(Ok(SqlValue::Text("'a':1 'fat':2".into())))
        );
    }

    fn rank(vec: &str, q: &str) -> f32 {
        let entries = tsvector::parse(vec).unwrap();
        let query = tsquery::parse(q).unwrap();
        super::ts_rank(&entries, &query)
    }

    #[test]
    fn ts_rank_frequency_and_nonmatch() {

        assert_eq!(rank("'a':1 'cat':3 'fat':2", "dog"), 0.0);

        assert!(rank("'a':1 'cat':3 'fat':2", "cat") > 0.0);
        assert!(rank("'cat':1,2,3", "cat") > rank("'cat':2", "cat"));

        assert!(rank("'a':1 'cat':3 'fat':2", "fat & cat") > rank("'a':1 'cat':3 'fat':2", "cat"));

        assert!((rank("'a':1 'cat':3 'fat':2", "cat") - 0.1 / 1.64493406685).abs() < 1e-6);
    }

    fn rank_cd(vec: &str, q: &str) -> f32 {
        let entries = tsvector::parse(vec).unwrap();
        let query = tsquery::parse(q).unwrap();
        super::calc_rank_cd(&entries, &query, &super::DEFAULT_WEIGHTS, 0)
    }

    #[test]
    fn ts_rank_cd_covers_and_ordering() {

        assert_eq!(rank_cd("'a':1 'cat':3 'fat':2", "dog"), 0.0);

        assert!((rank_cd("'fat':1 'cat':2", "fat & cat") - 0.1).abs() < 1e-6);

        assert!((rank_cd("'fat':1 'the':2 'cat':3", "fat & cat") - 0.05).abs() < 1e-6);

        assert!(rank_cd("'fat':1 'cat':2", "fat & cat") > rank_cd("'fat':1 'cat':9", "fat & cat"));

        assert!((rank_cd("'a':1,5 'b':2,6", "a & b") - 0.233333).abs() < 1e-4);

        assert_eq!(rank_cd("'fat' 'cat'", "fat & cat"), 0.0);

        assert!((rank_cd("'cat':1,4,9", "cat") - 0.3).abs() < 1e-6);

        assert!(rank_cd("'fat':1 'cat':2", "fat | dog") > 0.0);
    }

    #[test]
    fn ts_rank_cd_dispatch_and_norm() {
        let call2 = |a: &str, b: &str| match call(
            "ts_rank_cd",
            &[SqlValue::Text(a.into()), SqlValue::Text(b.into())],
        ) {
            Some(Ok(SqlValue::Real(r))) => r,
            other => panic!("{other:?}"),
        };
        assert!((call2("'fat':1 'cat':2", "fat & cat") - 0.1).abs() < 1e-6);

        assert_eq!(
            call(
                "ts_rank_cd",
                &[SqlValue::Null, SqlValue::Text("cat".into())]
            ),
            Some(Ok(SqlValue::Null))
        );

        let normed = match call(
            "ts_rank_cd",
            &[
                SqlValue::Text("'fat':1 'cat':2".into()),
                SqlValue::Text("fat & cat".into()),
                SqlValue::Int(2),
            ],
        ) {
            Some(Ok(SqlValue::Real(r))) => r,
            other => panic!("{other:?}"),
        };
        assert!((normed - 0.05).abs() < 1e-6);

        let weighted = match call(
            "ts_rank_cd",
            &[
                SqlValue::Text("{0.1,0.2,0.4,1.0}".into()),
                SqlValue::Text("'fat':1 'cat':2".into()),
                SqlValue::Text("fat & cat".into()),
            ],
        ) {
            Some(Ok(SqlValue::Real(r))) => r,
            other => panic!("{other:?}"),
        };
        assert!((weighted - 0.1).abs() < 1e-6);
    }

    #[test]
    fn setweight_and_headline_calls() {
        let two = |name: &str, a: &str, b: &str| match call(
            name,
            &[SqlValue::Text(a.into()), SqlValue::Text(b.into())],
        ) {
            Some(Ok(SqlValue::Text(t))) => t,
            other => panic!("{name} => {other:?}"),
        };
        assert_eq!(two("setweight", "'a':1 'fat':2", "A"), "'a':1A 'fat':2A");
        assert_eq!(two("setweight", "'a':1 'fat':2", "d"), "'a':1 'fat':2");
        assert_eq!(two("setweight", "'a' 'fat'", "B"), "'a' 'fat'");
        assert!(matches!(
            call(
                "setweight",
                &[SqlValue::Text("'a':1".into()), SqlValue::Text("X".into())]
            ),
            Some(Err(PgError::InvalidInputSyntax { .. }))
        ));

        assert_eq!(
            match call(
                "ts_headline",
                &[
                    SqlValue::Text("simple".into()),
                    SqlValue::Text("The Fat Cat".into()),
                    SqlValue::Text("fat".into()),
                ],
            ) {
                Some(Ok(SqlValue::Text(t))) => t,
                other => panic!("{other:?}"),
            },
            "The <b>Fat</b> Cat"
        );
    }

    #[test]
    fn tsvector_length_counts_distinct_lexemes() {

        assert_eq!(tsvector_length("'a':1 'cat':3 'fat':2 'sat':4"), Some(4));
        assert_eq!(tsvector_length("'cat':1,2,3"), Some(1));

        assert_eq!(tsvector_length("'a' 'cat' 'fat'"), Some(3));

        assert_eq!(tsvector_length("hello"), None);
        assert_eq!(tsvector_length("café"), None);
        assert_eq!(tsvector_length("a fat cat"), None);
    }

    #[test]
    fn malformed_query_errors() {
        assert!(matches!(
            call("to_tsquery", &[SqlValue::Text("& foo".into())]),
            Some(Err(PgError::TsquerySyntax { .. }))
        ));
    }
}
