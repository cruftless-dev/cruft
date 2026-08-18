
use crate::strip::{options_for_path, strip_ts_with_options, StripError, StripOptions};
use crate::ts_ast::TypeWitness;

#[derive(Debug)]
pub struct TransformError {
    pub message: String,
}

impl std::fmt::Display for TransformError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for TransformError {}

#[derive(Debug)]
pub enum TsSourceError {
    Strip(StripError),
    Transform(TransformError),
}

impl std::fmt::Display for TsSourceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TsSourceError::Strip(e) => write!(f, "{}", e),
            TsSourceError::Transform(e) => write!(f, "{}", e),
        }
    }
}

impl std::error::Error for TsSourceError {}

pub fn transform_enabled() -> bool {
    matches!(
        std::env::var("CRUFT_TS_TRANSFORM").as_deref(),
        Ok("1") | Ok("true") | Ok("TRUE") | Ok("yes") | Ok("on")
    )
}

pub fn ts_source_to_js_for_path(
    path: &str,
    src: &str,
) -> Result<(String, Vec<TypeWitness>), TsSourceError> {
    let options = options_for_path(path);
    if transform_enabled() {
        transform_ts_to_js_with_options(src, options).map_err(TsSourceError::Transform)
    } else {
        strip_ts_with_options(src, options).map_err(TsSourceError::Strip)
    }
}

pub fn transform_ts_to_js_with_options(
    src: &str,
    options: StripOptions,
) -> Result<(String, Vec<TypeWitness>), TransformError> {
    let lowered_enums = lower_enums(src)?;
    let src = lowered_enums.as_deref().unwrap_or(src);
    let lowered_params = lower_parameter_properties(src)?;
    let src = lowered_params.as_deref().unwrap_or(src);
    let (stripped, witnesses) =
        strip_ts_with_options(src, options).map_err(|e| TransformError {
            message: format!("transform pre-erasure failed: {}", e),
        })?;
    rusty_js_parser::parse_module(&stripped).map_err(|e| TransformError {
        message: format!("transform emitted invalid JavaScript: {:?}", e),
    })?;
    Ok((stripped, witnesses))
}

fn lower_enums(src: &str) -> Result<Option<String>, TransformError> {
    let bytes = src.as_bytes();
    let mut edits: Vec<(usize, usize, String)> = Vec::new();
    let mut i = 0usize;
    while let Some(enum_idx) = find_keyword(bytes, i, b"enum") {
        let mut start = enum_idx;
        let mut export = false;
        let prefix_start = line_prefix_start(bytes, enum_idx);
        let prefix = &src[prefix_start..enum_idx];
        let words: Vec<&str> = prefix.split_whitespace().collect();
        if words.iter().rev().any(|word| *word == "const") {
            return Err(TransformError {
                message:
                    "transform unsupported enum: const enum inlining is outside bounded lowering"
                        .to_string(),
            });
        }
        if words.iter().rev().any(|word| *word == "declare") {
            i = enum_idx + "enum".len();
            continue;
        }
        if let Some(word) = words.last() {
            if *word == "export" {
                export = true;
                start = prefix_start + prefix.rfind("export").unwrap_or(0);
            }
        }
        let name_start = skip_ws(bytes, enum_idx + "enum".len());
        let Some((name, name_end)) = read_ident(src, name_start) else {
            return Err(TransformError {
                message: "transform unsupported enum: enum name must be an identifier".to_string(),
            });
        };
        let brace_open = skip_ws(bytes, name_end);
        if bytes.get(brace_open) != Some(&b'{') {
            return Err(TransformError {
                message: "transform unsupported enum: enum body is missing".to_string(),
            });
        }
        let Some(brace_close) = find_matching(bytes, brace_open, b'{', b'}') else {
            return Err(TransformError {
                message: "transform unsupported enum: enum body is unbalanced".to_string(),
            });
        };
        let replacement = emit_enum_lowering(name, &src[brace_open + 1..brace_close], export)?;
        edits.push((start, brace_close + 1, replacement));
        i = brace_close + 1;
    }
    if edits.is_empty() {
        return Ok(None);
    }
    edits.sort_by_key(|(start, _, _)| *start);
    let mut out = String::with_capacity(src.len() + 128);
    let mut cursor = 0usize;
    for (start, end, replacement) in edits {
        if start < cursor {
            return Err(TransformError {
                message: "transform internal error: overlapping enum edits".to_string(),
            });
        }
        out.push_str(&src[cursor..start]);
        out.push_str(&replacement);
        cursor = end;
    }
    out.push_str(&src[cursor..]);
    Ok(Some(out))
}

#[derive(Clone, Debug)]
enum EnumValue {
    Number(i64),
    String(String),
}

