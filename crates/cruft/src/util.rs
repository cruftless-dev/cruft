
use crate::register::{make_callable_rooted, new_object, register_method, set_constant};
use rusty_js_runtime::abstract_ops;
use rusty_js_runtime::promise::{new_promise, reject_promise, resolve_promise};
use rusty_js_runtime::value::{InternalKind, Object, ObjectRef, PropertyKey};
use rusty_js_runtime::{Runtime, RuntimeError, Value};
use std::rc::Rc;

const PROMISIFY_CUSTOM_KEY: &str = "@@sym:nodejs.util.promisify.custom";
const DEFAULT_BREAK_LENGTH: usize = 80;

const INSPECT_COMPACT: i32 = 3;

thread_local! {

    static INSPECT_CURRENT_DEPTH: std::cell::Cell<i32> = const { std::cell::Cell::new(0) };

    static INSPECT_SHOW_PROXY: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

fn inspect_mark_current_depth(v: &Value, depth: i32, max_depth: i32) {
    if depth <= max_depth && matches!(v, Value::Object(_)) {
        INSPECT_CURRENT_DEPTH.with(|c| c.set(depth));
    }
}

fn util_string_value(s: &str) -> Value {
    Value::String(Rc::new(rusty_js_runtime::value::JsString::from(s)))
}

fn register_util_deprecate(rt: &mut Runtime, util: ObjectRef) {
    register_method(rt, util, "deprecate", |rt, args| {
        let fn_v = args.first().cloned().unwrap_or(Value::Undefined);
        if !rt.is_callable(&fn_v) {
            return Ok(fn_v);
        }
        let msg = match args.get(1) {
            Some(Value::String(s)) => s.as_str().to_string(),
            Some(other) => rusty_js_runtime::abstract_ops::to_string(other)
                .as_str()
                .to_string(),
            None => String::new(),
        };
        let code = match args.get(2) {
            Some(Value::String(s)) => Some(s.as_str().to_string()),
            _ => None,
        };
        let mut roots = Vec::new();
        if let Value::Object(id) = &fn_v {
            roots.push(*id);
        }
        let warned = std::rc::Rc::new(std::cell::Cell::new(false));
        let wrapper = make_callable_rooted(rt, "deprecated", roots, move |rt, call_args| {
            if !warned.get() {
                warned.set(true);
                if let Value::Object(pid) = rt.global_get("process") {
                    let ew = rt.object_get(pid, "emitWarning");
                    if rt.is_callable(&ew) {
                        let opts = new_object(rt);
                        rt.object_set(opts, "type".into(), util_string_value("DeprecationWarning"));
                        if let Some(c) = &code {
                            rt.object_set(opts, "code".into(), util_string_value(c));
                        }
                        let _ = rt.call_function(
                            ew,
                            Value::Object(pid),
                            vec![util_string_value(&msg), Value::Object(opts)],
                        );
                    }
                }
            }
            let this = rt.current_this();
            rt.call_function(fn_v.clone(), this, call_args.to_vec())
        });
        Ok(Value::Object(wrapper))
    });
}

fn util_invalid_arg_type(rt: &mut Runtime, msg: &str) -> RuntimeError {
    match rusty_js_runtime::intrinsics::make_error_instance(rt, "TypeError", msg) {
        Some(id) => {
            rt.object_set(
                id,
                "code".into(),
                Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                    "ERR_INVALID_ARG_TYPE",
                ))),
            );
            RuntimeError::Thrown(Value::Object(id))
        }
        None => RuntimeError::TypeError(msg.to_string()),
    }
}

fn inspect_num(n: f64) -> String {
    if n == 0.0 && n.is_sign_negative() {
        return "-0".to_string();
    }
    abstract_ops::to_string(&Value::Number(n))
        .as_str()
        .to_string()
}

fn format_number_preserve_negative_zero(n: f64) -> String {
    if n == 0.0 && n.is_sign_negative() {
        "-0".to_string()
    } else {
        abstract_ops::number_to_string(n)
    }
}

fn add_numeric_separators_to_digits(s: &str) -> String {
    let (sign, rest) = if let Some(rest) = s.strip_prefix('-') {
        ("-", rest)
    } else {
        ("", s)
    };
    let mut out = String::with_capacity(s.len() + s.len() / 3);
    out.push_str(sign);
    let first_group = rest.len() % 3;
    let mut idx = 0usize;
    if first_group != 0 {
        out.push_str(&rest[..first_group]);
        idx = first_group;
        if idx < rest.len() {
            out.push('_');
        }
    }
    while idx < rest.len() {
        out.push_str(&rest[idx..idx + 3]);
        idx += 3;
        if idx < rest.len() {
            out.push('_');
        }
    }
    out
}

fn add_numeric_separators_to_fraction(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + s.len() / 3);
    for (i, c) in s.chars().enumerate() {
        if i != 0 && i % 3 == 0 {
            out.push('_');
        }
        out.push(c);
    }
    out
}

fn add_numeric_separators(s: &str) -> String {
    let Some(first_digit) = s.find(|c: char| c.is_ascii_digit()) else {
        return s.to_string();
    };

    if s[first_digit..].contains(['e', 'E']) {
        return s.to_string();
    }
    if let Some(dot) = s.find('.') {
        let int_part = &s[..dot];
        let frac_part = &s[dot + 1..];
        if int_part
            .trim_start_matches(['+', '-'])
            .chars()
            .all(|c| c.is_ascii_digit())
            && !int_part.trim_start_matches(['+', '-']).is_empty()
            && frac_part.chars().all(|c| c.is_ascii_digit())
        {
            return format!(
                "{}.{}",
                add_numeric_separators_to_digits(int_part),
                add_numeric_separators_to_fraction(frac_part)
            );
        }
        return s.to_string();
    }
    let suffix = if s.ends_with('n') { "n" } else { "" };
    let digits_end = if suffix.is_empty() {
        s.len()
    } else {
        s.len() - 1
    };
    let digits = &s[..digits_end];
    if !digits
        .trim_start_matches(['+', '-'])
        .chars()
        .all(|c| c.is_ascii_digit())
    {
        return s.to_string();
    }
    format!("{}{}", add_numeric_separators_to_digits(digits), suffix)
}

fn util_format_decimal(
    rt: &mut Runtime,
    v: &Value,
    numeric_separator: bool,
) -> Result<String, RuntimeError> {
    let s = match v {
        Value::BigInt(b) => {
            let s = format!("{}n", b.to_decimal());
            if numeric_separator {
                add_numeric_separators(&s)
            } else {
                s
            }
        }
        Value::Symbol(_) => "NaN".to_string(),
        _ => {
            let s = format_number_preserve_negative_zero(rt.coerce_to_number(v)?);
            if numeric_separator {
                add_numeric_separators(&s)
            } else {
                s
            }
        }
    };
    Ok(s)
}

fn parse_float_prefix_number_text(s: &str) -> Option<String> {
    let trimmed = s.trim_start();
    let bytes = trimmed.as_bytes();
    let mut i = 0usize;
    if matches!(bytes.get(i), Some(b'+') | Some(b'-')) {
        i += 1;
    }
    if trimmed[i..].starts_with("Infinity") {
        let prefix = &trimmed[..i + "Infinity".len()];
        return Some(if prefix.starts_with('+') {
            "Infinity".to_string()
        } else {
            prefix.to_string()
        });
    }
    let digits_start = i;
    while matches!(bytes.get(i), Some(b'0'..=b'9')) {
        i += 1;
    }
    let mut saw_digit = i > digits_start;
    if matches!(bytes.get(i), Some(b'.')) {
        i += 1;
        let frac_start = i;
        while matches!(bytes.get(i), Some(b'0'..=b'9')) {
            i += 1;
        }
        saw_digit |= i > frac_start;
    }
    if !saw_digit {
        return None;
    }
    let mantissa_end = i;
    if matches!(bytes.get(i), Some(b'e') | Some(b'E')) {
        let exp_mark = i;
        i += 1;
        if matches!(bytes.get(i), Some(b'+') | Some(b'-')) {
            i += 1;
        }
        let exp_digits_start = i;
        while matches!(bytes.get(i), Some(b'0'..=b'9')) {
            i += 1;
        }
        if i == exp_digits_start {
            i = exp_mark;
        }
    }
    let prefix = &trimmed[..i.max(mantissa_end)];
    let parsed = prefix.parse::<f64>().ok()?;
    Some(abstract_ops::number_to_string(parsed).as_str().to_string())
}

fn util_format_float(rt: &mut Runtime, v: &Value) -> Result<String, RuntimeError> {
    let s = match v {
        Value::BigInt(b) => b.to_decimal(),
        Value::Symbol(_) => "NaN".to_string(),
        Value::Object(_) => {
            let s = rt.coerce_to_string(v)?;
            parse_float_prefix_number_text(s.as_str()).unwrap_or_else(|| "NaN".to_string())
        }
        _ => {
            let s = abstract_ops::to_string(v);
            parse_float_prefix_number_text(s.as_str()).unwrap_or_else(|| "NaN".to_string())
        }
    };
    Ok(s)
}

fn parse_int_prefix_number_text(s: &str) -> Option<String> {
    let trimmed = s.trim_start();
    let mut chars = trimmed.char_indices().peekable();
    let mut end = 0usize;
    if let Some((i, ch)) = chars.peek().copied() {
        if ch == '+' || ch == '-' {
            end = i + ch.len_utf8();
            chars.next();
        }
    }
    let mut saw_digit = false;
    while let Some((i, ch)) = chars.peek().copied() {
        if ch.is_ascii_digit() {
            saw_digit = true;
            end = i + ch.len_utf8();
            chars.next();
        } else {
            break;
        }
    }
    if !saw_digit {
        return None;
    }
    let prefix = &trimmed[..end];
    if prefix == "-0" {
        Some("-0".to_string())
    } else if prefix == "+0" {
        Some("0".to_string())
    } else {
        Some(prefix.trim_start_matches('+').to_string())
    }
}

fn util_format_integer(
    rt: &mut Runtime,
    v: &Value,
    numeric_separator: bool,
) -> Result<String, RuntimeError> {
    let s = match v {
        Value::BigInt(b) => format!("{}n", b.to_decimal()),
        Value::Symbol(_) => "NaN".to_string(),
        Value::Object(_) => {
            let s = rt.coerce_to_string(v)?;
            parse_int_prefix_number_text(s.as_str()).unwrap_or_else(|| "NaN".to_string())
        }
        _ => parse_int_prefix_number_text(abstract_ops::to_string(v).as_str())
            .unwrap_or_else(|| "NaN".to_string()),
    };
    Ok(if numeric_separator {
        add_numeric_separators(&s)
    } else {
        s
    })
}

fn util_format_string(
    rt: &mut Runtime,
    v: &Value,
    numeric_separator: bool,
) -> Result<String, RuntimeError> {
    let s = match v {
        Value::BigInt(b) => format!("{}n", b.to_decimal()),
        Value::Number(n) => format_number_preserve_negative_zero(*n),
        Value::Object(id) if rt.obj(*id).has_own_str("__date_ms") => {
            inspect_value(rt, v, 0, 2, DEFAULT_BREAK_LENGTH)
        }
        Value::Object(id) if matches!(rt.obj(*id).internal_kind, InternalKind::Array) => {
            inspect_value(rt, v, 0, 0, DEFAULT_BREAK_LENGTH)
        }
        Value::Object(id)
            if matches!(rt.obj(*id).internal_kind, InternalKind::Array)
                && inspect_display_name(rt, *id).is_some() =>
        {
            inspect_value(rt, v, 0, 2, DEFAULT_BREAK_LENGTH)
        }
        Value::Object(_) => {
            let has_string_coercion_method = {
                let to_primitive = rt.get_method(v, "@@toPrimitive")?;
                let to_string = rt.spec_get(v, "toString")?;
                let value_of = rt.spec_get(v, "valueOf")?;
                !matches!(to_primitive, Value::Undefined)
                    || rt.is_callable(&to_string)
                    || rt.is_callable(&value_of)
            };
            match rt.coerce_to_string(v) {
                Ok(s) if s == "[object Object]" => inspect_value(rt, v, 0, 0, DEFAULT_BREAK_LENGTH),
                Ok(s) => s,
                Err(_err) if !has_string_coercion_method => {
                    inspect_value(rt, v, 0, 0, DEFAULT_BREAK_LENGTH)
                }
                Err(err) => return Err(err),
            }
        }
        _ => abstract_ops::to_string(v).as_str().to_string(),
    };
    Ok(if numeric_separator {
        add_numeric_separators(&s)
    } else {
        s
    })
}

fn util_numeric_separator_option(rt: &mut Runtime, options: Option<&Value>) -> bool {
    if let Some(Value::Object(id)) = options {
        if abstract_ops::to_boolean(&rt.object_get(*id, "numericSeparator")) {
            return true;
        }
    }
    let Value::Object(util_id) = rt.global_get("util") else {
        return false;
    };
    let Value::Object(inspect_id) = rt.object_get(util_id, "inspect") else {
        return false;
    };
    let Value::Object(defaults_id) = rt.object_get(inspect_id, "defaultOptions") else {
        return false;
    };
    abstract_ops::to_boolean(&rt.object_get(defaults_id, "numericSeparator"))
}

fn util_numeric_separator_default(rt: &Runtime) -> bool {
    let Value::Object(util_id) = rt.global_get("util") else {
        return false;
    };
    let Value::Object(inspect_id) = rt.object_get(util_id, "inspect") else {
        return false;
    };
    let Value::Object(defaults_id) = rt.object_get(inspect_id, "defaultOptions") else {
        return false;
    };
    abstract_ops::to_boolean(&rt.object_get(defaults_id, "numericSeparator"))
}

fn util_inspect_custom_symbol(rt: &mut Runtime) -> Option<Rc<String>> {
    match rt.symbol_for_via(&[Value::String(Rc::new(
        rusty_js_runtime::value::JsString::from("nodejs.util.inspect.custom"),
    ))]) {
        Ok(Value::Symbol(sym)) => Some(sym),
        _ => None,
    }
}

fn util_inspect_function_value(rt: &Runtime) -> Value {
    let Value::Object(util_id) = rt.global_get("util") else {
        return Value::Undefined;
    };
    rt.object_get(util_id, "inspect")
}

pub(crate) fn inspect_for_assert_diff(rt: &mut Runtime, v: &Value) -> Option<String> {
    let inspect_fn = util_inspect_function_value(rt);
    if !rt.is_callable(&inspect_fn) {
        return None;
    }
    let opts = new_object(rt);
    rt.object_set(opts, "compact".into(), Value::Boolean(false));
    rt.object_set(opts, "customInspect".into(), Value::Boolean(false));
    rt.object_set(opts, "depth".into(), Value::Number(1000.0));
    rt.object_set(opts, "maxArrayLength".into(), Value::Number(f64::INFINITY));
    rt.object_set(opts, "showHidden".into(), Value::Boolean(false));
    rt.object_set(opts, "showProxy".into(), Value::Boolean(false));
    rt.object_set(opts, "sorted".into(), Value::Boolean(true));
    rt.object_set(opts, "getters".into(), Value::Boolean(true));
    rt.object_set(opts, "colors".into(), Value::Boolean(false));
    match rt.call_function(
        inspect_fn,
        Value::Undefined,
        vec![v.clone(), Value::Object(opts)],
    ) {
        Ok(Value::String(s)) => Some(s.as_str().to_string()),
        _ => None,
    }
}

fn util_inspect_options_arg(rt: &mut Runtime, arg: Option<&Value>) -> Value {
    match arg {
        Some(Value::Object(id)) => Value::Object(*id),
        _ => {
            let opts = new_object(rt);
            rt.object_set(opts, "colors".into(), Value::Boolean(false));
            Value::Object(opts)
        }
    }
}

fn render_custom_inspect_result(
    rt: &Runtime,
    result: &Value,
    max_depth: i32,
    break_length: usize,
    sorted: bool,
    max_string_length: Option<usize>,
    max_array_length: Option<usize>,
    colors: bool,
    show_hidden: bool,
) -> String {
    match result {
        Value::String(s) => s.as_str().to_string(),
        _ => inspect_value_with_options(
            rt,
            result,
            0,
            max_depth,
            break_length,
            sorted,
            max_string_length,
            max_array_length,
            colors,
            show_hidden,
        ),
    }
}

fn inspect_value_with_recursive_custom(
    rt: &mut Runtime,
    value: &Value,
    depth: i32,
    max_depth: i32,
    break_length: usize,
    sorted: bool,
    max_string_length: Option<usize>,
    max_array_length: Option<usize>,
    colors: bool,
    show_hidden: bool,
    options_arg: Option<Value>,
    seen: &mut Vec<ObjectRef>,
) -> Result<String, RuntimeError> {
    inspect_mark_current_depth(value, depth, max_depth);
    let Value::Object(id) = value else {
        return Ok(inspect_value_inner(
            rt,
            value,
            depth,
            max_depth,
            break_length,
            sorted,
            max_string_length,
            max_array_length,
            colors,
            show_hidden,
            seen,
        ));
    };
    if seen.contains(id) {
        return Ok("[Circular]".to_string());
    }
    if let Some(sym) = util_inspect_custom_symbol(rt) {
        let custom = rt
            .obj(*id)
            .get_own_symbol(&sym)
            .map(|desc| desc.value.clone())
            .unwrap_or(Value::Undefined);
        if rt.is_callable(&custom) {
            let options_arg = util_inspect_options_arg(rt, options_arg.as_ref());
            let inspect_arg = util_inspect_function_value(rt);
            let depth_arg = if max_depth == i32::MAX {
                max_depth
            } else {
                (max_depth - depth).max(-1)
            };
            let result = rt.call_function(
                custom,
                Value::Object(*id),
                vec![Value::Number(depth_arg as f64), options_arg, inspect_arg],
            )?;
            return Ok(match result {
                Value::String(s) => s.as_str().to_string(),
                other => inspect_value_with_recursive_custom(
                    rt,
                    &other,
                    depth,
                    max_depth,
                    break_length,
                    sorted,
                    max_string_length,
                    max_array_length,
                    colors,
                    show_hidden,
                    None,
                    seen,
                )?,
            });
        }
    }
    if depth > max_depth && inspect_depth_limit_has_content(rt, *id, show_hidden) {
        let is_array = matches!(rt.obj(*id).internal_kind, InternalKind::Array);
        return Ok(inspect_depth_limit_label(rt, *id, is_array));
    }
    if matches!(rt.obj(*id).internal_kind, InternalKind::Array) {
        seen.push(*id);
        let len = rt.array_length(*id);
        let cap = max_array_length.unwrap_or(len);
        let take = len.min(cap);
        let mut items = Vec::new();
        for idx in 0..take {
            items.push(inspect_value_with_recursive_custom(
                rt,
                &rt.object_get(*id, &idx.to_string()),
                depth + 1,
                max_depth,
                break_length,
                sorted,
                max_string_length,
                max_array_length,
                colors,
                show_hidden,
                None,
                seen,
            )?);
        }
        if take < len {
            items.push(format!("... {} more items", len - take));
        }
        let force_multiline =
            inspect_force_nested_group_multiline("[", "]", &items, depth, max_depth, break_length);
        let body = inspect_join_aggregate(
            "[",
            "]",
            &items,
            depth,
            if force_multiline { 0 } else { break_length },
        );
        seen.pop();
        return Ok(inspect_array_wrapper(rt, *id, len, body));
    }
    if !matches!(rt.obj(*id).internal_kind, InternalKind::Ordinary) {
        return Ok(inspect_value_inner(
            rt,
            value,
            depth,
            max_depth,
            break_length,
            sorted,
            max_string_length,
            max_array_length,
            colors,
            show_hidden,
            seen,
        ));
    }
    seen.push(*id);
    let mut keys = rt.ordinary_own_enumerable_string_keys(*id);
    if sorted {
        keys.sort();
    }
    let mut entries = Vec::new();
    for k in keys {
        let kd = if is_ident_key(&k) {
            k.clone()
        } else {
            inspect_str(&k)
        };
        let rendered = rt
            .obj(*id)
            .get_own(&k)
            .and_then(|d| inspect_accessor_placeholder(d.getter.is_some(), d.setter.is_some()))
            .map(str::to_string)
            .map(Ok)
            .unwrap_or_else(|| {
                let val = rt.object_get(*id, &k);
                inspect_value_with_recursive_custom(
                    rt,
                    &val,
                    depth + 1,
                    max_depth,
                    break_length,
                    sorted,
                    max_string_length,
                    max_array_length,
                    colors,
                    show_hidden,
                    None,
                    seen,
                )
            })?;
        entries.push(format!("{}: {}", kd, rendered));
    }
    let body = inspect_join_aggregate("{", "}", &entries, depth, break_length);
    seen.pop();
    Ok(inspect_object_wrapper(rt, *id, body))
}

fn value_contains_custom_inspect(
    rt: &Runtime,
    value: &Value,
    sym: &std::rc::Rc<String>,
    seen: &mut Vec<ObjectRef>,
) -> bool {
    let Value::Object(id) = value else {
        return false;
    };
    if seen.contains(id) {
        return false;
    }
    if rt.obj(*id).get_own_symbol(sym).is_some() {
        return true;
    }
    seen.push(*id);
    let found = if matches!(rt.obj(*id).internal_kind, InternalKind::Array) {
        let len = match rt.object_get(*id, "length") {
            Value::Number(n) if n.is_finite() && n >= 0.0 => n as usize,
            _ => 0,
        };
        (0..len).any(|idx| {
            value_contains_custom_inspect(rt, &rt.object_get(*id, &idx.to_string()), sym, seen)
        })
    } else if matches!(rt.obj(*id).internal_kind, InternalKind::Ordinary) {
        rt.ordinary_own_enumerable_string_keys(*id)
            .iter()
            .any(|k| value_contains_custom_inspect(rt, &rt.object_get(*id, k), sym, seen))
    } else {
        false
    };
    seen.pop();
    found
}

fn selected_sorted_comparator_value(rt: &Runtime, arg: Option<&Value>) -> Option<Value> {
    let Some(Value::Object(opts)) = arg else {
        return None;
    };
    let sorted = rt.object_get(*opts, "sorted");
    rt.is_callable(&sorted).then_some(sorted)
}

fn sort_rendered_entries_with_comparator(
    rt: &mut Runtime,
    entries: &mut [String],
    comparator: Value,
) -> Result<(), RuntimeError> {
    for i in 1..entries.len() {
        let mut j = i;
        while j > 0 {
            let result = rt.call_function(
                comparator.clone(),
                Value::Undefined,
                vec![
                    util_string_value(&entries[j]),
                    util_string_value(&entries[j - 1]),
                ],
            )?;
            let n = rt.coerce_to_number(&result)?;
            if n < 0.0 {
                entries.swap(j - 1, j);
                j -= 1;
            } else {
                break;
            }
        }
    }
    Ok(())
}

fn inspect_selected_comparator_suffix_entries(
    rt: &mut Runtime,
    id: ObjectRef,
    depth: i32,
    max_depth: i32,
    break_length: usize,
    comparator: &Value,
    max_string_length: Option<usize>,
    max_array_length: Option<usize>,
    colors: bool,
    show_hidden: bool,
    array_len: usize,
    include_length: bool,
    seen: &mut Vec<ObjectRef>,
) -> Result<Vec<String>, RuntimeError> {
    let mut entries = Vec::new();
    for k in rt.ordinary_own_enumerable_string_keys(id) {
        if k == "length" || k.parse::<usize>().is_ok_and(|idx| idx < array_len) {
            continue;
        }
        let kd = if is_ident_key(&k) {
            k.clone()
        } else {
            inspect_str(&k)
        };
        let rendered = rt
            .obj(id)
            .get_own(&k)
            .and_then(|d| inspect_accessor_placeholder(d.getter.is_some(), d.setter.is_some()))
            .map(str::to_string)
            .map(Ok)
            .unwrap_or_else(|| {
                let val = rt.object_get(id, &k);
                inspect_value_with_selected_sorted_comparator(
                    rt,
                    &val,
                    depth + 1,
                    max_depth,
                    break_length,
                    comparator,
                    max_string_length,
                    max_array_length,
                    colors,
                    show_hidden,
                    seen,
                )
            })?;
        entries.push(format!("{}: {}", kd, rendered));
    }
    if show_hidden && include_length {
        entries.push(format!("[length]: {}", array_len));
    }
    let mut hidden_keys = if show_hidden {
        inspect_hidden_own_string_keys(rt, id, array_len, false)
    } else {
        Vec::new()
    };
    for k in hidden_keys.drain(..) {
        let rendered = rt
            .obj(id)
            .get_own(&k)
            .and_then(|d| inspect_accessor_placeholder(d.getter.is_some(), d.setter.is_some()))
            .map(str::to_string)
            .map(Ok)
            .unwrap_or_else(|| {
                let val = rt.object_get(id, &k);
                inspect_value_with_selected_sorted_comparator(
                    rt,
                    &val,
                    depth + 1,
                    max_depth,
                    break_length,
                    comparator,
                    max_string_length,
                    max_array_length,
                    colors,
                    show_hidden,
                    seen,
                )
            })?;
        entries.push(format!("{}: {}", inspect_hidden_key(&k), rendered));
    }
    for sym in inspect_own_symbol_keys(rt, id, true) {
        entries.push(inspect_symbol_property_entry(
            rt,
            id,
            &sym,
            false,
            depth,
            max_depth,
            break_length,
            false,
            max_string_length,
            max_array_length,
            colors,
            show_hidden,
            seen,
        ));
    }
    if show_hidden {
        for sym in inspect_own_symbol_keys(rt, id, false) {
            entries.push(inspect_symbol_property_entry(
                rt,
                id,
                &sym,
                true,
                depth,
                max_depth,
                break_length,
                false,
                max_string_length,
                max_array_length,
                colors,
                show_hidden,
                seen,
            ));
        }
    }
    Ok(entries)
}

fn inspect_value_with_selected_sorted_comparator(
    rt: &mut Runtime,
    value: &Value,
    depth: i32,
    max_depth: i32,
    break_length: usize,
    comparator: &Value,
    max_string_length: Option<usize>,
    max_array_length: Option<usize>,
    colors: bool,
    show_hidden: bool,
    seen: &mut Vec<ObjectRef>,
) -> Result<String, RuntimeError> {
    let Value::Object(id) = value else {
        return Ok(inspect_value_inner(
            rt,
            value,
            depth,
            max_depth,
            break_length,
            false,
            max_string_length,
            max_array_length,
            colors,
            show_hidden,
            seen,
        ));
    };
    if seen.contains(id) {
        return Ok("[Circular]".to_string());
    }
    if depth > max_depth && inspect_depth_limit_has_content(rt, *id, show_hidden) {
        let is_array = matches!(rt.obj(*id).internal_kind, InternalKind::Array);
        return Ok(inspect_depth_limit_label(rt, *id, is_array));
    }
    if matches!(rt.obj(*id).internal_kind, InternalKind::Array) {
        seen.push(*id);
        let len = rt.array_length(*id);
        let cap = max_array_length.unwrap_or(len);
        let take = len.min(cap);
        let mut items = Vec::new();
        for idx in 0..take {
            items.push(inspect_value_with_selected_sorted_comparator(
                rt,
                &rt.object_get(*id, &idx.to_string()),
                depth + 1,
                max_depth,
                break_length,
                comparator,
                max_string_length,
                max_array_length,
                colors,
                show_hidden,
                seen,
            )?);
        }
        if take < len {
            items.push(format!("... {} more items", len - take));
        }
        let mut suffix_entries = Vec::new();
        for k in rt.ordinary_own_enumerable_string_keys(*id) {
            if k == "length" || k.parse::<usize>().is_ok_and(|idx| idx < len) {
                continue;
            }
            let kd = if is_ident_key(&k) {
                k.clone()
            } else {
                inspect_str(&k)
            };
            let rendered = rt
                .obj(*id)
                .get_own(&k)
                .and_then(|d| inspect_accessor_placeholder(d.getter.is_some(), d.setter.is_some()))
                .map(str::to_string)
                .map(Ok)
                .unwrap_or_else(|| {
                    let val = rt.object_get(*id, &k);
                    inspect_value_with_selected_sorted_comparator(
                        rt,
                        &val,
                        depth + 1,
                        max_depth,
                        break_length,
                        comparator,
                        max_string_length,
                        max_array_length,
                        colors,
                        show_hidden,
                        seen,
                    )
                })?;
            suffix_entries.push(format!("{}: {}", kd, rendered));
        }
        if show_hidden {
            suffix_entries.push(format!("[length]: {}", len));
        }
        let mut hidden_keys = if show_hidden {
            inspect_hidden_own_string_keys(rt, *id, len, false)
        } else {
            Vec::new()
        };
        for k in hidden_keys.drain(..) {
            let rendered = rt
                .obj(*id)
                .get_own(&k)
                .and_then(|d| inspect_accessor_placeholder(d.getter.is_some(), d.setter.is_some()))
                .map(str::to_string)
                .map(Ok)
                .unwrap_or_else(|| {
                    let val = rt.object_get(*id, &k);
                    inspect_value_with_selected_sorted_comparator(
                        rt,
                        &val,
                        depth + 1,
                        max_depth,
                        break_length,
                        comparator,
                        max_string_length,
                        max_array_length,
                        colors,
                        show_hidden,
                        seen,
                    )
                })?;
            suffix_entries.push(format!("{}: {}", inspect_hidden_key(&k), rendered));
        }
        for sym in inspect_own_symbol_keys(rt, *id, true) {
            suffix_entries.push(inspect_symbol_property_entry(
                rt,
                *id,
                &sym,
                false,
                depth,
                max_depth,
                break_length,
                false,
                max_string_length,
                max_array_length,
                colors,
                show_hidden,
                seen,
            ));
        }
        if show_hidden {
            for sym in inspect_own_symbol_keys(rt, *id, false) {
                suffix_entries.push(inspect_symbol_property_entry(
                    rt,
                    *id,
                    &sym,
                    true,
                    depth,
                    max_depth,
                    break_length,
                    false,
                    max_string_length,
                    max_array_length,
                    colors,
                    show_hidden,
                    seen,
                ));
            }
        }
        sort_rendered_entries_with_comparator(rt, &mut suffix_entries, comparator.clone())?;
        items.extend(suffix_entries);
        let force_multiline =
            inspect_force_nested_group_multiline("[", "]", &items, depth, max_depth, break_length);
        let body = inspect_join_aggregate(
            "[",
            "]",
            &items,
            depth,
            if force_multiline { 0 } else { break_length },
        );
        seen.pop();
        return Ok(inspect_array_wrapper(rt, *id, len, body));
    }
    if rt.obj(*id).has_own_str("__map_data") {
        if let Value::Object(storage) = rt.object_get(*id, "__map_data") {

            if depth > max_depth && !rt.ordinary_own_enumerable_string_keys(storage).is_empty() {
                return Ok("[Map]".to_string());
            }
            seen.push(*id);
            let orig = rt.object_get(*id, "__map_orig_keys");
            let skeys = rt.ordinary_own_enumerable_string_keys(storage);
            let mut entries = Vec::new();
            for sk in &skeys {
                let val = rt.object_get(storage, sk);
                let keyv = match &orig {
                    Value::Object(o) if rt.obj(*o).has_own_str(sk) => rt.object_get(*o, sk),
                    _ => {
                        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(sk.clone())))
                    }
                };
                entries.push(format!(
                    "{} => {}",
                    inspect_value_with_selected_sorted_comparator(
                        rt,
                        &keyv,
                        depth + 1,
                        max_depth,
                        break_length,
                        comparator,
                        max_string_length,
                        max_array_length,
                        colors,
                        show_hidden,
                        seen,
                    )?,
                    inspect_value_with_selected_sorted_comparator(
                        rt,
                        &val,
                        depth + 1,
                        max_depth,
                        break_length,
                        comparator,
                        max_string_length,
                        max_array_length,
                        colors,
                        show_hidden,
                        seen,
                    )?
                ));
            }
            entries.extend(inspect_selected_comparator_suffix_entries(
                rt,
                *id,
                depth,
                max_depth,
                break_length,
                comparator,
                max_string_length,
                max_array_length,
                colors,
                show_hidden,
                0,
                false,
                seen,
            )?);
            sort_rendered_entries_with_comparator(rt, &mut entries, comparator.clone())?;
            let body = if entries.len() >= 5 {
                inspect_multiline_block("{", "}", &entries, depth)
            } else {
                inspect_join_aggregate("{", "}", &entries, depth, break_length)
            };
            let rendered = format!("Map({}) {}", skeys.len(), body);
            seen.pop();
            return Ok(rendered);
        }
    }
    if rt.obj(*id).has_own_str("__set_data") {
        if let Value::Object(storage) = rt.object_get(*id, "__set_data") {

            if depth > max_depth && !rt.ordinary_own_enumerable_string_keys(storage).is_empty() {
                return Ok("[Set]".to_string());
            }
            seen.push(*id);
            let skeys = rt.ordinary_own_enumerable_string_keys(storage);
            let mut entries = Vec::new();
            for sk in &skeys {
                entries.push(inspect_value_with_selected_sorted_comparator(
                    rt,
                    &rt.object_get(storage, sk),
                    depth + 1,
                    max_depth,
                    break_length,
                    comparator,
                    max_string_length,
                    max_array_length,
                    colors,
                    show_hidden,
                    seen,
                )?);
            }
            entries.extend(inspect_selected_comparator_suffix_entries(
                rt,
                *id,
                depth,
                max_depth,
                break_length,
                comparator,
                max_string_length,
                max_array_length,
                colors,
                show_hidden,
                0,
                false,
                seen,
            )?);
            sort_rendered_entries_with_comparator(rt, &mut entries, comparator.clone())?;
            let rendered = format!(
                "Set({}) {}",
                skeys.len(),
                inspect_join_aggregate("{", "}", &entries, depth, break_length)
            );
            seen.pop();
            return Ok(rendered);
        }
    }
    if let Some(rendered) = inspect_selected_sorted_comparator_typed_view(
        rt,
        *id,
        comparator,
        depth,
        max_depth,
        break_length,
        max_string_length,
        max_array_length,
        colors,
        show_hidden,
        seen,
    ) {
        return rendered;
    }
    if matches!(rt.obj(*id).internal_kind, InternalKind::Error) {
        if let Some(error) = inspect_error_projection(rt, *id, colors) {
            seen.push(*id);
            let ref_index = seen.len() - 1;
            let suffix = inspect_error_suffix_selected_comparator(
                rt,
                *id,
                depth,
                max_depth,
                break_length,
                comparator,
                max_string_length,
                max_array_length,
                colors,
                show_hidden,
                error.contains('\n'),
                seen,
            )?;

            if depth > max_depth && !suffix.is_empty() {
                seen.pop();
                return Ok(format!("[{}]", util_ctor_name(rt, *id)));
            }

            let error = indent_continuation_lines(&error, (depth.max(0) as usize) * 2);
            let rendered = inspect_ref_prefix(format!("{}{}", error, suffix), ref_index);
            seen.pop();
            return Ok(rendered);
        }
    }
    if !matches!(rt.obj(*id).internal_kind, InternalKind::Ordinary) {
        return Ok(inspect_value_inner(
            rt,
            value,
            depth,
            max_depth,
            break_length,
            false,
            max_string_length,
            max_array_length,
            colors,
            show_hidden,
            seen,
        ));
    }
    seen.push(*id);
    let symbol_keys = inspect_own_symbol_keys(rt, *id, true);
    let mut hidden_keys = if show_hidden {
        inspect_hidden_own_string_keys(rt, *id, 0, false)
    } else {
        Vec::new()
    };
    let hidden_symbol_keys = if show_hidden {
        inspect_own_symbol_keys(rt, *id, false)
    } else {
        Vec::new()
    };
    let mut entries: Vec<String> = rt
        .ordinary_own_enumerable_string_keys(*id)
        .iter()
        .map(|k| {
            let kd = if is_ident_key(k) {
                k.clone()
            } else {
                inspect_str(k)
            };
            let rendered = rt
                .obj(*id)
                .get_own(k)
                .and_then(|d| inspect_accessor_placeholder(d.getter.is_some(), d.setter.is_some()))
                .map(str::to_string)
                .map(Ok)
                .unwrap_or_else(|| {
                    let val = rt.object_get(*id, k);
                    inspect_value_with_selected_sorted_comparator(
                        rt,
                        &val,
                        depth + 1,
                        max_depth,
                        break_length,
                        comparator,
                        max_string_length,
                        max_array_length,
                        colors,
                        show_hidden,
                        seen,
                    )
                })?;
            Ok(format!("{}: {}", kd, rendered))
        })
        .collect::<Result<Vec<_>, RuntimeError>>()?;
    for k in hidden_keys.drain(..) {
        let rendered = rt
            .obj(*id)
            .get_own(&k)
            .and_then(|d| inspect_accessor_placeholder(d.getter.is_some(), d.setter.is_some()))
            .map(str::to_string)
            .map(Ok)
            .unwrap_or_else(|| {
                let val = rt.object_get(*id, &k);
                inspect_value_with_selected_sorted_comparator(
                    rt,
                    &val,
                    depth + 1,
                    max_depth,
                    break_length,
                    comparator,
                    max_string_length,
                    max_array_length,
                    colors,
                    show_hidden,
                    seen,
                )
            })?;
        entries.push(format!("{}: {}", inspect_hidden_key(&k), rendered));
    }
    for sym in symbol_keys {
        entries.push(inspect_symbol_property_entry(
            rt,
            *id,
            &sym,
            false,
            depth,
            max_depth,
            break_length,
            false,
            max_string_length,
            max_array_length,
            colors,
            show_hidden,
            seen,
        ));
    }
    for sym in hidden_symbol_keys {
        entries.push(inspect_symbol_property_entry(
            rt,
            *id,
            &sym,
            true,
            depth,
            max_depth,
            break_length,
            false,
            max_string_length,
            max_array_length,
            colors,
            show_hidden,
            seen,
        ));
    }
    sort_rendered_entries_with_comparator(rt, &mut entries, comparator.clone())?;
    let force_multiline =
        inspect_force_nested_group_multiline("{", "}", &entries, depth, max_depth, break_length);
    let body = inspect_join_aggregate(
        "{",
        "}",
        &entries,
        depth,
        if force_multiline { 0 } else { break_length },
    );
    seen.pop();
    Ok(inspect_object_wrapper(rt, *id, body))
}

