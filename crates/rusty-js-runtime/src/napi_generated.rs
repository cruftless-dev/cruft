
#[no_mangle]
pub unsafe extern "C" fn napi_get_undefined(
    env: napi_env,
    result: *mut napi_value,
) -> napi_status {
    check_arg!(result);
    let env = env_mut!(env);
    *result = env.push_handle(Value::Undefined);
    napi_ok
}

#[no_mangle]
pub unsafe extern "C" fn napi_get_null(
    env: napi_env,
    result: *mut napi_value,
) -> napi_status {
    check_arg!(result);
    let env = env_mut!(env);
    *result = env.push_handle(Value::Null);
    napi_ok
}

#[no_mangle]
pub unsafe extern "C" fn napi_get_boolean(
    env: napi_env,
    value: bool,
    result: *mut napi_value,
) -> napi_status {
    check_arg!(result);
    let env = env_mut!(env);
    *result = env.push_handle(Value::Boolean(value));
    napi_ok
}

#[no_mangle]
pub unsafe extern "C" fn napi_get_global(
    env: napi_env,
    result: *mut napi_value,
) -> napi_status {
    check_arg!(result);
    let env = owner_env_mut!(env, "napi_get_global");
    let rt = &mut *env.rt;
    let global = match rt.global_object {
        Some(id) => Value::Object(id),
        None => rt.global_get("globalThis"),
    };
    *result = env.push_handle(global);
    napi_ok
}

#[no_mangle]
pub unsafe extern "C" fn napi_create_int32(
    env: napi_env,
    value: i32,
    result: *mut napi_value,
) -> napi_status {
    check_arg!(result);
    let env = env_mut!(env);
    *result = env.push_handle(Value::Number(value as f64));
    napi_ok
}

#[no_mangle]
pub unsafe extern "C" fn napi_create_uint32(
    env: napi_env,
    value: u32,
    result: *mut napi_value,
) -> napi_status {
    check_arg!(result);
    let env = env_mut!(env);
    *result = env.push_handle(Value::Number(value as f64));
    napi_ok
}

#[no_mangle]
pub unsafe extern "C" fn napi_create_int64(
    env: napi_env,
    value: i64,
    result: *mut napi_value,
) -> napi_status {
    check_arg!(result);
    let env = env_mut!(env);
    *result = env.push_handle(Value::Number(value as f64));
    napi_ok
}

#[no_mangle]
pub unsafe extern "C" fn napi_create_double(
    env: napi_env,
    value: f64,
    result: *mut napi_value,
) -> napi_status {
    check_arg!(result);
    let env = env_mut!(env);
    *result = env.push_handle(Value::Number(value));
    napi_ok
}

#[no_mangle]
pub unsafe extern "C" fn napi_create_string_utf8(
    env: napi_env,
    str: *const c_char,
    length: usize,
    result: *mut napi_value,
) -> napi_status {
    check_arg!(result);
    let env = env_mut!(env);
    let bytes = if length == usize::MAX {
        if str.is_null() {
            return napi_invalid_arg;
        }
        CStr::from_ptr(str).to_bytes()
    } else {
        if str.is_null() && length > 0 {
            return napi_invalid_arg;
        }
        std::slice::from_raw_parts(str as *const u8, length)
    };
    let s = String::from_utf8_lossy(bytes).into_owned();
    *result = env.push_handle(Value::String(Rc::new(crate::value::JsString::from(s))));
    napi_ok
}

#[no_mangle]
pub unsafe extern "C" fn napi_create_string_latin1(
    env: napi_env,
    str: *const c_char,
    length: usize,
    result: *mut napi_value,
) -> napi_status {
    check_arg!(result);
    let env = env_mut!(env);
    let bytes = if length == usize::MAX {
        if str.is_null() {
            return napi_invalid_arg;
        }
        CStr::from_ptr(str).to_bytes()
    } else {
        if str.is_null() && length > 0 {
            return napi_invalid_arg;
        }
        std::slice::from_raw_parts(str as *const u8, length)
    };
    let s = String::from_utf8_lossy(bytes).into_owned();
    *result = env.push_handle(Value::String(Rc::new(crate::value::JsString::from(s))));
    napi_ok
}

#[no_mangle]
pub unsafe extern "C" fn napi_create_object(
    env: napi_env,
    result: *mut napi_value,
) -> napi_status {
    check_arg!(result);
    let env = owner_env_mut!(env, "napi_create_object");
    let rt = &mut *env.rt;
    let id = rt.alloc_object(Object::new_ordinary());
    *result = env.push_handle(Value::Object(id));
    napi_ok
}

#[no_mangle]
pub unsafe extern "C" fn napi_create_array(
    env: napi_env,
    result: *mut napi_value,
) -> napi_status {
    check_arg!(result);
    let env = owner_env_mut!(env, "napi_create_array");
    let rt = &mut *env.rt;
    let id = rt.alloc_object(Object::new_array());
    *result = env.push_handle(Value::Object(id));
    napi_ok
}

#[no_mangle]
pub unsafe extern "C" fn napi_create_array_with_length(
    env: napi_env,
    length: usize,
    result: *mut napi_value,
) -> napi_status {
    check_arg!(result);
    let env = owner_env_mut!(env, "napi_create_array_with_length");
    let rt = &mut *env.rt;
    let id = rt.alloc_object(Object::new_array());
    rt.object_set(id, "length".into(), Value::Number(length as f64));
    *result = env.push_handle(Value::Object(id));
    napi_ok
}

#[no_mangle]
pub unsafe extern "C" fn napi_get_value_int32(
    env: napi_env,
    value: napi_value,
    result: *mut i32,
) -> napi_status {
    check_arg!(result);
    let env = env_mut!(env);
    let value = match env.get_handle(value) {
        Some(Value::Number(n)) => *n,
        _ => return napi_number_expected,
    };
    *result = value as i32;
    napi_ok
}

