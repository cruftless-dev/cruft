
use crate::register::{native_function, new_object, register_method, set_constant};
use rusty_js_runtime::value::{InternalKind, JsString, Object, ObjectRef, PropertyKey};
use rusty_js_runtime::{Runtime, RuntimeError, Value};
use std::borrow::Cow;
use std::rc::Rc;

fn sval(s: &str) -> Value {
    Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
        s.to_string(),
    )))
}

fn percent_decode(s: &str, plus_as_space: bool) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if plus_as_space && bytes[i] == b'+' {
            out.push(b' ');
            i += 1;
            continue;
        }
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let h1 = (bytes[i + 1] as char).to_digit(16);
            let h2 = (bytes[i + 2] as char).to_digit(16);
            if let (Some(h1), Some(h2)) = (h1, h2) {
                out.push(((h1 << 4) | h2) as u8);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn percent_decode_code_units_to_bytes(units: &[u16], plus_as_space: bool) -> Vec<u8> {
    let mut out = Vec::with_capacity(units.len());
    let mut i = 0;
    while i < units.len() {
        let b = (units[i] & 0xff) as u8;
        if plus_as_space && b == b'+' {
            out.push(b' ');
            i += 1;
            continue;
        }
        if b == b'%' && i + 2 < units.len() {
            let h1 = char::from((units[i + 1] & 0xff) as u8).to_digit(16);
            let h2 = char::from((units[i + 2] & 0xff) as u8).to_digit(16);
            if let (Some(h1), Some(h2)) = (h1, h2) {
                out.push(((h1 << 4) | h2) as u8);
                i += 3;
                continue;
            }
        }
        out.push(b);
        i += 1;
    }
    out
}

fn hex(n: u8) -> char {
    match n {
        0..=9 => (b'0' + n) as char,
        _ => (b'A' + (n - 10)) as char,
    }
}

fn percent_encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.as_bytes() {
        let keep = matches!(
            *b,
            b'A'..=b'Z'
                | b'a'..=b'z'
                | b'0'..=b'9'
                | b'-'
                | b'_'
                | b'.'
                | b'~'
                | b'!'
                | b'*'
                | b'\''
                | b'('
                | b')'
        );
        if keep {
            out.push(*b as char);
        } else {
            out.push('%');
            out.push(hex((*b >> 4) & 0x0f));
            out.push(hex(*b & 0x0f));
        }
    }
    out
}

fn querystring_uri_error(rt: &mut Runtime) -> RuntimeError {
    match rusty_js_runtime::intrinsics::make_error_instance(rt, "URIError", "URI malformed") {
        Some(id) => {
            rt.object_set(
                id,
                "code".into(),
                Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                    "ERR_INVALID_URI",
                ))),
            );
            RuntimeError::Thrown(Value::Object(id))
        }
        None => RuntimeError::TypeError("URI malformed".into()),
    }
}

fn percent_encode_code_units(units: &[u16]) -> Option<String> {
    let mut out = String::new();
    let mut i = 0usize;
    while i < units.len() {
        let u = units[i];
        let cp = if (0xD800..=0xDBFF).contains(&u) {
            match units.get(i + 1).copied() {
                Some(lo) if (0xDC00..=0xDFFF).contains(&lo) => {
                    i += 1;
                    0x1_0000 + (((u as u32 - 0xD800) << 10) | (lo as u32 - 0xDC00))
                }
                _ => return None,
            }
        } else if (0xDC00..=0xDFFF).contains(&u) {
            return None;
        } else {
            u as u32
        };
        i += 1;
        let ch = char::from_u32(cp)?;
        let mut buf = [0u8; 4];
        for b in ch.encode_utf8(&mut buf).bytes() {
            let keep = matches!(
                b,
                b'A'..=b'Z'
                    | b'a'..=b'z'
                    | b'0'..=b'9'
                    | b'-'
                    | b'_'
                    | b'.'
                    | b'~'
                    | b'!'
                    | b'*'
                    | b'\''
                    | b'('
                    | b')'
            );
            if keep {
                out.push(b as char);
            } else {
                out.push('%');
                out.push(hex((b >> 4) & 0x0f));
                out.push(hex(b & 0x0f));
            }
        }
    }
    Some(out)
}

fn querystring_escape_identity(s: &JsString) -> bool {
    s.code_units().iter().all(|unit| {
        let Ok(byte) = u8::try_from(*unit) else {
            return false;
        };
        matches!(
            byte,
            b'A'..=b'Z'
                | b'a'..=b'z'
                | b'0'..=b'9'
                | b'-'
                | b'_'
                | b'.'
                | b'~'
                | b'!'
                | b'*'
                | b'\''
                | b'('
                | b')'
        )
    })
}

fn percent_encode_js_string(
    rt: &mut Runtime,
    s: &rusty_js_runtime::value::JsString,
) -> Result<String, RuntimeError> {
    percent_encode_code_units(s.code_units().as_ref()).ok_or_else(|| querystring_uri_error(rt))
}

fn percent_encode_key(rt: &mut Runtime, s: &str) -> Result<String, RuntimeError> {
    percent_encode_code_units(&s.encode_utf16().collect::<Vec<_>>())
        .ok_or_else(|| querystring_uri_error(rt))
}