fn inspect_selected_sorted_comparator_object(
    rt: &mut Runtime,
    id: ObjectRef,
    comparator: Value,
    max_depth: i32,
    break_length: usize,
    max_string_length: Option<usize>,
    max_array_length: Option<usize>,
    colors: bool,
    show_hidden: bool,
) -> Result<Option<String>, RuntimeError> {
    let kind = &rt.obj(id).internal_kind;
    if matches!(kind, InternalKind::Array)
        || rt.obj(id).has_own_str("__map_data")
        || rt.obj(id).has_own_str("__set_data")
        || rt.typed_array_views.get(&id).is_some()
        || matches!(kind, InternalKind::Error)
    {
        let mut seen = Vec::new();
        return inspect_value_with_selected_sorted_comparator(
            rt,
            &Value::Object(id),
            0,
            max_depth,
            break_length,
            &comparator,
            max_string_length,
            max_array_length,
            colors,
            show_hidden,
            &mut seen,
        )
        .map(Some);
    }
    if !matches!(kind, InternalKind::Ordinary) {
        return Ok(None);
    }
    let mut seen = vec![id];
    let symbol_keys = inspect_own_symbol_keys(rt, id, true);
    let mut hidden_keys = if show_hidden {
        inspect_hidden_own_string_keys(rt, id, 0, false)
    } else {
        Vec::new()
    };
    let hidden_symbol_keys = if show_hidden {
        inspect_own_symbol_keys(rt, id, false)
    } else {
        Vec::new()
    };
    let mut entries: Vec<String> = rt
        .ordinary_own_enumerable_string_keys(id)
        .iter()
        .map(|k| {
            let kd = if is_ident_key(k) {
                k.clone()
            } else {
                inspect_str(k)
            };
            let rendered = rt
                .obj(id)
                .get_own(k)
                .and_then(|d| inspect_accessor_placeholder(d.getter.is_some(), d.setter.is_some()))
                .map(str::to_string)
                .map(Ok)
                .unwrap_or_else(|| {
                    let val = rt.object_get(id, k);
                    inspect_value_with_selected_sorted_comparator(
                        rt,
                        &val,
                        1,
                        max_depth,
                        break_length,
                        &comparator,
                        max_string_length,
                        max_array_length,
                        colors,
                        show_hidden,
                        &mut seen,
                    )
                })?;
            Ok(format!("{}: {}", kd, rendered))
        })
        .collect::<Result<Vec<_>, RuntimeError>>()?;
    entries.extend(hidden_keys.drain(..).map(|k| {
        let rendered = rt
            .obj(id)
            .get_own(&k)
            .and_then(|d| inspect_accessor_placeholder(d.getter.is_some(), d.setter.is_some()))
            .map(str::to_string)
            .unwrap_or_else(|| {
                let val = rt.object_get(id, &k);
                inspect_value_inner(
                    rt,
                    &val,
                    1,
                    max_depth,
                    break_length,
                    false,
                    max_string_length,
                    max_array_length,
                    colors,
                    show_hidden,
                    &mut seen,
                )
            });
        format!("{}: {}", inspect_hidden_key(&k), rendered)
    }));
    for sym in symbol_keys {
        entries.push(inspect_symbol_property_entry(
            rt,
            id,
            &sym,
            false,
            0,
            max_depth,
            break_length,
            false,
            max_string_length,
            max_array_length,
            colors,
            show_hidden,
            &mut seen,
        ));
    }
    for sym in hidden_symbol_keys {
        entries.push(inspect_symbol_property_entry(
            rt,
            id,
            &sym,
            true,
            0,
            max_depth,
            break_length,
            false,
            max_string_length,
            max_array_length,
            colors,
            show_hidden,
            &mut seen,
        ));
    }
    if entries.is_empty() {
        return Ok(Some(inspect_object_wrapper(rt, id, "{}".to_string())));
    }
    sort_rendered_entries_with_comparator(rt, &mut entries, comparator)?;
    Ok(Some(inspect_object_wrapper(
        rt,
        id,
        inspect_join_aggregate("{", "}", &entries, 0, break_length),
    )))
}

fn util_is_json_circular_error(rt: &Runtime, err: &RuntimeError) -> bool {
    match err {
        RuntimeError::TypeError(msg) => msg.contains("Converting circular structure to JSON"),
        RuntimeError::Thrown(Value::Object(id)) => match rt.object_get(*id, "message") {
            Value::String(msg) => msg
                .as_str()
                .contains("Converting circular structure to JSON"),
            _ => false,
        },
        RuntimeError::Thrown(Value::String(msg)) => msg
            .as_str()
            .contains("Converting circular structure to JSON"),
        _ => false,
    }
}

fn inspect_getter_throw_value(rt: &Runtime, err: &RuntimeError) -> Option<String> {
    let rendered = match err {
        RuntimeError::Thrown(Value::String(s)) => inspect_str(s.as_str()),
        RuntimeError::Thrown(Value::Number(n)) => format_number_preserve_negative_zero(*n),
        RuntimeError::Thrown(Value::Boolean(b)) => b.to_string(),
        RuntimeError::Thrown(Value::Null) => "null".to_string(),
        RuntimeError::Thrown(Value::Undefined) => "undefined".to_string(),
        RuntimeError::Thrown(Value::Symbol(sym)) => {
            abstract_ops::to_string(&Value::Symbol(sym.clone())).to_string()
        }
        RuntimeError::Thrown(Value::Object(id))
            if matches!(rt.obj(*id).internal_kind, InternalKind::Error) =>
        {
            inspect_error_projection(rt, *id, false)?
        }
        _ => return None,
    };
    Some(format!("[Getter: <Inspection threw ({rendered})>]"))
}

fn inspect_str(s: &str) -> String {
    inspect_str_with_max(s, None)
}

fn inspect_colorize(rendered: String, v: &Value) -> String {
    match v {
        Value::Undefined => format!("\x1b[90m{rendered}\x1b[39m"),
        Value::Null => format!("\x1b[1m{rendered}\x1b[22m"),
        Value::Boolean(_) | Value::Number(_) | Value::BigInt(_) => {
            format!("\x1b[33m{rendered}\x1b[39m")
        }
        Value::String(_) | Value::Symbol(_) => format!("\x1b[32m{rendered}\x1b[39m"),
        _ => rendered,
    }
}

fn inspect_colorize_special(rendered: String, code: &str) -> String {
    let close = if code == "1" { "22" } else { "39" };
    format!("\x1b[{code}m{rendered}\x1b[{close}m")
}

fn inspect_colorize_regexp(src: &str, flags: &str) -> String {
    let mut out = String::new();
    out.push_str(&inspect_colorize_special("/".to_string(), "32"));
    let mut chars = src.chars().peekable();
    let mut in_class = false;
    let mut group_depth = 0usize;
    let mut in_quantifier = false;
    let mut quantifier_after_class_escape = false;
    let mut in_named_backref = false;
    let mut in_group_name = false;
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            if matches!(chars.peek(), Some('p' | 'P')) {
                let p = chars.next().unwrap();
                if matches!(chars.peek(), Some('{')) {
                    chars.next();
                    out.push_str(&inspect_colorize_special(format!("\\{p}{{"), "31"));
                    continue;
                }
                out.push_str(&inspect_colorize_special(format!("\\{p}"), "33"));
                continue;
            }
            if matches!(chars.peek(), Some('k')) {
                chars.next();
                if matches!(chars.peek(), Some('<')) {
                    chars.next();
                    out.push_str(&inspect_colorize_special("\\k<".to_string(), "32"));
                    in_named_backref = true;
                    continue;
                }
                out.push_str(&inspect_colorize_special("\\k".to_string(), "33"));
                continue;
            }
            let mut atom = String::from("\\");
            if let Some(next) = chars.next() {
                atom.push(next);
            }
            let code = match atom.as_str() {
                "\\d" | "\\D" | "\\s" | "\\S" | "\\w" | "\\W" => "36",
                _ => "33",
            };
            quantifier_after_class_escape = code == "36";
            out.push_str(&inspect_colorize_special(atom, code));
            continue;
        }
        if in_named_backref {
            if ch == '>' {
                in_named_backref = false;
                out.push_str(&inspect_colorize_special(ch.to_string(), "32"));
            } else {
                out.push_str(&inspect_colorize_special(ch.to_string(), "31"));
            }
            continue;
        }
        if in_group_name {
            if ch == '>' {
                in_group_name = false;
                out.push_str(&inspect_colorize_special(ch.to_string(), "31"));
            } else {
                out.push_str(&inspect_colorize_special(ch.to_string(), "33"));
            }
            continue;
        }
        if in_quantifier {
            let code = match ch {
                '}' => {
                    in_quantifier = false;
                    if quantifier_after_class_escape {
                        "33"
                    } else {
                        "31"
                    }
                }
                '0'..='9' => {
                    if quantifier_after_class_escape {
                        "35"
                    } else {
                        "36"
                    }
                }
                ',' => "33",
                _ => "33",
            };
            out.push_str(&inspect_colorize_special(ch.to_string(), code));
            if !in_quantifier {
                quantifier_after_class_escape = false;
            }
            continue;
        }
        let code = match ch {
            '[' => {
                in_class = true;
                "31"
            }
            ']' => {
                in_class = false;
                "31"
            }
            '(' => {
                group_depth += 1;
                "31"
            }
            ')' => {
                group_depth = group_depth.saturating_sub(1);
                "31"
            }
            '?' if group_depth > 0 && matches!(chars.peek(), Some('<')) => {
                chars.next();
                if matches!(chars.peek(), Some('=' | '!')) {
                    let next = chars.next().unwrap();
                    out.push_str(&inspect_colorize_special(format!("?<{next}"), "31"));
                    continue;
                }
                out.push_str(&inspect_colorize_special("?<".to_string(), "31"));
                in_group_name = true;
                continue;
            }
            '?' if group_depth > 0 && matches!(chars.peek(), Some('=' | '!' | ':')) => {
                let next = chars.next().unwrap();
                out.push_str(&inspect_colorize_special(format!("?{next}"), "31"));
                continue;
            }
            '=' | '!' if group_depth > 0 => "31",
            '>' if group_depth > 0 => "31",
            '{' if !in_class => {
                in_quantifier = true;
                if quantifier_after_class_escape {
                    "33"
                } else {
                    "31"
                }
            }
            '}' if !in_class => "31",
            '-' => "36",
            '.' if !in_class => "36",
            '|' => "35",
            '^' if in_class => "33",
            '$' if in_class => "33",
            '*' | '+' | '?' | '^' | '$' => "35",
            _ if group_depth > 0 && !in_class => "36",
            _ => "33",
        };
        out.push_str(&inspect_colorize_special(ch.to_string(), code));
        if ch != '{' {
            quantifier_after_class_escape = false;
        }
    }
    out.push_str(&inspect_colorize_special("/".to_string(), "32"));
    if !flags.is_empty() {
        out.push_str(&inspect_colorize_special(flags.to_string(), "31"));
    }
    out
}

fn inspect_jsstring_with_max(
    s: &std::rc::Rc<rusty_js_runtime::value::JsString>,
    max_string: Option<usize>,
) -> String {
    if s.is_well_formed() {
        return inspect_str_with_max(s.as_str(), max_string);
    }

    const MAX_STRING: usize = 10000;

    enum Cu {
        Ch(char),
        Lone(u16),
    }
    let units = s.code_units();
    let mut chars: Vec<Cu> = Vec::with_capacity(units.len());
    let mut i = 0;
    while i < units.len() {
        let u = units[i];
        if (0xD800..=0xDBFF).contains(&u)
            && i + 1 < units.len()
            && (0xDC00..=0xDFFF).contains(&units[i + 1])
        {
            let cp = 0x10000 + (((u as u32) - 0xD800) << 10) + ((units[i + 1] as u32) - 0xDC00);
            chars.push(Cu::Ch(char::from_u32(cp).unwrap_or('\u{FFFD}')));
            i += 2;
        } else if (0xD800..=0xDFFF).contains(&u) {
            chars.push(Cu::Lone(u));
            i += 1;
        } else {
            chars.push(Cu::Ch(char::from_u32(u as u32).unwrap_or('\u{FFFD}')));
            i += 1;
        }
    }
    let multiline = chars.iter().any(|c| matches!(c, Cu::Ch('\n')));
    let max_string = max_string.unwrap_or(if multiline { 20 } else { MAX_STRING });
    let total = chars.len();
    let truncated = total > max_string;
    let body = if truncated {
        &chars[..max_string]
    } else {
        &chars[..]
    };

    let has = |q: char| body.iter().any(|c| matches!(c, Cu::Ch(x) if *x == q));
    let quote = if !has('\'') {
        '\''
    } else if !has('"') {
        '"'
    } else if !has('`')
        && !body
            .windows(2)
            .any(|w| matches!((&w[0], &w[1]), (Cu::Ch('$'), Cu::Ch('{'))))
    {
        '`'
    } else {
        '\''
    };
    let mut o = String::with_capacity(units.len() + 5);
    o.push(quote);
    for c in body {
        match c {
            Cu::Lone(u) => o.push_str(&format!("\\u{u:04x}")),
            Cu::Ch(ch) => match *ch {
                '\\' => o.push_str("\\\\"),
                '\u{08}' => o.push_str("\\b"),
                '\t' => o.push_str("\\t"),
                '\n' => o.push_str("\\n"),
                '\u{0C}' => o.push_str("\\f"),
                '\r' => o.push_str("\\r"),
                x if x == quote => {
                    o.push('\\');
                    o.push(x);
                }
                x if (x as u32) < 0x20 || x as u32 == 0x7F => {
                    o.push_str(&format!("\\x{:02X}", x as u32));
                }
                x => o.push(x),
            },
        }
    }
    o.push(quote);
    if truncated {
        let remaining = total - max_string;
        o.push_str("...");
        o.push(' ');
        o.push_str(&remaining.to_string());
        o.push_str(" more character");
        if remaining != 1 {
            o.push('s');
        }
    }
    o
}

fn inspect_str_with_max(s: &str, max_string: Option<usize>) -> String {

    const MAX_STRING: usize = 10000;
    let multiline = s.contains('\n');
    let max_string = max_string.unwrap_or(if multiline { 20 } else { MAX_STRING });
    let char_count = s.chars().count();
    let truncated = char_count > max_string;
    let body: String = if truncated {
        s.chars().take(max_string).collect()
    } else {
        s.to_string()
    };

    let quote = if !body.contains('\'') {
        '\''
    } else if !body.contains('"') {
        '"'
    } else if !body.contains('`') && !body.contains("${") {
        '`'
    } else {
        '\''
    };
    let mut o = String::with_capacity(body.len() + 5);
    o.push(quote);
    for c in body.chars() {
        match c {
            '\\' => o.push_str("\\\\"),

            '\u{08}' => o.push_str("\\b"),
            '\t' => o.push_str("\\t"),
            '\n' => o.push_str("\\n"),
            '\u{0C}' => o.push_str("\\f"),
            '\r' => o.push_str("\\r"),

            _ if c == quote => {
                o.push('\\');
                o.push(c);
            }

            c if (c as u32) < 0x20 || c as u32 == 0x7F => {
                o.push_str(&format!("\\x{:02X}", c as u32));
            }
            _ => o.push(c),
        }
    }
    o.push(quote);
    if truncated {
        let remaining = char_count - max_string;
        o.push_str("...");
        o.push(' ');
        o.push_str(&remaining.to_string());
        o.push_str(" more character");
        if remaining != 1 {
            o.push('s');
        }
    }
    o
}

fn inspect_accessor_placeholder(has_getter: bool, has_setter: bool) -> Option<&'static str> {
    match (has_getter, has_setter) {
        (true, true) => Some("[Getter/Setter]"),
        (true, false) => Some("[Getter]"),
        (false, true) => Some("[Setter]"),
        (false, false) => None,
    }
}

fn is_ident_key(k: &str) -> bool {
    let mut it = k.chars();
    match it.next() {
        Some(c) if c == '_' || c == '$' || c.is_ascii_alphabetic() => {}
        _ => return false,
    }
    it.all(|c| c == '_' || c == '$' || c.is_ascii_alphanumeric())
}

fn inspect_join_aggregate(
    open: &str,
    close: &str,
    entries: &[String],
    depth: i32,
    break_length: usize,
) -> String {
    if entries.is_empty() {
        return format!("{open}{close}");
    }

    let n = entries.len();
    let indent_lvl = (depth.max(0) as usize) * 2;
    let start = n + indent_lvl + open.chars().count() + 10;
    let any_multiline = entries.iter().any(|e| e.contains('\n'));
    let below = {
        let mut total = n + start;
        if any_multiline || total + n > break_length {
            false
        } else {
            let mut ok = true;
            for e in entries {
                total += e.chars().count();
                if total > break_length {
                    ok = false;
                    break;
                }
            }
            ok
        }
    };

    let compact_ok = INSPECT_CURRENT_DEPTH.with(|c| c.get()) - depth < INSPECT_COMPACT;
    if break_length > 0 && below && compact_ok {
        return format!("{open} {} {close}", entries.join(", "));
    }
    let inner_indent = "  ".repeat((depth + 1).max(0) as usize);
    let outer_indent = "  ".repeat(depth.max(0) as usize);
    let sep = format!(",\n{inner_indent}");
    format!(
        "{open}\n{inner_indent}{}\n{outer_indent}{close}",
        entries.join(&sep)
    )
}

fn inspect_join_promise_aggregate(entries: &[String], depth: i32, break_length: usize) -> String {
    if !entries.iter().any(|entry| entry.contains('\n')) {
        return inspect_join_aggregate("{", "}", entries, depth, break_length);
    }
    let inner_indent = "  ".repeat((depth + 1).max(0) as usize);
    let outer_indent = "  ".repeat(depth.max(0) as usize);
    let mut rendered = Vec::with_capacity(entries.len());
    for entry in entries {
        rendered.push(
            entry
                .lines()
                .map(|line| format!("{inner_indent}{line}"))
                .collect::<Vec<_>>()
                .join("\n"),
        );
    }
    let sep = ",\n";
    format!("{{\n{}\n{outer_indent}}}", rendered.join(sep))
}

fn inspect_indent_stack_frame_lines(rendered: String, indent: &str) -> String {
    if !rendered.contains('\n') {
        return rendered;
    }
    let mut out = String::with_capacity(rendered.len() + indent.len() * 2);
    let mut lines = rendered.split('\n');
    if let Some(first) = lines.next() {
        out.push_str(first);
    }
    for line in lines {
        out.push('\n');
        if line.trim_start().starts_with("at ") {
            out.push_str(indent);
        }
        out.push_str(line);
    }
    out
}

fn inspect_indent_aggregate_error_item_lines(rendered: String, depth: i32) -> String {
    if !rendered.contains('\n') {
        return rendered;
    }
    let stack_indent = "  ".repeat((depth + 2).max(0) as usize);
    let metadata_indent = "  ".repeat((depth + 1).max(0) as usize);
    let mut out = String::with_capacity(rendered.len() + stack_indent.len() * 2);
    let mut lines = rendered.split('\n');
    if let Some(first) = lines.next() {
        out.push_str(first);
    }
    for line in lines {
        out.push('\n');
        if !line.is_empty() {
            if line.trim_start().starts_with("at ") {
                out.push_str(&stack_indent);
            } else {
                out.push_str(&metadata_indent);
            }
        }
        out.push_str(line);
    }
    out
}

fn inspect_is_numeric_atom(s: &str) -> bool {
    s == "NaN" || s == "Infinity" || s == "-Infinity" || s.parse::<f64>().is_ok() || (s == "-0")
}

fn inspect_is_scalar_array_atom(s: &str) -> bool {
    !s.contains('\n') && !s.contains('{') && !s.contains('[') && !s.contains(':')
}

fn inspect_is_groupable_array_atom(s: &str) -> bool {
    !s.contains('\n')
}

fn inspect_join_columnar_array(entries: &[String], depth: i32, columns: usize) -> String {
    let columns = columns.min(entries.len()).max(1);
    let mut col_widths = vec![0usize; columns];
    for (idx, entry) in entries.iter().enumerate() {
        let col = idx % columns;
        col_widths[col] = col_widths[col].max(entry.chars().count());
    }
    inspect_join_scalar_columnar_array_with_widths(entries, depth, columns, &col_widths, None)
}

fn inspect_join_mixed_scalar_array(entries: &[String], depth: i32, columns: usize) -> String {
    let columns = columns.min(entries.len()).max(1);
    let mut col_widths = vec![0usize; columns];
    let mut shifted_quote_rows = vec![None; columns];
    for (idx, entry) in entries.iter().enumerate() {
        let col = idx % columns;
        let row = idx / columns;
        let len = entry.chars().count();
        let width = if entry.starts_with('\'') && col + 1 < columns {
            col_widths[col + 1] = col_widths[col + 1].max(len);
            shifted_quote_rows[col + 1] = Some(
                shifted_quote_rows[col + 1]
                    .map(|existing: usize| existing.min(row))
                    .unwrap_or(row),
            );
            0
        } else {
            len
        };
        col_widths[col] = col_widths[col].max(width);
    }
    inspect_join_scalar_columnar_array_with_widths(
        entries,
        depth,
        columns,
        &col_widths,
        Some(&shifted_quote_rows),
    )
}

fn inspect_join_scalar_columnar_array_with_widths(
    entries: &[String],
    depth: i32,
    columns: usize,
    col_widths: &[usize],
    shifted_quote_rows: Option<&[Option<usize>]>,
) -> String {
    let inner_indent = "  ".repeat((depth + 1).max(0) as usize);
    let outer_indent = "  ".repeat(depth.max(0) as usize);
    let mut out = String::from("[\n");
    for (row_idx, row) in entries.chunks(columns).enumerate() {
        if row_idx > 0 {
            out.push('\n');
        }
        out.push_str(&inner_indent);
        for (col, entry) in row.iter().enumerate() {
            if col > 0 {
                out.push(' ');
            }
            let width = match shifted_quote_rows.and_then(|rows| rows.get(col).copied().flatten()) {
                Some(quote_row) if row_idx <= quote_row => entry.chars().count(),
                _ => col_widths.get(col).copied().unwrap_or(0),
            };
            let len = entry.chars().count();
            let shifted_right_align = shifted_quote_rows
                .and_then(|rows| rows.get(col).copied().flatten())
                .is_some_and(|quote_row| row_idx > quote_row);
            if shifted_right_align {
                for _ in 0..width.saturating_sub(len) {
                    out.push(' ');
                }
            }
            out.push_str(entry);
            if row_idx * columns + col + 1 < entries.len() {
                out.push(',');
            }
            if col + 1 < row.len() && !shifted_right_align {
                for _ in 0..width.saturating_sub(len) {
                    out.push(' ');
                }
            }
        }
    }
    out.push('\n');
    out.push_str(&outer_indent);
    out.push(']');
    out
}

fn inspect_join_numeric_columnar_array_with_widths(
    entries: &[String],
    depth: i32,
    columns: usize,
    col_widths: &[usize],
) -> String {
    let inner_indent = "  ".repeat((depth + 1).max(0) as usize);
    let outer_indent = "  ".repeat(depth.max(0) as usize);
    let mut out = String::from("[\n");
    for (row_idx, row) in entries.chunks(columns).enumerate() {
        if row_idx > 0 {
            out.push('\n');
        }
        out.push_str(&inner_indent);
        for (col, entry) in row.iter().enumerate() {
            if col > 0 {
                out.push(' ');
            }
            let width = col_widths.get(col).copied().unwrap_or(0);
            let len = entry.chars().count();
            for _ in 0..width.saturating_sub(len) {
                out.push(' ');
            }
            out.push_str(entry);
            if row_idx * columns + col + 1 < entries.len() {
                out.push(',');
            }
        }
    }
    out.push('\n');
    out.push_str(&outer_indent);
    out.push(']');
    out
}

fn numeric_grid_columns(entries: &[String], depth: i32, break_length: usize) -> usize {
    let n = entries.len();
    if n == 0 {
        return 1;
    }
    const SEPARATOR_SPACE: usize = 2;
    let indentation_lvl = (depth.max(0) as usize).saturating_mul(2);
    let mut total_length = 0usize;
    let mut max_length = 0usize;
    for e in entries {
        let len = e.chars().count();
        total_length += len + SEPARATOR_SPACE;
        if len > max_length {
            max_length = len;
        }
    }
    let actual_max = max_length + SEPARATOR_SPACE;

    if actual_max * 3 + indentation_lvl < break_length
        && (total_length as f64 / actual_max as f64 > 5.0 || max_length <= 6)
    {
        let approx_char_heights = 2.5f64;
        let average_bias = (actual_max as f64 - total_length as f64 / n as f64).sqrt();
        let biased_max = (actual_max as f64 - 3.0 - average_bias).max(1.0);
        let columns = ((approx_char_heights * biased_max * n as f64).sqrt() / biased_max)
            .round()
            .min(((break_length - indentation_lvl) as f64 / actual_max as f64).floor())

            .min(12.0)
            .min(15.0);
        let columns = columns as usize;
        if columns <= 1 {
            return 1;
        }
        return columns;
    }
    1
}

fn inspect_join_numeric_array(entries: &[String], depth: i32, break_length: usize) -> String {
    if entries.is_empty() {
        return "[]".to_string();
    }
    let inline = format!("[ {} ]", entries.join(", "));

    let columns = if entries.len() > 6 {
        numeric_grid_columns(entries, depth, break_length)
    } else {
        1
    };
    if columns <= 1 {
        if break_length > 0 && inline.chars().count() < break_length {
            return inline;
        }

    }
    let columns = columns.min(entries.len()).max(1);
    let mut col_widths = vec![0usize; columns];
    for (idx, entry) in entries.iter().enumerate() {
        let col = idx % columns;
        col_widths[col] = col_widths[col].max(entry.chars().count());
    }
    inspect_join_numeric_columnar_array_with_widths(entries, depth, columns, &col_widths)
}

fn inspect_array_grid_body(
    items: &[String],
    depth: i32,
    break_length: usize,
    force_multiline: bool,
) -> String {
    let (grid_src, trailing_more): (&[String], Option<&String>) = match items.split_last() {
        Some((last, rest)) if last.starts_with("... ") && !rest.is_empty() => (rest, Some(last)),
        _ => (items, None),
    };
    let numeric = break_length > 0 && grid_src.iter().all(|i| inspect_is_numeric_atom(i));
    let scalar_grid = break_length > 0
        && grid_src.len() > 6
        && grid_src.iter().all(|i| inspect_is_groupable_array_atom(i))
        && numeric_grid_columns(grid_src, depth, break_length) > 1;
    if numeric || scalar_grid {
        let grid = if numeric {
            inspect_join_numeric_array(grid_src, depth, break_length)
        } else {

            let columns = numeric_grid_columns(grid_src, depth, break_length);
            if grid_src.iter().any(|i| i.starts_with('\''))
                && !grid_src.iter().all(|i| i.starts_with('\''))
            {
                inspect_join_mixed_scalar_array(grid_src, depth, columns)
            } else {
                inspect_join_columnar_array(grid_src, depth, columns)
            }
        };

        if let Some(pos) = grid.rfind('\n') {
            if let Some(marker) = trailing_more {
                let inner = "  ".repeat((depth + 1).max(0) as usize);
                let (head, tail) = grid.split_at(pos);
                return format!("{head},\n{inner}{marker}{tail}");
            }
            return grid;
        }
        if trailing_more.is_none() {
            return grid;
        }
    }

    inspect_join_aggregate(
        "[",
        "]",
        items,
        depth,
        if force_multiline { 0 } else { break_length },
    )
}

fn inspect_force_nested_group_multiline(
    open: &str,
    close: &str,
    entries: &[String],
    depth: i32,
    max_depth: i32,
    break_length: usize,
) -> bool {
    if max_depth != i32::MAX {
        return false;
    }
    let has_nested_inline_group = entries.iter().any(|e| e.contains("{ "));
    if break_length == 0 {
        return true;
    }
    let inline = format!("{open} {} {close}", entries.join(", "));
    let current_indent = "  ".repeat(depth.max(0) as usize).chars().count();
    if depth <= 1 && entries.len() == 1 {
        return has_nested_inline_group && inline.chars().count() + current_indent > break_length;
    }
    if has_nested_inline_group && inline.chars().count() + current_indent > break_length {
        return true;
    }
    depth >= 3 && break_length <= 20
}

fn util_ctor_name(rt: &Runtime, id: rusty_js_runtime::value::ObjectRef) -> String {
    if let Value::Object(c) = rt.object_get(id, "constructor") {
        if let Value::String(n) = rt.object_get(c, "name") {
            return n.as_str().to_string();
        }
    }
    String::new()
}

fn util_is_boxed_primitive(
    rt: &mut Runtime,
    id: rusty_js_runtime::value::ObjectRef,
    want: &str,
) -> bool {
    let proto = rt.global_get("Object");
    if let Value::Object(o) = proto {
        let p = rt.object_get(o, "prototype");
        if let Value::Object(pp) = p {
            let ts = rt.object_get(pp, "toString");
            if rt.is_callable(&ts) {
                if let Ok(Value::String(tag)) = rt.call_function(ts, Value::Object(id), Vec::new())
                {
                    return tag.as_str() == format!("[object {}]", want);
                }
            }
        }
    }
    false
}

fn inspect_display_name(rt: &Runtime, id: rusty_js_runtime::value::ObjectRef) -> Option<String> {
    rt.obj(id)
        .constructed_display_name
        .as_ref()
        .filter(|name| !name.is_empty() && name.as_str() != "Object" && name.as_str() != "Array")
        .cloned()
}

fn inspect_special_display_name(
    rt: &Runtime,
    id: rusty_js_runtime::value::ObjectRef,
) -> Option<String> {
    if let InternalKind::Generator(g) = &rt.obj(id).internal_kind {
        Some(if g.is_async {
            "Object [AsyncGenerator]".to_string()
        } else {
            "Object [Generator]".to_string()
        })
    } else if rt.obj(id).has_own_str("__weakref_target") {
        Some("WeakRef".to_string())
    } else if rt.obj(id).has_own_str("__finalization_cleanup") {
        Some("FinalizationRegistry".to_string())
    } else {
        None
    }
}

fn inspect_tostringtag(rt: &Runtime, id: rusty_js_runtime::value::ObjectRef) -> Option<String> {
    let sym = match rt.global_get("Symbol") {
        Value::Object(sym_ctor) => match rt.object_get(sym_ctor, "toStringTag") {
            Value::Symbol(rc) => rc,
            _ => return None,
        },
        _ => return None,
    };
    let mut cur = Some(id);
    let mut is_own = true;
    while let Some(o) = cur {
        let obj = rt.obj(o);

        if let Some(desc) = obj
            .get_own_symbol(&sym)
            .or_else(|| obj.get_own("@@toStringTag"))
        {
            if desc.getter.is_some() {
                return None;
            }
            let Value::String(s) = &desc.value else {
                return None;
            };
            if is_own && desc.enumerable {
                return None;
            }
            return Some(s.as_str().to_string());
        }
        cur = rt.obj(o).proto;
        is_own = false;
    }
    None
}

fn inspect_object_wrapper(
    rt: &Runtime,
    id: rusty_js_runtime::value::ObjectRef,
    body: String,
) -> String {
    if let Some(special) = inspect_special_display_name(rt, id) {
        return match rt.obj(id).proto.is_none() {
            true => format!("[{}: null prototype] {}", special, body),
            false => format!("{} {}", special, body),
        };
    }
    let display = inspect_display_name(rt, id);
    let proto_none = rt.obj(id).proto.is_none();

    let tag = if proto_none {
        None
    } else {
        inspect_tostringtag(rt, id)
    };
    match (display, proto_none) {
        (Some(name), true) => format!("[{}: null prototype] {}", name, body),
        (Some(name), false) => match &tag {
            Some(t) if *t != name => format!("{} [{}] {}", name, t, body),
            _ => format!("{} {}", name, body),
        },
        (None, true) => format!("[Object: null prototype] {}", body),
        (None, false) => match &tag {
            Some(t) => format!("Object [{}] {}", t, body),
            None => body,
        },
    }
}

fn inspect_array_wrapper(
    rt: &Runtime,
    id: rusty_js_runtime::value::ObjectRef,
    len: usize,
    body: String,
) -> String {
    match inspect_display_name(rt, id) {
        Some(name) => format!("{}({}) {}", name, len, body),
        None => body,
    }
}

fn inspect_ref_prefix(rendered: String, ref_index: usize) -> String {
    let prefix = format!("<ref *{}> ", ref_index);
    if rendered.starts_with(&prefix) || !rendered.contains(&format!("[Circular *{}]", ref_index)) {
        rendered
    } else {
        format!("{}{}", prefix, rendered)
    }
}