#[no_mangle]
pub unsafe extern "C" fn napi_get_value_uint32(
    env: napi_env,
    value: napi_value,
    result: *mut u32,
) -> napi_status {
    check_arg!(result);
    let env = env_mut!(env);
    let value = match env.get_handle(value) {
        Some(Value::Number(n)) => *n,
        _ => return napi_number_expected,
    };
    *result = value as u32;
    napi_ok
}

#[no_mangle]
pub unsafe extern "C" fn napi_get_value_int64(
    env: napi_env,
    value: napi_value,
    result: *mut i64,
) -> napi_status {
    check_arg!(result);
    let env = env_mut!(env);
    let value = match env.get_handle(value) {
        Some(Value::Number(n)) => *n,
        _ => return napi_number_expected,
    };
    *result = value as i64;
    napi_ok
}

#[no_mangle]
pub unsafe extern "C" fn napi_get_value_double(
    env: napi_env,
    value: napi_value,
    result: *mut f64,
) -> napi_status {
    check_arg!(result);
    let env = env_mut!(env);
    let value = match env.get_handle(value) {
        Some(Value::Number(n)) => *n,
        _ => return napi_number_expected,
    };
    *result = value;
    napi_ok
}

#[no_mangle]
pub unsafe extern "C" fn napi_get_value_bool(
    env: napi_env,
    value: napi_value,
    result: *mut bool,
) -> napi_status {
    check_arg!(result);
    let env = env_mut!(env);
    let value = match env.get_handle(value) {
        Some(Value::Boolean(b)) => *b,
        _ => return napi_boolean_expected,
    };
    *result = value;
    napi_ok
}

#[no_mangle]
pub unsafe extern "C" fn napi_typeof(
    env: napi_env,
    value: napi_value,
    result: *mut napi_valuetype,
) -> napi_status {
    check_arg!(result);
    let env = owner_env_mut!(env, "napi_typeof");
    let value = match env.get_handle(value) {
        Some(v) => v.clone(),
        None => return napi_invalid_arg,
    };
    let rt = &*env.rt;
    let t = match &value {
        Value::Undefined => napi_undefined,
        Value::Null => napi_null,
        Value::Boolean(_) => napi_boolean,
        Value::Number(_) => napi_number,
        Value::String(_) => napi_string,
        Value::Symbol(_) => napi_symbol,
        Value::BigInt(_) => napi_bigint,
        Value::Object(id) => match &rt.obj(*id).internal_kind {
            InternalKind::Function(_)
            | InternalKind::Closure(_)
            | InternalKind::BoundFunction(_) => napi_function,
            _ => napi_object_t,
        },
    };
    *result = t;
    napi_ok
}

#[no_mangle]
pub unsafe extern "C" fn napi_get_array_length(
    env: napi_env,
    value: napi_value,
    result: *mut u32,
) -> napi_status {
    check_arg!(result);
    let env = owner_env_mut!(env, "napi_get_array_length");
    let value = match env.get_handle(value) {
        Some(Value::Object(id)) => *id,
        _ => return napi_array_expected,
    };
    let rt = &mut *env.rt;
    let len = match rt.object_get(value, "length") {
        Value::Number(n) => n as u32,
        _ => 0,
    };
    *result = len;
    napi_ok
}

#[no_mangle]
pub unsafe extern "C" fn napi_get_named_property(
    env: napi_env,
    object: napi_value,
    utf8name: *const c_char,
    result: *mut napi_value,
) -> napi_status {
    check_arg!(result);
    check_arg!(utf8name);
    let env = owner_env_mut!(env, "napi_get_named_property");
    let object = match env.get_handle(object) {
        Some(Value::Object(id)) => *id,
        _ => return napi_object_expected,
    };
    let utf8name = CStr::from_ptr(utf8name).to_string_lossy().into_owned();
    let rt = &mut *env.rt;

    let v = rt
        .read_property(object, &utf8name)
        .unwrap_or(Value::Undefined);
    *result = env.push_handle(v);
    napi_ok
}

#[no_mangle]
pub unsafe extern "C" fn napi_get_property(
    env: napi_env,
    object: napi_value,
    key: napi_value,
    result: *mut napi_value,
) -> napi_status {
    check_arg!(result);
    let env = owner_env_mut!(env, "napi_get_property");
    let object = match env.get_handle(object) {
        Some(Value::Object(id)) => *id,
        _ => return napi_object_expected,
    };
    let key = match env.get_handle(key) {
        Some(v) => v.clone(),
        None => return napi_invalid_arg,
    };
    let rt = &mut *env.rt;
    let key_s = crate::abstract_ops::to_string(&key);

    let v = rt
        .read_property(object, key_s.as_str())
        .unwrap_or(Value::Undefined);
    *result = env.push_handle(v);
    napi_ok
}

#[no_mangle]
pub unsafe extern "C" fn napi_get_element(
    env: napi_env,
    object: napi_value,
    index: u32,
    result: *mut napi_value,
) -> napi_status {
    check_arg!(result);
    let env = owner_env_mut!(env, "napi_get_element");
    let object = match env.get_handle(object) {
        Some(Value::Object(id)) => *id,
        _ => return napi_object_expected,
    };
    let rt = &mut *env.rt;

    let v = rt
        .read_property(object, &index.to_string())
        .unwrap_or(Value::Undefined);
    *result = env.push_handle(v);
    napi_ok
}

#[no_mangle]
pub unsafe extern "C" fn napi_get_prototype(
    env: napi_env,
    object: napi_value,
    result: *mut napi_value,
) -> napi_status {
    check_arg!(result);
    let env = owner_env_mut!(env, "napi_get_prototype");
    let object = match env.get_handle(object) {
        Some(Value::Object(id)) => *id,
        _ => return napi_object_expected,
    };
    let rt = &*env.rt;
    let proto = match rt.obj(object).proto {
        Some(p) => Value::Object(p),
        None => Value::Null,
    };
    *result = env.push_handle(proto);
    napi_ok
}