fn value_to_qs_string(v: &Value) -> String {
    match v {
        Value::Undefined | Value::Null => String::new(),
        Value::String(s) => s.as_str().to_string(),
        Value::Boolean(b) => b.to_string(),
        Value::Number(n) => {
            if !n.is_finite() {
                String::new()
            } else if n.abs() >= 1e21 {
                rusty_js_runtime::abstract_ops::to_string(v)
                    .as_str()
                    .to_string()
            } else if n.fract() == 0.0 {
                format!("{}", *n as i64)
            } else {
                n.to_string()
            }
        }
        Value::BigInt(_) => rusty_js_runtime::abstract_ops::to_string(v)
            .as_str()
            .to_string(),
        Value::Object(_) | Value::Symbol(_) => String::new(),
        other => rusty_js_runtime::abstract_ops::to_string(other)
            .as_str()
            .to_string(),
    }
}

fn percent_encode_qs_value(rt: &mut Runtime, v: &Value) -> Result<String, RuntimeError> {
    match v {
        Value::String(s) => percent_encode_js_string(rt, s),
        _ => Ok(percent_encode(&value_to_qs_string(v))),
    }
}

fn parse_decoder_option(
    rt: &mut Runtime,
    opts: Option<&Value>,
) -> Result<Option<Value>, RuntimeError> {
    let Some(Value::Object(id)) = opts else {
        return Ok(None);
    };
    let first = rt.spec_get(&Value::Object(*id), "decodeURIComponent")?;
    if !rt.is_callable(&first) {
        return Ok(None);
    }
    let second = rt.spec_get(&Value::Object(*id), "decodeURIComponent")?;
    Ok(rt.is_callable(&second).then_some(second))
}

fn default_parse_decoder(rt: &Runtime, qs: ObjectRef) -> Result<Value, RuntimeError> {
    let decoder = rt.object_get(qs, "unescape");
    if rt.is_callable(&decoder) {
        Ok(decoder)
    } else {
        Err(RuntimeError::TypeError(
            "QueryString.unescape is not a function".into(),
        ))
    }
}

#[derive(Clone)]
enum ParseDecoder {
    Custom(Value),
    Default(Value),
}

impl ParseDecoder {
    fn value(&self) -> Value {
        match self {
            ParseDecoder::Custom(v) | ParseDecoder::Default(v) => v.clone(),
        }
    }

    fn falls_back_on_error(&self) -> bool {
        matches!(self, ParseDecoder::Custom(_))
    }
}

fn stringify_encoder_option(
    rt: &mut Runtime,
    opts: Option<&Value>,
) -> Result<Option<Value>, RuntimeError> {
    let Some(Value::Object(id)) = opts else {
        return Ok(None);
    };
    let first = rt.spec_get(&Value::Object(*id), "encodeURIComponent")?;
    if !rt.is_callable(&first) {
        return Ok(None);
    }
    Ok(Some(
        rt.spec_get(&Value::Object(*id), "encodeURIComponent")?,
    ))
}

fn call_codec_string(rt: &mut Runtime, f: Value, input: &str) -> Result<String, RuntimeError> {
    let result = rt.call_function(f, Value::Undefined, vec![sval(input)])?;
    rt.coerce_to_string(&result)
}

fn call_codec_value(rt: &mut Runtime, f: Value, input: &str) -> Result<Value, RuntimeError> {
    rt.call_function(f, Value::Undefined, vec![sval(input)])
}

fn decode_query_component_value(
    rt: &mut Runtime,
    raw: &str,
    plus_as_space: bool,
    decoder: Option<ParseDecoder>,
) -> Result<Value, RuntimeError> {
    if let Some(f) = decoder {
        let input = if plus_as_space && raw.as_bytes().contains(&b'+') {
            raw.replace('+', "%20")
        } else {
            raw.to_string()
        };
        match call_codec_value(rt, f.value(), &input) {
            Ok(decoded) => return Ok(decoded),
            Err(err) if f.falls_back_on_error() => {}
            Err(err) => return Err(err),
        }
    }
    Ok(sval(&percent_decode(raw, plus_as_space)))
}

fn decode_query_component_key(
    rt: &mut Runtime,
    raw: &str,
    plus_as_space: bool,
    decoder: Option<ParseDecoder>,
) -> Result<Option<PropertyKey>, RuntimeError> {
    let decoded = decode_query_component_value(rt, raw, plus_as_space, decoder)?;
    project_decoded_query_key(rt, &decoded)
}

fn parse_uses_original_default_decoder(
    decoder: &ParseDecoder,
    original_unescape: ObjectRef,
) -> bool {
    matches!(decoder, ParseDecoder::Default(Value::Object(id)) if *id == original_unescape)
}

fn parse_insert_default_decoded(
    rt: &mut Runtime,
    out: ObjectRef,
    raw_key: &str,
    raw_value: &str,
) -> Result<(), RuntimeError> {
    let key = percent_decode(raw_key, true);
    let val = sval(&percent_decode(raw_value, true));
    parse_insert(rt, out, PropertyKey::String(key), val)
}

fn decode_default_parse_component(raw: &str) -> Cow<'_, str> {
    if raw.as_bytes().iter().any(|b| matches!(*b, b'%' | b'+')) {
        Cow::Owned(percent_decode(raw, true))
    } else {
        Cow::Borrowed(raw)
    }
}