fn emit_enum_lowering(name: &str, body: &str, export: bool) -> Result<String, TransformError> {
    let members = parse_enum_members(body)?;
    let mut out = String::new();
    if export {
        out.push_str("export ");
    }
    out.push_str("var ");
    out.push_str(name);
    out.push_str(" = {};");
    for (member, value) in members {
        match value {
            EnumValue::Number(n) => {
                out.push_str(name);
                out.push('[');
                out.push_str(name);
                out.push('[');
                out.push_str(&js_string_literal(&member));
                out.push_str("] = ");
                out.push_str(&n.to_string());
                out.push_str("] = ");
                out.push_str(&js_string_literal(&member));
                out.push(';');
            }
            EnumValue::String(s) => {
                out.push_str(name);
                out.push('[');
                out.push_str(&js_string_literal(&member));
                out.push_str("] = ");
                out.push_str(&js_string_literal(&s));
                out.push(';');
            }
        }
    }
    Ok(out)
}

fn parse_enum_members(body: &str) -> Result<Vec<(String, EnumValue)>, TransformError> {
    let ranges = split_top_level_commas(body)?;
    let mut out = Vec::new();
    let mut next_numeric = 0i64;
    for (start, end) in ranges {
        let segment = body[start..end].trim();
        if segment.is_empty() {
            continue;
        }
        let (name, after_name) = parse_enum_member_name(segment)?;
        let rest = segment[after_name..].trim_start();
        let value = if let Some(expr) = rest.strip_prefix('=') {
            let expr = expr.trim();
            if expr.starts_with('"') || expr.starts_with('\'') {
                EnumValue::String(parse_string_literal(expr).ok_or_else(|| TransformError {
                    message: "transform unsupported enum: string initializer must be a simple literal"
                        .to_string(),
                })?)
            } else {
                let n = expr.parse::<i64>().map_err(|_| TransformError {
                    message:
                        "transform unsupported enum: numeric initializer must be an integer literal"
                            .to_string(),
                })?;
                next_numeric = n + 1;
                EnumValue::Number(n)
            }
        } else {
            let n = next_numeric;
            next_numeric += 1;
            EnumValue::Number(n)
        };
        out.push((name, value));
    }
    Ok(out)
}

fn parse_enum_member_name(segment: &str) -> Result<(String, usize), TransformError> {
    if segment.starts_with('"') || segment.starts_with('\'') {
        let end = skip_string_like(segment.as_bytes(), 0).ok_or_else(|| TransformError {
            message: "transform unsupported enum: unterminated quoted member name".to_string(),
        })?;
        let name = parse_string_literal(&segment[..=end]).ok_or_else(|| TransformError {
            message: "transform unsupported enum: quoted member name must be simple".to_string(),
        })?;
        return Ok((name, end + 1));
    }
    let Some((name, end)) = read_ident(segment, 0) else {
        return Err(TransformError {
            message:
                "transform unsupported enum: member name must be an identifier or string literal"
                    .to_string(),
        });
    };
    Ok((name.to_string(), end))
}

fn parse_string_literal(src: &str) -> Option<String> {
    let bytes = src.as_bytes();
    let quote = *bytes.first()?;
    if quote != b'\'' && quote != b'"' {
        return None;
    }
    let end = skip_string_like(bytes, 0)?;
    if skip_ws(bytes, end + 1) != bytes.len() {
        return None;
    }
    let mut out = String::new();
    let mut i = 1usize;
    while i < end {
        if bytes[i] == b'\\' {
            i += 1;
            let escaped = *bytes.get(i)?;
            match escaped {
                b'\\' => out.push('\\'),
                b'\'' => out.push('\''),
                b'"' => out.push('"'),
                b'n' => out.push('\n'),
                b'r' => out.push('\r'),
                b't' => out.push('\t'),
                other => out.push(other as char),
            }
        } else {
            out.push(bytes[i] as char);
        }
        i += 1;
    }
    Some(out)
}

fn js_string_literal(s: &str) -> String {
    let mut out = String::from("\"");
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            ch => out.push(ch),
        }
    }
    out.push('"');
    out
}

