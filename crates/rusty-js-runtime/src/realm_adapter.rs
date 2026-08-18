
use crate::caps::CapMode;
use crate::interp::{Runtime, RuntimeError};
use crate::value::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoundaryPolicy {

    NodeCompat,

    Parameterized,

    PrimitiveOnlyWithWrappedCallable,

    SharedHeapIdentity,
}

pub type Endowments = std::collections::HashMap<String, Value>;

pub fn realm_alloc(
    rt: &mut Runtime,
    ambient_denied: bool,
    endowments: Endowments,
    capability_mode: CapMode,
) -> usize {

    let idx = if ambient_denied {

        rt.allocate_compartment_realm(std::collections::HashMap::new())
    } else {
        rt.allocate_realm()
    };
    for (k, v) in endowments {

        let translated =
            crate::intrinsics::grant_translate(rt, idx, &v).unwrap_or(v);
        rt.realms[idx].globals_overrides.insert(k, translated);
    }

    rt.realms[idx].capability_mode = capability_mode;
    idx
}

pub fn realm_capability_mode(rt: &Runtime, realm_idx: usize) -> CapMode {
    rt.realms[realm_idx].capability_mode
}

pub fn effective_capability_mode_for_realm(rt: &Runtime, realm_idx: usize) -> CapMode {
    fn strictness(m: CapMode) -> u8 {
        match m {
            CapMode::Compat => 0,
            CapMode::Audit => 1,
            CapMode::SealedDeps => 2,
            CapMode::Sealed => 3,
        }
    }
    let realm_mode = rt.realms[realm_idx].capability_mode;

    let process_mode = rt.caps.mode;
    if strictness(realm_mode) >= strictness(process_mode) {
        realm_mode
    } else {
        process_mode
    }
}