#[no_mangle]
pub unsafe extern "C" fn napi_set_named_property(
    env: napi_env,
    object: napi_value,
    utf8name: *const c_char,
    value: napi_value,
) -> napi_status {
    check_arg!(utf8name);
    let env = owner_env_mut!(env, "napi_set_named_property");
    let object = match env.get_handle(object) {
        Some(Value::Object(id)) => *id,
        _ => return napi_object_expected,
    };
    let utf8name = CStr::from_ptr(utf8name).to_string_lossy().into_owned();
    let value = match env.get_handle(value) {
        Some(v) => v.clone(),
        None => return napi_invalid_arg,
    };
    let rt = &mut *env.rt;
    rt.object_set(object, utf8name, value);
    napi_ok
}

#[no_mangle]
pub unsafe extern "C" fn napi_set_property(
    env: napi_env,
    object: napi_value,
    key: napi_value,
    value: napi_value,
) -> napi_status {
    let env = owner_env_mut!(env, "napi_set_property");
    let object = match env.get_handle(object) {
        Some(Value::Object(id)) => *id,
        _ => return napi_object_expected,
    };
    let key = match env.get_handle(key) {
        Some(v) => v.clone(),
        None => return napi_invalid_arg,
    };
    let value = match env.get_handle(value) {
        Some(v) => v.clone(),
        None => return napi_invalid_arg,
    };
    let rt = &mut *env.rt;
    let key_s = crate::abstract_ops::to_string(&key);
    rt.object_set(object, key_s.as_str().to_string(), value);
    napi_ok
}

#[no_mangle]
pub unsafe extern "C" fn napi_set_element(
    env: napi_env,
    object: napi_value,
    index: u32,
    value: napi_value,
) -> napi_status {
    let env = owner_env_mut!(env, "napi_set_element");
    let object = match env.get_handle(object) {
        Some(Value::Object(id)) => *id,
        _ => return napi_object_expected,
    };
    let value = match env.get_handle(value) {
        Some(v) => v.clone(),
        None => return napi_invalid_arg,
    };
    let rt = &mut *env.rt;
    rt.object_set(object, index.to_string(), value);
    napi_ok
}

#[no_mangle]
pub unsafe extern "C" fn napi_has_named_property(
    env: napi_env,
    object: napi_value,
    utf8name: *const c_char,
    result: *mut bool,
) -> napi_status {
    check_arg!(result);
    check_arg!(utf8name);
    let env = owner_env_mut!(env, "napi_has_named_property");
    let object = match env.get_handle(object) {
        Some(Value::Object(id)) => *id,
        _ => return napi_object_expected,
    };
    let utf8name = CStr::from_ptr(utf8name).to_string_lossy().into_owned();
    let rt = &*env.rt;
    *result = rt.obj(object).has_own_str(&utf8name);
    napi_ok
}

#[no_mangle]
pub unsafe extern "C" fn napi_coerce_to_string(
    env: napi_env,
    value: napi_value,
    result: *mut napi_value,
) -> napi_status {
    check_arg!(result);
    let env = env_mut!(env);
    let v = env.get_handle(value).cloned().unwrap_or(Value::Undefined);
    let s = crate::abstract_ops::to_string(&v);
    *result = env.push_handle(Value::String(std::rc::Rc::new(crate::value::JsString::from(s))));
    napi_ok
}

#[no_mangle]
pub unsafe extern "C" fn napi_coerce_to_number(
    env: napi_env,
    value: napi_value,
    result: *mut napi_value,
) -> napi_status {
    check_arg!(result);
    let env = env_mut!(env);
    let v = env.get_handle(value).cloned().unwrap_or(Value::Undefined);
    let n = crate::abstract_ops::to_number(&v);
    *result = env.push_handle(Value::Number(n));
    napi_ok
}

#[no_mangle]
pub unsafe extern "C" fn napi_coerce_to_bool(
    env: napi_env,
    value: napi_value,
    result: *mut napi_value,
) -> napi_status {
    check_arg!(result);
    let env = env_mut!(env);
    let v = env.get_handle(value).cloned().unwrap_or(Value::Undefined);
    let b = crate::abstract_ops::to_boolean(&v);
    *result = env.push_handle(Value::Boolean(b));
    napi_ok
}

#[no_mangle]
pub unsafe extern "C" fn napi_throw(
    env: napi_env,
    error: napi_value,
) -> napi_status {
    let env = env_mut!(env);
    let v = env.get_handle(error).cloned().unwrap_or(Value::Undefined);
    env.pending_exception = Some(v);
    napi_ok
}

#[no_mangle]
pub unsafe extern "C" fn napi_throw_error(
    env: napi_env,
    _code: *const c_char,
    msg: *const c_char,
) -> napi_status {
    let env = env_mut!(env);
    let m = if msg.is_null() {
        "".into()
    } else {
        CStr::from_ptr(msg).to_string_lossy().into_owned()
    };
    env.pending_exception = Some(Value::String(Rc::new(crate::value::JsString::from(
        format!("Error: {}", m),
    ))));
    napi_ok
}

#[no_mangle]
pub unsafe extern "C" fn napi_throw_type_error(
    env: napi_env,
    _code: *const c_char,
    msg: *const c_char,
) -> napi_status {
    let env = env_mut!(env);
    let m = if msg.is_null() {
        "".into()
    } else {
        CStr::from_ptr(msg).to_string_lossy().into_owned()
    };
    env.pending_exception = Some(Value::String(Rc::new(crate::value::JsString::from(
        format!("TypeError: {}", m),
    ))));
    napi_ok
}

#[no_mangle]
pub unsafe extern "C" fn napi_throw_range_error(
    env: napi_env,
    _code: *const c_char,
    msg: *const c_char,
) -> napi_status {
    let env = env_mut!(env);
    let m = if msg.is_null() {
        "".into()
    } else {
        CStr::from_ptr(msg).to_string_lossy().into_owned()
    };
    env.pending_exception = Some(Value::String(Rc::new(crate::value::JsString::from(
        format!("RangeError: {}", m),
    ))));
    napi_ok
}

