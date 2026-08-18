
#![allow(unused_variables)]
use crate::Value;

#[derive(Clone, Debug)]
enum Json {
    Null,
    Bool(bool),
    Int(i64),
    Real(f64),
    Str(String),
    Arr(Vec<Json>),
    Obj(Vec<(String, Json)>),
}

pub fn table_valued(
    name: &str,
    arg: &Value,
) -> Option<Result<(Vec<String>, Vec<Vec<Value>>), String>> {
    let cols: Vec<String> = [
        "key", "value", "type", "atom", "id", "parent", "fullkey", "path",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    let doc = match arg {
        Value::Null => return Some(Ok((cols, Vec::new()))),
        v => match doc_of(Some(v)) {
            Some(j) => j,
            None => return Some(Err("malformed JSON".into())),
        },
    };
    let mut rows: Vec<Vec<Value>> = Vec::new();
    match name {
        "JSON_EACH" => {
            match &doc {
                Json::Arr(items) => {
                    for (i, el) in items.iter().enumerate() {
                        push_node(
                            &mut rows,
                            Value::Int(i as i64),
                            el,
                            Value::Null,
                            format!("$[{i}]"),
                            "$".into(),
                            i as i64,
                        );
                    }
                }
                Json::Obj(pairs) => {
                    for (i, (k, el)) in pairs.iter().enumerate() {
                        push_node(
                            &mut rows,
                            Value::Text(k.clone()),
                            el,
                            Value::Null,
                            format!("$.{k}"),
                            "$".into(),
                            i as i64,
                        );
                    }
                }
                other => push_node(
                    &mut rows,
                    Value::Null,
                    other,
                    Value::Null,
                    "$".into(),
                    "$".into(),
                    0,
                ),
            }
            Some(Ok((cols, rows)))
        }
        "JSON_TREE" => {
            let mut id = 0i64;
            tree(
                &mut rows,
                Value::Null,
                &doc,
                Value::Null,
                "$".to_string(),
                "$".to_string(),
                &mut id,
            );
            Some(Ok((cols, rows)))
        }
        _ => None,
    }
}

fn val_type(j: &Json) -> (Value, &'static str, Value) {
    match j {
        Json::Null => (Value::Null, "null", Value::Null),
        Json::Bool(true) => (Value::Int(1), "true", Value::Int(1)),
        Json::Bool(false) => (Value::Int(0), "false", Value::Int(0)),
        Json::Int(i) => (Value::Int(*i), "integer", Value::Int(*i)),
        Json::Real(r) => (Value::Real(*r), "real", Value::Real(*r)),
        Json::Str(s) => (Value::Text(s.clone()), "text", Value::Text(s.clone())),
        Json::Arr(_) => (Value::Text(serialize(j)), "array", Value::Null),
        Json::Obj(_) => (Value::Text(serialize(j)), "object", Value::Null),
    }
}

#[allow(clippy::too_many_arguments)]
fn push_node(
    rows: &mut Vec<Vec<Value>>,
    key: Value,
    j: &Json,
    parent: Value,
    fullkey: String,
    path: String,
    id: i64,
) {
    let (value, typ, atom) = val_type(j);
    rows.push(vec![
        key,
        value,
        Value::Text(typ.into()),
        atom,
        Value::Int(id),
        parent,
        Value::Text(fullkey),
        Value::Text(path),
    ]);
}

fn tree(
    rows: &mut Vec<Vec<Value>>,
    key: Value,
    j: &Json,
    parent: Value,
    fullkey: String,
    path: String,
    id: &mut i64,
) {
    let my = *id;
    *id += 1;
    push_node(rows, key, j, parent, fullkey.clone(), path, my);
    match j {
        Json::Arr(items) => {
            for (i, el) in items.iter().enumerate() {
                tree(
                    rows,
                    Value::Int(i as i64),
                    el,
                    Value::Int(my),
                    format!("{fullkey}[{i}]"),
                    fullkey.clone(),
                    id,
                );
            }
        }
        Json::Obj(pairs) => {
            for (k, el) in pairs {
                tree(
                    rows,
                    Value::Text(k.clone()),
                    el,
                    Value::Int(my),
                    format!("{fullkey}.{k}"),
                    fullkey.clone(),
                    id,
                );
            }
        }
        _ => {}
    }
}

pub fn call(name: &str, args: &[Value]) -> Option<Result<Value, String>> {
    let upper = name.to_uppercase();
    match upper.as_str() {
        "JSON" => Some(json_normalize(args)),
        "JSON_EXTRACT" => Some(json_extract(args)),
        "JSON_OBJECT" => Some(json_object(args)),
        "JSON_ARRAY" => Some(json_array(args)),
        "JSON_VALID" => Some(json_valid(args)),
        "JSON_TYPE" => Some(json_type(args)),
        "JSON_SET" => Some(json_edit(args, SetMode::Set)),
        "JSON_INSERT" => Some(json_edit(args, SetMode::Insert)),
        "JSON_REPLACE" => Some(json_edit(args, SetMode::Replace)),
        "JSON_REMOVE" => Some(json_remove(args)),
        "JSON_QUOTE" => Some(json_quote(args)),
        "JSON_ARRAY_LENGTH" => Some(json_array_length(args)),
        "JSON_PATCH" => Some(json_patch(args)),
        "JSON_PRETTY" => Some(json_pretty(args)),
        "JSON_ERROR_POSITION" => Some(json_error_position(args)),

        "JSONB" => Some(jsonb_normalize(args)),
        "JSONB_ARRAY" => Some(json_array(args).map(to_jsonb)),
        "JSONB_OBJECT" => Some(json_object(args).map(to_jsonb)),
        "JSONB_EXTRACT" => Some(jsonb_extract(args)),
        "JSONB_SET" => Some(json_edit(args, SetMode::Set).map(to_jsonb)),
        "JSONB_INSERT" => Some(json_edit(args, SetMode::Insert).map(to_jsonb)),
        "JSONB_REPLACE" => Some(json_edit(args, SetMode::Replace).map(to_jsonb)),
        "JSONB_REMOVE" => Some(json_remove(args).map(to_jsonb)),
        "JSONB_PATCH" => Some(json_patch(args).map(to_jsonb)),
        "->" => Some(arrow(args, false)),
        "->>" => Some(arrow(args, true)),
        _ => None,
    }
}

fn to_jsonb(v: Value) -> Value {
    match &v {
        Value::Text(s) => text_to_jsonb(s).map(Value::Blob).unwrap_or(v),
        _ => v,
    }
}

fn jsonb_normalize(args: &[Value]) -> Result<Value, String> {
    match args.first() {
        Some(Value::Null) | None => Ok(Value::Null),
        Some(Value::Blob(b)) => {
            if jsonb_to_json(b).is_some() {
                Ok(Value::Blob(b.clone()))
            } else {
                Err("malformed JSON".to_string())
            }
        }
        Some(v) => text_to_jsonb(&crate::text_of(v))
            .map(Value::Blob)
            .ok_or_else(|| "malformed JSON".to_string()),
    }
}

fn jsonb_extract(args: &[Value]) -> Result<Value, String> {
    if args.len() < 2 {
        return Err("jsonb_extract() needs a document and at least one path".to_string());
    }
    let doc = doc_of(args.first()).ok_or("malformed JSON")?;
    if args.len() == 2 {
        let path = crate::text_of(&args[1]);
        return Ok(match navigate(&doc, &path) {
            Some(j @ (Json::Arr(_) | Json::Obj(_))) => {
                let mut b = Vec::new();
                json_to_jsonb(j, &mut b);
                Value::Blob(b)
            }
            Some(j) => json_to_sql(j),
            None => Value::Null,
        });
    }
    let mut out = Vec::new();
    for p in &args[1..] {
        out.push(
            navigate(&doc, &crate::text_of(p))
                .cloned()
                .unwrap_or(Json::Null),
        );
    }
    let mut b = Vec::new();
    json_to_jsonb(&Json::Arr(out), &mut b);
    Ok(Value::Blob(b))
}

fn json_array_length(args: &[Value]) -> Result<Value, String> {
    if matches!(args.first(), Some(Value::Null) | None) {
        return Ok(Value::Null);
    }
    let doc = doc_of(args.first()).ok_or("malformed JSON")?;
    let target = if args.len() >= 2 {
        let path = crate::text_of(&args[1]);
        match navigate(&doc, &path) {
            Some(j) => j,
            None => return Ok(Value::Null),
        }
    } else {
        &doc
    };
    Ok(Value::Int(match target {
        Json::Arr(items) => items.len() as i64,
        _ => 0,
    }))
}

fn json_patch(args: &[Value]) -> Result<Value, String> {
    if args.len() != 2 {
        return Err("json_patch() needs a target and a patch".to_string());
    }
    if args.iter().any(|v| matches!(v, Value::Null)) {
        return Ok(Value::Null);
    }
    let target = doc_of(args.first()).ok_or("malformed JSON")?;
    let patch = doc_of(args.get(1)).ok_or("malformed JSON")?;
    Ok(Value::Text(serialize(&merge_patch(target, patch))))
}

fn merge_patch(target: Json, patch: Json) -> Json {
    let Json::Obj(patch_pairs) = patch else {

        return patch;
    };

    let mut base = match target {
        Json::Obj(pairs) => pairs,
        _ => Vec::new(),
    };
    for (k, v) in patch_pairs {
        if matches!(v, Json::Null) {
            base.retain(|(bk, _)| *bk != k);
        } else if let Some(slot) = base.iter_mut().find(|(bk, _)| *bk == k) {
            let cur = std::mem::replace(&mut slot.1, Json::Null);
            slot.1 = merge_patch(cur, v);
        } else {

            base.push((k, merge_patch(Json::Obj(Vec::new()), v)));
        }
    }
    Json::Obj(base)
}

fn json_pretty(args: &[Value]) -> Result<Value, String> {
    if matches!(args.first(), Some(Value::Null) | None) {
        return Ok(Value::Null);
    }
    let doc = doc_of(args.first()).ok_or("malformed JSON")?;
    let indent = match args.get(1) {
        Some(v) if !matches!(v, Value::Null) => crate::text_of(v),
        _ => "    ".to_string(),
    };
    let mut out = String::new();
    write_pretty(&doc, &indent, 0, &mut out);
    Ok(Value::Text(out))
}

fn write_pretty(j: &Json, indent: &str, depth: usize, out: &mut String) {
    match j {
        Json::Arr(items) if !items.is_empty() => {
            out.push('[');
            for (n, item) in items.iter().enumerate() {
                if n > 0 {
                    out.push(',');
                }
                out.push('\n');
                for _ in 0..=depth {
                    out.push_str(indent);
                }
                write_pretty(item, indent, depth + 1, out);
            }
            out.push('\n');
            for _ in 0..depth {
                out.push_str(indent);
            }
            out.push(']');
        }
        Json::Obj(pairs) if !pairs.is_empty() => {
            out.push('{');
            for (n, (k, v)) in pairs.iter().enumerate() {
                if n > 0 {
                    out.push(',');
                }
                out.push('\n');
                for _ in 0..=depth {
                    out.push_str(indent);
                }
                write_string(k, out);
                out.push_str(": ");
                write_pretty(v, indent, depth + 1, out);
            }
            out.push('\n');
            for _ in 0..depth {
                out.push_str(indent);
            }
            out.push('}');
        }

        _ => write_json(j, out),
    }
}

fn json_error_position(args: &[Value]) -> Result<Value, String> {
    if matches!(args.first(), Some(Value::Null) | None) {
        return Ok(Value::Null);
    }
    let s = require_text(args.first())?;
    let chars: Vec<char> = s.chars().collect();
    let mut p = Parser { chars, i: 0 };
    p.skip_ws();
    let pos = match p.value() {
        Some(_) => {
            p.skip_ws();
            if p.i == p.chars.len() {
                0
            } else {
                p.i + 1
            }
        }
        None => p.i + 1,
    };
    Ok(Value::Int(pos as i64))
}

pub fn group_array(values: &[Value]) -> Value {
    let items = values.iter().map(value_to_json).collect();
    Value::Text(serialize(&Json::Arr(items)))
}

pub fn group_object(pairs: &[(Value, Value)]) -> Value {
    let obj = pairs
        .iter()
        .map(|(k, v)| (crate::text_of(k), value_to_json(v)))
        .collect();
    Value::Text(serialize(&Json::Obj(obj)))
}

#[derive(Clone, Copy, PartialEq)]
enum SetMode {
    Set,
    Insert,
    Replace,
}

fn json_edit(args: &[Value], mode: SetMode) -> Result<Value, String> {
    if args.len() < 3 || (args.len() - 1) % 2 != 0 {
        return Err("json_set/insert/replace need a document and path,value pairs".to_string());
    }
    if matches!(args.first(), Some(Value::Null)) {
        return Ok(Value::Null);
    }
    let mut doc = doc_of(args.first()).ok_or("malformed JSON")?;
    for pair in args[1..].chunks(2) {
        let steps = match parse_path(&crate::text_of(&pair[0])) {
            Some(s) if !s.is_empty() => s,
            _ => continue,
        };
        apply_set(&mut doc, &steps, value_to_json(&pair[1]), mode);
    }
    Ok(Value::Text(serialize(&doc)))
}

fn apply_set(root: &mut Json, steps: &[Step], val: Json, mode: SetMode) {
    if steps.len() == 1 {
        match (&steps[0], root) {
            (Step::Key(k), Json::Obj(pairs)) => {
                let pos = pairs.iter().position(|(pk, _)| pk == k);
                match (pos, mode) {
                    (Some(_), SetMode::Insert) => {}
                    (None, SetMode::Replace) => {}
                    (Some(p), _) => pairs[p].1 = val,
                    (None, _) => pairs.push((k.clone(), val)),
                }
            }
            (Step::Index(i), Json::Arr(items)) => {
                if *i < items.len() {
                    if mode != SetMode::Insert {
                        items[*i] = val;
                    }
                } else if mode != SetMode::Replace {
                    items.push(val);
                }
            }
            _ => {}
        }
        return;
    }
    match (&steps[0], root) {
        (Step::Key(k), Json::Obj(pairs)) => {
            if let Some(p) = pairs.iter_mut().find(|(pk, _)| pk == k) {
                apply_set(&mut p.1, &steps[1..], val, mode);
            }
        }
        (Step::Index(i), Json::Arr(items)) => {
            if let Some(item) = items.get_mut(*i) {
                apply_set(item, &steps[1..], val, mode);
            }
        }
        _ => {}
    }
}

fn json_remove(args: &[Value]) -> Result<Value, String> {
    if args.is_empty() {
        return Err("json_remove() needs a document".to_string());
    }
    if matches!(args.first(), Some(Value::Null)) {
        return Ok(Value::Null);
    }
    let mut doc = doc_of(args.first()).ok_or("malformed JSON")?;
    for p in &args[1..] {
        if let Some(steps) = parse_path(&crate::text_of(p)) {
            if !steps.is_empty() {
                apply_remove(&mut doc, &steps);
            }
        }
    }
    Ok(Value::Text(serialize(&doc)))
}

fn apply_remove(root: &mut Json, steps: &[Step]) {
    if steps.len() == 1 {
        match (&steps[0], root) {
            (Step::Key(k), Json::Obj(pairs)) => pairs.retain(|(pk, _)| pk != k),
            (Step::Index(i), Json::Arr(items)) => {
                if *i < items.len() {
                    items.remove(*i);
                }
            }
            _ => {}
        }
        return;
    }
    match (&steps[0], root) {
        (Step::Key(k), Json::Obj(pairs)) => {
            if let Some(p) = pairs.iter_mut().find(|(pk, _)| pk == k) {
                apply_remove(&mut p.1, &steps[1..]);
            }
        }
        (Step::Index(i), Json::Arr(items)) => {
            if let Some(item) = items.get_mut(*i) {
                apply_remove(item, &steps[1..]);
            }
        }
        _ => {}
    }
}

fn json_quote(args: &[Value]) -> Result<Value, String> {
    let j = match args.first() {
        Some(Value::Null) | None => Json::Null,
        Some(Value::Int(i)) => Json::Int(*i),
        Some(Value::Real(r)) => Json::Real(*r),
        Some(Value::Text(s)) => Json::Str(s.clone()),
        Some(Value::Blob(b)) => Json::Str(String::from_utf8_lossy(b).into_owned()),
    };
    Ok(Value::Text(serialize(&j)))
}

fn json_normalize(args: &[Value]) -> Result<Value, String> {

    if let Some(Value::Blob(b)) = args.first() {
        return jsonb_to_text(b)
            .map(Value::Text)
            .ok_or_else(|| "malformed JSON".to_string());
    }
    match doc_of(args.first()) {
        Some(j) => Ok(Value::Text(serialize(&j))),
        None => Err("malformed JSON".to_string()),
    }
}

fn json_extract(args: &[Value]) -> Result<Value, String> {
    if args.len() < 2 {
        return Err("json_extract() needs a document and at least one path".to_string());
    }
    let doc = doc_of(args.first()).ok_or("malformed JSON")?;

    if args.len() == 2 {
        let path = crate::text_of(&args[1]);
        return Ok(match navigate(&doc, &path) {
            Some(j) => json_to_sql(j),
            None => Value::Null,
        });
    }

    let mut out = Vec::new();
    for p in &args[1..] {
        let path = crate::text_of(p);
        out.push(navigate(&doc, &path).cloned().unwrap_or(Json::Null));
    }
    Ok(Value::Text(serialize(&Json::Arr(out))))
}

fn json_object(args: &[Value]) -> Result<Value, String> {
    if args.len() % 2 != 0 {
        return Err("json_object() requires an even number of arguments".to_string());
    }
    let mut pairs = Vec::new();
    for chunk in args.chunks(2) {
        let key = crate::text_of(&chunk[0]);
        pairs.push((key, value_to_json(&chunk[1])));
    }
    Ok(Value::Text(serialize(&Json::Obj(pairs))))
}

fn json_array(args: &[Value]) -> Result<Value, String> {
    let items = args.iter().map(value_to_json).collect();
    Ok(Value::Text(serialize(&Json::Arr(items))))
}

fn json_valid(args: &[Value]) -> Result<Value, String> {
    let ok = match args.first() {
        Some(Value::Null) | None => return Ok(Value::Null),

        Some(Value::Blob(_)) => false,
        Some(v) => parse(&crate::text_of(v)).is_some(),
    };
    Ok(Value::Int(if ok { 1 } else { 0 }))
}

fn json_type(args: &[Value]) -> Result<Value, String> {
    let doc = doc_of(args.first()).ok_or("malformed JSON")?;
    let target = if args.len() >= 2 {
        let path = crate::text_of(&args[1]);
        match navigate(&doc, &path) {
            Some(j) => j,
            None => return Ok(Value::Null),
        }
    } else {
        &doc
    };
    Ok(Value::Text(type_name(target).to_string()))
}

fn arrow(args: &[Value], as_sql_text: bool) -> Result<Value, String> {
    if args.len() != 2 {
        return Err("-> / ->> require two arguments".to_string());
    }
    if matches!(args.first(), Some(Value::Null) | None) {
        return Ok(Value::Null);
    }
    let doc = match doc_of(args.first()) {
        Some(j) => j,
        None => return Ok(Value::Null),
    };
    let path = arrow_path(&args[1]);
    match navigate(&doc, &path) {
        None => Ok(Value::Null),
        Some(j) => {
            if as_sql_text {

                Ok(json_to_sql(j))
            } else {

                Ok(Value::Text(serialize(j)))
            }
        }
    }
}

fn arrow_path(v: &Value) -> String {
    match v {
        Value::Int(i) => format!("$[{}]", i),
        other => {
            let t = crate::text_of(other);
            if t.starts_with('$') {
                t
            } else {
                format!("$.{}", t)
            }
        }
    }
}

fn value_to_json(v: &Value) -> Json {
    match v {
        Value::Null => Json::Null,
        Value::Int(i) => Json::Int(*i),
        Value::Real(r) => Json::Real(*r),
        Value::Text(s) => {

            match parse(s) {
                Some(j @ (Json::Arr(_) | Json::Obj(_))) => j,
                _ => Json::Str(s.clone()),
            }
        }
        Value::Blob(b) => Json::Str(String::from_utf8_lossy(b).into_owned()),
    }
}

fn json_to_sql(j: &Json) -> Value {
    match j {
        Json::Null => Value::Null,
        Json::Bool(b) => Value::Int(if *b { 1 } else { 0 }),
        Json::Int(i) => Value::Int(*i),
        Json::Real(r) => Value::Real(*r),
        Json::Str(s) => Value::Text(s.clone()),
        Json::Arr(_) | Json::Obj(_) => Value::Text(serialize(j)),
    }
}

fn type_name(j: &Json) -> &'static str {
    match j {
        Json::Null => "null",
        Json::Bool(true) => "true",
        Json::Bool(false) => "false",
        Json::Int(_) => "integer",
        Json::Real(_) => "real",
        Json::Str(_) => "text",
        Json::Arr(_) => "array",
        Json::Obj(_) => "object",
    }
}