fn inspect_ref_index(seen: &[ObjectRef], id: ObjectRef) -> Option<usize> {
    seen.iter()
        .position(|seen_id| *seen_id == id)
        .map(|i| i + 1)
}

fn inspect_error_bracket_fallback(rt: &Runtime, id: ObjectRef) -> String {
    let header = inspect_error_header(rt, id);
    format!("[{header}]")
}

fn inspect_regexp_object(
    rt: &Runtime,
    id: ObjectRef,
    src: &str,
    flags: &str,
    depth: i32,
    max_depth: i32,
    break_length: usize,
    sorted: bool,
    max_string_length: Option<usize>,
    max_array_length: Option<usize>,
    colors: bool,
    show_hidden: bool,
    seen: &mut Vec<ObjectRef>,
) -> String {
    let base = if colors {
        inspect_colorize_regexp(src, flags)
    } else {
        format!("/{}/{}", src, flags)
    };
    let mut entries = Vec::new();
    if show_hidden {
        let last_index = rt.object_get(id, "lastIndex");
        entries.push(format!(
            "[lastIndex]: {}",
            inspect_value_inner(
                rt,
                &last_index,
                depth + 1,
                max_depth,
                break_length,
                sorted,
                max_string_length,
                max_array_length,
                colors,
                show_hidden,
                seen,
            )
        ));
    }
    let mut keys = rt.ordinary_own_enumerable_string_keys(id);
    if sorted {
        keys.sort();
    }
    for key in keys {
        if key == "lastIndex" {
            continue;
        }
        let kd = if is_ident_key(&key) {
            key.clone()
        } else {
            inspect_str(&key)
        };
        let rendered = rt
            .obj(id)
            .get_own(&key)
            .and_then(|d| inspect_accessor_placeholder(d.getter.is_some(), d.setter.is_some()))
            .map(str::to_string)
            .unwrap_or_else(|| {
                let value = rt.object_get(id, &key);
                inspect_value_inner(
                    rt,
                    &value,
                    depth + 1,
                    max_depth,
                    break_length,
                    sorted,
                    max_string_length,
                    max_array_length,
                    colors,
                    show_hidden,
                    seen,
                )
            });
        entries.push(format!("{}: {}", kd, rendered));
    }
    if show_hidden {
        for key in inspect_hidden_own_string_keys(rt, id, 0, sorted) {
            if key == "lastIndex" {
                continue;
            }
            entries.push(inspect_property_entry(
                rt,
                id,
                &key,
                true,
                depth,
                max_depth,
                break_length,
                sorted,
                max_string_length,
                max_array_length,
                colors,
                show_hidden,
                seen,
            ));
        }
    }
    if entries.is_empty() {
        base
    } else {
        format!(
            "{} {}",
            base,
            inspect_join_aggregate("{", "}", &entries, depth, break_length)
        )
    }
}

fn inspect_regexp_object_selected_getters(
    rt: &mut Runtime,
    id: ObjectRef,
    src: &str,
    flags: &str,
    depth: i32,
    max_depth: i32,
    break_length: usize,
    sorted: bool,
    max_string_length: Option<usize>,
    max_array_length: Option<usize>,
    colors: bool,
    show_hidden: bool,
    getters_mode: InspectGettersMode,
    seen: &mut Vec<ObjectRef>,
) -> String {
    let base = if colors {
        inspect_colorize_regexp(src, flags)
    } else {
        format!("/{}/{}", src, flags)
    };
    let mut entries = Vec::new();
    if show_hidden {
        let last_index = rt.object_get(id, "lastIndex");
        entries.push(format!(
            "[lastIndex]: {}",
            inspect_value_inner(
                rt,
                &last_index,
                depth + 1,
                max_depth,
                break_length,
                sorted,
                max_string_length,
                max_array_length,
                colors,
                show_hidden,
                seen,
            )
        ));
    }
    for key in inspect_ordinary_suffix_getter_keys(rt, id, show_hidden, sorted, &["lastIndex"]) {
        entries.push(inspect_selected_getter_property_entry(
            rt,
            id,
            key,
            depth,
            max_depth,
            break_length,
            sorted,
            max_string_length,
            max_array_length,
            colors,
            show_hidden,
            getters_mode,
            seen,
        ));
    }
    if entries.is_empty() {
        base
    } else {
        format!(
            "{} {}",
            base,
            inspect_join_aggregate("{", "}", &entries, depth, break_length)
        )
    }
}

fn inspect_error_header(rt: &Runtime, id: ObjectRef) -> String {
    let name = match rt.object_get(id, "name") {
        Value::String(s) if !s.is_empty() => s.as_str().to_string(),
        _ => "Error".to_string(),
    };
    let message = match rt.object_get(id, "message") {
        Value::String(s) => s.as_str().to_string(),
        _ => String::new(),
    };
    if message.is_empty() {
        name
    } else {
        format!("{name}: {message}")
    }
}

fn inspect_error_refresh_stack_header(rt: &Runtime, id: ObjectRef, stack: &str) -> String {
    let current = inspect_error_header(rt, id);
    match stack.split_once('\n') {
        Some((first, rest)) if first != current => format!("{current}\n{rest}"),
        _ => stack.to_string(),
    }
}

fn inspect_colorize_error_stack(stack: String) -> String {
    stack
        .split('\n')
        .map(|line| {
            if line.contains("node:internal/") {
                inspect_colorize_special(line.to_string(), "90")
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn inspect_error_projection(rt: &Runtime, id: ObjectRef, colors: bool) -> Option<String> {
    let is_native_error = matches!(rt.obj(id).internal_kind, InternalKind::Error);
    let has_hidden_error_name_message = rt.obj(id).get_own("name").is_some_and(|d| !d.enumerable)
        && rt.obj(id).get_own("message").is_some_and(|d| !d.enumerable);
    if let Some(desc) = rt.obj(id).get_own("stack") {
        if desc.getter.is_none() && desc.setter.is_none() {
            match &desc.value {
                Value::String(stack) => {
                    let stack = stack.as_str();
                    if stack.contains('\n') {
                        let stack = stack.to_string();
                        return Some(if colors {
                            inspect_colorize_error_stack(stack)
                        } else {
                            stack
                        });
                    }
                    if !stack.is_empty() {
                        return Some(format!("[{stack}]"));
                    }
                    return Some(inspect_error_bracket_fallback(rt, id));
                }
                Value::Undefined | Value::Null => {
                    return Some(inspect_error_bracket_fallback(rt, id))
                }
                value => {
                    let rendered = inspect_value(rt, value, 1, 2, DEFAULT_BREAK_LENGTH);
                    return Some(format!(
                        "[{}\n    {}]",
                        inspect_error_header(rt, id),
                        rendered
                    ));
                }
            }
        }
    }
    if is_native_error {
        if let Some(desc) = rt.obj(id).get_own("__error_stack__") {
            match &desc.value {
                Value::String(stack) => {
                    if !stack.as_str().is_empty() {
                        let stack = inspect_error_refresh_stack_header(rt, id, stack.as_str());
                        if !stack.contains('\n') {
                            return Some(format!("[{stack}]"));
                        }
                        return Some(if colors {
                            inspect_colorize_error_stack(stack)
                        } else {
                            stack
                        });
                    }
                }
                Value::Undefined | Value::Null => {
                    return Some(inspect_error_bracket_fallback(rt, id))
                }
                value => {
                    let rendered = inspect_value(rt, value, 1, 2, DEFAULT_BREAK_LENGTH);
                    return Some(format!(
                        "[{}\n    {}]",
                        inspect_error_header(rt, id),
                        rendered
                    ));
                }
            }
        }
    }
    if is_native_error {
        if let Value::String(stack) = rt.object_get(id, "stack") {
            let assertion_proto_fallback = stack.as_str() == "AssertionError"
                && matches!(
                    rt.object_get(id, "name"),
                    Value::String(name) if name.as_str() == "AssertionError"
                );
            if !stack.as_str().is_empty() && !assertion_proto_fallback {
                let stack = stack.as_str().to_string();
                return Some(if colors {
                    inspect_colorize_error_stack(stack)
                } else {
                    stack
                });
            }
        }
    }
    if !is_native_error && !has_hidden_error_name_message {
        return None;
    }
    Some(inspect_error_bracket_fallback(rt, id))
}

fn inspect_error_enumerable_value(
    rt: &Runtime,
    value: &Value,
    depth: i32,
    max_depth: i32,
    break_length: usize,
    sorted: bool,
    max_string_length: Option<usize>,
    max_array_length: Option<usize>,
    colors: bool,
    show_hidden: bool,
    seen: &mut Vec<ObjectRef>,
) -> String {
    if let Value::String(s) = value {
        let raw = s.as_str();
        if raw.contains('\n') {
            let preview: String = raw.split_inclusive('\n').take(10).collect();
            if preview.chars().count() <= 20 {
                return inspect_str(&format!("{preview}..."));
            }
            let mut parts = preview.split_inclusive('\n').map(inspect_str);
            if let Some(first) = parts.next() {
                let indent = "  ".repeat((depth + 2).max(0) as usize);
                let mut out = first;
                for part in parts {
                    out.push_str(" +\n");
                    out.push_str(&indent);
                    out.push_str(&part);
                }
                out.push_str(" +\n");
                out.push_str(&indent);
                out.push_str("'...'");
                return out;
            }
        }
        let char_count = raw.chars().count();
        if char_count > 512 {
            let display_len = char_count.saturating_sub(512).min(9488);
            let prefix: String = raw.chars().take(display_len).collect();
            return format!("'{}...'", prefix.replace('\\', "\\\\").replace('\'', "\\'"));
        }
    }
    if let Value::Object(id) = value {
        if matches!(rt.obj(*id).internal_kind, InternalKind::Array) {
            if matches!(rt.object_get(*id, "length"), Value::Number(n) if n >= 100.0) {
                return "[Array]".into();
            }
        }
    }
    inspect_value_inner(
        rt,
        value,
        depth + 1,
        max_depth,
        break_length,
        sorted,
        max_string_length,
        max_array_length,
        colors,
        show_hidden,
        seen,
    )
}

fn inspect_error_hidden_stack_header_keys(hidden_keys: &[String]) -> Vec<String> {
    ["stack", "message", "name"]
        .iter()
        .filter(|key| hidden_keys.iter().any(|hidden| hidden == **key))
        .map(|key| (*key).to_string())
        .collect()
}

fn inspect_error_suffix_selected_comparator(
    rt: &mut Runtime,
    id: ObjectRef,
    depth: i32,
    max_depth: i32,
    break_length: usize,
    comparator: &Value,
    max_string_length: Option<usize>,
    max_array_length: Option<usize>,
    colors: bool,
    show_hidden: bool,
    base_multiline: bool,
    seen: &mut Vec<ObjectRef>,
) -> Result<String, RuntimeError> {
    let mut entries: Vec<String> = Vec::new();
    for key in rt.ordinary_own_enumerable_string_keys(id) {
        if matches!(
            key.as_str(),
            "name" | "message" | "stack" | "__error_stack__"
        ) {
            continue;
        }
        let kd = if is_ident_key(&key) {
            key.clone()
        } else {
            inspect_str(&key)
        };
        let rendered = rt
            .obj(id)
            .get_own(&key)
            .and_then(|d| inspect_accessor_placeholder(d.getter.is_some(), d.setter.is_some()))
            .map(str::to_string)
            .map(Ok)
            .unwrap_or_else(|| {
                let val = rt.object_get(id, &key);
                inspect_value_with_selected_sorted_comparator(
                    rt,
                    &val,
                    depth + 1,
                    max_depth,
                    break_length,
                    comparator,
                    max_string_length,
                    max_array_length,
                    colors,
                    show_hidden,
                    seen,
                )
            })?;
        entries.push(format!("{}: {}", kd, rendered));
    }
    for sym in inspect_own_symbol_keys(rt, id, true) {
        entries.push(inspect_symbol_property_entry(
            rt,
            id,
            &sym,
            false,
            depth,
            max_depth,
            break_length,
            false,
            max_string_length,
            max_array_length,
            colors,
            show_hidden,
            seen,
        ));
    }
    let has_hidden_cause = rt
        .obj(id)
        .get_own("cause")
        .is_some_and(|desc| !desc.enumerable);
    if has_hidden_cause {
        entries.push(inspect_hidden_error_cause_property_entry(
            rt,
            id,
            depth,
            max_depth,
            break_length,
            false,
            max_string_length,
            max_array_length,
            colors,
            show_hidden,
            seen,
        ));
    }
    let hidden_keys = if show_hidden {
        Some(inspect_hidden_own_string_keys(rt, id, 0, false))
    } else {
        None
    };
    if let Some(hidden_keys) = hidden_keys.as_ref() {
        for key in inspect_error_hidden_stack_header_keys(hidden_keys) {
            entries.push(inspect_property_entry(
                rt,
                id,
                &key,
                true,
                depth,
                max_depth,
                break_length,
                false,
                max_string_length,
                max_array_length,
                colors,
                show_hidden,
                seen,
            ));
        }
    }
    if rt
        .obj(id)
        .get_own("errors")
        .is_some_and(|desc| !desc.enumerable)
    {
        entries.push(inspect_aggregate_errors_property_entry(
            rt,
            id,
            true,
            depth,
            max_depth,
            break_length,
            false,
            max_string_length,
            max_array_length,
            colors,
            show_hidden,
            seen,
        ));
    }
    if let Some(hidden_keys) = hidden_keys.as_ref() {
        for key in hidden_keys.iter().filter(|key| {
            !matches!(key.as_str(), "stack" | "message" | "name")
                && !matches!(key.as_str(), "cause" | "errors" | "__error_stack__")
        }) {
            entries.push(inspect_property_entry(
                rt,
                id,
                key,
                true,
                depth,
                max_depth,
                break_length,
                false,
                max_string_length,
                max_array_length,
                colors,
                show_hidden,
                seen,
            ));
        }
    }
    if show_hidden {
        for sym in inspect_own_symbol_keys(rt, id, false) {
            entries.push(inspect_symbol_property_entry(
                rt,
                id,
                &sym,
                true,
                depth,
                max_depth,
                break_length,
                false,
                max_string_length,
                max_array_length,
                colors,
                show_hidden,
                seen,
            ));
        }
    }
    if entries.is_empty() {
        return Ok(String::new());
    }
    sort_rendered_entries_with_comparator(rt, &mut entries, comparator.clone())?;

    let body = if show_hidden || base_multiline {
        inspect_multiline_block("{", "}", &entries, depth)
    } else {
        inspect_join_aggregate("{", "}", &entries, depth, break_length)
    };
    Ok(format!(" {}", body))
}

fn inspect_error_suffix(
    rt: &Runtime,
    id: ObjectRef,
    depth: i32,
    max_depth: i32,
    break_length: usize,
    sorted: bool,
    max_string_length: Option<usize>,
    max_array_length: Option<usize>,
    colors: bool,
    show_hidden: bool,
    base_multiline: bool,
    seen: &mut Vec<ObjectRef>,
) -> String {
    if show_hidden && sorted {
        let mut entries: Vec<String> = Vec::new();
        for sym in inspect_own_symbol_keys(rt, id, true) {
            entries.push(inspect_symbol_property_entry(
                rt,
                id,
                &sym,
                false,
                depth,
                max_depth,
                break_length,
                sorted,
                max_string_length,
                max_array_length,
                colors,
                show_hidden,
                seen,
            ));
        }
        for sym in inspect_own_symbol_keys(rt, id, false) {
            entries.push(inspect_symbol_property_entry(
                rt,
                id,
                &sym,
                true,
                depth,
                max_depth,
                break_length,
                sorted,
                max_string_length,
                max_array_length,
                colors,
                show_hidden,
                seen,
            ));
        }

        let hidden_keys = inspect_hidden_own_string_keys(rt, id, 0, sorted);
        let has_hidden_cause = rt
            .obj(id)
            .get_own("cause")
            .is_some_and(|desc| !desc.enumerable);
        let hidden_array_errors = rt
            .obj(id)
            .get_own("errors")
            .is_some_and(|desc| !desc.enumerable)
            && matches!(rt.object_get(id, "errors"), Value::Object(errors) if matches!(rt.obj(errors).internal_kind, InternalKind::Array));
        let mut hidden_order: Vec<String> = hidden_keys
            .iter()
            .filter(|key| {
                !matches!(key.as_str(), "__error_stack__")
                    && (matches!(key.as_str(), "stack" | "message" | "name")
                        || (key.as_str() == "cause" && has_hidden_cause)
                        || (key.as_str() == "errors" && hidden_array_errors)
                        || !matches!(key.as_str(), "cause" | "errors"))
            })
            .cloned()
            .collect();
        hidden_order.sort_by_key(|key| format!("[{key}]"));
        for key in hidden_order {
            if key == "cause" {
                entries.push(inspect_hidden_error_cause_property_entry(
                    rt,
                    id,
                    depth,
                    max_depth,
                    break_length,
                    sorted,
                    max_string_length,
                    max_array_length,
                    colors,
                    show_hidden,
                    seen,
                ));
            } else if key == "errors" {
                entries.push(inspect_aggregate_errors_property_entry(
                    rt,
                    id,
                    true,
                    depth,
                    max_depth,
                    break_length,
                    sorted,
                    max_string_length,
                    max_array_length,
                    colors,
                    show_hidden,
                    seen,
                ));
            } else {
                entries.push(inspect_property_entry(
                    rt,
                    id,
                    &key,
                    true,
                    depth,
                    max_depth,
                    break_length,
                    sorted,
                    max_string_length,
                    max_array_length,
                    colors,
                    show_hidden,
                    seen,
                ));
            }
        }

        let mut keys = rt.ordinary_own_enumerable_string_keys(id);
        keys.sort();
        for key in keys {
            if matches!(
                key.as_str(),
                "name" | "message" | "stack" | "__error_stack__"
            ) {
                continue;
            }
            let kd = if is_ident_key(&key) {
                key.clone()
            } else {
                inspect_str(&key)
            };
            let rendered = rt
                .obj(id)
                .get_own(&key)
                .and_then(|d| inspect_accessor_placeholder(d.getter.is_some(), d.setter.is_some()))
                .map(str::to_string)
                .unwrap_or_else(|| {
                    let value = rt.object_get(id, &key);
                    inspect_error_enumerable_value(
                        rt,
                        &value,
                        depth,
                        max_depth,
                        break_length,
                        sorted,
                        max_string_length,
                        max_array_length,
                        colors,
                        show_hidden,
                        seen,
                    )
                });
            entries.push(format!("{}: {}", kd, rendered));
        }
        if entries.is_empty() {
            return String::new();
        }
        return format!(" {}", inspect_multiline_block("{", "}", &entries, depth));
    }

    let mut entries: Vec<String> = Vec::new();
    let mut keys = rt.ordinary_own_enumerable_string_keys(id);
    if sorted {
        keys.sort();
    }
    let hidden_keys = if show_hidden {
        Some(inspect_hidden_own_string_keys(rt, id, 0, sorted))
    } else {
        None
    };
    if let Some(hidden_keys) = hidden_keys.as_ref() {
        for key in inspect_error_hidden_stack_header_keys(hidden_keys) {
            entries.push(inspect_property_entry(
                rt,
                id,
                &key,
                true,
                depth,
                max_depth,
                break_length,
                sorted,
                max_string_length,
                max_array_length,
                colors,
                show_hidden,
                seen,
            ));
        }
    }
    let has_hidden_cause = rt
        .obj(id)
        .get_own("cause")
        .is_some_and(|desc| !desc.enumerable);
    if show_hidden && has_hidden_cause {
        entries.push(inspect_hidden_error_cause_property_entry(
            rt,
            id,
            depth,
            max_depth,
            break_length,
            sorted,
            max_string_length,
            max_array_length,
            colors,
            show_hidden,
            seen,
        ));
    }
    let hidden_array_errors = rt
        .obj(id)
        .get_own("errors")
        .is_some_and(|desc| !desc.enumerable)
        && matches!(rt.object_get(id, "errors"), Value::Object(errors) if matches!(rt.obj(errors).internal_kind, InternalKind::Array));
    if show_hidden && hidden_array_errors {
        entries.push(inspect_aggregate_errors_property_entry(
            rt,
            id,
            true,
            depth,
            max_depth,
            break_length,
            sorted,
            max_string_length,
            max_array_length,
            colors,
            show_hidden,
            seen,
        ));
    }
    for key in keys {
        if matches!(
            key.as_str(),
            "name" | "message" | "stack" | "__error_stack__"
        ) {
            continue;
        }
        let kd = if is_ident_key(&key) {
            key.clone()
        } else {
            inspect_str(&key)
        };
        let rendered = if let Some(placeholder) = rt
            .obj(id)
            .get_own(&key)
            .and_then(|d| inspect_accessor_placeholder(d.getter.is_some(), d.setter.is_some()))
        {
            placeholder.to_string()
        } else {
            let value = rt.object_get(id, &key);
            let rendered = inspect_error_enumerable_value(
                rt,
                &value,
                depth,
                max_depth,
                break_length,
                sorted,
                max_string_length,
                max_array_length,
                colors,
                show_hidden,
                seen,
            );
            if matches!(value, Value::Object(value_id) if matches!(rt.obj(value_id).internal_kind, InternalKind::Error))
            {
                inspect_indent_stack_frame_lines(
                    rendered,
                    &"  ".repeat((depth + 1).max(0) as usize),
                )
            } else {
                rendered
            }
        };
        entries.push(format!("{}: {}", kd, rendered));
    }
    if let Some(hidden_keys) = hidden_keys.as_ref() {
        for key in hidden_keys.iter().filter(|key| {
            !matches!(key.as_str(), "stack" | "message" | "name")
                && !matches!(key.as_str(), "cause" | "errors" | "__error_stack__")
        }) {
            entries.push(inspect_property_entry(
                rt,
                id,
                key,
                true,
                depth,
                max_depth,
                break_length,
                sorted,
                max_string_length,
                max_array_length,
                colors,
                show_hidden,
                seen,
            ));
        }
    }
    for sym in inspect_own_symbol_keys(rt, id, true) {
        entries.push(inspect_symbol_property_entry(
            rt,
            id,
            &sym,
            false,
            depth,
            max_depth,
            break_length,
            sorted,
            max_string_length,
            max_array_length,
            colors,
            show_hidden,
            seen,
        ));
    }
    if !show_hidden && has_hidden_cause {
        entries.push(inspect_hidden_error_cause_property_entry(
            rt,
            id,
            depth,
            max_depth,
            break_length,
            sorted,
            max_string_length,
            max_array_length,
            colors,
            show_hidden,
            seen,
        ));
    }
    if !show_hidden && hidden_array_errors {
        entries.push(inspect_aggregate_errors_property_entry(
            rt,
            id,
            true,
            depth,
            max_depth,
            break_length,
            sorted,
            max_string_length,
            max_array_length,
            colors,
            show_hidden,
            seen,
        ));
    }
    if show_hidden {
        for sym in inspect_own_symbol_keys(rt, id, false) {
            entries.push(inspect_symbol_property_entry(
                rt,
                id,
                &sym,
                true,
                depth,
                max_depth,
                break_length,
                sorted,
                max_string_length,
                max_array_length,
                colors,
                show_hidden,
                seen,
            ));
        }
    }
    if entries.is_empty() {
        String::new()
    } else if show_hidden || base_multiline {

        format!(" {}", inspect_multiline_block("{", "}", &entries, depth))
    } else {
        format!(
            " {}",
            inspect_join_aggregate("{", "}", &entries, depth, break_length)
        )
    }
}

fn inspect_error_suffix_selected_getters(
    rt: &mut Runtime,
    id: ObjectRef,
    depth: i32,
    max_depth: i32,
    break_length: usize,
    sorted: bool,
    max_string_length: Option<usize>,
    max_array_length: Option<usize>,
    colors: bool,
    show_hidden: bool,
    getters_mode: Option<InspectGettersMode>,
    base_multiline: bool,
    seen: &mut Vec<ObjectRef>,
) -> String {
    let mut entries: Vec<String> = Vec::new();
    let mut keys = rt.ordinary_own_enumerable_string_keys(id);
    if sorted {
        keys.sort();
    }
    for key in keys {
        if matches!(
            key.as_str(),
            "name" | "message" | "stack" | "__error_stack__"
        ) {
            continue;
        }
        if let Some(mode) = getters_mode {
            entries.push(inspect_selected_getter_property_entry(
                rt,
                id,
                InspectGetterKey::String(key, false),
                depth,
                max_depth,
                break_length,
                sorted,
                max_string_length,
                max_array_length,
                colors,
                show_hidden,
                mode,
                seen,
            ));
        } else {
            let kd = if is_ident_key(&key) {
                key.clone()
            } else {
                inspect_str(&key)
            };
            let value = rt.object_get(id, &key);
            entries.push(format!(
                "{}: {}",
                kd,
                inspect_error_enumerable_value(
                    rt,
                    &value,
                    depth,
                    max_depth,
                    break_length,
                    sorted,
                    max_string_length,
                    max_array_length,
                    colors,
                    show_hidden,
                    seen,
                )
            ));
        }
    }
    for sym in inspect_own_symbol_keys(rt, id, true) {
        if let Some(mode) = getters_mode {
            entries.push(inspect_selected_getter_property_entry(
                rt,
                id,
                InspectGetterKey::Symbol(sym, false),
                depth,
                max_depth,
                break_length,
                sorted,
                max_string_length,
                max_array_length,
                colors,
                show_hidden,
                mode,
                seen,
            ));
        } else {
            entries.push(inspect_symbol_property_entry(
                rt,
                id,
                &sym,
                false,
                depth,
                max_depth,
                break_length,
                sorted,
                max_string_length,
                max_array_length,
                colors,
                show_hidden,
                seen,
            ));
        }
    }
    let hidden_keys = if show_hidden {
        Some(inspect_hidden_own_string_keys(rt, id, 0, sorted))
    } else {
        None
    };
    if let Some(hidden_keys) = hidden_keys.as_ref() {
        for key in inspect_error_hidden_stack_header_keys(hidden_keys) {
            if let Some(mode) = getters_mode {
                entries.push(inspect_selected_getter_property_entry(
                    rt,
                    id,
                    InspectGetterKey::String(key, true),
                    depth,
                    max_depth,
                    break_length,
                    sorted,
                    max_string_length,
                    max_array_length,
                    colors,
                    show_hidden,
                    mode,
                    seen,
                ));
            } else {
                entries.push(inspect_property_entry(
                    rt,
                    id,
                    &key,
                    true,
                    depth,
                    max_depth,
                    break_length,
                    sorted,
                    max_string_length,
                    max_array_length,
                    colors,
                    show_hidden,
                    seen,
                ));
            }
        }
    }
    if rt
        .obj(id)
        .get_own("cause")
        .is_some_and(|desc| !desc.enumerable)
    {
        entries.push(inspect_hidden_error_cause_property_entry(
            rt,
            id,
            depth,
            max_depth,
            break_length,
            sorted,
            max_string_length,
            max_array_length,
            colors,
            show_hidden,
            seen,
        ));
    }
    if rt
        .obj(id)
        .get_own("errors")
        .is_some_and(|desc| !desc.enumerable)
    {
        entries.push(inspect_aggregate_errors_property_entry(
            rt,
            id,
            true,
            depth,
            max_depth,
            break_length,
            sorted,
            max_string_length,
            max_array_length,
            colors,
            show_hidden,
            seen,
        ));
    }
    if let Some(hidden_keys) = hidden_keys.as_ref() {
        for key in hidden_keys.iter().filter(|key| {
            !matches!(key.as_str(), "stack" | "message" | "name")
                && !matches!(key.as_str(), "cause" | "errors" | "__error_stack__")
        }) {
            if let Some(mode) = getters_mode {
                entries.push(inspect_selected_getter_property_entry(
                    rt,
                    id,
                    InspectGetterKey::String(key.clone(), true),
                    depth,
                    max_depth,
                    break_length,
                    sorted,
                    max_string_length,
                    max_array_length,
                    colors,
                    show_hidden,
                    mode,
                    seen,
                ));
            } else {
                entries.push(inspect_property_entry(
                    rt,
                    id,
                    key,
                    true,
                    depth,
                    max_depth,
                    break_length,
                    sorted,
                    max_string_length,
                    max_array_length,
                    colors,
                    show_hidden,
                    seen,
                ));
            }
        }
    }
    if show_hidden {
        for sym in inspect_own_symbol_keys(rt, id, false) {
            if let Some(mode) = getters_mode {
                entries.push(inspect_selected_getter_property_entry(
                    rt,
                    id,
                    InspectGetterKey::Symbol(sym, true),
                    depth,
                    max_depth,
                    break_length,
                    sorted,
                    max_string_length,
                    max_array_length,
                    colors,
                    show_hidden,
                    mode,
                    seen,
                ));
            } else {
                entries.push(inspect_symbol_property_entry(
                    rt,
                    id,
                    &sym,
                    true,
                    depth,
                    max_depth,
                    break_length,
                    sorted,
                    max_string_length,
                    max_array_length,
                    colors,
                    show_hidden,
                    seen,
                ));
            }
        }
    }
    let hidden_array_errors = rt
        .obj(id)
        .get_own("errors")
        .is_some_and(|desc| !desc.enumerable)
        && matches!(rt.object_get(id, "errors"), Value::Object(errors) if matches!(rt.obj(errors).internal_kind, InternalKind::Array));
    if show_hidden && hidden_array_errors && !sorted {
        let mut hidden_entries = Vec::new();
        let mut visible_entries = Vec::new();
        for entry in entries {
            if entry.starts_with('[') {
                hidden_entries.push(entry);
            } else {
                visible_entries.push(entry);
            }
        }
        hidden_entries.extend(visible_entries);
        entries = hidden_entries;
    }
    if sorted {
        entries.sort();
    }
    if entries.is_empty() {
        String::new()
    } else if base_multiline {
        format!(" {}", inspect_multiline_block("{", "}", &entries, depth))
    } else {
        format!(
            " {}",
            inspect_join_aggregate("{", "}", &entries, depth, break_length)
        )
    }
}

fn indent_continuation_lines(s: &str, spaces: usize) -> String {
    if spaces == 0 || !s.contains('\n') {
        return s.to_string();
    }
    let pad = " ".repeat(spaces);
    let mut out = String::new();
    for (i, line) in s.split('\n').enumerate() {
        if i > 0 {
            out.push('\n');
            out.push_str(&pad);
        }
        out.push_str(line);
    }
    out
}

fn inspect_depth_limit_has_content(rt: &Runtime, id: ObjectRef, show_hidden: bool) -> bool {
    if matches!(rt.obj(id).internal_kind, InternalKind::Array) {
        return matches!(rt.object_get(id, "length"), Value::Number(n) if n > 0.0);
    }
    if !rt.ordinary_own_enumerable_string_keys(id).is_empty() {
        return true;
    }

    if !inspect_own_symbol_keys(rt, id, true).is_empty() {
        return true;
    }
    show_hidden
        && (!inspect_own_symbol_keys(rt, id, false).is_empty()
            || !inspect_hidden_own_string_keys(rt, id, 0, false).is_empty())
}

fn inspect_depth_limit_label(rt: &Runtime, id: ObjectRef, is_array: bool) -> String {
    if !is_array && rt.obj(id).proto.is_none() {
        return match inspect_display_name(rt, id) {
            Some(name) => format!("[{name}: null prototype]"),
            None => "[Object: null prototype]".to_string(),
        };
    }
    match inspect_display_name(rt, id) {
        Some(name) => format!("[{name}]"),
        None if is_array => "[Array]".to_string(),
        None => "[Object]".to_string(),
    }
}

fn inspect_multiline_block(open: &str, close: &str, entries: &[String], depth: i32) -> String {
    if entries.is_empty() {
        return format!("{open}{close}");
    }
    let inner_indent = "  ".repeat((depth + 1).max(0) as usize);
    let outer_indent = "  ".repeat(depth.max(0) as usize);
    let sep = format!(",\n{inner_indent}");
    format!(
        "{open}\n{inner_indent}{}\n{outer_indent}{close}",
        entries.join(&sep)
    )
}

fn inspect_colorized_value(rt: &Runtime, v: &Value) -> String {
    let rendered = inspect_value(rt, v, 0, 2, DEFAULT_BREAK_LENGTH);
    match v {
        Value::Undefined => format!("\x1b[90m{rendered}\x1b[39m"),
        Value::Null => format!("\x1b[1m{rendered}\x1b[22m"),
        Value::Boolean(_) | Value::Number(_) | Value::BigInt(_) => {
            format!("\x1b[33m{rendered}\x1b[39m")
        }
        Value::Symbol(_) => format!("\x1b[32m{rendered}\x1b[39m"),
        _ => rendered,
    }
}

fn util_make_error(rt: &mut Runtime, name: &str, message: &str) -> Value {
    let obj = rt.alloc_object(Object::new_ordinary());
    rt.object_set(
        obj,
        "name".into(),
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(name))),
    );
    rt.object_set(
        obj,
        "message".into(),
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(message))),
    );
    Value::Object(obj)
}

fn ms_to_iso(ms: f64) -> String {
    if !ms.is_finite() {
        return "Invalid Date".to_string();
    }
    let ms_i = ms as i64;
    let days = ms_i.div_euclid(86_400_000);
    let rem = ms_i.rem_euclid(86_400_000);
    let (h, mn, s, milli) = (
        rem / 3_600_000,
        (rem / 60_000) % 60,
        (rem / 1000) % 60,
        rem % 1000,
    );
    let z = days + 719_468;
    let era = (if z >= 0 { z } else { z - 146_096 }) / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let mth = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if mth <= 2 { y + 1 } else { y };
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z",
        y, mth, d, h, mn, s, milli
    )
}

fn inspect_value(
    rt: &Runtime,
    v: &Value,
    depth: i32,
    max_depth: i32,
    break_length: usize,
) -> String {
    inspect_value_with_sorted(rt, v, depth, max_depth, break_length, false)
}

pub(crate) fn inspect_default(rt: &Runtime, v: &Value) -> String {
    inspect_value(rt, v, 0, 2, DEFAULT_BREAK_LENGTH)
}

struct ShowProxyGuard(bool);
impl ShowProxyGuard {
    fn set(v: bool) -> Self {
        ShowProxyGuard(INSPECT_SHOW_PROXY.with(|c| c.replace(v)))
    }
}
impl Drop for ShowProxyGuard {
    fn drop(&mut self) {
        INSPECT_SHOW_PROXY.with(|c| c.set(self.0));
    }
}

fn inspect_value_with_sorted(
    rt: &Runtime,
    v: &Value,
    depth: i32,
    max_depth: i32,
    break_length: usize,
    sorted: bool,
) -> String {
    inspect_value_with_options(
        rt,
        v,
        depth,
        max_depth,
        break_length,
        sorted,
        None,
        None,
        false,
        false,
    )
}

fn inspect_show_hidden_allows_key(key: &str) -> bool {
    !key.starts_with("__")
}

fn inspect_hidden_key(key: &str) -> String {
    if is_ident_key(key) {
        format!("[{}]", key)
    } else {
        format!("[{}]", inspect_str(key))
    }
}

