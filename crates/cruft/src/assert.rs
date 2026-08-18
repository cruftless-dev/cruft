
use crate::register::{
    make_callable, make_callable_rooted, new_object, register_method, set_constant,
};
use rusty_js_runtime::abstract_ops;
use rusty_js_runtime::promise::{new_promise, reject_promise, resolve_promise};
use rusty_js_runtime::value::{InternalKind, Object, ObjectRef, PropertyDescriptor, PropertyKey};
use rusty_js_runtime::{Runtime, RuntimeError, Value};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

fn assertion_error_proto(rt: &Runtime) -> Option<ObjectRef> {
    match rt.global_get("__node_assert") {
        Value::Object(assert) => match rt.object_get(assert, "AssertionError") {
            Value::Object(ctor) => match rt.object_get(ctor, "prototype") {
                Value::Object(proto) => Some(proto),
                _ => None,
            },
            _ => None,
        },
        _ => None,
    }
}

fn build_assertion_error(rt: &mut Runtime, msg: String) -> Value {
    let proto = assertion_error_proto(rt);
    let mut o = Object::new_ordinary();
    o.internal_kind = InternalKind::Error;
    if let Some(p) = proto {
        o.proto = Some(p);
    }
    let id = rt.alloc_object(o);
    rt.object_set(
        id,
        "name".into(),
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
            "AssertionError",
        ))),
    );
    rt.object_set(
        id,
        "message".into(),
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(msg))),
    );
    rt.object_set(
        id,
        "code".into(),
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
            "ERR_ASSERTION",
        ))),
    );
    Value::Object(id)
}

fn assertion_error(rt: &mut Runtime, msg: String) -> RuntimeError {
    RuntimeError::Thrown(build_assertion_error(rt, msg))
}

fn node_code_range_error(rt: &mut Runtime, code: &str, msg: &str) -> RuntimeError {
    match rusty_js_runtime::intrinsics::make_error_instance(rt, "RangeError", msg) {
        Some(id) => {
            rt.object_set(
                id,
                "code".into(),
                Value::String(Rc::new(rusty_js_runtime::value::JsString::from(code))),
            );
            RuntimeError::Thrown(Value::Object(id))
        }
        None => RuntimeError::RangeError(msg.to_string()),
    }
}

fn node_code_type_error(rt: &mut Runtime, code: &str, msg: &str) -> RuntimeError {
    match rusty_js_runtime::intrinsics::make_error_instance(rt, "TypeError", msg) {
        Some(id) => {
            rt.object_set(
                id,
                "code".into(),
                Value::String(Rc::new(rusty_js_runtime::value::JsString::from(code))),
            );
            RuntimeError::Thrown(Value::Object(id))
        }
        None => RuntimeError::TypeError(msg.to_string()),
    }
}

fn node_code_error(rt: &mut Runtime, code: &str, msg: &str) -> RuntimeError {
    match rusty_js_runtime::intrinsics::make_error_instance(rt, "Error", msg) {
        Some(id) => {
            rt.object_set(
                id,
                "code".into(),
                Value::String(Rc::new(rusty_js_runtime::value::JsString::from(code))),
            );
            RuntimeError::Thrown(Value::Object(id))
        }
        None => RuntimeError::Thrown(Value::String(Rc::new(
            rusty_js_runtime::value::JsString::from(msg),
        ))),
    }
}

fn node_code_type_error_value(rt: &mut Runtime, code: &str, msg: &str) -> Value {
    match rusty_js_runtime::intrinsics::make_error_instance(rt, "TypeError", msg) {
        Some(id) => {
            rt.object_set(
                id,
                "code".into(),
                Value::String(Rc::new(rusty_js_runtime::value::JsString::from(code))),
            );
            Value::Object(id)
        }
        None => Value::String(Rc::new(rusty_js_runtime::value::JsString::from(msg))),
    }
}

fn invalid_fn_arg_suffix(rt: &Runtime, v: &Value) -> String {
    match v {
        Value::String(s) => format!(" Received type string ('{}')", s.as_str()),
        Value::Number(_) => {
            format!(
                " Received type number ({})",
                abstract_ops::to_string(v).as_str()
            )
        }
        Value::Boolean(b) => format!(" Received type boolean ({b})"),
        Value::Null => " Received null".into(),
        Value::Undefined => " Received undefined".into(),
        Value::Object(id) => {
            let name = match &rt.obj(*id).internal_kind {
                InternalKind::Array => "Array",
                InternalKind::RegExp(_) => "RegExp",
                InternalKind::Function(_)
                | InternalKind::Closure(_)
                | InternalKind::BoundFunction(_) => "Function",
                _ => "Object",
            };
            format!(" Received an instance of {name}")
        }
        _ => format!(" Received {}", abstract_ops::to_string(v).as_str()),
    }
}

fn validate_assert_callable_fn(rt: &mut Runtime, f: &Value) -> Result<(), RuntimeError> {
    if rt.is_callable(f) {
        return Ok(());
    }
    let msg = format!(
        "The \"fn\" argument must be of type function.{}",
        invalid_fn_arg_suffix(rt, f)
    );
    Err(node_code_type_error(rt, "ERR_INVALID_ARG_TYPE", &msg))
}

fn invalid_options_arg_error(rt: &mut Runtime, v: &Value) -> RuntimeError {
    let suffix = match v {
        Value::String(s) => format!(" Received type string ('{}')", s.as_str()),
        Value::Number(_) => {
            format!(
                " Received type number ({})",
                abstract_ops::to_string(v).as_str()
            )
        }
        Value::Boolean(b) => format!(" Received type boolean ({b})"),
        Value::Symbol(_) => format!(
            " Received type symbol ({})",
            abstract_ops::to_string(v).as_str()
        ),
        Value::Null => " Received null".into(),
        Value::Undefined => " Received undefined".into(),
        Value::Object(id) => {
            let name = match &rt.obj(*id).internal_kind {
                InternalKind::Array => "Array",
                InternalKind::RegExp(_) => "RegExp",
                _ => "Object",
            };
            format!(" Received an instance of {name}")
        }
        _ => format!(" Received {}", abstract_ops::to_string(v).as_str()),
    };
    node_code_type_error(
        rt,
        "ERR_INVALID_ARG_TYPE",
        &format!("The \"options\" argument must be of type object.{suffix}"),
    )
}

fn assert_error_with_fields(
    rt: &mut Runtime,
    msg: String,
    actual: Value,
    expected: Value,
    operator: &str,
    generated: bool,
    diff: Option<&str>,
) -> RuntimeError {
    let err = build_assertion_error(rt, msg);
    if let Value::Object(id) = &err {
        rt.object_set(*id, "actual".into(), actual);
        rt.object_set(*id, "expected".into(), expected);
        rt.object_set(
            *id,
            "operator".into(),
            Value::String(Rc::new(rusty_js_runtime::value::JsString::from(operator))),
        );
        rt.object_set(*id, "generatedMessage".into(), Value::Boolean(generated));
        rt.object_set(
            *id,
            "diff".into(),
            Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                diff.unwrap_or("simple"),
            ))),
        );
    }
    RuntimeError::Thrown(err)
}

fn assert_instance_diff(rt: &Runtime) -> Option<String> {
    match rt.current_this() {
        Value::Object(id) => match rt.object_get(id, "__assert_diff") {
            Value::String(s) => Some(s.as_str().to_string()),
            _ => None,
        },
        _ => None,
    }
}

fn assert_instance_skip_prototype(rt: &Runtime) -> bool {
    match rt.current_this() {
        Value::Object(id) => matches!(
            rt.object_get(id, "__assert_skip_prototype"),
            Value::Boolean(true)
        ),
        _ => false,
    }
}

fn missing_args_error(rt: &mut Runtime) -> RuntimeError {
    node_code_type_error(
        rt,
        "ERR_MISSING_ARGS",
        "The \"actual\" and \"expected\" arguments must be specified",
    )
}

fn copy_assert_method(
    rt: &mut Runtime,
    dst: ObjectRef,
    src: ObjectRef,
    src_name: &str,
    dst_name: &str,
) {
    let method = rt.object_get(src, src_name);
    rt.object_set(dst, dst_name.to_string(), method);
}

fn install_assert_instance_surface(
    rt: &mut Runtime,
    instance: ObjectRef,
    assert: ObjectRef,
    strict: ObjectRef,
    assertion_error_ctor: ObjectRef,
    strict_mode: bool,
    diff_mode: &str,
    skip_prototype: bool,
) {
    let method_src = if strict_mode { strict } else { assert };
    for name in [
        "ok",
        "equal",
        "notEqual",
        "strictEqual",
        "notStrictEqual",
        "deepEqual",
        "notDeepEqual",
        "deepStrictEqual",
        "notDeepStrictEqual",
        "partialDeepStrictEqual",
        "throws",
        "doesNotThrow",
        "ifError",
        "match",
        "doesNotMatch",
        "rejects",
        "doesNotReject",
        "fail",
    ] {
        copy_assert_method(rt, instance, method_src, name, name);
    }
    rt.object_set(
        instance,
        "__assert_diff".into(),
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(diff_mode))),
    );
    rt.object_set(
        instance,
        "__assert_skip_prototype".into(),
        Value::Boolean(skip_prototype),
    );
    rt.object_set(
        instance,
        "AssertionError".into(),
        Value::Object(assertion_error_ctor),
    );
    rt.object_set(instance, "strict".into(), Value::Object(strict));
}

fn arg_msg(args: &[Value], idx: usize, fallback: &str) -> String {
    match args.get(idx) {
        Some(Value::String(s)) => {
            let mut out = String::new();
            let mut chars = s.as_str().chars().peekable();
            let mut subst = args.iter().skip(idx + 1);
            while let Some(ch) = chars.next() {
                if ch == '%' {
                    match chars.peek().copied() {
                        Some('%') => {
                            chars.next();
                            out.push('%');
                        }
                        Some(spec @ ('i' | 'd' | 's')) => {
                            chars.next();
                            if let Some(v) = subst.next() {
                                out.push_str(abstract_ops::to_string(v).as_str());
                            } else {
                                out.push('%');
                                out.push(spec);
                            }
                        }
                        _ => out.push(ch),
                    }
                } else {
                    out.push(ch);
                }
            }
            out
        }
        Some(other) => abstract_ops::to_string(other).as_str().to_string(),
        None => fallback.to_string(),
    }
}

fn assert_scalar_display(value: &Value) -> String {
    match value {
        Value::Null => "null".into(),
        Value::Undefined => "undefined".into(),
        Value::String(s) => format!("'{}'", s.as_str()),
        Value::Number(n) if n.is_infinite() && n.is_sign_positive() => "Infinity".into(),
        Value::Number(n) if n.is_infinite() && n.is_sign_negative() => "-Infinity".into(),
        Value::Number(n) if n.fract() == 0.0 => format!("{}", *n as i64),
        Value::Number(n) => n.to_string(),
        Value::Boolean(b) => b.to_string(),
        _ => abstract_ops::to_string(value).as_str().to_string(),
    }
}

fn custom_error_message_arg(rt: &Runtime, args: &[Value], idx: usize) -> Option<Value> {
    let Some(Value::Object(id)) = args.get(idx) else {
        return None;
    };
    if matches!(rt.obj(*id).internal_kind, InternalKind::Error)
        || assert_object_looks_error_branded(rt, *id)
    {
        Some(Value::Object(*id))
    } else {
        None
    }
}

fn assert_object_looks_error_branded(rt: &Runtime, id: ObjectRef) -> bool {
    let has_message = matches!(rt.object_get(id, "message"), Value::String(_));
    if !has_message {
        return false;
    }
    let name = match rt.object_get(id, "name") {
        Value::String(s) => s.as_str().to_string(),
        _ => rt
            .obj(id)
            .proto
            .and_then(|proto| match rt.object_get(proto, "constructor") {
                Value::Object(ctor) => match rt.object_get(ctor, "name") {
                    Value::String(s) => Some(s.as_str().to_string()),
                    _ => None,
                },
                _ => None,
            })
            .unwrap_or_default(),
    };
    name == "Error" || name.ends_with("Error")
}

fn assert_quote_string(s: &str) -> String {
    format!("'{s}'")
}

fn assert_regex_default_msg(rt: &Runtime, args: &[Value], negated: bool) -> String {
    let regex_display = match args.get(1) {
        Some(Value::Object(id)) => match regexp_signature(rt, *id) {
            Some((src, flags)) => format!("/{src}/{flags}"),
            None => String::from("regexp"),
        },
        _ => String::from("regexp"),
    };
    let input_display = match args.first() {
        Some(Value::String(s)) => assert_quote_string(s.as_str()),
        _ => String::new(),
    };
    let lead = if negated {
        "The input was expected to not match the regular expression"
    } else {
        "The input did not match the regular expression"
    };
    format!("{lead} {regex_display}. Input:\n\n{input_display}\n")
}

fn assert_multiline_diff_lines(s: &str, marker: &str, limit: Option<usize>) -> String {
    let mut out = Vec::new();
    let lines = s.split_inclusive('\n');
    for (idx, line) in lines.take(limit.unwrap_or(usize::MAX)).enumerate() {
        let escaped = line.replace('\n', "\\n");
        let pad = if idx == 0 { "" } else { "  " };
        out.push(format!("{marker} {pad}'{escaped}' +"));
    }
    out.join("\n")
}

fn assert_simple_truncate(s: &str) -> Option<String> {
    const LIMIT: usize = 508;
    if s.chars().count() <= LIMIT {
        return None;
    }
    let mut out: String = s.chars().take(LIMIT).collect();
    out.push_str("...");
    Some(out)
}

const MYERS_DELETE: i8 = -1;
const MYERS_NOP: i8 = 0;
const MYERS_INSERT: i8 = 1;
const MYERS_NOP_COLLAPSE: i32 = 5;

fn myers_lines_equal(a: &str, b: &str, check_comma: bool) -> bool {
    if a == b {
        return true;
    }
    if check_comma {

        let mut ac = String::with_capacity(a.len() + 1);
        ac.push_str(a);
        ac.push(',');
        if ac == b {
            return true;
        }
        let mut bc = String::with_capacity(b.len() + 1);
        bc.push_str(b);
        bc.push(',');
        return a == bc;
    }
    false
}

fn myers_diff(actual: &[&str], expected: &[&str], check_comma: bool) -> Vec<(i8, String)> {
    let n = actual.len() as i64;
    let m = expected.len() as i64;
    let max = n + m;
    if max == 0 {
        return Vec::new();
    }
    let voff = max;
    let mut v = vec![0i32; (2 * max + 1) as usize];
    let mut trace: Vec<Vec<i32>> = Vec::new();
    for d in 0..=max {
        trace.push(v.clone());
        let mut k = -d;
        while k <= d {
            let offset = (k + voff) as usize;

            let take_next = k == -d || (k != d && v[offset - 1] < v[offset + 1]);
            let mut x = if take_next {
                v[offset + 1] as i64
            } else {
                v[offset - 1] as i64 + 1
            };
            let mut y = x - k;
            while x < n
                && y < m
                && myers_lines_equal(actual[x as usize], expected[y as usize], check_comma)
            {
                x += 1;
                y += 1;
            }
            v[offset] = x as i32;
            if x >= n && y >= m {
                return myers_backtrack(&trace, actual, expected, check_comma, voff);
            }
            k += 2;
        }
    }
    Vec::new()
}