#[no_mangle]
pub unsafe extern "C" fn napi_throw_syntax_error(
    env: napi_env,
    _code: *const c_char,
    msg: *const c_char,
) -> napi_status {
    let env = env_mut!(env);
    let m = if msg.is_null() {
        "".into()
    } else {
        CStr::from_ptr(msg).to_string_lossy().into_owned()
    };
    env.pending_exception = Some(Value::String(Rc::new(crate::value::JsString::from(
        format!("SyntaxError: {}", m),
    ))));
    napi_ok
}

#[no_mangle]
pub unsafe extern "C" fn napi_is_exception_pending(
    env: napi_env,
    result: *mut bool,
) -> napi_status {
    check_arg!(result);
    let env = env_mut!(env);
    *result = env.pending_exception.is_some();
    napi_ok
}

#[no_mangle]
pub unsafe extern "C" fn napi_get_and_clear_last_exception(
    env: napi_env,
    result: *mut napi_value,
) -> napi_status {
    check_arg!(result);
    let env = env_mut!(env);
    let v = env.pending_exception.take().unwrap_or(Value::Undefined);
    *result = env.push_handle(v);
    napi_ok
}

#[no_mangle]
pub unsafe extern "C" fn napi_create_error(
    env: napi_env,
    code: napi_value,
    msg: napi_value,
    result: *mut napi_value,
) -> napi_status {
    check_arg!(result);
    *result = make_error_obj(env, msg, code, "Error");
    napi_ok
}

#[no_mangle]
pub unsafe extern "C" fn napi_create_type_error(
    env: napi_env,
    code: napi_value,
    msg: napi_value,
    result: *mut napi_value,
) -> napi_status {
    check_arg!(result);
    *result = make_error_obj(env, msg, code, "TypeError");
    napi_ok
}

#[no_mangle]
pub unsafe extern "C" fn napi_create_range_error(
    env: napi_env,
    code: napi_value,
    msg: napi_value,
    result: *mut napi_value,
) -> napi_status {
    check_arg!(result);
    *result = make_error_obj(env, msg, code, "RangeError");
    napi_ok
}

#[no_mangle]
pub unsafe extern "C" fn napi_create_syntax_error(
    env: napi_env,
    code: napi_value,
    msg: napi_value,
    result: *mut napi_value,
) -> napi_status {
    check_arg!(result);
    *result = make_error_obj(env, msg, code, "SyntaxError");
    napi_ok
}

#[no_mangle]
pub unsafe extern "C" fn napi_is_array(
    env: napi_env,
    value: napi_value,
    result: *mut bool,
) -> napi_status {
    check_arg!(result);
    let env = owner_env_mut!(env, "napi_is_array");
    let value = match env.get_handle(value) {
        Some(Value::Object(id)) => *id,
        _ => {
            *result = false;
            return napi_ok;
        }
    };
    let rt = &*env.rt;
    *result = matches!(rt.obj(value).internal_kind, InternalKind::Array);
    napi_ok
}

#[no_mangle]
pub unsafe extern "C" fn napi_is_error(
    env: napi_env,
    value: napi_value,
    result: *mut bool,
) -> napi_status {
    check_arg!(result);
    let env = owner_env_mut!(env, "napi_is_error");
    let value = match env.get_handle(value) {
        Some(Value::Object(id)) => *id,
        _ => {
            *result = false;
            return napi_ok;
        }
    };
    let rt = &*env.rt;
    *result = matches!(rt.obj(value).internal_kind, InternalKind::Error);
    napi_ok
}

#[no_mangle]
pub unsafe extern "C" fn napi_is_buffer(
    env: napi_env,
    value: napi_value,
    result: *mut bool,
) -> napi_status {
    check_arg!(result);
    let env = owner_env_mut!(env, "napi_is_buffer");
    let value = match env.get_handle(value) {
        Some(Value::Object(id)) => *id,
        _ => {
            *result = false;
            return napi_ok;
        }
    };
    let rt = &*env.rt;
    *result = rt.obj(value).is_buffer || matches!(rt.object_get(value, "__is_buffer__"), Value::Boolean(true));
    napi_ok
}

#[no_mangle]
pub unsafe extern "C" fn napi_strict_equals(
    env: napi_env,
    lhs: napi_value,
    rhs: napi_value,
    result: *mut bool,
) -> napi_status {
    check_arg!(result);
    let env = env_mut!(env);
    let l = env.get_handle(lhs).cloned();
    let r = env.get_handle(rhs).cloned();
    *result = match (l, r) {
        (Some(a), Some(b)) => crate::abstract_ops::is_strictly_equal(&a, &b),
        _ => false,
    };
    napi_ok
}

#[no_mangle]
pub unsafe extern "C" fn napi_instanceof(
    env: napi_env,
    object: napi_value,
    constructor: napi_value,
    result: *mut bool,
) -> napi_status {
    check_arg!(result);
    let env = owner_env_mut!(env, "napi_instanceof");
    let obj_id = match env.get_handle(object) {
        Some(Value::Object(id)) => *id,
        _ => {
            *result = false;
            return napi_ok;
        }
    };
    let ctor_id = match env.get_handle(constructor) {
        Some(Value::Object(id)) => *id,
        _ => {
            *result = false;
            return napi_ok;
        }
    };
    let rt = &*env.rt;
    let proto_target = match rt.object_get(ctor_id, "prototype") {
        Value::Object(id) => id,
        _ => {
            *result = false;
            return napi_ok;
        }
    };
    let mut cur = rt.obj(obj_id).proto;
    while let Some(p) = cur {
        if p == proto_target {
            *result = true;
            return napi_ok;
        }
        cur = rt.obj(p).proto;
    }
    *result = false;
    napi_ok
}

#[no_mangle]
pub unsafe extern "C" fn napi_is_promise(
    env: napi_env,
    value: napi_value,
    result: *mut bool,
) -> napi_status {
    check_arg!(result);
    let env = owner_env_mut!(env, "napi_is_promise");
    let value = match env.get_handle(value) {
        Some(Value::Object(id)) => *id,
        _ => {
            *result = false;
            return napi_ok;
        }
    };
    let rt = &*env.rt;
    *result = matches!(rt.obj(value).internal_kind, InternalKind::Promise(_));
    napi_ok
}