pub fn realm_evaluate(
    rt: &mut Runtime,
    realm_idx: usize,
    realm_globalthis: Option<rusty_js_gc::ObjectId>,
    source: &str,
    url: &str,
) -> Result<Value, RuntimeError> {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static EVAL_COUNTER: AtomicUsize = AtomicUsize::new(0);
    let n = EVAL_COUNTER.fetch_add(1, Ordering::Relaxed);
    let stash_key = format!("__realm_adapter_out_{}", n);
    let expr_candidate = source.trim().trim_end_matches(';').trim_end();
    let expr_source = format!("{} = ({});", stash_key, expr_candidate);
    let trimmed = source.trim_start();

    let statement_leading = trimmed.starts_with('{')
        || trimmed.starts_with(';')
        || [
            "function",
            "async function",
            "class",
            "var",
            "let",
            "const",
            "throw",
            "if",
            "for",
            "while",
            "switch",
            "try",
            "do",
            "with",
            "debugger",
            "return",
            "break",
            "continue",
            "import",
            "export",
        ]
        .iter()
        .any(|kw| {
            trimmed.strip_prefix(kw).is_some_and(|rest| {
                rest.is_empty()
                    || !rest.starts_with(|c: char| c.is_alphanumeric() || c == '_' || c == '$')
            })
        });
    let has_strict_directive = source.contains("\"use strict\"") || source.contains("'use strict'");

    let prior_realm = rt.current_realm;
    let prior_gt = rt.global_object;
    let prior_this = std::mem::replace(&mut rt.current_this, Value::Undefined);
    let swapped_global_env = if realm_idx == 0 {
        None
    } else {
        let prior = (
            std::mem::take(&mut rt.global_lexical_bindings),
            std::mem::take(&mut rt.global_immutable_lexical_bindings),
            std::mem::take(&mut rt.global_var_names),
        );
        rt.global_lexical_bindings =
            std::mem::take(&mut rt.realms[realm_idx].global_lexical_bindings);
        rt.global_immutable_lexical_bindings =
            std::mem::take(&mut rt.realms[realm_idx].global_immutable_lexical_bindings);
        rt.global_var_names = std::mem::take(&mut rt.realms[realm_idx].global_var_names);
        Some(prior)
    };
    rt.current_realm = realm_idx;
    if let Some(gt) = realm_globalthis {
        rt.global_object = Some(gt);
        rt.current_this = Value::Object(gt);
    } else {
        rt.current_this = prior_this.clone();
    }
    rt.current_module_url.push(url.to_string());

    let expr_shaped = !statement_leading && rusty_js_parser::parse_script(&expr_source).is_ok();

    let result = if expr_shaped {
        let expr_result = rt.evaluate_script(&expr_source, url);
        rt.module_remove(url);
        match expr_result {
            Ok(_) => {
                let v = match realm_globalthis {
                    Some(gt) => {
                        let from_context = rt.object_get(gt, &stash_key);
                        if matches!(from_context, Value::Undefined) {
                            rt.global_get(&stash_key)
                        } else {
                            from_context
                        }
                    }
                    None => rt.global_get(&stash_key),
                };
                if let Some(gt) = realm_globalthis {
                    rt.obj_mut(gt).remove_str(&stash_key);
                }
                if let Some(gt) = rt.global_object {
                    rt.obj_mut(gt).remove_str(&stash_key);
                }
                Ok(v)
            }
            Err(e)
                if (matches!(e, RuntimeError::CompileError(_)) && !has_strict_directive)
                    || matches!(
                    &e,
                    RuntimeError::SyntaxError(msg)
                        if msg.contains("expected `RParen`")
                            || msg.contains("unexpected token in expression: Punct(RParen)")
                    ) =>
            {

                let stmt_url = format!("{}#stmt", url);
                let completion_key = format!("__realm_adapter_cmpl_{}", n);
                rt.define_global_property(&completion_key, Value::Undefined);
                rusty_js_bytecode::compiler::set_eval_completion_stash(Some(
                    completion_key.clone(),
                ));
                let statement_source =
                    realm_statement_source_with_tail_stash(source, &completion_key);
                let r = rt.run_script(&statement_source, &stmt_url);
                rusty_js_bytecode::compiler::set_eval_completion_stash(None);
                let mut completion =
                    realm_read_globalish_stash(rt, realm_globalthis, &completion_key);
                if matches!(completion, Value::Undefined) {
                    if let Ok(v) = &r {
                        if !matches!(v, Value::Undefined) {
                            completion = v.clone();
                        }
                    }
                }
                if matches!(completion, Value::Undefined) {
                    if let Some(v) = realm_tail_function_expression_completion(
                        rt,
                        realm_globalthis,
                        source,
                        &stmt_url,
                        n,
                    )? {
                        completion = v;
                    }
                }
                realm_remove_globalish_stash(rt, realm_globalthis, &completion_key);
                match r {
                    Ok(_) => Ok(completion),
                    Err(RuntimeError::CompileError(msg)) => Err(RuntimeError::SyntaxError(msg)),
                    Err(e) => Err(e),
                }
            }
            Err(e) => Err(e),
        }
    } else {
        let stmt_url = format!("{}#stmt", url);
        let completion_key = format!("__realm_adapter_cmpl_{}", n);
        rt.define_global_property(&completion_key, Value::Undefined);
        rusty_js_bytecode::compiler::set_eval_completion_stash(Some(completion_key.clone()));
        let statement_source = realm_statement_source_with_tail_stash(source, &completion_key);
        let r = rt.run_script(&statement_source, &stmt_url);
        rusty_js_bytecode::compiler::set_eval_completion_stash(None);
        let mut completion = realm_read_globalish_stash(rt, realm_globalthis, &completion_key);
        if matches!(completion, Value::Undefined) {
            if let Ok(v) = &r {
                if !matches!(v, Value::Undefined) {
                    completion = v.clone();
                }
            }
        }
        if matches!(completion, Value::Undefined) {
            if let Some(v) = realm_tail_function_expression_completion(
                rt,
                realm_globalthis,
                source,
                &stmt_url,
                n,
            )? {
                completion = v;
            }
        }
        realm_remove_globalish_stash(rt, realm_globalthis, &completion_key);
        match r {
            Ok(_) => Ok(completion),
            Err(RuntimeError::CompileError(msg)) => Err(RuntimeError::SyntaxError(msg)),
            Err(e) => Err(e),
        }
    };

    rt.current_module_url.pop();

    if let Some((lexical, immutable, var_names)) = swapped_global_env {

        let _ = std::mem::take(&mut rt.global_lexical_bindings);
        let _ = std::mem::take(&mut rt.global_immutable_lexical_bindings);
        rt.realms[realm_idx].global_lexical_bindings = Default::default();
        rt.realms[realm_idx].global_immutable_lexical_bindings = Default::default();
        rt.realms[realm_idx].global_var_names = std::mem::take(&mut rt.global_var_names);
        rt.global_lexical_bindings = lexical;
        rt.global_immutable_lexical_bindings = immutable;
        rt.global_var_names = var_names;
    }
    rt.current_realm = prior_realm;
    rt.global_object = prior_gt;
    rt.current_this = prior_this;
    result
}