fn myers_backtrack(
    trace: &[Vec<i32>],
    actual: &[&str],
    expected: &[&str],
    check_comma: bool,
    voff: i64,
) -> Vec<(i8, String)> {
    let mut x = actual.len() as i64;
    let mut y = expected.len() as i64;
    let mut result: Vec<(i8, String)> = Vec::new();
    for d in (0..trace.len()).rev() {
        let v = &trace[d];
        let k = x - y;
        let offset = (k + voff) as usize;
        let dd = d as i64;
        let prev_k = if k == -dd || (k != dd && v[offset - 1] < v[offset + 1]) {
            k + 1
        } else {
            k - 1
        };
        let prev_x = v[(prev_k + voff) as usize] as i64;
        let prev_y = prev_x - prev_k;
        while x > prev_x && y > prev_y {
            let actual_item = actual[(x - 1) as usize];

            let value = if check_comma && !actual_item.ends_with(',') {
                expected[(y - 1) as usize]
            } else {
                actual_item
            };
            result.push((MYERS_NOP, value.to_string()));
            x -= 1;
            y -= 1;
        }
        if d > 0 {
            if x > prev_x {
                x -= 1;
                result.push((MYERS_INSERT, actual[x as usize].to_string()));
            } else {
                y -= 1;
                result.push((MYERS_DELETE, expected[y as usize].to_string()));
            }
        }
    }
    result
}

fn print_myers_diff(diff: &[(i8, String)]) -> (String, bool) {
    let mut message = String::new();
    let mut skipped = false;
    let mut nop_count: i32 = 0;
    let n = diff.len();
    for idx in (0..n).rev() {
        let (op, value) = (diff[idx].0, &diff[idx].1);
        let prev_op = if idx < n - 1 {
            Some(diff[idx + 1].0)
        } else {
            None
        };
        if prev_op == Some(MYERS_NOP) && op != MYERS_NOP {
            if nop_count == MYERS_NOP_COLLAPSE + 1 {
                message.push_str(&format!("  {}\n", diff[idx + 1].1));
            } else if nop_count == MYERS_NOP_COLLAPSE + 2 {
                message.push_str(&format!("  {}\n", diff[idx + 2].1));
                message.push_str(&format!("  {}\n", diff[idx + 1].1));
            } else if nop_count >= MYERS_NOP_COLLAPSE + 3 {
                message.push_str("...\n");
                message.push_str(&format!("  {}\n", diff[idx + 1].1));
                skipped = true;
            }
            nop_count = 0;
        }
        if op == MYERS_INSERT {
            message.push_str(&format!("+ {}\n", value));
        } else if op == MYERS_DELETE {
            message.push_str(&format!("- {}\n", value));
        } else {
            if nop_count < MYERS_NOP_COLLAPSE {
                message.push_str(&format!("  {}\n", value));
            }
            nop_count += 1;
        }
    }
    let trimmed = message.trim_end().to_string();
    (format!("\n{}", trimmed), skipped)
}

fn assert_is_zero_number(v: &Value) -> bool {
    matches!(v, Value::Number(n) if *n == 0.0)
}

fn assert_simple_diff(
    actual: &Value,
    actual_str: &str,
    expected: &Value,
    expected_str: &str,
) -> (String, bool) {
    let mut strings_len = actual_str.chars().count() + expected_str.chars().count();

    if matches!(actual, Value::String(_)) {
        strings_len = strings_len.saturating_sub(2);
    }
    if matches!(expected, Value::String(_)) {
        strings_len = strings_len.saturating_sub(2);
    }

    let not_both_zero = !(assert_is_zero_number(actual) && assert_is_zero_number(expected));
    if strings_len <= 12 && not_both_zero {
        return (format!("{} !== {}", actual_str, expected_str), false);
    }

    let mut message = format!("\n+ {}\n- {}", actual_str, expected_str);
    if actual_str.chars().count() + expected_str.chars().count() <= 80 {
        let ac: Vec<char> = actual_str.chars().collect();
        let ec: Vec<char> = expected_str.chars().collect();
        let mut indicator_idx: i64 = -1;
        for i in 0..ac.len() {
            if i >= ec.len() || ac[i] != ec[i] {
                if i >= 3 {
                    indicator_idx = i as i64;
                }
                break;
            }
        }
        if indicator_idx != -1 {
            message.push('\n');
            message.push_str(&" ".repeat((indicator_idx + 2) as usize));
            message.push('^');
        }
    }
    (message, true)
}

fn assert_loose_deep_diff(rt: &mut Runtime, actual: &Value, expected: &Value) -> Option<String> {
    let ia = crate::util::inspect_for_assert_diff(rt, actual)?;
    let ib = crate::util::inspect_for_assert_diff(rt, expected)?;
    Some(format!(
        "Expected values to be loosely deep-equal:\n\n{}\n\nshould loosely deep-equal\n\n{}",
        ia, ib
    ))
}

fn assert_negated_single_message(
    rt: &mut Runtime,
    operator: &str,
    actual: &Value,
) -> Option<String> {
    let mut base = match operator {
        "notDeepStrictEqual" => "Expected \"actual\" not to be strictly deep-equal to:",
        "notStrictEqual" => "Expected \"actual\" to be strictly unequal to:",
        _ => return None,
    };

    if operator == "notStrictEqual" && matches!(actual, Value::Object(_)) {
        base = "Expected \"actual\" not to be reference-equal to \"expected\":";
    }
    let ia = crate::util::inspect_for_assert_diff(rt, actual)?;
    let lines: Vec<&str> = ia.split('\n').collect();
    if lines.len() == 1 {
        let sep = if lines[0].chars().count() > 5 {
            "\n\n"
        } else {
            " "
        };
        Some(format!("{}{}{}", base, sep, lines[0]))
    } else {
        Some(format!("{}\n\n{}\n", base, ia))
    }
}

fn assert_not_loose_deep_message(
    rt: &mut Runtime,
    actual: &Value,
    expected: &Value,
) -> Option<String> {
    let ia = crate::util::inspect_for_assert_diff(rt, actual)?;
    let ib = crate::util::inspect_for_assert_diff(rt, expected)?;
    if ia == ib {
        Some(format!(
            "Expected \"actual\" not to be loosely deep-equal to:\n\n{}",
            ia
        ))
    } else {
        Some(format!(
            "Expected values not to be loosely deep-equal:\n\n{}\n\nshould not loosely deep-equal\n\n{}",
            ia, ib
        ))
    }
}

fn assert_readable_operator(operator: &str) -> Option<&'static str> {
    match operator {
        "deepStrictEqual" => Some("Expected values to be strictly deep-equal:"),
        "strictEqual" => Some("Expected values to be strictly equal:"),

        "strictEqualObject" => Some("Expected \"actual\" to be reference-equal to \"expected\":"),
        _ => None,
    }
}

fn assert_structural_diff(
    rt: &mut Runtime,
    actual: &Value,
    expected: &Value,
    operator: &str,
    custom_message: Option<&str>,
) -> Option<String> {
    let header_op = assert_readable_operator(operator)?;

    let header = custom_message.unwrap_or(header_op);
    let ia = crate::util::inspect_for_assert_diff(rt, actual)?;
    let ib = crate::util::inspect_for_assert_diff(rt, expected)?;
    let la: Vec<&str> = ia.split('\n').collect();
    let lb: Vec<&str> = ib.split('\n').collect();

    let actual_is_object = matches!(actual, Value::Object(_)) && !rt.is_callable(actual);
    let expected_is_object = matches!(expected, Value::Object(_)) && !rt.is_callable(expected);
    let is_simple = !(la.len() > 1 || lb.len() > 1) && (!actual_is_object || !expected_is_object);
    if is_simple {

        let (body, uses_legend) = assert_simple_diff(
            actual,
            la.first().copied().unwrap_or(""),
            expected,
            lb.first().copied().unwrap_or(""),
        );
        if uses_legend {

            return Some(format!("{}\n+ actual - expected\n{}\n", header, body));
        }
        return Some(format!("{}\n\n{}\n", header, body));
    }

    if ia == ib {

        let ni_header =
            custom_message.unwrap_or("Values have same structure but are not reference-equal:");
        return Some(format!("{}\n\n{}\n", ni_header, ia));
    }
    let diff = myers_diff(&la, &lb, actual_is_object);
    let (body, skipped) = print_myers_diff(&diff);
    let skipped_msg = if skipped { "\n... Skipped lines" } else { "" };

    Some(format!(
        "{}\n+ actual - expected{}\n{}\n",
        header, skipped_msg, body
    ))
}

fn generated_assert_message(
    actual: &Value,
    expected: &Value,
    operator: &str,
    diff: Option<&str>,
    fallback: &str,
) -> String {
    match (operator, actual, expected) {
        ("strictEqual", Value::String(a), Value::String(b))
            if a.as_str().contains('\n') || b.as_str().contains('\n') =>
        {
            format!(
                "Expected values to be strictly equal:\n+ actual - expected\n\n{}\n{}\n",
                assert_multiline_diff_lines(a.as_str(), "+", None),
                assert_multiline_diff_lines(b.as_str(), "-", None)
            )
        }
        ("strictEqual", Value::String(a), Value::String(b))
            if (a.as_str().is_empty() || b.as_str().is_empty())
                && a.as_str().chars().count() + b.as_str().chars().count() <= 20 =>
        {
            format!(
                "Expected values to be strictly equal:\n\n{} !== {}\n",
                assert_quote_string(a.as_str()),
                assert_quote_string(b.as_str())
            )
        }
        ("strictEqual", Value::String(a), Value::String(b)) => {
            format!(
                "Expected values to be strictly equal:\n+ actual - expected\n\n+ {}\n- {}\n",
                assert_quote_string(a.as_str()),
                assert_quote_string(b.as_str())
            )
        }
        ("strictEqual", _, _) => {
            format!(
                "Expected values to be strictly equal:\n\n{} !== {}\n",
                assert_scalar_display(actual),
                assert_scalar_display(expected)
            )
        }
        ("notStrictEqual", Value::String(a), Value::String(_)) if a.as_str().contains('\n') => {
            let limit = if matches!(diff, Some("simple")) {
                Some(47)
            } else {
                None
            };
            format!(
                "Expected \"actual\" to be strictly unequal to:\n\n{}\n",
                assert_multiline_diff_lines(a.as_str(), "", limit)
            )
        }
        ("notStrictEqual", Value::String(a), Value::String(_)) => {
            format!(
                "Expected \"actual\" to be strictly unequal to:\n\n{}",
                assert_quote_string(a.as_str())
            )
        }
        ("notStrictEqual", _, _) => {
            format!(
                "Expected \"actual\" to be strictly unequal to: {}",
                assert_scalar_display(actual)
            )
        }
        ("deepEqual", Value::String(a), Value::String(b)) => {
            if a.as_str().contains('\n') || b.as_str().contains('\n') {
                let limit = if matches!(diff, Some("simple")) {
                    Some(52)
                } else {
                    None
                };
                return format!(
                    "Expected values to be loosely deep-equal:\n\n{}\n\nshould loosely deep-equal\n\n{}",
                    assert_multiline_diff_lines(a.as_str(), "", limit),
                    assert_multiline_diff_lines(b.as_str(), "", limit)
                );
            }
            let (a, b, already_quoted) = if matches!(diff, Some("simple")) {
                match (
                    assert_simple_truncate(a.as_str()),
                    assert_simple_truncate(b.as_str()),
                ) {
                    (Some(a), Some(b)) => (format!("'{a}"), format!("'{b}"), true),
                    _ => (a.as_str().to_string(), b.as_str().to_string(), false),
                }
            } else {
                (a.as_str().to_string(), b.as_str().to_string(), false)
            };
            let a = if already_quoted {
                a
            } else {
                assert_quote_string(&a)
            };
            let b = if already_quoted {
                b
            } else {
                assert_quote_string(&b)
            };
            format!(
                "Expected values to be loosely deep-equal:\n\n{}\n\nshould loosely deep-equal\n\n{}",
                a, b
            )
        }

        ("deepStrictEqual", a, b)
            if matches!(a, Value::String(_) | Value::Number(_) | Value::Boolean(_))
                && matches!(b, Value::String(_) | Value::Number(_) | Value::Boolean(_))
                && !matches!((a, b), (Value::Number(x), Value::Number(y)) if x == y) =>
        {
            format!(
                "Expected values to be strictly deep-equal:\n\n{} !== {}\n",
                assert_scalar_display(a),
                assert_scalar_display(b)
            )
        }
        _ => fallback.to_string(),
    }
}

fn if_error_display(rt: &Runtime, value: &Value) -> String {
    if let Value::Object(id) = value {
        let message = match rt.object_get(*id, "message") {
            Value::String(s) => s.as_str().to_string(),
            _ => String::new(),
        };
        if !message.is_empty() {
            return message;
        }
        if let Value::String(name) = rt.object_get(*id, "name") {
            let name = name.as_str();
            if !name.is_empty() && name != "Error" {
                return name.to_string();
            }
        }
    }
    abstract_ops::to_string(value).as_str().to_string()
}

fn async_assert_value_name(rt: &Runtime, v: &Value) -> String {
    match v {
        Value::Undefined => "undefined".to_string(),
        Value::Null => "null".to_string(),
        Value::Boolean(b) => b.to_string(),
        Value::Number(n) => {
            if n.fract() == 0.0 {
                format!("{}", *n as i64)
            } else {
                abstract_ops::to_string(v).as_str().to_string()
            }
        }
        Value::String(s) => format!("'{}'", s.as_str()),
        Value::Object(id) => match rt.object_get(*id, "constructor") {
            Value::Object(ctor) => match rt.object_get(ctor, "name") {
                Value::String(name) if !name.as_str().is_empty() => {
                    format!("an instance of {}", name.as_str())
                }
                _ => "an instance of Object".to_string(),
            },
            _ => "an instance of Object".to_string(),
        },
        _ => abstract_ops::to_string(v).as_str().to_string(),
    }
}

fn async_assert_invalid_arg_value(rt: &mut Runtime, v: &Value) -> Value {
    let type_name = match v {
        Value::String(_) => "string",
        Value::Number(_) => "number",
        Value::Boolean(_) => "boolean",
        Value::Undefined => "undefined",
        Value::Null => "null",
        Value::Object(_) => "Object",
        _ => "value",
    };
    let msg = format!(
        "The \"promiseFn\" argument must be of type function or an instance of Promise. Received type {} ({})",
        type_name,
        async_assert_value_name(rt, v)
    );
    node_code_type_error_value(rt, "ERR_INVALID_ARG_TYPE", &msg)
}

fn async_assert_invalid_return_value(rt: &mut Runtime, v: &Value) -> Value {
    let msg = format!(
        "Expected instance of Promise to be returned from the \"promiseFn\" function but got {}.",
        async_assert_value_name(rt, v)
    );
    node_code_type_error_value(rt, "ERR_INVALID_RETURN_VALUE", &msg)
}

fn is_promise_object(rt: &Runtime, v: &Value) -> bool {
    matches!(
        v,
        Value::Object(id) if matches!(rt.obj(*id).internal_kind, InternalKind::Promise(_))
    )
}