#[no_mangle]
pub unsafe extern "C" fn napi_has_property(
    env: napi_env,
    object: napi_value,
    key: napi_value,
    result: *mut bool,
) -> napi_status {
    check_arg!(result);
    let env = owner_env_mut!(env, "napi_has_property");
    let object = match env.get_handle(object) {
        Some(Value::Object(id)) => *id,
        _ => return napi_object_expected,
    };
    let key = match env.get_handle(key) {
        Some(v) => v.clone(),
        None => return napi_invalid_arg,
    };
    let rt = &*env.rt;
    let key_s = crate::abstract_ops::to_string(&key);
    *result = rt.has_property(object, key_s.as_str());
    napi_ok
}

#[no_mangle]
pub unsafe extern "C" fn napi_has_own_property(
    env: napi_env,
    object: napi_value,
    key: napi_value,
    result: *mut bool,
) -> napi_status {
    check_arg!(result);
    let env = owner_env_mut!(env, "napi_has_own_property");
    let object = match env.get_handle(object) {
        Some(Value::Object(id)) => *id,
        _ => return napi_object_expected,
    };
    let key = match env.get_handle(key) {
        Some(v) => v.clone(),
        None => return napi_invalid_arg,
    };
    let rt = &*env.rt;
    let key_s = crate::abstract_ops::to_string(&key);
    *result = rt.obj(object).has_own_str(key_s.as_str());
    napi_ok
}

#[no_mangle]
pub unsafe extern "C" fn napi_has_element(
    env: napi_env,
    object: napi_value,
    index: u32,
    result: *mut bool,
) -> napi_status {
    check_arg!(result);
    let env = owner_env_mut!(env, "napi_has_element");
    let object = match env.get_handle(object) {
        Some(Value::Object(id)) => *id,
        _ => return napi_object_expected,
    };
    let rt = &*env.rt;
    *result = rt.has_property(object, &index.to_string());
    napi_ok
}

#[no_mangle]
pub unsafe extern "C" fn napi_delete_property(
    env: napi_env,
    object: napi_value,
    key: napi_value,
    result: *mut bool,
) -> napi_status {
    check_arg!(result);
    let env = owner_env_mut!(env, "napi_delete_property");
    let object = match env.get_handle(object) {
        Some(Value::Object(id)) => *id,
        _ => return napi_object_expected,
    };
    let key = match env.get_handle(key) {
        Some(v) => v.clone(),
        None => return napi_invalid_arg,
    };
    let rt = &mut *env.rt;
    let key_s = crate::abstract_ops::to_string(&key);
    rt.obj_mut(object).remove_str(key_s.as_str());
    *result = true;
    napi_ok
}

#[no_mangle]
pub unsafe extern "C" fn napi_delete_element(
    env: napi_env,
    object: napi_value,
    index: u32,
    result: *mut bool,
) -> napi_status {
    check_arg!(result);
    let env = owner_env_mut!(env, "napi_delete_element");
    let object = match env.get_handle(object) {
        Some(Value::Object(id)) => *id,
        _ => return napi_object_expected,
    };
    let rt = &mut *env.rt;
    rt.obj_mut(object).remove_str(&index.to_string());
    *result = true;
    napi_ok
}

#[no_mangle]
pub unsafe extern "C" fn napi_get_value_string_utf8(
    env: napi_env,
    value: napi_value,
    buf: *mut c_char,
    bufsize: usize,
    result: *mut usize,
) -> napi_status {
    napi_get_value_string_utf8__impl(env, value, buf, bufsize, result)
}

#[no_mangle]
pub unsafe extern "C" fn napi_get_value_string_latin1(
    env: napi_env,
    value: napi_value,
    buf: *mut c_char,
    bufsize: usize,
    result: *mut usize,
) -> napi_status {
    napi_get_value_string_latin1__impl(env, value, buf, bufsize, result)
}

#[no_mangle]
pub unsafe extern "C" fn napi_get_value_string_utf16(
    env: napi_env,
    value: napi_value,
    buf: *mut u16,
    bufsize: usize,
    result: *mut usize,
) -> napi_status {
    napi_get_value_string_utf16__impl(env, value, buf, bufsize, result)
}

#[no_mangle]
pub unsafe extern "C" fn napi_create_symbol(
    env: napi_env,
    description: napi_value,
    result: *mut napi_value,
) -> napi_status {
    napi_create_symbol__impl(env, description, result)
}

#[no_mangle]
pub unsafe extern "C" fn napi_create_bigint_int64(
    env: napi_env,
    value: i64,
    result: *mut napi_value,
) -> napi_status {
    napi_create_bigint_int64__impl(env, value, result)
}

#[no_mangle]
pub unsafe extern "C" fn napi_create_bigint_uint64(
    env: napi_env,
    value: u64,
    result: *mut napi_value,
) -> napi_status {
    napi_create_bigint_uint64__impl(env, value, result)
}

#[no_mangle]
pub unsafe extern "C" fn napi_get_value_bigint_int64(
    env: napi_env,
    value: napi_value,
    result: *mut i64,
    lossless: *mut bool,
) -> napi_status {
    napi_get_value_bigint_int64__impl(env, value, result, lossless)
}

#[no_mangle]
pub unsafe extern "C" fn napi_object_freeze(
    env: napi_env,
    object: napi_value,
) -> napi_status {
    napi_object_freeze__impl(env, object)
}

#[no_mangle]
pub unsafe extern "C" fn napi_get_version(
    env: napi_env,
    result: *mut u32,
) -> napi_status {
    napi_get_version__impl(env, result)
}

#[no_mangle]
pub unsafe extern "C" fn napi_get_node_version(
    env: napi_env,
    result: *mut *const c_void,
) -> napi_status {
    napi_get_node_version__impl(env, result)
}

#[no_mangle]
pub unsafe extern "C" fn napi_create_bigint_words(
    env: napi_env,
    sign_bit: i32,
    word_count: usize,
    words: *const u64,
    result: *mut napi_value,
) -> napi_status {
    napi_create_bigint_words__impl(env, sign_bit, word_count, words, result)
}