fn require_text(v: Option<&Value>) -> Result<String, String> {
    match v {
        Some(v) => Ok(crate::text_of(v)),
        None => Err("missing argument".to_string()),
    }
}

enum Step {
    Key(String),
    Index(usize),
}

fn navigate<'a>(root: &'a Json, path: &str) -> Option<&'a Json> {
    let steps = parse_path(path)?;
    let mut cur = root;
    for step in steps {
        match (step, cur) {
            (Step::Key(k), Json::Obj(pairs)) => {
                cur = &pairs.iter().find(|(pk, _)| *pk == k)?.1;
            }
            (Step::Index(i), Json::Arr(items)) => {
                cur = items.get(i)?;
            }
            _ => return None,
        }
    }
    Some(cur)
}

fn parse_path(path: &str) -> Option<Vec<Step>> {
    let chars: Vec<char> = path.chars().collect();
    let mut i = 0;
    if chars.get(i) != Some(&'$') {
        return None;
    }
    i += 1;
    let mut steps = Vec::new();
    while i < chars.len() {
        match chars[i] {
            '.' => {
                i += 1;
                let start = i;
                while i < chars.len() && chars[i] != '.' && chars[i] != '[' {
                    i += 1;
                }
                if i == start {
                    return None;
                }
                steps.push(Step::Key(chars[start..i].iter().collect()));
            }
            '[' => {
                i += 1;
                let start = i;
                while i < chars.len() && chars[i] != ']' {
                    i += 1;
                }
                if i >= chars.len() {
                    return None;
                }
                let inner: String = chars[start..i].iter().collect();
                i += 1;
                let idx: usize = inner.trim().parse().ok()?;
                steps.push(Step::Index(idx));
            }
            _ => return None,
        }
    }
    Some(steps)
}