fn accepted_async_assert_subject(rt: &Runtime, v: &Value) -> bool {
    if rt.is_callable(v) {
        return false;
    }
    if is_promise_object(rt, v) {
        return true;
    }
    let Value::Object(id) = v else {
        return false;
    };
    rt.is_callable(&rt.object_get(*id, "then")) && rt.is_callable(&rt.object_get(*id, "catch"))
}

fn assert_rejects_expected_matches(rt: &Runtime, reason: &Value, expected: &Value) -> bool {
    match expected {
        Value::Undefined => true,
        Value::Object(_) if rt.is_callable(expected) => true,
        Value::Object(expected_id) => {
            let Value::Object(reason_id) = reason else {
                return false;
            };
            for key in rt.ordinary_own_enumerable_string_keys(*expected_id) {
                let ev = rt.object_get(*expected_id, &key);
                let rv = rt.object_get(*reason_id, &key);
                if !deep_equal(rt, &rv, &ev, true) {
                    return false;
                }
            }
            true
        }
        _ => true,
    }
}

fn assert_rejects_mismatch_error(
    rt: &mut Runtime,
    reason: Value,
    expected: Value,
    message: Option<String>,
) -> Value {
    let generated = message.is_none();
    let msg =
        message.unwrap_or_else(|| "The promise was rejected with an unexpected reason.".into());
    let err = build_assertion_error(rt, msg);
    if let Value::Object(id) = &err {
        rt.object_set(*id, "actual".into(), reason);
        rt.object_set(*id, "expected".into(), expected);
        rt.object_set(
            *id,
            "operator".into(),
            Value::String(Rc::new(rusty_js_runtime::value::JsString::from("rejects"))),
        );
        rt.object_set(*id, "generatedMessage".into(), Value::Boolean(generated));
        rt.object_set(
            *id,
            "stack".into(),
            Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                "AssertionError [ERR_ASSERTION]\n    at Function.rejects",
            ))),
        );
    }
    err
}

fn assert_rejects_caught_display(rt: &Runtime, reason: &Value) -> String {
    if let Value::Object(id) = reason {
        let name = match rt.object_get(*id, "name") {
            Value::String(s) if !s.as_str().is_empty() => s.as_str().to_string(),
            _ => "Error".to_string(),
        };
        let message = match rt.object_get(*id, "message") {
            Value::String(s) => s.as_str().to_string(),
            _ => String::new(),
        };
        if !message.is_empty() {
            return format!("{name}: {message}");
        }
        return name;
    }
    abstract_ops::to_string(reason).as_str().to_string()
}

fn assert_matcher_is_error_constructor(
    rt: &Runtime,
    matcher: ObjectRef,
    prototype: ObjectRef,
) -> bool {
    let error_proto = match rt.global_get("Error") {
        Value::Object(error_ctor) => match rt.object_get(error_ctor, "prototype") {
            Value::Object(proto) => proto,
            _ => return false,
        },
        _ => return false,
    };
    if prototype == error_proto {
        return rt.is_callable(&Value::Object(matcher));
    }
    let mut cur = rt.obj(prototype).proto;
    while let Some(id) = cur {
        if id == error_proto {
            return rt.is_callable(&Value::Object(matcher));
        }
        cur = rt.obj(id).proto;
    }
    false
}

fn assert_error_name_message(rt: &Runtime, value: &Value) -> Option<(String, String)> {
    let Value::Object(id) = value else {
        return None;
    };
    let name = match rt.object_get(*id, "name") {
        Value::String(s) if !s.as_str().is_empty() => s.as_str().to_string(),
        _ => rt
            .obj(*id)
            .proto
            .and_then(|proto| match rt.object_get(proto, "constructor") {
                Value::Object(ctor) => match rt.object_get(ctor, "name") {
                    Value::String(s) if !s.as_str().is_empty() => Some(s.as_str().to_string()),
                    _ => None,
                },
                _ => None,
            })
            .unwrap_or_else(|| "Error".to_string()),
    };
    let message = match rt.object_get(*id, "message") {
        Value::String(s) => s.as_str().to_string(),
        _ => String::new(),
    };
    Some((name, message))
}

fn does_not_throw_assertion_message(rt: &Runtime, args: &[Value], thrown: &Value) -> String {

    let actual_message = match assert_error_name_message(rt, thrown) {
        Some((_, message)) => message,
        None => abstract_ops::to_string(thrown).as_str().to_string(),
    };
    let details = match args.get(2) {
        Some(m) if !matches!(m, Value::Undefined) => {
            format!(": {}", abstract_ops::to_string(m).as_str())
        }
        _ => ".".to_string(),
    };
    format!("Got unwanted exception{details}\nActual message: \"{actual_message}\"")
}

fn assert_throws_matcher_accepts(
    rt: &mut Runtime,
    thrown: &Value,
    matcher: &Value,
) -> Result<bool, RuntimeError> {
    if matches!(matcher, Value::Undefined) {
        return Ok(true);
    }
    if let Value::Object(id) = matcher {
        if matches!(rt.obj(*id).internal_kind, InternalKind::RegExp(_)) {
            let input = match assert_error_name_message(rt, thrown) {
                Some((name, message)) if !message.is_empty() => Value::String(Rc::new(
                    rusty_js_runtime::value::JsString::from(format!("{name}: {message}")),
                )),
                Some((name, _)) => Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                    name.as_str(),
                ))),
                None => Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                    abstract_ops::to_string(thrown).as_str(),
                ))),
            };
            let test = rt.object_get(*id, "test");
            let verdict = rt.call_function(test, Value::Object(*id), vec![input])?;
            return Ok(abstract_ops::to_boolean(&verdict));
        }
        let prototype = rt.object_get(*id, "prototype");
        if let Value::Object(proto) = prototype {
            if assert_matcher_is_error_constructor(rt, *id, proto) {
                return rt.ordinary_has_instance(thrown, matcher);
            }
            if rt.global_get("Array") == *matcher {
                return rt.ordinary_has_instance(thrown, matcher);
            }
            if rt.ordinary_has_instance(thrown, matcher)? {
                return Ok(true);
            }
        }
        if matches!(
            rt.obj(*id).internal_kind,
            InternalKind::Ordinary | InternalKind::Error
        ) && !assert_object_matcher_is_assertion_expectation(rt, thrown, *id)
        {
            return assert_object_matcher_accepts(rt, thrown, *id);
        }
    }
    if rt.is_callable(matcher) {
        let verdict = rt.call_function(matcher.clone(), Value::Undefined, vec![thrown.clone()])?;
        return Ok(matches!(verdict, Value::Boolean(true)));
    }
    Ok(true)
}

fn assert_object_matcher_is_assertion_expectation(
    rt: &Runtime,
    thrown: &Value,
    id: ObjectRef,
) -> bool {
    let keys = rt.ordinary_own_enumerable_string_keys(id);
    let thrown_is_assertion_error = matches!(
        thrown,
        Value::Object(actual)
            if matches!(rt.object_get(*actual, "name"), Value::String(s) if s.as_str() == "AssertionError")
                || matches!(rt.object_get(*actual, "code"), Value::String(s) if s.as_str() == "ERR_ASSERTION")
    );
    matches!(rt.object_get(id, "code"), Value::String(s) if s.as_str().starts_with("ERR_"))
        || matches!(rt.object_get(id, "generatedMessage"), Value::Boolean(_))
        || rt.obj(id).has_own_str("operator")
        || matches!(rt.object_get(id, "name"), Value::String(s) if s.as_str() == "AssertionError")
        || (thrown_is_assertion_error && keys.len() == 1 && keys[0] == "message")
}

fn assert_regexp_value_accepts(
    rt: &mut Runtime,
    matcher: ObjectRef,
    value: &Value,
) -> Result<bool, RuntimeError> {
    let input = Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
        abstract_ops::to_string(value).as_str(),
    )));
    let test = rt.object_get(matcher, "test");
    let verdict = rt.call_function(test, Value::Object(matcher), vec![input])?;
    Ok(abstract_ops::to_boolean(&verdict))
}

fn assert_object_matcher_actual(rt: &Runtime, thrown: &Value, key: &str) -> Option<Value> {
    let Value::Object(id) = thrown else {
        return None;
    };
    if matches!(key, "name" | "message") {
        if let Some((name, message)) = assert_error_name_message(rt, thrown) {
            let value = if key == "name" { name } else { message };
            return Some(Value::String(Rc::new(
                rusty_js_runtime::value::JsString::from(value.as_str()),
            )));
        }
    }
    if rt.obj(*id).has_own_str(key) {
        return Some(rt.object_get(*id, key));
    }
    let inherited = rt.object_get(*id, key);
    if matches!(inherited, Value::Undefined) {
        None
    } else {
        Some(inherited)
    }
}

fn assert_object_matcher_accepts(
    rt: &mut Runtime,
    thrown: &Value,
    expected_id: ObjectRef,
) -> Result<bool, RuntimeError> {
    if !matches!(thrown, Value::Object(_)) {
        return Ok(false);
    }
    let mut keys = rt.ordinary_own_enumerable_string_keys(expected_id);
    if matches!(rt.obj(expected_id).internal_kind, InternalKind::Error) {
        if !keys.iter().any(|k| k == "name") {
            keys.push("name".into());
        }
        if !keys.iter().any(|k| k == "message") {
            keys.push("message".into());
        }
    }
    for key in keys {
        let expected = rt.object_get(expected_id, &key);
        let Some(actual) = assert_object_matcher_actual(rt, thrown, &key) else {
            return Ok(false);
        };
        if let Value::Object(regex) = expected {
            if matches!(rt.obj(regex).internal_kind, InternalKind::RegExp(_)) {
                if !assert_regexp_value_accepts(rt, regex, &actual)? {
                    return Ok(false);
                }
                continue;
            }
        }
        if !deep_equal_checked(rt, &actual, &expected, true)? {
            return Ok(false);
        }
    }
    Ok(true)
}

fn assert_validate_throws_matcher(
    rt: &mut Runtime,
    thrown: &Value,
    matcher: &Value,
) -> Result<(), RuntimeError> {
    match matcher {
        Value::Object(id)
            if matches!(rt.obj(*id).internal_kind, InternalKind::Ordinary)
                && rt.ordinary_own_enumerable_string_keys(*id).is_empty() =>
        {
            Err(node_code_type_error(
                rt,
                "ERR_INVALID_ARG_VALUE",
                "The argument 'error' may not be an empty object. Received {}",
            ))
        }
        Value::String(expected) => {
            let actual = match assert_error_name_message(rt, thrown) {
                Some((_, message)) => message,
                None => abstract_ops::to_string(thrown).as_str().to_string(),
            };
            if actual == expected.as_str() {
                let noun = if assert_error_name_message(rt, thrown).is_some() {
                    "error message"
                } else {
                    "error"
                };
                Err(node_code_error(
                    rt,
                    "ERR_AMBIGUOUS_ARGUMENT",
                    &format!(
                        "The \"error/message\" argument is ambiguous. The {noun} \"{}\" is identical to the message.",
                        expected.as_str()
                    ),
                ))
            } else {
                Ok(())
            }
        }
        _ => Ok(()),
    }
}

fn assert_matcher_constructor_name(rt: &Runtime, matcher: &Value) -> Option<String> {
    let Value::Object(id) = matcher else {
        return None;
    };
    if !rt.is_callable(matcher) {
        return None;
    }
    match rt.object_get(*id, "name") {
        Value::String(s) if !s.as_str().is_empty() => Some(s.as_str().to_string()),
        _ => Some("Function".into()),
    }
}

fn assert_object_constructor_name(rt: &Runtime, value: &Value) -> Option<String> {
    let Value::Object(id) = value else {
        return None;
    };
    let ctor = rt.object_get(*id, "constructor");
    match ctor {
        Value::Object(ctor_id) => match rt.object_get(ctor_id, "name") {
            Value::String(s) if !s.as_str().is_empty() => Some(s.as_str().to_string()),
            _ => None,
        },
        _ => None,
    }
}

fn assert_throws_mismatch_error(rt: &mut Runtime, thrown: Value, matcher: Value) -> RuntimeError {
    if let Some(expected_name) = assert_matcher_constructor_name(rt, &matcher) {
        let (actual_name, actual_message) =
            assert_error_name_message(rt, &thrown).unwrap_or_else(|| {
                (
                    "Error".into(),
                    abstract_ops::to_string(&thrown).as_str().to_string(),
                )
            });
        let actual_name = assert_object_constructor_name(rt, &thrown).unwrap_or(actual_name);
        return assert_error_with_fields(
            rt,
            format!(
                "The error is expected to be an instance of \"{expected_name}\". Received \"{actual_name}\"\n\nError message:\n\n{actual_message}"
            ),
            thrown,
            matcher,
            "throws",
            true,
            assert_instance_diff(rt).as_deref(),
        );
    }
    assertion_error(rt, "Thrown error did not match.".into())
}

fn suppressed_rejected_promise(rt: &mut Runtime, reason: Value) -> Result<Value, RuntimeError> {
    let p = new_promise(rt);
    rt.object_set(
        p,
        "__cruft_suppress_unhandled_rejection".into(),
        Value::Boolean(true),
    );
    reject_promise(rt, p, reason);
    Ok(Value::Object(p))
}

fn sync_throw_rejected_promise(rt: &mut Runtime, thrown: Value) -> Result<Value, RuntimeError> {
    let p = new_promise(rt);
    reject_promise(rt, p, thrown);
    Ok(Value::Object(p))
}

fn ok_impl(rt: &mut Runtime, args: &[Value]) -> Result<Value, RuntimeError> {
    let v = args.first().cloned().unwrap_or(Value::Undefined);
    if abstract_ops::to_boolean(&v) {
        Ok(Value::Undefined)
    } else {
        if let Some(err) = custom_error_message_arg(rt, args, 1) {
            return Err(RuntimeError::Thrown(err));
        }
        let msg = if args.len() > 1 && !matches!(args.get(1), Some(Value::Undefined)) {
            arg_msg(args, 1, "assertion failed")
        } else if args.is_empty() {
            "No value argument passed to `assert.ok()`".into()
        } else if matches!(v, Value::Null) {
            let rendered_args = if args.len() > 1 {
                "assert.ok(null, undefined)"
            } else {
                "assert.ok(null)"
            };
            format!("The expression evaluated to a falsy value:\n\n  {rendered_args}\n")
        } else {
            "assertion failed".into()
        };
        Err(assertion_error(rt, msg))
    }
}

fn loose_eq(a: &Value, b: &Value) -> bool {

    if let (Value::Number(x), Value::Number(y)) = (a, b) {
        if x.is_nan() && y.is_nan() {
            return true;
        }
    }
    abstract_ops::is_loosely_equal(a, b)
}

fn strict_eq(a: &Value, b: &Value) -> bool {

    abstract_ops::same_value(a, b)
}

fn assert_proxy_own_keys_preflight(rt: &mut Runtime, value: &Value) -> Result<(), RuntimeError> {
    if let Value::Object(id) = value {
        if rt.proxy_target_handler_checked(*id)?.is_some() {
            let _ = rt.reflect_own_keys_via(value)?;
        }
    }
    Ok(())
}