fn inspect_hidden_own_string_keys(
    rt: &Runtime,
    id: ObjectRef,
    array_len: usize,
    sorted: bool,
) -> Vec<String> {
    let mut keys: Vec<String> = rt
        .obj(id)
        .properties
        .iter()
        .filter_map(|(key, desc)| match key {
            PropertyKey::String(name)
                if !desc.enumerable
                    && name != "length"
                    && !name.parse::<usize>().is_ok_and(|idx| idx < array_len)
                    && inspect_show_hidden_allows_key(name) =>
            {
                Some(name.clone())
            }
            _ => None,
        })
        .collect();
    if sorted {
        keys.sort();
    }
    keys
}

fn inspect_symbol_label(sym: &Rc<String>, hidden: bool) -> String {
    let rendered = abstract_ops::to_string(&Value::Symbol(sym.clone()))
        .as_str()
        .to_string();
    if hidden {
        format!("[{}]", rendered)
    } else {
        rendered
    }
}

fn inspect_own_symbol_keys(rt: &Runtime, id: ObjectRef, enumerable: bool) -> Vec<Rc<String>> {
    rt.obj(id)
        .properties
        .iter()
        .filter_map(|(key, desc)| match key {
            PropertyKey::Symbol(sym) if desc.enumerable == enumerable => Some(sym.clone()),
            _ => None,
        })
        .collect()
}

fn inspect_symbol_property_entry(
    rt: &Runtime,
    id: ObjectRef,
    sym: &Rc<String>,
    hidden: bool,
    depth: i32,
    max_depth: i32,
    break_length: usize,
    sorted: bool,
    max_string_length: Option<usize>,
    max_array_length: Option<usize>,
    colors: bool,
    show_hidden: bool,
    seen: &mut Vec<ObjectRef>,
) -> String {
    let rendered = rt
        .obj(id)
        .get_own_symbol(sym)
        .and_then(|d| inspect_accessor_placeholder(d.getter.is_some(), d.setter.is_some()))
        .map(str::to_string)
        .unwrap_or_else(|| {
            let val = rt
                .obj(id)
                .get_own_symbol(sym)
                .map(|d| d.value.clone())
                .unwrap_or(Value::Undefined);
            inspect_value_inner(
                rt,
                &val,
                depth + 1,
                max_depth,
                break_length,
                sorted,
                max_string_length,
                max_array_length,
                colors,
                show_hidden,
                seen,
            )
        });
    format!("{}: {}", inspect_symbol_label(sym, hidden), rendered)
}

fn inspect_boxed_string_payload(rt: &Runtime, id: ObjectRef) -> Option<String> {
    if util_ctor_name(rt, id) != "String" {
        return None;
    }
    let len = match rt.object_get(id, "length") {
        Value::Number(n) if n.is_finite() && n >= 0.0 && n.fract() == 0.0 => n as usize,
        _ => return None,
    };
    let mut out = String::new();
    for idx in 0..len {
        match rt.object_get(id, &idx.to_string()) {
            Value::String(s) => out.push_str(s.as_str()),
            _ => return None,
        }
    }
    Some(out)
}

fn inspect_boxed_string_entry(
    rt: &Runtime,
    id: ObjectRef,
    key: &str,
    hidden: bool,
    depth: i32,
    max_depth: i32,
    break_length: usize,
    sorted: bool,
    max_string_length: Option<usize>,
    max_array_length: Option<usize>,
    colors: bool,
    show_hidden: bool,
    seen: &mut Vec<ObjectRef>,
) -> String {
    let kd = if hidden {
        inspect_hidden_key(key)
    } else if is_ident_key(key) {
        key.to_string()
    } else {
        inspect_str(key)
    };
    let rendered = rt
        .obj(id)
        .get_own(key)
        .and_then(|d| inspect_accessor_placeholder(d.getter.is_some(), d.setter.is_some()))
        .map(str::to_string)
        .unwrap_or_else(|| {
            let val = rt.object_get(id, key);
            inspect_value_inner(
                rt,
                &val,
                depth + 1,
                max_depth,
                break_length,
                sorted,
                max_string_length,
                max_array_length,
                colors,
                show_hidden,
                seen,
            )
        });
    format!("{}: {}", kd, rendered)
}

fn inspect_boxed_string_object(
    rt: &Runtime,
    id: ObjectRef,
    payload: &str,
    depth: i32,
    max_depth: i32,
    break_length: usize,
    sorted: bool,
    max_string_length: Option<usize>,
    max_array_length: Option<usize>,
    colors: bool,
    show_hidden: bool,
    seen: &mut Vec<ObjectRef>,
) -> String {
    let len = payload.chars().count();
    let mut entries = Vec::new();
    if show_hidden {
        entries.push(format!("[length]: {}", len));
    }
    let mut keys = rt.ordinary_own_enumerable_string_keys(id);
    if sorted {
        keys.sort();
    }
    for k in keys {
        if k == "length" || k.parse::<usize>().is_ok_and(|idx| idx < len) {
            continue;
        }
        entries.push(inspect_boxed_string_entry(
            rt,
            id,
            &k,
            false,
            depth,
            max_depth,
            break_length,
            sorted,
            max_string_length,
            max_array_length,
            colors,
            show_hidden,
            seen,
        ));
    }
    if show_hidden {
        let mut hidden_keys = inspect_hidden_own_string_keys(rt, id, len, sorted);
        for k in hidden_keys.drain(..) {
            entries.push(inspect_boxed_string_entry(
                rt,
                id,
                &k,
                true,
                depth,
                max_depth,
                break_length,
                sorted,
                max_string_length,
                max_array_length,
                colors,
                show_hidden,
                seen,
            ));
        }
    }
    let prefix = format!(
        "[String: {}]",
        inspect_str_with_max(payload, max_string_length)
    );
    let prefix = if colors {
        inspect_colorize_special(prefix, "32")
    } else {
        prefix
    };
    if entries.is_empty() {
        return prefix;
    }
    let force_multiline =
        inspect_force_nested_group_multiline("{", "}", &entries, depth, max_depth, break_length);
    format!(
        "{} {}",
        prefix,
        inspect_join_aggregate(
            "{",
            "}",
            &entries,
            depth,
            if force_multiline { 0 } else { break_length },
        )
    )
}

fn inspect_boxed_primitive_object(
    rt: &Runtime,
    id: ObjectRef,
    prefix: String,
    color_code: &str,
    depth: i32,
    max_depth: i32,
    break_length: usize,
    sorted: bool,
    max_string_length: Option<usize>,
    max_array_length: Option<usize>,
    colors: bool,
    show_hidden: bool,
    seen: &mut Vec<ObjectRef>,
) -> String {
    let mut keys = rt.ordinary_own_enumerable_string_keys(id);
    if sorted {
        keys.sort();
    }
    let mut entries: Vec<String> = keys
        .into_iter()
        .map(|k| {
            inspect_property_entry(
                rt,
                id,
                &k,
                false,
                depth,
                max_depth,
                break_length,
                sorted,
                max_string_length,
                max_array_length,
                colors,
                show_hidden,
                seen,
            )
        })
        .collect();
    if show_hidden {
        let mut hidden_keys = inspect_hidden_own_string_keys(rt, id, 0, sorted);
        for k in hidden_keys.drain(..) {
            entries.push(inspect_property_entry(
                rt,
                id,
                &k,
                true,
                depth,
                max_depth,
                break_length,
                sorted,
                max_string_length,
                max_array_length,
                colors,
                show_hidden,
                seen,
            ));
        }
    }
    for sym in inspect_own_symbol_keys(rt, id, true) {
        entries.push(inspect_symbol_property_entry(
            rt,
            id,
            &sym,
            false,
            depth,
            max_depth,
            break_length,
            sorted,
            max_string_length,
            max_array_length,
            colors,
            show_hidden,
            seen,
        ));
    }
    if show_hidden {
        for sym in inspect_own_symbol_keys(rt, id, false) {
            entries.push(inspect_symbol_property_entry(
                rt,
                id,
                &sym,
                true,
                depth,
                max_depth,
                break_length,
                sorted,
                max_string_length,
                max_array_length,
                colors,
                show_hidden,
                seen,
            ));
        }
    }
    let prefix = if colors {
        inspect_colorize_special(prefix, color_code)
    } else {
        prefix
    };
    if entries.is_empty() {
        return prefix;
    }
    let force_multiline =
        inspect_force_nested_group_multiline("{", "}", &entries, depth, max_depth, break_length);
    format!(
        "{} {}",
        prefix,
        inspect_join_aggregate(
            "{",
            "}",
            &entries,
            depth,
            if force_multiline { 0 } else { break_length },
        )
    )
}

fn inspect_date_object(
    rt: &Runtime,
    id: ObjectRef,
    prefix: String,
    depth: i32,
    max_depth: i32,
    break_length: usize,
    sorted: bool,
    max_string_length: Option<usize>,
    max_array_length: Option<usize>,
    colors: bool,
    show_hidden: bool,
    seen: &mut Vec<ObjectRef>,
) -> String {
    let mut entries: Vec<String> = Vec::new();
    let mut visible_keys = rt.ordinary_own_enumerable_string_keys(id);
    if sorted {
        visible_keys.sort();
        for sym in inspect_own_symbol_keys(rt, id, true) {
            entries.push(inspect_symbol_property_entry(
                rt,
                id,
                &sym,
                false,
                depth,
                max_depth,
                break_length,
                sorted,
                max_string_length,
                max_array_length,
                colors,
                show_hidden,
                seen,
            ));
        }
        if show_hidden {
            for sym in inspect_own_symbol_keys(rt, id, false) {
                entries.push(inspect_symbol_property_entry(
                    rt,
                    id,
                    &sym,
                    true,
                    depth,
                    max_depth,
                    break_length,
                    sorted,
                    max_string_length,
                    max_array_length,
                    colors,
                    show_hidden,
                    seen,
                ));
            }
            let mut hidden_keys = inspect_hidden_own_string_keys(rt, id, 0, sorted);
            for key in hidden_keys.drain(..) {
                entries.push(inspect_property_entry(
                    rt,
                    id,
                    &key,
                    true,
                    depth,
                    max_depth,
                    break_length,
                    sorted,
                    max_string_length,
                    max_array_length,
                    colors,
                    show_hidden,
                    seen,
                ));
            }
        }
        for key in visible_keys {
            entries.push(inspect_property_entry(
                rt,
                id,
                &key,
                false,
                depth,
                max_depth,
                break_length,
                sorted,
                max_string_length,
                max_array_length,
                colors,
                show_hidden,
                seen,
            ));
        }
    } else {
        for key in visible_keys {
            entries.push(inspect_property_entry(
                rt,
                id,
                &key,
                false,
                depth,
                max_depth,
                break_length,
                sorted,
                max_string_length,
                max_array_length,
                colors,
                show_hidden,
                seen,
            ));
        }
        if show_hidden {
            let mut hidden_keys = inspect_hidden_own_string_keys(rt, id, 0, sorted);
            for key in hidden_keys.drain(..) {
                entries.push(inspect_property_entry(
                    rt,
                    id,
                    &key,
                    true,
                    depth,
                    max_depth,
                    break_length,
                    sorted,
                    max_string_length,
                    max_array_length,
                    colors,
                    show_hidden,
                    seen,
                ));
            }
        }
        for sym in inspect_own_symbol_keys(rt, id, true) {
            entries.push(inspect_symbol_property_entry(
                rt,
                id,
                &sym,
                false,
                depth,
                max_depth,
                break_length,
                sorted,
                max_string_length,
                max_array_length,
                colors,
                show_hidden,
                seen,
            ));
        }
        if show_hidden {
            for sym in inspect_own_symbol_keys(rt, id, false) {
                entries.push(inspect_symbol_property_entry(
                    rt,
                    id,
                    &sym,
                    true,
                    depth,
                    max_depth,
                    break_length,
                    sorted,
                    max_string_length,
                    max_array_length,
                    colors,
                    show_hidden,
                    seen,
                ));
            }
        }
    }

    let prefix = if colors {
        inspect_colorize_special(prefix, "35")
    } else {
        prefix
    };
    if entries.is_empty() {
        return prefix;
    }
    let force_multiline =
        inspect_force_nested_group_multiline("{", "}", &entries, depth, max_depth, break_length);
    format!(
        "{} {}",
        prefix,
        inspect_join_aggregate(
            "{",
            "}",
            &entries,
            depth,
            if force_multiline { 0 } else { break_length },
        )
    )
}

fn inspect_property_entry(
    rt: &Runtime,
    id: ObjectRef,
    key: &str,
    hidden: bool,
    depth: i32,
    max_depth: i32,
    break_length: usize,
    sorted: bool,
    max_string_length: Option<usize>,
    max_array_length: Option<usize>,
    colors: bool,
    show_hidden: bool,
    seen: &mut Vec<ObjectRef>,
) -> String {
    let label = if hidden {
        inspect_hidden_key(key)
    } else if is_ident_key(key) {
        key.to_string()
    } else {
        inspect_str(key)
    };
    let rendered = rt
        .obj(id)
        .get_own(key)
        .and_then(|d| inspect_accessor_placeholder(d.getter.is_some(), d.setter.is_some()))
        .map(str::to_string)
        .unwrap_or_else(|| {
            let val = rt.object_get(id, key);
            inspect_value_inner(
                rt,
                &val,
                depth + 1,
                max_depth,
                break_length,
                sorted,
                max_string_length,
                max_array_length,
                colors,
                show_hidden,
                seen,
            )
        });
    format!("{label}: {rendered}")
}

fn inspect_hidden_error_cause_property_entry(
    rt: &Runtime,
    id: ObjectRef,
    depth: i32,
    max_depth: i32,
    break_length: usize,
    sorted: bool,
    max_string_length: Option<usize>,
    max_array_length: Option<usize>,
    colors: bool,
    show_hidden: bool,
    seen: &mut Vec<ObjectRef>,
) -> String {
    let value = rt.object_get(id, "cause");
    if show_hidden && depth >= max_depth {
        if let Value::Object(error_id) = &value {
            if matches!(rt.obj(*error_id).internal_kind, InternalKind::Error) {
                let name = match rt.object_get(*error_id, "name") {
                    Value::String(s) if !s.is_empty() => s.as_str().to_string(),
                    _ => "Error".to_string(),
                };
                return format!("[cause]: [{name}]");
            }
        }
    }
    let rendered = inspect_value_inner(
        rt,
        &value,
        depth + 1,
        max_depth,
        break_length,
        sorted,
        max_string_length,
        max_array_length,
        colors,
        show_hidden,
        seen,
    );
    let rendered =
        inspect_indent_stack_frame_lines(rendered, &"  ".repeat((depth + 1).max(0) as usize));
    format!("[cause]: {rendered}")
}