mod jb {
    pub const NULL: u8 = 0;
    pub const TRUE: u8 = 1;
    pub const FALSE: u8 = 2;
    pub const INT: u8 = 3;
    pub const INT5: u8 = 4;
    pub const FLOAT: u8 = 5;
    pub const FLOAT5: u8 = 6;
    pub const TEXT: u8 = 7;
    pub const TEXTJ: u8 = 8;
    pub const TEXT5: u8 = 9;
    pub const TEXTRAW: u8 = 10;
    pub const ARRAY: u8 = 11;
    pub const OBJECT: u8 = 12;
}

fn jb_header(ty: u8, size: usize, out: &mut Vec<u8>) {
    if size <= 11 {
        out.push(((size as u8) << 4) | ty);
    } else if size <= 0xff {
        out.push((12 << 4) | ty);
        out.push(size as u8);
    } else if size <= 0xffff {
        out.push((13 << 4) | ty);
        out.extend_from_slice(&(size as u16).to_be_bytes());
    } else if size <= 0xffff_ffff {
        out.push((14 << 4) | ty);
        out.extend_from_slice(&(size as u32).to_be_bytes());
    } else {
        out.push((15 << 4) | ty);
        out.extend_from_slice(&(size as u64).to_be_bytes());
    }
}

fn jb_elem(ty: u8, payload: &[u8], out: &mut Vec<u8>) {
    jb_header(ty, payload.len(), out);
    out.extend_from_slice(payload);
}