fn realm_read_globalish_stash(
    rt: &mut Runtime,
    realm_globalthis: Option<rusty_js_gc::ObjectId>,
    key: &str,
) -> Value {
    match realm_globalthis {
        Some(gt) => {
            let from_context = rt.object_get(gt, key);
            if matches!(from_context, Value::Undefined) {
                rt.global_get(key)
            } else {
                from_context
            }
        }
        None => rt.global_get(key),
    }
}

fn realm_statement_source_with_tail_stash<'a>(
    source: &'a str,
    completion_key: &str,
) -> std::borrow::Cow<'a, str> {
    let Some(tail_start) = find_top_level_parenthesized_function_tail(source) else {
        return std::borrow::Cow::Borrowed(source);
    };
    let tail = source[tail_start..].trim().trim_end_matches(';').trim_end();
    std::borrow::Cow::Owned(format!(
        "{}{} = {};",
        &source[..tail_start],
        completion_key,
        tail
    ))
}

fn realm_remove_globalish_stash(
    rt: &mut Runtime,
    realm_globalthis: Option<rusty_js_gc::ObjectId>,
    key: &str,
) {
    if let Some(gt) = realm_globalthis {
        rt.obj_mut(gt).remove_str(key);
    }
    if let Some(gt) = rt.global_object {
        rt.obj_mut(gt).remove_str(key);
    }
}

fn realm_tail_function_expression_completion(
    rt: &mut Runtime,
    realm_globalthis: Option<rusty_js_gc::ObjectId>,
    source: &str,
    url: &str,
    n: usize,
) -> Result<Option<Value>, RuntimeError> {
    let tail_start = find_top_level_parenthesized_function_tail(source);
    let Some(tail_start) = tail_start else {
        return Ok(None);
    };
    let last = source[tail_start..].trim().trim_end_matches(';').trim_end();
    let inner = last
        .strip_prefix('(')
        .and_then(|s| s.strip_suffix(')'))
        .map(str::trim);
    let Some(inner) = inner else {
        return Ok(None);
    };
    if !(inner.starts_with("function")
        || inner.starts_with("async function")
        || inner.starts_with("function*")
        || inner.starts_with("async function*"))
    {
        return Ok(None);
    }
    let stash_key = format!("__realm_adapter_tail_fn_{}", n);
    let expr_source = format!("{} = ({});", stash_key, last);
    rt.define_global_property(&stash_key, Value::Undefined);
    rt.evaluate_script(&expr_source, &format!("{}#tail-fn", url))?;
    let v = match realm_globalthis {
        Some(gt) => {
            let from_context = rt.object_get(gt, &stash_key);
            if matches!(from_context, Value::Undefined) {
                rt.global_get(&stash_key)
            } else {
                from_context
            }
        }
        None => rt.global_get(&stash_key),
    };
    if let Some(gt) = realm_globalthis {
        rt.obj_mut(gt).remove_str(&stash_key);
    }
    if let Some(gt) = rt.global_object {
        rt.obj_mut(gt).remove_str(&stash_key);
    }
    Ok(Some(v))
}