fn assert_accessor_snapshot(
    rt: &mut Runtime,
    id: ObjectRef,
) -> Result<Option<Value>, RuntimeError> {
    if !matches!(rt.obj(id).internal_kind, InternalKind::Ordinary) {
        return Ok(None);
    }
    let keys = rt.ordinary_own_enumerable_string_keys(id);
    let has_accessor = keys.iter().any(|key| {
        rt.obj(id)
            .get_own(key)
            .is_some_and(|desc| desc.getter.is_some() || desc.setter.is_some())
    });
    if !has_accessor {
        return Ok(None);
    }
    let snapshot = new_object(rt);
    let proto = rt.obj(id).proto;
    rt.set_object_prototype_internal(snapshot, proto);
    for key in keys {
        let value = rt.spec_get(&Value::Object(id), &key)?;
        rt.obj_mut(snapshot).dict_mut().insert(
            PropertyKey::String(key),
            PropertyDescriptor {
                value,
                writable: true,
                enumerable: true,
                configurable: true,
                getter: None,
                setter: None,
            },
        );
    }
    Ok(Some(Value::Object(snapshot)))
}

fn assert_compare_value(rt: &mut Runtime, value: &Value) -> Result<Value, RuntimeError> {
    let Value::Object(mut id) = value else {
        return Ok(value.clone());
    };
    while let Some((target, _handler)) = rt.proxy_target_handler_checked(id)? {
        id = target;
    }
    if let Some(snapshot) = assert_accessor_snapshot(rt, id)? {
        return Ok(snapshot);
    }
    Ok(Value::Object(id))
}

fn deep_equal_checked(
    rt: &mut Runtime,
    a: &Value,
    b: &Value,
    strict: bool,
) -> Result<bool, RuntimeError> {
    assert_proxy_own_keys_preflight(rt, a)?;
    assert_proxy_own_keys_preflight(rt, b)?;
    let a = assert_compare_value(rt, a)?;
    let b = assert_compare_value(rt, b)?;
    Ok(deep_equal(rt, &a, &b, strict))
}

fn partial_deep_equal_checked(
    rt: &mut Runtime,
    actual: &Value,
    expected: &Value,
    strict: bool,
) -> Result<bool, RuntimeError> {
    assert_proxy_own_keys_preflight(rt, actual)?;
    assert_proxy_own_keys_preflight(rt, expected)?;
    let actual = assert_compare_value(rt, actual)?;
    let expected = assert_compare_value(rt, expected)?;
    Ok(partial_deep_equal(rt, &actual, &expected, strict))
}

fn error_prototype(rt: &Runtime) -> Option<ObjectRef> {
    match rt.global_get("Error") {
        Value::Object(ctor) => match rt.object_get(ctor, "prototype") {
            Value::Object(proto) => Some(proto),
            _ => None,
        },
        _ => None,
    }
}

fn object_proto_chain_contains(rt: &Runtime, mut id: ObjectRef, target: ObjectRef) -> bool {
    let mut hops = 0;
    while hops < 64 {
        if id == target {
            return true;
        }
        let Some(proto) = rt.obj(id).proto else {
            return false;
        };
        id = proto;
        hops += 1;
    }
    false
}

fn has_enumerable_to_string_tag(rt: &Runtime, id: ObjectRef) -> bool {
    rt.ordinary_own_enumerable_property_keys(id)
        .into_iter()
        .any(|key| match key {
            PropertyKey::Symbol(sym) => sym.as_str() == "Symbol.toStringTag",
            PropertyKey::String(name) => name == "@@toStringTag",
        })
}

fn assert_error_like(rt: &Runtime, id: ObjectRef) -> bool {
    if matches!(rt.obj(id).internal_kind, InternalKind::Error) {
        return true;
    }
    let Some(error_proto) = error_prototype(rt) else {
        return false;
    };
    has_enumerable_to_string_tag(rt, id) && object_proto_chain_contains(rt, id, error_proto)
}

fn deep_equal_error(
    rt: &Runtime,
    a: ObjectRef,
    b: ObjectRef,
    strict: bool,
    memo: &mut DeepEqualMemo,
) -> Option<bool> {
    let a_is_error = assert_error_like(rt, a);
    let b_is_error = assert_error_like(rt, b);
    if !a_is_error && !b_is_error {
        return None;
    }
    if a_is_error != b_is_error {
        return Some(false);
    }
    for key in ["name", "message"] {
        if !deep_equal_inner(
            rt,
            &rt.object_get(a, key),
            &rt.object_get(b, key),
            strict,
            memo,
        ) {
            return Some(false);
        }
    }
    for key in ["cause", "errors"] {
        let a_has = rt.obj(a).has_own_str(key);
        let b_has = rt.obj(b).has_own_str(key);
        if a_has != b_has {
            return Some(false);
        }
        if a_has
            && !deep_equal_inner(
                rt,
                &rt.object_get(a, key),
                &rt.object_get(b, key),
                strict,
                memo,
            )
        {
            return Some(false);
        }
    }
    Some(true)
}

fn dom_exception_prototype(rt: &Runtime) -> Option<ObjectRef> {
    match rt.global_get("DOMException") {
        Value::Object(ctor) => match rt.object_get(ctor, "prototype") {
            Value::Object(proto) => Some(proto),
            _ => None,
        },
        _ => None,
    }
}

fn dom_exception_like(rt: &Runtime, id: ObjectRef) -> bool {
    let Some(proto) = dom_exception_prototype(rt) else {
        return false;
    };
    object_proto_chain_contains(rt, id, proto)
}

fn deep_equal_dom_exception(rt: &Runtime, a: ObjectRef, b: ObjectRef) -> Option<bool> {
    let a_is_dom = dom_exception_like(rt, a);
    let b_is_dom = dom_exception_like(rt, b);
    if !a_is_dom && !b_is_dom {
        return None;
    }
    if a_is_dom != b_is_dom {
        return Some(false);
    }
    let mut memo = DeepEqualMemo::default();
    Some(
        strict_eq(&rt.object_get(a, "name"), &rt.object_get(b, "name"))
            && strict_eq(&rt.object_get(a, "message"), &rt.object_get(b, "message"))
            && strict_eq(&rt.object_get(a, "code"), &rt.object_get(b, "code"))
            && deep_equal_public_props(rt, a, b, true, &mut memo, &["stack"]),
    )
}

fn object_string_kind(rt: &Runtime, id: ObjectRef, key: &str) -> Option<String> {
    match rt.object_get(id, key) {
        Value::String(s) => Some(s.as_str().to_string()),
        _ => None,
    }
}

fn typed_array_len(rt: &Runtime, id: ObjectRef) -> Option<usize> {
    let view = rt.typed_array_views.get(&id)?;
    let buffer = rt.array_buffers.get(&view.buffer)?;
    if buffer.detached {
        return Some(0);
    }
    Some(view.fixed_length.unwrap_or_else(|| {
        buffer.byte_len().saturating_sub(view.byte_offset) / view.bytes_per_element
    }))
}

fn typed_array_raw_bytes(rt: &Runtime, id: ObjectRef) -> Option<Vec<u8>> {
    let view = rt.typed_array_views.get(&id)?;
    let buffer = rt.array_buffers.get(&view.buffer)?;
    if buffer.detached {
        return Some(Vec::new());
    }
    let len = typed_array_len(rt, id)?;
    let start = view.byte_offset.min(buffer.byte_len());
    let byte_len = len.saturating_mul(view.bytes_per_element);
    let end = start.saturating_add(byte_len).min(buffer.byte_len());
    Some(buffer.read_bytes(start, end))
}

fn binary_public_keys(rt: &Runtime, id: ObjectRef) -> Vec<String> {
    let indexed_len = typed_array_len(rt, id);
    rt.ordinary_own_enumerable_string_keys(id)
        .into_iter()
        .filter(|key| {
            if matches!(key.as_str(), "length" | "byteLength") || key.starts_with("__") {
                return false;
            }
            if let Some(len) = indexed_len {
                if key.parse::<usize>().is_ok_and(|idx| idx < len) {
                    return false;
                }
            }
            true
        })
        .collect()
}

#[derive(Clone, Default)]
struct DeepEqualMemo {
    pairs: HashSet<(ObjectRef, ObjectRef)>,
    active_actual_to_expected: HashMap<ObjectRef, ObjectRef>,
    active_expected_to_actual: HashMap<ObjectRef, ObjectRef>,
}

impl DeepEqualMemo {
    fn enter(&mut self, actual: ObjectRef, expected: ObjectRef) -> Option<bool> {
        if self.pairs.contains(&(actual, expected)) {
            return Some(true);
        }
        if self
            .active_actual_to_expected
            .get(&actual)
            .is_some_and(|seen| *seen != expected)
        {
            return Some(false);
        }
        if self
            .active_expected_to_actual
            .get(&expected)
            .is_some_and(|seen| *seen != actual)
        {
            return Some(false);
        }
        self.active_actual_to_expected.insert(actual, expected);
        self.active_expected_to_actual.insert(expected, actual);
        self.pairs.insert((actual, expected));
        None
    }

    fn leave(&mut self, actual: ObjectRef, expected: ObjectRef) {
        if self.active_actual_to_expected.get(&actual) == Some(&expected) {
            self.active_actual_to_expected.remove(&actual);
        }
        if self.active_expected_to_actual.get(&expected) == Some(&actual) {
            self.active_expected_to_actual.remove(&expected);
        }
    }
}

fn with_deep_equal_pair(
    memo: &mut DeepEqualMemo,
    actual: ObjectRef,
    expected: ObjectRef,
    compare: impl FnOnce(&mut DeepEqualMemo) -> bool,
) -> bool {
    if let Some(eq) = memo.enter(actual, expected) {
        return eq;
    }
    let result = compare(memo);
    memo.leave(actual, expected);
    result
}

fn deep_equal_binary_props(
    rt: &Runtime,
    a: ObjectRef,
    b: ObjectRef,
    strict: bool,
    memo: &mut DeepEqualMemo,
) -> bool {
    let ka = binary_public_keys(rt, a);
    let kb = binary_public_keys(rt, b);
    if ka.len() != kb.len() {
        return false;
    }
    let sb: HashSet<&String> = kb.iter().collect();
    for k in &ka {
        if !sb.contains(k) {
            return false;
        }
        if !deep_equal_inner(rt, &rt.object_get(a, k), &rt.object_get(b, k), strict, memo) {
            return false;
        }
    }
    !strict || deep_equal_symbol_props(rt, a, b, strict, memo)
}

fn partial_deep_equal_binary_props(
    rt: &Runtime,
    actual: ObjectRef,
    expected: ObjectRef,
    strict: bool,
    memo: &mut DeepEqualMemo,
) -> bool {
    let actual_keys = binary_public_keys(rt, actual);
    let expected_keys = binary_public_keys(rt, expected);
    let actual_set: HashSet<&String> = actual_keys.iter().collect();
    for k in &expected_keys {
        if !actual_set.contains(k) {
            return false;
        }
        if !partial_deep_equal_inner(
            rt,
            &rt.object_get(actual, k),
            &rt.object_get(expected, k),
            strict,
            memo,
        ) {
            return false;
        }
    }
    !strict || deep_equal_symbol_props(rt, actual, expected, strict, memo)
}

fn assert_public_string_keys(rt: &Runtime, id: ObjectRef, internal_slots: &[&str]) -> Vec<String> {
    rt.ordinary_own_enumerable_string_keys(id)
        .into_iter()
        .filter(|key| {
            !key.starts_with("__") && !internal_slots.iter().any(|slot| key.as_str() == *slot)
        })
        .collect()
}

fn assert_public_symbol_keys(rt: &Runtime, id: ObjectRef) -> Vec<Rc<String>> {
    rt.ordinary_own_enumerable_property_keys(id)
        .into_iter()
        .filter_map(|key| match key {
            PropertyKey::Symbol(sym) => Some(sym),
            PropertyKey::String(_) => None,
        })
        .collect()
}

fn deep_equal_symbol_props(
    rt: &Runtime,
    a: ObjectRef,
    b: ObjectRef,
    strict: bool,
    memo: &mut DeepEqualMemo,
) -> bool {
    let ka = assert_public_symbol_keys(rt, a);
    let kb = assert_public_symbol_keys(rt, b);
    if ka.len() != kb.len() {
        return false;
    }
    let sb: HashSet<&Rc<String>> = kb.iter().collect();
    for sym in &ka {
        if !sb.contains(sym) {
            return false;
        }
        let av = rt
            .obj(a)
            .get_own_symbol(sym)
            .map(|desc| desc.value.clone())
            .unwrap_or(Value::Undefined);
        let bv = rt
            .obj(b)
            .get_own_symbol(sym)
            .map(|desc| desc.value.clone())
            .unwrap_or(Value::Undefined);
        if !deep_equal_inner(rt, &av, &bv, strict, memo) {
            return false;
        }
    }
    true
}

fn deep_equal_public_props(
    rt: &Runtime,
    a: ObjectRef,
    b: ObjectRef,
    strict: bool,
    memo: &mut DeepEqualMemo,
    internal_slots: &[&str],
) -> bool {
    let ka = assert_public_string_keys(rt, a, internal_slots);
    let kb = assert_public_string_keys(rt, b, internal_slots);
    if ka.len() != kb.len() {
        return false;
    }
    let sb: HashSet<&String> = kb.iter().collect();
    for k in &ka {
        if !sb.contains(k) {
            return false;
        }
        if !deep_equal_inner(rt, &rt.object_get(a, k), &rt.object_get(b, k), strict, memo) {
            return false;
        }
    }
    !strict || deep_equal_symbol_props(rt, a, b, strict, memo)
}

fn partial_deep_equal_public_props(
    rt: &Runtime,
    actual: ObjectRef,
    expected: ObjectRef,
    strict: bool,
    memo: &mut DeepEqualMemo,
    internal_slots: &[&str],
) -> bool {
    let actual_keys = assert_public_string_keys(rt, actual, internal_slots);
    let expected_keys = assert_public_string_keys(rt, expected, internal_slots);
    let actual_set: HashSet<&String> = actual_keys.iter().collect();
    for k in &expected_keys {
        if !actual_set.contains(k) {
            return false;
        }
        if !partial_deep_equal_inner(
            rt,
            &rt.object_get(actual, k),
            &rt.object_get(expected, k),
            strict,
            memo,
        ) {
            return false;
        }
    }
    !strict || deep_equal_symbol_props(rt, actual, expected, strict, memo)
}

fn array_index_key(key: &str, len: usize) -> Option<usize> {
    key.parse::<usize>().ok().filter(|idx| *idx < len)
}