fn text_to_jsonb(src: &str) -> Option<Vec<u8>> {
    let mut e = JbEnc {
        c: src.chars().collect(),
        i: 0,
    };
    e.skip_ws();
    let mut out = Vec::new();
    e.value(&mut out)?;
    e.skip_ws();
    if e.i == e.c.len() {
        Some(out)
    } else {
        None
    }
}

struct JbEnc {
    c: Vec<char>,
    i: usize,
}

impl JbEnc {
    fn peek(&self) -> Option<char> {
        self.c.get(self.i).copied()
    }
    fn skip_ws(&mut self) {
        while matches!(self.peek(), Some(' ' | '\t' | '\n' | '\r')) {
            self.i += 1;
        }
    }
    fn value(&mut self, out: &mut Vec<u8>) -> Option<()> {
        self.skip_ws();
        match self.peek()? {
            '{' => self.object(out),
            '[' => self.array(out),
            '"' => {
                let (ty, raw) = self.raw_string()?;
                jb_elem(ty, raw.as_bytes(), out);
                Some(())
            }
            't' => self.lit("true", jb::TRUE, out),
            'f' => self.lit("false", jb::FALSE, out),
            'n' => self.lit("null", jb::NULL, out),
            c if c == '-' || c.is_ascii_digit() => {
                let (ty, raw) = self.raw_number()?;
                jb_elem(ty, raw.as_bytes(), out);
                Some(())
            }
            _ => None,
        }
    }
    fn lit(&mut self, word: &str, ty: u8, out: &mut Vec<u8>) -> Option<()> {
        for wc in word.chars() {
            if self.peek()? != wc {
                return None;
            }
            self.i += 1;
        }
        out.push(ty);
        Some(())
    }