fn find_top_level_parenthesized_function_tail(source: &str) -> Option<usize> {
    let bytes = source.as_bytes();
    let mut paren = 0usize;
    let mut brace = 0usize;
    let mut bracket = 0usize;
    let mut i = 0usize;
    while i < bytes.len() {
        match bytes[i] {
            b'\'' | b'"' => {
                i = skip_quoted_js(bytes, i)?;
                continue;
            }
            b'`' => {
                i = skip_template_js(bytes, i)?;
                continue;
            }
            b'/' if i + 1 < bytes.len() && bytes[i + 1] == b'/' => {
                i += 2;
                while i < bytes.len() && bytes[i] != b'\n' && bytes[i] != b'\r' {
                    i += 1;
                }
                continue;
            }
            b'/' if i + 1 < bytes.len() && bytes[i + 1] == b'*' => {
                i += 2;
                while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                    i += 1;
                }
                i = (i + 2).min(bytes.len());
                continue;
            }
            b'(' if paren == 0 && brace == 0 && bracket == 0 => {
                let tail = &source[i + 1..];
                if tail.starts_with("function")
                    || tail.starts_with("async function")
                    || tail.starts_with("function*")
                    || tail.starts_with("async function*")
                {
                    return Some(i);
                }
                paren += 1;
            }
            b'(' => paren += 1,
            b')' => paren = paren.saturating_sub(1),
            b'{' => brace += 1,
            b'}' => brace = brace.saturating_sub(1),
            b'[' => bracket += 1,
            b']' => bracket = bracket.saturating_sub(1),
            _ => {}
        }
        i += 1;
    }
    None
}

fn skip_quoted_js(bytes: &[u8], start: usize) -> Option<usize> {
    let quote = bytes[start];
    let mut i = start + 1;
    while i < bytes.len() {
        if bytes[i] == b'\\' {
            i += 2;
            continue;
        }
        if bytes[i] == quote {
            return Some(i + 1);
        }
        i += 1;
    }
    None
}

fn skip_template_js(bytes: &[u8], start: usize) -> Option<usize> {
    let mut i = start + 1;
    while i < bytes.len() {
        if bytes[i] == b'\\' {
            i += 2;
            continue;
        }
        if bytes[i] == b'`' {
            return Some(i + 1);
        }
        i += 1;
    }
    None
}

pub fn boundary_filter(
    rt: &mut Runtime,
    value: Value,
    policy: BoundaryPolicy,
) -> Result<Value, RuntimeError> {
    match policy {

        BoundaryPolicy::SharedHeapIdentity => Ok(value),
        BoundaryPolicy::NodeCompat | BoundaryPolicy::Parameterized => match value {
            Value::Object(object) => {
                if rt.heap.owner(object) == Some(rt.current_realm) {
                    Ok(Value::Object(object))
                } else {
                    Err(RuntimeError::TypeError(
                        "boundary_filter: foreign-owned Object cannot cross by identity; use structured clone, wrapped callable, or Tier-3 external handle".into(),
                    ))
                }
            }
            other => Ok(other),
        },
        BoundaryPolicy::PrimitiveOnlyWithWrappedCallable => {

            match value {
                Value::Undefined
                | Value::Null
                | Value::Boolean(_)
                | Value::Number(_)
                | Value::String(_)
                | Value::BigInt(_)
                | Value::Symbol(_) => Ok(value),
                Value::Object(_) if rt.is_callable(&value) => {

                    Err(RuntimeError::TypeError(
                        "boundary_filter: callable values must route through wrapped_callable(target_realm) — call site missing the target_realm context".into(),
                    ))
                }
                Value::Object(_) => Err(RuntimeError::TypeError(
                    "ShadowRealm boundary: non-callable Object cannot cross the realm boundary (ECMA-262 §27.5.1.4 step 4)".into(),
                )),
            }
        }
    }
}

fn copy_wrapped_function_name_and_length(
    rt: &mut Runtime,
    wrapper: &mut crate::value::Object,
    target: &Value,
) -> Result<(), RuntimeError> {
    let length_desc = rt.object_get_own_property_descriptor_via(
        target,
        &Value::String(std::rc::Rc::new(crate::value::JsString::from(
            "length".to_string(),
        ))),
    )?;
    let length = if !matches!(length_desc, Value::Undefined) {
        match rt.spec_get(target, "length")? {
            Value::Number(n) if n == f64::INFINITY => f64::INFINITY,
            Value::Number(n) if n == f64::NEG_INFINITY || n.is_nan() => 0.0,
            Value::Number(n) => n.trunc().max(0.0),
            _ => 0.0,
        }
    } else {
        0.0
    };
    let name = match rt.spec_get(target, "name")? {
        Value::String(s) => s.as_str().to_string(),
        _ => String::new(),
    };
    crate::value::install_function_meta_props(&mut wrapper.properties, &name, length);
    Ok(())
}