fn querystring_default_parse_unique_fast(rt: &mut Runtime, input: &str) -> Option<Value> {
    let mut pairs = Vec::new();
    for raw in input.split('&').take(1000) {
        if raw.is_empty() {
            continue;
        }
        let (key, value) = raw.split_once('=').unwrap_or((raw, ""));
        let key = decode_default_parse_component(key).into_owned();
        if pairs.iter().any(|(seen, _)| *seen == key) {
            return None;
        }
        let value = decode_default_parse_component(value);
        pairs.push((key, value));
    }

    let mut object = Object::new_dictionary_with_property_capacity(pairs.len());
    for (key, value) in pairs {
        object.set_own(key, sval(value.as_ref()));
    }
    Some(Value::Object(
        rt.alloc_object_with_explicit_null_proto(object),
    ))
}

fn project_decoded_query_key(
    rt: &mut Runtime,
    decoded: &Value,
) -> Result<Option<PropertyKey>, RuntimeError> {
    if matches!(decoded, Value::Symbol(_)) {
        return Ok(None);
    }
    if let Value::Object(id) = decoded {
        if matches!(rt.obj(*id).internal_kind, InternalKind::Array) {
            return Ok(Some(PropertyKey::String(
                querystring_array_to_string(rt, *id)?.unwrap_or_default(),
            )));
        }
        return Ok(Some(rt.to_property_key(decoded)?));
    }
    Ok(Some(PropertyKey::String(
        rusty_js_runtime::abstract_ops::to_string(decoded)
            .as_str()
            .to_string(),
    )))
}

fn encode_query_key(
    rt: &mut Runtime,
    key: &str,
    encoder: Option<Value>,
) -> Result<String, RuntimeError> {
    match encoder {
        Some(f) => call_codec_string(rt, f, key),
        None => percent_encode_key(rt, key),
    }
}

fn encode_query_value(
    rt: &mut Runtime,
    value: &Value,
    encoder: Option<Value>,
) -> Result<String, RuntimeError> {
    match encoder {
        Some(f) => call_codec_string(rt, f, &value_to_qs_string(value)),
        None => percent_encode_qs_value(rt, value),
    }
}

fn object_value_via_get(rt: &mut Runtime, id: ObjectRef, key: &str) -> Result<Value, RuntimeError> {
    rt.spec_get(&Value::Object(id), key)
}

fn own_enumerable_string_keys(
    rt: &mut Runtime,
    id: ObjectRef,
) -> Result<Vec<String>, RuntimeError> {
    let keys_v = rt.enumerable_own_keys(&Value::Object(id))?;
    let mut out = Vec::new();
    if let Value::Object(keys) = keys_v {
        let len = rt.array_length(keys);
        for i in 0..len {
            if let Value::String(s) = rt.object_get(keys, &i.to_string()) {
                out.push(s.as_str().to_string());
            }
        }
    }
    Ok(out)
}

fn array_length(rt: &Runtime, id: ObjectRef) -> Option<usize> {
    if !matches!(rt.obj(id).internal_kind, InternalKind::Array) {
        return None;
    }
    match rt.object_get(id, "length") {
        Value::Number(n) if n >= 0.0 => Some(n as usize),
        _ => None,
    }
}

enum MaxKeysGate {
    Limit(usize),
    Unlimited { positive_numeric: bool },
    DefaultLimit,
}

fn max_keys_gate(rt: &mut Runtime, args: &[Value]) -> Result<MaxKeysGate, RuntimeError> {
    let Some(Value::Object(opts)) = args.get(3) else {
        return Ok(MaxKeysGate::Limit(1000));
    };
    let max = match rt.spec_get(&Value::Object(*opts), "maxKeys")? {
        Value::Number(n) if n > 0.0 && n.is_finite() && n.fract() == 0.0 => {
            MaxKeysGate::Limit(n as usize)
        }
        Value::Number(n) => MaxKeysGate::Unlimited {
            positive_numeric: n > 0.0,
        },
        _ => MaxKeysGate::DefaultLimit,
    };
    Ok(max)
}

fn max_keys_schedule(rt: &mut Runtime, args: &[Value]) -> Result<MaxKeysGate, RuntimeError> {
    let first = max_keys_gate(rt, args)?;
    match first {
        MaxKeysGate::DefaultLimit => Ok(MaxKeysGate::Limit(1000)),
        MaxKeysGate::Limit(_) => {
            let _ = max_keys_gate(rt, args)?;
            max_keys_gate(rt, args)
        }
        MaxKeysGate::Unlimited {
            positive_numeric: true,
        } => {
            let _ = max_keys_gate(rt, args)?;
            max_keys_gate(rt, args)
        }
        MaxKeysGate::Unlimited {
            positive_numeric: false,
        } => match max_keys_gate(rt, args)? {
            MaxKeysGate::Limit(_)
            | MaxKeysGate::Unlimited {
                positive_numeric: true,
            } => max_keys_gate(rt, args),
            MaxKeysGate::DefaultLimit => Ok(MaxKeysGate::Limit(1000)),
            MaxKeysGate::Unlimited {
                positive_numeric: false,
            } => Ok(MaxKeysGate::Unlimited {
                positive_numeric: false,
            }),
        },
    }
}