fn partial_deep_equal_array(
    rt: &Runtime,
    actual: ObjectRef,
    expected: ObjectRef,
    strict: bool,
    memo: &mut DeepEqualMemo,
) -> bool {
    let Value::Number(actual_len_num) = rt.object_get(actual, "length") else {
        return false;
    };
    let Value::Number(expected_len_num) = rt.object_get(expected, "length") else {
        return false;
    };
    if actual_len_num != expected_len_num || actual_len_num < 0.0 {
        return false;
    }
    let len = actual_len_num as usize;
    let actual_keys = rt.ordinary_own_enumerable_string_keys(actual);
    let expected_keys = rt.ordinary_own_enumerable_string_keys(expected);
    let actual_indices: HashSet<usize> = actual_keys
        .iter()
        .filter_map(|key| array_index_key(key, len))
        .collect();
    let expected_indices: HashSet<usize> = expected_keys
        .iter()
        .filter_map(|key| array_index_key(key, len))
        .collect();
    if actual_indices.is_subset(&expected_indices) || expected_indices.is_subset(&actual_indices) {
        if actual_indices != expected_indices {
            return false;
        }
    }
    for idx in actual_indices.intersection(&expected_indices) {
        let key = idx.to_string();
        if !partial_deep_equal_inner(
            rt,
            &rt.object_get(actual, &key),
            &rt.object_get(expected, &key),
            strict,
            memo,
        ) {
            return false;
        }
    }
    let actual_key_set: HashSet<&String> = actual_keys.iter().collect();
    for key in expected_keys
        .iter()
        .filter(|key| array_index_key(key, len).is_none())
    {
        if !actual_key_set.contains(key) {
            return false;
        }
        if !partial_deep_equal_inner(
            rt,
            &rt.object_get(actual, key),
            &rt.object_get(expected, key),
            strict,
            memo,
        ) {
            return false;
        }
    }
    !strict || deep_equal_symbol_props(rt, actual, expected, strict, memo)
}

fn deep_equal_binary(
    rt: &Runtime,
    a: ObjectRef,
    b: ObjectRef,
    strict: bool,
    memo: &mut DeepEqualMemo,
) -> Option<bool> {
    let a_view = rt.typed_array_views.get(&a);
    let b_view = rt.typed_array_views.get(&b);
    if a_view.is_some() || b_view.is_some() {
        let (Some(av), Some(bv)) = (a_view, b_view) else {
            return Some(false);
        };
        if av.element_kind != bv.element_kind {
            return Some(false);
        }
        let Some(alen) = typed_array_len(rt, a) else {
            return Some(false);
        };
        let Some(blen) = typed_array_len(rt, b) else {
            return Some(false);
        };
        if alen != blen {
            return Some(false);
        }
        if strict {
            let Some(a_bytes) = typed_array_raw_bytes(rt, a) else {
                return Some(false);
            };
            let Some(b_bytes) = typed_array_raw_bytes(rt, b) else {
                return Some(false);
            };
            return Some(a_bytes == b_bytes && deep_equal_binary_props(rt, a, b, strict, memo));
        }
        for idx in 0..alen {
            let key = idx.to_string();
            let av = rt.object_get(a, &key);
            let bv = rt.object_get(b, &key);
            if !deep_equal_inner(rt, &av, &bv, strict, memo) {
                return Some(false);
            }
        }
        return Some(deep_equal_binary_props(rt, a, b, strict, memo));
    }

    let a_buf = rt.array_buffers.get(&a);
    let b_buf = rt.array_buffers.get(&b);
    if a_buf.is_some() || b_buf.is_some() {
        let (Some(ab), Some(bb)) = (a_buf, b_buf) else {
            return Some(false);
        };
        let ak = object_string_kind(rt, a, "__kind").unwrap_or_else(|| "ArrayBuffer".to_string());
        let bk = object_string_kind(rt, b, "__kind").unwrap_or_else(|| "ArrayBuffer".to_string());
        if ak != bk {
            return Some(false);
        }
        if ab.byte_len() != bb.byte_len() {
            return Some(false);
        }
        return Some(
            ab.read_bytes(0, ab.byte_len()) == bb.read_bytes(0, bb.byte_len())
                && deep_equal_binary_props(rt, a, b, strict, memo),
        );
    }

    None
}

fn partial_deep_equal_binary(
    rt: &Runtime,
    actual: ObjectRef,
    expected: ObjectRef,
    strict: bool,
    memo: &mut DeepEqualMemo,
) -> Option<bool> {
    let actual_view = rt.typed_array_views.get(&actual);
    let expected_view = rt.typed_array_views.get(&expected);
    if actual_view.is_some() || expected_view.is_some() {
        let (Some(av), Some(ev)) = (actual_view, expected_view) else {
            return Some(false);
        };
        if av.element_kind != ev.element_kind {
            return Some(false);
        }
        let Some(actual_len) = typed_array_len(rt, actual) else {
            return Some(false);
        };
        let Some(expected_len) = typed_array_len(rt, expected) else {
            return Some(false);
        };
        if expected_len > actual_len {
            return Some(false);
        }
        if strict {
            let Some(actual_bytes) = typed_array_raw_bytes(rt, actual) else {
                return Some(false);
            };
            let Some(expected_bytes) = typed_array_raw_bytes(rt, expected) else {
                return Some(false);
            };
            let expected_bytes_len = expected_len * ev.bytes_per_element;
            return Some(
                actual_bytes.get(0..expected_bytes_len) == Some(expected_bytes.as_slice())
                    && partial_deep_equal_binary_props(rt, actual, expected, strict, memo),
            );
        }
        for idx in 0..expected_len {
            let key = idx.to_string();
            let av = rt.object_get(actual, &key);
            let ev = rt.object_get(expected, &key);
            if !partial_deep_equal_inner(rt, &av, &ev, strict, memo) {
                return Some(false);
            }
        }
        return Some(partial_deep_equal_binary_props(
            rt, actual, expected, strict, memo,
        ));
    }

    let actual_buf = rt.array_buffers.get(&actual);
    let expected_buf = rt.array_buffers.get(&expected);
    if actual_buf.is_some() || expected_buf.is_some() {
        let (Some(ab), Some(eb)) = (actual_buf, expected_buf) else {
            return Some(false);
        };
        let ak =
            object_string_kind(rt, actual, "__kind").unwrap_or_else(|| "ArrayBuffer".to_string());
        let ek =
            object_string_kind(rt, expected, "__kind").unwrap_or_else(|| "ArrayBuffer".to_string());
        if ak != ek {
            return Some(false);
        }
        let expected_len = eb.byte_len();
        if expected_len > ab.byte_len() {
            return Some(false);
        }
        return Some(
            ab.read_bytes(0, expected_len) == eb.read_bytes(0, expected_len)
                && partial_deep_equal_binary_props(rt, actual, expected, strict, memo),
        );
    }

    None
}

fn boxed_primitive_value(rt: &Runtime, id: ObjectRef) -> Option<(&'static str, Value)> {
    let from_kind = match &rt.obj(id).internal_kind {
        InternalKind::NumberWrapper(v) => Some(("Number", v.clone())),
        InternalKind::StringWrapper(v) => Some(("String", v.clone())),
        InternalKind::BooleanWrapper(v) => Some(("Boolean", v.clone())),
        InternalKind::BigIntWrapper(v) => Some(("BigInt", v.clone())),
        _ => None,
    };
    if from_kind.is_some() {
        return from_kind;
    }
    match rt.obj(id).get_own("__primitive__").map(|desc| &desc.value) {
        Some(Value::Number(n)) => Some(("Number", Value::Number(*n))),
        Some(Value::String(s)) => Some(("String", Value::String(s.clone()))),
        Some(Value::Boolean(b)) => Some(("Boolean", Value::Boolean(*b))),
        Some(Value::BigInt(b)) => Some(("BigInt", Value::BigInt(b.clone()))),
        Some(Value::Symbol(s)) => Some(("Symbol", Value::Symbol(s.clone()))),
        _ => None,
    }
}

fn deep_equal_boxed_primitive(
    rt: &Runtime,
    a: ObjectRef,
    b: ObjectRef,
    strict: bool,
    memo: &mut DeepEqualMemo,
) -> Option<bool> {
    match (boxed_primitive_value(rt, a), boxed_primitive_value(rt, b)) {
        (None, None) => None,
        (Some((ak, av)), Some((bk, bv))) => Some(
            ak == bk
                && deep_equal_inner(rt, &av, &bv, strict, memo)
                && deep_equal_public_props(
                    rt,
                    a,
                    b,
                    strict,
                    memo,
                    &["__primitive", "__primitive__"],
                ),
        ),
        _ => Some(false),
    }
}

fn partial_deep_equal_boxed_primitive(
    rt: &Runtime,
    actual: ObjectRef,
    expected: ObjectRef,
    strict: bool,
    memo: &mut DeepEqualMemo,
) -> Option<bool> {
    match (
        boxed_primitive_value(rt, actual),
        boxed_primitive_value(rt, expected),
    ) {
        (None, None) => None,
        (Some((ak, av)), Some((ek, ev))) => Some(
            ak == ek
                && partial_deep_equal_inner(rt, &av, &ev, strict, memo)
                && partial_deep_equal_public_props(
                    rt,
                    actual,
                    expected,
                    strict,
                    memo,
                    &["__primitive", "__primitive__"],
                ),
        ),
        _ => Some(false),
    }
}

fn regexp_signature(rt: &Runtime, id: ObjectRef) -> Option<(String, String)> {
    match &rt.obj(id).internal_kind {
        InternalKind::RegExp(re) => Some((re.source.as_ref().clone(), re.flags.as_ref().clone())),
        _ => None,
    }
}

fn deep_equal_regexp(
    rt: &Runtime,
    a: ObjectRef,
    b: ObjectRef,
    strict: bool,
    memo: &mut DeepEqualMemo,
) -> Option<bool> {
    match (regexp_signature(rt, a), regexp_signature(rt, b)) {
        (None, None) => None,
        (Some((asrc, aflags)), Some((bsrc, bflags))) => Some(
            asrc == bsrc
                && aflags == bflags
                && deep_equal_inner(
                    rt,
                    &rt.object_get(a, "lastIndex"),
                    &rt.object_get(b, "lastIndex"),
                    strict,
                    memo,
                )
                && deep_equal_public_props(rt, a, b, strict, memo, &[]),
        ),
        _ => Some(false),
    }
}

fn partial_deep_equal_regexp(
    rt: &Runtime,
    actual: ObjectRef,
    expected: ObjectRef,
    strict: bool,
    memo: &mut DeepEqualMemo,
) -> Option<bool> {
    match (regexp_signature(rt, actual), regexp_signature(rt, expected)) {
        (None, None) => None,
        (Some((asrc, aflags)), Some((esrc, eflags))) => Some(
            asrc == esrc
                && aflags == eflags
                && partial_deep_equal_inner(
                    rt,
                    &rt.object_get(actual, "lastIndex"),
                    &rt.object_get(expected, "lastIndex"),
                    strict,
                    memo,
                )
                && partial_deep_equal_public_props(rt, actual, expected, strict, memo, &[]),
        ),
        _ => Some(false),
    }
}

fn url_href(rt: &Runtime, id: ObjectRef) -> Option<Value> {
    match rt.object_get(id, "__url_href__") {
        Value::Undefined => None,
        href => Some(href),
    }
}

fn deep_equal_url(
    rt: &Runtime,
    a: ObjectRef,
    b: ObjectRef,
    strict: bool,
    memo: &mut DeepEqualMemo,
) -> Option<bool> {
    match (url_href(rt, a), url_href(rt, b)) {
        (None, None) => None,
        (Some(ah), Some(bh)) => {
            Some(strict_eq(&ah, &bh) && deep_equal_public_props(rt, a, b, strict, memo, &[]))
        }
        _ => Some(false),
    }
}

fn partial_deep_equal_url(
    rt: &Runtime,
    actual: ObjectRef,
    expected: ObjectRef,
    strict: bool,
    memo: &mut DeepEqualMemo,
) -> Option<bool> {
    match (url_href(rt, actual), url_href(rt, expected)) {
        (None, None) => None,
        (Some(ah), Some(eh)) => Some(
            strict_eq(&ah, &eh)
                && partial_deep_equal_public_props(rt, actual, expected, strict, memo, &[]),
        ),
        _ => Some(false),
    }
}

fn assert_is_function_object(rt: &Runtime, id: ObjectRef) -> bool {
    matches!(
        rt.obj(id).internal_kind,
        InternalKind::Function(_) | InternalKind::Closure(_) | InternalKind::BoundFunction(_)
    )
}

fn deep_equal(rt: &Runtime, a: &Value, b: &Value, strict: bool) -> bool {
    let mut memo = DeepEqualMemo::default();
    deep_equal_inner(rt, a, b, strict, &mut memo)
}

fn deep_equal_inner(
    rt: &Runtime,
    a: &Value,
    b: &Value,
    strict: bool,
    memo: &mut DeepEqualMemo,
) -> bool {
    match (a, b) {
        (Value::Object(ia), Value::Object(ib)) => {
            if ia == ib {
                return true;
            }
            if assert_is_function_object(rt, *ia) || assert_is_function_object(rt, *ib) {
                return false;
            }
            return with_deep_equal_pair(memo, *ia, *ib, |memo| {
                if let Some(eq) = deep_equal_error(rt, *ia, *ib, strict, memo) {
                    return eq;
                }
                if let Some(eq) = deep_equal_dom_exception(rt, *ia, *ib) {
                    return eq;
                }
                if strict
                    && !assert_instance_skip_prototype(rt)
                    && rt.obj(*ia).proto != rt.obj(*ib).proto
                {
                    return false;
                }
                if let Some(eq) = deep_equal_boxed_primitive(rt, *ia, *ib, strict, memo) {
                    return eq;
                }
                if let Some(eq) = deep_equal_regexp(rt, *ia, *ib, strict, memo) {
                    return eq;
                }
                if let Some(eq) = deep_equal_url(rt, *ia, *ib, strict, memo) {
                    return eq;
                }
                let a_date = rt.is_date_object(*ia);
                let b_date = rt.is_date_object(*ib);
                if a_date || b_date {
                    if !(a_date && b_date) {
                        return false;
                    }
                    return strict_eq(
                        &rt.object_get(*ia, "__date_ms"),
                        &rt.object_get(*ib, "__date_ms"),
                    ) && deep_equal_public_props(
                        rt,
                        *ia,
                        *ib,
                        strict,
                        memo,
                        &["__date_ms"],
                    );
                }
                if let Some(eq) = deep_equal_binary(rt, *ia, *ib, strict, memo) {
                    return eq;
                }
                let (am, bm) = (
                    rt.obj(*ia).has_own_str("__map_data"),
                    rt.obj(*ib).has_own_str("__map_data"),
                );
                if am || bm {
                    if !(am && bm) {
                        return false;
                    }
                    if assert_is_weak_collection(rt, *ia, "__is_weakmap")
                        || assert_is_weak_collection(rt, *ib, "__is_weakmap")
                    {
                        return false;
                    }
                    return deep_equal_map(rt, *ia, *ib, strict, memo);
                }
                let (as_, bs) = (
                    rt.obj(*ia).has_own_str("__set_data"),
                    rt.obj(*ib).has_own_str("__set_data"),
                );
                if as_ || bs {
                    if !(as_ && bs) {
                        return false;
                    }
                    if assert_is_weak_collection(rt, *ia, "__is_weakset")
                        || assert_is_weak_collection(rt, *ib, "__is_weakset")
                    {
                        return false;
                    }
                    return deep_equal_set(rt, *ia, *ib, strict, memo);
                }
                let aa = matches!(rt.obj(*ia).internal_kind, InternalKind::Array);
                let ba = matches!(rt.obj(*ib).internal_kind, InternalKind::Array);
                if aa != ba {
                    return false;
                }
                if aa && !strict_eq(&rt.object_get(*ia, "length"), &rt.object_get(*ib, "length")) {
                    return false;
                }
                let a_args = matches!(
                    rt.obj(*ia).internal_kind,
                    InternalKind::MappedArguments { .. }
                );
                let b_args = matches!(
                    rt.obj(*ib).internal_kind,
                    InternalKind::MappedArguments { .. }
                );
                if a_args != b_args {
                    return false;
                }
                let ka = rt.ordinary_own_enumerable_string_keys(*ia);
                let kb = rt.ordinary_own_enumerable_string_keys(*ib);
                if ka.len() != kb.len() {
                    return false;
                }
                let sb: HashSet<&String> = kb.iter().collect();
                for k in &ka {
                    if !sb.contains(k) {
                        return false;
                    }
                    if !deep_equal_inner(
                        rt,
                        &rt.object_get(*ia, k),
                        &rt.object_get(*ib, k),
                        strict,
                        memo,
                    ) {
                        return false;
                    }
                }
                if strict && !deep_equal_symbol_props(rt, *ia, *ib, strict, memo) {
                    return false;
                }
                true
            });
        }
        (Value::Object(_), _) | (_, Value::Object(_)) => false,
        _ => {
            if let (Value::Symbol(x), Value::Symbol(y)) = (a, b) {
                return x.as_str() == y.as_str();
            }
            if let (Value::Number(x), Value::Number(y)) = (a, b) {
                if x.is_nan() && y.is_nan() {
                    return true;
                }
            }
            if strict {
                strict_eq(a, b)
            } else {
                abstract_ops::is_loosely_equal(a, b)
            }
        }
    }
}