    fn raw_string(&mut self) -> Option<(u8, String)> {
        self.i += 1;
        let mut raw = String::new();
        let mut had_escape = false;
        loop {
            let ch = self.peek()?;
            self.i += 1;
            match ch {
                '"' => break,
                '\\' => {
                    had_escape = true;
                    raw.push('\\');
                    let e = self.peek()?;
                    self.i += 1;
                    raw.push(e);

                    if e == 'u' {
                        for _ in 0..4 {
                            raw.push(self.peek()?);
                            self.i += 1;
                        }
                    }
                }
                c => raw.push(c),
            }
        }
        Some((if had_escape { jb::TEXTJ } else { jb::TEXT }, raw))
    }

    fn raw_number(&mut self) -> Option<(u8, String)> {
        let start = self.i;
        let mut is_float = false;
        if self.peek() == Some('-') {
            self.i += 1;
        }
        while let Some(c) = self.peek() {
            if c.is_ascii_digit() {
                self.i += 1;
            } else if matches!(c, '.' | 'e' | 'E' | '+' | '-') {
                is_float = true;
                self.i += 1;
            } else {
                break;
            }
        }
        if self.i == start {
            return None;
        }
        let raw: String = self.c[start..self.i].iter().collect();
        Some((if is_float { jb::FLOAT } else { jb::INT }, raw))
    }
    fn array(&mut self, out: &mut Vec<u8>) -> Option<()> {
        self.i += 1;
        let mut buf = Vec::new();
        self.skip_ws();
        if self.peek() == Some(']') {
            self.i += 1;
            jb_elem(jb::ARRAY, &buf, out);
            return Some(());
        }
        loop {
            self.value(&mut buf)?;
            self.skip_ws();
            match self.peek()? {
                ',' => self.i += 1,
                ']' => {
                    self.i += 1;
                    jb_elem(jb::ARRAY, &buf, out);
                    return Some(());
                }
                _ => return None,
            }
        }
    }
    fn object(&mut self, out: &mut Vec<u8>) -> Option<()> {
        self.i += 1;
        let mut buf = Vec::new();
        self.skip_ws();
        if self.peek() == Some('}') {
            self.i += 1;
            jb_elem(jb::OBJECT, &buf, out);
            return Some(());
        }
        loop {
            self.skip_ws();
            if self.peek() != Some('"') {
                return None;
            }
            let (ty, raw) = self.raw_string()?;
            jb_elem(ty, raw.as_bytes(), &mut buf);
            self.skip_ws();
            if self.peek() != Some(':') {
                return None;
            }
            self.i += 1;
            self.value(&mut buf)?;
            self.skip_ws();
            match self.peek()? {
                ',' => self.i += 1,
                '}' => {
                    self.i += 1;
                    jb_elem(jb::OBJECT, &buf, out);
                    return Some(());
                }
                _ => return None,
            }
        }
    }
}