fn lower_parameter_properties(src: &str) -> Result<Option<String>, TransformError> {
    let bytes = src.as_bytes();
    let mut edits: Vec<(usize, usize, String)> = Vec::new();
    let mut i = 0usize;
    while let Some(class_idx) = find_keyword(bytes, i, b"class") {
        let Some(open_brace) = find_next_byte(bytes, class_idx + 5, b'{') else {
            return Err(TransformError {
                message: "transform unsupported parameter property: class body is missing"
                    .to_string(),
            });
        };
        let Some(close_brace) = find_matching(bytes, open_brace, b'{', b'}') else {
            return Err(TransformError {
                message: "transform unsupported parameter property: class body is unbalanced"
                    .to_string(),
            });
        };
        let is_derived = src[class_idx..open_brace].contains("extends");
        let mut j = open_brace + 1;
        while let Some(ctor_idx) = find_keyword_in_range(bytes, j, close_brace, b"constructor") {
            let p = skip_ws(bytes, ctor_idx + "constructor".len());
            if bytes.get(p) != Some(&b'(') {
                j = ctor_idx + "constructor".len();
                continue;
            }
            let Some(param_close) = find_matching(bytes, p, b'(', b')') else {
                return Err(TransformError {
                    message:
                        "transform unsupported parameter property: constructor parameters are unbalanced"
                            .to_string(),
                });
            };
            let body_open = skip_ws(bytes, param_close + 1);
            if bytes.get(body_open) != Some(&b'{') {
                return Err(TransformError {
                    message:
                        "transform unsupported parameter property: constructor overload/signature cannot lower"
                            .to_string(),
                });
            }
            let Some(body_close) = find_matching(bytes, body_open, b'{', b'}') else {
                return Err(TransformError {
                    message:
                        "transform unsupported parameter property: constructor body is unbalanced"
                            .to_string(),
                });
            };
            let lowered = lower_constructor_params(&src[p + 1..param_close])?;
            if !lowered.assignments.is_empty() {
                edits.push((p + 1, param_close, lowered.params));
                let insertion = if is_derived {
                    find_super_insertion(src, body_open, body_close).ok_or_else(|| {
                        TransformError {
                            message: "transform unsupported parameter property: derived constructor must call super() before synthesized assignments".to_string(),
                        }
                    })?
                } else {
                    body_open + 1
                };
                edits.push((insertion, insertion, lowered.assignments.join("")));
            }
            j = body_close + 1;
        }
        i = close_brace + 1;
    }
    if edits.is_empty() {
        return Ok(None);
    }
    edits.sort_by_key(|(start, _, _)| *start);
    let mut out = String::with_capacity(src.len() + 128);
    let mut cursor = 0usize;
    for (start, end, replacement) in edits {
        if start < cursor {
            return Err(TransformError {
                message: "transform internal error: overlapping parameter-property edits"
                    .to_string(),
            });
        }
        out.push_str(&src[cursor..start]);
        out.push_str(&replacement);
        cursor = end;
    }
    out.push_str(&src[cursor..]);
    Ok(Some(out))
}

struct LoweredParams {
    params: String,
    assignments: Vec<String>,
}

fn lower_constructor_params(params_src: &str) -> Result<LoweredParams, TransformError> {
    let ranges = split_top_level_commas(params_src)?;
    let mut params = String::with_capacity(params_src.len());
    let mut assignments = Vec::new();
    let mut cursor = 0usize;
    for (start, end) in ranges {
        params.push_str(&params_src[cursor..start]);
        let segment = &params_src[start..end];
        let lowered = lower_param_segment(segment)?;
        params.push_str(&lowered.param);
        if let Some(name) = lowered.property {
            assignments.push(format!("this.{name} = {name};"));
        }
        cursor = end;
    }
    params.push_str(&params_src[cursor..]);
    Ok(LoweredParams {
        params,
        assignments,
    })
}

struct LoweredParam {
    param: String,
    property: Option<String>,
}

fn lower_param_segment(segment: &str) -> Result<LoweredParam, TransformError> {
    let leading = segment.len() - segment.trim_start().len();
    let trimmed = &segment[leading..];
    let mut pos = 0usize;
    let mut saw_modifier = false;
    loop {
        let Some((word, word_end)) = read_ident(&trimmed[pos..], 0) else {
            break;
        };
        if matches!(word, "public" | "private" | "protected" | "readonly") {
            saw_modifier = true;
            pos += word_end;
            pos += ws_len(&trimmed[pos..]);
        } else {
            break;
        }
    }
    if !saw_modifier {
        return Ok(LoweredParam {
            param: segment.to_string(),
            property: None,
        });
    }
    let Some((name, name_end)) = read_ident(&trimmed[pos..], 0) else {
        return Err(TransformError {
            message:
                "transform unsupported parameter property: property parameter must be a simple identifier"
                    .to_string(),
        });
    };
    if trimmed[pos + name_end..].trim_start().starts_with('?') {
        return Err(TransformError {
            message:
                "transform unsupported parameter property: optional parameter properties are not yet lowered"
                    .to_string(),
        });
    }
    let mut param = String::with_capacity(segment.len());
    param.push_str(&segment[..leading]);
    param.push_str(&trimmed[pos..]);
    Ok(LoweredParam {
        param,
        property: Some(name.to_string()),
    })
}