fn querystring_array_to_string(
    rt: &mut Runtime,
    id: ObjectRef,
) -> Result<Option<String>, RuntimeError> {
    let Some(len) = array_length(rt, id) else {
        return Ok(None);
    };
    let mut out = String::new();
    for i in 0..len {
        if i > 0 {
            out.push(',');
        }
        match rt.spec_get(&Value::Object(id), &i.to_string())? {
            Value::Undefined | Value::Null => {}
            value => out.push_str(&rt.coerce_to_string(&value)?),
        }
    }
    Ok(Some(out))
}

fn querystring_arg_string_or_default(
    rt: &mut Runtime,
    args: &[Value],
    index: usize,
    default: &str,
) -> Result<String, RuntimeError> {
    match args.get(index) {
        Some(Value::Undefined | Value::Null) | None => Ok(default.to_string()),
        Some(Value::Object(id)) if matches!(rt.obj(*id).internal_kind, InternalKind::Array) => {
            Ok(querystring_array_to_string(rt, *id)?.unwrap_or_default())
        }
        Some(v) => rt.coerce_to_string(v),
    }
}

fn querystring_sep_eq_or_default(
    rt: &mut Runtime,
    args: &[Value],
    index: usize,
    default: &str,
) -> Result<String, RuntimeError> {
    if let Some(Value::Object(id)) = args.get(index) {
        if matches!(rt.obj(*id).internal_kind, InternalKind::Array) {
            return Ok(querystring_array_to_string(rt, *id)?.unwrap_or_default());
        }
    }
    let value = querystring_arg_string_or_default(rt, args, index, default)?;
    if value.is_empty() {
        Ok(default.to_string())
    } else {
        Ok(value)
    }
}

fn querystring_static_sep_eq_or_default(
    rt: &mut Runtime,
    args: &[Value],
    index: usize,
    default: &str,
) -> Result<Option<String>, RuntimeError> {
    match args.get(index) {
        Some(Value::Object(_)) | Some(Value::Symbol(_)) => Ok(None),
        _ => Ok(Some(querystring_sep_eq_or_default(
            rt, args, index, default,
        )?)),
    }
}

fn parse_sep_eq_or_default(
    rt: &mut Runtime,
    args: &[Value],
    index: usize,
    default: &str,
) -> Result<String, RuntimeError> {
    if let Some(Value::Symbol(_)) = args.get(index) {
        return Ok(rusty_js_runtime::abstract_ops::to_string(&args[index])
            .as_str()
            .to_string());
    }
    querystring_sep_eq_or_default(rt, args, index, default)
}

fn querystring_input_string(args: &[Value], index: usize) -> String {
    match args.get(index) {
        Some(Value::Undefined | Value::Null) | None => String::new(),
        Some(v) => rusty_js_runtime::abstract_ops::to_string(v)
            .as_str()
            .to_string(),
    }
}

fn node_querystring_parse_input(args: &[Value], index: usize) -> String {
    match args.get(index) {
        Some(Value::String(s)) => s.as_str().to_string(),
        _ => String::new(),
    }
}

fn node_querystring_scalar_input_js_string(
    rt: &mut Runtime,
    args: &[Value],
    index: usize,
) -> Result<Rc<JsString>, RuntimeError> {
    match args.get(index) {
        Some(Value::Symbol(_)) => Err(RuntimeError::TypeError(
            "Cannot convert a Symbol value to a string".into(),
        )),
        Some(Value::String(s)) => Ok(s.clone()),
        Some(value) => Ok(Rc::new(JsString::from(rt.coerce_to_string(value)?))),
        None => Ok(Rc::new(JsString::from("undefined"))),
    }
}

fn querystring_invalid_buffer_size_type_error(rt: &mut Runtime) -> RuntimeError {
    let msg = "The \"size\" argument must be of type number. Received undefined";
    querystring_buffer_size_type_error(rt, msg)
}

fn querystring_buffer_size_type_error(rt: &mut Runtime, msg: &str) -> RuntimeError {
    match rusty_js_runtime::intrinsics::make_error_instance(rt, "TypeError", msg) {
        Some(id) => {
            rt.object_set(
                id,
                "code".into(),
                Value::String(Rc::new(JsString::from("ERR_INVALID_ARG_TYPE"))),
            );
            RuntimeError::Thrown(Value::Object(id))
        }
        None => RuntimeError::TypeError(msg.into()),
    }
}

fn querystring_buffer_size_range_error(rt: &mut Runtime, received: &str) -> RuntimeError {
    let msg = format!(
        "The value of \"size\" is out of range. It must be >= 0 && <= 9007199254740991. Received {received}"
    );
    match rusty_js_runtime::intrinsics::make_error_instance(rt, "RangeError", &msg) {
        Some(id) => {
            rt.object_set(
                id,
                "code".into(),
                Value::String(Rc::new(JsString::from("ERR_OUT_OF_RANGE"))),
            );
            RuntimeError::Thrown(Value::Object(id))
        }
        None => RuntimeError::RangeError(msg),
    }
}