fn map_original_key(rt: &Runtime, map: ObjectRef, storage_key: &str) -> Value {
    match rt.object_get(map, "__map_orig_keys") {
        Value::Object(orig) if rt.obj(orig).has_own_str(storage_key) => {
            rt.object_get(orig, storage_key)
        }
        _ => Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
            storage_key.to_string(),
        ))),
    }
}

fn storage_entry_value(rt: &Runtime, storage: ObjectRef, key: &PropertyKey) -> Value {
    match key {
        PropertyKey::String(s) => rt.object_get(storage, s),
        PropertyKey::Symbol(sym) => rt
            .obj(storage)
            .get_own_symbol(sym)
            .map(|desc| desc.value.clone())
            .unwrap_or_else(|| rt.object_get(storage, sym.as_str())),
    }
}

fn map_original_key_value(rt: &Runtime, map: ObjectRef, storage_key: &PropertyKey) -> Value {
    match storage_key {
        PropertyKey::String(s) => map_original_key(rt, map, s),
        PropertyKey::Symbol(sym) => Value::Symbol(sym.clone()),
    }
}

fn assert_collection_public_keys(rt: &Runtime, id: ObjectRef) -> Vec<String> {
    rt.ordinary_own_enumerable_string_keys(id)
        .into_iter()
        .filter(|key| {
            !matches!(
                key.as_str(),
                "__map_data" | "__map_orig_keys" | "__set_data"
            )
        })
        .collect()
}

fn assert_is_weak_collection(rt: &Runtime, id: ObjectRef, marker: &str) -> bool {
    matches!(rt.object_get(id, marker), Value::Boolean(true))
}

fn deep_equal_collection_props(
    rt: &Runtime,
    a: ObjectRef,
    b: ObjectRef,
    strict: bool,
    memo: &mut DeepEqualMemo,
) -> bool {
    let ka = assert_collection_public_keys(rt, a);
    let kb = assert_collection_public_keys(rt, b);
    if ka.len() != kb.len() {
        return false;
    }
    let sb: HashSet<&String> = kb.iter().collect();
    for k in &ka {
        if !sb.contains(k) {
            return false;
        }
        if !deep_equal_inner(rt, &rt.object_get(a, k), &rt.object_get(b, k), strict, memo) {
            return false;
        }
    }
    true
}

fn partial_deep_equal_collection_props(
    rt: &Runtime,
    actual: ObjectRef,
    expected: ObjectRef,
    strict: bool,
    memo: &mut DeepEqualMemo,
) -> bool {
    let actual_keys = assert_collection_public_keys(rt, actual);
    let expected_keys = assert_collection_public_keys(rt, expected);
    let actual_set: HashSet<&String> = actual_keys.iter().collect();
    for k in &expected_keys {
        if !actual_set.contains(k) {
            return false;
        }
        if !partial_deep_equal_inner(
            rt,
            &rt.object_get(actual, k),
            &rt.object_get(expected, k),
            strict,
            memo,
        ) {
            return false;
        }
    }
    true
}

fn deep_equal_set(
    rt: &Runtime,
    a: ObjectRef,
    b: ObjectRef,
    strict: bool,
    memo: &mut DeepEqualMemo,
) -> bool {
    let (sa, sb) = match (
        rt.object_get(a, "__set_data"),
        rt.object_get(b, "__set_data"),
    ) {
        (Value::Object(x), Value::Object(y)) => (x, y),
        _ => return false,
    };
    let ka = rt.ordinary_own_enumerable_property_keys(sa);
    let kb = rt.ordinary_own_enumerable_property_keys(sb);
    if ka.len() != kb.len() {
        return false;
    }
    let mut used = vec![false; kb.len()];
    'outer: for ak in &ka {
        let av = storage_entry_value(rt, sa, ak);
        for (idx, bk) in kb.iter().enumerate() {
            if used[idx] {
                continue;
            }
            let mut candidate_memo = memo.clone();
            if deep_equal_inner(
                rt,
                &av,
                &storage_entry_value(rt, sb, bk),
                strict,
                &mut candidate_memo,
            ) {
                *memo = candidate_memo;
                used[idx] = true;
                continue 'outer;
            }
        }
        return false;
    }
    deep_equal_collection_props(rt, a, b, strict, memo)
}

fn deep_equal_map(
    rt: &Runtime,
    a: ObjectRef,
    b: ObjectRef,
    strict: bool,
    memo: &mut DeepEqualMemo,
) -> bool {
    let (sa, sb) = match (
        rt.object_get(a, "__map_data"),
        rt.object_get(b, "__map_data"),
    ) {
        (Value::Object(x), Value::Object(y)) => (x, y),
        _ => return false,
    };
    let ka = rt.ordinary_own_enumerable_property_keys(sa);
    let kb = rt.ordinary_own_enumerable_property_keys(sb);
    if ka.len() != kb.len() {
        return false;
    }
    let mut used = vec![false; kb.len()];
    'outer: for ak in &ka {
        let av = storage_entry_value(rt, sa, ak);
        let akey = map_original_key_value(rt, a, ak);
        for (idx, bk) in kb.iter().enumerate() {
            if used[idx] {
                continue;
            }
            let bkey = map_original_key_value(rt, b, bk);
            let mut candidate_memo = memo.clone();
            if deep_equal_inner(rt, &akey, &bkey, strict, &mut candidate_memo)
                && deep_equal_inner(
                    rt,
                    &av,
                    &storage_entry_value(rt, sb, bk),
                    strict,
                    &mut candidate_memo,
                )
            {
                *memo = candidate_memo;
                used[idx] = true;
                continue 'outer;
            }
        }
        return false;
    }
    deep_equal_collection_props(rt, a, b, strict, memo)
}

fn partial_deep_equal(rt: &Runtime, actual: &Value, expected: &Value, strict: bool) -> bool {
    let mut memo = DeepEqualMemo::default();
    partial_deep_equal_inner(rt, actual, expected, strict, &mut memo)
}

fn partial_deep_equal_inner(
    rt: &Runtime,
    actual: &Value,
    expected: &Value,
    strict: bool,
    memo: &mut DeepEqualMemo,
) -> bool {
    match (actual, expected) {
        (Value::Object(ia), Value::Object(ib)) => {
            if ia == ib {
                return true;
            }
            if assert_is_function_object(rt, *ia) || assert_is_function_object(rt, *ib) {
                return false;
            }
            return with_deep_equal_pair(memo, *ia, *ib, |memo| {
                if let Some(eq) = deep_equal_error(rt, *ia, *ib, strict, memo) {
                    return eq;
                }
                if let Some(eq) = deep_equal_dom_exception(rt, *ia, *ib) {
                    return eq;
                }
                if strict
                    && !assert_instance_skip_prototype(rt)
                    && rt.obj(*ia).proto != rt.obj(*ib).proto
                {
                    return false;
                }
                if let Some(eq) = partial_deep_equal_boxed_primitive(rt, *ia, *ib, strict, memo) {
                    return eq;
                }
                if let Some(eq) = partial_deep_equal_regexp(rt, *ia, *ib, strict, memo) {
                    return eq;
                }
                if let Some(eq) = partial_deep_equal_url(rt, *ia, *ib, strict, memo) {
                    return eq;
                }
                let a_date = rt.is_date_object(*ia);
                let b_date = rt.is_date_object(*ib);
                if a_date || b_date {
                    if !(a_date && b_date) {
                        return false;
                    }
                    return strict_eq(
                        &rt.object_get(*ia, "__date_ms"),
                        &rt.object_get(*ib, "__date_ms"),
                    ) && partial_deep_equal_public_props(
                        rt,
                        *ia,
                        *ib,
                        strict,
                        memo,
                        &["__date_ms"],
                    );
                }
                if let Some(eq) = partial_deep_equal_binary(rt, *ia, *ib, strict, memo) {
                    return eq;
                }
                let (am, bm) = (
                    rt.obj(*ia).has_own_str("__map_data"),
                    rt.obj(*ib).has_own_str("__map_data"),
                );
                if am || bm {
                    if !(am && bm) {
                        return false;
                    }
                    if assert_is_weak_collection(rt, *ia, "__is_weakmap")
                        || assert_is_weak_collection(rt, *ib, "__is_weakmap")
                    {
                        return false;
                    }
                    return partial_deep_equal_map(rt, *ia, *ib, strict, memo);
                }
                let (as_, bs) = (
                    rt.obj(*ia).has_own_str("__set_data"),
                    rt.obj(*ib).has_own_str("__set_data"),
                );
                if as_ || bs {
                    if !(as_ && bs) {
                        return false;
                    }
                    if assert_is_weak_collection(rt, *ia, "__is_weakset")
                        || assert_is_weak_collection(rt, *ib, "__is_weakset")
                    {
                        return false;
                    }
                    return partial_deep_equal_set(rt, *ia, *ib, strict, memo);
                }
                let aa = matches!(rt.obj(*ia).internal_kind, InternalKind::Array);
                let ba = matches!(rt.obj(*ib).internal_kind, InternalKind::Array);
                if aa != ba {
                    return false;
                }
                if aa {
                    return partial_deep_equal_array(rt, *ia, *ib, strict, memo);
                }
                let a_args = matches!(
                    rt.obj(*ia).internal_kind,
                    InternalKind::MappedArguments { .. }
                );
                let b_args = matches!(
                    rt.obj(*ib).internal_kind,
                    InternalKind::MappedArguments { .. }
                );
                if a_args != b_args {
                    return false;
                }
                let actual_keys = rt.ordinary_own_enumerable_string_keys(*ia);
                let expected_keys = rt.ordinary_own_enumerable_string_keys(*ib);
                let actual_set: HashSet<&String> = actual_keys.iter().collect();
                for k in &expected_keys {
                    if !actual_set.contains(k) {
                        return false;
                    }
                    if !partial_deep_equal_inner(
                        rt,
                        &rt.object_get(*ia, k),
                        &rt.object_get(*ib, k),
                        strict,
                        memo,
                    ) {
                        return false;
                    }
                }
                if strict && !deep_equal_symbol_props(rt, *ia, *ib, strict, memo) {
                    return false;
                }
                true
            });
        }
        (Value::Object(_), _) | (_, Value::Object(_)) => false,
        _ => {
            if let (Value::Symbol(x), Value::Symbol(y)) = (actual, expected) {
                return x.as_str() == y.as_str();
            }
            if let (Value::Number(x), Value::Number(y)) = (actual, expected) {
                if x.is_nan() && y.is_nan() {
                    return true;
                }
            }
            if strict {
                strict_eq(actual, expected)
            } else {
                abstract_ops::is_loosely_equal(actual, expected)
            }
        }
    }
}

fn partial_deep_equal_set(
    rt: &Runtime,
    actual: ObjectRef,
    expected: ObjectRef,
    strict: bool,
    memo: &mut DeepEqualMemo,
) -> bool {
    let (actual_store, expected_store) = match (
        rt.object_get(actual, "__set_data"),
        rt.object_get(expected, "__set_data"),
    ) {
        (Value::Object(x), Value::Object(y)) => (x, y),
        _ => return false,
    };
    let actual_keys = rt.ordinary_own_enumerable_property_keys(actual_store);
    let expected_keys = rt.ordinary_own_enumerable_property_keys(expected_store);
    let mut used = vec![false; actual_keys.len()];
    'outer: for ek in &expected_keys {
        let ev = storage_entry_value(rt, expected_store, ek);
        for (idx, ak) in actual_keys.iter().enumerate() {
            if used[idx] {
                continue;
            }
            let mut candidate_memo = memo.clone();
            if partial_deep_equal_inner(
                rt,
                &storage_entry_value(rt, actual_store, ak),
                &ev,
                strict,
                &mut candidate_memo,
            ) {
                *memo = candidate_memo;
                used[idx] = true;
                continue 'outer;
            }
        }
        return false;
    }
    partial_deep_equal_collection_props(rt, actual, expected, strict, memo)
}

fn partial_deep_equal_map(
    rt: &Runtime,
    actual: ObjectRef,
    expected: ObjectRef,
    strict: bool,
    memo: &mut DeepEqualMemo,
) -> bool {
    let (actual_store, expected_store) = match (
        rt.object_get(actual, "__map_data"),
        rt.object_get(expected, "__map_data"),
    ) {
        (Value::Object(x), Value::Object(y)) => (x, y),
        _ => return false,
    };
    let actual_keys = rt.ordinary_own_enumerable_property_keys(actual_store);
    let expected_keys = rt.ordinary_own_enumerable_property_keys(expected_store);
    let mut used = vec![false; actual_keys.len()];
    'outer: for ek in &expected_keys {
        let ev = storage_entry_value(rt, expected_store, ek);
        let ekey = map_original_key_value(rt, expected, ek);
        for (idx, ak) in actual_keys.iter().enumerate() {
            if used[idx] {
                continue;
            }
            let akey = map_original_key_value(rt, actual, ak);
            let mut candidate_memo = memo.clone();
            if partial_deep_equal_inner(rt, &akey, &ekey, strict, &mut candidate_memo)
                && partial_deep_equal_inner(
                    rt,
                    &storage_entry_value(rt, actual_store, ak),
                    &ev,
                    strict,
                    &mut candidate_memo,
                )
            {
                *memo = candidate_memo;
                used[idx] = true;
                continue 'outer;
            }
        }
        return false;
    }
    partial_deep_equal_collection_props(rt, actual, expected, strict, memo)
}