fn inspect_aggregate_errors_property_entry(
    rt: &Runtime,
    id: ObjectRef,
    hidden: bool,
    depth: i32,
    max_depth: i32,
    break_length: usize,
    sorted: bool,
    max_string_length: Option<usize>,
    max_array_length: Option<usize>,
    colors: bool,
    show_hidden: bool,
    seen: &mut Vec<ObjectRef>,
) -> String {
    let label = if hidden { "[errors]" } else { "errors" };
    let Value::Object(errors) = rt.object_get(id, "errors") else {
        return inspect_property_entry(
            rt,
            id,
            "errors",
            hidden,
            depth,
            max_depth,
            break_length,
            sorted,
            max_string_length,
            max_array_length,
            colors,
            show_hidden,
            seen,
        );
    };
    if !matches!(rt.obj(errors).internal_kind, InternalKind::Array) {
        return inspect_property_entry(
            rt,
            id,
            "errors",
            hidden,
            depth,
            max_depth,
            break_length,
            sorted,
            max_string_length,
            max_array_length,
            colors,
            show_hidden,
            seen,
        );
    }
    if depth >= max_depth {
        return format!("{label}: [Array]");
    }

    let len = match rt.object_get(errors, "length") {
        Value::Number(n) if n.is_finite() && n >= 0.0 => n as usize,
        _ => 0,
    };
    let display_len = max_array_length.unwrap_or(len).min(len);
    let mut items = Vec::new();
    for idx in 0..display_len {
        let value = rt.object_get(errors, &idx.to_string());
        let rendered = match &value {
            Value::Object(error_id)
                if depth + 2 > max_depth
                    && matches!(rt.obj(*error_id).internal_kind, InternalKind::Error) =>
            {
                let name = match rt.object_get(*error_id, "name") {
                    Value::String(s) if !s.is_empty() => s.as_str().to_string(),
                    _ => "Error".to_string(),
                };
                format!("[{name}]")
            }
            _ => inspect_value_inner(
                rt,
                &value,
                depth + 1,
                max_depth,
                break_length,
                sorted,
                max_string_length,
                max_array_length,
                colors,
                show_hidden,
                seen,
            ),
        };
        items.push(inspect_indent_aggregate_error_item_lines(rendered, depth));
    }
    if display_len < len {
        items.push(format!(
            "... {} more item{}",
            len - display_len,
            if len - display_len == 1 { "" } else { "s" }
        ));
    }
    if show_hidden {
        items.push(format!("[length]: {}", len));
    }
    let force_multiline = items.iter().any(|item| item.contains('\n'))
        || inspect_force_nested_group_multiline(
            "[",
            "]",
            &items,
            depth + 1,
            max_depth,
            break_length,
        );
    let body = inspect_join_aggregate(
        "[",
        "]",
        &items,
        depth + 1,
        if force_multiline { 0 } else { break_length },
    );
    format!("{label}: {}", inspect_array_wrapper(rt, errors, len, body))
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum InspectGettersMode {
    Get,
    Set,
    Both,
}

fn inspect_getters_option_mode(rt: &Runtime, opts: ObjectRef) -> Option<InspectGettersMode> {
    match rt.object_get(opts, "getters") {
        Value::Boolean(true) => Some(InspectGettersMode::Both),
        Value::String(s) if s.as_str() == "get" => Some(InspectGettersMode::Get),
        Value::String(s) if s.as_str() == "set" => Some(InspectGettersMode::Set),
        Value::String(s) if s.as_str() == "getters" => Some(InspectGettersMode::Set),
        _ => None,
    }
}

#[derive(Clone)]
enum InspectGetterKey {
    String(String, bool),
    Symbol(Rc<String>, bool),
}

fn inspect_value_with_selected_getters(
    rt: &mut Runtime,
    value: &Value,
    depth: i32,
    max_depth: i32,
    break_length: usize,
    sorted: bool,
    max_string_length: Option<usize>,
    max_array_length: Option<usize>,
    colors: bool,
    show_hidden: bool,
    getters_mode: InspectGettersMode,
    seen: &mut Vec<ObjectRef>,
) -> String {
    match value {

        Value::Object(child) => inspect_selected_getters_map_set(
            rt,
            *child,
            depth,
            max_depth,
            break_length,
            sorted,
            max_string_length,
            max_array_length,
            colors,
            show_hidden,
            getters_mode,
        )
        .or_else(|| {
            inspect_selected_getters_object(
                rt,
                *child,
                depth,
                max_depth,
                break_length,
                sorted,
                max_string_length,
                max_array_length,
                colors,
                show_hidden,
                getters_mode,
                seen,
            )
        })
        .unwrap_or_else(|| {
            inspect_value_inner(
                rt,
                value,
                depth,
                max_depth,
                break_length,
                sorted,
                max_string_length,
                max_array_length,
                colors,
                show_hidden,
                seen,
            )
        }),
        _ => inspect_value_inner(
            rt,
            value,
            depth,
            max_depth,
            break_length,
            sorted,
            max_string_length,
            max_array_length,
            colors,
            show_hidden,
            seen,
        ),
    }
}

fn inspect_selected_getter_render(
    rt: &mut Runtime,
    id: ObjectRef,
    key: &str,
    desc: Option<rusty_js_runtime::value::PropertyDescriptor>,
    depth: i32,
    max_depth: i32,
    break_length: usize,
    sorted: bool,
    max_string_length: Option<usize>,
    max_array_length: Option<usize>,
    colors: bool,
    show_hidden: bool,
    getters_mode: InspectGettersMode,
    seen: &mut Vec<ObjectRef>,
) -> String {
    match desc {
        Some(d) if d.getter.is_some() && d.setter.is_none() => {
            if getters_mode == InspectGettersMode::Set {
                return "[Getter]".to_string();
            }
            let getter = d.getter.unwrap();
            match rt.call_getter_function(getter, Value::Object(id), key) {
                Ok(value) => {
                    let rendered = inspect_value_with_selected_getters(
                        rt,
                        &value,
                        depth + 1,
                        max_depth,
                        break_length,
                        sorted,
                        max_string_length,
                        max_array_length,
                        colors,
                        show_hidden,
                        getters_mode,
                        seen,
                    );
                    if matches!(value, Value::Object(_)) {
                        format!("[Getter] {rendered}")
                    } else {
                        format!("[Getter: {rendered}]")
                    }
                }
                Err(err) => inspect_getter_throw_value(rt, &err).unwrap_or_else(|| {
                    inspect_accessor_placeholder(true, d.setter.is_some())
                        .unwrap_or("[Getter]")
                        .to_string()
                }),
            }
        }
        Some(d) if d.getter.is_some() && d.setter.is_some() => {
            if matches!(
                getters_mode,
                InspectGettersMode::Both | InspectGettersMode::Set
            ) {
                let getter = d.getter.unwrap();
                match rt.call_getter_function(getter, Value::Object(id), key) {
                    Ok(value) => {
                        let rendered = inspect_value_with_selected_getters(
                            rt,
                            &value,
                            depth + 1,
                            max_depth,
                            break_length,
                            sorted,
                            max_string_length,
                            max_array_length,
                            colors,
                            show_hidden,
                            getters_mode,
                            seen,
                        );
                        if matches!(value, Value::Object(_)) {
                            format!("[Getter/Setter] {rendered}")
                        } else {
                            format!("[Getter/Setter: {rendered}]")
                        }
                    }
                    Err(err) => inspect_getter_throw_value(rt, &err).unwrap_or_else(|| {
                        inspect_accessor_placeholder(true, true)
                            .unwrap_or("[Getter/Setter]")
                            .to_string()
                    }),
                }
            } else {
                inspect_accessor_placeholder(true, true)
                    .unwrap_or("[Getter/Setter]")
                    .to_string()
            }
        }
        Some(d) if d.setter.is_some() => inspect_accessor_placeholder(false, true)
            .unwrap_or("[Setter]")
            .to_string(),
        Some(d) => match &d.value {
            Value::Object(child) if *child == id => "[Circular]".to_string(),
            _ => inspect_value_with_selected_getters(
                rt,
                &d.value,
                depth + 1,
                max_depth,
                break_length,
                sorted,
                max_string_length,
                max_array_length,
                colors,
                show_hidden,
                getters_mode,
                seen,
            ),
        },
        None => "undefined".to_string(),
    }
}

fn inspect_selected_getter_property_entry(
    rt: &mut Runtime,
    id: ObjectRef,
    key: InspectGetterKey,
    depth: i32,
    max_depth: i32,
    break_length: usize,
    sorted: bool,
    max_string_length: Option<usize>,
    max_array_length: Option<usize>,
    colors: bool,
    show_hidden: bool,
    getters_mode: InspectGettersMode,
    seen: &mut Vec<ObjectRef>,
) -> String {
    let (label, desc) = match &key {
        InspectGetterKey::String(key, hidden) => {
            let label = if *hidden {
                inspect_hidden_key(key)
            } else if is_ident_key(key) {
                key.clone()
            } else {
                inspect_str(key)
            };
            let desc = rt.obj(id).get_own(key).cloned();

            if desc.is_none() {
                let value = rt.object_get(id, key);
                let rendered = inspect_value_with_selected_getters(
                    rt,
                    &value,
                    depth + 1,
                    max_depth,
                    break_length,
                    sorted,
                    max_string_length,
                    max_array_length,
                    colors,
                    show_hidden,
                    getters_mode,
                    seen,
                );
                return format!("{label}: {rendered}");
            }
            (label, desc)
        }
        InspectGetterKey::Symbol(sym, hidden) => {
            let label = inspect_symbol_label(sym, *hidden);
            (label, rt.obj(id).get_own_symbol(sym).cloned())
        }
    };
    let rendered = inspect_selected_getter_render(
        rt,
        id,
        &label,
        desc,
        depth,
        max_depth,
        break_length,
        sorted,
        max_string_length,
        max_array_length,
        colors,
        show_hidden,
        getters_mode,
        seen,
    );
    format!("{label}: {rendered}")
}

fn inspect_selected_getters_object(
    rt: &mut Runtime,
    id: ObjectRef,
    depth: i32,
    max_depth: i32,
    break_length: usize,
    sorted: bool,
    max_string_length: Option<usize>,
    max_array_length: Option<usize>,
    colors: bool,
    show_hidden: bool,
    getters_mode: InspectGettersMode,
    seen: &mut Vec<ObjectRef>,
) -> Option<String> {
    let kind = &rt.obj(id).internal_kind;
    if matches!(
        kind,
        InternalKind::Array
            | InternalKind::Error
            | InternalKind::Function(_)
            | InternalKind::Closure(_)
            | InternalKind::BoundFunction(_)
            | InternalKind::RegExp(_)
    ) {
        return None;
    }

    if depth > max_depth {
        return Some("[Object]".to_string());
    }
    if let Some(pos) = seen.iter().rposition(|x| *x == id) {
        return Some(format!("[Circular *{}]", pos + 1));
    }

    let mut keys: Vec<InspectGetterKey> = rt
        .ordinary_own_enumerable_string_keys(id)
        .into_iter()
        .map(|name| InspectGetterKey::String(name, false))
        .collect();
    for (key, desc) in rt.obj(id).properties.iter() {
        match key {
            PropertyKey::String(name)
                if show_hidden && !desc.enumerable && inspect_show_hidden_allows_key(name) =>
            {
                keys.push(InspectGetterKey::String(name.clone(), true));
            }
            PropertyKey::Symbol(sym) if desc.enumerable => {
                keys.push(InspectGetterKey::Symbol(sym.clone(), false));
            }
            PropertyKey::Symbol(sym) if show_hidden && !desc.enumerable => {
                keys.push(InspectGetterKey::Symbol(sym.clone(), true));
            }
            _ => {}
        }
    }
    if sorted {
        keys.sort_by(|a, b| {
            let label = |key: &InspectGetterKey| match key {
                InspectGetterKey::String(s, _) => s.clone(),
                InspectGetterKey::Symbol(sym, hidden) => inspect_symbol_label(sym, *hidden),
            };
            label(a).cmp(&label(b))
        });
    }
    let ref_index = seen.len() + 1;
    seen.push(id);
    let mut entries = Vec::new();
    for key in keys {
        entries.push(inspect_selected_getter_property_entry(
            rt,
            id,
            key,
            depth,
            max_depth,
            break_length,
            sorted,
            max_string_length,
            max_array_length,
            colors,
            show_hidden,
            getters_mode,
            seen,
        ));
    }
    let rendered = inspect_join_aggregate("{", "}", &entries, depth, break_length);
    seen.pop();
    Some(inspect_ref_prefix(rendered, ref_index))
}

fn inspect_selected_getters_function(
    rt: &mut Runtime,
    id: ObjectRef,
    depth: i32,
    max_depth: i32,
    break_length: usize,
    sorted: bool,
    max_string_length: Option<usize>,
    max_array_length: Option<usize>,
    colors: bool,
    show_hidden: bool,
    getters_mode: InspectGettersMode,
) -> Option<String> {
    let kind = &rt.obj(id).internal_kind;
    if !matches!(
        kind,
        InternalKind::Function(_) | InternalKind::Closure(_) | InternalKind::BoundFunction(_)
    ) {
        return None;
    }
    let name = match rt.object_get(id, "name") {
        Value::String(s) if !s.is_empty() => format!(": {}", s.as_str()),
        _ => " (anonymous)".to_string(),
    };
    let has_circular_prototype = matches!(rt.object_get(id, "prototype"), Value::Object(proto) if matches!(rt.object_get(proto, "constructor"), Value::Object(constructor) if constructor == id));
    let mut prefix = if show_hidden && has_circular_prototype {
        format!("<ref *1> [Function{}]", name)
    } else {
        format!("[Function{}]", name)
    };
    if colors {
        prefix = inspect_colorize_special(prefix, "36");
    }
    let mut visible_keys: Vec<InspectGetterKey> = rt
        .obj(id)
        .properties
        .iter()
        .filter_map(|(key, desc)| match key {
            PropertyKey::String(name)
                if desc.enumerable && !matches!(name.as_str(), "length" | "name" | "prototype") =>
            {
                Some(InspectGetterKey::String(name.clone(), false))
            }
            PropertyKey::Symbol(sym) if desc.enumerable => {
                Some(InspectGetterKey::Symbol(sym.clone(), false))
            }
            _ => None,
        })
        .collect();
    let mut hidden_keys: Vec<InspectGetterKey> = if show_hidden {
        rt.obj(id)
            .properties
            .iter()
            .filter_map(|(key, desc)| match key {
                PropertyKey::String(name)
                    if !desc.enumerable
                        && !matches!(name.as_str(), "length" | "name" | "prototype") =>
                {
                    Some(InspectGetterKey::String(name.clone(), true))
                }
                PropertyKey::Symbol(sym) if !desc.enumerable => {
                    Some(InspectGetterKey::Symbol(sym.clone(), true))
                }
                _ => None,
            })
            .collect()
    } else {
        Vec::new()
    };
    if sorted {
        let label = |key: &InspectGetterKey| match key {
            InspectGetterKey::String(s, _) => s.clone(),
            InspectGetterKey::Symbol(sym, hidden) => inspect_symbol_label(sym, *hidden),
        };
        visible_keys.sort_by(|a, b| label(a).cmp(&label(b)));
        hidden_keys.sort_by(|a, b| label(a).cmp(&label(b)));
    }
    if !show_hidden && visible_keys.is_empty() {
        return Some(prefix);
    }
    let mut seen = vec![id];
    let mut entries = Vec::new();
    if show_hidden {
        for key in ["length", "name", "prototype"] {
            if rt.obj(id).get_own(key).is_some() {
                entries.push(inspect_property_entry(
                    rt,
                    id,
                    key,
                    true,
                    depth,
                    max_depth,
                    break_length,
                    sorted,
                    max_string_length,
                    max_array_length,
                    colors,
                    show_hidden,
                    &mut seen,
                ));
            }
        }
    }
    for key in visible_keys.into_iter().chain(hidden_keys) {
        entries.push(inspect_selected_getter_property_entry(
            rt,
            id,
            key,
            depth,
            max_depth,
            break_length,
            sorted,
            max_string_length,
            max_array_length,
            colors,
            show_hidden,
            getters_mode,
            &mut seen,
        ));
    }
    if entries.is_empty() {
        return Some(prefix);
    }
    let force_multiline = (show_hidden && has_circular_prototype)
        || inspect_force_nested_group_multiline("{", "}", &entries, depth, max_depth, break_length);
    Some(format!(
        "{} {}",
        prefix,
        inspect_join_aggregate(
            "{",
            "}",
            &entries,
            depth,
            if force_multiline { 0 } else { break_length },
        )
    ))
}

fn inspect_selected_getters_array(
    rt: &mut Runtime,
    id: ObjectRef,
    depth: i32,
    max_depth: i32,
    break_length: usize,
    sorted: bool,
    max_string_length: Option<usize>,
    max_array_length: Option<usize>,
    colors: bool,
    show_hidden: bool,
    getters_mode: InspectGettersMode,
) -> Option<String> {
    if !matches!(rt.obj(id).internal_kind, InternalKind::Array) {
        return None;
    }
    if depth >= max_depth {
        return Some("[Array]".to_string());
    }
    let len = match rt.object_get(id, "length") {
        Value::Number(n) => n as usize,
        _ => 0,
    };
    let mut seen = vec![id];
    let mut items: Vec<String> = Vec::new();
    let mut hole_run = 0usize;
    let flush_holes = |items: &mut Vec<String>, run: &mut usize| {
        if *run > 0 {
            items.push(format!(
                "<{} empty item{}>",
                run,
                if *run == 1 { "" } else { "s" }
            ));
            *run = 0;
        }
    };
    let display_len = max_array_length.unwrap_or(len).min(len);
    for i in 0..display_len {
        let key = i.to_string();

        if !rt.obj(id).has_own_str(&key) {
            hole_run += 1;
            continue;
        }
        flush_holes(&mut items, &mut hole_run);
        let desc = rt.obj(id).get_own(&key).cloned();
        if desc.is_some() {

            items.push(inspect_selected_getter_render(
                rt,
                id,
                &key,
                desc,
                depth,
                max_depth,
                break_length,
                sorted,
                max_string_length,
                max_array_length,
                colors,
                show_hidden,
                getters_mode,
                &mut seen,
            ));
        } else {

            let value = rt.object_get(id, &key);
            items.push(inspect_value_with_selected_getters(
                rt,
                &value,
                depth + 1,
                max_depth,
                break_length,
                sorted,
                max_string_length,
                max_array_length,
                colors,
                show_hidden,
                getters_mode,
                &mut seen,
            ));
        }
    }
    flush_holes(&mut items, &mut hole_run);
    if display_len < len {
        let remaining = len - display_len;
        items.push(format!(
            "... {} more item{}",
            remaining,
            if remaining == 1 { "" } else { "s" }
        ));
    }
    if show_hidden {
        items.push(format!("[length]: {}", len));
    }
    let mut visible_keys: Vec<InspectGetterKey> = rt
        .obj(id)
        .properties
        .iter()
        .filter_map(|(key, desc)| match key {
            PropertyKey::String(name)
                if desc.enumerable
                    && name != "length"
                    && !name.parse::<usize>().is_ok_and(|idx| idx < len) =>
            {
                Some(InspectGetterKey::String(name.clone(), false))
            }
            PropertyKey::Symbol(sym) if desc.enumerable => {
                Some(InspectGetterKey::Symbol(sym.clone(), false))
            }
            _ => None,
        })
        .collect();
    let mut hidden_keys: Vec<InspectGetterKey> = if show_hidden {
        rt.obj(id)
            .properties
            .iter()
            .filter_map(|(key, desc)| match key {
                PropertyKey::String(name)
                    if !desc.enumerable
                        && name != "length"
                        && !name.parse::<usize>().is_ok_and(|idx| idx < len)
                        && inspect_show_hidden_allows_key(name) =>
                {
                    Some(InspectGetterKey::String(name.clone(), true))
                }
                PropertyKey::Symbol(sym) if !desc.enumerable => {
                    Some(InspectGetterKey::Symbol(sym.clone(), true))
                }
                _ => None,
            })
            .collect()
    } else {
        Vec::new()
    };
    if sorted {
        let label = |key: &InspectGetterKey| match key {
            InspectGetterKey::String(s, _) => s.clone(),
            InspectGetterKey::Symbol(sym, hidden) => inspect_symbol_label(sym, *hidden),
        };
        visible_keys.sort_by(|a, b| label(a).cmp(&label(b)));
        hidden_keys.sort_by(|a, b| label(a).cmp(&label(b)));
    }
    let has_hidden_user_entries = !hidden_keys.is_empty();
    for key in visible_keys.into_iter().chain(hidden_keys) {
        items.push(inspect_selected_getter_property_entry(
            rt,
            id,
            key,
            depth,
            max_depth,
            break_length,
            sorted,
            max_string_length,
            max_array_length,
            colors,
            show_hidden,
            getters_mode,
            &mut seen,
        ));
    }
    let force_multiline = has_hidden_user_entries
        || inspect_force_nested_group_multiline("[", "]", &items, depth, max_depth, break_length);
    let body = inspect_join_aggregate(
        "[",
        "]",
        &items,
        depth,
        if force_multiline { 0 } else { break_length },
    );
    Some(inspect_array_wrapper(rt, id, len, body))
}

fn inspect_hex_bytes(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn inspect_map_set_suffix_getter_keys(
    rt: &Runtime,
    id: ObjectRef,
    show_hidden: bool,
    sorted: bool,
) -> Vec<InspectGetterKey> {
    let mut keys: Vec<InspectGetterKey> = rt
        .obj(id)
        .properties
        .iter()
        .filter_map(|(key, desc)| match key {
            PropertyKey::String(name)
                if matches!(
                    name.as_str(),
                    "__map_data"
                        | "__map_orig_keys"
                        | "__set_data"
                        | "__weak_collection_storage"
                        | "__is_weakmap"
                        | "__is_weakset"
                ) =>
            {
                None
            }
            PropertyKey::String(name) if desc.enumerable => {
                Some(InspectGetterKey::String(name.clone(), false))
            }
            PropertyKey::String(name)
                if show_hidden && !desc.enumerable && inspect_show_hidden_allows_key(name) =>
            {
                Some(InspectGetterKey::String(name.clone(), true))
            }
            PropertyKey::Symbol(sym) if desc.enumerable => {
                Some(InspectGetterKey::Symbol(sym.clone(), false))
            }
            PropertyKey::Symbol(sym) if show_hidden && !desc.enumerable => {
                Some(InspectGetterKey::Symbol(sym.clone(), true))
            }
            _ => None,
        })
        .collect();
    if sorted {
        keys.sort_by(|a, b| {
            let label = |key: &InspectGetterKey| match key {
                InspectGetterKey::String(s, _) => s.clone(),
                InspectGetterKey::Symbol(sym, hidden) => inspect_symbol_label(sym, *hidden),
            };
            label(a).cmp(&label(b))
        });
    }
    keys
}

fn inspect_ordinary_suffix_getter_keys(
    rt: &Runtime,
    id: ObjectRef,
    show_hidden: bool,
    sorted: bool,
    skip: &[&str],
) -> Vec<InspectGetterKey> {
    let mut keys: Vec<InspectGetterKey> = rt
        .obj(id)
        .properties
        .iter()
        .filter_map(|(key, desc)| match key {
            PropertyKey::String(name) if skip.iter().any(|skip| *skip == name.as_str()) => None,
            PropertyKey::String(name) if desc.enumerable => {
                Some(InspectGetterKey::String(name.clone(), false))
            }
            PropertyKey::String(name)
                if show_hidden && !desc.enumerable && inspect_show_hidden_allows_key(name) =>
            {
                Some(InspectGetterKey::String(name.clone(), true))
            }
            PropertyKey::Symbol(sym) if desc.enumerable => {
                Some(InspectGetterKey::Symbol(sym.clone(), false))
            }
            PropertyKey::Symbol(sym) if show_hidden && !desc.enumerable => {
                Some(InspectGetterKey::Symbol(sym.clone(), true))
            }
            _ => None,
        })
        .collect();
    if sorted {
        keys.sort_by(|a, b| {
            let label = |key: &InspectGetterKey| match key {
                InspectGetterKey::String(s, _) => s.clone(),
                InspectGetterKey::Symbol(sym, hidden) => inspect_symbol_label(sym, *hidden),
            };
            label(a).cmp(&label(b))
        });
    }
    keys
}

fn inspect_selected_getters_map_set(
    rt: &mut Runtime,
    id: ObjectRef,
    depth: i32,
    max_depth: i32,
    break_length: usize,
    sorted: bool,
    max_string_length: Option<usize>,
    max_array_length: Option<usize>,
    colors: bool,
    show_hidden: bool,
    getters_mode: InspectGettersMode,
) -> Option<String> {
    if let Value::Object(storage) = rt.object_get(id, "__map_data") {
        let orig = rt.object_get(id, "__map_orig_keys");
        let skeys = rt.ordinary_own_enumerable_string_keys(storage);
        let mut seen = vec![id];
        let mut entries = Vec::new();
        for sk in &skeys {
            let val = rt.object_get(storage, sk);
            let keyv = match &orig {
                Value::Object(o) if rt.obj(*o).has_own_str(sk) => rt.object_get(*o, sk),
                _ => Value::String(Rc::new(rusty_js_runtime::value::JsString::from(sk.clone()))),
            };
            let rendered_key = if matches!(keyv, Value::Object(child) if child != id) {
                let mut local_seen = Vec::new();
                inspect_value_with_selected_getters(
                    rt,
                    &keyv,
                    depth + 1,
                    max_depth,
                    break_length,
                    sorted,
                    max_string_length,
                    max_array_length,
                    colors,
                    show_hidden,
                    getters_mode,
                    &mut local_seen,
                )
            } else {
                inspect_value_with_selected_getters(
                    rt,
                    &keyv,
                    depth + 1,
                    max_depth,
                    break_length,
                    sorted,
                    max_string_length,
                    max_array_length,
                    colors,
                    show_hidden,
                    getters_mode,
                    &mut seen,
                )
            };
            let rendered_value = if matches!(val, Value::Object(child) if child != id) {
                let mut local_seen = Vec::new();
                inspect_value_with_selected_getters(
                    rt,
                    &val,
                    depth + 1,
                    max_depth,
                    break_length,
                    sorted,
                    max_string_length,
                    max_array_length,
                    colors,
                    show_hidden,
                    getters_mode,
                    &mut local_seen,
                )
            } else {
                inspect_value_with_selected_getters(
                    rt,
                    &val,
                    depth + 1,
                    max_depth,
                    break_length,
                    sorted,
                    max_string_length,
                    max_array_length,
                    colors,
                    show_hidden,
                    getters_mode,
                    &mut seen,
                )
            };
            entries.push(format!("{} => {}", rendered_key, rendered_value));
        }
        for key in inspect_map_set_suffix_getter_keys(rt, id, show_hidden, sorted) {
            entries.push(inspect_selected_getter_property_entry(
                rt,
                id,
                key,
                depth,
                max_depth,
                break_length,
                sorted,
                max_string_length,
                max_array_length,
                colors,
                show_hidden,
                getters_mode,
                &mut seen,
            ));
        }
        let has_hidden_symbol = show_hidden && !inspect_own_symbol_keys(rt, id, false).is_empty();
        let body = if has_hidden_symbol {
            inspect_multiline_block("{", "}", &entries, depth)
        } else {
            inspect_join_aggregate("{", "}", &entries, depth, break_length)
        };
        return Some(format!("Map({}) {}", skeys.len(), body));
    }
    if let Value::Object(storage) = rt.object_get(id, "__set_data") {
        let skeys = rt.ordinary_own_enumerable_string_keys(storage);
        let mut seen = vec![id];
        let mut entries = Vec::new();
        for sk in &skeys {
            let value = rt.object_get(storage, sk);
            let rendered = if matches!(value, Value::Object(child) if child != id) {
                let mut local_seen = Vec::new();
                inspect_value_with_selected_getters(
                    rt,
                    &value,
                    depth + 1,
                    max_depth,
                    break_length,
                    sorted,
                    max_string_length,
                    max_array_length,
                    colors,
                    show_hidden,
                    getters_mode,
                    &mut local_seen,
                )
            } else {
                inspect_value_with_selected_getters(
                    rt,
                    &value,
                    depth + 1,
                    max_depth,
                    break_length,
                    sorted,
                    max_string_length,
                    max_array_length,
                    colors,
                    show_hidden,
                    getters_mode,
                    &mut seen,
                )
            };
            entries.push(rendered);
        }
        for key in inspect_map_set_suffix_getter_keys(rt, id, show_hidden, sorted) {
            entries.push(inspect_selected_getter_property_entry(
                rt,
                id,
                key,
                depth,
                max_depth,
                break_length,
                sorted,
                max_string_length,
                max_array_length,
                colors,
                show_hidden,
                getters_mode,
                &mut seen,
            ));
        }
        return Some(format!(
            "Set({}) {}",
            skeys.len(),
            inspect_join_aggregate("{", "}", &entries, depth, break_length)
        ));
    }
    None
}

fn inspect_weak_collection_object(
    rt: &Runtime,
    id: ObjectRef,
    label: &str,
    depth: i32,
    max_depth: i32,
    break_length: usize,
    sorted: bool,
    max_string_length: Option<usize>,
    max_array_length: Option<usize>,
    colors: bool,
    show_hidden: bool,
    seen: &mut Vec<ObjectRef>,
) -> String {
    let mut entries = Vec::new();
    if !show_hidden {
        entries.push("<items unknown>".to_string());
    }
    for key in inspect_map_set_suffix_getter_keys(rt, id, show_hidden, sorted) {
        match key {
            InspectGetterKey::String(key, hidden) => entries.push(inspect_property_entry(
                rt,
                id,
                &key,
                hidden,
                depth,
                max_depth,
                break_length,
                sorted,
                max_string_length,
                max_array_length,
                colors,
                show_hidden,
                seen,
            )),
            InspectGetterKey::Symbol(sym, hidden) => entries.push(inspect_symbol_property_entry(
                rt,
                id,
                &sym,
                hidden,
                depth,
                max_depth,
                break_length,
                sorted,
                max_string_length,
                max_array_length,
                colors,
                show_hidden,
                seen,
            )),
        }
    }
    let body = if show_hidden && entries.is_empty() {
        "{  }".to_string()
    } else {
        inspect_join_aggregate("{", "}", &entries, depth, break_length)
    };
    format!("{label} {body}")
}

fn inspect_weak_collection_object_selected_getters(
    rt: &mut Runtime,
    id: ObjectRef,
    label: &str,
    depth: i32,
    max_depth: i32,
    break_length: usize,
    sorted: bool,
    max_string_length: Option<usize>,
    max_array_length: Option<usize>,
    colors: bool,
    show_hidden: bool,
    getters_mode: InspectGettersMode,
    seen: &mut Vec<ObjectRef>,
) -> String {
    let mut entries = Vec::new();
    if !show_hidden {
        entries.push("<items unknown>".to_string());
    }
    for key in inspect_map_set_suffix_getter_keys(rt, id, show_hidden, sorted) {
        entries.push(inspect_selected_getter_property_entry(
            rt,
            id,
            key,
            depth,
            max_depth,
            break_length,
            sorted,
            max_string_length,
            max_array_length,
            colors,
            show_hidden,
            getters_mode,
            seen,
        ));
    }
    let body = if show_hidden && entries.is_empty() {
        "{  }".to_string()
    } else {
        inspect_join_aggregate("{", "}", &entries, depth, break_length)
    };
    format!("{label} {body}")
}

fn inspect_promise_state_entry(
    rt: &Runtime,
    id: ObjectRef,
    depth: i32,
    max_depth: i32,
    break_length: usize,
    sorted: bool,
    max_string_length: Option<usize>,
    max_array_length: Option<usize>,
    colors: bool,
    show_hidden: bool,
    seen: &mut Vec<ObjectRef>,
) -> String {
    match &rt.obj(id).internal_kind {
        InternalKind::Promise(state) => match state.status {
            rusty_js_runtime::value::PromiseStatus::Fulfilled => inspect_value_inner(
                rt,
                &state.value,
                depth + 1,
                max_depth,
                break_length,
                sorted,
                max_string_length,
                max_array_length,
                colors,
                show_hidden,
                seen,
            ),
            rusty_js_runtime::value::PromiseStatus::Rejected => format!(
                "<rejected> {}",
                inspect_value_inner(
                    rt,
                    &state.value,
                    depth + 1,
                    max_depth,
                    break_length,
                    sorted,
                    max_string_length,
                    max_array_length,
                    colors,
                    show_hidden,
                    seen,
                )
            ),
            rusty_js_runtime::value::PromiseStatus::Pending => "<pending>".to_string(),
        },
        _ => "<pending>".to_string(),
    }
}

fn inspect_promise_object(
    rt: &Runtime,
    id: ObjectRef,
    depth: i32,
    max_depth: i32,
    break_length: usize,
    sorted: bool,
    max_string_length: Option<usize>,
    max_array_length: Option<usize>,
    colors: bool,
    show_hidden: bool,
    seen: &mut Vec<ObjectRef>,
) -> String {
    let mut entries = vec![inspect_promise_state_entry(
        rt,
        id,
        depth,
        max_depth,
        break_length,
        sorted,
        max_string_length,
        max_array_length,
        colors,
        show_hidden,
        seen,
    )];
    for key in inspect_ordinary_suffix_getter_keys(
        rt,
        id,
        show_hidden,
        sorted,
        &["__async_id__", "__trigger_async_id__"],
    ) {
        match key {
            InspectGetterKey::String(key, hidden) => entries.push(inspect_property_entry(
                rt,
                id,
                &key,
                hidden,
                depth,
                max_depth,
                break_length,
                sorted,
                max_string_length,
                max_array_length,
                colors,
                show_hidden,
                seen,
            )),
            InspectGetterKey::Symbol(sym, hidden) => entries.push(inspect_symbol_property_entry(
                rt,
                id,
                &sym,
                hidden,
                depth,
                max_depth,
                break_length,
                sorted,
                max_string_length,
                max_array_length,
                colors,
                show_hidden,
                seen,
            )),
        }
    }
    format!(
        "Promise {}",
        inspect_join_promise_aggregate(&entries, depth, break_length)
    )
}

fn inspect_promise_object_selected_getters(
    rt: &mut Runtime,
    id: ObjectRef,
    depth: i32,
    max_depth: i32,
    break_length: usize,
    sorted: bool,
    max_string_length: Option<usize>,
    max_array_length: Option<usize>,
    colors: bool,
    show_hidden: bool,
    getters_mode: InspectGettersMode,
    seen: &mut Vec<ObjectRef>,
) -> String {
    let mut entries = vec![inspect_promise_state_entry(
        rt,
        id,
        depth,
        max_depth,
        break_length,
        sorted,
        max_string_length,
        max_array_length,
        colors,
        show_hidden,
        seen,
    )];
    for key in inspect_ordinary_suffix_getter_keys(
        rt,
        id,
        show_hidden,
        sorted,
        &["__async_id__", "__trigger_async_id__"],
    ) {
        entries.push(inspect_selected_getter_property_entry(
            rt,
            id,
            key,
            depth,
            max_depth,
            break_length,
            sorted,
            max_string_length,
            max_array_length,
            colors,
            show_hidden,
            getters_mode,
            seen,
        ));
    }
    format!(
        "Promise {}",
        inspect_join_promise_aggregate(&entries, depth, break_length)
    )
}

fn inspect_array_buffer_metadata(bytes: &[u8]) -> String {
    format!(
        "ArrayBuffer {{ [Uint8Contents]: <{}>, [byteLength]: {} }}",
        inspect_hex_bytes(bytes),
        bytes.len()
    )
}

fn inspect_array_buffer_render(
    bytes: &[u8],
    name: &str,
    depth: i32,
    break_length: usize,
    max_array_length: Option<usize>,
) -> String {
    let byte_len = bytes.len();
    let shown = byte_len.min(max_array_length.unwrap_or(100));
    let hex = bytes[..shown]
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .join(" ");
    let contents = if shown < byte_len {
        let more = byte_len - shown;
        if shown == 0 {
            format!("... {more} more bytes")
        } else {
            format!("{hex} ... {more} more bytes")
        }
    } else {
        hex
    };
    let entries = vec![
        format!("[Uint8Contents]: <{contents}>"),
        format!("[byteLength]: {byte_len}"),
    ];
    let open = format!("{name} {{");
    inspect_join_aggregate(&open, "}", &entries, depth, break_length)
}

fn inspect_typed_view_prefix_items(
    rt: &Runtime,
    id: ObjectRef,
    depth: i32,
    max_depth: i32,
    break_length: usize,
) -> Option<(String, Vec<String>)> {
    let view = rt.typed_array_views.get(&id)?;
    if &*view.element_kind == "DataView" {
        return None;
    }
    let len = match rt.object_get(id, "length") {
        Value::Number(n) if n.is_finite() && n >= 0.0 => n as usize,
        _ => 0,
    };
    let mut items = Vec::with_capacity(len);
    for idx in 0..len {
        let value = rt.object_get(id, &idx.to_string());
        items.push(inspect_value(
            rt,
            &value,
            depth + 1,
            max_depth,
            break_length,
        ));
    }
    Some((format!("{}({len})", view.element_kind), items))
}

fn inspect_data_view_prefix_items(
    rt: &Runtime,
    id: ObjectRef,
    depth: i32,
    break_length: usize,
    max_array_length: Option<usize>,
) -> Option<(String, Vec<String>)> {
    let (byte_offset, backing) = {
        let view = rt.typed_array_views.get(&id)?;
        if &*view.element_kind != "DataView" {
            return None;
        }
        (view.byte_offset, view.buffer)
    };

    let view_bytes = rt.typed_array_view_bytes(id)?;
    let buffer_bytes = rt
        .array_buffers
        .get(&backing)
        .map(|r| r.to_bytes())
        .unwrap_or_else(|| view_bytes.clone());
    let buffer = inspect_array_buffer_render(
        &buffer_bytes,
        "ArrayBuffer",
        depth + 1,
        break_length,
        max_array_length,
    );
    Some((
        "DataView".to_string(),
        vec![
            format!("[byteLength]: {}", view_bytes.len()),
            format!("[byteOffset]: {}", byte_offset),
            format!("[buffer]: {}", buffer),
        ],
    ))
}

fn buffer_inspect_max_bytes(rt: &Runtime) -> usize {
    match rt.global_get("__cruft_buffer_inspect_max_bytes") {
        Value::Number(n) if n.is_infinite() && n.is_sign_positive() => usize::MAX,
        Value::Number(n) if n.is_finite() && n >= 0.0 => n as usize,
        _ => 50,
    }
}

fn inspect_buffer_object(
    rt: &Runtime,
    id: ObjectRef,
    depth: i32,
    max_depth: i32,
    break_length: usize,
    sorted: bool,
    max_string_length: Option<usize>,
    max_array_length: Option<usize>,
    colors: bool,
    show_hidden: bool,
    seen: &mut Vec<ObjectRef>,
) -> Option<String> {
    if !rt.obj(id).is_buffer {
        return None;
    }
    let bytes = rt.typed_array_view_bytes(id)?;
    let max = buffer_inspect_max_bytes(rt);
    let shown = bytes.len().min(max);
    let mut entries = Vec::new();
    let mut byte_entry = if shown > 0 {
        inspect_hex_bytes(&bytes[..shown])
    } else {
        String::new()
    };
    if shown < bytes.len() {
        let remaining = bytes.len() - shown;
        let unit = if remaining == 1 { "byte" } else { "bytes" };
        if byte_entry.is_empty() {
            byte_entry = format!("... {remaining} more {unit}");
        } else {
            byte_entry.push_str(&format!(" ... {remaining} more {unit}"));
        }
    }
    if !byte_entry.is_empty() {
        entries.push(byte_entry);
    }
    for key in inspect_typed_array_visible_keys(rt, id, sorted).into_iter() {
        match key {
            InspectGetterKey::String(key, _) => entries.push(inspect_property_entry(
                rt,
                id,
                &key,
                false,
                depth,
                max_depth,
                break_length,
                sorted,
                max_string_length,
                max_array_length,
                colors,
                show_hidden,
                seen,
            )),
            InspectGetterKey::Symbol(sym, _) => entries.push(inspect_symbol_property_entry(
                rt,
                id,
                &sym,
                false,
                depth,
                max_depth,
                break_length,
                sorted,
                max_string_length,
                max_array_length,
                colors,
                show_hidden,
                seen,
            )),
        }
    }
    if show_hidden {
        for key in inspect_typed_array_hidden_user_keys(rt, id, sorted).into_iter() {
            match key {
                InspectGetterKey::String(key, _) => entries.push(inspect_property_entry(
                    rt,
                    id,
                    &key,
                    true,
                    depth,
                    max_depth,
                    break_length,
                    sorted,
                    max_string_length,
                    max_array_length,
                    colors,
                    show_hidden,
                    seen,
                )),
                InspectGetterKey::Symbol(sym, _) => entries.push(inspect_symbol_property_entry(
                    rt,
                    id,
                    &sym,
                    true,
                    depth,
                    max_depth,
                    break_length,
                    sorted,
                    max_string_length,
                    max_array_length,
                    colors,
                    show_hidden,
                    seen,
                )),
            }
        }
    }
    Some(if entries.is_empty() {
        "<Buffer >".to_string()
    } else {
        format!("<Buffer {}>", entries.join(", "))
    })
}

fn inspect_typed_array_visible_keys(
    rt: &Runtime,
    id: ObjectRef,
    sorted: bool,
) -> Vec<InspectGetterKey> {
    let mut keys = rt
        .obj(id)
        .properties
        .iter()
        .filter_map(|(key, desc)| match key {
            PropertyKey::String(name)
                if desc.enumerable
                    && !matches!(
                        name.as_str(),
                        "length" | "byteLength" | "byteOffset" | "buffer"
                    )
                    && !name.parse::<usize>().is_ok() =>
            {
                Some(InspectGetterKey::String(name.clone(), false))
            }
            PropertyKey::Symbol(sym) if desc.enumerable => {
                Some(InspectGetterKey::Symbol(sym.clone(), false))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    if sorted {
        let label = |key: &InspectGetterKey| match key {
            InspectGetterKey::String(s, _) => s.clone(),
            InspectGetterKey::Symbol(sym, hidden) => inspect_symbol_label(sym, *hidden),
        };
        keys.sort_by(|a, b| label(a).cmp(&label(b)));
    }
    keys
}

fn inspect_typed_array_hidden_user_keys(
    rt: &Runtime,
    id: ObjectRef,
    sorted: bool,
) -> Vec<InspectGetterKey> {
    let mut keys = rt
        .obj(id)
        .properties
        .iter()
        .filter_map(|(key, desc)| match key {
            PropertyKey::String(name)
                if !desc.enumerable
                    && !matches!(
                        name.as_str(),
                        "length" | "byteLength" | "byteOffset" | "buffer"
                    )
                    && !name.parse::<usize>().is_ok()
                    && inspect_show_hidden_allows_key(name) =>
            {
                Some(InspectGetterKey::String(name.clone(), true))
            }
            PropertyKey::Symbol(sym) if !desc.enumerable => {
                Some(InspectGetterKey::Symbol(sym.clone(), true))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    if sorted {
        let label = |key: &InspectGetterKey| match key {
            InspectGetterKey::String(s, _) => s.clone(),
            InspectGetterKey::Symbol(sym, hidden) => inspect_symbol_label(sym, *hidden),
        };
        keys.sort_by(|a, b| label(a).cmp(&label(b)));
    }
    keys
}

fn inspect_typed_view_object(
    rt: &Runtime,
    id: ObjectRef,
    depth: i32,
    max_depth: i32,
    break_length: usize,
    sorted: bool,
    max_string_length: Option<usize>,
    max_array_length: Option<usize>,
    colors: bool,
    show_hidden: bool,
    seen: &mut Vec<ObjectRef>,
) -> Option<String> {
    if let Some(rendered) = inspect_buffer_object(
        rt,
        id,
        depth,
        max_depth,
        break_length,
        sorted,
        max_string_length,
        max_array_length,
        colors,
        show_hidden,
        seen,
    ) {
        return Some(rendered);
    }
    let (prefix, mut items) = if let Some((prefix, mut items)) =
        inspect_typed_view_prefix_items(rt, id, depth, max_depth, break_length)
    {
        let len = items.len();
        let view = rt.typed_array_views.get(&id)?;
        if show_hidden {
            items.push(format!("[BYTES_PER_ELEMENT]: {}", view.bytes_per_element));
            items.push(format!("[length]: {len}"));
            items.push(format!(
                "[byteLength]: {}",
                len.saturating_mul(view.bytes_per_element)
            ));
            items.push(format!("[byteOffset]: {}", view.byte_offset));
            items.push(format!(
                "[buffer]: ArrayBuffer {{ [byteLength]: {} }}",
                len.saturating_mul(view.bytes_per_element)
            ));
        }
        (prefix, items)
    } else {
        inspect_data_view_prefix_items(rt, id, depth, break_length, max_array_length)?
    };
    let force_multiline = prefix == "DataView" || show_hidden;
    let aggregate_open = if prefix == "DataView" { "{" } else { "[" };
    let aggregate_close = if prefix == "DataView" { "}" } else { "]" };
    let separator = if prefix == "DataView" { " " } else { " " };

    let numeric_element_count = items.len();
    let pure_numeric_view = prefix != "DataView" && !force_multiline;
    for key in inspect_typed_array_visible_keys(rt, id, sorted).into_iter() {
        match key {
            InspectGetterKey::String(key, _) => items.push(inspect_property_entry(
                rt,
                id,
                &key,
                false,
                depth,
                max_depth,
                break_length,
                sorted,
                max_string_length,
                max_array_length,
                colors,
                show_hidden,
                seen,
            )),
            InspectGetterKey::Symbol(sym, _) => items.push(inspect_symbol_property_entry(
                rt,
                id,
                &sym,
                false,
                depth,
                max_depth,
                break_length,
                sorted,
                max_string_length,
                max_array_length,
                colors,
                show_hidden,
                seen,
            )),
        }
    }
    if show_hidden {
        for key in inspect_typed_array_hidden_user_keys(rt, id, sorted).into_iter() {
            match key {
                InspectGetterKey::String(key, _) => items.push(inspect_property_entry(
                    rt,
                    id,
                    &key,
                    true,
                    depth,
                    max_depth,
                    break_length,
                    sorted,
                    max_string_length,
                    max_array_length,
                    colors,
                    show_hidden,
                    seen,
                )),
                InspectGetterKey::Symbol(sym, _) => items.push(inspect_symbol_property_entry(
                    rt,
                    id,
                    &sym,
                    true,
                    depth,
                    max_depth,
                    break_length,
                    sorted,
                    max_string_length,
                    max_array_length,
                    colors,
                    show_hidden,
                    seen,
                )),
            }
        }
    }
    let body = if pure_numeric_view && items.len() == numeric_element_count {

        inspect_join_numeric_array(&items, depth, break_length)
    } else {
        inspect_join_aggregate(
            aggregate_open,
            aggregate_close,
            &items,
            depth,
            if force_multiline { 0 } else { break_length },
        )
    };
    let rendered = format!("{prefix}{separator}{body}");
    Some(match inspect_ref_index(seen, id) {
        Some(ref_index) => inspect_ref_prefix(rendered, ref_index),
        None => rendered,
    })
}

fn inspect_selected_sorted_comparator_typed_view(
    rt: &mut Runtime,
    id: ObjectRef,
    comparator: &Value,
    depth: i32,
    max_depth: i32,
    break_length: usize,
    max_string_length: Option<usize>,
    max_array_length: Option<usize>,
    colors: bool,
    show_hidden: bool,
    seen: &mut Vec<ObjectRef>,
) -> Option<Result<String, RuntimeError>> {
    let view = rt.typed_array_views.get(&id)?.clone();
    let (prefix, mut items) = if &*view.element_kind == "DataView" {
        inspect_data_view_prefix_items(rt, id, depth, break_length, max_array_length)?
    } else {
        let (prefix, mut items) =
            inspect_typed_view_prefix_items(rt, id, depth, max_depth, break_length)?;
        let len = items.len();
        if show_hidden {
            items.push(format!("[BYTES_PER_ELEMENT]: {}", view.bytes_per_element));
            items.push(format!("[length]: {len}"));
            items.push(format!(
                "[byteLength]: {}",
                len.saturating_mul(view.bytes_per_element)
            ));
            items.push(format!("[byteOffset]: {}", view.byte_offset));
            items.push(format!(
                "[buffer]: ArrayBuffer {{ [byteLength]: {} }}",
                len.saturating_mul(view.bytes_per_element)
            ));
        }
        (prefix, items)
    };
    let is_data_view = prefix == "DataView";
    let aggregate_open = if is_data_view { "{" } else { "[" };
    let aggregate_close = if is_data_view { "}" } else { "]" };
    let separator = " ";
    let mut user_entries = Vec::new();
    for key in inspect_typed_array_visible_keys(rt, id, false) {
        match key {
            InspectGetterKey::String(key, _) => user_entries.push(inspect_property_entry(
                rt,
                id,
                &key,
                false,
                depth,
                max_depth,
                break_length,
                false,
                max_string_length,
                max_array_length,
                colors,
                show_hidden,
                seen,
            )),
            InspectGetterKey::Symbol(sym, _) => user_entries.push(inspect_symbol_property_entry(
                rt,
                id,
                &sym,
                false,
                depth,
                max_depth,
                break_length,
                false,
                max_string_length,
                max_array_length,
                colors,
                show_hidden,
                seen,
            )),
        }
    }
    if show_hidden {
        for key in inspect_typed_array_hidden_user_keys(rt, id, false) {
            match key {
                InspectGetterKey::String(key, _) => user_entries.push(inspect_property_entry(
                    rt,
                    id,
                    &key,
                    true,
                    depth,
                    max_depth,
                    break_length,
                    false,
                    max_string_length,
                    max_array_length,
                    colors,
                    show_hidden,
                    seen,
                )),
                InspectGetterKey::Symbol(sym, _) => {
                    user_entries.push(inspect_symbol_property_entry(
                        rt,
                        id,
                        &sym,
                        true,
                        depth,
                        max_depth,
                        break_length,
                        false,
                        max_string_length,
                        max_array_length,
                        colors,
                        show_hidden,
                        seen,
                    ))
                }
            }
        }
    }
    if is_data_view {
        items.extend(user_entries);
        if let Err(err) = sort_rendered_entries_with_comparator(rt, &mut items, comparator.clone())
        {
            return Some(Err(err));
        }
    } else {
        if let Err(err) =
            sort_rendered_entries_with_comparator(rt, &mut user_entries, comparator.clone())
        {
            return Some(Err(err));
        }
        items.extend(user_entries);
    }
    let force_multiline = is_data_view || show_hidden;
    let body = inspect_join_aggregate(
        aggregate_open,
        aggregate_close,
        &items,
        depth,
        if force_multiline { 0 } else { break_length },
    );
    Some(Ok(format!("{prefix}{separator}{body}")))
}

fn inspect_selected_getters_typed_view(
    rt: &mut Runtime,
    id: ObjectRef,
    depth: i32,
    max_depth: i32,
    break_length: usize,
    sorted: bool,
    max_string_length: Option<usize>,
    max_array_length: Option<usize>,
    colors: bool,
    show_hidden: bool,
    getters_mode: InspectGettersMode,
) -> Option<String> {
    let (prefix, mut items) = if let Some((prefix, mut items)) =
        inspect_typed_view_prefix_items(rt, id, depth, max_depth, break_length)
    {
        let len = items.len();
        let view = rt.typed_array_views.get(&id)?;
        if show_hidden {
            items.push(format!("[BYTES_PER_ELEMENT]: {}", view.bytes_per_element));
            items.push(format!("[length]: {len}"));
            items.push(format!(
                "[byteLength]: {}",
                len.saturating_mul(view.bytes_per_element)
            ));
            items.push(format!("[byteOffset]: {}", view.byte_offset));
            items.push(format!(
                "[buffer]: ArrayBuffer {{ [byteLength]: {} }}",
                len.saturating_mul(view.bytes_per_element)
            ));
        }
        (prefix, items)
    } else {
        inspect_data_view_prefix_items(rt, id, depth, break_length, max_array_length)?
    };
    let force_multiline = prefix == "DataView" || show_hidden;
    let aggregate_open = if prefix == "DataView" { "{" } else { "[" };
    let aggregate_close = if prefix == "DataView" { "}" } else { "]" };
    let mut seen = vec![id];
    for key in inspect_typed_array_visible_keys(rt, id, sorted)
        .into_iter()
        .chain(if show_hidden {
            inspect_typed_array_hidden_user_keys(rt, id, sorted)
        } else {
            Vec::new()
        })
    {
        items.push(inspect_selected_getter_property_entry(
            rt,
            id,
            key,
            depth,
            max_depth,
            break_length,
            sorted,
            max_string_length,
            max_array_length,
            colors,
            show_hidden,
            getters_mode,
            &mut seen,
        ));
    }
    let body = inspect_join_aggregate(
        aggregate_open,
        aggregate_close,
        &items,
        depth,
        if force_multiline { 0 } else { break_length },
    );
    Some(format!("{prefix} {body}"))
}

fn inspect_function_object(
    rt: &Runtime,
    id: ObjectRef,
    depth: i32,
    max_depth: i32,
    break_length: usize,
    sorted: bool,
    max_string_length: Option<usize>,
    max_array_length: Option<usize>,
    colors: bool,
    show_hidden: bool,
    seen: &mut Vec<ObjectRef>,
) -> String {

    let (is_class, is_generator, is_async) = match &rt.obj(id).internal_kind {
        rusty_js_runtime::value::InternalKind::Closure(ci) => (
            ci.proto.is_class_constructor,
            ci.proto.is_generator,
            ci.proto.is_async,
        ),
        _ => (false, false, false),
    };
    let base = if is_class {
        "class"
    } else {
        match (is_async, is_generator) {
            (true, true) => "AsyncGeneratorFunction",
            (true, false) => "AsyncFunction",
            (false, true) => "GeneratorFunction",
            (false, false) => "Function",
        }
    };
    let mut name = match rt.object_get(id, "name") {
        Value::String(s) if !s.is_empty() => {
            if is_class {
                format!(" {}", s.as_str())
            } else {
                format!(": {}", s.as_str())
            }
        }
        _ => " (anonymous)".to_string(),
    };

    if is_class {
        if let Some(sup) = rt.obj(id).proto {
            if let Value::String(s) = rt.object_get(sup, "name") {
                if !s.is_empty() {
                    name.push_str(&format!(" extends {}", s.as_str()));
                }
            }
        }
    }
    let has_circular_prototype = matches!(rt.object_get(id, "prototype"), Value::Object(proto) if matches!(rt.object_get(proto, "constructor"), Value::Object(constructor) if constructor == id));
    let prefix = if show_hidden && has_circular_prototype {
        format!("<ref *1> [{base}{}]", name)
    } else {
        format!("[{base}{}]", name)
    };
    let prefix = if colors {
        inspect_colorize_special(prefix, "36")
    } else {
        prefix
    };
    let mut entries = Vec::new();
    let mut has_non_metadata_hidden = false;
    if show_hidden {
        for key in ["length", "name", "prototype"] {
            if rt.obj(id).get_own(key).is_some() {
                entries.push(inspect_property_entry(
                    rt,
                    id,
                    key,
                    true,
                    depth,
                    max_depth,
                    break_length,
                    sorted,
                    max_string_length,
                    max_array_length,
                    colors,
                    show_hidden,
                    seen,
                ));
            }
        }
    }
    if show_hidden {
        let mut hidden_keys = inspect_hidden_own_string_keys(rt, id, 0, sorted);
        for key in hidden_keys.drain(..) {
            if matches!(key.as_str(), "length" | "name" | "prototype") {
                continue;
            }
            has_non_metadata_hidden = true;
            entries.push(inspect_property_entry(
                rt,
                id,
                &key,
                true,
                depth,
                max_depth,
                break_length,
                sorted,
                max_string_length,
                max_array_length,
                colors,
                show_hidden,
                seen,
            ));
        }
    }
    let mut keys = rt.ordinary_own_enumerable_string_keys(id);
    if sorted {
        keys.sort();
    }
    for key in keys {
        if matches!(key.as_str(), "length" | "name" | "prototype") {
            continue;
        }
        entries.push(inspect_property_entry(
            rt,
            id,
            &key,
            false,
            depth,
            max_depth,
            break_length,
            sorted,
            max_string_length,
            max_array_length,
            colors,
            show_hidden,
            seen,
        ));
    }
    for sym in inspect_own_symbol_keys(rt, id, true) {
        entries.push(inspect_symbol_property_entry(
            rt,
            id,
            &sym,
            false,
            depth,
            max_depth,
            break_length,
            sorted,
            max_string_length,
            max_array_length,
            colors,
            show_hidden,
            seen,
        ));
    }
    if show_hidden {
        for sym in inspect_own_symbol_keys(rt, id, false) {
            has_non_metadata_hidden = true;
            entries.push(inspect_symbol_property_entry(
                rt,
                id,
                &sym,
                true,
                depth,
                max_depth,
                break_length,
                sorted,
                max_string_length,
                max_array_length,
                colors,
                show_hidden,
                seen,
            ));
        }
    }
    if entries.is_empty() {
        return prefix;
    }
    let force_multiline = (show_hidden && (has_circular_prototype || has_non_metadata_hidden))
        || inspect_force_nested_group_multiline("{", "}", &entries, depth, max_depth, break_length);
    format!(
        "{} {}",
        prefix,
        inspect_join_aggregate(
            "{",
            "}",
            &entries,
            depth,
            if force_multiline { 0 } else { break_length },
        )
    )
}

fn inspect_value_with_options(
    rt: &Runtime,
    v: &Value,
    depth: i32,
    max_depth: i32,
    break_length: usize,
    sorted: bool,
    max_string_length: Option<usize>,
    max_array_length: Option<usize>,
    colors: bool,
    show_hidden: bool,
) -> String {
    let mut seen = Vec::new();
    inspect_value_inner(
        rt,
        v,
        depth,
        max_depth,
        break_length,
        sorted,
        max_string_length,
        max_array_length,
        colors,
        show_hidden,
        &mut seen,
    )
}

fn inspect_value_inner(
    rt: &Runtime,
    v: &Value,
    depth: i32,
    max_depth: i32,
    break_length: usize,
    sorted: bool,
    max_string_length: Option<usize>,
    max_array_length: Option<usize>,
    colors: bool,
    show_hidden: bool,
    seen: &mut Vec<ObjectRef>,
) -> String {
    inspect_mark_current_depth(v, depth, max_depth);
    match v {
        Value::Undefined => {
            let rendered = "undefined".to_string();
            if colors {
                inspect_colorize(rendered, v)
            } else {
                rendered
            }
        }
        Value::Null => {
            let rendered = "null".to_string();
            if colors {
                inspect_colorize(rendered, v)
            } else {
                rendered
            }
        }
        Value::Boolean(b) => {
            let rendered = if *b { "true" } else { "false" }.to_string();
            if colors {
                inspect_colorize(rendered, v)
            } else {
                rendered
            }
        }
        Value::Number(n) => {
            let rendered = if util_numeric_separator_default(rt) {
                add_numeric_separators(&inspect_num(*n))
            } else {
                inspect_num(*n)
            };
            if colors {
                inspect_colorize(rendered, v)
            } else {
                rendered
            }
        }
        Value::String(s) => {
            let rendered = inspect_jsstring_with_max(s, max_string_length);
            if colors {
                inspect_colorize(rendered, v)
            } else {
                rendered
            }
        }
        Value::BigInt(_) => {
            let rendered = format!("{}n", abstract_ops::to_string(v).as_str());
            let rendered = if util_numeric_separator_default(rt) {
                add_numeric_separators(&rendered)
            } else {
                rendered
            };
            if colors {
                inspect_colorize(rendered, v)
            } else {
                rendered
            }
        }
        Value::Symbol(_) => {
            let rendered = abstract_ops::to_string(v).as_str().to_string();
            if colors {
                inspect_colorize(rendered, v)
            } else {
                rendered
            }
        }
        Value::Object(id) => {

            let proxy_pair = match &rt.obj(*id).internal_kind {
                InternalKind::Proxy(p) => Some((p.target, p.handler)),
                _ => None,
            };
            if let Some((target, handler)) = proxy_pair {
                let show_proxy = INSPECT_SHOW_PROXY.with(|c| c.get());

                let target_str = inspect_value_inner(
                    rt,
                    &Value::Object(target),
                    if show_proxy { depth + 1 } else { depth },
                    max_depth,
                    break_length,
                    sorted,
                    max_string_length,
                    max_array_length,
                    colors,
                    show_hidden,
                    seen,
                );

                if show_proxy {
                    let handler_str = inspect_value_inner(
                        rt,
                        &Value::Object(handler),
                        depth + 1,
                        max_depth,
                        break_length,
                        sorted,
                        max_string_length,
                        max_array_length,
                        colors,
                        show_hidden,
                        seen,
                    );

                    return inspect_join_aggregate(
                        "Proxy [",
                        "]",
                        &[target_str, handler_str],
                        depth,
                        break_length,
                    );
                }
                return format!("Proxy({})", target_str);
            }
            if rt.obj(*id).has_own_str("__broadcast_channel") {
                if depth >= max_depth {
                    return "BroadcastChannel".to_string();
                }
                let name = match rt.object_get(*id, "name") {
                    Value::String(s) => s.as_str().to_string(),
                    _ => String::new(),
                };
                let active = !matches!(rt.object_get(*id, "active"), Value::Boolean(false));
                return format!(
                    "BroadcastChannel {{ name: {}, active: {} }}",
                    inspect_str(&name),
                    active
                );
            }
            if rt.obj(*id).has_own_str("__block_list") {
                if depth >= max_depth {
                    return "[BlockList]".to_string();
                }
                let rules = rt.object_get(*id, "rules");
                let rendered_rules = inspect_value_inner(
                    rt,
                    &rules,
                    depth + 1,
                    max_depth,
                    break_length,
                    sorted,
                    max_string_length,
                    max_array_length,
                    colors,
                    show_hidden,
                    seen,
                );
                return format!("BlockList {{ rules: {} }}", rendered_rules);
            }
            if rt.obj(*id).has_own_str("__blob_bytes") {
                if depth >= max_depth {
                    return "[Blob]".to_string();
                }
                let size = match rt.object_get(*id, "size") {
                    Value::Number(n) => n,
                    _ => 0.0,
                };
                let type_s = match rt.object_get(*id, "type") {
                    Value::String(s) => s.as_str().to_string(),
                    _ => String::new(),
                };
                return format!("Blob {{ size: {}, type: '{}' }}", size as i64, type_s);
            }
            if let Some(pos) = seen.iter().position(|seen_id| seen_id == id) {
                return format!("[Circular *{}]", pos + 1);
            }
            let ref_index = seen.len() + 1;
            seen.push(*id);
            let (is_fn, regexp_sf, is_array) = {
                let k = &rt.obj(*id).internal_kind;
                let re = if let InternalKind::RegExp(re) = k {
                    Some((
                        re.source.as_str().to_string(),
                        re.flags.as_str().to_string(),
                    ))
                } else {
                    None
                };
                (
                    matches!(
                        k,
                        InternalKind::Function(_)
                            | InternalKind::Closure(_)
                            | InternalKind::BoundFunction(_)
                    ),
                    re,
                    matches!(k, InternalKind::Array),
                )
            };
            if is_fn {
                let rendered = inspect_function_object(
                    rt,
                    *id,
                    depth,
                    max_depth,
                    break_length,
                    sorted,
                    max_string_length,
                    max_array_length,
                    colors,
                    show_hidden,
                    seen,
                );
                seen.pop();
                return rendered;
            }

            if let Some((src, fl)) = regexp_sf {
                let rendered = inspect_regexp_object(
                    rt,
                    *id,
                    &src,
                    &fl,
                    depth,
                    max_depth,
                    break_length,
                    sorted,
                    max_string_length,
                    max_array_length,
                    colors,
                    show_hidden,
                    seen,
                );
                let rendered = inspect_ref_prefix(rendered, ref_index);
                seen.pop();
                return rendered;
            }

            if rt.obj(*id).has_own_str("__date_ms") {
                if let Value::Number(ms) = rt.object_get(*id, "__date_ms") {
                    let rendered = inspect_date_object(
                        rt,
                        *id,
                        ms_to_iso(ms),
                        depth,
                        max_depth,
                        break_length,
                        sorted,
                        max_string_length,
                        max_array_length,
                        colors,
                        show_hidden,
                        seen,
                    );
                    seen.pop();
                    return rendered;
                }
            }
            if let Some(error) = inspect_error_projection(rt, *id, colors) {
                let ref_index = seen.len();
                let suffix = inspect_error_suffix(
                    rt,
                    *id,
                    depth,
                    max_depth,
                    break_length,
                    sorted,
                    max_string_length,
                    max_array_length,
                    colors,
                    show_hidden,
                    error.contains('\n'),
                    seen,
                );

                if depth > max_depth && !suffix.is_empty() {
                    seen.pop();
                    return format!("[{}]", util_ctor_name(rt, *id));
                }

                let error = indent_continuation_lines(&error, (depth.max(0) as usize) * 2);
                let rendered = format!("{}{}", error, suffix);
                let rendered = inspect_ref_prefix(rendered, ref_index);
                seen.pop();
                return rendered;
            }
            if let Some(rendered) = inspect_typed_view_object(
                rt,
                *id,
                depth,
                max_depth,
                break_length,
                sorted,
                max_string_length,
                max_array_length,
                colors,
                show_hidden,
                seen,
            ) {
                seen.pop();
                return rendered;
            }
            if rt.obj(*id).has_own_str("signal") {
                let signal = rt.object_get(*id, "signal");
                let rendered_signal = inspect_value_inner(
                    rt,
                    &signal,
                    depth + 1,
                    max_depth,
                    break_length,
                    sorted,
                    max_string_length,
                    max_array_length,
                    colors,
                    show_hidden,
                    seen,
                );
                let rendered = format!("AbortController {{ signal: {} }}", rendered_signal);
                seen.pop();
                return rendered;
            }
            if rt.obj(*id).has_own_str("aborted") && rt.obj(*id).has_own_str("reason") {
                if depth >= max_depth {
                    seen.pop();
                    return "[AbortSignal]".to_string();
                }
                let aborted = match rt.object_get(*id, "aborted") {
                    Value::Boolean(v) => v,
                    _ => false,
                };
                let rendered = format!("AbortSignal {{ aborted: {} }}", aborted);
                seen.pop();
                return rendered;
            }
            if let Some(record) = rt.array_buffers.get(id) {

                let bytes = record.to_bytes();
                let name = if record.shared.is_some() {
                    "SharedArrayBuffer"
                } else {
                    "ArrayBuffer"
                };
                let rendered = inspect_array_buffer_render(
                    &bytes,
                    name,
                    depth,
                    break_length,
                    max_array_length,
                );
                seen.pop();
                return rendered;
            }
            if matches!(rt.obj(*id).internal_kind, InternalKind::Promise(_)) {
                let rendered = inspect_promise_object(
                    rt,
                    *id,
                    depth,
                    max_depth,
                    break_length,
                    sorted,
                    max_string_length,
                    max_array_length,
                    colors,
                    show_hidden,
                    seen,
                );
                let rendered = inspect_ref_prefix(rendered, ref_index);
                seen.pop();
                return rendered;
            }
            if rt.obj(*id).has_own_str("__is_weakmap") {
                if depth > max_depth {
                    seen.pop();
                    return "[WeakMap]".to_string();
                }
                let rendered = inspect_weak_collection_object(
                    rt,
                    *id,
                    "WeakMap",
                    depth,
                    max_depth,
                    break_length,
                    sorted,
                    max_string_length,
                    max_array_length,
                    colors,
                    show_hidden,
                    seen,
                );
                let rendered = inspect_ref_prefix(rendered, ref_index);
                seen.pop();
                return rendered;
            }
            if rt.obj(*id).has_own_str("__is_weakset") {
                if depth > max_depth {
                    seen.pop();
                    return "[WeakSet]".to_string();
                }
                let rendered = inspect_weak_collection_object(
                    rt,
                    *id,
                    "WeakSet",
                    depth,
                    max_depth,
                    break_length,
                    sorted,
                    max_string_length,
                    max_array_length,
                    colors,
                    show_hidden,
                    seen,
                );
                let rendered = inspect_ref_prefix(rendered, ref_index);
                seen.pop();
                return rendered;
            }

            if rt.obj(*id).has_own_str("__map_data") {
                if let Value::Object(storage) = rt.object_get(*id, "__map_data") {
                    let orig = rt.object_get(*id, "__map_orig_keys");
                    let skeys = rt.ordinary_own_enumerable_string_keys(storage);
                    let suffix_keys =
                        inspect_map_set_suffix_getter_keys(rt, *id, show_hidden, sorted);
                    if skeys.is_empty() && suffix_keys.is_empty() {
                        seen.pop();
                        return "Map(0) {}".to_string();
                    }

                    if depth > max_depth {
                        seen.pop();
                        return "[Map]".to_string();
                    }
                    let mut entries: Vec<String> = skeys
                        .iter()
                        .map(|sk| {
                            let val = rt.object_get(storage, sk);
                            let keyv = match &orig {
                                Value::Object(o) if rt.obj(*o).has_own_str(sk) => {
                                    rt.object_get(*o, sk)
                                }
                                _ => Value::String(Rc::new(
                                    rusty_js_runtime::value::JsString::from(sk.clone()),
                                )),
                            };
                            format!(
                                "{} => {}",
                                inspect_value_inner(
                                    rt,
                                    &keyv,
                                    depth + 1,
                                    max_depth,
                                    break_length,
                                    sorted,
                                    max_string_length,
                                    max_array_length,
                                    colors,
                                    show_hidden,
                                    seen
                                ),
                                inspect_value_inner(
                                    rt,
                                    &val,
                                    depth + 1,
                                    max_depth,
                                    break_length,
                                    sorted,
                                    max_string_length,
                                    max_array_length,
                                    colors,
                                    show_hidden,
                                    seen
                                )
                            )
                        })
                        .collect();
                    for key in suffix_keys {
                        match key {
                            InspectGetterKey::String(key, hidden) => {
                                entries.push(inspect_property_entry(
                                    rt,
                                    *id,
                                    &key,
                                    hidden,
                                    depth,
                                    max_depth,
                                    break_length,
                                    sorted,
                                    max_string_length,
                                    max_array_length,
                                    colors,
                                    show_hidden,
                                    seen,
                                ));
                            }
                            InspectGetterKey::Symbol(sym, hidden) => {
                                entries.push(inspect_symbol_property_entry(
                                    rt,
                                    *id,
                                    &sym,
                                    hidden,
                                    depth,
                                    max_depth,
                                    break_length,
                                    sorted,
                                    max_string_length,
                                    max_array_length,
                                    colors,
                                    show_hidden,
                                    seen,
                                ));
                            }
                        }
                    }
                    let rendered = format!(
                        "Map({}) {}",
                        skeys.len(),
                        inspect_join_aggregate("{", "}", &entries, depth, break_length)
                    );
                    let rendered = inspect_ref_prefix(rendered, ref_index);
                    seen.pop();
                    return rendered;
                }
            }

            if rt.obj(*id).has_own_str("__set_data") {
                if let Value::Object(storage) = rt.object_get(*id, "__set_data") {
                    let skeys = rt.ordinary_own_enumerable_string_keys(storage);
                    let suffix_keys =
                        inspect_map_set_suffix_getter_keys(rt, *id, show_hidden, sorted);
                    if skeys.is_empty() && suffix_keys.is_empty() {
                        seen.pop();
                        return "Set(0) {}".to_string();
                    }

                    if depth > max_depth {
                        seen.pop();
                        return "[Set]".to_string();
                    }
                    let mut vals: Vec<String> = skeys
                        .iter()
                        .map(|sk| {
                            inspect_value_inner(
                                rt,
                                &rt.object_get(storage, sk),
                                depth + 1,
                                max_depth,
                                break_length,
                                sorted,
                                max_string_length,
                                max_array_length,
                                colors,
                                show_hidden,
                                seen,
                            )
                        })
                        .collect();
                    for key in suffix_keys {
                        match key {
                            InspectGetterKey::String(key, hidden) => {
                                vals.push(inspect_property_entry(
                                    rt,
                                    *id,
                                    &key,
                                    hidden,
                                    depth,
                                    max_depth,
                                    break_length,
                                    sorted,
                                    max_string_length,
                                    max_array_length,
                                    colors,
                                    show_hidden,
                                    seen,
                                ));
                            }
                            InspectGetterKey::Symbol(sym, hidden) => {
                                vals.push(inspect_symbol_property_entry(
                                    rt,
                                    *id,
                                    &sym,
                                    hidden,
                                    depth,
                                    max_depth,
                                    break_length,
                                    sorted,
                                    max_string_length,
                                    max_array_length,
                                    colors,
                                    show_hidden,
                                    seen,
                                ));
                            }
                        }
                    }
                    let rendered = format!(
                        "Set({}) {}",
                        skeys.len(),
                        inspect_join_aggregate("{", "}", &vals, depth, break_length)
                    );
                    let rendered = inspect_ref_prefix(rendered, ref_index);
                    seen.pop();
                    return rendered;
                }
            }
            if depth > max_depth && inspect_depth_limit_has_content(rt, *id, show_hidden) {
                let rendered = inspect_depth_limit_label(rt, *id, is_array);
                seen.pop();
                return rendered;
            }
            if !is_array {
                if let InternalKind::NumberWrapper(Value::Number(n)) = &rt.obj(*id).internal_kind {
                    let rendered = inspect_boxed_primitive_object(
                        rt,
                        *id,
                        format!("[Number: {}]", inspect_num(*n)),
                        "33",
                        depth,
                        max_depth,
                        break_length,
                        sorted,
                        max_string_length,
                        max_array_length,
                        colors,
                        show_hidden,
                        seen,
                    );
                    seen.pop();
                    return rendered;
                }
                if let InternalKind::BooleanWrapper(Value::Boolean(b)) = &rt.obj(*id).internal_kind
                {
                    let rendered = inspect_boxed_primitive_object(
                        rt,
                        *id,
                        format!("[Boolean: {}]", b),
                        "33",
                        depth,
                        max_depth,
                        break_length,
                        sorted,
                        max_string_length,
                        max_array_length,
                        colors,
                        show_hidden,
                        seen,
                    );
                    seen.pop();
                    return rendered;
                }
                if let InternalKind::BigIntWrapper(Value::BigInt(b)) = &rt.obj(*id).internal_kind {
                    let rendered = inspect_boxed_primitive_object(
                        rt,
                        *id,
                        format!("[BigInt: {}n]", b.to_decimal()),
                        "33",
                        depth,
                        max_depth,
                        break_length,
                        sorted,
                        max_string_length,
                        max_array_length,
                        colors,
                        show_hidden,
                        seen,
                    );
                    seen.pop();
                    return rendered;
                }
                if let Some(payload) = inspect_boxed_string_payload(rt, *id) {
                    let rendered = inspect_boxed_string_object(
                        rt,
                        *id,
                        &payload,
                        depth,
                        max_depth,
                        break_length,
                        sorted,
                        max_string_length,
                        max_array_length,
                        colors,
                        show_hidden,
                        seen,
                    );
                    seen.pop();
                    return rendered;
                }
            }
            if is_array {
                let len = match rt.object_get(*id, "length") {
                    Value::Number(n) => n as usize,
                    _ => 0,
                };

                let mut items: Vec<String> = Vec::new();
                let mut hole_run = 0usize;
                let flush_holes = |items: &mut Vec<String>, run: &mut usize| {
                    if *run > 0 {
                        items.push(format!(
                            "<{} empty item{}>",
                            run,
                            if *run == 1 { "" } else { "s" }
                        ));
                        *run = 0;
                    }
                };
                let display_len = max_array_length.unwrap_or(len).min(len);
                for i in 0..display_len {
                    let key = i.to_string();
                    if rt.obj(*id).has_own_str(&key) {
                        flush_holes(&mut items, &mut hole_run);
                        items.push(inspect_value_inner(
                            rt,
                            &rt.object_get(*id, &key),
                            depth + 1,
                            max_depth,
                            break_length,
                            sorted,
                            max_string_length,
                            max_array_length,
                            colors,
                            show_hidden,
                            seen,
                        ));
                    } else {
                        hole_run += 1;
                    }
                }
                flush_holes(&mut items, &mut hole_run);
                if display_len < len {
                    let remaining = len - display_len;
                    items.push(format!(
                        "... {} more item{}",
                        remaining,
                        if remaining == 1 { "" } else { "s" }
                    ));
                }
                if show_hidden {
                    items.push(format!("[length]: {}", len));
                }
                for k in rt.ordinary_own_enumerable_string_keys(*id) {
                    if k == "length" || k.parse::<usize>().is_ok_and(|idx| idx < len) {
                        continue;
                    }
                    let kd = if is_ident_key(&k) {
                        k.clone()
                    } else {
                        inspect_str(&k)
                    };
                    let val = rt.object_get(*id, &k);
                    items.push(format!(
                        "{}: {}",
                        kd,
                        inspect_value_inner(
                            rt,
                            &val,
                            depth + 1,
                            max_depth,
                            break_length,
                            sorted,
                            max_string_length,
                            max_array_length,
                            colors,
                            show_hidden,
                            seen,
                        )
                    ));
                }
                for sym in inspect_own_symbol_keys(rt, *id, true) {
                    items.push(inspect_symbol_property_entry(
                        rt,
                        *id,
                        &sym,
                        false,
                        depth,
                        max_depth,
                        break_length,
                        sorted,
                        max_string_length,
                        max_array_length,
                        colors,
                        show_hidden,
                        seen,
                    ));
                }
                if show_hidden {
                    let mut hidden_keys = inspect_hidden_own_string_keys(rt, *id, len, sorted);
                    for k in hidden_keys.drain(..) {
                        let val = rt.object_get(*id, &k);
                        items.push(format!(
                            "{}: {}",
                            inspect_hidden_key(&k),
                            inspect_value_inner(
                                rt,
                                &val,
                                depth + 1,
                                max_depth,
                                break_length,
                                sorted,
                                max_string_length,
                                max_array_length,
                                colors,
                                show_hidden,
                                seen,
                            )
                        ));
                    }
                    for sym in inspect_own_symbol_keys(rt, *id, false) {
                        items.push(inspect_symbol_property_entry(
                            rt,
                            *id,
                            &sym,
                            true,
                            depth,
                            max_depth,
                            break_length,
                            sorted,
                            max_string_length,
                            max_array_length,
                            colors,
                            show_hidden,
                            seen,
                        ));
                    }
                }
                let force_multiline = inspect_force_nested_group_multiline(
                    "[",
                    "]",
                    &items,
                    depth,
                    max_depth,
                    break_length,
                );
                let body = inspect_array_grid_body(&items, depth, break_length, force_multiline);
                let rendered =
                    inspect_ref_prefix(inspect_array_wrapper(rt, *id, len, body), ref_index);
                seen.pop();
                rendered
            } else {
                let mut keys = rt.ordinary_own_enumerable_string_keys(*id);
                if sorted {
                    keys.sort();
                }
                let symbol_keys = inspect_own_symbol_keys(rt, *id, true);
                let mut hidden_keys = if show_hidden {
                    inspect_hidden_own_string_keys(rt, *id, 0, sorted)
                } else {
                    Vec::new()
                };
                let hidden_symbol_keys = if show_hidden {
                    inspect_own_symbol_keys(rt, *id, false)
                } else {
                    Vec::new()
                };
                if keys.is_empty()
                    && symbol_keys.is_empty()
                    && hidden_keys.is_empty()
                    && hidden_symbol_keys.is_empty()
                {
                    let rendered = inspect_object_wrapper(rt, *id, "{}".to_string());
                    seen.pop();
                    return rendered;
                }
                let mut entries: Vec<String> = keys
                    .iter()
                    .map(|k| {
                        let kd = if is_ident_key(k) {
                            k.clone()
                        } else {
                            inspect_str(k)
                        };
                        let rendered = rt
                            .obj(*id)
                            .get_own(k)
                            .and_then(|d| {
                                inspect_accessor_placeholder(d.getter.is_some(), d.setter.is_some())
                            })
                            .map(str::to_string)
                            .unwrap_or_else(|| {
                                let val = rt.object_get(*id, k);
                                inspect_value_inner(
                                    rt,
                                    &val,
                                    depth + 1,
                                    max_depth,
                                    break_length,
                                    sorted,
                                    max_string_length,
                                    max_array_length,
                                    colors,
                                    show_hidden,
                                    seen,
                                )
                            });
                        format!("{}: {}", kd, rendered)
                    })
                    .collect();
                for sym in symbol_keys {
                    entries.push(inspect_symbol_property_entry(
                        rt,
                        *id,
                        &sym,
                        false,
                        depth,
                        max_depth,
                        break_length,
                        sorted,
                        max_string_length,
                        max_array_length,
                        colors,
                        show_hidden,
                        seen,
                    ));
                }
                entries.extend(hidden_keys.drain(..).map(|k| {
                    let rendered = rt
                        .obj(*id)
                        .get_own(&k)
                        .and_then(|d| {
                            inspect_accessor_placeholder(d.getter.is_some(), d.setter.is_some())
                        })
                        .map(str::to_string)
                        .unwrap_or_else(|| {
                            let val = rt.object_get(*id, &k);
                            inspect_value_inner(
                                rt,
                                &val,
                                depth + 1,
                                max_depth,
                                break_length,
                                sorted,
                                max_string_length,
                                max_array_length,
                                colors,
                                show_hidden,
                                seen,
                            )
                        });
                    format!("{}: {}", inspect_hidden_key(&k), rendered)
                }));
                for sym in hidden_symbol_keys {
                    entries.push(inspect_symbol_property_entry(
                        rt,
                        *id,
                        &sym,
                        true,
                        depth,
                        max_depth,
                        break_length,
                        sorted,
                        max_string_length,
                        max_array_length,
                        colors,
                        show_hidden,
                        seen,
                    ));
                }
                let force_multiline = inspect_force_nested_group_multiline(
                    "{",
                    "}",
                    &entries,
                    depth,
                    max_depth,
                    break_length,
                );
                let body = inspect_join_aggregate(
                    "{",
                    "}",
                    &entries,
                    depth,
                    if force_multiline { 0 } else { break_length },
                );
                let rendered = inspect_ref_prefix(inspect_object_wrapper(rt, *id, body), ref_index);
                seen.pop();
                rendered
            }
        }
    }
}

pub fn install(rt: &mut Runtime) {
    let util = new_object(rt);

    register_method(rt, util, "styleText", |rt, args| {

        fn codes(fmt: &str) -> Option<(u8, u8)> {
            Some(match fmt {
                "reset" => (0, 0),
                "bold" => (1, 22),
                "dim" => (2, 22),
                "italic" => (3, 23),
                "underline" => (4, 24),
                "blink" => (5, 25),
                "inverse" => (7, 27),
                "hidden" => (8, 28),
                "strikethrough" => (9, 29),
                "doubleunderline" => (21, 24),
                "black" => (30, 39),
                "red" => (31, 39),
                "green" => (32, 39),
                "yellow" => (33, 39),
                "blue" => (34, 39),
                "magenta" => (35, 39),
                "cyan" => (36, 39),
                "white" => (37, 39),
                "gray" | "grey" => (90, 39),
                "redBright" => (91, 39),
                "greenBright" => (92, 39),
                "yellowBright" => (93, 39),
                "blueBright" => (94, 39),
                "magentaBright" => (95, 39),
                "cyanBright" => (96, 39),
                "whiteBright" => (97, 39),
                "bgBlack" => (40, 49),
                "bgRed" => (41, 49),
                "bgGreen" => (42, 49),
                "bgYellow" => (43, 49),
                "bgBlue" => (44, 49),
                "bgMagenta" => (45, 49),
                "bgCyan" => (46, 49),
                "bgWhite" => (47, 49),
                _ => return None,
            })
        }
        let text = match args.get(1) {
            Some(Value::String(s)) => s.as_str().to_string(),
            Some(other) => rusty_js_runtime::abstract_ops::to_string(other)
                .as_str()
                .to_string(),
            None => String::new(),
        };

        let mut formats: Vec<String> = Vec::new();
        match args.first() {
            Some(Value::String(s)) => formats.push(s.as_str().to_string()),
            Some(Value::Object(id)) => {
                let len = match rt.object_get(*id, "length") {
                    Value::Number(n) if n >= 0.0 => n as usize,
                    _ => 0,
                };
                for i in 0..len {
                    if let Value::String(s) = rt.object_get(*id, &i.to_string()) {
                        formats.push(s.as_str().to_string());
                    }
                }
            }
            _ => {}
        }

        let opts = args.get(2);
        let validate = match opts {
            Some(Value::Object(id)) => {
                !matches!(rt.object_get(*id, "validateStream"), Value::Boolean(false))
            }
            _ => true,
        };
        let apply = if !validate {
            true
        } else {
            let stream = match opts {
                Some(Value::Object(id)) => match rt.object_get(*id, "stream") {
                    Value::Object(s) => Some(s),
                    _ => None,
                },
                _ => None,
            }
            .or_else(|| match rt.global_get("process") {
                Value::Object(p) => match rt.object_get(p, "stdout") {
                    Value::Object(s) => Some(s),
                    _ => None,
                },
                _ => None,
            });
            matches!(
                stream.map(|s| rt.object_get(s, "isTTY")),
                Some(Value::Boolean(true))
            )
        };
        if !apply || formats.is_empty() {
            return Ok(Value::String(Rc::new(
                rusty_js_runtime::value::JsString::from(text),
            )));
        }

        let resolved: Vec<(u8, u8)> = formats.iter().filter_map(|f| codes(f)).collect();
        let mut out = String::new();
        for (open, _) in &resolved {
            out.push_str(&format!("\x1b[{open}m"));
        }
        out.push_str(&text);
        for (_, close) in resolved.iter().rev() {
            out.push_str(&format!("\x1b[{close}m"));
        }
        Ok(Value::String(Rc::new(
            rusty_js_runtime::value::JsString::from(out),
        )))
    });
    register_method(rt, util, "debuglog", |rt, _args| {

        let f =
            crate::register::make_callable(rt, "debuglog_fn", |_rt, _args| Ok(Value::Undefined));
        rt.object_set(f, "enabled".into(), Value::Boolean(false));
        Ok(Value::Object(f))
    });
    register_method(rt, util, "debug", |rt, _args| {
        Ok(Value::Object(crate::register::make_callable(
            rt,
            "debug_fn",
            |_rt, _args| Ok(Value::Undefined),
        )))
    });
    register_util_deprecate(rt, util);

    let inspect_fn = crate::register::make_callable(rt, "inspect", |rt, args| {
        let v = args.first().cloned().unwrap_or(Value::Undefined);

        let max_depth = match args.get(1) {
            Some(Value::Object(opts)) => match rt.object_get(*opts, "depth") {
                Value::Null => i32::MAX,
                Value::Number(n) if n.is_finite() => n as i32,
                Value::Number(_) => i32::MAX,

                Value::Undefined if rt.obj(*opts).has_own_str("depth") => i32::MAX,
                _ => 2,
            },
            _ => 2,
        };
        let mut break_length = match args.get(1) {
            Some(Value::Object(opts)) => match rt.object_get(*opts, "breakLength") {
                Value::Number(n) if n.is_finite() && n >= 0.0 => n as usize,
                _ => DEFAULT_BREAK_LENGTH,
            },
            _ => DEFAULT_BREAK_LENGTH,
        };
        let sorted = match args.get(1) {
            Some(Value::Object(opts)) => abstract_ops::to_boolean(&rt.object_get(*opts, "sorted")),
            _ => false,
        };
        let show_proxy = match args.get(1) {
            Some(Value::Object(opts)) => {
                abstract_ops::to_boolean(&rt.object_get(*opts, "showProxy"))
            }
            _ => false,
        };

        let _show_proxy_guard = ShowProxyGuard::set(show_proxy);
        let max_string_length = match args.get(1) {
            Some(Value::Object(opts)) => match rt.object_get(*opts, "maxStringLength") {
                Value::Number(n) if n.is_finite() && n >= 0.0 => Some(n as usize),
                _ => None,
            },
            _ => None,
        };
        let max_array_length = match args.get(1) {
            Some(Value::Object(opts)) => match rt.object_get(*opts, "maxArrayLength") {
                Value::Number(n) if n.is_finite() && n >= 0.0 => Some(n as usize),
                Value::Null | Value::Number(_) => None,
                _ => Some(100),
            },
            _ => Some(100),
        };
        let colors = match args.get(1) {
            Some(Value::Object(opts)) => abstract_ops::to_boolean(&rt.object_get(*opts, "colors")),
            _ => false,
        };
        let show_hidden = match args.get(1) {
            Some(Value::Object(opts)) => {
                abstract_ops::to_boolean(&rt.object_get(*opts, "showHidden"))
            }
            Some(Value::Boolean(b)) => *b,
            _ => false,
        };
        let getters = match args.get(1) {
            Some(Value::Object(opts)) => inspect_getters_option_mode(rt, *opts),
            _ => None,
        };
        if matches!(
            args.get(1),
            Some(Value::Object(opts))
                if matches!(rt.object_get(*opts, "compact"), Value::Boolean(false))
        ) {
            break_length = 0;
        }
        let default_options = match rt.global_get("util") {
            Value::Object(util_id) => match rt.object_get(util_id, "inspect") {
                Value::Object(inspect_id) => match rt.object_get(inspect_id, "defaultOptions") {
                    Value::Object(defaults_id) => Some(defaults_id),
                    _ => None,
                },
                _ => None,
            },
            _ => None,
        };
        let old_numeric_separator = default_options
            .map(|id| abstract_ops::to_boolean(&rt.object_get(id, "numericSeparator")))
            .unwrap_or(false);
        if let (Some(defaults_id), Some(Value::Object(_))) = (default_options, args.get(1)) {
            let numeric_separator = util_numeric_separator_option(rt, args.get(1));
            rt.object_set(
                defaults_id,
                "numericSeparator".into(),
                Value::Boolean(numeric_separator),
            );
        }
        let custom_inspect_enabled = match args.get(1) {
            Some(Value::Object(opts)) => {
                !matches!(rt.object_get(*opts, "customInspect"), Value::Boolean(false))
            }
            _ => true,
        };
        if custom_inspect_enabled {
            if let Value::Object(id) = v {
                if let Some(sym) = util_inspect_custom_symbol(rt) {
                    let custom = rt
                        .obj(id)
                        .get_own_symbol(&sym)
                        .map(|desc| desc.value.clone())
                        .unwrap_or(Value::Undefined);
                    if rt.is_callable(&custom) {
                        let options_arg = util_inspect_options_arg(rt, args.get(1));
                        let inspect_arg = util_inspect_function_value(rt);
                        let result = rt.call_function(
                            custom,
                            Value::Object(id),
                            vec![Value::Number(max_depth as f64), options_arg, inspect_arg],
                        );
                        let result = match result {
                            Ok(result) => result,
                            Err(err) => {
                                if let Some(defaults_id) = default_options {
                                    rt.object_set(
                                        defaults_id,
                                        "numericSeparator".into(),
                                        Value::Boolean(old_numeric_separator),
                                    );
                                }
                                return Err(err);
                            }
                        };
                        let rendered = render_custom_inspect_result(
                            rt,
                            &result,
                            max_depth,
                            break_length,
                            sorted,
                            max_string_length,
                            max_array_length,
                            colors,
                            show_hidden,
                        );
                        if let Some(defaults_id) = default_options {
                            rt.object_set(
                                defaults_id,
                                "numericSeparator".into(),
                                Value::Boolean(old_numeric_separator),
                            );
                        }
                        return Ok(Value::String(Rc::new(
                            rusty_js_runtime::value::JsString::from(rendered),
                        )));
                    }
                }
            }
        }
        let selected_sorted_comparator = selected_sorted_comparator_value(rt, args.get(1));
        if custom_inspect_enabled && selected_sorted_comparator.is_none() {
            if let Value::Object(_) = v {
                if let Some(sym) = util_inspect_custom_symbol(rt) {
                    let mut contains_seen = Vec::new();
                    if value_contains_custom_inspect(rt, &v, &sym, &mut contains_seen) {
                        let mut seen = Vec::new();
                        match inspect_value_with_recursive_custom(
                            rt,
                            &v,
                            0,
                            max_depth,
                            break_length,
                            sorted,
                            max_string_length,
                            max_array_length,
                            colors,
                            show_hidden,
                            args.get(1).cloned(),
                            &mut seen,
                        ) {
                            Ok(rendered) => {
                                if let Some(defaults_id) = default_options {
                                    rt.object_set(
                                        defaults_id,
                                        "numericSeparator".into(),
                                        Value::Boolean(old_numeric_separator),
                                    );
                                }
                                return Ok(Value::String(Rc::new(
                                    rusty_js_runtime::value::JsString::from(rendered),
                                )));
                            }
                            Err(err) => {
                                if let Some(defaults_id) = default_options {
                                    rt.object_set(
                                        defaults_id,
                                        "numericSeparator".into(),
                                        Value::Boolean(old_numeric_separator),
                                    );
                                }
                                return Err(err);
                            }
                        }
                    }
                }
            }
        }
        if let (Value::Object(id), Some(comparator)) = (v.clone(), selected_sorted_comparator) {
            match inspect_selected_sorted_comparator_object(
                rt,
                id,
                comparator,
                max_depth,
                break_length,
                max_string_length,
                max_array_length,
                colors,
                show_hidden,
            ) {
                Ok(Some(rendered)) => {
                    if let Some(defaults_id) = default_options {
                        rt.object_set(
                            defaults_id,
                            "numericSeparator".into(),
                            Value::Boolean(old_numeric_separator),
                        );
                    }
                    return Ok(Value::String(Rc::new(
                        rusty_js_runtime::value::JsString::from(rendered),
                    )));
                }
                Ok(None) => {}
                Err(err) => {
                    if let Some(defaults_id) = default_options {
                        rt.object_set(
                            defaults_id,
                            "numericSeparator".into(),
                            Value::Boolean(old_numeric_separator),
                        );
                    }
                    return Err(err);
                }
            }
        }
        if let Some(getters_mode) = getters {
            if let Value::Object(id) = v {
                if let Some(rendered) = inspect_selected_getters_function(
                    rt,
                    id,
                    0,
                    max_depth,
                    break_length,
                    sorted,
                    max_string_length,
                    max_array_length,
                    colors,
                    show_hidden,
                    getters_mode,
                ) {
                    if let Some(defaults_id) = default_options {
                        rt.object_set(
                            defaults_id,
                            "numericSeparator".into(),
                            Value::Boolean(old_numeric_separator),
                        );
                    }
                    return Ok(Value::String(Rc::new(
                        rusty_js_runtime::value::JsString::from(rendered),
                    )));
                }
                if let Some(rendered) = inspect_selected_getters_array(
                    rt,
                    id,
                    0,
                    max_depth,
                    break_length,
                    sorted,
                    max_string_length,
                    max_array_length,
                    colors,
                    show_hidden,
                    getters_mode,
                ) {
                    if let Some(defaults_id) = default_options {
                        rt.object_set(
                            defaults_id,
                            "numericSeparator".into(),
                            Value::Boolean(old_numeric_separator),
                        );
                    }
                    return Ok(Value::String(Rc::new(
                        rusty_js_runtime::value::JsString::from(rendered),
                    )));
                }
                if let Some(rendered) = inspect_selected_getters_typed_view(
                    rt,
                    id,
                    0,
                    max_depth,
                    break_length,
                    sorted,
                    max_string_length,
                    max_array_length,
                    colors,
                    show_hidden,
                    getters_mode,
                ) {
                    if let Some(defaults_id) = default_options {
                        rt.object_set(
                            defaults_id,
                            "numericSeparator".into(),
                            Value::Boolean(old_numeric_separator),
                        );
                    }
                    return Ok(Value::String(Rc::new(
                        rusty_js_runtime::value::JsString::from(rendered),
                    )));
                }
                let regexp_source_flags = match &rt.obj(id).internal_kind {
                    InternalKind::RegExp(re) => Some((
                        re.source.as_str().to_string(),
                        re.flags.as_str().to_string(),
                    )),
                    _ => None,
                };
                if let Some((src, flags)) = regexp_source_flags {
                    let mut seen = vec![id];
                    let rendered = inspect_regexp_object_selected_getters(
                        rt,
                        id,
                        &src,
                        &flags,
                        0,
                        max_depth,
                        break_length,
                        sorted,
                        max_string_length,
                        max_array_length,
                        colors,
                        show_hidden,
                        getters_mode,
                        &mut seen,
                    );
                    if let Some(defaults_id) = default_options {
                        rt.object_set(
                            defaults_id,
                            "numericSeparator".into(),
                            Value::Boolean(old_numeric_separator),
                        );
                    }
                    return Ok(Value::String(Rc::new(
                        rusty_js_runtime::value::JsString::from(rendered),
                    )));
                }
                if rt.obj(id).has_own_str("__is_weakmap") || rt.obj(id).has_own_str("__is_weakset")
                {
                    let label = if rt.obj(id).has_own_str("__is_weakmap") {
                        Some("WeakMap")
                    } else if rt.obj(id).has_own_str("__is_weakset") {
                        Some("WeakSet")
                    } else {
                        None
                    };
                    if let Some(label) = label {
                        let mut seen = vec![id];
                        let rendered = inspect_weak_collection_object_selected_getters(
                            rt,
                            id,
                            label,
                            0,
                            max_depth,
                            break_length,
                            sorted,
                            max_string_length,
                            max_array_length,
                            colors,
                            show_hidden,
                            getters_mode,
                            &mut seen,
                        );
                        if let Some(defaults_id) = default_options {
                            rt.object_set(
                                defaults_id,
                                "numericSeparator".into(),
                                Value::Boolean(old_numeric_separator),
                            );
                        }
                        return Ok(Value::String(Rc::new(
                            rusty_js_runtime::value::JsString::from(rendered),
                        )));
                    }
                }
                if matches!(rt.obj(id).internal_kind, InternalKind::Promise(_)) {
                    let mut seen = vec![id];
                    let rendered = inspect_promise_object_selected_getters(
                        rt,
                        id,
                        0,
                        max_depth,
                        break_length,
                        sorted,
                        max_string_length,
                        max_array_length,
                        colors,
                        show_hidden,
                        getters_mode,
                        &mut seen,
                    );
                    if let Some(defaults_id) = default_options {
                        rt.object_set(
                            defaults_id,
                            "numericSeparator".into(),
                            Value::Boolean(old_numeric_separator),
                        );
                    }
                    return Ok(Value::String(Rc::new(
                        rusty_js_runtime::value::JsString::from(rendered),
                    )));
                }
                if let Some(rendered) = inspect_selected_getters_map_set(
                    rt,
                    id,
                    0,
                    max_depth,
                    break_length,
                    sorted,
                    max_string_length,
                    max_array_length,
                    colors,
                    show_hidden,
                    getters_mode,
                ) {
                    if let Some(defaults_id) = default_options {
                        rt.object_set(
                            defaults_id,
                            "numericSeparator".into(),
                            Value::Boolean(old_numeric_separator),
                        );
                    }
                    return Ok(Value::String(Rc::new(
                        rusty_js_runtime::value::JsString::from(rendered),
                    )));
                }
                let mut selected_seen = Vec::new();
                if let Some(rendered) = inspect_selected_getters_object(
                    rt,
                    id,
                    0,
                    max_depth,
                    break_length,
                    sorted,
                    max_string_length,
                    max_array_length,
                    colors,
                    show_hidden,
                    getters_mode,
                    &mut selected_seen,
                ) {
                    if let Some(defaults_id) = default_options {
                        rt.object_set(
                            defaults_id,
                            "numericSeparator".into(),
                            Value::Boolean(old_numeric_separator),
                        );
                    }
                    return Ok(Value::String(Rc::new(
                        rusty_js_runtime::value::JsString::from(rendered),
                    )));
                }
                if matches!(rt.obj(id).internal_kind, InternalKind::Error) {
                    if let Some(error) = inspect_error_projection(rt, id, colors) {
                        let mut seen = vec![id];
                        let rendered = format!(
                            "{}{}",
                            error,
                            inspect_error_suffix_selected_getters(
                                rt,
                                id,
                                0,
                                max_depth,
                                break_length,
                                sorted,
                                max_string_length,
                                max_array_length,
                                colors,
                                show_hidden,
                                Some(getters_mode),
                                error.contains('\n'),
                                &mut seen,
                            )
                        );
                        if let Some(defaults_id) = default_options {
                            rt.object_set(
                                defaults_id,
                                "numericSeparator".into(),
                                Value::Boolean(old_numeric_separator),
                            );
                        }
                        return Ok(Value::String(Rc::new(
                            rusty_js_runtime::value::JsString::from(rendered),
                        )));
                    }
                }
            }
        }
        let rendered = inspect_value_with_options(
            rt,
            &v,
            0,
            max_depth,
            break_length,
            sorted,
            max_string_length,
            max_array_length,
            colors,
            show_hidden,
        );
        if let Some(defaults_id) = default_options {
            rt.object_set(
                defaults_id,
                "numericSeparator".into(),
                Value::Boolean(old_numeric_separator),
            );
        }
        Ok(Value::String(Rc::new(
            rusty_js_runtime::value::JsString::from(rendered),
        )))
    });
    let inspect_custom_symbol = rt
        .symbol_for_via(&[Value::String(Rc::new(
            rusty_js_runtime::value::JsString::from("nodejs.util.inspect.custom"),
        ))])
        .unwrap_or_else(|_| Value::Symbol(Rc::new("@@sym:nodejs.util.inspect.custom".to_string())));
    rt.object_set(inspect_fn, "custom".into(), inspect_custom_symbol);

    let colors_obj = crate::register::new_object(rt);
    let palette: &[(&str, i32, i32)] = &[
        ("reset", 0, 0),
        ("bold", 1, 22),
        ("dim", 2, 22),
        ("italic", 3, 23),
        ("underline", 4, 24),
        ("blink", 5, 25),
        ("inverse", 7, 27),
        ("hidden", 8, 28),
        ("strikethrough", 9, 29),
        ("doubleunderline", 21, 24),
        ("overlined", 53, 55),
        ("framed", 51, 54),
        ("black", 30, 39),
        ("red", 31, 39),
        ("green", 32, 39),
        ("yellow", 33, 39),
        ("blue", 34, 39),
        ("magenta", 35, 39),
        ("cyan", 36, 39),
        ("white", 37, 39),
        ("gray", 90, 39),
        ("redBright", 91, 39),
        ("greenBright", 92, 39),
        ("yellowBright", 93, 39),
        ("blueBright", 94, 39),
        ("magentaBright", 95, 39),
        ("cyanBright", 96, 39),
        ("whiteBright", 97, 39),
        ("bgBlack", 40, 49),
        ("bgRed", 41, 49),
        ("bgGreen", 42, 49),
        ("bgYellow", 43, 49),
        ("bgBlue", 44, 49),
        ("bgMagenta", 45, 49),
        ("bgCyan", 46, 49),
        ("bgWhite", 47, 49),
        ("bgGray", 100, 49),
        ("bgRedBright", 101, 49),
        ("bgGreenBright", 102, 49),
        ("bgYellowBright", 103, 49),
        ("bgBlueBright", 104, 49),
        ("bgMagentaBright", 105, 49),
        ("bgCyanBright", 106, 49),
        ("bgWhiteBright", 107, 49),
    ];
    for (name, open, close) in palette {

        let pair_obj = rt.alloc_object(rusty_js_runtime::value::Object::new_array());
        rt.object_set(pair_obj, "0".into(), Value::Number(*open as f64));
        rt.object_set(pair_obj, "1".into(), Value::Number(*close as f64));
        rt.object_set(colors_obj, name.to_string(), Value::Object(pair_obj));
    }

    for (alias, target) in [
        ("grey", "gray"),
        ("blackBright", "gray"),
        ("bgGrey", "bgGray"),
        ("bgBlackBright", "bgGray"),
    ] {
        let arr = rt.object_get(colors_obj, target);
        rt.define_data_property_attrs(colors_obj, alias.into(), arr, false, false, true);
    }
    rt.object_set(inspect_fn, "colors".into(), Value::Object(colors_obj));

    let styles_obj = crate::register::new_object(rt);
    let styles: &[(&str, &str)] = &[
        ("special", "cyan"),
        ("number", "yellow"),
        ("bigint", "yellow"),
        ("boolean", "yellow"),
        ("undefined", "grey"),
        ("null", "bold"),
        ("string", "green"),
        ("symbol", "green"),
        ("date", "magenta"),
        ("regexp", "red"),
        ("module", "underline"),
    ];
    for (name, color) in styles {
        rt.object_set(
            styles_obj,
            name.to_string(),
            Value::String(Rc::new(rusty_js_runtime::value::JsString::from(*color))),
        );
    }
    rt.object_set(inspect_fn, "styles".into(), Value::Object(styles_obj));

    let default_options_obj = crate::register::new_object(rt);
    rt.object_set(
        default_options_obj,
        "showHidden".into(),
        Value::Boolean(false),
    );
    rt.object_set(default_options_obj, "depth".into(), Value::Number(2.0));
    rt.object_set(default_options_obj, "colors".into(), Value::Boolean(false));
    rt.object_set(
        default_options_obj,
        "customInspect".into(),
        Value::Boolean(true),
    );
    rt.object_set(
        default_options_obj,
        "showProxy".into(),
        Value::Boolean(false),
    );
    rt.object_set(
        default_options_obj,
        "maxArrayLength".into(),
        Value::Number(100.0),
    );
    rt.object_set(
        default_options_obj,
        "maxStringLength".into(),
        Value::Number(10000.0),
    );
    rt.object_set(
        default_options_obj,
        "breakLength".into(),
        Value::Number(80.0),
    );
    rt.object_set(default_options_obj, "compact".into(), Value::Number(3.0));
    rt.object_set(default_options_obj, "sorted".into(), Value::Boolean(false));
    rt.object_set(default_options_obj, "getters".into(), Value::Boolean(false));
    rt.object_set(
        default_options_obj,
        "numericSeparator".into(),
        Value::Boolean(false),
    );
    rt.object_set(
        inspect_fn,
        "defaultOptions".into(),
        Value::Object(default_options_obj),
    );
    rt.object_set(util, "inspect".into(), Value::Object(inspect_fn));

    register_method(rt, util, "format", |rt, args| {
        if args.is_empty() {
            return Ok(Value::String(Rc::new(
                rusty_js_runtime::value::JsString::from(String::new()),
            )));
        }

        if args.len() == 1 {
            if let Value::String(s) = &args[0] {
                return Ok(Value::String(s.clone()));
            }
        }

        if !matches!(&args[0], Value::String(_)) {
            let parts: Vec<String> = args
                .iter()
                .map(|a| match a {
                    Value::String(s) => s.as_str().to_string(),
                    other => inspect_value(rt, other, 0, 2, DEFAULT_BREAK_LENGTH),
                })
                .collect();
            return Ok(Value::String(Rc::new(
                rusty_js_runtime::value::JsString::from(parts.join(" ")),
            )));
        }
        let fmt = match &args[0] {
            Value::String(s) => s.as_str(),
            _ => unreachable!("non-string first arg returned above"),
        };
        if fmt == "%s=%d" && args.len() == 3 {
            if let (Value::String(key), Value::Number(number)) = (&args[1], &args[2]) {
                let mut rendered = String::with_capacity(key.as_str().len() + 1 + 24);
                rendered.push_str(key.as_str());
                rendered.push('=');
                let number = format_number_preserve_negative_zero(*number);
                let separator_relevant = !number.contains(['.', 'e', 'E'])
                    && number
                        .trim_start_matches(['+', '-'])
                        .trim_end_matches('n')
                        .len()
                        > 3;
                if separator_relevant && util_numeric_separator_option(rt, None) {
                    rendered.push_str(&add_numeric_separators(&number));
                } else {
                    rendered.push_str(&number);
                }
                return Ok(Value::String(Rc::new(
                    rusty_js_runtime::value::JsString::from(rendered),
                )));
            }
        }
        let mut out = String::new();
        let mut chars = fmt.chars().peekable();
        let mut arg_idx = 1usize;
        let mut numeric_separator = None;
        while let Some(c) = chars.next() {
            if c == '%' {
                match chars.next() {
                    Some(spec @ ('s' | 'd' | 'i' | 'f' | 'j')) if arg_idx >= args.len() => {

                        out.push('%');
                        out.push(spec);
                    }
                    Some('s') => {
                        let a = args[arg_idx].clone();
                        arg_idx += 1;
                        if let Value::String(s) = &a {
                            out.push_str(s.as_str());
                        } else {
                            let numeric_separator = *numeric_separator
                                .get_or_insert_with(|| util_numeric_separator_option(rt, None));
                            out.push_str(&util_format_string(rt, &a, numeric_separator)?);
                        }
                    }
                    Some('d') => {

                        let a = args[arg_idx].clone();
                        arg_idx += 1;
                        let numeric_separator = *numeric_separator
                            .get_or_insert_with(|| util_numeric_separator_option(rt, None));
                        out.push_str(&util_format_decimal(rt, &a, numeric_separator)?);
                    }
                    Some('i') => {
                        let a = args[arg_idx].clone();
                        arg_idx += 1;
                        let numeric_separator = *numeric_separator
                            .get_or_insert_with(|| util_numeric_separator_option(rt, None));
                        out.push_str(&util_format_integer(rt, &a, numeric_separator)?);
                    }
                    Some('f') => {
                        let a = args[arg_idx].clone();
                        arg_idx += 1;
                        out.push_str(&util_format_float(rt, &a)?);
                    }
                    Some('j') => {
                        let a = args[arg_idx].clone();
                        arg_idx += 1;
                        let s = match json_stringify_via_intrinsic(rt, &a) {
                            Ok(s) => s,
                            Err(err) if util_is_json_circular_error(rt, &err) => {
                                "[Circular]".to_string()
                            }
                            Err(err) => return Err(err),
                        };
                        if s.is_empty()
                            && matches!(a, Value::Undefined | Value::Symbol(_) | Value::Object(_))
                        {
                            out.push_str("undefined");
                        } else {
                            out.push_str(&s);
                        }
                    }

                    Some(spec @ ('o' | 'O')) => {
                        if arg_idx >= args.len() {
                            out.push('%');
                            out.push(spec);
                        } else {
                            let a = args[arg_idx].clone();
                            arg_idx += 1;
                            if spec == 'o' {

                                let _sp = ShowProxyGuard::set(true);
                                out.push_str(&inspect_value_with_options(
                                    rt,
                                    &a,
                                    0,
                                    i32::MAX,
                                    DEFAULT_BREAK_LENGTH,
                                    false,
                                    None,
                                    None,
                                    false,
                                    true,
                                ));
                            } else {
                                out.push_str(&inspect_value(rt, &a, 0, 2, DEFAULT_BREAK_LENGTH));
                            }
                        }
                    }

                    Some('c') => {
                        if arg_idx >= args.len() {
                            out.push_str("%c");
                        } else {
                            arg_idx += 1;
                        }
                    }
                    Some('%') => out.push('%'),
                    Some(other) => {
                        out.push('%');
                        out.push(other);
                    }
                    None => out.push('%'),
                }
            } else {
                out.push(c);
            }
        }

        for i in arg_idx..args.len() {
            out.push(' ');
            match &args[i] {
                Value::String(s) => out.push_str(s.as_str()),
                v => out.push_str(&inspect_value(rt, v, 0, 2, DEFAULT_BREAK_LENGTH)),
            }
        }
        Ok(Value::String(Rc::new(
            rusty_js_runtime::value::JsString::from(out),
        )))
    });

    register_method(rt, util, "inherits", |rt, args| {
        let ctor = args.first().cloned().unwrap_or(Value::Undefined);
        if !rt.is_callable(&ctor) {
            let received = match ctor {
                Value::Null => "null",
                Value::Undefined => "undefined",
                _ => "non-function",
            };
            return Err(util_invalid_arg_type(
                rt,
                &format!("The \"ctor\" argument must be of type function. Received {received}"),
            ));
        }
        let ctor_id = match ctor {
            Value::Object(id) => id,
            _ => unreachable!("callable values are objects"),
        };

        let super_ctor = args.get(1).cloned().unwrap_or(Value::Undefined);
        if !rt.is_callable(&super_ctor) {
            let received = match super_ctor {
                Value::Null => "null",
                Value::Undefined => "undefined",
                _ => "non-function",
            };
            return Err(util_invalid_arg_type(
                rt,
                &format!(
                    "The \"superCtor\" argument must be of type function. Received {received}"
                ),
            ));
        }
        let super_id = match super_ctor {
            Value::Object(id) => id,
            _ => unreachable!("callable values are objects"),
        };
        let super_proto = rt.object_get(super_id, "prototype");
        if !matches!(super_proto, Value::Object(_)) {
            return Err(util_invalid_arg_type(
                rt,
                "The \"superCtor.prototype\" property must be of type object. Received undefined",
            ));
        }
        rt.obj_mut(ctor_id)
            .set_own_internal("super_".into(), Value::Object(super_id));
        let new_proto = match rt.object_get(ctor_id, "prototype") {
            Value::Object(id) => id,
            _ => rt.alloc_object(Object::new_ordinary()),
        };
        if let Value::Object(sp) = super_proto {
            rt.set_object_prototype_internal(new_proto, Some(sp));
        }
        rt.obj_mut(new_proto)
            .set_own_internal("constructor".into(), Value::Object(ctor_id));
        rt.object_set(ctor_id, "prototype".into(), Value::Object(new_proto));
        Ok(Value::Undefined)
    });

    register_method(rt, util, "promisify", |rt, args| {
        let Some(Value::Object(fn_id)) = args.first() else {
            return Err(RuntimeError::TypeError(
                "util.promisify: original must be a function".into(),
            ));
        };
        let custom = rt.object_get(*fn_id, PROMISIFY_CUSTOM_KEY);
        if rt.is_callable(&custom) {
            return Ok(custom);
        }
        let original = *fn_id;
        let wrapper = make_callable_rooted(rt, "promisified", vec![original], move |rt, args| {
            let promise = new_promise(rt);
            let cb = make_callable_rooted(
                rt,
                "promisifyCallback",
                vec![promise],
                move |rt, cb_args| {
                    let err = cb_args.first().cloned().unwrap_or(Value::Null);
                    if !matches!(err, Value::Null | Value::Undefined | Value::Boolean(false)) {
                        reject_promise(rt, promise, err);
                        return Ok(Value::Undefined);
                    }
                    let value = cb_args.get(1).cloned().unwrap_or(Value::Undefined);
                    resolve_promise(rt, promise, value);
                    Ok(Value::Undefined)
                },
            );
            let mut call_args = args.to_vec();
            call_args.push(Value::Object(cb));
            match rt.call_function(Value::Object(original), Value::Undefined, call_args) {
                Ok(_) => {}
                Err(err) => {
                    let value = util_make_error(rt, "Error", &format!("{:?}", err));
                    reject_promise(rt, promise, value);
                }
            }
            Ok(Value::Object(promise))
        });
        Ok(Value::Object(wrapper))
    });
    if let Value::Object(promisify_fn) = rt.object_get(util, "promisify") {
        rt.object_set(
            promisify_fn,
            "custom".into(),
            Value::Symbol(Rc::new(PROMISIFY_CUSTOM_KEY.to_string())),
        );
    }
    register_method(rt, util, "callbackify", |rt, args| {
        let Some(Value::Object(fn_id)) = args.first() else {
            return Err(RuntimeError::TypeError(
                "util.callbackify: original must be a function".into(),
            ));
        };
        let original = *fn_id;
        let wrapper = make_callable_rooted(rt, "callbackified", vec![original], move |rt, args| {
            let mut call_args = args.to_vec();
            let callback = match call_args.pop() {
                Some(Value::Object(cb)) if rt.is_callable(&Value::Object(cb)) => cb,
                _ => {
                    return Err(RuntimeError::TypeError(
                        "util.callbackify: callback must be a function".into(),
                    ))
                }
            };
            let result = rt.call_function(Value::Object(original), Value::Undefined, call_args)?;
            let on_fulfilled = make_callable_rooted(
                rt,
                "callbackifyFulfilled",
                vec![callback],
                move |rt, vals| {
                    let value = vals.first().cloned().unwrap_or(Value::Undefined);
                    rt.call_function(
                        Value::Object(callback),
                        Value::Undefined,
                        vec![Value::Null, value],
                    )?;
                    Ok(Value::Undefined)
                },
            );
            let on_rejected = make_callable_rooted(
                rt,
                "callbackifyRejected",
                vec![callback],
                move |rt, vals| {
                    let mut reason = vals.first().cloned().unwrap_or(Value::Undefined);
                    if matches!(
                        reason,
                        Value::Null | Value::Undefined | Value::Boolean(false)
                    ) {
                        let err = rt.alloc_object(Object::new_ordinary());
                        rt.object_set(
                            err,
                            "name".into(),
                            Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                                "Error",
                            ))),
                        );
                        rt.object_set(
                            err,
                            "message".into(),
                            Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                                "Promise was rejected with falsy value",
                            ))),
                        );
                        rt.object_set(err, "reason".into(), reason);
                        reason = Value::Object(err);
                    }
                    rt.call_function(
                        Value::Object(callback),
                        Value::Undefined,
                        vec![reason, Value::Undefined],
                    )?;
                    Ok(Value::Undefined)
                },
            );
            let then = match result {
                Value::Object(p) => rt.object_get(p, "then"),
                _ => Value::Undefined,
            };
            if rt.is_callable(&then) {
                rt.call_function(
                    then,
                    result,
                    vec![Value::Object(on_fulfilled), Value::Object(on_rejected)],
                )?;
            }
            Ok(Value::Undefined)
        });
        Ok(Value::Object(wrapper))
    });
    register_util_deprecate(rt, util);

    if !matches!(rt.global_get("TextDecoder"), Value::Undefined) {
        set_constant(rt, util, "TextDecoder", rt.global_get("TextDecoder"));
    }
    if !matches!(rt.global_get("TextEncoder"), Value::Undefined) {
        set_constant(rt, util, "TextEncoder", rt.global_get("TextEncoder"));
    }

    register_method(rt, util, "isDeepStrictEqual", |rt, args| {
        let a = args.first().cloned().unwrap_or(Value::Undefined);
        let b = args.get(1).cloned().unwrap_or(Value::Undefined);
        let sa = json_stringify_via_intrinsic(rt, &a)?;
        let sb = json_stringify_via_intrinsic(rt, &b)?;
        Ok(Value::Boolean(sa == sb))
    });

    let types = new_object(rt);
    register_method(rt, types, "isPromise", |rt, args| {
        Ok(Value::Boolean(matches!(args.first(),
            Some(Value::Object(id)) if matches!(rt.obj(*id).internal_kind, InternalKind::Promise(_)))))
    });
    register_method(rt, types, "isRegExp", |rt, args| {
        Ok(Value::Boolean(matches!(args.first(),
            Some(Value::Object(id)) if matches!(rt.obj(*id).internal_kind, InternalKind::RegExp(_)))))
    });

    register_method(rt, types, "isMap", |rt, args| {
        Ok(Value::Boolean(matches!(args.first(),
            Some(Value::Object(id)) if rt.obj(*id).has_own_str("__map_data")
                && !matches!(rt.object_get(*id, "__is_weakmap"), Value::Boolean(true)))))
    });
    register_method(rt, types, "isSet", |rt, args| {
        Ok(Value::Boolean(matches!(args.first(),
            Some(Value::Object(id)) if rt.obj(*id).has_own_str("__set_data")
                && !matches!(rt.object_get(*id, "__is_weakset"), Value::Boolean(true)))))
    });
    register_method(rt, types, "isDate", |rt, args| {
        Ok(Value::Boolean(matches!(args.first(),
            Some(Value::Object(id)) if rt.obj(*id).has_own_str("__date_ms"))))
    });
    register_method(rt, types, "isNativeError", |rt, args| {
        Ok(Value::Boolean(matches!(args.first(),
            Some(Value::Object(id)) if matches!(rt.obj(*id).internal_kind, InternalKind::Error))))
    });

    register_method(rt, types, "isArrayBuffer", |rt, args| {
        Ok(Value::Boolean(matches!(args.first(),
            Some(Value::Object(id))
                if rt.array_buffers.get(id).map(|r| r.shared.is_none()).unwrap_or(false))))
    });
    register_method(rt, types, "isAnyArrayBuffer", |rt, args| {
        Ok(Value::Boolean(matches!(args.first(),
            Some(Value::Object(id)) if rt.array_buffers.contains_key(id))))
    });
    register_method(rt, types, "isTypedArray", |rt, args| {
        Ok(Value::Boolean(matches!(args.first(),
            Some(Value::Object(id)) if rt.typed_array_views.get(id).is_some_and(|v| &*v.element_kind != "DataView"))))
    });
    register_method(rt, types, "isDataView", |rt, args| {
        Ok(Value::Boolean(matches!(args.first(),
            Some(Value::Object(id)) if rt.typed_array_views.get(id).is_some_and(|v| &*v.element_kind == "DataView"))))
    });

    register_method(rt, types, "isWeakMap", |rt, args| {
        Ok(Value::Boolean(matches!(args.first(),
            Some(Value::Object(id)) if matches!(rt.object_get(*id, "__is_weakmap"), Value::Boolean(true)))))
    });
    register_method(rt, types, "isWeakSet", |rt, args| {
        Ok(Value::Boolean(matches!(args.first(),
            Some(Value::Object(id)) if matches!(rt.object_get(*id, "__is_weakset"), Value::Boolean(true)))))
    });

    for ta in [
        "Uint8Array",
        "Uint8ClampedArray",
        "Int8Array",
        "Int16Array",
        "Uint16Array",
        "Int32Array",
        "Uint32Array",
        "Float16Array",
        "Float32Array",
        "Float64Array",
        "BigInt64Array",
        "BigUint64Array",
    ] {
        let name = format!("is{}", ta);
        let want = ta;
        register_method(rt, types, &name, move |rt, args| {
            let id = match args.first() {
                Some(Value::Object(i)) => *i,
                _ => return Ok(Value::Boolean(false)),
            };

            Ok(Value::Boolean(
                rt.typed_array_views
                    .get(&id)
                    .is_some_and(|v| &*v.element_kind == want),
            ))
        });
    }

    for (pred, ctor) in [
        ("isAsyncFunction", "AsyncFunction"),
        ("isGeneratorFunction", "GeneratorFunction"),
        ("isStringObject", "String"),
        ("isNumberObject", "Number"),
        ("isBooleanObject", "Boolean"),
        ("isSymbolObject", "Symbol"),
        ("isBigIntObject", "BigInt"),
    ] {
        let want = ctor;
        let is_fn = ctor.ends_with("Function");
        register_method(rt, types, pred, move |rt, args| {
            let id = match args.first() {
                Some(Value::Object(i)) => *i,
                _ => return Ok(Value::Boolean(false)),
            };
            if is_fn {

                let callable = rt.is_callable(&Value::Object(id));
                return Ok(Value::Boolean(callable && util_ctor_name(rt, id) == want));
            }

            let tag_matches = util_ctor_name(rt, id) == want
                && !rt.typed_array_views.contains_key(&id)
                && !rt.array_buffers.contains_key(&id);
            Ok(Value::Boolean(
                tag_matches && util_is_boxed_primitive(rt, id, want),
            ))
        });
    }
    register_method(rt, types, "isBoxedPrimitive", |rt, args| {
        let id = match args.first() {
            Some(Value::Object(i)) => *i,
            _ => return Ok(Value::Boolean(false)),
        };
        for w in ["String", "Number", "Boolean", "Symbol", "BigInt"] {
            if util_ctor_name(rt, id) == w && util_is_boxed_primitive(rt, id, w) {
                return Ok(Value::Boolean(true));
            }
        }
        Ok(Value::Boolean(false))
    });
    register_method(rt, types, "isModuleNamespaceObject", |rt, args| {
        Ok(Value::Boolean(matches!(args.first(),
            Some(Value::Object(id)) if matches!(rt.obj(*id).internal_kind, InternalKind::ModuleNamespace))))
    });

    register_method(rt, types, "isSharedArrayBuffer", |rt, args| {
        Ok(Value::Boolean(matches!(args.first(),
            Some(Value::Object(id))
                if rt.array_buffers.get(id).map(|r| r.shared.is_some()).unwrap_or(false))))
    });

    register_method(rt, types, "isProxy", |rt, args| {
        Ok(Value::Boolean(matches!(args.first(),
            Some(Value::Object(id)) if matches!(rt.obj(*id).internal_kind, InternalKind::Proxy(_)))))
    });

    register_method(rt, types, "isGeneratorObject", |rt, args| {
        Ok(Value::Boolean(matches!(args.first(),
            Some(Value::Object(id)) if matches!(rt.obj(*id).internal_kind, InternalKind::Generator(_)))))
    });
    register_method(rt, types, "isArgumentsObject", |rt, args| {
        Ok(Value::Boolean(matches!(args.first(),
            Some(Value::Object(id)) if matches!(rt.obj(*id).internal_kind, InternalKind::MappedArguments { .. }))))
    });

    register_method(rt, types, "isMapIterator", |rt, args| {
        Ok(Value::Boolean(match args.first() {
            Some(Value::Object(id)) => {
                matches!(rt.object_get(*id, "__coll_storage"), Value::String(s) if s.as_str() == "__map_data")
            }
            _ => false,
        }))
    });
    register_method(rt, types, "isSetIterator", |rt, args| {
        Ok(Value::Boolean(match args.first() {
            Some(Value::Object(id)) => {
                matches!(rt.object_get(*id, "__coll_storage"), Value::String(s) if s.as_str() == "__set_data")
            }
            _ => false,
        }))
    });

    register_method(rt, types, "isKeyObject", |rt, args| {
        Ok(Value::Boolean(matches!(args.first(),
            Some(Value::Object(id)) if matches!(rt.object_get(*id, "__cruft_key_object"), Value::Boolean(true)))))
    });

    for pred in ["isExternal", "isCryptoKey", "isFloat16Array"] {
        register_method(rt, types, pred, |_rt, _args| Ok(Value::Boolean(false)));
    }

    register_method(rt, types, "isArrayBufferView", |rt, args| {
        Ok(Value::Boolean(
            matches!(args.first(), Some(Value::Object(id)) if rt.typed_array_views.contains_key(id)),
        ))
    });

    set_constant(rt, util, "types", Value::Object(types));

    rt.define_global_property("util_types", Value::Object(types));

    fn u_sval(s: &str) -> Value {
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
            s.to_string(),
        )))
    }

    if let Value::Object(arr) = rt.global_get("Array") {
        let f = rt.object_get(arr, "isArray");
        if rt.is_callable(&f) {
            rt.object_set(util, "isArray".into(), f);
        }
    }

    register_method(rt, util, "_extend", |rt, args| {
        if let Value::Object(o) = rt.global_get("Object") {
            let assign = rt.object_get(o, "assign");
            if rt.is_callable(&assign) {
                return rt.call_function(assign, Value::Undefined, args.to_vec());
            }
        }
        Ok(args.first().cloned().unwrap_or(Value::Undefined))
    });

    register_method(rt, util, "stripVTControlCharacters", |rt, args| {
        let s = match args.first() {
            Some(Value::String(s)) => s.as_str().to_string(),
            _ => return Ok(args.first().cloned().unwrap_or(Value::Undefined)),
        };
        let mut out = String::new();
        let mut chars = s.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '\u{1b}' {
                if chars.peek() == Some(&'[') {
                    chars.next();
                    while let Some(&n) = chars.peek() {
                        chars.next();
                        if n.is_ascii_alphabetic() {
                            break;
                        }
                    }
                } else {
                    chars.next();
                }
            } else {
                out.push(c);
            }
        }
        let _ = rt;
        Ok(Value::String(Rc::new(
            rusty_js_runtime::value::JsString::from(out),
        )))
    });

    register_method(rt, util, "toUSVString", |rt, args| {
        match args.first() {
            Some(Value::String(s)) => {

                if s.is_well_formed() {
                    Ok(Value::String(s.clone()))
                } else {
                    Ok(u_sval(&String::from_utf16_lossy(&s.code_units())))
                }
            }
            Some(v) => Ok(u_sval(&rt.coerce_to_string(v)?)),
            None => Ok(u_sval("undefined")),
        }
    });

    register_method(rt, util, "formatWithOptions", |rt, args| {
        if !matches!(args.first(), Some(Value::Object(_))) {
            let err = util_make_error(
                rt,
                "TypeError",
                "The \"inspectOptions\" argument must be of type object",
            );
            if let Value::Object(id) = err {
                rt.object_set(id, "code".into(), u_sval("ERR_INVALID_ARG_TYPE"));
            }
            return Err(RuntimeError::Thrown(err));
        }
        let colors = args
            .first()
            .and_then(|v| match v {
                Value::Object(id) => Some(abstract_ops::to_boolean(&rt.object_get(*id, "colors"))),
                _ => None,
            })
            .unwrap_or(false);
        if colors && args.get(1).is_some_and(|v| !matches!(v, Value::String(_))) {
            let parts: Vec<String> = args
                .get(1..)
                .unwrap_or(&[])
                .iter()
                .map(|a| match a {
                    Value::String(s) => s.as_str().to_string(),
                    other => inspect_colorized_value(rt, other),
                })
                .collect();
            return Ok(Value::String(Rc::new(
                rusty_js_runtime::value::JsString::from(parts.join(" ")),
            )));
        }
        let cg = match rt.global_get("util") {
            Value::Object(i) => i,
            _ => return Ok(u_sval("")),
        };
        let default_options = match rt.object_get(cg, "inspect") {
            Value::Object(inspect_id) => match rt.object_get(inspect_id, "defaultOptions") {
                Value::Object(defaults_id) => Some(defaults_id),
                _ => None,
            },
            _ => None,
        };
        let old_numeric_separator = default_options
            .map(|id| abstract_ops::to_boolean(&rt.object_get(id, "numericSeparator")))
            .unwrap_or(false);
        if let Some(defaults_id) = default_options {
            let numeric_separator = util_numeric_separator_option(rt, args.first());
            rt.object_set(
                defaults_id,
                "numericSeparator".into(),
                Value::Boolean(numeric_separator),
            );
        }
        let f = rt.object_get(cg, "format");
        let result = rt.call_function(f, Value::Object(cg), args.get(1..).unwrap_or(&[]).to_vec());
        if let Some(defaults_id) = default_options {
            rt.object_set(
                defaults_id,
                "numericSeparator".into(),
                Value::Boolean(old_numeric_separator),
            );
        }
        result
    });

    fn system_error_arg(rt: &mut Runtime, v: Option<&Value>) -> Result<i64, RuntimeError> {
        let n = match v {
            Some(Value::Number(n)) if n.is_finite() && n.fract() == 0.0 => *n as i64,
            _ => {
                return Err(RuntimeError::TypeError(
                    "The \"err\" argument must be of type number.".into(),
                ))
            }
        };
        if n >= 0 {
            let msg = format!(
                "The value of \"err\" is out of range. It must be a negative integer. Received {n}"
            );
            let ctor = rt.global_get("RangeError");
            if let Ok(Value::Object(id)) = rt.construct(ctor, vec![u_sval(&msg)]) {
                rt.object_set(id, "code".into(), u_sval("ERR_OUT_OF_RANGE"));
                return Err(RuntimeError::Thrown(Value::Object(id)));
            }
            return Err(RuntimeError::RangeError(msg));
        }
        Ok(n)
    }

    const SYSTEM_ERROR_TABLE: &[(i64, &str, &str)] = &[
        (-4095, "EOF", "end of file"),
        (-4094, "UNKNOWN", "unknown error"),
        (-4080, "ECHARSET", "invalid Unicode character"),
        (-4028, "EFTYPE", "inappropriate file type or format"),
        (-3014, "EAI_PROTOCOL", "resolved protocol is unknown"),
        (-3013, "EAI_BADHINTS", "invalid value for hints"),
        (-3011, "EAI_SOCKTYPE", "socket type not supported"),
        (
            -3010,
            "EAI_SERVICE",
            "service not available for socket type",
        ),
        (-3009, "EAI_OVERFLOW", "argument buffer overflow"),
        (-3008, "EAI_NONAME", "unknown node or service"),
        (-3007, "EAI_NODATA", "no address"),
        (-3006, "EAI_MEMORY", "out of memory"),
        (-3005, "EAI_FAMILY", "ai_family not supported"),
        (-3004, "EAI_FAIL", "permanent failure"),
        (-3003, "EAI_CANCELED", "request canceled"),
        (-3002, "EAI_BADFLAGS", "bad ai_flags value"),
        (-3001, "EAI_AGAIN", "temporary failure"),
        (-3000, "EAI_ADDRFAMILY", "address family not supported"),
        (-125, "ECANCELED", "operation canceled"),
        (-121, "EREMOTEIO", "remote I/O error"),
        (-114, "EALREADY", "connection already in progress"),
        (-113, "EHOSTUNREACH", "host is unreachable"),
        (-112, "EHOSTDOWN", "host is down"),
        (-111, "ECONNREFUSED", "connection refused"),
        (-110, "ETIMEDOUT", "connection timed out"),
        (
            -108,
            "ESHUTDOWN",
            "cannot send after transport endpoint shutdown",
        ),
        (-107, "ENOTCONN", "socket is not connected"),
        (-106, "EISCONN", "socket is already connected"),
        (-105, "ENOBUFS", "no buffer space available"),
        (-104, "ECONNRESET", "connection reset by peer"),
        (-103, "ECONNABORTED", "software caused connection abort"),
        (-101, "ENETUNREACH", "network is unreachable"),
        (-100, "ENETDOWN", "network is down"),
        (-99, "EADDRNOTAVAIL", "address not available"),
        (-98, "EADDRINUSE", "address already in use"),
        (-97, "EAFNOSUPPORT", "address family not supported"),
        (-95, "ENOTSUP", "operation not supported on socket"),
        (-94, "ESOCKTNOSUPPORT", "socket type not supported"),
        (-93, "EPROTONOSUPPORT", "protocol not supported"),
        (-92, "ENOPROTOOPT", "protocol not available"),
        (-91, "EPROTOTYPE", "protocol wrong type for socket"),
        (-90, "EMSGSIZE", "message too long"),
        (-89, "EDESTADDRREQ", "destination address required"),
        (-88, "ENOTSOCK", "socket operation on non-socket"),
        (-84, "EILSEQ", "illegal byte sequence"),
        (-75, "EOVERFLOW", "value too large for defined data type"),
        (-71, "EPROTO", "protocol error"),
        (-64, "ENONET", "machine is not on the network"),
        (-61, "ENODATA", "no data available"),
        (-49, "EUNATCH", "protocol driver not attached"),
        (-40, "ELOOP", "too many symbolic links encountered"),
        (-39, "ENOTEMPTY", "directory not empty"),
        (-38, "ENOSYS", "function not implemented"),
        (-36, "ENAMETOOLONG", "name too long"),
        (-34, "ERANGE", "result too large"),
        (-32, "EPIPE", "broken pipe"),
        (-31, "EMLINK", "too many links"),
        (-30, "EROFS", "read-only file system"),
        (-29, "ESPIPE", "invalid seek"),
        (-28, "ENOSPC", "no space left on device"),
        (-27, "EFBIG", "file too large"),
        (-26, "ETXTBSY", "text file is busy"),
        (-25, "ENOTTY", "inappropriate ioctl for device"),
        (-24, "EMFILE", "too many open files"),
        (-23, "ENFILE", "file table overflow"),
        (-22, "EINVAL", "invalid argument"),
        (-21, "EISDIR", "illegal operation on a directory"),
        (-20, "ENOTDIR", "not a directory"),
        (-19, "ENODEV", "no such device"),
        (-18, "EXDEV", "cross-device link not permitted"),
        (-17, "EEXIST", "file already exists"),
        (-16, "EBUSY", "resource busy or locked"),
        (-14, "EFAULT", "bad address in system call argument"),
        (-13, "EACCES", "permission denied"),
        (-12, "ENOMEM", "not enough memory"),
        (-11, "EAGAIN", "resource temporarily unavailable"),
        (-9, "EBADF", "bad file descriptor"),
        (-8, "ENOEXEC", "exec format error"),
        (-7, "E2BIG", "argument list too long"),
        (-6, "ENXIO", "no such device or address"),
        (-5, "EIO", "i/o error"),
        (-4, "EINTR", "interrupted system call"),
        (-3, "ESRCH", "no such process"),
        (-2, "ENOENT", "no such file or directory"),
        (-1, "EPERM", "operation not permitted"),
    ];

    register_method(rt, util, "getSystemErrorName", |rt, a| {
        let errno = system_error_arg(rt, a.first())?;
        match SYSTEM_ERROR_TABLE.iter().find(|(e, _, _)| *e == errno) {
            Some((_, name, _)) => Ok(u_sval(name)),
            None => Ok(u_sval(&format!("Unknown system error {errno}"))),
        }
    });
    register_method(rt, util, "getSystemErrorMessage", |rt, a| {
        let errno = system_error_arg(rt, a.first())?;
        match SYSTEM_ERROR_TABLE.iter().find(|(e, _, _)| *e == errno) {
            Some((_, _, msg)) => Ok(u_sval(msg)),
            None => Ok(u_sval(&format!("Unknown system error {errno}"))),
        }
    });
    register_method(rt, util, "getSystemErrorMap", |rt, _a| {

        let map = match rt.global_get("Map") {
            Value::Object(mc) => match rt.construct(Value::Object(mc), Vec::new()) {
                Ok(Value::Object(m)) => m,
                _ => return Ok(Value::Undefined),
            },
            _ => return Ok(Value::Undefined),
        };
        let set = rt.object_get(map, "set");
        if !rt.is_callable(&set) {
            return Ok(Value::Object(map));
        }
        for (errno, name, msg) in SYSTEM_ERROR_TABLE {
            let pair = rt.alloc_object(rusty_js_runtime::value::Object::new_array());
            rt.object_set(pair, "0".into(), u_sval(name));
            rt.object_set(pair, "1".into(), u_sval(msg));
            rt.object_set(pair, "length".into(), Value::Number(2.0));
            let _ = rt.call_function(
                set.clone(),
                Value::Object(map),
                vec![Value::Number(*errno as f64), Value::Object(pair)],
            );
        }
        Ok(Value::Object(map))
    });
    register_method(rt, util, "getCallSites", |rt, _a| {
        Ok(Value::Object(
            rt.alloc_object(rusty_js_runtime::value::Object::new_array()),
        ))
    });
    register_method(rt, util, "aborted", |rt, _a| {
        let p = rusty_js_runtime::promise::new_promise(rt);
        Ok(Value::Object(p))
    });

    {
        let c = crate::register::make_callable(rt, "MIMEType", |rt, a| {
            let this = match rt.current_this() {
                Value::Object(id) => id,
                _ => rt.alloc_object(Object::new_ordinary()),
            };
            let input = match a.first() {
                Some(Value::String(s)) => s.as_str().to_string(),
                Some(v) => rt.coerce_to_string(v)?,
                None => String::new(),
            };
            let (ty, sub, pairs) = mime_parse(&input);
            rt.object_set(this, "type".into(), util_string_value(&ty));
            rt.object_set(this, "subtype".into(), util_string_value(&sub));
            rt.object_set(
                this,
                "essence".into(),
                util_string_value(&format!("{ty}/{sub}")),
            );
            let params = build_mime_params(rt, pairs);
            rt.object_set(this, "params".into(), Value::Object(params));
            register_method(rt, this, "toString", |rt, _a| {
                let this = match rt.current_this() {
                    Value::Object(id) => id,
                    _ => return Ok(util_string_value("")),
                };
                let essence = match rt.object_get(this, "essence") {
                    Value::String(s) => s.as_str().to_string(),
                    _ => String::new(),
                };
                let params_str = match rt.object_get(this, "params") {
                    Value::Object(p) => mime_read_pairs(rt, p)
                        .iter()
                        .map(|(k, v)| format!(";{k}={}", mime_quote_value(v)))
                        .collect::<String>(),
                    _ => String::new(),
                };
                Ok(util_string_value(&format!("{essence}{params_str}")))
            });
            Ok(Value::Object(this))
        });
        let proto = new_object(rt);
        rt.object_set(proto, "constructor".into(), Value::Object(c));
        rt.object_set(c, "prototype".into(), Value::Object(proto));
        rt.object_set(util, "MIMEType".to_string(), Value::Object(c));
    }
    {
        let c = crate::register::make_callable(rt, "MIMEParams", |rt, _a| {
            let params = build_mime_params(rt, Vec::new());
            Ok(Value::Object(params))
        });
        let proto = new_object(rt);
        rt.object_set(proto, "constructor".into(), Value::Object(c));
        rt.object_set(c, "prototype".into(), Value::Object(proto));
        rt.object_set(util, "MIMEParams".to_string(), Value::Object(c));
    }

    register_method(rt, util, "parseArgs", |rt, args| {
        fn string_array_from(rt: &mut Runtime, v: Value) -> Vec<String> {
            let Value::Object(id) = v else {
                return Vec::new();
            };
            let len = rt.array_length(id);
            let mut out = Vec::new();
            for i in 0..len {
                match rt.object_get(id, &i.to_string()) {
                    Value::String(s) => out.push(s.as_str().to_string()),
                    Value::Undefined => {}
                    other => out.push(abstract_ops::to_string(&other).as_str().to_string()),
                }
            }
            out
        }

        fn set_array_strings(
            rt: &mut Runtime,
            values: &[String],
        ) -> rusty_js_runtime::value::ObjectRef {
            let arr = rt.alloc_object(Object::new_array());
            for (i, value) in values.iter().enumerate() {
                rt.object_set(arr, i.to_string(), u_sval(value));
            }
            rt.object_set(arr, "length".into(), Value::Number(values.len() as f64));
            arr
        }

        fn set_value(
            values_obj: rusty_js_runtime::value::ObjectRef,
            rt: &mut Runtime,
            key: &str,
            value: Value,
        ) {
            rt.object_set(values_obj, key.to_string(), value);
        }

        fn set_value_multi(
            values_obj: rusty_js_runtime::value::ObjectRef,
            rt: &mut Runtime,
            key: &str,
            value: Value,
            multiple: bool,
        ) {
            if !multiple {
                rt.object_set(values_obj, key.to_string(), value);
                return;
            }

            if let Value::Object(arr) = rt.object_get(values_obj, key) {
                if let Value::Number(len) = rt.object_get(arr, "length") {
                    let len = len as usize;
                    rt.object_set(arr, len.to_string(), value);
                    rt.object_set(arr, "length".into(), Value::Number((len + 1) as f64));
                    return;
                }
            }
            let arr = rt.alloc_object(Object::new_array());
            rt.object_set(arr, "0".into(), value);
            rt.object_set(arr, "length".into(), Value::Number(1.0));
            rt.object_set(values_obj, key.to_string(), Value::Object(arr));
        }

        let opts = match args.first() {
            Some(Value::Object(id)) => Some(*id),
            _ => None,
        };
        let raw_args = opts
            .map(|id| string_array_from(rt, rt.object_get(id, "args")))
            .unwrap_or_default();
        let allow_positionals = opts
            .map(|id| matches!(rt.object_get(id, "allowPositionals"), Value::Boolean(true)))
            .unwrap_or(false);

        let strict = opts
            .map(|id| !matches!(rt.object_get(id, "strict"), Value::Boolean(false)))
            .unwrap_or(true);
        let mut known_long: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut known_short: std::collections::HashSet<String> = std::collections::HashSet::new();

        let mut short_to_long: std::collections::HashMap<String, String> =
            std::collections::HashMap::new();
        let mut long_type: std::collections::HashMap<String, String> =
            std::collections::HashMap::new();

        let mut long_multiple: std::collections::HashSet<String> = std::collections::HashSet::new();

        let mut long_default: Vec<(String, Value)> = Vec::new();
        if let Some(id) = opts {
            if let Value::Object(options_id) = rt.object_get(id, "options") {
                for key in rt.ordinary_own_enumerable_string_keys(options_id) {
                    if let Value::Object(spec) = rt.object_get(options_id, &key) {
                        if let Value::String(s) = rt.object_get(spec, "short") {
                            known_short.insert(s.as_str().to_string());
                            short_to_long.insert(s.as_str().to_string(), key.clone());
                        }
                        if let Value::String(t) = rt.object_get(spec, "type") {
                            long_type.insert(key.clone(), t.as_str().to_string());
                        }
                        if matches!(rt.object_get(spec, "multiple"), Value::Boolean(true)) {
                            long_multiple.insert(key.clone());
                        }
                        if rt.obj(spec).has_own_str("default") {
                            long_default.push((key.clone(), rt.object_get(spec, "default")));
                        }
                    }
                    known_long.insert(key);
                }
            }
        }
        let unknown_option_error = |rt: &mut Runtime, flag: &str| -> RuntimeError {
            let msg = format!("Unknown option '{flag}'");
            let ctor = rt.global_get("TypeError");
            if let Ok(Value::Object(id)) = rt.construct(ctor, vec![u_sval(&msg)]) {
                rt.object_set(id, "code".into(), u_sval("ERR_PARSE_ARGS_UNKNOWN_OPTION"));
                return RuntimeError::Thrown(Value::Object(id));
            }
            RuntimeError::TypeError(msg)
        };

        let o = new_object(rt);
        let v = new_object(rt);
        let tokens_requested = opts
            .map(|id| matches!(rt.object_get(id, "tokens"), Value::Boolean(true)))
            .unwrap_or(false);
        let tokens_arr = rt.alloc_object(Object::new_array());
        let mut ntok = 0usize;
        let mut positionals = Vec::new();
        let mut i = 0usize;
        while i < raw_args.len() {
            let token = &raw_args[i];
            let idx0 = i;
            if token == "--" {
                if tokens_requested {
                    let t = parseargs_terminator_token(rt, idx0);
                    rt.object_set(tokens_arr, ntok.to_string(), t);
                    ntok += 1;
                }
                if allow_positionals {
                    for (off, p) in raw_args.iter().skip(i + 1).enumerate() {
                        positionals.push(p.clone());
                        if tokens_requested {
                            let t = parseargs_positional_token(rt, i + 1 + off, p);
                            rt.object_set(tokens_arr, ntok.to_string(), t);
                            ntok += 1;
                        }
                    }
                }
                break;
            } else if let Some(rest) = token.strip_prefix("--") {

                let key = rest.split_once('=').map(|(k, _)| k).unwrap_or(rest);
                if strict && !known_long.contains(key) {
                    return Err(unknown_option_error(rt, &format!("--{key}")));
                }
                if let Some((key, value)) = rest.split_once('=') {
                    set_value_multi(v, rt, key, u_sval(value), long_multiple.contains(key));
                    if tokens_requested {
                        let t = parseargs_option_token(
                            rt,
                            key,
                            &format!("--{key}"),
                            idx0,
                            Some(value),
                            true,
                        );
                        rt.object_set(tokens_arr, ntok.to_string(), t);
                        ntok += 1;
                    }
                } else if long_type.get(key).map(|t| t == "string").unwrap_or(false)
                    && raw_args
                        .get(i + 1)
                        .is_some_and(|next| !next.starts_with('-'))
                {
                    i += 1;
                    set_value_multi(
                        v,
                        rt,
                        rest,
                        u_sval(&raw_args[i]),
                        long_multiple.contains(rest),
                    );
                    if tokens_requested {
                        let val = raw_args[i].clone();
                        let t = parseargs_option_token(
                            rt,
                            key,
                            &format!("--{key}"),
                            idx0,
                            Some(&val),
                            false,
                        );
                        rt.object_set(tokens_arr, ntok.to_string(), t);
                        ntok += 1;
                    }
                } else {
                    set_value_multi(
                        v,
                        rt,
                        rest,
                        Value::Boolean(true),
                        long_multiple.contains(rest),
                    );
                    if tokens_requested {
                        let t =
                            parseargs_option_token(rt, key, &format!("--{key}"), idx0, None, false);
                        rt.object_set(tokens_arr, ntok.to_string(), t);
                        ntok += 1;
                    }
                }
            } else if token.starts_with('-') && token.len() > 1 {

                let chars: Vec<char> = token.trim_start_matches('-').chars().collect();
                let mut idx = 0usize;
                while idx < chars.len() {
                    let ch = chars[idx];
                    let short = ch.to_string();
                    let key = short_to_long.get(&short).cloned().unwrap_or(short);
                    if strict && !known_long.contains(&key) {
                        return Err(unknown_option_error(rt, &format!("-{ch}")));
                    }
                    let is_string = long_type.get(&key).map(|t| t == "string").unwrap_or(false);
                    let raw_name = format!("-{ch}");
                    if is_string {

                        let attached: String = chars[idx + 1..].iter().collect();
                        if !attached.is_empty() {
                            set_value_multi(
                                v,
                                rt,
                                &key,
                                u_sval(&attached),
                                long_multiple.contains(&key),
                            );
                            if tokens_requested {
                                let t = parseargs_option_token(
                                    rt,
                                    &key,
                                    &raw_name,
                                    idx0,
                                    Some(&attached),
                                    false,
                                );
                                rt.object_set(tokens_arr, ntok.to_string(), t);
                                ntok += 1;
                            }
                            break;
                        }

                        if raw_args
                            .get(i + 1)
                            .is_some_and(|next| !next.starts_with('-'))
                        {
                            i += 1;
                            let val = raw_args[i].clone();
                            set_value_multi(
                                v,
                                rt,
                                &key,
                                u_sval(&val),
                                long_multiple.contains(&key),
                            );
                            if tokens_requested {
                                let t = parseargs_option_token(
                                    rt,
                                    &key,
                                    &raw_name,
                                    idx0,
                                    Some(&val),
                                    false,
                                );
                                rt.object_set(tokens_arr, ntok.to_string(), t);
                                ntok += 1;
                            }
                        } else {
                            set_value_multi(
                                v,
                                rt,
                                &key,
                                Value::Boolean(true),
                                long_multiple.contains(&key),
                            );
                            if tokens_requested {
                                let t =
                                    parseargs_option_token(rt, &key, &raw_name, idx0, None, false);
                                rt.object_set(tokens_arr, ntok.to_string(), t);
                                ntok += 1;
                            }
                        }
                        break;
                    }

                    set_value_multi(
                        v,
                        rt,
                        &key,
                        Value::Boolean(true),
                        long_multiple.contains(&key),
                    );
                    if tokens_requested {
                        let t = parseargs_option_token(rt, &key, &raw_name, idx0, None, false);
                        rt.object_set(tokens_arr, ntok.to_string(), t);
                        ntok += 1;
                    }
                    idx += 1;
                }
            } else if allow_positionals {
                positionals.push(token.clone());
                if tokens_requested {
                    let t = parseargs_positional_token(rt, idx0, token);
                    rt.object_set(tokens_arr, ntok.to_string(), t);
                    ntok += 1;
                }
            }
            i += 1;
        }

        for (key, default) in &long_default {
            if !rt.obj(v).has_own_str(key) {
                rt.object_set(v, key.clone(), default.clone());
            }
        }

        let p = set_array_strings(rt, &positionals);
        rt.object_set(o, "values".into(), Value::Object(v));
        rt.object_set(o, "positionals".into(), Value::Object(p));
        if tokens_requested {
            rt.object_set(tokens_arr, "length".into(), Value::Number(ntok as f64));
            rt.object_set(o, "tokens".into(), Value::Object(tokens_arr));
        }
        Ok(Value::Object(o))
    });
    register_method(rt, util, "parseEnv", |rt, _a| {
        Ok(Value::Object(new_object(rt)))
    });
    register_method(rt, util, "diff", |rt, _a| {
        Ok(Value::Object(
            rt.alloc_object(rusty_js_runtime::value::Object::new_array()),
        ))
    });
    register_method(rt, util, "transferableAbortController", |rt, _a| {
        let ctor = rt.global_get("AbortController");
        let controller = rt.construct(ctor, Vec::new())?;
        Ok(controller)
    });
    register_method(rt, util, "transferableAbortSignal", |_rt, args| {
        Ok(args.first().cloned().unwrap_or(Value::Undefined))
    });
    register_method(rt, util, "convertProcessSignalToExitCode", |_rt, _a| {
        Ok(Value::Number(0.0))
    });
    register_method(rt, util, "setTraceSigInt", |_rt, _a| Ok(Value::Undefined));
    register_method(rt, util, "_errnoException", |_rt, _a| {
        Err(RuntimeError::TypeError("util._errnoException".into()))
    });
    register_method(rt, util, "_exceptionWithHostPort", |_rt, _a| {
        Err(RuntimeError::TypeError(
            "util._exceptionWithHostPort".into(),
        ))
    });

    rt.define_global_property("util", Value::Object(util));
}