fn querystring_buffer_size_type_error_for_value(rt: &mut Runtime, v: &Value) -> RuntimeError {
    match v {
        Value::Undefined => querystring_invalid_buffer_size_type_error(rt),
        Value::Null => querystring_buffer_size_type_error(
            rt,
            "The \"size\" argument must be of type number. Received null",
        ),
        Value::Boolean(true) => querystring_buffer_size_type_error(
            rt,
            "The \"size\" argument must be of type number. Received type boolean (true)",
        ),
        Value::Boolean(false) => querystring_buffer_size_type_error(
            rt,
            "The \"size\" argument must be of type number. Received type boolean (false)",
        ),
        Value::String(s) => querystring_buffer_size_type_error(
            rt,
            &format!(
                "The \"size\" argument must be of type number. Received type string ('{}')",
                s.as_str()
            ),
        ),
        Value::Symbol(sym) => querystring_buffer_size_type_error(
            rt,
            &format!(
                "The \"size\" argument must be of type number. Received type symbol ({})",
                rusty_js_runtime::abstract_ops::to_string(&Value::Symbol(sym.clone())).as_str()
            ),
        ),
        Value::BigInt(b) => querystring_buffer_size_type_error(
            rt,
            &format!(
                "The \"size\" argument must be of type number. Received type bigint ({}n)",
                b
            ),
        ),
        Value::Object(_) => querystring_buffer_size_type_error(
            rt,
            "The \"size\" argument must be of type number. Received an instance of Object",
        ),
        Value::Number(_) => querystring_invalid_buffer_size_type_error(rt),
    }
}

fn querystring_buffer_length_value(
    rt: &mut Runtime,
    length_value: &Value,
) -> Result<usize, RuntimeError> {
    let Value::Number(n) = length_value else {
        return Err(querystring_buffer_size_type_error_for_value(
            rt,
            length_value,
        ));
    };
    if !n.is_finite() || *n < 0.0 || *n > 9_007_199_254_740_991.0 {
        let received = if n.is_nan() {
            "NaN".to_string()
        } else if *n == f64::INFINITY {
            "Infinity".to_string()
        } else if *n == f64::NEG_INFINITY {
            "-Infinity".to_string()
        } else if n.fract() == 0.0 {
            format!("{}", *n as i64)
        } else {
            n.to_string()
        };
        return Err(querystring_buffer_size_range_error(rt, &received));
    }
    Ok(n.floor() as usize)
}

fn node_querystring_unescape_buffer_input(
    rt: &mut Runtime,
    args: &[Value],
) -> Result<Rc<JsString>, RuntimeError> {
    let value = args.first().unwrap_or(&Value::Undefined);
    let length = match value {
        Value::Undefined => {
            return Err(RuntimeError::TypeError(
                "Cannot read properties of undefined (reading 'length')".into(),
            ));
        }
        Value::Null => {
            return Err(RuntimeError::TypeError(
                "Cannot read properties of null (reading 'length')".into(),
            ));
        }
        Value::String(s) => s.code_unit_len(),
        Value::Object(id) => {
            let length_value = rt.spec_get(&Value::Object(*id), "length")?;
            querystring_buffer_length_value(rt, &length_value)?
        }
        _ => return Err(querystring_invalid_buffer_size_type_error(rt)),
    };

    let string = match value {
        Value::String(s) => s.clone(),
        _ => Rc::new(JsString::from(rt.coerce_to_string(value)?)),
    };
    let units = string.code_units();
    let end = length.min(units.len());
    Ok(Rc::new(JsString::from_code_units(units[..end].to_vec())))
}

fn parse_insert(
    rt: &mut Runtime,
    out: ObjectRef,
    key: PropertyKey,
    val: Value,
) -> Result<(), RuntimeError> {
    let _value_roots = rt.push_temporary_value_roots(&[Value::Object(out), val.clone()]);
    let existing = match &key {
        PropertyKey::String(s) => rt.object_get(out, s),
        PropertyKey::Symbol(sym) => rt
            .obj(out)
            .get_own_symbol(sym)
            .map(|d| d.value.clone())
            .unwrap_or(Value::Undefined),
    };
    match existing {
        Value::Undefined => rt.object_set_pk(out, key, val),
        Value::Null => {
            return Err(RuntimeError::TypeError(
                "Cannot read properties of null (reading 'pop')".into(),
            ));
        }
        Value::Object(arr) if array_length(rt, arr).is_some() => {
            let _array_roots = rt.push_temporary_value_roots(&[
                Value::Object(out),
                Value::Object(arr),
                val.clone(),
            ]);
            let len = array_length(rt, arr).unwrap();
            rt.object_set(arr, len.to_string(), val);
            rt.object_set(arr, "length".into(), Value::Number((len + 1) as f64));
        }
        existing => {
            let _existing_roots =
                rt.push_temporary_value_roots(&[Value::Object(out), existing.clone(), val.clone()]);
            let arr = rt.alloc_object(Object::new_array());
            let _array_roots = rt.push_temporary_value_roots(&[
                Value::Object(out),
                Value::Object(arr),
                existing.clone(),
                val.clone(),
            ]);
            rt.object_set(arr, "0".into(), existing);
            rt.object_set(arr, "1".into(), val);
            rt.object_set(arr, "length".into(), Value::Number(2.0));
            rt.object_set_pk(out, key, Value::Object(arr));
        }
    }
    Ok(())
}