pub fn install(rt: &mut Runtime) {
    let assert = make_callable(rt, "assert", ok_impl);

    register_method(rt, assert, "ok", ok_impl);

    let error_proto = match rt.global_get("Error") {
        Value::Object(e) => match rt.object_get(e, "prototype") {
            Value::Object(p) => Some(p),
            _ => None,
        },
        _ => None,
    };
    let ae_proto = new_object(rt);
    if let Some(ep) = error_proto {
        rt.set_object_prototype_internal(ae_proto, Some(ep));
    }
    rt.object_set(
        ae_proto,
        "name".into(),
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
            "AssertionError",
        ))),
    );
    rt.define_data_property_attrs(
        ae_proto,
        "stack",
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
            "AssertionError",
        ))),
        true,
        false,
        true,
    );
    let ae_ctor = make_callable(rt, "AssertionError", |rt, args| {
        let msg = match args.first() {

            Some(Value::Object(o)) => match rt.object_get(*o, "message") {
                Value::String(s) => s.as_str().to_string(),
                Value::Undefined => String::new(),
                v => abstract_ops::to_string(&v).as_str().to_string(),
            },
            Some(v) => return Err(invalid_options_arg_error(rt, v)),
            None => return Err(invalid_options_arg_error(rt, &Value::Undefined)),
        };
        let err = build_assertion_error(rt, msg);
        if let Value::Object(this_id) = rt.current_this() {
            rt.define_data_property_attrs(
                this_id,
                "stack",
                Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                    "AssertionError",
                ))),
                true,
                false,
                true,
            );
        }
        if let Value::Object(id) = err {
            rt.define_data_property_attrs(
                id,
                "stack",
                Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                    "AssertionError",
                ))),
                true,
                false,
                true,
            );
            Ok(Value::Object(id))
        } else {
            Ok(err)
        }
    });
    set_constant(rt, ae_ctor, "prototype", Value::Object(ae_proto));
    rt.object_set(ae_proto, "constructor".into(), Value::Object(ae_ctor));
    set_constant(rt, assert, "AssertionError", Value::Object(ae_ctor));
    register_method(rt, assert, "equal", |rt, args| {
        if args.len() < 2 {
            return Err(missing_args_error(rt));
        }
        let a = args.first().cloned().unwrap_or(Value::Undefined);
        let b = args.get(1).cloned().unwrap_or(Value::Undefined);
        if loose_eq(&a, &b) {
            Ok(Value::Undefined)
        } else {

            let generated = args.get(2).is_none();
            let msg = if generated {
                format!(
                    "{} == {}",
                    crate::util::inspect_default(rt, &a),
                    crate::util::inspect_default(rt, &b)
                )
            } else {
                arg_msg(args, 2, "equal failed")
            };
            Err(assert_error_with_fields(
                rt,
                msg,
                a,
                b,
                "==",
                generated,
                assert_instance_diff(rt).as_deref(),
            ))
        }
    });
    register_method(rt, assert, "notEqual", |rt, args| {
        if args.len() < 2 {
            return Err(missing_args_error(rt));
        }
        let a = args.first().cloned().unwrap_or(Value::Undefined);
        let b = args.get(1).cloned().unwrap_or(Value::Undefined);
        if !loose_eq(&a, &b) {
            Ok(Value::Undefined)
        } else {
            let generated = args.get(2).is_none();
            let msg = if generated {
                format!(
                    "{} != {}",
                    assert_scalar_display(&a),
                    assert_scalar_display(&b)
                )
            } else {
                arg_msg(args, 2, "notEqual failed")
            };
            Err(assert_error_with_fields(
                rt,
                msg,
                a,
                b,
                "!=",
                generated,
                assert_instance_diff(rt).as_deref(),
            ))
        }
    });
    register_method(rt, assert, "strictEqual", |rt, args| {
        if args.len() < 2 {
            return Err(missing_args_error(rt));
        }
        let a = args.first().cloned().unwrap_or(Value::Undefined);
        let b = args.get(1).cloned().unwrap_or(Value::Undefined);
        if strict_eq(&a, &b) {
            Ok(Value::Undefined)
        } else {
            if let Some(err) = custom_error_message_arg(rt, args, 2) {
                return Err(RuntimeError::Thrown(err));
            }
            let diff = assert_instance_diff(rt);
            let generated = args.get(2).is_none();

            let a_obj = matches!(&a, Value::Object(_)) && !rt.is_callable(&a);
            let b_obj = matches!(&b, Value::Object(_)) && !rt.is_callable(&b);
            let a_fn = rt.is_callable(&a);
            let b_fn = rt.is_callable(&b);
            let structural_operator = if (a_obj && b_obj) || (a_fn && b_fn) {
                "strictEqualObject"
            } else {
                "strictEqual"
            };
            let msg = match args.get(2) {
                None => assert_structural_diff(rt, &a, &b, structural_operator, None)
                    .unwrap_or_else(|| {
                        generated_assert_message(
                            &a,
                            &b,
                            "strictEqual",
                            diff.as_deref(),
                            "strictEqual failed",
                        )
                    }),

                Some(Value::String(s)) => {
                    let custom = s.as_str().to_string();
                    assert_structural_diff(rt, &a, &b, structural_operator, Some(&custom))
                        .unwrap_or(custom)
                }
                Some(_) => arg_msg(args, 2, "strictEqual failed"),
            };
            Err(assert_error_with_fields(
                rt,
                msg,
                a,
                b,
                "strictEqual",
                generated,
                diff.as_deref(),
            ))
        }
    });
    register_method(rt, assert, "notStrictEqual", |rt, args| {
        if args.len() < 2 {
            return Err(missing_args_error(rt));
        }
        let a = args.first().cloned().unwrap_or(Value::Undefined);
        let b = args.get(1).cloned().unwrap_or(Value::Undefined);
        if !strict_eq(&a, &b) {
            Ok(Value::Undefined)
        } else {
            let diff = assert_instance_diff(rt);
            let generated = args.get(2).is_none();
            let msg = if generated {

                match assert_negated_single_message(rt, "notStrictEqual", &a) {
                    Some(s) => s,
                    None => generated_assert_message(
                        &a,
                        &b,
                        "notStrictEqual",
                        diff.as_deref(),
                        "notStrictEqual failed",
                    ),
                }
            } else {
                arg_msg(args, 2, "notStrictEqual failed")
            };
            Err(assert_error_with_fields(
                rt,
                msg,
                a,
                b,
                "notStrictEqual",
                generated,
                diff.as_deref(),
            ))
        }
    });

    register_method(rt, assert, "deepEqual", |rt, args| {
        if args.len() < 2 {
            return Err(missing_args_error(rt));
        }
        let a = args.first().cloned().unwrap_or(Value::Undefined);
        let b = args.get(1).cloned().unwrap_or(Value::Undefined);
        if deep_equal_checked(rt, &a, &b, false)? {
            Ok(Value::Undefined)
        } else {
            let diff = assert_instance_diff(rt);
            let generated = args.get(2).is_none();
            let msg = if generated {

                match assert_loose_deep_diff(rt, &a, &b) {
                    Some(s) => s,
                    None => generated_assert_message(
                        &a,
                        &b,
                        "deepEqual",
                        diff.as_deref(),
                        "deepEqual failed",
                    ),
                }
            } else {
                arg_msg(args, 2, "deepEqual failed")
            };
            Err(assert_error_with_fields(
                rt,
                msg,
                a,
                b,
                "deepEqual",
                generated,
                diff.as_deref(),
            ))
        }
    });
    register_method(rt, assert, "notDeepEqual", |rt, args| {
        if args.len() < 2 {
            return Err(missing_args_error(rt));
        }
        let a = args.first().cloned().unwrap_or(Value::Undefined);
        let b = args.get(1).cloned().unwrap_or(Value::Undefined);
        if !deep_equal_checked(rt, &a, &b, false)? {
            Ok(Value::Undefined)
        } else {
            let generated = args.get(2).is_none();
            let msg = if generated {
                assert_not_loose_deep_message(rt, &a, &b)
                    .unwrap_or_else(|| "notDeepEqual failed".to_string())
            } else {
                arg_msg(args, 2, "notDeepEqual failed")
            };
            Err(assert_error_with_fields(
                rt,
                msg,
                a,
                b,
                "notDeepEqual",
                generated,
                assert_instance_diff(rt).as_deref(),
            ))
        }
    });
    register_method(rt, assert, "deepStrictEqual", |rt, args| {
        if args.len() < 2 {
            return Err(missing_args_error(rt));
        }
        let a = args.first().cloned().unwrap_or(Value::Undefined);
        let b = args.get(1).cloned().unwrap_or(Value::Undefined);
        if deep_equal_checked(rt, &a, &b, true)? {
            Ok(Value::Undefined)
        } else {
            let diff = assert_instance_diff(rt);
            let generated = args.get(2).is_none();
            let msg = match args.get(2) {

                None => assert_structural_diff(rt, &a, &b, "deepStrictEqual", None).unwrap_or_else(
                    || {
                        generated_assert_message(
                            &a,
                            &b,
                            "deepStrictEqual",
                            diff.as_deref(),
                            "deepStrictEqual failed",
                        )
                    },
                ),

                Some(Value::String(s)) => {
                    let custom = s.as_str().to_string();
                    assert_structural_diff(rt, &a, &b, "deepStrictEqual", Some(&custom))
                        .unwrap_or(custom)
                }

                Some(_) => arg_msg(args, 2, "deepStrictEqual failed"),
            };
            Err(assert_error_with_fields(
                rt,
                msg,
                a,
                b,
                "deepStrictEqual",
                generated,
                diff.as_deref(),
            ))
        }
    });
    register_method(rt, assert, "notDeepStrictEqual", |rt, args| {
        if args.len() < 2 {
            return Err(missing_args_error(rt));
        }
        let a = args.first().cloned().unwrap_or(Value::Undefined);
        let b = args.get(1).cloned().unwrap_or(Value::Undefined);
        if !deep_equal_checked(rt, &a, &b, true)? {
            Ok(Value::Undefined)
        } else {
            let generated = args.get(2).is_none();
            let msg = if generated {
                assert_negated_single_message(rt, "notDeepStrictEqual", &a)
                    .unwrap_or_else(|| "notDeepStrictEqual failed".to_string())
            } else {
                arg_msg(args, 2, "notDeepStrictEqual failed")
            };
            Err(assert_error_with_fields(
                rt,
                msg,
                a,
                b,
                "notDeepStrictEqual",
                generated,
                assert_instance_diff(rt).as_deref(),
            ))
        }
    });
    register_method(rt, assert, "partialDeepStrictEqual", |rt, args| {
        if args.len() < 2 {
            return Err(missing_args_error(rt));
        }
        let actual = args.first().cloned().unwrap_or(Value::Undefined);
        let expected = args.get(1).cloned().unwrap_or(Value::Undefined);
        if partial_deep_equal_checked(rt, &actual, &expected, true)? {
            Ok(Value::Undefined)
        } else {
            Err(assert_error_with_fields(
                rt,
                arg_msg(args, 2, "partialDeepStrictEqual failed"),
                actual,
                expected,
                "partialDeepStrictEqual",
                args.get(2).is_none(),
                assert_instance_diff(rt).as_deref(),
            ))
        }
    });

    register_method(rt, assert, "fail", |rt, args| {
        if args.len() == 1 {
            if let Some(Value::Object(id)) = args.first() {
                if matches!(rt.obj(*id).internal_kind, InternalKind::Error) {
                    return Err(RuntimeError::Thrown(Value::Object(*id)));
                }
            }
        }
        Err(assert_error_with_fields(
            rt,
            arg_msg(args, 0, "Failed"),
            Value::Undefined,
            Value::Undefined,
            "fail",
            args.first().is_none(),
            assert_instance_diff(rt).as_deref(),
        ))
    });

    register_method(rt, assert, "throws", |rt, args| {
        let f = args.first().cloned().unwrap_or(Value::Undefined);
        validate_assert_callable_fn(rt, &f)?;
        match rt.call_function(f, Value::Undefined, vec![]) {
            Err(RuntimeError::Thrown(thrown)) => {
                let matcher = args.get(1).cloned().unwrap_or(Value::Undefined);
                assert_validate_throws_matcher(rt, &thrown, &matcher)?;
                if assert_throws_matcher_accepts(rt, &thrown, &matcher)? {
                    Ok(Value::Undefined)
                } else {
                    Err(assert_throws_mismatch_error(rt, thrown, matcher))
                }
            }
            Err(_) => Ok(Value::Undefined),
            Ok(_) => {
                let matcher = args.get(1).cloned().unwrap_or(Value::Undefined);
                let msg = match assert_matcher_constructor_name(rt, &matcher) {
                    Some(name) => format!("Missing expected exception ({name})."),
                    None => "Missing expected exception.".into(),
                };
                Err(assert_error_with_fields(
                    rt,
                    msg,
                    Value::Undefined,
                    Value::Undefined,
                    "throws",
                    false,
                    assert_instance_diff(rt).as_deref(),
                ))
            }
        }
    });
    register_method(rt, assert, "doesNotThrow", |rt, args| {
        let f = args.first().cloned().unwrap_or(Value::Undefined);
        validate_assert_callable_fn(rt, &f)?;
        match rt.call_function(f, Value::Undefined, vec![]) {
            Ok(_) => Ok(Value::Undefined),
            Err(RuntimeError::Thrown(thrown)) => {
                let matcher = args.get(1).cloned().unwrap_or(Value::Undefined);
                if matches!(matcher, Value::Undefined)
                    || assert_throws_matcher_accepts(rt, &thrown, &matcher)?
                {
                    let msg = does_not_throw_assertion_message(rt, args, &thrown);
                    let diff = assert_instance_diff(rt);
                    Err(assert_error_with_fields(
                        rt,
                        msg,
                        thrown.clone(),
                        args.get(1).cloned().unwrap_or(Value::Undefined),
                        "doesNotThrow",
                        false,
                        diff.as_deref(),
                    ))
                } else {
                    Err(RuntimeError::Thrown(thrown))
                }
            }
            Err(other) => Err(other),
        }
    });

    register_method(rt, assert, "ifError", |rt, args| {
        let v = args.first().cloned().unwrap_or(Value::Undefined);
        match v {
            Value::Undefined | Value::Null => Ok(Value::Undefined),
            other => Err(assert_error_with_fields(
                rt,
                format!(
                    "ifError got unwanted exception: {}",
                    if_error_display(rt, &other)
                ),
                other,
                Value::Null,
                "ifError",
                true,
                assert_instance_diff(rt).as_deref(),
            )),
        }
    });

    register_method(rt, assert, "match", |rt, args| {
        if !matches!(args.first(), Some(Value::String(_))) {
            return Err(assert_match_string_arg_error(rt, args, "match"));
        }
        if regexp_test(rt, args)? {
            Ok(Value::Undefined)
        } else {
            if let Some(err) = custom_error_message_arg(rt, args, 2) {
                return Err(RuntimeError::Thrown(err));
            }
            let default = assert_regex_default_msg(rt, args, false);
            let generated = matches!(args.get(2), None | Some(Value::Undefined));
            let msg = arg_msg(args, 2, &default);
            let diff = assert_instance_diff(rt);
            Err(assert_error_with_fields(
                rt,
                msg,
                args.first().cloned().unwrap_or(Value::Undefined),
                args.get(1).cloned().unwrap_or(Value::Undefined),
                "match",
                generated,
                diff.as_deref(),
            ))
        }
    });
    register_method(rt, assert, "doesNotMatch", |rt, args| {
        if !matches!(args.first(), Some(Value::String(_))) {
            return Err(assert_match_string_arg_error(rt, args, "doesNotMatch"));
        }
        if !regexp_test(rt, args)? {
            Ok(Value::Undefined)
        } else {
            if let Some(err) = custom_error_message_arg(rt, args, 2) {
                return Err(RuntimeError::Thrown(err));
            }
            let default = assert_regex_default_msg(rt, args, true);
            let generated = matches!(args.get(2), None | Some(Value::Undefined));
            let msg = arg_msg(args, 2, &default);
            let diff = assert_instance_diff(rt);
            Err(assert_error_with_fields(
                rt,
                msg,
                args.first().cloned().unwrap_or(Value::Undefined),
                args.get(1).cloned().unwrap_or(Value::Undefined),
                "doesNotMatch",
                generated,
                diff.as_deref(),
            ))
        }
    });

    register_method(rt, assert, "rejects", |rt, args| rejects_impl(rt, args));
    register_method(rt, assert, "doesNotReject", |rt, args| {
        does_not_reject_impl(rt, args)
    });

    let assertion_error_ctor = make_callable(rt, "AssertionError", |rt, args| {
        let this = match rt.current_this() {
            Value::Object(obj) => obj,
            _ => rt.alloc_object(rusty_js_runtime::value::Object::new_ordinary()),
        };
        let message = match args.first() {
            Some(Value::Object(opts)) => match rt.object_get(*opts, "message") {
                Value::String(s) => s.as_str().to_string(),
                _ => String::new(),
            },
            Some(v) => return Err(invalid_options_arg_error(rt, v)),
            None => return Err(invalid_options_arg_error(rt, &Value::Undefined)),
        };
        rt.object_set(
            this,
            "name".into(),
            Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                "AssertionError",
            ))),
        );
        rt.object_set(
            this,
            "message".into(),
            Value::String(Rc::new(rusty_js_runtime::value::JsString::from(message))),
        );
        rt.object_set(
            this,
            "code".into(),
            Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                "ERR_ASSERTION",
            ))),
        );
        Ok(Value::Object(this))
    });
    set_constant(
        rt,
        assertion_error_ctor,
        "prototype",
        Value::Object(ae_proto),
    );
    rt.object_set(
        ae_proto,
        "constructor".into(),
        Value::Object(assertion_error_ctor),
    );
    set_constant(
        rt,
        assert,
        "AssertionError",
        Value::Object(assertion_error_ctor),
    );

    let strict = make_callable(rt, "strict", ok_impl);

    let strict_map: &[(&str, &str)] = &[
        ("ok", "ok"),
        ("strictEqual", "equal"),
        ("notStrictEqual", "notEqual"),
        ("deepStrictEqual", "deepEqual"),
        ("notDeepStrictEqual", "notDeepEqual"),
        ("strictEqual", "strictEqual"),
        ("notStrictEqual", "notStrictEqual"),
        ("deepStrictEqual", "deepStrictEqual"),
        ("partialDeepStrictEqual", "partialDeepStrictEqual"),
        ("notDeepStrictEqual", "notDeepStrictEqual"),
        ("throws", "throws"),
        ("doesNotThrow", "doesNotThrow"),
        ("ifError", "ifError"),
        ("match", "match"),
        ("doesNotMatch", "doesNotMatch"),
        ("rejects", "rejects"),
        ("doesNotReject", "doesNotReject"),
        ("fail", "fail"),
    ];
    for (src, dst) in strict_map {
        let m = rt.object_get(assert, src);
        rt.object_set(strict, (*dst).to_string(), m);
    }
    rt.object_set(
        strict,
        "AssertionError".into(),
        Value::Object(assertion_error_ctor),
    );
    rt.object_set(strict, "strict".into(), Value::Object(strict));
    set_constant(rt, assert, "strict", Value::Object(strict));

    let assert_class_proto = new_object(rt);
    let assert_class = make_callable_rooted(
        rt,
        "Assert",
        vec![assert, strict, assertion_error_ctor],
        move |rt, args| {
            if rt.current_new_target.is_none() {
                return Err(node_code_type_error(
                    rt,
                    "ERR_CONSTRUCT_CALL_REQUIRED",
                    "Class constructor Assert cannot be invoked without 'new'",
                ));
            }
            let opts = args.first().cloned().unwrap_or(Value::Undefined);
            let mut strict_mode = true;
            let mut diff_mode = "simple".to_string();
            let mut skip_prototype = false;
            if let Value::Object(obj) = opts {
                match rt.object_get(obj, "diff") {
                    Value::String(s) if s.as_str() == "simple" || s.as_str() == "full" => {
                        diff_mode = s.as_str().to_string();
                    }
                    Value::Undefined => {}
                    _ => {
                        return Err(node_code_type_error(
                            rt,
                            "ERR_INVALID_ARG_VALUE",
                            "The property 'options.diff' must be one of: 'simple', 'full'",
                        ));
                    }
                }
                if matches!(rt.object_get(obj, "strict"), Value::Boolean(false)) {
                    strict_mode = false;
                }
                if matches!(rt.object_get(obj, "skipPrototype"), Value::Boolean(true)) {
                    skip_prototype = true;
                }
            }
            let this = match rt.current_this() {
                Value::Object(obj) => obj,
                _ => rt.alloc_object(Object::new_ordinary()),
            };
            install_assert_instance_surface(
                rt,
                this,
                assert,
                strict,
                assertion_error_ctor,
                strict_mode,
                &diff_mode,
                skip_prototype,
            );
            Ok(Value::Object(this))
        },
    );
    rt.object_set(
        assert_class_proto,
        "constructor".into(),
        Value::Object(assert_class),
    );
    rt.object_set(
        assert_class,
        "prototype".into(),
        Value::Object(assert_class_proto),
    );
    set_constant(rt, assert, "Assert", Value::Object(assert_class));
    rt.object_set(strict, "Assert".into(), Value::Object(assert_class));

    let internal_myers = new_object(rt);
    register_method(rt, internal_myers, "myersDiff", |rt, args| {
        let len = |rt: &Runtime, v: Option<&Value>| -> f64 {
            match v {
                Some(Value::Object(id)) => match rt.object_get(*id, "length") {
                    Value::Number(n) if n.is_finite() && n >= 0.0 => n,
                    _ => 0.0,
                },
                _ => 0.0,
            }
        };
        let total = len(rt, args.first()) + len(rt, args.get(1));
        if total >= 2_147_483_648.0 {
            let received = if total.fract() == 0.0 {
                format!("{}", total as i64)
            } else {
                abstract_ops::to_string(&Value::Number(total))
                    .as_str()
                    .to_string()
            };
            return Err(node_code_range_error(
                rt,
                "ERR_OUT_OF_RANGE",
                &format!(
                    "The value of \"myersDiff input size\" is out of range. It must be < 2^31. Received {received}"
                ),
            ));
        }
        Ok(Value::Object(rt.alloc_object(Object::new_array())))
    });
    rt.define_global_property("__node_assert_strict", Value::Object(strict));
    rt.define_global_property("__node_assert", Value::Object(assert));
    rt.define_global_property(
        "__cruft_internal_assert_myers_diff",
        Value::Object(internal_myers),
    );
}