#[no_mangle]
pub unsafe extern "C" fn napi_get_value_bigint_uint64(
    env: napi_env,
    value: napi_value,
    result: *mut u64,
    lossless: *mut bool,
) -> napi_status {
    napi_get_value_bigint_uint64__impl(env, value, result, lossless)
}

#[no_mangle]
pub unsafe extern "C" fn napi_get_value_bigint_words(
    env: napi_env,
    value: napi_value,
    sign_bit: *mut i32,
    word_count: *mut usize,
    words: *mut u64,
) -> napi_status {
    napi_get_value_bigint_words__impl(env, value, sign_bit, word_count, words)
}

#[no_mangle]
pub unsafe extern "C" fn napi_create_external(
    env: napi_env,
    data: *mut c_void,
    finalize_cb: *mut c_void,
    finalize_hint: *mut c_void,
    result: *mut napi_value,
) -> napi_status {
    napi_create_external__impl(env, data, finalize_cb, finalize_hint, result)
}

#[no_mangle]
pub unsafe extern "C" fn napi_get_value_external(
    env: napi_env,
    value: napi_value,
    result: *mut *mut c_void,
) -> napi_status {
    napi_get_value_external__impl(env, value, result)
}

#[no_mangle]
pub unsafe extern "C" fn napi_wrap(
    env: napi_env,
    object: napi_value,
    native: *mut c_void,
    finalize_cb: *mut c_void,
    finalize_hint: *mut c_void,
    result: *mut napi_ref,
) -> napi_status {
    napi_wrap__impl(env, object, native, finalize_cb, finalize_hint, result)
}

#[no_mangle]
pub unsafe extern "C" fn napi_unwrap(
    env: napi_env,
    object: napi_value,
    result: *mut *mut c_void,
) -> napi_status {
    napi_unwrap__impl(env, object, result)
}

#[no_mangle]
pub unsafe extern "C" fn napi_remove_wrap(
    env: napi_env,
    object: napi_value,
    result: *mut *mut c_void,
) -> napi_status {
    napi_remove_wrap__impl(env, object, result)
}

#[no_mangle]
pub unsafe extern "C" fn napi_add_finalizer(
    env: napi_env,
    object: napi_value,
    native_object: *mut c_void,
    finalize_cb: *mut c_void,
    finalize_hint: *mut c_void,
    result: *mut napi_ref,
) -> napi_status {
    napi_add_finalizer__impl(env, object, native_object, finalize_cb, finalize_hint, result)
}

#[no_mangle]
pub unsafe extern "C" fn napi_get_property_names(
    env: napi_env,
    object: napi_value,
    result: *mut napi_value,
) -> napi_status {
    napi_get_property_names__impl(env, object, result)
}

#[no_mangle]
pub unsafe extern "C" fn napi_define_properties(
    env: napi_env,
    object: napi_value,
    property_count: usize,
    properties: *const napi_property_descriptor,
) -> napi_status {
    napi_define_properties__impl(env, object, property_count, properties)
}

#[no_mangle]
pub unsafe extern "C" fn napi_coerce_to_object(
    env: napi_env,
    value: napi_value,
    result: *mut napi_value,
) -> napi_status {
    napi_coerce_to_object__impl(env, value, result)
}

#[no_mangle]
pub unsafe extern "C" fn napi_object_seal(
    env: napi_env,
    object: napi_value,
) -> napi_status {
    napi_object_seal__impl(env, object)
}

#[no_mangle]
pub unsafe extern "C" fn napi_get_last_error_info(
    env: napi_env,
    result: *mut *const napi_extended_error_info,
) -> napi_status {
    napi_get_last_error_info__impl(env, result)
}

#[no_mangle]
pub unsafe extern "C" fn napi_get_arraybuffer_info(
    env: napi_env,
    value: napi_value,
    data: *mut *mut c_void,
    byte_length: *mut usize,
) -> napi_status {
    napi_get_arraybuffer_info__impl(env, value, data, byte_length)
}

#[no_mangle]
pub unsafe extern "C" fn napi_get_buffer_info(
    env: napi_env,
    value: napi_value,
    data: *mut *mut c_void,
    length: *mut usize,
) -> napi_status {
    napi_get_buffer_info__impl(env, value, data, length)
}

#[no_mangle]
pub unsafe extern "C" fn napi_adjust_external_memory(
    env: napi_env,
    change_in_bytes: i64,
    result: *mut i64,
) -> napi_status {
    napi_adjust_external_memory__impl(env, change_in_bytes, result)
}

#[no_mangle]
pub unsafe extern "C" fn napi_fatal_exception(
    env: napi_env,
    err: napi_value,
) -> napi_status {
    napi_fatal_exception__impl(env, err)
}

#[no_mangle]
pub unsafe extern "C" fn napi_add_env_cleanup_hook(
    env: napi_env,
    fun: *mut c_void,
    arg: *mut c_void,
) -> napi_status {
    napi_add_env_cleanup_hook__impl(env, fun, arg)
}

#[no_mangle]
pub unsafe extern "C" fn napi_remove_env_cleanup_hook(
    env: napi_env,
    fun: *mut c_void,
    arg: *mut c_void,
) -> napi_status {
    napi_remove_env_cleanup_hook__impl(env, fun, arg)
}

#[no_mangle]
pub unsafe extern "C" fn napi_create_reference(
    env: napi_env,
    value: napi_value,
    initial_refcount: u32,
    result: *mut napi_ref,
) -> napi_status {
    napi_create_reference__impl(env, value, initial_refcount, result)
}

#[no_mangle]
pub unsafe extern "C" fn napi_delete_reference(
    env: napi_env,
    r: napi_ref,
) -> napi_status {
    napi_delete_reference__impl(env, r)
}

#[no_mangle]
pub unsafe extern "C" fn napi_reference_ref(
    env: napi_env,
    r: napi_ref,
    result: *mut u32,
) -> napi_status {
    napi_reference_ref__impl(env, r, result)
}