fn json_stringify_via_intrinsic(rt: &mut Runtime, v: &Value) -> Result<String, RuntimeError> {

    let json = match rt.global_get("JSON") {
        Value::Undefined => return Err(RuntimeError::TypeError("JSON intrinsic missing".into())),
        v => v,
    };
    let json_id = match json {
        Value::Object(id) => id,
        _ => return Err(RuntimeError::TypeError("JSON is not an object".into())),
    };
    let stringify = rt.object_get(json_id, "stringify");
    let s = rt.call_function(stringify, Value::Object(json_id), vec![v.clone()])?;
    Ok(match s {
        Value::String(s) => s.as_str().to_string(),
        _ => String::new(),
    })
}

fn mime_unescape_quoted(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            if let Some(n) = chars.next() {
                out.push(n);
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn mime_parse(input: &str) -> (String, String, Vec<(String, String)>) {
    let input = input.trim();
    let (essence, rest) = match input.find(';') {
        Some(i) => (&input[..i], &input[i + 1..]),
        None => (input, ""),
    };
    let essence = essence.trim();
    let (ty, sub) = match essence.find('/') {
        Some(i) => (
            essence[..i].trim().to_ascii_lowercase(),
            essence[i + 1..].trim().to_ascii_lowercase(),
        ),
        None => (essence.to_ascii_lowercase(), String::new()),
    };
    let mut params: Vec<(String, String)> = Vec::new();
    for part in rest.split(';') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let (name, value) = match part.find('=') {
            Some(i) => (
                part[..i].trim().to_ascii_lowercase(),
                part[i + 1..].trim().to_string(),
            ),
            None => continue,
        };
        if name.is_empty() {
            continue;
        }
        let value = if value.len() >= 2 && value.starts_with('"') && value.ends_with('"') {
            mime_unescape_quoted(&value[1..value.len() - 1])
        } else {
            value
        };
        if !params.iter().any(|(k, _)| k == &name) {
            params.push((name, value));
        }
    }
    (ty, sub, params)
}

fn mime_quote_value(v: &str) -> String {
    let is_token = !v.is_empty()
        && v.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&b));
    if is_token {
        v.to_string()
    } else {
        format!("\"{}\"", v.replace('\\', "\\\\").replace('"', "\\\""))
    }
}