fn jb_read<'a>(p: &'a [u8], i: &mut usize) -> Option<(u8, &'a [u8])> {
    let hdr = *p.get(*i)?;
    *i += 1;
    let ty = hdr & 0x0f;
    let sclass = hdr >> 4;
    let size = match sclass {
        0..=11 => sclass as usize,
        12 => {
            let s = *p.get(*i)? as usize;
            *i += 1;
            s
        }
        13 => {
            let s = u16::from_be_bytes([*p.get(*i)?, *p.get(*i + 1)?]) as usize;
            *i += 2;
            s
        }
        14 => {
            let b = p.get(*i..*i + 4)?;
            *i += 4;
            u32::from_be_bytes([b[0], b[1], b[2], b[3]]) as usize
        }
        _ => {
            let b = p.get(*i..*i + 8)?;
            *i += 8;
            u64::from_be_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]) as usize
        }
    };
    let payload = p.get(*i..*i + size)?;
    *i += size;
    Some((ty, payload))
}

fn jsonb_to_json_at(p: &[u8], i: &mut usize) -> Option<Json> {
    let (ty, payload) = jb_read(p, i)?;
    let text = || std::str::from_utf8(payload).ok().map(|s| s.to_string());
    Some(match ty {
        jb::NULL => Json::Null,
        jb::TRUE => Json::Bool(true),
        jb::FALSE => Json::Bool(false),
        jb::INT | jb::INT5 => {
            let s = text()?;
            match s.parse::<i64>() {
                Ok(n) => Json::Int(n),
                Err(_) => Json::Real(s.parse().ok()?),
            }
        }
        jb::FLOAT | jb::FLOAT5 => Json::Real(text()?.parse().ok()?),
        jb::TEXT | jb::TEXTRAW => Json::Str(text()?),
        jb::TEXTJ | jb::TEXT5 => {

            let quoted = format!("\"{}\"", text()?);
            match parse(&quoted)? {
                Json::Str(s) => Json::Str(s),
                _ => return None,
            }
        }
        jb::ARRAY => {
            let mut items = Vec::new();
            let mut j = 0;
            while j < payload.len() {
                items.push(jsonb_to_json_at(payload, &mut j)?);
            }
            Json::Arr(items)
        }
        jb::OBJECT => {
            let mut pairs = Vec::new();
            let mut j = 0;
            while j < payload.len() {
                let key = match jsonb_to_json_at(payload, &mut j)? {
                    Json::Str(s) => s,
                    _ => return None,
                };
                let val = jsonb_to_json_at(payload, &mut j)?;
                pairs.push((key, val));
            }
            Json::Obj(pairs)
        }
        _ => return None,
    })
}