fn append_query_pair(out: &mut String, sep: &str, key: &str, eq: &str, value: &str) {
    if !out.is_empty() {
        out.push_str(sep);
    }
    out.push_str(key);
    out.push_str(eq);
    out.push_str(value);
}

fn querystring_keep_byte(b: u8) -> bool {
    matches!(
        b,
        b'A'..=b'Z'
            | b'a'..=b'z'
            | b'0'..=b'9'
            | b'-'
            | b'_'
            | b'.'
            | b'~'
            | b'!'
            | b'*'
            | b'\''
            | b'('
            | b')'
    )
}

fn append_percent_encoded_utf8(out: &mut String, s: &str) {
    for b in s.as_bytes() {
        if querystring_keep_byte(*b) {
            out.push(*b as char);
        } else {
            out.push('%');
            out.push(hex((*b >> 4) & 0x0f));
            out.push(hex(*b & 0x0f));
        }
    }
}

fn append_fast_encoded_key(
    rt: &mut Runtime,
    out: &mut String,
    key: &str,
) -> Result<(), RuntimeError> {
    if key.as_bytes().iter().all(|b| querystring_keep_byte(*b)) {
        out.push_str(key);
        return Ok(());
    }
    out.push_str(&percent_encode_key(rt, key)?);
    Ok(())
}

fn append_fast_encoded_value(
    rt: &mut Runtime,
    out: &mut String,
    value: &Value,
) -> Result<(), RuntimeError> {
    match value {
        Value::Undefined | Value::Null => Ok(()),
        Value::String(s) if querystring_escape_identity(s) => {
            out.push_str(s.as_str());
            Ok(())
        }
        Value::String(s) => {
            out.push_str(&percent_encode_js_string(rt, s)?);
            Ok(())
        }
        Value::Boolean(b) => {
            out.push_str(if *b { "true" } else { "false" });
            Ok(())
        }
        Value::Number(n) if n.is_finite() && n.fract() == 0.0 && n.abs() < 1e21 => {
            use std::fmt::Write;
            let _ = write!(out, "{}", *n as i64);
            Ok(())
        }
        Value::Number(_) | Value::BigInt(_) => {
            append_percent_encoded_utf8(out, value_to_qs_string(value).as_str());
            Ok(())
        }
        Value::Object(_) | Value::Symbol(_) => Ok(()),
    }
}

fn can_fast_stringify_value(v: &Value) -> bool {
    matches!(
        v,
        Value::Undefined
            | Value::Null
            | Value::String(_)
            | Value::Boolean(_)
            | Value::Number(_)
            | Value::BigInt(_)
    )
}

fn fast_array_arg_to_string(rt: &Runtime, id: ObjectRef) -> Option<String> {
    let object = rt.obj(id);
    if !matches!(object.internal_kind, InternalKind::Array) || !object.array_dense {
        return None;
    }
    let len = match object.get_own_str_borrowed("length") {
        Some(desc) if desc.getter.is_some() || desc.setter.is_some() => return None,
        Some(desc) => match desc.value {
            Value::Number(n) if n >= 0.0 => n as usize,
            _ => return None,
        },
        None => object.array_store_len(),
    };
    if len != object.array_store_len() {
        return None;
    }
    let mut out = String::new();
    for i in 0..len {
        if i > 0 {
            out.push(',');
        }
        match object.array_store_get(i) {
            Value::Undefined | Value::Null => {}
            Value::String(s) => out.push_str(s.as_str()),
            Value::Boolean(b) => out.push_str(if b { "true" } else { "false" }),
            Value::Number(n) => {
                out.push_str(rusty_js_runtime::abstract_ops::to_string(&Value::Number(n)).as_str())
            }
            Value::BigInt(b) => {
                out.push_str(rusty_js_runtime::abstract_ops::to_string(&Value::BigInt(b)).as_str())
            }
            Value::Object(_) | Value::Symbol(_) => return None,
        }
    }
    Some(out)
}

fn fast_static_sep_eq_arg(
    rt: &mut Runtime,
    args: &[Value],
    index: usize,
    default: &str,
) -> Option<Result<String, RuntimeError>> {
    match args.get(index) {
        Some(Value::Object(id)) => fast_array_arg_to_string(rt, *id).map(Ok),
        Some(Value::Symbol(_)) => None,
        _ => Some(
            querystring_arg_string_or_default(rt, args, index, default).map(|value| {
                if value.is_empty() {
                    default.to_string()
                } else {
                    value
                }
            }),
        ),
    }
}

fn querystring_default_shape_stringify_fast(
    rt: &mut Runtime,
    obj: ObjectRef,
    sep: &str,
    eq: &str,
) -> Option<Result<Value, RuntimeError>> {
    let pairs = {
        let object = rt.obj(obj);
        if !matches!(object.internal_kind, InternalKind::Ordinary) {
            return None;
        }
        let shape = object.shape.as_ref()?;
        if object.properties.iter().any(|(key, desc)| {
            desc.enumerable
                && key.is_string()
                && key.as_str() != "__primitive__"
                && !key.as_str().starts_with("@@sym:")
        }) {
            return None;
        }
        let mut pairs = Vec::with_capacity(shape.slot_count() as usize);
        for (key, slot) in shape.iter_slots() {
            if key.bytes().next().is_some_and(|b| b.is_ascii_digit()) {
                return None;
            }
            let value = object.shape_values.get(slot as usize)?.clone();
            if !can_fast_stringify_value(&value) {
                return None;
            }
            pairs.push((key.to_string(), value));
        }
        pairs
    };
    let mut out = String::new();
    for (key, value) in pairs {
        if !out.is_empty() {
            out.push_str(sep);
        }
        if let Err(err) = append_fast_encoded_key(rt, &mut out, &key) {
            return Some(Err(err));
        }
        out.push_str(eq);
        if let Err(err) = append_fast_encoded_value(rt, &mut out, &value) {
            return Some(Err(err));
        }
    }
    Some(Ok(sval(&out)))
}