fn mime_read_pairs(rt: &mut Runtime, params: ObjectRef) -> Vec<(String, String)> {
    let arr = match rt.object_get(params, "__pairs") {
        Value::Object(id) => id,
        _ => return Vec::new(),
    };
    let len = rt.array_length(arr);
    let mut out = Vec::new();
    for i in 0..len {
        if let Value::Object(pair) = rt.object_get(arr, &i.to_string()) {
            let k = match rt.object_get(pair, "0") {
                Value::String(s) => s.as_str().to_string(),
                _ => continue,
            };
            let v = match rt.object_get(pair, "1") {
                Value::String(s) => s.as_str().to_string(),
                _ => String::new(),
            };
            out.push((k, v));
        }
    }
    out
}

fn mime_write_pairs(rt: &mut Runtime, params: ObjectRef, pairs: &[(String, String)]) {
    let arr = rt.alloc_object(Object::new_array());
    for (i, (k, v)) in pairs.iter().enumerate() {
        let pair = rt.alloc_object(Object::new_array());
        rt.object_set(pair, "0".into(), util_string_value(k));
        rt.object_set(pair, "1".into(), util_string_value(v));
        rt.object_set(pair, "length".into(), Value::Number(2.0));
        rt.object_set(arr, i.to_string().into(), Value::Object(pair));
    }
    rt.object_set(arr, "length".into(), Value::Number(pairs.len() as f64));
    rt.object_set(params, "__pairs".into(), Value::Object(arr));
}