fn split_top_level_commas(src: &str) -> Result<Vec<(usize, usize)>, TransformError> {
    let bytes = src.as_bytes();
    let mut ranges = Vec::new();
    let mut start = 0usize;
    let mut i = 0usize;
    let mut paren = 0i32;
    let mut brace = 0i32;
    let mut bracket = 0i32;
    let mut angle = 0i32;
    while i < bytes.len() {
        match bytes[i] {
            b'\'' | b'"' | b'`' => {
                i = skip_string_like(bytes, i).ok_or_else(|| TransformError {
                    message:
                        "transform unsupported parameter property: unterminated literal in parameter list"
                            .to_string(),
                })?;
            }
            b'(' => paren += 1,
            b')' => paren -= 1,
            b'{' => brace += 1,
            b'}' => brace -= 1,
            b'[' => bracket += 1,
            b']' => bracket -= 1,
            b'<' => angle += 1,
            b'>' if angle > 0 => angle -= 1,
            b',' if paren == 0 && brace == 0 && bracket == 0 && angle == 0 => {
                ranges.push((start, i));
                start = i + 1;
            }
            _ => {}
        }
        i += 1;
    }
    ranges.push((start, bytes.len()));
    Ok(ranges)
}

fn find_super_insertion(src: &str, body_open: usize, body_close: usize) -> Option<usize> {
    let bytes = src.as_bytes();
    let mut i = body_open + 1;
    while let Some(super_idx) = find_keyword_in_range(bytes, i, body_close, b"super") {
        let p = skip_ws(bytes, super_idx + "super".len());
        if bytes.get(p) == Some(&b'(') {
            let call_close = find_matching(bytes, p, b'(', b')')?;
            let after = skip_ws(bytes, call_close + 1);
            return Some(if bytes.get(after) == Some(&b';') {
                after + 1
            } else {
                call_close + 1
            });
        }
        i = super_idx + "super".len();
    }
    None
}

fn find_keyword(bytes: &[u8], start: usize, kw: &[u8]) -> Option<usize> {
    find_keyword_in_range(bytes, start, bytes.len(), kw)
}

fn find_keyword_in_range(bytes: &[u8], start: usize, end: usize, kw: &[u8]) -> Option<usize> {
    let mut i = start;
    while i + kw.len() <= end {
        if &bytes[i..i + kw.len()] == kw
            && !is_ident_byte(bytes.get(i.wrapping_sub(1)).copied().unwrap_or(b'\0'))
            && !is_ident_byte(bytes.get(i + kw.len()).copied().unwrap_or(b'\0'))
        {
            return Some(i);
        }
        i += 1;
    }
    None
}

fn find_next_byte(bytes: &[u8], start: usize, needle: u8) -> Option<usize> {
    bytes[start..]
        .iter()
        .position(|b| *b == needle)
        .map(|off| start + off)
}

fn line_prefix_start(bytes: &[u8], idx: usize) -> usize {
    let mut i = idx;
    while i > 0 && !matches!(bytes[i - 1], b'\n' | b'\r' | b';' | b'{') {
        i -= 1;
    }
    i
}

fn find_matching(bytes: &[u8], open_idx: usize, open: u8, close: u8) -> Option<usize> {
    if bytes.get(open_idx) != Some(&open) {
        return None;
    }
    let mut depth = 1i32;
    let mut i = open_idx + 1;
    while i < bytes.len() {
        match bytes[i] {
            b'\'' | b'"' | b'`' => i = skip_string_like(bytes, i)?,
            b if b == open => depth += 1,
            b if b == close => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

fn skip_string_like(bytes: &[u8], start: usize) -> Option<usize> {
    let quote = *bytes.get(start)?;
    let mut i = start + 1;
    while i < bytes.len() {
        if bytes[i] == b'\\' {
            i += 2;
            continue;
        }
        if bytes[i] == quote {
            return Some(i);
        }
        i += 1;
    }
    None
}

fn skip_ws(bytes: &[u8], mut i: usize) -> usize {
    while matches!(bytes.get(i), Some(b' ' | b'\n' | b'\r' | b'\t')) {
        i += 1;
    }
    i
}

fn ws_len(s: &str) -> usize {
    s.as_bytes()
        .iter()
        .take_while(|b| matches!(b, b' ' | b'\n' | b'\r' | b'\t'))
        .count()
}

fn read_ident(src: &str, start: usize) -> Option<(&str, usize)> {
    let bytes = src.as_bytes();
    let first = *bytes.get(start)?;
    if !(first == b'_' || first == b'$' || first.is_ascii_alphabetic()) {
        return None;
    }
    let mut end = start + 1;
    while let Some(b) = bytes.get(end) {
        if *b == b'_' || *b == b'$' || b.is_ascii_alphanumeric() {
            end += 1;
        } else {
            break;
        }
    }
    Some((&src[start..end], end))
}

fn is_ident_byte(b: u8) -> bool {
    b == b'_' || b == b'$' || b.is_ascii_alphanumeric()
}