fn regexp_test(rt: &mut Runtime, args: &[Value]) -> Result<bool, RuntimeError> {
    let s = args.first().cloned().unwrap_or(Value::Undefined);
    let re = args.get(1).cloned().unwrap_or(Value::Undefined);
    let re_id = match re {
        Value::Object(id) => id,
        _ => {
            return Err(node_code_type_error(
                rt,
                "ERR_INVALID_ARG_TYPE",
                "The \"regexp\" argument must be an instance of RegExp. Received type string ('string')",
            ))
        }
    };
    let test = rt.object_get(re_id, "test");
    let r = rt.call_function(test, Value::Object(re_id), vec![s])?;
    Ok(abstract_ops::to_boolean(&r))
}

fn assert_match_string_arg_error(rt: &mut Runtime, args: &[Value], operator: &str) -> RuntimeError {
    let actual = args.first().cloned().unwrap_or(Value::Undefined);
    let expected = args.get(1).cloned().unwrap_or(Value::Undefined);

    let type_name = rt.type_of_value(&actual);
    let rendered = crate::util::inspect_default(rt, &actual);
    assert_error_with_fields(
        rt,
        format!(
            "The \"string\" argument must be of type string. Received type {type_name} ({rendered})"
        ),
        actual,
        expected,
        operator,
        true,
        assert_instance_diff(rt).as_deref(),
    )
}

fn rejects_impl(rt: &mut Runtime, args: &[Value]) -> Result<Value, RuntimeError> {
    let mut actual = args.first().cloned().unwrap_or(Value::Undefined);
    let expected = args.get(1).cloned().unwrap_or(Value::Undefined);
    let user_message = match args.get(2) {
        Some(Value::String(s)) => Some(s.as_str().to_string()),
        Some(v) if !matches!(v, Value::Undefined) => {
            Some(abstract_ops::to_string(v).as_str().to_string())
        }
        _ => None,
    };
    let from_fn = rt.is_callable(&actual);
    if rt.is_callable(&actual) {
        actual = match rt.call_function(actual, Value::Undefined, vec![]) {
            Ok(value) => value,
            Err(RuntimeError::Thrown(thrown)) => return sync_throw_rejected_promise(rt, thrown),
            Err(err) => return Err(err),
        };
    }

    if !accepted_async_assert_subject(rt, &actual) {
        let err = if from_fn {
            async_assert_invalid_return_value(rt, &actual)
        } else {
            async_assert_invalid_arg_value(rt, &actual)
        };
        return suppressed_rejected_promise(rt, err);
    }

    let out = new_promise(rt);
    let fulfilled_expected = expected.clone();
    let on_fulfilled = Value::Object(make_callable_rooted(
        rt,
        "assertRejectsFulfilled",
        vec![out],
        move |rt, _args| {
            let msg = if rt.is_callable(&fulfilled_expected) {
                match &fulfilled_expected {
                    Value::Object(id) => match rt.object_get(*id, "name") {
                        Value::String(name) if !name.as_str().is_empty() => {
                            format!("Missing expected rejection ({}).", name.as_str())
                        }
                        _ => "Missing expected rejection.".to_string(),
                    },
                    _ => "Missing expected rejection.".to_string(),
                }
            } else {
                "Missing expected rejection.".to_string()
            };
            let err = build_assertion_error(rt, msg);
            if let Value::Object(id) = &err {
                rt.object_set(
                    *id,
                    "operator".into(),
                    Value::String(Rc::new(rusty_js_runtime::value::JsString::from("rejects"))),
                );
            }
            reject_promise(rt, out, err);
            Ok(Value::Undefined)
        },
    ));
    let on_rejected = Value::Object(make_callable_rooted(
        rt,
        "assertRejectsRejected",
        vec![out],
        move |rt, args| {
            let reason = args.first().cloned().unwrap_or(Value::Undefined);
            if rt.is_callable(&expected) {

                if rt.instanceof_operator(&reason, &expected).unwrap_or(false) {
                    resolve_promise(rt, out, Value::Undefined);
                    return Ok(Value::Undefined);
                }
                let is_error_ctor = match &expected {
                    Value::Object(exp_id) => match rt.object_get(*exp_id, "prototype") {
                        Value::Object(proto) => {
                            assert_matcher_is_error_constructor(rt, *exp_id, proto)
                        }
                        _ => false,
                    },
                    _ => false,
                };
                if is_error_ctor {
                    let err = assert_rejects_mismatch_error(
                        rt,
                        reason,
                        expected.clone(),
                        user_message.clone(),
                    );
                    reject_promise(rt, out, err);
                    return Ok(Value::Undefined);
                }
                let ok =
                    rt.call_function(expected.clone(), Value::Undefined, vec![reason.clone()])?;
                if !matches!(ok, Value::Boolean(true)) {
                    let received = async_assert_value_name(rt, &ok);
                    let caught = assert_rejects_caught_display(rt, &reason);
                    let err = assert_rejects_mismatch_error(
                        rt,
                        reason,
                        expected.clone(),
                        Some(format!(
                            "The \"validate\" validation function is expected to return \"true\". Received {received}\n\nCaught error:\n\n{caught}"
                        )),
                    );
                    reject_promise(rt, out, err);
                    return Ok(Value::Undefined);
                }
            } else if !assert_rejects_expected_matches(rt, &reason, &expected) {
                let err = assert_rejects_mismatch_error(
                    rt,
                    reason,
                    expected.clone(),
                    user_message.clone(),
                );
                reject_promise(rt, out, err);
                return Ok(Value::Undefined);
            }
            resolve_promise(rt, out, Value::Undefined);
            Ok(Value::Undefined)
        },
    ));
    let promise = rt.promise_resolve_via(&actual)?;
    let _ = rt.promise_then_via(&[promise, on_fulfilled, on_rejected])?;
    Ok(Value::Object(out))
}

fn does_not_reject_impl(rt: &mut Runtime, args: &[Value]) -> Result<Value, RuntimeError> {
    let mut actual = args.first().cloned().unwrap_or(Value::Undefined);
    let expected = args.get(1).cloned().unwrap_or(Value::Undefined);
    let from_fn = rt.is_callable(&actual);
    if from_fn {
        actual = match rt.call_function(actual, Value::Undefined, vec![]) {
            Ok(value) => value,
            Err(RuntimeError::Thrown(thrown)) => return sync_throw_rejected_promise(rt, thrown),
            Err(err) => return Err(err),
        };
    }

    if !accepted_async_assert_subject(rt, &actual) {
        let err = if from_fn {
            async_assert_invalid_return_value(rt, &actual)
        } else {
            async_assert_invalid_arg_value(rt, &actual)
        };
        return suppressed_rejected_promise(rt, err);
    }

    let out = new_promise(rt);
    let on_fulfilled = Value::Object(make_callable_rooted(
        rt,
        "assertDoesNotRejectFulfilled",
        vec![out],
        move |rt, _args| {
            resolve_promise(rt, out, Value::Undefined);
            Ok(Value::Undefined)
        },
    ));
    let on_rejected = Value::Object(make_callable_rooted(
        rt,
        "assertDoesNotRejectRejected",
        vec![out],
        move |rt, args| {
            let reason = args.first().cloned().unwrap_or(Value::Undefined);
            if rt.is_callable(&expected) {
                let _ =
                    rt.call_function(expected.clone(), Value::Undefined, vec![reason.clone()])?;
            }
            let msg = match &reason {
                Value::Object(id) => match rt.object_get(*id, "message") {
                    Value::String(s) if !s.as_str().is_empty() => {
                        format!(
                            "Got unwanted rejection.\nActual message: \"{}\"",
                            s.as_str()
                        )
                    }
                    _ => "Got unwanted rejection.".to_string(),
                },
                _ => "Got unwanted rejection.".to_string(),
            };
            let err = build_assertion_error(rt, msg);
            if let Value::Object(id) = &err {
                rt.object_set(*id, "actual".into(), reason);
                rt.object_set(
                    *id,
                    "operator".into(),
                    Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                        "doesNotReject",
                    ))),
                );
                rt.object_set(
                    *id,
                    "stack".into(),
                    Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                        "AssertionError [ERR_ASSERTION]\n    at processTicksAndRejections",
                    ))),
                );
            }
            reject_promise(rt, out, err);
            Ok(Value::Undefined)
        },
    ));
    let promise = rt.promise_resolve_via(&actual)?;
    let _ = rt.promise_then_via(&[promise, on_fulfilled, on_rejected])?;
    Ok(Value::Object(out))
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