fn build_mime_params(rt: &mut Runtime, pairs: Vec<(String, String)>) -> ObjectRef {
    let params = new_object(rt);
    mime_write_pairs(rt, params, &pairs);
    register_method(rt, params, "get", |rt, a| {
        let this = match rt.current_this() {
            Value::Object(id) => id,
            _ => return Ok(Value::Null),
        };
        let name = match a.first() {
            Some(Value::String(s)) => s.as_str().to_ascii_lowercase(),
            Some(v) => rt.coerce_to_string(v)?.to_ascii_lowercase(),
            None => return Ok(Value::Null),
        };
        for (k, v) in mime_read_pairs(rt, this) {
            if k == name {
                return Ok(util_string_value(&v));
            }
        }
        Ok(Value::Null)
    });
    register_method(rt, params, "has", |rt, a| {
        let this = match rt.current_this() {
            Value::Object(id) => id,
            _ => return Ok(Value::Boolean(false)),
        };
        let name = match a.first() {
            Some(Value::String(s)) => s.as_str().to_ascii_lowercase(),
            Some(v) => rt.coerce_to_string(v)?.to_ascii_lowercase(),
            None => return Ok(Value::Boolean(false)),
        };
        Ok(Value::Boolean(
            mime_read_pairs(rt, this).iter().any(|(k, _)| k == &name),
        ))
    });
    register_method(rt, params, "set", |rt, a| {
        let this = match rt.current_this() {
            Value::Object(id) => id,
            _ => return Ok(Value::Undefined),
        };
        let name = match a.first() {
            Some(Value::String(s)) => s.as_str().to_ascii_lowercase(),
            Some(v) => rt.coerce_to_string(v)?.to_ascii_lowercase(),
            None => return Ok(Value::Undefined),
        };
        let value = match a.get(1) {
            Some(Value::String(s)) => s.as_str().to_string(),
            Some(v) => rt.coerce_to_string(v)?,
            None => String::new(),
        };
        let mut pairs = mime_read_pairs(rt, this);
        if let Some(p) = pairs.iter_mut().find(|(k, _)| k == &name) {
            p.1 = value;
        } else {
            pairs.push((name, value));
        }
        mime_write_pairs(rt, this, &pairs);
        Ok(Value::Undefined)
    });
    register_method(rt, params, "delete", |rt, a| {
        let this = match rt.current_this() {
            Value::Object(id) => id,
            _ => return Ok(Value::Undefined),
        };
        let name = match a.first() {
            Some(Value::String(s)) => s.as_str().to_ascii_lowercase(),
            Some(v) => rt.coerce_to_string(v)?.to_ascii_lowercase(),
            None => return Ok(Value::Undefined),
        };
        let mut pairs = mime_read_pairs(rt, this);
        pairs.retain(|(k, _)| k != &name);
        mime_write_pairs(rt, this, &pairs);
        Ok(Value::Undefined)
    });
    register_method(rt, params, "entries", |rt, _a| Ok(mime_make_iter(rt, 0)));
    register_method(rt, params, "keys", |rt, _a| Ok(mime_make_iter(rt, 1)));
    register_method(rt, params, "values", |rt, _a| Ok(mime_make_iter(rt, 2)));
    register_method(rt, params, "toString", |rt, _a| {
        let this = match rt.current_this() {
            Value::Object(id) => id,
            _ => return Ok(util_string_value("")),
        };
        let s = mime_read_pairs(rt, this)
            .iter()
            .map(|(k, v)| format!("{k}={}", mime_quote_value(v)))
            .collect::<Vec<_>>()
            .join(";");
        Ok(util_string_value(&s))
    });
    params
}

fn mime_make_iter(rt: &mut Runtime, mode: u8) -> Value {
    let this = match rt.current_this() {
        Value::Object(id) => id,
        _ => return Value::Undefined,
    };
    let pairs = mime_read_pairs(rt, this);
    let arr = rt.alloc_object(Object::new_array());
    for (i, (k, v)) in pairs.iter().enumerate() {
        let item = match mode {
            1 => util_string_value(k),
            2 => util_string_value(v),
            _ => {
                let pair = rt.alloc_object(Object::new_array());
                rt.object_set(pair, "0".into(), util_string_value(k));
                rt.object_set(pair, "1".into(), util_string_value(v));
                rt.object_set(pair, "length".into(), Value::Number(2.0));
                Value::Object(pair)
            }
        };
        rt.object_set(arr, i.to_string().into(), item);
    }
    rt.object_set(arr, "length".into(), Value::Number(pairs.len() as f64));
    Value::Object(arr)
}

fn parseargs_option_token(
    rt: &mut Runtime,
    name: &str,
    raw_name: &str,
    index: usize,
    value: Option<&str>,
    inline: bool,
) -> Value {
    let t = new_object(rt);
    rt.object_set(t, "kind".into(), util_string_value("option"));
    rt.object_set(t, "name".into(), util_string_value(name));
    rt.object_set(t, "rawName".into(), util_string_value(raw_name));
    rt.object_set(t, "index".into(), Value::Number(index as f64));
    match value {
        Some(s) => {
            rt.object_set(t, "value".into(), util_string_value(s));
            rt.object_set(t, "inlineValue".into(), Value::Boolean(inline));
        }
        None => {

            rt.object_set(t, "value".into(), Value::Undefined);
            rt.object_set(t, "inlineValue".into(), Value::Undefined);
        }
    }
    Value::Object(t)
}

fn parseargs_positional_token(rt: &mut Runtime, index: usize, value: &str) -> Value {
    let t = new_object(rt);
    rt.object_set(t, "kind".into(), util_string_value("positional"));
    rt.object_set(t, "index".into(), Value::Number(index as f64));
    rt.object_set(t, "value".into(), util_string_value(value));
    Value::Object(t)
}

fn parseargs_terminator_token(rt: &mut Runtime, index: usize) -> Value {
    let t = new_object(rt);
    rt.object_set(t, "kind".into(), util_string_value("option-terminator"));
    rt.object_set(t, "index".into(), Value::Number(index as f64));
    Value::Object(t)
}