#[no_mangle]
pub unsafe extern "C" fn napi_reference_unref(
    env: napi_env,
    r: napi_ref,
    result: *mut u32,
) -> napi_status {
    napi_reference_unref__impl(env, r, result)
}

#[no_mangle]
pub unsafe extern "C" fn napi_get_reference_value(
    env: napi_env,
    r: napi_ref,
    result: *mut napi_value,
) -> napi_status {
    napi_get_reference_value__impl(env, r, result)
}

#[no_mangle]
pub unsafe extern "C" fn napi_open_handle_scope(
    env: napi_env,
    result: *mut napi_handle_scope,
) -> napi_status {
    napi_open_handle_scope__impl(env, result)
}

#[no_mangle]
pub unsafe extern "C" fn napi_close_handle_scope(
    env: napi_env,
    scope: napi_handle_scope,
) -> napi_status {
    napi_close_handle_scope__impl(env, scope)
}

#[no_mangle]
pub unsafe extern "C" fn napi_open_escapable_handle_scope(
    env: napi_env,
    result: *mut napi_escapable_handle_scope,
) -> napi_status {
    napi_open_escapable_handle_scope__impl(env, result)
}

#[no_mangle]
pub unsafe extern "C" fn napi_close_escapable_handle_scope(
    env: napi_env,
    scope: napi_escapable_handle_scope,
) -> napi_status {
    napi_close_escapable_handle_scope__impl(env, scope)
}

#[no_mangle]
pub unsafe extern "C" fn napi_escape_handle(
    env: napi_env,
    scope: napi_escapable_handle_scope,
    escapee: napi_value,
    result: *mut napi_value,
) -> napi_status {
    napi_escape_handle__impl(env, scope, escapee, result)
}

#[no_mangle]
pub unsafe extern "C" fn napi_create_function(
    env: napi_env,
    utf8name: *const c_char,
    length: usize,
    cb: Option<napi_callback>,
    data: *mut c_void,
    result: *mut napi_value,
) -> napi_status {
    napi_create_function__impl(env, utf8name, length, cb, data, result)
}

#[no_mangle]
pub unsafe extern "C" fn napi_get_cb_info(
    env: napi_env,
    cbinfo: napi_callback_info,
    argc: *mut usize,
    argv: *mut napi_value,
    this_arg: *mut napi_value,
    data: *mut *mut c_void,
) -> napi_status {
    napi_get_cb_info__impl(env, cbinfo, argc, argv, this_arg, data)
}

#[no_mangle]
pub unsafe extern "C" fn napi_call_function(
    env: napi_env,
    recv: napi_value,
    func: napi_value,
    argc: usize,
    argv: *const napi_value,
    result: *mut napi_value,
) -> napi_status {
    napi_call_function__impl(env, recv, func, argc, argv, result)
}

#[no_mangle]
pub unsafe extern "C" fn napi_new_instance(
    env: napi_env,
    constructor: napi_value,
    argc: usize,
    argv: *const napi_value,
    result: *mut napi_value,
) -> napi_status {
    napi_new_instance__impl(env, constructor, argc, argv, result)
}

#[no_mangle]
pub unsafe extern "C" fn napi_get_new_target(
    env: napi_env,
    cbinfo: napi_callback_info,
    result: *mut napi_value,
) -> napi_status {
    napi_get_new_target__impl(env, cbinfo, result)
}

#[no_mangle]
pub unsafe extern "C" fn napi_create_promise(
    env: napi_env,
    deferred: *mut napi_deferred,
    promise: *mut napi_value,
) -> napi_status {
    napi_create_promise__impl(env, deferred, promise)
}

#[no_mangle]
pub unsafe extern "C" fn napi_resolve_deferred(
    env: napi_env,
    deferred: napi_deferred,
    resolution: napi_value,
) -> napi_status {
    napi_resolve_deferred__impl(env, deferred, resolution)
}

#[no_mangle]
pub unsafe extern "C" fn napi_reject_deferred(
    env: napi_env,
    deferred: napi_deferred,
    reason: napi_value,
) -> napi_status {
    napi_reject_deferred__impl(env, deferred, reason)
}

#[no_mangle]
pub unsafe extern "C" fn napi_create_arraybuffer(
    env: napi_env,
    byte_length: usize,
    data: *mut *mut c_void,
    result: *mut napi_value,
) -> napi_status {
    napi_create_arraybuffer__impl(env, byte_length, data, result)
}

#[no_mangle]
pub unsafe extern "C" fn napi_detach_arraybuffer(
    env: napi_env,
    arraybuffer: napi_value,
) -> napi_status {
    napi_detach_arraybuffer__impl(env, arraybuffer)
}

#[no_mangle]
pub unsafe extern "C" fn napi_create_buffer(
    env: napi_env,
    length: usize,
    data: *mut *mut c_void,
    result: *mut napi_value,
) -> napi_status {
    napi_create_buffer__impl(env, length, data, result)
}

#[no_mangle]
pub unsafe extern "C" fn napi_define_class(
    env: napi_env,
    utf8name: *const c_char,
    length: usize,
    ctor: Option<napi_callback>,
    data: *mut c_void,
    property_count: usize,
    properties: *const napi_property_descriptor,
    result: *mut napi_value,
) -> napi_status {
    napi_define_class__impl(env, utf8name, length, ctor, data, property_count, properties, result)
}

#[no_mangle]
pub unsafe extern "C" fn napi_create_async_work(
    env: napi_env,
    async_resource: napi_value,
    async_resource_name: napi_value,
    execute: Option<unsafe extern "C" fn(env: napi_env, data: *mut c_void)>,
    complete: Option<unsafe extern "C" fn(env: napi_env, status: napi_status, data: *mut c_void)>,
    data: *mut c_void,
    result: *mut *mut NapiAsyncWork,
) -> napi_status {
    napi_create_async_work__impl(env, async_resource, async_resource_name, execute, complete, data, result)
}