pub fn wrapped_callable(
    rt: &mut Runtime,
    target_realm: usize,
    callable: Value,
    policy: BoundaryPolicy,
) -> Result<Value, RuntimeError> {
    if !matches!(policy, BoundaryPolicy::PrimitiveOnlyWithWrappedCallable) {
        return Ok(callable);
    }
    if !rt.is_callable(&callable) {
        return Err(RuntimeError::TypeError(
            "wrapped_callable: target is not callable (ECMA-262 §27.5.3 step 2.b)".into(),
        ));
    }
    let (target_realm, callable) = match callable {
        Value::Object(id) => match (
            rt.object_get(id, "__shadowrealm_wrapped_target"),
            rt.object_get(id, "__shadowrealm_target_realm"),
        ) {
            (Value::Object(target), Value::Number(realm)) => {
                (realm as usize, Value::Object(target))
            }
            _ => (target_realm, Value::Object(id)),
        },
        other => (target_realm, other),
    };

    let target_for_meta = callable.clone();
    let target_callable = callable;
    let wrapper_creation_realm = rt.current_realm;
    let mut wrapper = crate::intrinsics::make_native(
        "wrapped",
        move |rt: &mut Runtime, args: &[Value]| -> Result<Value, RuntimeError> {

            let mut wrapped_args: Vec<Value> = Vec::with_capacity(args.len());
            let caller_realm = rt.current_realm;
            for arg in args {
                match arg {
                    Value::Undefined
                    | Value::Null
                    | Value::Boolean(_)
                    | Value::Number(_)
                    | Value::String(_)
                    | Value::BigInt(_)
                    | Value::Symbol(_) => wrapped_args.push(arg.clone()),
                    Value::Object(_) if rt.is_callable(arg) => {
                        let prior_realm = rt.enter_realm(target_realm);
                        let wrapped_arg = wrapped_callable(
                            rt,
                            caller_realm,
                            arg.clone(),
                            BoundaryPolicy::PrimitiveOnlyWithWrappedCallable,
                        );
                        rt.exit_realm(prior_realm);
                        wrapped_args.push(wrapped_arg?);
                    }
                    Value::Object(_) => {
                        return Err(RuntimeError::TypeError(
                            "wrapped callable argument is not a primitive or callable (ECMA-262 §27.5.3 step 2.b)".into(),
                        ));
                    }
                }
            }

            let prior_realm = rt.enter_realm(target_realm);
            let raw_result =
                rt.call_function(target_callable.clone(), Value::Undefined, wrapped_args);
            rt.exit_realm(prior_realm);
            let _ = caller_realm;

            match raw_result {
                Ok(v) if matches!(v, Value::Object(_)) && rt.is_callable(&v) => wrapped_callable(
                    rt,
                    target_realm,
                    v,
                    BoundaryPolicy::PrimitiveOnlyWithWrappedCallable,
                ),
                Ok(v) => boundary_filter(rt, v, BoundaryPolicy::PrimitiveOnlyWithWrappedCallable),
                Err(RuntimeError::Thrown(v)) => {

                    let msg = match &v {
                        Value::String(s) => s.as_str().to_string(),
                        _ => format!("thrown value {:?}", v),
                    };
                    Err(RuntimeError::TypeError(format!(
                        "ShadowRealm callable threw: {}",
                        msg
                    )))
                }
                Err(e) => {

                    let msg = format!("{:?}", e);
                    Err(RuntimeError::TypeError(format!(
                        "ShadowRealm callable error: {}",
                        msg
                    )))
                }
            }
        },
    );
    if let crate::value::InternalKind::Function(fi) = &mut wrapper.internal_kind {
        fi.creation_realm = wrapper_creation_realm;
    }

    let prior_realm = rt.enter_realm(target_realm);
    let copy_result = copy_wrapped_function_name_and_length(rt, &mut wrapper, &target_for_meta);
    rt.exit_realm(prior_realm);
    copy_result.map_err(|e| {
        RuntimeError::TypeError(format!(
            "ShadowRealm WrappedFunctionCreate CopyNameAndLength failed: {:?}",
            e
        ))
    })?;
    wrapper.set_own_internal("__shadowrealm_wrapped_target".into(), target_for_meta);
    wrapper.set_own_internal(
        "__shadowrealm_target_realm".into(),
        Value::Number(target_realm as f64),
    );
    Ok(Value::Object(rt.alloc_object(wrapper)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interp::Runtime;

    fn run_test_on_large_stack(f: impl FnOnce() + Send + 'static) {
        std::thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn(f)
            .expect("spawn large-stack realm adapter test runner")
            .join()
            .expect("large-stack realm adapter test runner must not panic");
    }

    fn fresh() -> Runtime {
        let mut rt = Runtime::new();
        rt.install_intrinsics();
        rt
    }

    #[test]
    fn r2_realm_alloc_ambient_denied_sets_flag() {
        run_test_on_large_stack(|| {
            let mut rt = fresh();
            let idx = realm_alloc(&mut rt, true, Endowments::new(), CapMode::Compat);
            assert!(rt.realms[idx].ambient_denied);
        });
    }

    #[test]
    fn r2_realm_alloc_ambient_open_clears_flag() {
        run_test_on_large_stack(|| {
            let mut rt = fresh();
            let idx = realm_alloc(&mut rt, false, Endowments::new(), CapMode::Compat);
            assert!(!rt.realms[idx].ambient_denied);
        });
    }

    #[test]
    fn r2_realm_alloc_endowments_become_overrides() {
        run_test_on_large_stack(|| {
            let mut rt = fresh();
            let mut endow = Endowments::new();
            endow.insert("X".to_string(), Value::Number(42.0));
            let idx = realm_alloc(&mut rt, true, endow, CapMode::Compat);
            match rt.realms[idx].globals_overrides.get("X") {
                Some(Value::Number(n)) => assert_eq!(*n, 42.0),
                other => panic!("expected Number(42.0) override, got {:?}", other),
            }
        });
    }

    #[test]
    fn r2_boundary_filter_node_compat_primitives_pass_through() {
        run_test_on_large_stack(|| {
            let mut rt = fresh();
            let v = Value::Number(42.0);
            let out = boundary_filter(&mut rt, v.clone(), BoundaryPolicy::NodeCompat).unwrap();
            match (v, out) {
                (Value::Number(a), Value::Number(b)) => assert_eq!(a, b),
                _ => panic!("expected Number passthrough"),
            }
        });
    }

    #[test]
    fn r2_boundary_filter_parameterized_primitives_pass_through() {
        run_test_on_large_stack(|| {
            let mut rt = fresh();
            let v = Value::String(std::rc::Rc::new(crate::value::JsString::from("hello")));
            let out = boundary_filter(&mut rt, v, BoundaryPolicy::Parameterized).unwrap();
            match out {
                Value::String(s) => assert_eq!(s.as_str(), "hello"),
                _ => panic!("expected String passthrough"),
            }
        });
    }

    #[test]
    fn r2_boundary_filter_identity_allows_same_owner_object() {
        run_test_on_large_stack(|| {
            let mut rt = fresh();
            let object = rt.alloc_object(crate::value::Object::new_ordinary());

            let out = boundary_filter(&mut rt, Value::Object(object), BoundaryPolicy::NodeCompat)
                .unwrap();

            assert_eq!(out, Value::Object(object));
        });
    }

    #[test]
    fn r2_boundary_filter_identity_refuses_foreign_owner_object() {
        run_test_on_large_stack(|| {
            let mut rt = fresh();
            let realm_idx = rt.allocate_realm();
            let prior = rt.current_realm;
            rt.current_realm = realm_idx;
            let object = rt.alloc_object(crate::value::Object::new_ordinary());
            rt.current_realm = prior;

            let err = boundary_filter(
                &mut rt,
                Value::Object(object),
                BoundaryPolicy::Parameterized,
            )
            .expect_err("foreign-owned object identity must fail closed");

            match err {
                RuntimeError::TypeError(message) => {
                    assert!(message.contains("foreign-owned Object cannot cross by identity"));
                }
                other => panic!("expected TypeError, got {:?}", other),
            }
        });
    }

    #[test]
    fn r4_boundary_filter_shadowrealm_primitives_pass_through() {
        run_test_on_large_stack(|| {
            let mut rt = fresh();

            for v in [
                Value::Undefined,
                Value::Null,
                Value::Boolean(true),
                Value::Number(42.0),
                Value::String(std::rc::Rc::new(crate::value::JsString::from("hi"))),
            ] {
                let result = boundary_filter(
                    &mut rt,
                    v.clone(),
                    BoundaryPolicy::PrimitiveOnlyWithWrappedCallable,
                );
                assert!(
                    result.is_ok(),
                    "expected primitive {:?} to pass through, got {:?}",
                    v,
                    result
                );
            }
        });
    }

    #[test]
    fn r4_wrapped_callable_rejects_non_callable() {
        run_test_on_large_stack(|| {
            let mut rt = fresh();

            let result = wrapped_callable(
                &mut rt,
                0,
                Value::Number(7.0),
                BoundaryPolicy::PrimitiveOnlyWithWrappedCallable,
            );
            match result {
                Err(RuntimeError::TypeError(msg)) => {
                    assert!(
                        msg.contains("not callable") || msg.contains("§27.5.3"),
                        "expected callable-check TypeError, got: {}",
                        msg
                    );
                }
                other => panic!("expected TypeError on non-callable, got {:?}", other),
            }
        });
    }

    #[test]
    fn r4_wrapped_callable_passes_through_non_shadowrealm_policy() {
        run_test_on_large_stack(|| {
            let mut rt = fresh();

            let v = Value::Number(42.0);
            let result = wrapped_callable(&mut rt, 0, v.clone(), BoundaryPolicy::NodeCompat);
            assert!(matches!(result, Ok(Value::Number(_))));
        });
    }

    #[test]
    fn r6_capability_mode_stored_on_realm() {
        run_test_on_large_stack(|| {
            let mut rt = fresh();
            let idx = realm_alloc(&mut rt, true, Endowments::new(), CapMode::Sealed);
            assert_eq!(realm_capability_mode(&rt, idx), CapMode::Sealed);
        });
    }

    #[test]
    fn r6_capability_mode_defaults_to_compat_when_unset() {
        run_test_on_large_stack(|| {
            let mut rt = fresh();
            let idx = realm_alloc(&mut rt, false, Endowments::new(), CapMode::Compat);
            assert_eq!(realm_capability_mode(&rt, idx), CapMode::Compat);
        });
    }

    #[test]
    fn r6_effective_mode_strictness_only_tightens_sealed_realm_under_compat_process() {
        run_test_on_large_stack(|| {

            let mut rt = fresh();
            let idx = realm_alloc(&mut rt, true, Endowments::new(), CapMode::Sealed);

            assert_eq!(
                effective_capability_mode_for_realm(&rt, idx),
                CapMode::Sealed
            );
        });
    }

    #[test]
    fn r6_effective_mode_strictness_only_tightens_compat_realm_under_sealed_process() {
        run_test_on_large_stack(|| {

            let mut rt = fresh();
            rt.set_cap_mode(CapMode::Sealed);
            let idx = realm_alloc(&mut rt, false, Endowments::new(), CapMode::Compat);

            assert_eq!(
                effective_capability_mode_for_realm(&rt, idx),
                CapMode::Sealed
            );
        });
    }

    #[test]
    fn r6_effective_mode_audit_strictness_ordering() {
        run_test_on_large_stack(|| {
            let mut rt = fresh();
            rt.set_cap_mode(CapMode::Audit);
            let idx = realm_alloc(&mut rt, false, Endowments::new(), CapMode::Compat);

            assert_eq!(
                effective_capability_mode_for_realm(&rt, idx),
                CapMode::Audit
            );
            let idx2 = realm_alloc(&mut rt, true, Endowments::new(), CapMode::SealedDeps);

            assert_eq!(
                effective_capability_mode_for_realm(&rt, idx2),
                CapMode::SealedDeps
            );
        });
    }
}