fn jsonb_to_json(p: &[u8]) -> Option<Json> {
    let mut i = 0;
    let v = jsonb_to_json_at(p, &mut i)?;
    if i == p.len() {
        Some(v)
    } else {
        None
    }
}

fn jsonb_to_text_at(p: &[u8], i: &mut usize, out: &mut String) -> Option<()> {
    let (ty, payload) = jb_read(p, i)?;
    let text = || std::str::from_utf8(payload).ok();
    match ty {
        jb::NULL => out.push_str("null"),
        jb::TRUE => out.push_str("true"),
        jb::FALSE => out.push_str("false"),
        jb::INT | jb::INT5 | jb::FLOAT | jb::FLOAT5 => out.push_str(text()?),
        jb::TEXT | jb::TEXTJ | jb::TEXT5 => {
            out.push('"');
            out.push_str(text()?);
            out.push('"');
        }
        jb::TEXTRAW => write_string(text()?, out),
        jb::ARRAY => {
            out.push('[');
            let mut j = 0;
            let mut n = 0;
            while j < payload.len() {
                if n > 0 {
                    out.push(',');
                }
                jsonb_to_text_at(payload, &mut j, out)?;
                n += 1;
            }
            out.push(']');
        }
        jb::OBJECT => {
            out.push('{');
            let mut j = 0;
            let mut n = 0;
            while j < payload.len() {
                if n > 0 {
                    out.push(',');
                }
                jsonb_to_text_at(payload, &mut j, out)?;
                out.push(':');
                jsonb_to_text_at(payload, &mut j, out)?;
                n += 1;
            }
            out.push('}');
        }
        _ => return None,
    }
    Some(())
}

fn jsonb_to_text(p: &[u8]) -> Option<String> {
    let mut out = String::new();
    let mut i = 0;
    jsonb_to_text_at(p, &mut i, &mut out)?;
    Some(out)
}

fn json_to_jsonb(j: &Json, out: &mut Vec<u8>) {
    match j {
        Json::Null => out.push(jb::NULL),
        Json::Bool(true) => out.push(jb::TRUE),
        Json::Bool(false) => out.push(jb::FALSE),
        Json::Int(n) => jb_elem(jb::INT, n.to_string().as_bytes(), out),
        Json::Real(r) => jb_elem(jb::FLOAT, format_real(*r).as_bytes(), out),
        Json::Str(s) => {

            let mut body = String::new();
            write_string(s, &mut body);
            let inner = &body[1..body.len() - 1];
            let ty = if inner.len() == s.len() {
                jb::TEXT
            } else {
                jb::TEXTJ
            };
            jb_elem(ty, inner.as_bytes(), out);
        }
        Json::Arr(items) => {
            let mut buf = Vec::new();
            for it in items {
                json_to_jsonb(it, &mut buf);
            }
            jb_elem(jb::ARRAY, &buf, out);
        }
        Json::Obj(pairs) => {
            let mut buf = Vec::new();
            for (k, v) in pairs {
                json_to_jsonb(&Json::Str(k.clone()), &mut buf);
                json_to_jsonb(v, &mut buf);
            }
            jb_elem(jb::OBJECT, &buf, out);
        }
    }
}

fn doc_of(v: Option<&Value>) -> Option<Json> {
    match v {
        Some(Value::Blob(b)) => jsonb_to_json(b),
        Some(v) => parse(&crate::text_of(v)),
        None => None,
    }
}

fn parse(s: &str) -> Option<Json> {
    let chars: Vec<char> = s.chars().collect();
    let mut p = Parser { chars, i: 0 };
    p.skip_ws();
    let v = p.value()?;
    p.skip_ws();
    if p.i == p.chars.len() {
        Some(v)
    } else {
        None
    }
}

struct Parser {
    chars: Vec<char>,
    i: usize,
}

impl Parser {
    fn peek(&self) -> Option<char> {
        self.chars.get(self.i).copied()
    }

    fn skip_ws(&mut self) {
        while let Some(c) = self.peek() {
            if c == ' ' || c == '\t' || c == '\n' || c == '\r' {
                self.i += 1;
            } else {
                break;
            }
        }
    }

    fn value(&mut self) -> Option<Json> {
        self.skip_ws();
        match self.peek()? {
            '{' => self.object(),
            '[' => self.array(),
            '"' => self.string().map(Json::Str),
            't' | 'f' => self.boolean(),
            'n' => self.null(),
            c if c == '-' || c.is_ascii_digit() => self.number(),
            _ => None,
        }
    }

    fn object(&mut self) -> Option<Json> {
        self.i += 1;
        let mut pairs = Vec::new();
        self.skip_ws();
        if self.peek() == Some('}') {
            self.i += 1;
            return Some(Json::Obj(pairs));
        }
        loop {
            self.skip_ws();
            if self.peek() != Some('"') {
                return None;
            }
            let key = self.string()?;
            self.skip_ws();
            if self.peek() != Some(':') {
                return None;
            }
            self.i += 1;
            let val = self.value()?;
            pairs.push((key, val));
            self.skip_ws();
            match self.peek()? {
                ',' => {
                    self.i += 1;
                }
                '}' => {
                    self.i += 1;
                    return Some(Json::Obj(pairs));
                }
                _ => return None,
            }
        }
    }