#[no_mangle]
pub unsafe extern "C" fn napi_queue_async_work(
    env: napi_env,
    work: *mut NapiAsyncWork,
) -> napi_status {
    napi_queue_async_work__impl(env, work)
}

#[no_mangle]
pub unsafe extern "C" fn napi_cancel_async_work(
    env: napi_env,
    work: *mut NapiAsyncWork,
) -> napi_status {
    napi_cancel_async_work__impl(env, work)
}

#[no_mangle]
pub unsafe extern "C" fn napi_delete_async_work(
    env: napi_env,
    work: *mut NapiAsyncWork,
) -> napi_status {
    napi_delete_async_work__impl(env, work)
}

#[no_mangle]
pub unsafe extern "C" fn napi_create_threadsafe_function(
    env: napi_env,
    func: napi_value,
    async_resource: napi_value,
    async_resource_name: napi_value,
    max_queue_size: usize,
    initial_thread_count: usize,
    thread_finalize_data: *mut c_void,
    thread_finalize_cb: *mut c_void,
    context: *mut c_void,
    call_js_cb: Option<napi_threadsafe_function_call_js>,
    result: *mut napi_threadsafe_function,
) -> napi_status {
    napi_create_threadsafe_function__impl(env, func, async_resource, async_resource_name, max_queue_size, initial_thread_count, thread_finalize_data, thread_finalize_cb, context, call_js_cb, result)
}

#[no_mangle]
pub unsafe extern "C" fn napi_acquire_threadsafe_function(
    tsfn: napi_threadsafe_function,
) -> napi_status {
    napi_acquire_threadsafe_function__impl(tsfn)
}

#[no_mangle]
pub unsafe extern "C" fn napi_release_threadsafe_function(
    tsfn: napi_threadsafe_function,
    mode: napi_threadsafe_function_release_mode,
) -> napi_status {
    napi_release_threadsafe_function__impl(tsfn, mode)
}

#[no_mangle]
pub unsafe extern "C" fn napi_call_threadsafe_function(
    tsfn: napi_threadsafe_function,
    data: *mut c_void,
    mode: napi_threadsafe_function_call_mode,
) -> napi_status {
    napi_call_threadsafe_function__impl(tsfn, data, mode)
}

#[no_mangle]
pub unsafe extern "C" fn napi_ref_threadsafe_function(
    env: napi_env,
    tsfn: napi_threadsafe_function,
) -> napi_status {
    napi_ref_threadsafe_function__impl(env, tsfn)
}

#[no_mangle]
pub unsafe extern "C" fn napi_unref_threadsafe_function(
    env: napi_env,
    tsfn: napi_threadsafe_function,
) -> napi_status {
    napi_unref_threadsafe_function__impl(env, tsfn)
}

#[no_mangle]
pub unsafe extern "C" fn napi_get_threadsafe_function_context(
    tsfn: napi_threadsafe_function,
    result: *mut *mut c_void,
) -> napi_status {
    napi_get_threadsafe_function_context__impl(tsfn, result)
}

#[no_mangle]
pub unsafe extern "C" fn napi_is_arraybuffer(
    env: napi_env,
    value: napi_value,
    result: *mut bool,
) -> napi_status {
    napi_is_arraybuffer__impl(env, value, result)
}

#[no_mangle]
pub unsafe extern "C" fn napi_is_typedarray(
    env: napi_env,
    value: napi_value,
    result: *mut bool,
) -> napi_status {
    napi_is_typedarray__impl(env, value, result)
}

#[no_mangle]
pub unsafe extern "C" fn napi_is_dataview(
    env: napi_env,
    value: napi_value,
    result: *mut bool,
) -> napi_status {
    napi_is_dataview__impl(env, value, result)
}

#[no_mangle]
pub unsafe extern "C" fn napi_is_date(
    env: napi_env,
    value: napi_value,
    result: *mut bool,
) -> napi_status {
    napi_is_date__impl(env, value, result)
}

#[no_mangle]
pub unsafe extern "C" fn napi_create_date(
    env: napi_env,
    time: f64,
    result: *mut napi_value,
) -> napi_status {
    napi_create_date__impl(env, time, result)
}

#[no_mangle]
pub unsafe extern "C" fn napi_get_date_value(
    env: napi_env,
    value: napi_value,
    result: *mut f64,
) -> napi_status {
    napi_get_date_value__impl(env, value, result)
}

#[no_mangle]
pub unsafe extern "C" fn napi_create_string_utf16(
    env: napi_env,
    str: *const u16,
    length: usize,
    result: *mut napi_value,
) -> napi_status {
    napi_create_string_utf16__impl(env, str, length, result)
}

#[no_mangle]
pub unsafe extern "C" fn napi_type_tag_object(
    env: napi_env,
    value: napi_value,
    type_tag: *const napi_type_tag,
) -> napi_status {
    napi_type_tag_object__impl(env, value, type_tag)
}

#[no_mangle]
pub unsafe extern "C" fn napi_check_object_type_tag(
    env: napi_env,
    value: napi_value,
    type_tag: *const napi_type_tag,
    result: *mut bool,
) -> napi_status {
    napi_check_object_type_tag__impl(env, value, type_tag, result)
}

#[no_mangle]
pub unsafe extern "C" fn napi_create_dataview(
    env: napi_env,
    length: usize,
    arraybuffer: napi_value,
    byte_offset: usize,
    result: *mut napi_value,
) -> napi_status {
    napi_create_dataview__impl(env, length, arraybuffer, byte_offset, result)
}

#[no_mangle]
pub unsafe extern "C" fn napi_run_script(
    env: napi_env,
    script: napi_value,
    result: *mut napi_value,
) -> napi_status {
    napi_run_script__impl(env, script, result)
}

#[no_mangle]
pub unsafe extern "C" fn napi_create_buffer_copy(
    env: napi_env,
    length: usize,
    data: *const c_void,
    result_data: *mut *mut c_void,
    result: *mut napi_value,
) -> napi_status {
    napi_create_buffer_copy__impl(env, length, data, result_data, result)
}