pub fn install_canonical(rt: &mut Runtime) {
    let ns = new_object(rt);

    register_method(rt, ns, "parse", |rt, args| {
        let input = querystring_input_string(args, 0);
        let out = rt.alloc_object_with_explicit_null_proto(Object::new_dictionary());
        let _out_root = rt.push_temporary_value_roots(&[Value::Object(out)]);
        if input.is_empty() {
            return Ok(Value::Object(out));
        }
        for raw in input.split('&') {
            if raw.is_empty() {
                continue;
            }
            let (k, v) = raw.split_once('=').unwrap_or((raw, ""));
            let key = percent_decode(k, true);
            let val = sval(&percent_decode(v, true));
            parse_insert(rt, out, PropertyKey::String(key), val)?;
        }
        Ok(Value::Object(out))
    });

    register_method(rt, ns, "stringify", |rt, args| {
        let obj = match args.first() {
            Some(Value::Object(id)) => *id,
            _ => return Ok(sval("")),
        };
        let mut out = String::new();
        for key in own_enumerable_string_keys(rt, obj)? {
            let value = object_value_via_get(rt, obj, &key)?;
            if let Value::Object(arr) = value {
                if let Some(len) = array_length(rt, arr) {
                    for i in 0..len {
                        let v = object_value_via_get(rt, arr, &i.to_string())?;
                        let encoded_key = percent_encode_key(rt, &key)?;
                        let encoded_value = percent_encode_qs_value(rt, &v)?;
                        append_query_pair(&mut out, "&", &encoded_key, "=", &encoded_value);
                    }
                    continue;
                }
            }
            let encoded_key = percent_encode_key(rt, &key)?;
            let encoded_value = percent_encode_qs_value(rt, &value)?;
            append_query_pair(&mut out, "&", &encoded_key, "=", &encoded_value);
        }
        Ok(sval(&out))
    });

    rt.define_global_property("__cruft_querystring", Value::Object(ns));
}