    fn array(&mut self) -> Option<Json> {
        self.i += 1;
        let mut items = Vec::new();
        self.skip_ws();
        if self.peek() == Some(']') {
            self.i += 1;
            return Some(Json::Arr(items));
        }
        loop {
            let val = self.value()?;
            items.push(val);
            self.skip_ws();
            match self.peek()? {
                ',' => {
                    self.i += 1;
                }
                ']' => {
                    self.i += 1;
                    return Some(Json::Arr(items));
                }
                _ => return None,
            }
        }
    }

    fn string(&mut self) -> Option<String> {
        self.i += 1;
        let mut out = String::new();
        loop {
            let c = self.peek()?;
            self.i += 1;
            match c {
                '"' => return Some(out),
                '\\' => {
                    let e = self.peek()?;
                    self.i += 1;
                    match e {
                        '"' => out.push('"'),
                        '\\' => out.push('\\'),
                        '/' => out.push('/'),
                        'b' => out.push('\u{0008}'),
                        'f' => out.push('\u{000C}'),
                        'n' => out.push('\n'),
                        'r' => out.push('\r'),
                        't' => out.push('\t'),
                        'u' => {
                            let mut code = 0u32;
                            for _ in 0..4 {
                                let h = self.peek()?;
                                self.i += 1;
                                code = code * 16 + h.to_digit(16)?;
                            }

                            if (0xD800..=0xDBFF).contains(&code) {
                                if self.peek() == Some('\\') {
                                    self.i += 1;
                                    if self.peek() == Some('u') {
                                        self.i += 1;
                                        let mut lo = 0u32;
                                        for _ in 0..4 {
                                            let h = self.peek()?;
                                            self.i += 1;
                                            lo = lo * 16 + h.to_digit(16)?;
                                        }
                                        let c = 0x10000 + ((code - 0xD800) << 10) + (lo - 0xDC00);
                                        out.push(char::from_u32(c)?);
                                        continue;
                                    } else {
                                        return None;
                                    }
                                }
                            }
                            out.push(char::from_u32(code).unwrap_or('\u{FFFD}'));
                        }
                        _ => return None,
                    }
                }
                c if (c as u32) < 0x20 => return None,
                c => out.push(c),
            }
        }
    }

    fn boolean(&mut self) -> Option<Json> {
        if self.matches("true") {
            Some(Json::Bool(true))
        } else if self.matches("false") {
            Some(Json::Bool(false))
        } else {
            None
        }
    }

    fn null(&mut self) -> Option<Json> {
        if self.matches("null") {
            Some(Json::Null)
        } else {
            None
        }
    }

    fn matches(&mut self, kw: &str) -> bool {
        let kc: Vec<char> = kw.chars().collect();
        if self.i + kc.len() > self.chars.len() {
            return false;
        }
        if self.chars[self.i..self.i + kc.len()] == kc[..] {
            self.i += kc.len();
            true
        } else {
            false
        }
    }

    fn number(&mut self) -> Option<Json> {
        let start = self.i;
        let mut is_real = false;
        if self.peek() == Some('-') {
            self.i += 1;
        }
        while let Some(c) = self.peek() {
            match c {
                '0'..='9' => self.i += 1,
                '.' | 'e' | 'E' | '+' | '-' => {
                    is_real = true;
                    self.i += 1;
                }
                _ => break,
            }
        }
        let text: String = self.chars[start..self.i].iter().collect();
        if is_real {
            text.parse::<f64>().ok().map(Json::Real)
        } else {
            match text.parse::<i64>() {
                Ok(i) => Some(Json::Int(i)),
                Err(_) => text.parse::<f64>().ok().map(Json::Real),
            }
        }
    }
}

fn serialize(j: &Json) -> String {
    let mut out = String::new();
    write_json(j, &mut out);
    out
}

fn write_json(j: &Json, out: &mut String) {
    match j {
        Json::Null => out.push_str("null"),
        Json::Bool(true) => out.push_str("true"),
        Json::Bool(false) => out.push_str("false"),
        Json::Int(i) => out.push_str(&i.to_string()),
        Json::Real(r) => out.push_str(&format_real(*r)),
        Json::Str(s) => write_string(s, out),
        Json::Arr(items) => {
            out.push('[');
            for (n, item) in items.iter().enumerate() {
                if n > 0 {
                    out.push(',');
                }
                write_json(item, out);
            }
            out.push(']');
        }
        Json::Obj(pairs) => {
            out.push('{');
            for (n, (k, v)) in pairs.iter().enumerate() {
                if n > 0 {
                    out.push(',');
                }
                write_string(k, out);
                out.push(':');
                write_json(v, out);
            }
            out.push('}');
        }
    }
}

fn format_real(r: f64) -> String {
    if r.is_finite() && r.fract() == 0.0 {
        format!("{:.1}", r)
    } else {
        r.to_string()
    }
}

fn write_string(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{0008}' => out.push_str("\\b"),
            '\u{000C}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push('"');
}