pub fn install(rt: &mut Runtime) {
    let qs = new_object(rt);

    register_method(rt, qs, "escape", |rt, args| {
        let input = node_querystring_scalar_input_js_string(rt, args, 0)?;
        if querystring_escape_identity(&input) {
            return Ok(Value::String(input));
        }
        let escaped = percent_encode_js_string(rt, &input)?;
        Ok(sval(&escaped))
    });

    let original_unescape = native_function(rt, "unescape", |rt, args| {
        let input = node_querystring_scalar_input_js_string(rt, args, 0)?;
        Ok(sval(&percent_decode(input.as_str(), false)))
    });
    let Value::Object(original_unescape_id) = original_unescape else {
        unreachable!("native_function returns an object")
    };
    rt.object_set(qs, "unescape".into(), original_unescape);

    register_method(rt, qs, "parse", move |rt, args| {
        let input = node_querystring_parse_input(args, 0);
        if input.is_empty() {
            let out = rt.alloc_object_with_explicit_null_proto(Object::new_dictionary());
            return Ok(Value::Object(out));
        }
        if args.len() <= 1 {
            let decoder = default_parse_decoder(rt, qs)?;
            if decoder == Value::Object(original_unescape_id) {
                if let Some(value) = querystring_default_parse_unique_fast(rt, &input) {
                    return Ok(value);
                }
            }
        }
        let out = rt.alloc_object_with_explicit_null_proto(Object::new_dictionary());
        let _out_root = rt.push_temporary_value_roots(&[Value::Object(out)]);
        let decoder = match parse_decoder_option(rt, args.get(3))? {
            Some(decoder) => ParseDecoder::Custom(decoder),
            None => ParseDecoder::Default(default_parse_decoder(rt, qs)?),
        };
        let sep = parse_sep_eq_or_default(rt, args, 1, "&")?;
        let eq = parse_sep_eq_or_default(rt, args, 2, "=")?;
        if sep.is_empty() {
            let key = if eq.is_empty() {
                ""
            } else {
                input.split_once(&eq).map(|(k, _)| k).unwrap_or(&input)
            };
            if let Some(key) = decode_query_component_key(rt, key, true, Some(decoder.clone()))? {
                let val = sval("");
                parse_insert(rt, out, key, val)?;
            }
            return Ok(Value::Object(out));
        }
        let mut seen = 0usize;
        let max = max_keys_schedule(rt, args)?;
        let use_default_fast_path =
            parse_uses_original_default_decoder(&decoder, original_unescape_id);
        for raw in input.split(&sep) {
            match max {
                MaxKeysGate::Limit(max) => {
                    if seen >= max {
                        break;
                    }
                }
                MaxKeysGate::Unlimited { .. } | MaxKeysGate::DefaultLimit => {}
            }
            seen += 1;
            if raw.is_empty() {
                continue;
            }
            let (k, v) = if eq.is_empty() {
                ("", raw)
            } else {
                match raw.split_once(&eq) {
                    Some((k, v)) => (k, v),
                    None => (raw, ""),
                }
            };
            if use_default_fast_path {
                parse_insert_default_decoded(rt, out, k, v)?;
                continue;
            }
            let decoded_key = decode_query_component_value(rt, k, true, Some(decoder.clone()))?;
            let val = decode_query_component_value(rt, v, true, Some(decoder.clone()))?;
            let _decoded_roots = rt.push_temporary_value_roots(&[decoded_key.clone(), val.clone()]);
            let key = project_decoded_query_key(rt, &decoded_key)?;
            if let Some(key) = key {
                parse_insert(rt, out, key, val)?;
            }
        }
        Ok(Value::Object(out))
    });

    register_method(rt, qs, "stringify", |rt, args| {
        let obj = match args.first() {
            Some(Value::Object(id)) => *id,
            _ => return Ok(sval("")),
        };
        if args.len() <= 3 {
            let sep = match fast_static_sep_eq_arg(rt, args, 1, "&") {
                Some(Ok(sep)) => Some(sep),
                Some(Err(err)) => return Err(err),
                None => None,
            };
            let eq = match fast_static_sep_eq_arg(rt, args, 2, "=") {
                Some(Ok(eq)) => Some(eq),
                Some(Err(err)) => return Err(err),
                None => None,
            };
            if let (Some(sep), Some(eq)) = (sep, eq) {
                if let Some(result) = querystring_default_shape_stringify_fast(rt, obj, &sep, &eq) {
                    return result;
                }
            }
        }
        let encoder = stringify_encoder_option(rt, args.get(3))?;
        let static_sep = querystring_static_sep_eq_or_default(rt, args, 1, "&")?;
        let static_eq = querystring_static_sep_eq_or_default(rt, args, 2, "=")?;
        let mut out = String::new();
        for key in own_enumerable_string_keys(rt, obj)? {
            let value = object_value_via_get(rt, obj, &key)?;
            if let Value::Object(arr) = value {
                if let Some(len) = array_length(rt, arr) {
                    let encoded_key = encode_query_key(rt, &key, encoder.clone())?;
                    for i in 0..len {
                        let v = object_value_via_get(rt, arr, &i.to_string())?;
                        let encoded_value = encode_query_value(rt, &v, encoder.clone())?;
                        let sep = if out.is_empty() {
                            ""
                        } else {
                            static_sep.as_deref().unwrap_or("")
                        };
                        let dynamic_sep;
                        let sep = if !out.is_empty() && static_sep.is_none() {
                            dynamic_sep = querystring_sep_eq_or_default(rt, args, 1, "&")?;
                            dynamic_sep.as_str()
                        } else {
                            sep
                        };
                        let dynamic_eq;
                        let eq = if let Some(eq) = static_eq.as_deref() {
                            eq
                        } else {
                            dynamic_eq = querystring_sep_eq_or_default(rt, args, 2, "=")?;
                            dynamic_eq.as_str()
                        };
                        append_query_pair(&mut out, &sep, &encoded_key, &eq, &encoded_value);
                    }
                    continue;
                }
            }
            let encoded_key = encode_query_key(rt, &key, encoder.clone())?;
            let encoded_value = encode_query_value(rt, &value, encoder.clone())?;
            let sep = if out.is_empty() {
                ""
            } else {
                static_sep.as_deref().unwrap_or("")
            };
            let dynamic_sep;
            let sep = if !out.is_empty() && static_sep.is_none() {
                dynamic_sep = querystring_sep_eq_or_default(rt, args, 1, "&")?;
                dynamic_sep.as_str()
            } else {
                sep
            };
            let dynamic_eq;
            let eq = if let Some(eq) = static_eq.as_deref() {
                eq
            } else {
                dynamic_eq = querystring_sep_eq_or_default(rt, args, 2, "=")?;
                dynamic_eq.as_str()
            };
            append_query_pair(&mut out, &sep, &encoded_key, &eq, &encoded_value);
        }
        Ok(sval(&out))
    });

    let parse = rt.object_get(qs, "parse");
    let stringify = rt.object_get(qs, "stringify");
    let escape = rt.object_get(qs, "escape");
    let unescape = rt.object_get(qs, "unescape");
    set_constant(rt, qs, "decode", parse);
    set_constant(rt, qs, "encode", stringify);
    set_constant(rt, qs, "escape", escape);
    set_constant(rt, qs, "unescape", unescape);
    register_method(rt, qs, "unescapeBuffer", |rt, args| {
        let input = node_querystring_unescape_buffer_input(rt, args)?;
        let plus_as_space = args
            .get(1)
            .map(rusty_js_runtime::abstract_ops::to_boolean)
            .unwrap_or(false);
        let bytes = percent_decode_code_units_to_bytes(input.code_units().as_ref(), plus_as_space);
        Ok(crate::node_stubs::intrinsic_buffer_from_bytes(rt, &bytes))
    });
    rt.define_global_property("querystring", Value::Object(qs));
}
