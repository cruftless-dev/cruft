
use crate::register::{make_callable, make_callable_rooted, new_object, register_method};
use rusty_js_runtime::value::{InternalKind, Object as RtObject, PropertyDescriptor, PropertyKey};
use rusty_js_runtime::{HostEnqueuePhase, Runtime, RuntimeError, Value};
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

fn stream_event_name(rt: &Runtime, v: Option<&Value>) -> String {
    match v {
        Some(Value::String(s)) => s.as_str().to_string(),
        Some(other) => rusty_js_runtime::abstract_ops::to_string(other)
            .as_str()
            .to_string(),
        None => String::new(),
    }
}

fn stream_emit(rt: &mut Runtime, obj: rusty_js_runtime::ObjectRef, event: &str, args: Vec<Value>) {
    let emit = rt.object_get(obj, "emit");
    if rt.is_callable(&emit) {
        let mut emit_args = Vec::with_capacity(args.len() + 1);
        emit_args.push(Value::String(Rc::new(
            rusty_js_runtime::value::JsString::from(event),
        )));
        emit_args.extend(args);
        let _ = rt.call_function(emit, Value::Object(obj), emit_args);
        return;
    }
    let reg = match rt.object_get(obj, "__stream_listeners") {
        Value::Object(r) => r,
        _ => return,
    };
    let arr = match rt.object_get(reg, event) {
        Value::Object(a) => a,
        _ => return,
    };
    let n = rt.array_length(arr);
    for i in 0..n {
        let cb = rt.object_get(arr, &i.to_string());
        if rt.is_callable(&cb) {
            let _ = rt.call_function(cb, Value::Object(obj), args.clone());
        }
    }
}

fn stream_listener_registry(
    rt: &mut Runtime,
    obj: rusty_js_runtime::ObjectRef,
    create: bool,
) -> Option<rusty_js_runtime::ObjectRef> {
    let key = Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
        "__stream_listeners",
    )));
    if matches!(
        rt.object_get_own_property_descriptor_via(&Value::Object(obj), &key),
        Ok(Value::Object(_))
    ) {
        if let Value::Object(reg) = rt.object_get(obj, "__stream_listeners") {
            return Some(reg);
        }
    }
    if !create {
        return None;
    }
    let reg = rt.alloc_object(RtObject::new_ordinary());
    rt.object_set(obj, "__stream_listeners".into(), Value::Object(reg));
    Some(reg)
}

fn b64_incremental(rt: &mut Runtime, obj: rusty_js_runtime::ObjectRef, chunk: &Value) -> String {
    let latin1 = buf_to_latin1(rt, chunk);
    let pend = match rt.object_get(obj, "__rs_b64pend") {
        Value::String(s) => s.as_str().to_string(),
        _ => String::new(),
    };
    let mut bytes: Vec<u8> = pend.chars().map(|c| (c as u32 & 0xff) as u8).collect();
    bytes.extend(latin1.chars().map(|c| (c as u32 & 0xff) as u8));
    let n = (bytes.len() / 3) * 3;
    let complete: String = bytes[..n].iter().map(|&b| b as char).collect();
    let rest: String = bytes[n..].iter().map(|&b| b as char).collect();
    rt.object_set(
        obj,
        "__rs_b64pend".into(),
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(rest))),
    );
    latin1_to_base64(rt, &complete)
}

fn b64_flush(rt: &mut Runtime, obj: rusty_js_runtime::ObjectRef) -> String {
    let pend = match rt.object_get(obj, "__rs_b64pend") {
        Value::String(s) => s.as_str().to_string(),
        _ => String::new(),
    };
    if pend.is_empty() {
        return String::new();
    }
    rt.object_set(
        obj,
        "__rs_b64pend".into(),
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(""))),
    );
    latin1_to_base64(rt, &pend)
}

fn buf_to_latin1(rt: &mut Runtime, chunk: &Value) -> String {
    if let Value::Object(c) = chunk {
        let ts = rt.object_get(*c, "toString");
        if rt.is_callable(&ts) {
            let enc = Value::String(Rc::new(rusty_js_runtime::value::JsString::from("latin1")));
            if let Ok(Value::String(s)) = rt.call_function(ts, chunk.clone(), vec![enc]) {
                return s.as_str().to_string();
            }
        }
    }
    String::new()
}

fn latin1_to_base64(rt: &mut Runtime, latin1: &str) -> String {
    if latin1.is_empty() {
        return String::new();
    }
    let buffer = rt.global_get("Buffer");
    if let Value::Object(b) = buffer {
        let from = rt.object_get(b, "from");
        if rt.is_callable(&from) {
            let s = Value::String(Rc::new(rusty_js_runtime::value::JsString::from(latin1)));
            let l1 = Value::String(Rc::new(rusty_js_runtime::value::JsString::from("latin1")));
            if let Ok(buf @ Value::Object(_)) = rt.call_function(from, buffer.clone(), vec![s, l1])
            {
                if let Value::Object(bid) = buf {
                    let ts = rt.object_get(bid, "toString");
                    if rt.is_callable(&ts) {
                        let b64 = Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                            "base64",
                        )));
                        if let Ok(Value::String(out)) = rt.call_function(ts, buf.clone(), vec![b64])
                        {
                            return out.as_str().to_string();
                        }
                    }
                }
            }
        }
    }
    String::new()
}

fn buffer_from_string(
    rt: &mut Runtime,
    s: Rc<rusty_js_runtime::value::JsString>,
    enc: &str,
) -> Value {
    let buffer = rt.global_get("Buffer");
    if let Value::Object(b) = buffer {
        let from = rt.object_get(b, "from");
        if rt.is_callable(&from) {
            let input = Value::String(s.clone());
            let encoding = Value::String(Rc::new(rusty_js_runtime::value::JsString::from(enc)));
            if let Ok(buf @ Value::Object(_)) =
                rt.call_function(from, Value::Object(b), vec![input, encoding])
            {
                return buf;
            }
        }
    }
    Value::String(s)
}

fn stream_buffer_head(rt: &mut Runtime, buf: rusty_js_runtime::ObjectRef) -> usize {
    match rt.object_get(buf, "__rbuf_head") {
        Value::Number(n) if n.is_finite() && n >= 0.0 => n as usize,
        _ => 0,
    }
}

fn stream_buffer_set_head(rt: &mut Runtime, buf: rusty_js_runtime::ObjectRef, head: usize) {
    rt.obj_mut(buf)
        .set_own_internal("__rbuf_head".into(), Value::Number(head as f64));
}

fn readable_object_mode(rt: &mut Runtime, stream_id: rusty_js_runtime::ObjectRef) -> bool {
    if matches!(rt.object_get(stream_id, "__objmode"), Value::Boolean(true)) {
        return true;
    }
    match rt.object_get(stream_id, "_readableState") {
        Value::Object(rs) => matches!(rt.object_get(rs, "objectMode"), Value::Boolean(true)),
        _ => false,
    }
}

fn readable_from_set_index(rt: &mut Runtime, stream_id: rusty_js_runtime::ObjectRef, index: usize) {
    rt.obj_mut(stream_id)
        .set_own("__riter_index".into(), Value::Number(index as f64));
}

fn readable_chunk_units(
    rt: &mut Runtime,
    stream_id: rusty_js_runtime::ObjectRef,
    chunk: &Value,
) -> f64 {
    if readable_object_mode(rt, stream_id) {
        return 1.0;
    }
    match chunk {
        Value::String(s) => s.as_str().len() as f64,
        Value::Object(id) => match rt.object_get(*id, "length") {
            Value::Number(n) if n.is_finite() && n >= 0.0 => n,
            _ => 1.0,
        },
        _ => 1.0,
    }
}

fn readable_state_number(rt: &Runtime, stream_id: rusty_js_runtime::ObjectRef, key: &str) -> f64 {
    match rt.object_get(stream_id, "_readableState") {
        Value::Object(rs) => match rt.object_get(rs, key) {
            Value::Number(n) if n.is_finite() && n >= 0.0 => n,
            _ => 0.0,
        },
        _ => 0.0,
    }
}

fn readable_state_set(
    rt: &mut Runtime,
    stream_id: rusty_js_runtime::ObjectRef,
    key: &str,
    value: Value,
) {
    if let Value::Object(rs) = rt.object_get(stream_id, "_readableState") {
        rt.object_set(rs, key.into(), value.clone());
    }

    if key == "length" {
        rt.object_set(stream_id, "readableLength".into(), value);
    }
}

fn stream_finish_readable_if_drained(rt: &mut Runtime, obj: rusty_js_runtime::ObjectRef) {
    if !matches!(rt.object_get(obj, "__rended"), Value::Boolean(true))
        || matches!(rt.object_get(obj, "__endfired"), Value::Boolean(true))
    {
        return;
    }
    let drained = match rt.object_get(obj, "__rbuf") {
        Value::Object(buf) => {

            let head = if matches!(rt.object_get(obj, "__rbuf_source"), Value::Boolean(true)) {
                match rt.object_get(obj, "__riter_index") {
                    Value::Number(n) if n.is_finite() && n >= 0.0 => n as usize,
                    _ => 0,
                }
            } else {
                stream_buffer_head(rt, buf)
            };
            head >= rt.array_length(buf)
        }
        _ => true,
    };
    if !drained {
        return;
    }
    rt.object_set(obj, "__endfired".into(), Value::Boolean(true));
    rt.object_set(obj, "readableEnded".into(), Value::Boolean(true));
    readable_state_set(rt, obj, "ended", Value::Boolean(true));
    stream_emit(rt, obj, "end", Vec::new());
    stream_emit(rt, obj, "close", Vec::new());
}

fn async_iter_flow_pump(rt: &mut Runtime, stream: rusty_js_runtime::ObjectRef) {
    if !matches!(rt.object_get(stream, "__async_iter"), Value::Object(_))
        || !matches!(rt.object_get(stream, "__flowing"), Value::Boolean(true))
        || matches!(rt.object_get(stream, "__endfired"), Value::Boolean(true))
    {
        return;
    }
    let next = rt.object_get(stream, "next");
    if !rt.is_callable(&next) {
        return;
    }
    let p = match rt.call_function(next, Value::Object(stream), Vec::new()) {
        Ok(v) => v,
        Err(_) => return,
    };
    let then = match &p {
        Value::Object(pid) => rt.object_get(*pid, "then"),
        _ => Value::Undefined,
    };
    if !rt.is_callable(&then) {
        return;
    }
    let on_ful = make_callable(rt, "readable.asyncPump", move |rt, a| {
        let result = a.first().cloned().unwrap_or(Value::Undefined);
        let (value, done) = match &result {
            Value::Object(rid) => (
                rt.object_get(*rid, "value"),
                matches!(rt.object_get(*rid, "done"), Value::Boolean(true)),
            ),
            _ => (Value::Undefined, true),
        };
        if done {
            if !matches!(rt.object_get(stream, "__endfired"), Value::Boolean(true)) {
                rt.object_set(stream, "__endfired".into(), Value::Boolean(true));
                rt.object_set(stream, "readableEnded".into(), Value::Boolean(true));
                readable_state_set(rt, stream, "ended", Value::Boolean(true));
                stream_emit(rt, stream, "end", Vec::new());
                stream_emit(rt, stream, "close", Vec::new());
            }
        } else {

            let value = if matches!(
                rt.object_get(stream, "__coerce_buffer"),
                Value::Boolean(true)
            ) {
                match &value {
                    Value::String(_) | Value::Object(_) => {
                        let buf_ctor = rt.global_get("Buffer");
                        if let Value::Object(bc) = buf_ctor {
                            let from = rt.object_get(bc, "from");
                            if rt.is_callable(&from) {
                                match rt.call_function(from, buf_ctor.clone(), vec![value.clone()])
                                {
                                    Ok(b) => b,
                                    Err(_) => value,
                                }
                            } else {
                                value
                            }
                        } else {
                            value
                        }
                    }
                    _ => value,
                }
            } else {
                value
            };
            stream_emit(rt, stream, "data", vec![value]);
            async_iter_flow_pump(rt, stream);
        }
        Ok(Value::Undefined)
    });
    let on_rej = make_callable(rt, "readable.asyncPumpErr", move |rt, a| {
        let err = a.first().cloned().unwrap_or(Value::Undefined);
        stream_emit(rt, stream, "error", vec![err]);
        Ok(Value::Undefined)
    });
    let _ = rt.call_function(then, p, vec![Value::Object(on_ful), Value::Object(on_rej)]);
}

fn make_stream_buffer_from_chunks(
    rt: &mut Runtime,
    chunks: impl IntoIterator<Item = Value>,
) -> rusty_js_runtime::ObjectRef {
    let mut rbuf = RtObject::new_array();
    rbuf.array_dense = true;
    rbuf.dense_elements.extend(chunks);
    let rbuf = rt.alloc_object(rbuf);
    stream_buffer_set_head(rt, rbuf, 0);
    rbuf
}

fn materialize_source_backed_rbuf(
    rt: &mut Runtime,
    stream_id: rusty_js_runtime::ObjectRef,
    source_id: rusty_js_runtime::ObjectRef,
) -> rusty_js_runtime::ObjectRef {
    let len = rt.array_length(source_id);
    let mut rbuf = RtObject::new_array();
    rbuf.array_dense = true;
    rbuf.dense_elements.reserve(len);
    for i in 0..len {
        rbuf.dense_elements
            .push(rt.object_get(source_id, &i.to_string()));
    }
    let rbuf = rt.alloc_object(rbuf);
    let head = match rt.object_get(stream_id, "__riter_index") {
        Value::Number(n) if n.is_finite() && n >= 0.0 => n as usize,
        _ => 0,
    };
    stream_buffer_set_head(rt, rbuf, head.min(len));
    rt.object_set(stream_id, "__rbuf".into(), Value::Object(rbuf));
    rt.object_set(stream_id, "__rbuf_source".into(), Value::Boolean(false));
    rbuf
}

fn stream_buffer_push_tail(
    rt: &mut Runtime,
    buf: rusty_js_runtime::ObjectRef,
    head: usize,
) -> usize {
    match rt.object_get(buf, "__rbuf_push_tail") {
        Value::Number(n) if n.is_finite() && n >= head as f64 => n as usize,
        _ => head,
    }
}

fn stream_buffer_set_push_tail(rt: &mut Runtime, buf: rusty_js_runtime::ObjectRef, tail: usize) {
    rt.object_set(buf, "__rbuf_push_tail".into(), Value::Number(tail as f64));
}

fn stream_drain(rt: &mut Runtime, obj: rusty_js_runtime::ObjectRef) {
    if !matches!(rt.object_get(obj, "__flowing"), Value::Boolean(true)) {
        return;
    }
    let buf = match rt.object_get(obj, "__rbuf") {
        Value::Object(b) => b,
        _ => return,
    };
    loop {
        if !matches!(rt.object_get(obj, "__flowing"), Value::Boolean(true)) {
            break;
        }
        let len = rt.array_length(buf);
        let head = stream_buffer_head(rt, buf);
        if head >= len {
            break;
        }
        let chunk = rt.object_get(buf, &head.to_string());
        stream_buffer_set_head(rt, buf, head + 1);

        let next_length = (readable_state_number(rt, obj, "length")
            - readable_chunk_units(rt, obj, &chunk))
        .max(0.0);
        readable_state_set(rt, obj, "length", Value::Number(next_length));

        let chunk = match rt.object_get(obj, "__rs_encoding") {
            Value::String(enc) if matches!(chunk, Value::Object(_)) => {
                if enc.as_str() == "base64" {

                    Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                        b64_incremental(rt, obj, &chunk),
                    )))
                } else {
                    let ts = rt.object_get(
                        if let Value::Object(c) = chunk {
                            c
                        } else {
                            unreachable!()
                        },
                        "toString",
                    );
                    if rt.is_callable(&ts) {
                        let encv = Value::String(enc);
                        rt.call_function(ts, chunk.clone(), vec![encv])
                            .unwrap_or(chunk)
                    } else {
                        chunk
                    }
                }
            }
            _ => chunk,
        };
        stream_emit(rt, obj, "data", vec![chunk]);
    }
    if matches!(rt.object_get(obj, "__rended"), Value::Boolean(true))
        && !matches!(rt.object_get(obj, "__endfired"), Value::Boolean(true))
    {
        rt.object_set(obj, "__endfired".into(), Value::Boolean(true));

        if matches!(rt.object_get(obj, "__rs_encoding"), Value::String(ref s) if s.as_str() == "base64")
        {
            let tail = b64_flush(rt, obj);
            if !tail.is_empty() {
                let tv = Value::String(Rc::new(rusty_js_runtime::value::JsString::from(tail)));
                stream_emit(rt, obj, "data", vec![tv]);
            }
        }

        rt.object_set(obj, "readableEnded".into(), Value::Boolean(true));
        if let Value::Object(rs) = rt.object_get(obj, "_readableState") {
            rt.object_set(rs, "ended".into(), Value::Boolean(true));
        }
        stream_emit(rt, obj, "end", Vec::new());
        stream_emit(rt, obj, "close", Vec::new());
    }
}

fn stream_end_transform_readable(rt: &mut Runtime, this: rusty_js_runtime::ObjectRef) {
    let end_readable = make_callable(rt, "stream.flushCb", move |rt, _a| {
        if !matches!(rt.object_get(this, "__rended"), Value::Boolean(true)) {
            rt.object_set(this, "__rended".into(), Value::Boolean(true));
            rt.object_set(this, "readable".into(), Value::Boolean(false));
            readable_state_set(rt, this, "ended", Value::Boolean(true));
            readable_state_set(rt, this, "reading", Value::Boolean(false));
            stream_emit(rt, this, "readable", Vec::new());
            stream_finish_readable_if_drained(rt, this);
            stream_drain(rt, this);
        }
        Ok(Value::Undefined)
    });
    let opts_flush = match rt.object_get(this, "_options") {
        Value::Object(o) => rt.object_get(o, "flush"),
        _ => Value::Undefined,
    };
    let flusher = if rt.is_callable(&opts_flush) {
        opts_flush
    } else {
        rt.object_get(this, "_flush")
    };
    if rt.is_callable(&flusher) && !stream_is_default_template(rt, &flusher) {
        let _ = rt.call_function(
            flusher,
            Value::Object(this),
            vec![Value::Object(end_readable)],
        );
    } else {
        let _ = rt.call_function(Value::Object(end_readable), Value::Object(this), Vec::new());
    }
}

fn stream_schedule_read(rt: &mut Runtime, obj: rusty_js_runtime::ObjectRef) {
    if matches!(
        rt.object_get(obj, "__stream_read_scheduled"),
        Value::Boolean(true)
    ) {
        return;
    }
    let read = rt.object_get(obj, "_read");
    if !rt.is_callable(&read) || stream_is_default_template(rt, &read) {
        return;
    }
    readable_state_set(rt, obj, "reading", Value::Boolean(true));
    rt.object_set(obj, "__stream_read_scheduled".into(), Value::Boolean(true));
    rt.enqueue_microtask_rooted("stream readable _read", vec![obj], move |rt| {
        rt.object_set(obj, "__stream_read_scheduled".into(), Value::Boolean(false));
        if matches!(rt.object_get(obj, "__rended"), Value::Boolean(true)) {
            readable_state_set(rt, obj, "reading", Value::Boolean(false));
            return Ok(());
        }
        let read = rt.object_get(obj, "_read");
        if rt.is_callable(&read) && !stream_is_default_template(rt, &read) {
            let high_water_mark = match rt.object_get(obj, "_readableState") {
                Value::Object(rs) => match rt.object_get(rs, "highWaterMark") {
                    Value::Number(n) if n.is_finite() && n > 0.0 => n,
                    _ => 16384.0,
                },
                _ => 16384.0,
            };
            let _ = rt.call_function(
                read,
                Value::Object(obj),
                vec![Value::Number(high_water_mark)],
            );
        }
        Ok(())
    });
}

fn stream_push_value(
    rt: &mut Runtime,
    this: rusty_js_runtime::ObjectRef,
    chunk: &Value,
) -> Result<bool, RuntimeError> {
    if matches!(chunk, Value::String(ref s) if s.as_str().is_empty())
        && !crate::stream::readable_object_mode(rt, this)
    {
        stream_schedule_read(rt, this);
        return Ok(true);
    }
    let mut buf = match rt.object_get(this, "__rbuf") {
        Value::Object(b) => b,
        _ => return Ok(true),
    };
    if matches!(rt.object_get(this, "__rbuf_source"), Value::Boolean(true)) {
        buf = materialize_source_backed_rbuf(rt, this, buf);
    }
    let len = rt.array_length(buf);
    let head = stream_buffer_head(rt, buf);
    let ended = matches!(rt.object_get(this, "__rended"), Value::Boolean(true));
    if ended && head > 0 && head < len {
        let tail = stream_buffer_push_tail(rt, buf, head).min(len);
        for i in (tail..len).rev() {
            let v = rt.object_get(buf, &i.to_string());
            rt.object_set(buf, (i + 1).to_string(), v);
        }
        rt.object_set(buf, tail.to_string(), chunk.clone());
        stream_buffer_set_push_tail(rt, buf, tail + 1);
    } else {
        rt.object_set(buf, len.to_string(), chunk.clone());
    }
    rt.object_set(buf, "length".into(), Value::Number((len + 1) as f64));
    let next_length =
        readable_state_number(rt, this, "length") + readable_chunk_units(rt, this, chunk);
    readable_state_set(rt, this, "length", Value::Number(next_length));
    let high_water_mark = readable_state_number(rt, this, "highWaterMark");
    if next_length < high_water_mark {
        stream_schedule_read(rt, this);
    }
    stream_emit(rt, this, "readable", Vec::new());
    stream_drain(rt, this);
    Ok(!ended)
}

fn stream_has_callable_transform_impl(rt: &mut Runtime, this: rusty_js_runtime::ObjectRef) -> bool {
    let opts = rt.object_get(this, "_options");
    if let Value::Object(o) = opts {
        if rt.is_callable(&rt.object_get(o, "transform")) {
            return true;
        }
    }
    let transform = rt.object_get(this, "_transform");
    rt.is_callable(&transform) && !stream_is_default_template(rt, &transform)
}

fn stream_is_default_template(rt: &mut Runtime, value: &Value) -> bool {
    match value {
        Value::Object(f) => matches!(
            rt.object_get(*f, "__cruft_stream_default_template"),
            Value::Boolean(true)
        ),
        _ => false,
    }
}

fn install_stream_default_template(
    rt: &mut Runtime,
    proto: rusty_js_runtime::ObjectRef,
    name: &str,
) {
    let f = make_callable(rt, name, |_rt, _args| Ok(Value::Undefined));
    rt.obj_mut(f).set_own_internal(
        "__cruft_stream_default_template".into(),
        Value::Boolean(true),
    );
    rt.object_set(proto, name.into(), Value::Object(f));
}

fn stream_chunk_is_binary(rt: &Runtime, v: &Value) -> bool {
    match v {
        Value::String(_) => true,
        Value::Object(id) => rt.obj(*id).is_buffer || rt.typed_array_views.contains_key(id),
        _ => false,
    }
}

fn stream_received_desc(rt: &Runtime, v: &Value) -> String {
    use rusty_js_runtime::abstract_ops::to_string;
    match v {
        Value::Number(_) => format!("type number ({})", to_string(v).as_str()),
        Value::Boolean(_) => format!("type boolean ({})", to_string(v).as_str()),
        Value::BigInt(_) => format!("type bigint ({}n)", to_string(v).as_str()),
        Value::Symbol(_) => format!("type symbol ({})", to_string(v).as_str()),
        Value::Undefined => "undefined".to_string(),
        Value::Object(id) => {
            let name = match rt.obj(*id).proto {
                Some(p) => match rt.object_get(p, "constructor") {
                    Value::Object(cid) => match rt.object_get(cid, "name") {
                        Value::String(s) if !s.as_str().is_empty() => s.as_str().to_string(),
                        _ => "Object".to_string(),
                    },
                    _ => "Object".to_string(),
                },
                None => "Object".to_string(),
            };
            format!("an instance of {name}")
        }
        _ => to_string(v).as_str().to_string(),
    }
}

fn stream_coded_error(rt: &mut Runtime, code: &str, msg: &str) -> RuntimeError {
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

fn make_stream_instance(
    rt: &mut Runtime,
    opts: Option<rusty_js_runtime::ObjectRef>,
    receiver: Option<rusty_js_runtime::ObjectRef>,
    kind: &str,
    prototype: Option<rusty_js_runtime::ObjectRef>,
    initial_rbuf: Option<rusty_js_runtime::ObjectRef>,
    initial_rended: bool,
    initial_riter_index: Option<f64>,
) -> rusty_js_runtime::ObjectRef {

    let fresh_instance = receiver.is_none();
    let id = if let Some(r) = receiver {
        r
    } else {
        rt.alloc_object(RtObject::new_ordinary())
    };
    if let Some(proto) = prototype {
        rt.set_object_prototype_internal(id, Some(proto));
    }
    if !fresh_instance && kind == "Stream" {
        let has_userland_stream_state = |rt: &mut Runtime, key: &str| -> bool {
            let k = Value::String(Rc::new(rusty_js_runtime::value::JsString::from(key)));
            matches!(
                rt.object_get_own_property_descriptor_via(&Value::Object(id), &k),
                Ok(Value::Object(_))
            )
        };
        if has_userland_stream_state(rt, "_readableState")
            || has_userland_stream_state(rt, "_writableState")
        {
            return id;
        }
    }
    let has_initial_rbuf = initial_rbuf.is_some();
    let rbuf = initial_rbuf.unwrap_or_else(|| {
        let mut rbuf_obj = RtObject::new_array();
        rbuf_obj.array_dense = true;
        let rbuf = rt.alloc_object(rbuf_obj);
        rt.object_set(rbuf, "length".into(), Value::Number(0.0));
        stream_buffer_set_head(rt, rbuf, 0);
        rbuf
    });

    let rs = rt.alloc_object(RtObject::new_ordinary());
    let rs_buf = if has_initial_rbuf {
        rbuf
    } else {
        rt.alloc_object(RtObject::new_array())
    };

    let rs_pipes = rt.alloc_object(RtObject::new_array());
    {
        let rs_obj = rt.obj_mut(rs);
        rs_obj.set_own("highWaterMark".into(), Value::Number(16384.0));
        rs_obj.set_own("buffer".into(), Value::Object(rs_buf));
        rs_obj.set_own("length".into(), Value::Number(0.0));
        rs_obj.set_own("ended".into(), Value::Boolean(false));
        rs_obj.set_own("pipes".into(), Value::Object(rs_pipes));
        rs_obj.set_own("pipesCount".into(), Value::Number(0.0));
    }
    let ws = if kind == "Readable" {
        None
    } else {
        let ws = rt.alloc_object(RtObject::new_ordinary());
        let ws_buf = rt.alloc_object(RtObject::new_array());
        {
            let ws_obj = rt.obj_mut(ws);
            ws_obj.set_own("highWaterMark".into(), Value::Number(16384.0));
            ws_obj.set_own("buffer".into(), Value::Object(ws_buf));
            ws_obj.set_own("length".into(), Value::Number(0.0));
            ws_obj.set_own("ended".into(), Value::Boolean(false));
            ws_obj.set_own("finished".into(), Value::Boolean(false));
        }
        Some(ws)
    };

    let option_truthy = |rt: &Runtime, o: rusty_js_runtime::ObjectRef, key: &str| -> bool {
        matches!(rt.object_get(o, key), Value::Boolean(true))
            || matches!(rt.object_get(o, key), Value::Number(n) if n != 0.0)
    };
    let object_mode = opts
        .map(|o| option_truthy(rt, o, "objectMode"))
        .unwrap_or(false);
    let readable_object_mode = opts
        .map(|o| object_mode || option_truthy(rt, o, "readableObjectMode"))
        .unwrap_or(object_mode);
    let writable_object_mode = opts
        .map(|o| object_mode || option_truthy(rt, o, "writableObjectMode"))
        .unwrap_or(object_mode);
    let decode_strings = opts
        .map(|o| !matches!(rt.object_get(o, "decodeStrings"), Value::Boolean(false)))
        .unwrap_or(true);

    let default_hwm = if object_mode { 16.0 } else { 65536.0 };
    let high_water_mark = opts
        .and_then(|o| match rt.object_get(o, "highWaterMark") {
            Value::Number(n) if n.is_finite() && n >= 0.0 => Some(n),
            _ => None,
        })
        .unwrap_or(default_hwm);
    rt.obj_mut(rs)
        .set_own("highWaterMark".into(), Value::Number(high_water_mark));
    rt.obj_mut(rs)
        .set_own("objectMode".into(), Value::Boolean(readable_object_mode));
    if let Some(ws) = ws {
        rt.obj_mut(ws)
            .set_own("highWaterMark".into(), Value::Number(high_water_mark));
        rt.obj_mut(ws)
            .set_own("objectMode".into(), Value::Boolean(writable_object_mode));
        rt.obj_mut(ws)
            .set_own("decodeStrings".into(), Value::Boolean(decode_strings));
    }

    let has_own = |rt: &mut Runtime, key: &str| -> bool {
        let k = Value::String(Rc::new(rusty_js_runtime::value::JsString::from(key)));
        matches!(
            rt.object_get_own_property_descriptor_via(&Value::Object(id), &k),
            Ok(Value::Object(_))
        )
    };
    if fresh_instance || !has_own(rt, "_readableState") {
        if fresh_instance {
            rt.obj_mut(id)
                .set_own("_readableState".into(), Value::Object(rs));
        } else {
            rt.object_set(id, "_readableState".into(), Value::Object(rs));
        }
    }
    if let Some(ws) = ws {
        if fresh_instance || !has_own(rt, "_writableState") {
            if fresh_instance {
                rt.obj_mut(id)
                    .set_own("_writableState".into(), Value::Object(ws));
            } else {
                rt.object_set(id, "_writableState".into(), Value::Object(ws));
            }
        }
    }

    let has_readable = kind != "Writable";
    let has_writable = ws.is_some();
    if has_readable {
        rt.object_set(
            id,
            "readableObjectMode".into(),
            Value::Boolean(readable_object_mode),
        );
        rt.object_set(
            id,
            "readableHighWaterMark".into(),
            Value::Number(high_water_mark),
        );
        rt.object_set(id, "readableLength".into(), Value::Number(0.0));
        rt.object_set(id, "readableEncoding".into(), Value::Null);
        rt.object_set(id, "readableEnded".into(), Value::Boolean(false));
        if matches!(rt.object_get(id, "readableFlowing"), Value::Undefined) {
            rt.object_set(id, "readableFlowing".into(), Value::Null);
        }
        register_method(rt, id, "isPaused", |rt, _a| {

            let flowing = match rt.current_this() {
                Value::Object(t) => rt.object_get(t, "readableFlowing"),
                _ => Value::Null,
            };
            Ok(Value::Boolean(matches!(flowing, Value::Boolean(false))))
        });
    }
    if has_writable {
        rt.object_set(
            id,
            "writableObjectMode".into(),
            Value::Boolean(writable_object_mode),
        );
        rt.object_set(
            id,
            "writableHighWaterMark".into(),
            Value::Number(high_water_mark),
        );
        rt.object_set(id, "writableLength".into(), Value::Number(0.0));
        rt.object_set(id, "writableFinished".into(), Value::Boolean(false));
        rt.object_set(id, "writableEnded".into(), Value::Boolean(false));

        rt.object_set(id, "writableCorked".into(), Value::Number(0.0));
        rt.object_set(id, "writableNeedDrain".into(), Value::Boolean(false));
        rt.object_set(id, "writableAborted".into(), Value::Boolean(false));

        if !rt.is_callable(&rt.object_get(id, "cork")) {
            register_method(rt, id, "cork", |rt, _a| {
                if let Value::Object(t) = rt.current_this() {
                    let n = match rt.object_get(t, "__corked") {
                        Value::Number(n) if n.is_finite() && n >= 0.0 => n,
                        _ => 0.0,
                    };
                    rt.object_set(t, "__corked".into(), Value::Number(n + 1.0));
                    rt.object_set(t, "writableCorked".into(), Value::Number(n + 1.0));
                }
                Ok(rt.current_this())
            });
        }
        if !rt.is_callable(&rt.object_get(id, "uncork")) {
            register_method(rt, id, "uncork", |rt, _a| {
                let this = match rt.current_this() {
                    Value::Object(t) => t,
                    _ => return Ok(rt.current_this()),
                };
                let n = match rt.object_get(this, "__corked") {
                    Value::Number(n) if n > 0.0 => n,
                    _ => 0.0,
                };
                let n = (n - 1.0).max(0.0);
                rt.object_set(this, "__corked".into(), Value::Number(n));
                rt.object_set(this, "writableCorked".into(), Value::Number(n));
                if n > 0.0 {
                    return Ok(Value::Object(this));
                }
                let buf = match rt.object_get(this, "__cork_buffer") {
                    Value::Object(b) => b,
                    _ => return Ok(Value::Object(this)),
                };
                let len = rt.array_length(buf);
                if len == 0 {
                    return Ok(Value::Object(this));
                }

                let fresh = rt.alloc_object(RtObject::new_array());
                rt.object_set(fresh, "length".into(), Value::Number(0.0));
                rt.object_set(this, "__cork_buffer".into(), Value::Object(fresh));
                let cb = make_callable(rt, "stream.uncorkCb", |_rt, _a| Ok(Value::Undefined));
                let writev = rt.object_get(this, "_writev");
                if rt.is_callable(&writev) && !stream_is_default_template(rt, &writev) {
                    let _ = rt.call_function(
                        writev,
                        Value::Object(this),
                        vec![Value::Object(buf), Value::Object(cb)],
                    );
                } else {
                    let write = rt.object_get(this, "_write");
                    for i in 0..len {
                        if let Value::Object(entry) = rt.object_get(buf, &i.to_string()) {
                            let chunk = rt.object_get(entry, "chunk");
                            let enc = rt.object_get(entry, "encoding");
                            if rt.is_callable(&write) {
                                let cb2 = make_callable(rt, "stream.uncorkCb", |_rt, _a| {
                                    Ok(Value::Undefined)
                                });
                                let _ = rt.call_function(
                                    write.clone(),
                                    Value::Object(this),
                                    vec![chunk, enc, Value::Object(cb2)],
                                );
                            }
                        }
                    }
                }
                Ok(Value::Object(this))
            });
        }
    }
    if matches!(rt.object_get(id, "destroyed"), Value::Undefined) {
        rt.object_set(id, "destroyed".into(), Value::Boolean(false));
    }
    if matches!(rt.object_get(id, "closed"), Value::Undefined) {
        rt.object_set(id, "closed".into(), Value::Boolean(false));
    }
    if matches!(rt.object_get(id, "errored"), Value::Undefined) {
        rt.object_set(id, "errored".into(), Value::Null);
    }
    if let Some(o_id) = opts {
        if fresh_instance {
            rt.obj_mut(id)
                .set_own("_options".into(), Value::Object(o_id));
        } else {
            rt.object_set(id, "_options".into(), Value::Object(o_id));
        }

        for (opt, internal) in [
            ("read", "_read"),
            ("write", "_write"),
            ("writev", "_writev"),
            ("transform", "_transform"),
            ("flush", "_flush"),
            ("final", "_final"),
            ("destroy", "_destroy"),
            ("construct", "_construct"),
        ] {
            let f = rt.object_get(o_id, opt);
            if rt.is_callable(&f) {
                rt.object_set(id, internal.into(), f);
            }
        }
    }

    let stream_kind = if kind == "Readable" {
        None
    } else {
        Some(Value::String(Rc::new(
            rusty_js_runtime::value::JsString::from(kind),
        )))
    };
    if fresh_instance {
        let obj = rt.obj_mut(id);
        if let Some(stream_kind) = stream_kind {
            obj.set_own("__stream_kind".into(), stream_kind);
        }
        obj.set_own("__rbuf".into(), Value::Object(rbuf));
        obj.set_own("__flowing".into(), Value::Boolean(false));
        obj.set_own("readableFlowing".into(), Value::Null);
        obj.set_own("__rended".into(), Value::Boolean(initial_rended));
        obj.set_own("readable".into(), Value::Boolean(!initial_rended));
        if kind != "Readable" {
            obj.set_own("writable".into(), Value::Boolean(true));
            obj.set_own("writableEnded".into(), Value::Boolean(false));
        }

        obj.set_own("readableEnded".into(), Value::Boolean(false));
        if let Some(index) = initial_riter_index {
            obj.set_own("__riter_index".into(), Value::Number(index));
        }
        obj.set_own("__endfired".into(), Value::Boolean(false));
    } else {
        if let Some(stream_kind) = stream_kind {
            rt.object_set(id, "__stream_kind".into(), stream_kind);
        }
        rt.object_set(id, "__rbuf".into(), Value::Object(rbuf));
        rt.object_set(id, "__flowing".into(), Value::Boolean(false));
        rt.object_set(id, "readableFlowing".into(), Value::Null);
        rt.object_set(id, "__rended".into(), Value::Boolean(initial_rended));
        rt.object_set(id, "readable".into(), Value::Boolean(!initial_rended));
        if kind != "Readable" {
            rt.object_set(id, "writable".into(), Value::Boolean(true));
            rt.object_set(id, "writableEnded".into(), Value::Boolean(false));
        }
        if let Some(index) = initial_riter_index {
            rt.object_set(id, "__riter_index".into(), Value::Number(index));
        }
        rt.object_set(id, "__endfired".into(), Value::Boolean(false));
        rt.object_set(id, "readableEnded".into(), Value::Boolean(false));
    }

    if !rt.is_callable(&rt.object_get(id, "on")) {
        let on_impl = |rt: &mut Runtime, args: &[Value]| -> Result<Value, RuntimeError> {
            let this = match rt.current_this() {
                Value::Object(t) => t,
                _ => return Ok(Value::Undefined),
            };
            let event = stream_event_name(rt, args.first());
            let listener = args.get(1).cloned().unwrap_or(Value::Undefined);
            if !rt.is_callable(&listener) {
                return Ok(Value::Object(this));
            }
            let reg = match stream_listener_registry(rt, this, true) {
                Some(r) => r,
                _ => return Ok(Value::Object(this)),
            };
            let arr = match rt.object_get(reg, &event) {
                Value::Object(a) => a,
                _ => {
                    let a = rt.alloc_object(RtObject::new_array());
                    rt.object_set(a, "length".into(), Value::Number(0.0));
                    rt.object_set(reg, event.clone(), Value::Object(a));
                    a
                }
            };
            let len = rt.array_length(arr);
            rt.object_set(arr, len.to_string(), listener.clone());
            rt.object_set(arr, "length".into(), Value::Number((len + 1) as f64));

            if event == "data" {
                rt.object_set(this, "__flowing".into(), Value::Boolean(true));
                rt.object_set(this, "readableFlowing".into(), Value::Boolean(true));

                if matches!(rt.object_get(this, "__async_iter"), Value::Object(_)) {
                    if !matches!(
                        rt.object_get(this, "__async_pump_started"),
                        Value::Boolean(true)
                    ) {
                        rt.object_set(this, "__async_pump_started".into(), Value::Boolean(true));
                        async_iter_flow_pump(rt, this);
                    }
                    return Ok(Value::Object(this));
                }
                stream_schedule_read(rt, this);
                if !matches!(
                    rt.object_get(this, "__stream_drain_scheduled"),
                    Value::Boolean(true)
                ) {
                    rt.object_set(
                        this,
                        "__stream_drain_scheduled".into(),
                        Value::Boolean(true),
                    );
                    rt.enqueue_microtask_rooted(
                        "stream data-listener drain",
                        vec![this],
                        move |rt| {
                            rt.object_set(
                                this,
                                "__stream_drain_scheduled".into(),
                                Value::Boolean(false),
                            );
                            stream_drain(rt, this);
                            Ok(())
                        },
                    );
                }
            } else if event == "end"
                && matches!(rt.object_get(this, "__endfired"), Value::Boolean(true))
            {
                let _ = rt.call_function(listener, Value::Object(this), Vec::new());
            } else if event == "readable" {
                let readable_len = readable_state_number(rt, this, "length");
                let readable_ended = match rt.object_get(this, "_readableState") {
                    Value::Object(rs) => {
                        matches!(rt.object_get(rs, "ended"), Value::Boolean(true))
                    }
                    _ => false,
                }

                || matches!(rt.object_get(this, "__rended"), Value::Boolean(true));
                if readable_len > 0.0 || readable_ended {
                    let _ = rt.call_function(listener, Value::Object(this), Vec::new());
                } else {
                    stream_schedule_read(rt, this);
                }
            }
            Ok(Value::Object(this))
        };
        register_method(rt, id, "on", on_impl);
        register_method(rt, id, "once", on_impl);
        register_method(rt, id, "addListener", on_impl);

        let remove_impl = |rt: &mut Runtime, args: &[Value]| -> Result<Value, RuntimeError> {
            let this = match rt.current_this() {
                Value::Object(t) => t,
                _ => return Ok(Value::Undefined),
            };
            let event = stream_event_name(rt, args.first());
            let target = args.get(1).cloned().unwrap_or(Value::Undefined);
            if let Some(reg) = stream_listener_registry(rt, this, false) {
                if let Value::Object(arr) = rt.object_get(reg, &event) {
                    let n = rt.array_length(arr);
                    let mut last_match: Option<usize> = None;
                    for i in 0..n {
                        if rt.object_get(arr, &i.to_string()) == target {
                            last_match = Some(i);
                        }
                    }
                    if let Some(idx) = last_match {
                        let na = rt.alloc_object(RtObject::new_array());
                        let mut k = 0usize;
                        for i in 0..n {
                            if i == idx {
                                continue;
                            }
                            let cb = rt.object_get(arr, &i.to_string());
                            rt.object_set(na, k.to_string(), cb);
                            k += 1;
                        }
                        rt.object_set(na, "length".into(), Value::Number(k as f64));
                        rt.object_set(reg, event, Value::Object(na));
                    }
                }
            }
            Ok(Value::Object(this))
        };
        register_method(rt, id, "off", remove_impl);
        register_method(rt, id, "removeListener", remove_impl);

        register_method(rt, id, "setMaxListeners", |rt, args| {
            let this = match rt.current_this() {
                Value::Object(t) => t,
                _ => return Ok(rt.current_this()),
            };
            let n = args.first().cloned().unwrap_or(Value::Undefined);
            rt.object_set(this, "__max_listeners".into(), n);
            Ok(Value::Object(this))
        });
        register_method(rt, id, "getMaxListeners", |rt, _args| {
            let this = match rt.current_this() {
                Value::Object(t) => t,
                _ => return Ok(Value::Number(10.0)),
            };
            match rt.object_get(this, "__max_listeners") {
                Value::Number(n) => Ok(Value::Number(n)),
                _ => Ok(Value::Number(10.0)),
            }
        });
        register_method(rt, id, "listenerCount", |rt, args| {
            let this = match rt.current_this() {
                Value::Object(t) => t,
                _ => return Ok(Value::Number(0.0)),
            };
            let event = stream_event_name(rt, args.first());
            if let Some(reg) = stream_listener_registry(rt, this, false) {
                if let Value::Object(arr) = rt.object_get(reg, &event) {
                    return Ok(Value::Number(rt.array_length(arr) as f64));
                }
            }
            Ok(Value::Number(0.0))
        });
        let listeners_impl = |rt: &mut Runtime, args: &[Value]| -> Result<Value, RuntimeError> {
            let out = rt.alloc_object(RtObject::new_array());
            let mut n = 0usize;
            if let Value::Object(this) = rt.current_this() {
                let event = stream_event_name(rt, args.first());
                if let Some(reg) = stream_listener_registry(rt, this, false) {
                    if let Value::Object(arr) = rt.object_get(reg, &event) {
                        let len = rt.array_length(arr);
                        for i in 0..len {
                            let cb = rt.object_get(arr, &i.to_string());
                            rt.object_set(out, i.to_string(), cb);
                        }
                        n = len;
                    }
                }
            }
            rt.object_set(out, "length".into(), Value::Number(n as f64));
            Ok(Value::Object(out))
        };
        register_method(rt, id, "listeners", listeners_impl);
        register_method(rt, id, "rawListeners", listeners_impl);
        register_method(rt, id, "eventNames", |rt, _args| {
            let out = rt.alloc_object(RtObject::new_array());
            let mut n = 0usize;
            if let Value::Object(this) = rt.current_this() {
                if let Some(reg) = stream_listener_registry(rt, this, false) {
                    for key in rt.ordinary_own_enumerable_string_keys(reg) {
                        if let Value::Object(arr) = rt.object_get(reg, &key) {
                            if rt.array_length(arr) > 0 {
                                rt.object_set(
                                    out,
                                    n.to_string(),
                                    Value::String(Rc::new(
                                        rusty_js_runtime::value::JsString::from(key.as_str()),
                                    )),
                                );
                                n += 1;
                            }
                        }
                    }
                }
            }
            rt.object_set(out, "length".into(), Value::Number(n as f64));
            Ok(Value::Object(out))
        });
        register_method(rt, id, "removeAllListeners", |rt, args| {
            let this = match rt.current_this() {
                Value::Object(t) => t,
                _ => return Ok(rt.current_this()),
            };
            match args.first() {
                Some(v) if !matches!(v, Value::Undefined) => {
                    let event = stream_event_name(rt, args.first());
                    if let Some(reg) = stream_listener_registry(rt, this, false) {
                        let empty = rt.alloc_object(RtObject::new_array());
                        rt.object_set(empty, "length".into(), Value::Number(0.0));
                        rt.object_set(reg, event, Value::Object(empty));
                    }
                }
                _ => {
                    let fresh = rt.alloc_object(RtObject::new_ordinary());
                    rt.object_set(this, "__stream_listeners".into(), Value::Object(fresh));
                }
            }
            Ok(Value::Object(this))
        });
        let prepend_impl = |rt: &mut Runtime, args: &[Value]| -> Result<Value, RuntimeError> {
            let this = match rt.current_this() {
                Value::Object(t) => t,
                _ => return Ok(Value::Undefined),
            };
            let event = stream_event_name(rt, args.first());
            let listener = args.get(1).cloned().unwrap_or(Value::Undefined);
            if !rt.is_callable(&listener) {
                return Ok(Value::Object(this));
            }
            let reg = match stream_listener_registry(rt, this, true) {
                Some(r) => r,
                _ => return Ok(Value::Object(this)),
            };
            let old = match rt.object_get(reg, &event) {
                Value::Object(a) => Some(a),
                _ => None,
            };
            let na = rt.alloc_object(RtObject::new_array());
            rt.object_set(na, "0".into(), listener.clone());
            let mut n = 1usize;
            if let Some(a) = old {
                let len = rt.array_length(a);
                for i in 0..len {
                    let cb = rt.object_get(a, &i.to_string());
                    rt.object_set(na, (i + 1).to_string(), cb);
                    n += 1;
                }
            }
            rt.object_set(na, "length".into(), Value::Number(n as f64));
            rt.object_set(reg, event.clone(), Value::Object(na));
            if event == "data" {
                rt.object_set(this, "__flowing".into(), Value::Boolean(true));
                rt.object_set(this, "readableFlowing".into(), Value::Boolean(true));
                if matches!(rt.object_get(this, "__async_iter"), Value::Object(_)) {
                    if !matches!(
                        rt.object_get(this, "__async_pump_started"),
                        Value::Boolean(true)
                    ) {
                        rt.object_set(this, "__async_pump_started".into(), Value::Boolean(true));
                        async_iter_flow_pump(rt, this);
                    }
                    return Ok(Value::Object(this));
                }
                stream_schedule_read(rt, this);
                stream_drain(rt, this);
            } else if event == "end"
                && matches!(rt.object_get(this, "__endfired"), Value::Boolean(true))
            {
                let _ = rt.call_function(listener, Value::Object(this), Vec::new());
            }
            Ok(Value::Object(this))
        };
        register_method(rt, id, "prependListener", prepend_impl);
        register_method(rt, id, "prependOnceListener", prepend_impl);
        register_method(rt, id, "emit", |rt, args| {
            let this = match rt.current_this() {
                Value::Object(t) => t,
                _ => return Ok(Value::Boolean(false)),
            };
            let event = stream_event_name(rt, args.first());
            let reg = match stream_listener_registry(rt, this, false) {
                Some(r) => r,
                _ => return Ok(Value::Boolean(false)),
            };
            let arr = match rt.object_get(reg, &event) {
                Value::Object(a) => a,
                _ => return Ok(Value::Boolean(false)),
            };
            let n = rt.array_length(arr);
            let rest: Vec<Value> = args.iter().skip(1).cloned().collect();
            let mut fired = false;
            for i in 0..n {
                let cb = rt.object_get(arr, &i.to_string());
                if rt.is_callable(&cb) {
                    let _ = rt.call_function(cb, Value::Object(this), rest.clone());
                    fired = true;
                }
            }
            Ok(Value::Boolean(fired))
        });

        if !rt.is_callable(&rt.object_get(id, "pipe")) {
            register_method(rt, id, "pipe", |rt, args| {
                let this = match rt.current_this() {
                    Value::Object(t) => t,
                    _ => return Ok(Value::Undefined),
                };
                let dest = match args.first() {
                    Some(Value::Object(d)) => *d,
                    _ => return Ok(rt.current_this()),
                };
                let on = rt.object_get(this, "on");
                if !rt.is_callable(&on) {
                    return Ok(Value::Object(dest));
                }
                let sv =
                    |s: &str| Value::String(Rc::new(rusty_js_runtime::value::JsString::from(s)));

                let on_end = make_callable(rt, "pipe.onEnd", move |rt, _a| {
                    let e = rt.object_get(dest, "end");
                    if rt.is_callable(&e) {
                        let _ = rt.call_function(e, Value::Object(dest), Vec::new());
                    }
                    Ok(Value::Undefined)
                });
                let ev = sv("end");
                let _ = rt.call_function(
                    on.clone(),
                    Value::Object(this),
                    vec![ev, Value::Object(on_end)],
                );
                let on_data = make_callable(rt, "pipe.onData", move |rt, a| {
                    let chunk = a.first().cloned().unwrap_or(Value::Undefined);
                    let w = rt.object_get(dest, "write");
                    if rt.is_callable(&w) {
                        let _ = rt.call_function(w, Value::Object(dest), vec![chunk]);
                    }
                    Ok(Value::Undefined)
                });
                let dv = sv("data");
                let _ = rt.call_function(on, Value::Object(this), vec![dv, Value::Object(on_data)]);

                if let Value::Object(rs) = rt.object_get(this, "_readableState") {
                    if let Value::Object(pipes) = rt.object_get(rs, "pipes") {
                        let push = rt.object_get(pipes, "push");
                        if rt.is_callable(&push) {
                            let _ = rt.call_function(
                                push,
                                Value::Object(pipes),
                                vec![Value::Object(dest)],
                            );
                            let len = rt.object_get(pipes, "length");
                            rt.object_set(rs, "pipesCount".into(), len);
                        }
                    }
                }

                let d_emit = rt.object_get(dest, "emit");
                if rt.is_callable(&d_emit) {
                    let _ = rt.call_function(
                        d_emit,
                        Value::Object(dest),
                        vec![sv("pipe"), Value::Object(this)],
                    );
                }
                Ok(Value::Object(dest))
            });
        }
        register_method(rt, id, "unpipe", |rt, args| {

            if let Value::Object(this) = rt.current_this() {
                if let Value::Object(rs) = rt.object_get(this, "_readableState") {
                    let target = match args.first() {
                        Some(Value::Object(d)) => Some(*d),
                        _ => None,
                    };
                    let new_pipes = rt.alloc_object(RtObject::new_array());
                    let mut removed: Vec<rusty_js_runtime::ObjectRef> = Vec::new();
                    if let Value::Object(pipes) = rt.object_get(rs, "pipes") {
                        let len = match rt.object_get(pipes, "length") {
                            Value::Number(n) => n as usize,
                            _ => 0,
                        };
                        let push = rt.object_get(new_pipes, "push");
                        for i in 0..len {
                            let el = rt.object_get(pipes, &i.to_string());

                            let keep = match (target, &el) {
                                (Some(t), Value::Object(o)) => *o != t,
                                (None, _) => false,
                                _ => true,
                            };
                            if keep {
                                if rt.is_callable(&push) {
                                    let _ = rt.call_function(
                                        push.clone(),
                                        Value::Object(new_pipes),
                                        vec![el],
                                    );
                                }
                            } else if let Value::Object(o) = el {
                                removed.push(o);
                            }
                        }
                    }
                    rt.object_set(rs, "pipes".into(), Value::Object(new_pipes));
                    let len = rt.object_get(new_pipes, "length");
                    rt.object_set(rs, "pipesCount".into(), len);

                    let unpipe_ev =
                        Value::String(Rc::new(rusty_js_runtime::value::JsString::from("unpipe")));
                    for dest in removed {
                        let d_emit = rt.object_get(dest, "emit");
                        if rt.is_callable(&d_emit) {
                            let _ = rt.call_function(
                                d_emit,
                                Value::Object(dest),
                                vec![unpipe_ev.clone(), Value::Object(this)],
                            );
                        }
                    }
                }
            }
            Ok(rt.current_this())
        });

        register_method(rt, id, "write", |rt, args| {
            let this = match rt.current_this() {
                Value::Object(t) => t,
                _ => return Ok(Value::Boolean(true)),
            };
            let chunk = args.first().cloned().unwrap_or(Value::Undefined);
            let opts = rt.object_get(this, "_options");
            let kind = match rt.object_get(this, "__stream_kind") {
                Value::String(s) => s.as_str().to_string(),
                _ => String::new(),
            };
            let enc = Value::String(Rc::new(rusty_js_runtime::value::JsString::from("utf8")));
            let is_transform = kind == "Transform" || kind == "PassThrough";
            let write_encoding = match args.get(1) {
                Some(Value::String(s)) => s.as_str().to_string(),
                _ => "utf8".to_string(),
            };
            let user_cb = if rt.is_callable(args.get(1).unwrap_or(&Value::Undefined)) {
                args.get(1).cloned().unwrap_or(Value::Undefined)
            } else if rt.is_callable(args.get(2).unwrap_or(&Value::Undefined)) {
                args.get(2).cloned().unwrap_or(Value::Undefined)
            } else {
                Value::Undefined
            };
            let writable_object_mode = match rt.object_get(this, "_writableState") {
                Value::Object(ws) => {
                    matches!(rt.object_get(ws, "objectMode"), Value::Boolean(true))
                }
                _ => false,
            };

            if matches!(chunk, Value::Null) {
                return Err(stream_coded_error(
                    rt,
                    "ERR_STREAM_NULL_VALUES",
                    "May not write null values to stream",
                ));
            }
            if !writable_object_mode && !stream_chunk_is_binary(rt, &chunk) {
                let recv = stream_received_desc(rt, &chunk);
                let msg = format!(
                    "The \"chunk\" argument must be of type string or an instance of Buffer, TypedArray, or DataView. Received {recv}"
                );
                return Err(stream_coded_error(rt, "ERR_INVALID_ARG_TYPE", &msg));
            }
            let decode_strings = match rt.object_get(this, "_writableState") {
                Value::Object(ws) => {
                    !matches!(rt.object_get(ws, "decodeStrings"), Value::Boolean(false))
                }
                _ => true,
            };
            let (chunk, enc) = match chunk {
                Value::String(s) if !writable_object_mode && decode_strings => (
                    buffer_from_string(rt, s, &write_encoding),
                    Value::String(Rc::new(rusty_js_runtime::value::JsString::from("buffer"))),
                ),
                other => (other, enc),
            };

            if matches!(rt.object_get(this, "__corked"), Value::Number(n) if n > 0.0) {
                let buf = match rt.object_get(this, "__cork_buffer") {
                    Value::Object(b) => b,
                    _ => {
                        let a = rt.alloc_object(RtObject::new_array());
                        rt.object_set(a, "length".into(), Value::Number(0.0));
                        rt.object_set(this, "__cork_buffer".into(), Value::Object(a));
                        a
                    }
                };
                let entry = new_object(rt);
                rt.object_set(entry, "chunk".into(), chunk);
                rt.object_set(entry, "encoding".into(), enc);
                let blen = rt.array_length(buf);
                rt.object_set(buf, blen.to_string(), Value::Object(entry));
                rt.object_set(buf, "length".into(), Value::Number((blen + 1) as f64));
                if rt.is_callable(&user_cb) {
                    let _ = rt.call_function(user_cb, Value::Object(this), Vec::new());
                }
                return Ok(Value::Boolean(true));
            }

            let chunk_len = if writable_object_mode {
                1.0
            } else {
                match &chunk {
                    Value::Object(cid) => match rt.object_get(*cid, "length") {
                        Value::Number(n) => n,
                        _ => 0.0,
                    },
                    _ => 1.0,
                }
            };
            let write_hwm = match rt.object_get(this, "_writableState") {
                Value::Object(ws) => match rt.object_get(ws, "highWaterMark") {
                    Value::Number(n) => n,
                    _ => 16384.0,
                },
                _ => 16384.0,
            };
            let zero_hwm_passthrough = kind == "PassThrough"
                && matches!(
                    rt.object_get(this, "_writableState"),
                    Value::Object(ws) if matches!(rt.object_get(ws, "highWaterMark"), Value::Number(n) if n == 0.0)
                );

            let f = {
                let from_opts = if let Value::Object(o) = opts {
                    let fname = if is_transform { "transform" } else { "write" };
                    rt.object_get(o, fname)
                } else {
                    Value::Undefined
                };
                if rt.is_callable(&from_opts) {
                    from_opts
                } else {

                    let user_write = rt.object_get(this, "_write");
                    if is_transform
                        && rt.is_callable(&user_write)
                        && !stream_is_default_template(rt, &user_write)
                    {
                        user_write
                    } else {
                        let mname = if is_transform { "_transform" } else { "_write" };
                        rt.object_get(this, mname)
                    }
                }
            };
            if rt.is_callable(&f) && !stream_is_default_template(rt, &f) {
                if is_transform {
                    let pending = match rt.object_get(this, "__pending_transform_cbs") {
                        Value::Number(n) if n.is_finite() && n >= 0.0 => n,
                        _ => 0.0,
                    };
                    rt.object_set(
                        this,
                        "__pending_transform_cbs".into(),
                        Value::Number(pending + 1.0),
                    );
                }

                if !is_transform {
                    let pending = match rt.object_get(this, "__writable_length") {
                        Value::Number(n) if n.is_finite() && n >= 0.0 => n,
                        _ => 0.0,
                    };
                    rt.object_set(
                        this,
                        "__writable_length".into(),
                        Value::Number(pending + chunk_len),
                    );
                }

                let cb = make_callable(rt, "stream.writeCb", move |rt, a| {

                    if !is_transform {
                        let pending = match rt.object_get(this, "__writable_length") {
                            Value::Number(n) if n.is_finite() && n > 0.0 => n,
                            _ => 0.0,
                        };
                        let pending = (pending - chunk_len).max(0.0);
                        rt.object_set(this, "__writable_length".into(), Value::Number(pending));
                        if matches!(
                            rt.object_get(this, "__stream_need_drain"),
                            Value::Boolean(true)
                        ) && pending < write_hwm
                        {
                            rt.object_set(
                                this,
                                "__stream_need_drain".into(),
                                Value::Boolean(false),
                            );
                            stream_emit(rt, this, "drain", Vec::new());
                        }
                    }

                    let err = a.first().cloned().unwrap_or(Value::Undefined);
                    if is_transform {
                        let pending = match rt.object_get(this, "__pending_transform_cbs") {
                            Value::Number(n) if n.is_finite() && n > 0.0 => n - 1.0,
                            _ => 0.0,
                        };
                        rt.object_set(
                            this,
                            "__pending_transform_cbs".into(),
                            Value::Number(pending),
                        );
                    }
                    if !matches!(err, Value::Undefined | Value::Null) {
                        stream_emit(rt, this, "error", vec![err]);
                        return Ok(Value::Undefined);
                    }
                    let out = a.get(1).cloned().unwrap_or(Value::Undefined);
                    if !matches!(out, Value::Undefined | Value::Null) {
                        let push = rt.object_get(this, "push");
                        if rt.is_callable(&push) {
                            let _ = rt.call_function(push, Value::Object(this), vec![out]);
                        }
                    }
                    if is_transform
                        && matches!(
                            rt.object_get(this, "__end_after_pending_transform"),
                            Value::Boolean(true)
                        )
                        && matches!(
                            rt.object_get(this, "__pending_transform_cbs"),
                            Value::Number(n) if n <= 0.0
                        )
                    {
                        rt.object_set(
                            this,
                            "__end_after_pending_transform".into(),
                            Value::Boolean(false),
                        );
                        stream_end_transform_readable(rt, this);
                    }
                    if rt.is_callable(&user_cb) {
                        let _ = rt.call_function(user_cb.clone(), Value::Object(this), Vec::new());
                    }
                    Ok(Value::Undefined)
                });
                let _ =
                    rt.call_function(f, Value::Object(this), vec![chunk, enc, Value::Object(cb)]);
            } else if is_transform {

                let _ = stream_push_value(rt, this, &chunk)?;
                if rt.is_callable(&user_cb) {
                    let _ = rt.call_function(user_cb, Value::Object(this), Vec::new());
                }
            } else if rt.is_callable(&user_cb) {
                let _ = rt.call_function(user_cb, Value::Object(this), Vec::new());
            }
            if zero_hwm_passthrough {
                rt.object_set(this, "__stream_need_drain".into(), Value::Boolean(true));
                Ok(Value::Boolean(false))
            } else if !is_transform {

                let pending = match rt.object_get(this, "__writable_length") {
                    Value::Number(n) if n.is_finite() && n >= 0.0 => n,
                    _ => 0.0,
                };
                let can_continue = pending < write_hwm;
                if !can_continue {
                    rt.object_set(this, "__stream_need_drain".into(), Value::Boolean(true));
                }
                Ok(Value::Boolean(can_continue))
            } else {
                Ok(Value::Boolean(true))
            }
        });
        if !rt.is_callable(&rt.object_get(id, "end")) {
            register_method(rt, id, "end", |rt, args| {
                let this = match rt.current_this() {
                    Value::Object(t) => t,
                    _ => return Ok(rt.current_this()),
                };
                let kind = match rt.object_get(this, "__stream_kind") {
                    Value::String(s) => s.as_str().to_string(),
                    _ => String::new(),
                };
                let end_cb = if rt.is_callable(args.first().unwrap_or(&Value::Undefined)) {
                    args.first().cloned().unwrap_or(Value::Undefined)
                } else if rt.is_callable(args.get(1).unwrap_or(&Value::Undefined)) {
                    args.get(1).cloned().unwrap_or(Value::Undefined)
                } else if rt.is_callable(args.get(2).unwrap_or(&Value::Undefined)) {
                    args.get(2).cloned().unwrap_or(Value::Undefined)
                } else {
                    Value::Undefined
                };

                if let Some(chunk) = args.first() {
                    if !matches!(chunk, Value::Undefined | Value::Null) && !rt.is_callable(chunk) {
                        if kind == "PassThrough" && !stream_has_callable_transform_impl(rt, this) {
                            let _ = stream_push_value(rt, this, chunk)?;
                        } else {
                            let w = rt.object_get(this, "write");
                            if rt.is_callable(&w) {
                                let _ =
                                    rt.call_function(w, Value::Object(this), vec![chunk.clone()]);
                            }
                        }
                    }
                }
                rt.object_set(this, "writable".into(), Value::Boolean(false));
                rt.object_set(this, "writableEnded".into(), Value::Boolean(true));
                if let Value::Object(ws) = rt.object_get(this, "_writableState") {
                    rt.object_set(ws, "ended".into(), Value::Boolean(true));
                }

                if kind == "Transform" || kind == "PassThrough" {
                    let pending = match rt.object_get(this, "__pending_transform_cbs") {
                        Value::Number(n) if n.is_finite() && n > 0.0 => n,
                        _ => 0.0,
                    };
                    if pending > 0.0 {
                        rt.object_set(
                            this,
                            "__end_after_pending_transform".into(),
                            Value::Boolean(true),
                        );
                    } else {
                        stream_end_transform_readable(rt, this);
                    }
                }
                let finish_cb = make_callable(rt, "stream.finalCb", move |rt, a| {
                    let err = a.first().cloned().unwrap_or(Value::Undefined);
                    if !matches!(err, Value::Undefined | Value::Null) {
                        stream_emit(rt, this, "error", vec![err]);
                        return Ok(Value::Undefined);
                    }
                    if !matches!(rt.object_get(this, "__wfinishfired"), Value::Boolean(true)) {
                        rt.object_set(this, "__wfinishfired".into(), Value::Boolean(true));
                        if let Value::Object(ws) = rt.object_get(this, "_writableState") {
                            rt.object_set(ws, "finished".into(), Value::Boolean(true));
                        }

                        let end_cb = end_cb.clone();
                        let mut fin_roots = vec![this];
                        if let Value::Object(cbid) = &end_cb {
                            fin_roots.push(*cbid);
                        }
                        rt.enqueue_host_phase_rooted(
                            HostEnqueuePhase::HostCompletionMacrotask,
                            "stream.finish",
                            fin_roots,
                            move |rt| {
                                if rt.is_callable(&end_cb) {
                                    let _ = rt.call_function(
                                        end_cb.clone(),
                                        Value::Object(this),
                                        Vec::new(),
                                    );
                                }
                                stream_emit(rt, this, "finish", Vec::new());
                                stream_emit(rt, this, "close", Vec::new());
                                Ok(())
                            },
                        );
                    }
                    Ok(Value::Undefined)
                });
                let opts_final = match rt.object_get(this, "_options") {
                    Value::Object(o) => rt.object_get(o, "final"),
                    _ => Value::Undefined,
                };
                let finalizer = if rt.is_callable(&opts_final) {
                    opts_final
                } else {
                    rt.object_get(this, "_final")
                };
                if rt.is_callable(&finalizer) && !stream_is_default_template(rt, &finalizer) {
                    let _ = rt.call_function(
                        finalizer,
                        Value::Object(this),
                        vec![Value::Object(finish_cb)],
                    );
                } else {
                    let _ =
                        rt.call_function(Value::Object(finish_cb), Value::Object(this), Vec::new());
                }
                Ok(Value::Object(this))
            });
        }
        register_method(rt, id, "destroy", |rt, args| {
            let this = match rt.current_this() {
                Value::Object(t) => t,
                _ => return Ok(rt.current_this()),
            };

            if matches!(rt.object_get(this, "destroyed"), Value::Boolean(true)) {
                return Ok(Value::Object(this));
            }

            rt.object_set(this, "destroyed".into(), Value::Boolean(true));
            let err = args
                .first()
                .cloned()
                .filter(|e| !matches!(e, Value::Undefined | Value::Null));
            if let Some(e) = &err {
                rt.object_set(this, "errored".into(), e.clone());
            }
            rt.enqueue_host_phase_rooted(
                HostEnqueuePhase::HostCompletionMacrotask,
                "stream.destroy",
                vec![this],
                move |rt| {
                    if let Some(e) = &err {
                        stream_emit(rt, this, "error", vec![e.clone()]);
                    }
                    rt.object_set(this, "closed".into(), Value::Boolean(true));
                    stream_emit(rt, this, "close", Vec::new());
                    Ok(())
                },
            );
            Ok(Value::Object(this))
        });
        register_method(rt, id, "pause", |rt, _args| {
            if let Value::Object(t) = rt.current_this() {
                rt.object_set(t, "__flowing".into(), Value::Boolean(false));
                rt.object_set(t, "readableFlowing".into(), Value::Boolean(false));
            }
            Ok(rt.current_this())
        });
        register_method(rt, id, "resume", |rt, _args| {
            if let Value::Object(t) = rt.current_this() {
                rt.object_set(t, "__flowing".into(), Value::Boolean(true));
                rt.object_set(t, "readableFlowing".into(), Value::Boolean(true));
                stream_schedule_read(rt, t);
                stream_drain(rt, t);
            }
            Ok(rt.current_this())
        });
        register_method(rt, id, "read", |rt, args| {
            let this = match rt.current_this() {
                Value::Object(t) => t,
                _ => return Ok(Value::Null),
            };
            let buf = match rt.object_get(this, "__rbuf") {
                Value::Object(b) => b,
                _ => return Ok(Value::Null),
            };
            let len = rt.array_length(buf);
            let source_backed =
                matches!(rt.object_get(this, "__rbuf_source"), Value::Boolean(true));
            let head = if source_backed {
                match rt.object_get(this, "__riter_index") {
                    Value::Number(n) if n.is_finite() && n >= 0.0 => n as usize,
                    _ => 0,
                }
            } else {
                stream_buffer_head(rt, buf)
            };
            if len == 0 {

                stream_finish_readable_if_drained(rt, this);
                return Ok(Value::Null);
            }
            if head >= len {
                stream_finish_readable_if_drained(rt, this);
                return Ok(Value::Null);
            }

            if let Some(Value::Number(nf)) = args.first() {
                if nf.is_finite()
                    && *nf >= 0.0
                    && !source_backed
                    && !crate::stream::readable_object_mode(rt, this)
                {
                    let want = *nf as usize;
                    let buffered = readable_state_number(rt, this, "length").max(0.0) as usize;
                    let ended = matches!(rt.object_get(this, "__rended"), Value::Boolean(true));

                    if want == 0 || (want > buffered && !ended) {
                        if want > buffered {
                            stream_schedule_read(rt, this);
                        }
                        return Ok(Value::Null);
                    }

                    let take = want.min(buffered);
                    if take == 0 {
                        return Ok(Value::Null);
                    }
                    let is_string =
                        matches!(rt.object_get(buf, &head.to_string()), Value::String(_));
                    let mut out: Vec<u8> = Vec::with_capacity(take);
                    let mut idx = head;
                    while out.len() < take && idx < len {
                        let ch = rt.object_get(buf, &idx.to_string());
                        let cb = consume_chunk_bytes(rt, &ch);
                        let need = take - out.len();
                        if cb.len() <= need {
                            out.extend_from_slice(&cb);
                            idx += 1;
                        } else {
                            out.extend_from_slice(&cb[..need]);

                            let rem = &cb[need..];
                            let rem_val = if is_string {
                                Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                                    String::from_utf8_lossy(rem).into_owned(),
                                )))
                            } else {
                                crate::net::net_buffer_from_bytes(rt, rem)
                            };
                            rt.object_set(buf, idx.to_string(), rem_val);
                            break;
                        }
                    }
                    stream_buffer_set_head(rt, buf, idx);
                    let next_length = (buffered as f64 - out.len() as f64).max(0.0);
                    readable_state_set(rt, this, "length", Value::Number(next_length));
                    let high_water_mark = readable_state_number(rt, this, "highWaterMark");
                    if next_length < high_water_mark {
                        stream_schedule_read(rt, this);
                    }
                    stream_finish_readable_if_drained(rt, this);
                    let result = if is_string {
                        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                            String::from_utf8_lossy(&out).into_owned(),
                        )))
                    } else {
                        crate::net::net_buffer_from_bytes(rt, &out)
                    };
                    return Ok(result);
                }
            }
            let chunk = rt.object_get(buf, &head.to_string());
            if source_backed {
                readable_from_set_index(rt, this, head + 1);
            } else {
                stream_buffer_set_head(rt, buf, head + 1);
            }
            let next_length = (readable_state_number(rt, this, "length")
                - readable_chunk_units(rt, this, &chunk))
            .max(0.0);
            readable_state_set(rt, this, "length", Value::Number(next_length));
            let high_water_mark = readable_state_number(rt, this, "highWaterMark");
            if next_length < high_water_mark {
                stream_schedule_read(rt, this);
            }
            if next_length == 0.0
                && matches!(
                    rt.object_get(this, "__stream_need_drain"),
                    Value::Boolean(true)
                )
            {
                rt.object_set(this, "__stream_need_drain".into(), Value::Boolean(false));
                stream_emit(rt, this, "drain", Vec::new());
            }
            stream_finish_readable_if_drained(rt, this);
            Ok(chunk)
        });
        register_method(rt, id, "unshift", |rt, args| {
            let this = match rt.current_this() {
                Value::Object(t) => t,
                _ => return Ok(Value::Boolean(false)),
            };
            let Some(chunk) = args.first() else {
                return Ok(Value::Boolean(false));
            };
            if matches!(chunk, Value::Null) {
                return Ok(Value::Boolean(false));
            }
            let mut buf = match rt.object_get(this, "__rbuf") {
                Value::Object(b) => b,
                _ => return Ok(Value::Boolean(false)),
            };
            if matches!(rt.object_get(this, "__rbuf_source"), Value::Boolean(true)) {
                buf = materialize_source_backed_rbuf(rt, this, buf);
            }
            let len = rt.array_length(buf);
            let head = stream_buffer_head(rt, buf);
            if head > 0 {
                rt.object_set(buf, (head - 1).to_string(), chunk.clone());
                stream_buffer_set_head(rt, buf, head - 1);
            } else {
                for i in (0..len).rev() {
                    let v = rt.object_get(buf, &i.to_string());
                    rt.object_set(buf, (i + 1).to_string(), v);
                }
                rt.object_set(buf, "0".into(), chunk.clone());
                rt.object_set(buf, "length".into(), Value::Number((len + 1) as f64));
            }
            let next_length =
                readable_state_number(rt, this, "length") + readable_chunk_units(rt, this, chunk);
            readable_state_set(rt, this, "length", Value::Number(next_length));
            stream_emit(rt, this, "readable", Vec::new());
            Ok(Value::Boolean(true))
        });

        register_method(rt, id, "push", |rt, args| {
            let this = match rt.current_this() {
                Value::Object(t) => t,
                _ => return Ok(Value::Boolean(false)),
            };
            match args.first() {
                None | Some(Value::Null) => {
                    rt.object_set(this, "__rended".into(), Value::Boolean(true));
                    rt.object_set(this, "readable".into(), Value::Boolean(false));
                    readable_state_set(rt, this, "ended", Value::Boolean(true));
                    readable_state_set(rt, this, "reading", Value::Boolean(false));
                    stream_emit(rt, this, "readable", Vec::new());
                    stream_finish_readable_if_drained(rt, this);
                    stream_drain(rt, this);
                    Ok(Value::Boolean(false))
                }
                Some(chunk) => stream_push_value(rt, this, chunk).map(Value::Boolean),
            }
        });
        register_method(rt, id, "setEncoding", |rt, args| {
            if let Value::Object(t) = rt.current_this() {
                if let Some(e @ Value::String(_)) = args.first() {
                    rt.object_set(t, "__rs_encoding".into(), e.clone());
                }
            }
            Ok(rt.current_this())
        });
        register_method(rt, id, "toArray", |rt, _args| {
            use rusty_js_runtime::promise::{new_promise, reject_promise, resolve_promise};
            let stream = match rt.current_this() {
                Value::Object(t) => t,
                _ => return Ok(Value::Undefined),
            };
            let promise = new_promise(rt);
            let chunks = rt.alloc_object(RtObject::new_array());
            rt.object_set(chunks, "length".into(), Value::Number(0.0));
            let on_data = make_callable(rt, "stream.toArray.data", move |rt, a| {
                let chunk = a.first().cloned().unwrap_or(Value::Undefined);
                let len = rt.array_length(chunks);
                rt.object_set(chunks, len.to_string(), chunk);
                rt.object_set(chunks, "length".into(), Value::Number((len + 1) as f64));
                Ok(Value::Undefined)
            });
            let on_end = make_callable(rt, "stream.toArray.end", move |rt, _a| {
                resolve_promise(rt, promise, Value::Object(chunks));
                Ok(Value::Undefined)
            });
            let on_error = make_callable(rt, "stream.toArray.error", move |rt, a| {
                let error = a.first().cloned().unwrap_or(Value::Undefined);
                reject_promise(rt, promise, error);
                Ok(Value::Undefined)
            });
            let sv = |s: &str| Value::String(Rc::new(rusty_js_runtime::value::JsString::from(s)));
            let on = rt.object_get(stream, "on");
            if rt.is_callable(&on) {
                rt.call_function(
                    on.clone(),
                    Value::Object(stream),
                    vec![sv("error"), Value::Object(on_error)],
                )?;
                rt.call_function(
                    on.clone(),
                    Value::Object(stream),
                    vec![sv("end"), Value::Object(on_end)],
                )?;
                rt.call_function(
                    on,
                    Value::Object(stream),
                    vec![sv("data"), Value::Object(on_data)],
                )?;
            }
            rt.object_set(stream, "__flowing".into(), Value::Boolean(true));
            rt.object_set(stream, "readableFlowing".into(), Value::Boolean(true));
            stream_schedule_read(rt, stream);
            stream_drain(rt, stream);
            Ok(Value::Object(promise))
        });

        install_async_iterator(rt, id);
    }
    id
}

pub(crate) fn install_async_iterator(rt: &mut Runtime, id: rusty_js_runtime::ObjectRef) {
    {
        register_method(rt, id, "@@asyncIterator", |rt, _a| {
            use rusty_js_runtime::promise::{new_promise, reject_promise, resolve_promise};
            let stream = match rt.current_this() {
                Value::Object(t) => t,
                _ => return Ok(Value::Undefined),
            };

            if let Value::Object(_) = rt.object_get(stream, "__async_iter") {
                return Ok(Value::Object(stream));
            }
            let iter = new_object(rt);
            static AI_SEQ: AtomicU64 = AtomicU64::new(0);
            let root_key = format!(
                "stream.async_iterator.active.{}",
                AI_SEQ.fetch_add(1, Ordering::Relaxed)
            );
            rt.retain_host_roots(root_key.clone(), vec![Value::Object(iter)]);
            rt.set_engine_sentinel(
                iter,
                "__ai_root_key",
                Value::String(Rc::new(rusty_js_runtime::value::JsString::from(root_key))),
            );
            let queue = rt.alloc_object(rusty_js_runtime::Object::new_array());
            let waiters = rt.alloc_object(rusty_js_runtime::Object::new_array());
            rt.set_engine_sentinel(iter, "__ai_queue", Value::Object(queue));
            rt.set_engine_sentinel(iter, "__ai_waiters", Value::Object(waiters));
            rt.set_engine_sentinel(iter, "__ai_done", Value::Boolean(false));
            rt.set_engine_sentinel(iter, "__ai_haserr", Value::Boolean(false));
            rt.set_engine_sentinel(iter, "__ai_err", Value::Undefined);
            let sv = |s: &str| {
                Value::String(std::rc::Rc::new(rusty_js_runtime::value::JsString::from(s)))
            };
            let on_data = make_callable(rt, "stream.ai.data", move |rt, a| {
                let chunk = a.first().cloned().unwrap_or(Value::Undefined);
                if let Some(w) = ai_sentinel(rt, iter, "__ai_waiters") {
                    if ai_len(rt, w) > 0 {
                        if let Value::Object(p) = ai_shift(rt, w) {
                            let res = ai_result(rt, chunk, false);
                            resolve_promise(rt, p, res);
                            return Ok(Value::Undefined);
                        }
                    }
                }
                if let Some(q) = ai_sentinel(rt, iter, "__ai_queue") {
                    ai_push(rt, q, chunk);
                }
                Ok(Value::Undefined)
            });
            let on_end = make_callable(rt, "stream.ai.end", move |rt, _a| {
                rt.set_engine_sentinel(iter, "__ai_done", Value::Boolean(true));
                if let Some(w) = ai_sentinel(rt, iter, "__ai_waiters") {
                    while ai_len(rt, w) > 0 {
                        if let Value::Object(p) = ai_shift(rt, w) {
                            let res = ai_result(rt, Value::Undefined, true);
                            resolve_promise(rt, p, res);
                        } else {
                            break;
                        }
                    }
                }
                if let Value::String(k) = rt.object_get(iter, "__ai_root_key") {
                    rt.release_host_roots(&k.as_str().to_string());
                }

                let destroy = rt.object_get(stream, "destroy");
                if rt.is_callable(&destroy) {
                    let _ = rt.call_function(destroy, Value::Object(stream), Vec::new());
                }
                Ok(Value::Undefined)
            });
            let on_err = make_callable(rt, "stream.ai.err", move |rt, a| {
                let reason = a.first().cloned().unwrap_or(Value::Undefined);
                rt.set_engine_sentinel(iter, "__ai_done", Value::Boolean(true));
                if let Some(w) = ai_sentinel(rt, iter, "__ai_waiters") {
                    if ai_len(rt, w) > 0 {
                        if let Value::Object(p) = ai_shift(rt, w) {
                            reject_promise(rt, p, reason);
                            return Ok(Value::Undefined);
                        }
                    }
                }
                rt.set_engine_sentinel(iter, "__ai_haserr", Value::Boolean(true));
                rt.set_engine_sentinel(iter, "__ai_err", reason);
                if let Value::String(k) = rt.object_get(iter, "__ai_root_key") {
                    rt.release_host_roots(&k.as_str().to_string());
                }
                Ok(Value::Undefined)
            });
            let on = rt.object_get(stream, "on");
            if rt.is_callable(&on) {

                let _ = rt.call_function(
                    on.clone(),
                    Value::Object(stream),
                    vec![sv("end"), Value::Object(on_end)],
                );
                let _ = rt.call_function(
                    on.clone(),
                    Value::Object(stream),
                    vec![sv("error"), Value::Object(on_err)],
                );
                let _ = rt.call_function(
                    on,
                    Value::Object(stream),
                    vec![sv("data"), Value::Object(on_data)],
                );
            }
            register_method(rt, iter, "next", |rt, _a| {
                use rusty_js_runtime::promise::{new_promise, reject_promise, resolve_promise};
                let it = match rt.current_this() {
                    Value::Object(t) => t,
                    _ => return Ok(Value::Undefined),
                };
                if let Some(q) = ai_sentinel(rt, it, "__ai_queue") {
                    if ai_len(rt, q) > 0 {
                        let chunk = ai_shift(rt, q);
                        let p = new_promise(rt);
                        let res = ai_result(rt, chunk, false);
                        resolve_promise(rt, p, res);
                        return Ok(Value::Object(p));
                    }
                }
                if matches!(rt.object_get(it, "__ai_haserr"), Value::Boolean(true)) {
                    let reason = rt.object_get(it, "__ai_err");
                    rt.set_engine_sentinel(it, "__ai_haserr", Value::Boolean(false));
                    let p = new_promise(rt);
                    reject_promise(rt, p, reason);
                    return Ok(Value::Object(p));
                }
                if matches!(rt.object_get(it, "__ai_done"), Value::Boolean(true)) {
                    let p = new_promise(rt);
                    let res = ai_result(rt, Value::Undefined, true);
                    resolve_promise(rt, p, res);
                    return Ok(Value::Object(p));
                }
                let p = new_promise(rt);
                if let Some(w) = ai_sentinel(rt, it, "__ai_waiters") {
                    ai_push(rt, w, Value::Object(p));
                }
                Ok(Value::Object(p))
            });
            register_method(rt, iter, "return", move |rt, a| {
                if let Value::Object(this) = rt.current_this() {
                    rt.set_engine_sentinel(this, "__ai_done", Value::Boolean(true));
                    if let Value::String(k) = rt.object_get(this, "__ai_root_key") {
                        rt.release_host_roots(&k.as_str().to_string());
                    }
                }

                let destroy = rt.object_get(stream, "destroy");
                if rt.is_callable(&destroy) {
                    let _ = rt.call_function(destroy, Value::Object(stream), Vec::new());
                }
                let p = new_promise(rt);

                let ret_val = a.first().cloned().unwrap_or(Value::Undefined);
                let res = ai_result(rt, ret_val, true);
                resolve_promise(rt, p, res);
                Ok(Value::Object(p))
            });
            register_method(rt, iter, "@@asyncIterator", |rt, _a| Ok(rt.current_this()));
            let _ = new_promise;
            let _ = reject_promise;
            let _ = resolve_promise;
            Ok(Value::Object(iter))
        });
        if let Value::Object(method) = rt.object_get(id, "@@asyncIterator") {
            rt.obj_mut(id).dict_mut().insert(
                PropertyKey::String("@@asyncIterator".into()),
                PropertyDescriptor {
                    value: Value::Object(method),
                    writable: true,
                    enumerable: false,
                    configurable: true,
                    getter: None,
                    setter: None,
                },
            );
        }
    }
}

fn make_readable_from_sync_instance(
    rt: &mut Runtime,
    prototype: Option<rusty_js_runtime::ObjectRef>,
    rbuf: rusty_js_runtime::ObjectRef,
    source_backed: bool,
) -> rusty_js_runtime::ObjectRef {
    let id = rt.alloc_object(RtObject::new_ordinary());
    if let Some(proto) = prototype {
        rt.set_object_prototype_internal(id, Some(proto));
    }

    let initial_len = rt.array_length(rbuf) as f64;
    let rs_pipes = rt.alloc_object(RtObject::new_array());
    let rs = rt.alloc_object(RtObject::new_ordinary());
    {
        let rs_obj = rt.obj_mut(rs);
        rs_obj.set_own("highWaterMark".into(), Value::Number(16384.0));
        rs_obj.set_own("buffer".into(), Value::Object(rbuf));
        rs_obj.set_own("length".into(), Value::Number(initial_len));
        rs_obj.set_own("ended".into(), Value::Boolean(false));
        rs_obj.set_own("objectMode".into(), Value::Boolean(true));
        rs_obj.set_own("pipes".into(), Value::Object(rs_pipes));
        rs_obj.set_own("pipesCount".into(), Value::Number(0.0));
    }

    let obj = rt.obj_mut(id);
    obj.set_own("_readableState".into(), Value::Object(rs));
    obj.set_own("__rbuf".into(), Value::Object(rbuf));
    if source_backed {
        obj.set_own("__rbuf_source".into(), Value::Boolean(true));
    }
    obj.set_own("__flowing".into(), Value::Boolean(false));
    obj.set_own("readableFlowing".into(), Value::Null);
    obj.set_own("__rended".into(), Value::Boolean(true));
    obj.set_own("__riter_index".into(), Value::Number(0.0));
    obj.set_own("__endfired".into(), Value::Boolean(false));
    id
}

fn mark_readable_from_object_mode(rt: &mut Runtime, stream_id: rusty_js_runtime::ObjectRef) {
    if let Value::Object(rs) = rt.object_get(stream_id, "_readableState") {
        rt.object_set(rs, "objectMode".into(), Value::Boolean(true));
    }
}

fn stream_iter_result(rt: &mut Runtime, value: Value, done: bool) -> Value {
    let result = new_object(rt);
    rt.object_set(result, "value".into(), value);
    rt.object_set(result, "done".into(), Value::Boolean(done));
    let p = rusty_js_runtime::promise::new_promise(rt);
    rusty_js_runtime::promise::resolve_promise(rt, p, Value::Object(result));
    Value::Object(p)
}

fn ai_len(rt: &mut Runtime, arr: rusty_js_runtime::value::ObjectRef) -> usize {
    match rt.object_get(arr, "length") {
        Value::Number(n) if n > 0.0 => n as usize,
        _ => 0,
    }
}
fn ai_push(rt: &mut Runtime, arr: rusty_js_runtime::value::ObjectRef, v: Value) {
    let push = rt.object_get(arr, "push");
    if rt.is_callable(&push) {
        let _ = rt.call_function(push, Value::Object(arr), vec![v]);
    }
}
fn ai_shift(rt: &mut Runtime, arr: rusty_js_runtime::value::ObjectRef) -> Value {
    let sh = rt.object_get(arr, "shift");
    if rt.is_callable(&sh) {
        rt.call_function(sh, Value::Object(arr), vec![])
            .unwrap_or(Value::Undefined)
    } else {
        Value::Undefined
    }
}
fn ai_result(rt: &mut Runtime, value: Value, done: bool) -> Value {
    let r = new_object(rt);
    rt.object_set(r, "value".into(), value);
    rt.object_set(r, "done".into(), Value::Boolean(done));
    Value::Object(r)
}
fn ai_sentinel(
    rt: &mut Runtime,
    iter: rusty_js_runtime::value::ObjectRef,
    key: &str,
) -> Option<rusty_js_runtime::value::ObjectRef> {
    match rt.object_get(iter, key) {
        Value::Object(o) => Some(o),
        _ => None,
    }
}

fn collect_sync_values(rt: &mut Runtime, source: Value) -> Result<Vec<Value>, RuntimeError> {
    if let Value::Object(id) = source.clone() {
        let iter_method = rt.get_method(&Value::Object(id), "@@iterator")?;
        if !rt.is_callable(&iter_method) {
            return Err(RuntimeError::TypeError(
                "Readable.from source is not iterable".into(),
            ));
        }

        let iter = rt.call_function(iter_method, Value::Object(id), Vec::new())?;
        let iter_id = match iter {
            Value::Object(iter_id) => iter_id,
            _ => return Ok(Vec::new()),
        };
        let next = rt.object_get(iter_id, "next");
        if !rt.is_callable(&next) {
            return Ok(Vec::new());
        }
        let mut out = Vec::new();
        for _ in 0..10_000 {
            let step = rt.call_function(next.clone(), Value::Object(iter_id), Vec::new())?;
            let step_id = match step {
                Value::Object(step_id) => step_id,
                _ => break,
            };
            if matches!(rt.object_get(step_id, "done"), Value::Boolean(true)) {
                break;
            }
            out.push(rt.object_get(step_id, "value"));
        }
        return Ok(out);
    }

    Err(RuntimeError::TypeError(
        "Readable.from source is not iterable".into(),
    ))
}

fn pristine_array_sync_source(
    rt: &mut Runtime,
    id: rusty_js_runtime::ObjectRef,
) -> Option<(rusty_js_runtime::ObjectRef, usize)> {
    if !matches!(rt.obj(id).internal_kind, InternalKind::Array) {
        return None;
    }
    if rt.obj(id).has_own_str("@@iterator") {
        return None;
    }
    let iterator_symbol = match rt.global_get("Symbol") {
        Value::Object(symbol_ctor) => match rt.object_get(symbol_ctor, "iterator") {
            Value::Symbol(sym) => Some(sym),
            _ => None,
        },
        _ => None,
    };
    if let Some(sym) = iterator_symbol {
        if rt
            .obj(id)
            .properties
            .contains_key(&PropertyKey::Symbol(sym.clone()))
        {
            return None;
        }
    }
    let array_proto = rt.array_prototype?;
    let proto_iter = rt.object_get(array_proto, "@@iterator");
    if !matches!(
        (proto_iter, rt.intrinsic_array_iterator_method_id),
        (Value::Object(actual), Some(expected)) if actual == expected
    ) {
        return None;
    }
    if let Value::Number(len) = rt.object_get(id, "length") {
        if len.is_finite() && len >= 0.0 {
            return Some((id, len as usize));
        }
    }
    None
}

fn indexed_sync_source(
    rt: &mut Runtime,
    source: &Value,
) -> Result<Option<(rusty_js_runtime::ObjectRef, usize)>, RuntimeError> {
    if let Value::Object(id) = source {
        if let Some(source) = pristine_array_sync_source(rt, *id) {
            return Ok(Some(source));
        }
        let iter_method = rt.get_method(&Value::Object(*id), "@@iterator")?;
        if !rt.is_callable(&iter_method) {
            return Err(RuntimeError::TypeError(
                "Readable.from source is not iterable".into(),
            ));
        }
        if !matches!(
            (iter_method, rt.intrinsic_array_iterator_method_id),
            (Value::Object(actual), Some(expected)) if actual == expected
        ) {
            return Ok(None);
        }
        if let Value::Number(len) = rt.object_get(*id, "length") {
            if len.is_finite() && len >= 0.0 {
                return Ok(Some((*id, len as usize)));
            }
        }
    }
    Ok(None)
}

fn make_readable_from(
    rt: &mut Runtime,
    source: Value,
    readable_proto: Option<rusty_js_runtime::ObjectRef>,
) -> Result<Value, RuntimeError> {

    if let Value::Object(id) = source.clone() {
        let async_iter_method = rt.object_get(id, "@@asyncIterator");
        if rt.is_callable(&async_iter_method) {
            let iter = rt.call_function(async_iter_method, source.clone(), Vec::new())?;
            if let Value::Object(iter_id) = iter {
                let stream_id = make_stream_instance(
                    rt,
                    None,
                    None,
                    "Readable",
                    readable_proto,
                    None,
                    false,
                    None,
                );
                mark_readable_from_object_mode(rt, stream_id);
                rt.object_set(stream_id, "__async_iter".into(), Value::Object(iter_id));
                return Ok(Value::Object(stream_id));
            }
        }
    }
    if let Some((source_id, _len)) = indexed_sync_source(rt, &source)? {
        let stream_id = make_readable_from_sync_instance(rt, readable_proto, source_id, true);
        return Ok(Value::Object(stream_id));
    }
    if matches!(source, Value::String(_)) {
        let rbuf = make_stream_buffer_from_chunks(rt, vec![source]);
        let stream_id = make_readable_from_sync_instance(rt, readable_proto, rbuf, false);
        return Ok(Value::Object(stream_id));
    }
    let chunks = collect_sync_values(rt, source)?;

    let rbuf = make_stream_buffer_from_chunks(rt, chunks);
    let stream_id = make_readable_from_sync_instance(rt, readable_proto, rbuf, false);
    Ok(Value::Object(stream_id))
}

const STREAM_ITERATOR_HELPERS_JS: &str = r#"
(() => {
  const Readable = globalThis.__cruft_Readable;
  if (!Readable || !Readable.prototype) return;
  const P = Readable.prototype;
  if (typeof P.map === 'function') return;
  const src = (s) => s;
  // node always passes the callback a 2nd options arg carrying a `signal`
  // AbortSignal (created per helper CALL, reused across elements).
  const mkOpts = (opts) => {
    if (opts && opts.signal) return opts;
    const o = Object.assign({}, opts);
    o.signal = new AbortController().signal;
    return o;
  };
  P.map = function (fn, opts) {
    const self = this; const o = mkOpts(opts);
    return Readable.from((async function* () {
      for await (const x of src(self)) yield await fn(x, o);
    })());
  };
  P.filter = function (fn, opts) {
    const self = this; const o = mkOpts(opts);
    return Readable.from((async function* () {
      for await (const x of src(self)) if (await fn(x, o)) yield x;
    })());
  };
  P.take = function (n) {
    const self = this;
    return Readable.from((async function* () {
      if (n <= 0) return; let c = 0;
      for await (const x of src(self)) { yield x; if (++c >= n) return; }
    })());
  };
  P.drop = function (n) {
    const self = this;
    return Readable.from((async function* () {
      let c = 0;
      for await (const x of src(self)) { if (c++ < n) continue; yield x; }
    })());
  };
  P.flatMap = function (fn, opts) {
    const self = this; const o = mkOpts(opts);
    return Readable.from((async function* () {
      for await (const x of src(self)) {
        const r = await fn(x, o);
        if (r != null && (typeof r[Symbol.asyncIterator] === 'function' ||
                          typeof r[Symbol.iterator] === 'function')) {
          yield* r;
        } else {
          yield r;
        }
      }
    })());
  };
  P.reduce = async function (fn, initial) {
    let acc = initial; let hasAcc = arguments.length >= 2; const o = {};
    for await (const x of src(this)) {
      if (!hasAcc) { acc = x; hasAcc = true; continue; }
      acc = await fn(acc, x, o);
    }
    if (!hasAcc) {
      const e = new TypeError('Reduce of an empty stream requires an initial value');
      e.code = 'ERR_MISSING_ARGS';
      throw e;
    }
    return acc;
  };
  P.forEach = async function (fn, opts) {
    const o = mkOpts(opts);
    for await (const x of src(this)) await fn(x, o);
  };
  P.some = async function (fn, opts) {
    const o = mkOpts(opts);
    for await (const x of src(this)) if (await fn(x, o)) return true;
    return false;
  };
  P.every = async function (fn, opts) {
    const o = mkOpts(opts);
    for await (const x of src(this)) if (!(await fn(x, o))) return false;
    return true;
  };
  P.find = async function (fn, opts) {
    const o = mkOpts(opts);
    for await (const x of src(this)) if (await fn(x, o)) return x;
    return undefined;
  };
  P.iterator = function () { return this[Symbol.asyncIterator](); };
  // compose: node accepts STREAM operands AND FUNCTION operators (`fn(source)` ->
  // (async-)iterable). cruft's Rust-native module `compose` only pipes streams and
  // HANGS on a function, so wrap it: a `source + operator-functions` chain is built
  // with `Readable.from(fn(...))`; pure-stream (and function-first) cases delegate to
  // the Rust impl unchanged. Used for BOTH the module `stream.compose` and
  // `Readable.prototype.compose`.
  const rustCompose = globalThis.__cruft_stream_compose;
  const streamMod = globalThis.__cruft_stream_module;
  const isOperand = (a) =>
    a != null &&
    (typeof a === 'function' ||
      typeof a.pipe === 'function' ||
      typeof a[Symbol.asyncIterator] === 'function' ||
      typeof a[Symbol.iterator] === 'function');
  const composeImpl = function (...args) {
    const operands = args.filter(isOperand);
    const hasFn = operands.some((a) => typeof a === 'function');
    // No function operator, or a function FIRST (needs a writable source): the Rust
    // impl handles / attempts these — preserve its tested stream behavior.
    if (!hasFn || typeof operands[0] === 'function' || typeof rustCompose !== 'function') {
      if (typeof rustCompose === 'function') return rustCompose.apply(null, args);
      throw new TypeError('compose is not supported');
    }
    let src = operands[0];
    for (let i = 1; i < operands.length; i++) {
      const op = operands[i];
      if (typeof op === 'function') {
        src = op(src);
      } else {
        // a stream operand mixed after the source — defer to the Rust pipe chain.
        return rustCompose.apply(null, args);
      }
    }
    return Readable.from(src);
  };
  if (streamMod && typeof rustCompose === 'function') {
    streamMod.compose = composeImpl;
  }
  P.compose = function (stream, options) { return composeImpl(this, stream, options); };
  // `Writable.fromWeb(webWritableStream[, options])` — bridge a WHATWG
  // WritableStream into a node Writable (edge-runtime / undici adapters).
  // Readable.fromWeb is native (Rust); this mirrors it over the public
  // Writable ctor + the web writer API. Forwards each chunk to the writer,
  // maps `end` -> writer.close() and destroy -> writer.abort().
  if (streamMod && streamMod.Writable && typeof streamMod.Writable.fromWeb !== 'function') {
    const Writable = streamMod.Writable;
    Writable.fromWeb = function (webWritable, options) {
      const writer = webWritable.getWriter();
      const opts = Object.assign({}, options);
      opts.write = function (chunk, enc, cb) {
        writer.write(chunk).then(function () { cb(); }, function (e) { cb(e); });
      };
      opts.final = function (cb) {
        writer.close().then(function () { cb(); }, function (e) { cb(e); });
      };
      opts.destroy = function (err, cb) {
        writer.abort(err).then(function () { cb(err); }, function () { cb(err); });
      };
      return new Writable(opts);
    };
  }
  // `Duplex.from(body)` — combine a source into a Duplex. Supports the two
  // shapes cruft needs: a `{readable, writable}` PAIR (the Duplex.fromWeb
  // dependency) and an iterable / async-iterable / generator (readable-side
  // Duplex, like Readable.from). Reads relay the readable half; writes forward
  // to the writable half (discarded when absent).
  if (streamMod && streamMod.Duplex && typeof streamMod.Duplex.from !== 'function') {
    const Duplex = streamMod.Duplex;
    Duplex.from = function (body) {
      let rside = null, wside = null;
      const isPair = body && typeof body === 'object'
        && (typeof body.readable === 'object' || typeof body.writable === 'object')
        && typeof body[Symbol.asyncIterator] !== 'function'
        && typeof body[Symbol.iterator] !== 'function';
      if (isPair) {
        rside = body.readable || null;
        wside = body.writable || null;
      } else {
        rside = Readable.from(typeof body === 'function' ? body() : body);
      }
      const d = new Duplex({
        objectMode: true,
        read() {
          if (!rside) { this.push(null); return; }
          if (this._cruftReadStarted) return;
          this._cruftReadStarted = true;
          // Relay the readable half in flowing mode; the Duplex's own buffer +
          // the consumer's pull provide backpressure. (A manual pause/resume in
          // read() deadlocks once the internal hwm fills.)
          rside.on('data', (c) => { d.push(c); });
          rside.on('end', () => d.push(null));
          rside.on('error', (e) => d.destroy(e));
          rside.resume();
        },
        write(chunk, enc, cb) { if (wside) wside.write(chunk, enc, cb); else cb(); },
        final(cb) { if (wside) wside.end(cb); else cb(); },
      });
      return d;
    };
  }
  // `Duplex.fromWeb({readable, writable}[, options])` — bridge a WHATWG
  // readable/writable PAIR into a node Duplex, via the fromWeb halves.
  if (streamMod && streamMod.Duplex && typeof streamMod.Duplex.fromWeb !== 'function'
      && typeof streamMod.Duplex.from === 'function'
      && typeof streamMod.Writable.fromWeb === 'function') {
    const Duplex = streamMod.Duplex;
    Duplex.fromWeb = function (pair, options) {
      const readable = Readable.fromWeb(pair.readable, options);
      const writable = streamMod.Writable.fromWeb(pair.writable, options);
      return Duplex.from({ readable, writable });
    };
  }
  // node's crypto.createHash()/createHmac() return a Transform stream (writable
  // in, digest out) so `fs.createReadStream(f).pipe(createHash('sha256'))` and
  // `pipeline(src, hash)` work. cruft's native Hash/Hmac carry update/digest but
  // NO stream interface (`.on`/`.write`/`.end`/`.pipe` were absent). Wrap the
  // native object in a Transform whose `transform` feeds `update` and whose
  // `flush` pushes the final `digest`, while keeping the direct update/digest API.
  const nodeCrypto = globalThis.crypto;
  if (streamMod && streamMod.Transform && nodeCrypto
      && typeof nodeCrypto.createHash === 'function'
      && !nodeCrypto.__cruftHashStreamWrapped) {
    Object.defineProperty(nodeCrypto, '__cruftHashStreamWrapped', {
      value: true, enumerable: false, configurable: true, writable: true,
    });
    const Transform = streamMod.Transform;
    const wrapDigest = (native) => {
      // The underlying digest may only be finalized ONCE. node lets `.digest()`
      // be called after `end()`/`pipeline()` AND emits the digest on the stream
      // side — the SAME finalization. Compute the raw Buffer once and cache it so
      // the stream flush and an explicit `.digest(enc)` both serve it (calling
      // native.digest() twice throws "Digest already called").
      let cached = null;
      let explicitDigest = false;
      const finalize = () => {
        if (cached === null) cached = native.digest();
        return cached;
      };
      const t = new Transform({
        transform(chunk, enc, cb) {
          try { native.update(chunk, enc); cb(); } catch (e) { cb(e); }
        },
        flush(cb) {
          try { this.push(finalize()); cb(); } catch (e) { cb(e); }
        },
      });
      t.update = function (data, enc) { native.update(data, enc); return t; };
      t.digest = function (enc) {
        // A SECOND explicit .digest() throws "Digest already called" like node;
        // but .digest() AFTER a stream flush (which finalized internally) must
        // still return the cached digest, so only an explicit prior call bars it.
        if (explicitDigest) { return native.digest(enc); }
        explicitDigest = true;
        const buf = finalize();
        return enc ? buf.toString(enc) : buf;
      };
      if (typeof native.copy === 'function') {
        t.copy = function () { return wrapDigest(native.copy()); };
      }
      t.__cruft_crypto_kind = typeof native.copy === 'function' ? 'hash' : 'hmac';
      return t;
    };
    const origHash = nodeCrypto.createHash;
    nodeCrypto.createHash = function (alg, opts) {
      return wrapDigest(origHash.call(nodeCrypto, alg, opts));
    };
    const origHmac = nodeCrypto.createHmac;
    if (typeof origHmac === 'function') {
      nodeCrypto.createHmac = function (alg, key, opts) {
        return wrapDigest(origHmac.call(nodeCrypto, alg, key, opts));
      };
    }
  }
  // node's crypto.createCipheriv()/createDecipheriv() likewise return a Transform
  // stream (writable plaintext in, ciphertext out, and vice-versa), so
  // `readStream.pipe(cipher).pipe(writeStream)` and `cipher.on('data', …)` work.
  // cruft's native Cipher/Decipher carried update/final but NO stream interface.
  // Wrap them in a Transform whose `transform` pushes `update(chunk)` and whose
  // `flush` pushes `final()`, keeping the direct update/final/authTag/AAD API.
  if (streamMod && streamMod.Transform && nodeCrypto
      && typeof nodeCrypto.createCipheriv === 'function'
      && !nodeCrypto.__cruftCipherStreamWrapped) {
    Object.defineProperty(nodeCrypto, '__cruftCipherStreamWrapped', {
      value: true, enumerable: false, configurable: true, writable: true,
    });
    const Transform = streamMod.Transform;
    const wrapCipher = (native) => {
      const t = new Transform({
        transform(chunk, enc, cb) {
          try {
            const out = native.update(chunk, enc);
            if (out && out.length) this.push(out);
            cb();
          } catch (e) { cb(e); }
        },
        flush(cb) {
          try {
            const out = native.final();
            if (out && out.length) this.push(out);
            cb();
          } catch (e) { cb(e); }
        },
      });
      // Preserve the direct one-shot API (some callers mix update()/final() with
      // the stream). `this`-returning config methods return the wrapper for chaining.
      t.update = function (...a) { return native.update(...a); };
      t.final = function (...a) { return native.final(...a); };
      for (const m of ['setAutoPadding', 'setAAD', 'setAuthTag', 'getAuthTag']) {
        if (typeof native[m] === 'function') {
          t[m] = function (...a) {
            const r = native[m](...a);
            return r === native ? t : r;
          };
        }
      }
      t.__cruft_crypto_kind = native.__cruft_crypto_kind;
      return t;
    };
    for (const name of ['createCipheriv', 'createDecipheriv', 'createCipher', 'createDecipher']) {
      const orig = nodeCrypto[name];
      if (typeof orig === 'function') {
        nodeCrypto[name] = function (...a) { return wrapCipher(orig.apply(nodeCrypto, a)); };
      }
    }
  }
})();
"#;

pub fn install_iterator_helpers(rt: &mut Runtime) {

    let readable = match rt.global_get("stream") {
        Value::Object(s) => rt.object_get(s, "Readable"),
        _ => return,
    };
    if !rt.is_callable(&readable) {
        return;
    }

    let stream_mod = rt.global_get("stream");
    let compose = match &stream_mod {
        Value::Object(s) => rt.object_get(*s, "compose"),
        _ => Value::Undefined,
    };
    rt.define_global_property("__cruft_Readable", readable);
    rt.define_global_property("__cruft_stream_compose", compose);
    rt.define_global_property("__cruft_stream_module", stream_mod);
    let _ = rt.run_script(
        STREAM_ITERATOR_HELPERS_JS,
        "cruft:internal/stream-operators.js",
    );
    rt.define_global_property("__cruft_Readable", Value::Undefined);
    rt.define_global_property("__cruft_stream_compose", Value::Undefined);
    rt.define_global_property("__cruft_stream_module", Value::Undefined);
}

pub fn ensure_installed(rt: &mut Runtime) {
    rt.materialize_lazy_host_module("stream");
}

pub fn install(rt: &mut Runtime) {
    let stream = new_object(rt);

    for name in &[
        "Readable",
        "Writable",
        "Transform",
        "Duplex",
        "PassThrough",
        "Stream",
    ] {
        let ctor = new_object(rt);

        let proto = new_object(rt);
        rt.set_own_frozen_property(ctor, "prototype".into(), Value::Object(proto));
        rt.obj_mut(proto)
            .set_own_internal("constructor".into(), Value::Object(ctor));

        let nm = *name;
        register_method(rt, ctor, "__call__", move |_rt, _args| {
            Err(RuntimeError::TypeError(format!(
                "internal: {} called via __call__",
                nm
            )))
        });

        let _ = nm;
        rt.object_set(stream, name.to_string(), Value::Object(ctor));
    }

    for name in &[
        "Readable",
        "Writable",
        "Transform",
        "Duplex",
        "PassThrough",
        "Stream",
    ] {
        let nm = name.to_string();
        register_method(rt, stream, name, move |rt, args| {
            let opts = match args.first() {
                Some(Value::Object(id)) => Some(*id),
                _ => None,
            };
            let _ = &nm;

            let receiver = match rt.current_this() {
                Value::Object(id) => Some(id),
                _ => None,
            };
            Ok(Value::Object(make_stream_instance(
                rt, opts, receiver, &nm, None, None, false, None,
            )))
        });

        let proto = new_object(rt);
        if let Value::Object(ctor) = rt.object_get(stream, name) {
            rt.set_own_frozen_property(ctor, "prototype".into(), Value::Object(proto));
            rt.obj_mut(proto)
                .set_own_internal("constructor".into(), Value::Object(ctor));

            make_stream_instance(rt, None, Some(proto), name, None, None, false, None);
            if *name == "Stream" {
                for key in ["write", "end"] {
                    let key = Value::String(Rc::new(rusty_js_runtime::value::JsString::from(key)));
                    let _ = rt.delete_own_via(&Value::Object(proto), &key);
                }
            }
            match *name {
                "Writable" => install_stream_default_template(rt, proto, "_write"),
                "Transform" | "PassThrough" => {
                    install_stream_default_template(rt, proto, "_transform")
                }
                "Duplex" => {
                    install_stream_default_template(rt, proto, "_write");
                    install_stream_default_template(rt, proto, "_transform");
                }
                _ => {}
            }
        }
    }

    {
        let class_proto =
            |rt: &mut Runtime, name: &str| -> Option<rusty_js_runtime::value::ObjectRef> {
                match rt.object_get(stream, name) {
                    Value::Object(ctor) => match rt.object_get(ctor, "prototype") {
                        Value::Object(p) => Some(p),
                        _ => None,
                    },
                    _ => None,
                }
            };
        for (child, parent) in &[
            ("Readable", "Stream"),
            ("Writable", "Stream"),
            ("Duplex", "Readable"),
            ("Transform", "Duplex"),
            ("PassThrough", "Transform"),
        ] {
            if let (Some(c), Some(p)) = (class_proto(rt, child), class_proto(rt, parent)) {
                rt.set_object_prototype_internal(c, Some(p));
            }
        }

        let ee_proto = match rt.global_get("__cruft_events") {
            Value::Object(ee) => match rt.object_get(ee, "prototype") {
                Value::Object(p) => Some(p),
                _ => None,
            },
            _ => None,
        };
        if let (Some(sp), Some(ee)) = (class_proto(rt, "Stream"), ee_proto) {
            rt.set_object_prototype_internal(sp, Some(ee));
        }
        if let Value::Object(writable_ctor) = rt.object_get(stream, "Writable") {
            let has_instance = make_callable_rooted(
                rt,
                "[Symbol.hasInstance]",
                vec![writable_ctor],
                move |rt, args| {
                    let value = args.first().cloned().unwrap_or(Value::Undefined);
                    let target = rt.current_this();
                    let ordinary = rt.ordinary_has_instance(&value, &target)?;
                    if ordinary || !matches!(target, Value::Object(id) if id == writable_ctor) {
                        return Ok(Value::Boolean(ordinary));
                    }
                    let Value::Object(value_id) = value else {
                        return Ok(Value::Boolean(false));
                    };
                    let kind = match rt.object_get(value_id, "__stream_kind") {
                        Value::String(s) => s.as_str().to_string(),
                        _ => String::new(),
                    };
                    Ok(Value::Boolean(matches!(
                        kind.as_str(),
                        "Duplex" | "Transform" | "PassThrough"
                    )))
                },
            );
            rt.obj_mut(writable_ctor)
                .set_own_internal("@@hasInstance".into(), Value::Object(has_instance));
        }
    }

    register_method(rt, stream, "pipeline", |rt, args| {
        let mut stages: Vec<Value> = args.to_vec();
        let callback = match stages.last() {
            Some(v) if rt.is_callable(v) => stages.pop(),
            _ => None,
        };
        let fired = std::rc::Rc::new(std::cell::Cell::new(false));

        if let Some(cb) = &callback {
            for stage in &stages {
                let Value::Object(s) = stage else { continue };
                let on = rt.object_get(*s, "on");
                if !rt.is_callable(&on) {
                    continue;
                }
                let cb_e = cb.clone();
                let fired_e = fired.clone();
                let on_err = make_callable(rt, "pipeline.onError", move |rt, a| {

                    if fired_e.replace(true) {
                        return Ok(Value::Undefined);
                    }
                    let err = a.first().cloned().unwrap_or(Value::Undefined);
                    let _ = rt.call_function(cb_e.clone(), Value::Undefined, vec![err]);
                    Ok(Value::Undefined)
                });
                let ev = Value::String(Rc::new(rusty_js_runtime::value::JsString::from("error")));
                let _ = rt.call_function(on, Value::Object(*s), vec![ev, Value::Object(on_err)]);
            }
        }

        for i in 0..stages.len().saturating_sub(1) {
            if let Value::Object(a) = stages[i] {
                let pipe = rt.object_get(a, "pipe");
                if rt.is_callable(&pipe) {
                    let _ = rt.call_function(pipe, Value::Object(a), vec![stages[i + 1].clone()]);
                }
            }
        }
        let last = stages.last().cloned().unwrap_or(Value::Undefined);

        if let Some(cb) = callback {
            if let Value::Object(l) = last {
                let on = rt.object_get(l, "on");
                if rt.is_callable(&on) {
                    let cb_d = cb.clone();
                    let fired_d = fired.clone();
                    let on_done = make_callable(rt, "pipeline.onDone", move |rt, _a| {
                        if fired_d.replace(true) {
                            return Ok(Value::Undefined);
                        }

                        let _ = rt.call_function(cb_d.clone(), Value::Undefined, Vec::new());
                        Ok(Value::Undefined)
                    });
                    for event in ["finish", "end", "close"] {
                        let ev =
                            Value::String(Rc::new(rusty_js_runtime::value::JsString::from(event)));
                        let _ = rt.call_function(
                            on.clone(),
                            Value::Object(l),
                            vec![ev, Value::Object(on_done)],
                        );
                    }
                }
            }
        }
        Ok(last)
    });
    register_method(rt, stream, "finished", |rt, args| {
        let callback = args.get(1).cloned().unwrap_or(Value::Undefined);

        if let (Some(Value::Object(s)), true) = (args.first(), rt.is_callable(&callback)) {
            let s = *s;
            let called = std::rc::Rc::new(std::cell::Cell::new(false));
            let cb_err = callback.clone();
            let called_e = called.clone();
            let on_err = make_callable(rt, "finished.cb.err", move |rt, a| {
                if !called_e.replace(true) {
                    let err = a.first().cloned().unwrap_or(Value::Undefined);
                    let _ = rt.call_function(cb_err.clone(), Value::Undefined, vec![err]);
                }
                Ok(Value::Undefined)
            });
            let cb_done = callback.clone();
            let called_d = called.clone();
            let on_done = make_callable(rt, "finished.cb.done", move |rt, _a| {
                if !called_d.replace(true) {
                    let _ = rt.call_function(cb_done.clone(), Value::Undefined, Vec::new());
                }
                Ok(Value::Undefined)
            });
            let on = rt.object_get(s, "on");
            if rt.is_callable(&on) {
                for ev_name in ["error", "end", "close", "finish"] {
                    let ev = Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                        ev_name.to_string(),
                    )));
                    let listener = if ev_name == "error" {
                        Value::Object(on_err)
                    } else {
                        Value::Object(on_done)
                    };
                    let _ = rt.call_function(on.clone(), Value::Object(s), vec![ev, listener]);
                }
            }
        } else if rt.is_callable(&callback) {

            let _ = rt.call_function(callback, Value::Undefined, Vec::new())?;
        }
        let cleanup = make_callable(rt, "finishedCleanup", |_rt, _args| Ok(Value::Undefined));
        Ok(Value::Object(cleanup))
    });

    let static_helpers: &[(&str, Value)] = &[
        ("getDefaultHighWaterMark", Value::Undefined),
        ("setDefaultHighWaterMark", Value::Undefined),
        ("isReadable", Value::Undefined),
        ("isWritable", Value::Undefined),
        ("isDisturbed", Value::Undefined),
        ("isErrored", Value::Undefined),
    ];

    for &(name, _) in static_helpers {
        for ctor_key in &["Readable", "Writable"] {
            if let Value::Object(ctor) = rt.object_get(stream, ctor_key) {
                let nm = name.to_string();
                register_method(rt, ctor, name, move |_rt, args| match nm.as_str() {
                    "getDefaultHighWaterMark" => {
                        let object_mode = matches!(args.first(), Some(Value::Boolean(true)));
                        Ok(Value::Number(if object_mode {
                            16.0
                        } else {
                            16.0 * 1024.0
                        }))
                    }
                    "setDefaultHighWaterMark" => Ok(Value::Undefined),
                    "isReadable" | "isWritable" => Ok(Value::Boolean(false)),
                    "isDisturbed" | "isErrored" => Ok(Value::Boolean(false)),
                    _ => Ok(Value::Undefined),
                });
            }
        }

        let nm = name.to_string();
        register_method(rt, stream, name, move |_rt, args| match nm.as_str() {
            "getDefaultHighWaterMark" => {
                let object_mode = matches!(args.first(), Some(Value::Boolean(true)));
                Ok(Value::Number(if object_mode {
                    16.0
                } else {
                    16.0 * 1024.0
                }))
            }
            "setDefaultHighWaterMark" => Ok(Value::Undefined),
            "isReadable" | "isWritable" => Ok(Value::Boolean(false)),
            "isDisturbed" | "isErrored" => Ok(Value::Boolean(false)),
            _ => Ok(Value::Undefined),
        });
    }

    if let Value::Object(readable) = rt.object_get(stream, "Readable") {
        if let Value::Object(readable_proto) = rt.object_get(readable, "prototype") {
            register_method(rt, readable_proto, "next", |rt, _args| {
                let this = match rt.current_this() {
                    Value::Object(t) => t,
                    _ => return Ok(stream_iter_result(rt, Value::Undefined, true)),
                };
                if let Value::Object(iter_id) = rt.object_get(this, "__async_iter") {
                    let next = rt.object_get(iter_id, "next");
                    if !rt.is_callable(&next) {
                        return Ok(stream_iter_result(rt, Value::Undefined, true));
                    }

                    return rt.call_function(next, Value::Object(iter_id), Vec::new());
                }
                let buf = match rt.object_get(this, "__rbuf") {
                    Value::Object(b) => b,
                    _ => return Ok(stream_iter_result(rt, Value::Undefined, true)),
                };
                let len = rt.array_length(buf);
                let idx = match rt.object_get(this, "__riter_index") {
                    Value::Number(n) if n.is_finite() && n >= 0.0 => n as usize,
                    _ => 0,
                };
                if idx >= len {
                    return Ok(stream_iter_result(rt, Value::Undefined, true));
                }
                let value = rt.object_get(buf, &idx.to_string());
                readable_from_set_index(rt, this, idx + 1);
                Ok(stream_iter_result(rt, value, false))
            });

            install_async_iterator(rt, readable_proto);
            register_method(rt, readable_proto, "@@iterator", |rt, _args| {
                Ok(rt.current_this())
            });

            register_method(rt, readable_proto, "wrap", |rt, args| {
                let this = match rt.current_this() {
                    Value::Object(t) => t,
                    _ => return Ok(Value::Undefined),
                };
                let legacy = match args.first() {
                    Some(Value::Object(id)) => *id,
                    _ => return Ok(rt.current_this()),
                };
                rt.object_set(legacy, "__wrap_dest__".into(), Value::Object(this));
                let on = rt.object_get(legacy, "on");
                if rt.is_callable(&on) {
                    let on_data = make_callable(rt, "", |rt, args| {
                        if let Value::Object(t) = rt.current_this() {
                            if let Value::Object(dest) = rt.object_get(t, "__wrap_dest__") {
                                let push = rt.object_get(dest, "push");
                                let chunk = args.first().cloned().unwrap_or(Value::Undefined);
                                let _ = rt.call_function(push, Value::Object(dest), vec![chunk]);
                            }
                        }
                        Ok(Value::Undefined)
                    });
                    let on_end = make_callable(rt, "", |rt, _args| {
                        if let Value::Object(t) = rt.current_this() {
                            if let Value::Object(dest) = rt.object_get(t, "__wrap_dest__") {
                                let push = rt.object_get(dest, "push");
                                let _ =
                                    rt.call_function(push, Value::Object(dest), vec![Value::Null]);
                            }
                        }
                        Ok(Value::Undefined)
                    });
                    let on_error = make_callable(rt, "", |rt, args| {
                        if let Value::Object(t) = rt.current_this() {
                            if let Value::Object(dest) = rt.object_get(t, "__wrap_dest__") {
                                let emit = rt.object_get(dest, "emit");
                                let e = args.first().cloned().unwrap_or(Value::Undefined);
                                let s = Value::String(Rc::new(
                                    rusty_js_runtime::value::JsString::from("error".to_string()),
                                ));
                                let _ = rt.call_function(emit, Value::Object(dest), vec![s, e]);
                            }
                        }
                        Ok(Value::Undefined)
                    });
                    let s = |v: &str| {
                        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                            v.to_string(),
                        )))
                    };
                    let _ = rt.call_function(
                        on.clone(),
                        Value::Object(legacy),
                        vec![s("data"), Value::Object(on_data)],
                    );
                    let _ = rt.call_function(
                        on.clone(),
                        Value::Object(legacy),
                        vec![s("end"), Value::Object(on_end)],
                    );
                    let _ = rt.call_function(
                        on,
                        Value::Object(legacy),
                        vec![s("error"), Value::Object(on_error)],
                    );
                }
                Ok(Value::Object(this))
            });
        }
        register_method(rt, readable, "from", move |rt, args| {
            let source = args.first().cloned().unwrap_or(Value::Undefined);
            let readable_proto = match rt.object_get(readable, "prototype") {
                Value::Object(proto) => Some(proto),
                _ => None,
            };
            let is_string_source = matches!(source, Value::String(_));
            let result = make_readable_from(rt, source, readable_proto)?;

            if let Value::Object(sid) = &result {
                let opts = args.get(1).and_then(|o| match o {
                    Value::Object(oid) => Some(*oid),
                    _ => None,
                });
                let object_mode = opts
                    .map(|oid| match rt.object_get(oid, "objectMode") {
                        Value::Boolean(b) => b,
                        _ => true,
                    })
                    .unwrap_or(true);
                let default_hwm = if is_string_source { 16.0 } else { 1.0 };
                let hwm = opts
                    .and_then(|oid| match rt.object_get(oid, "highWaterMark") {
                        Value::Number(n) if n.is_finite() && n >= 0.0 => Some(n),
                        _ => None,
                    })
                    .unwrap_or(default_hwm);
                rt.object_set(
                    *sid,
                    "readableObjectMode".into(),
                    Value::Boolean(object_mode),
                );
                rt.object_set(*sid, "readableHighWaterMark".into(), Value::Number(hwm));
            }
            Ok(result)
        });

        register_method(rt, readable, "toWeb", |rt, args| {
            let src = match args.first() {
                Some(Value::Object(id)) => *id,
                _ => return Ok(Value::Undefined),
            };
            let source = new_object(rt);
            rt.object_set(source, "__toweb_src__".into(), Value::Object(src));
            register_method(rt, source, "start", |rt, args| {
                let this = match rt.current_this() {
                    Value::Object(t) => t,
                    _ => return Ok(Value::Undefined),
                };
                let src = match rt.object_get(this, "__toweb_src__") {
                    Value::Object(s) => s,
                    _ => return Ok(Value::Undefined),
                };
                let ctrl = args.first().cloned().unwrap_or(Value::Undefined);
                rt.object_set(src, "__toweb_ctrl__".into(), ctrl);
                let on = rt.object_get(src, "on");
                if !rt.is_callable(&on) {
                    return Ok(Value::Undefined);
                }
                let on_data = make_callable(rt, "", |rt, args| {
                    if let Value::Object(t) = rt.current_this() {
                        if let Value::Object(c) = rt.object_get(t, "__toweb_ctrl__") {
                            let enqueue = rt.object_get(c, "enqueue");
                            let mut chunk = args.first().cloned().unwrap_or(Value::Undefined);

                            if !readable_object_mode(rt, t) {
                                if let Value::Object(_) = chunk {
                                    let u8 = rt.global_get("Uint8Array");
                                    if let Ok(v) = rt.construct(u8, vec![chunk.clone()]) {
                                        chunk = v;
                                    }
                                }
                            }
                            let _ = rt.call_function(enqueue, Value::Object(c), vec![chunk]);
                        }
                    }
                    Ok(Value::Undefined)
                });
                let on_end = make_callable(rt, "", |rt, _args| {
                    if let Value::Object(t) = rt.current_this() {
                        if let Value::Object(c) = rt.object_get(t, "__toweb_ctrl__") {
                            let close = rt.object_get(c, "close");
                            let _ = rt.call_function(close, Value::Object(c), Vec::new());
                        }
                    }
                    Ok(Value::Undefined)
                });
                let on_error = make_callable(rt, "", |rt, args| {
                    if let Value::Object(t) = rt.current_this() {
                        if let Value::Object(c) = rt.object_get(t, "__toweb_ctrl__") {
                            let errf = rt.object_get(c, "error");
                            let e = args.first().cloned().unwrap_or(Value::Undefined);
                            let _ = rt.call_function(errf, Value::Object(c), vec![e]);
                        }
                    }
                    Ok(Value::Undefined)
                });
                let s = |v: &str| {
                    Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                        v.to_string(),
                    )))
                };
                let _ = rt.call_function(
                    on.clone(),
                    Value::Object(src),
                    vec![s("data"), Value::Object(on_data)],
                );
                let _ = rt.call_function(
                    on.clone(),
                    Value::Object(src),
                    vec![s("end"), Value::Object(on_end)],
                );
                let _ = rt.call_function(
                    on,
                    Value::Object(src),
                    vec![s("error"), Value::Object(on_error)],
                );
                Ok(Value::Undefined)
            });
            rt.materialize_lazy_global("ReadableStream");
            let rs_ctor = rt.global_get("ReadableStream");
            rt.construct(rs_ctor, vec![Value::Object(source)])
        });

        register_method(rt, readable, "fromWeb", move |rt, args| {
            let source = args.first().cloned().unwrap_or(Value::Undefined);
            let readable_proto = match rt.object_get(readable, "prototype") {
                Value::Object(proto) => Some(proto),
                _ => None,
            };
            let result = make_readable_from(rt, source, readable_proto)?;
            if let Value::Object(stream_id) = result {
                rt.object_set(stream_id, "__coerce_buffer".into(), Value::Boolean(true));

                if let Value::Object(rs) = rt.object_get(stream_id, "_readableState") {
                    rt.object_set(rs, "objectMode".into(), Value::Boolean(false));
                }
            }
            Ok(result)
        });
    }

    let readable_compat = new_object(rt);
    for key in &[
        "Duplex",
        "PassThrough",
        "Readable",
        "Stream",
        "Transform",
        "Writable",
        "default",
        "finished",
        "isDisturbed",
        "isErrored",
        "isReadable",
        "pipeline",
    ] {
        let v = rt.object_get(stream, key);
        if !matches!(v, Value::Undefined) {
            rt.object_set(readable_compat, (*key).to_string(), v);
        }
    }
    for key in &[
        "ReadableState",
        "_fromList",
        "_isUint8Array",
        "_uint8ArrayToBuffer",
        "addAbortSignal",
        "compose",
        "destroy",
        "from",
        "fromWeb",
        "toWeb",
        "wrap",
    ] {
        register_method(rt, readable_compat, key, |_rt, _args| Ok(Value::Undefined));
    }
    rt.object_set(readable_compat, "length".into(), Value::Number(0.0));
    rt.object_set(
        readable_compat,
        "name".into(),
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
            "Stream".to_string(),
        ))),
    );
    let readable_proto = new_object(rt);
    rt.object_set(
        readable_compat,
        "prototype".into(),
        Value::Object(readable_proto),
    );
    rt.define_global_property("__readable_stream_compat", Value::Object(readable_compat));

    match rt.object_get(stream, "Stream") {
        Value::Object(stream_ctor) => {
            for k in rt.ordinary_own_enumerable_string_keys(stream) {
                if matches!(
                    k.as_str(),
                    "Stream" | "prototype" | "name" | "length" | "constructor"
                ) {
                    continue;
                }
                let v = rt.object_get(stream, &k);
                rt.object_set(stream_ctor, k, v);
            }

            rt.object_set(stream_ctor, "Stream".into(), Value::Object(stream_ctor));
            rt.define_global_property("stream", Value::Object(stream_ctor));
        }
        _ => rt.define_global_property("stream", Value::Object(stream)),
    }

    install_stream_consumers(rt);
    install_stream_promises(rt);
    install_stream_web(rt);

    if let Value::Object(se) = rt.global_get("stream") {
        for f in [
            "_isArrayBufferView",
            "_isUint8Array",
            "_uint8ArrayToBuffer",
            "destroy",
            "duplexPair",
            "isDestroyed",
        ] {
            register_method(rt, se, f, |_rt, _a| Ok(Value::Undefined));
        }

        register_method(rt, se, "addAbortSignal", |rt, args| {
            let signal = args.first().cloned().unwrap_or(Value::Undefined);
            let stream = args.get(1).cloned().unwrap_or(Value::Undefined);
            let Value::Object(sig_id) = signal else {
                return Ok(stream);
            };
            let Value::Object(stream_id) = stream else {
                return Ok(stream);
            };
            let on_abort = make_callable(rt, "addAbortSignal.onAbort", move |rt, _a| {
                let s = |x: &str| {
                    Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                        x.to_string(),
                    )))
                };
                let err = match rt
                    .construct(rt.global_get("Error"), vec![s("The operation was aborted")])
                {
                    Ok(Value::Object(id)) => {
                        rt.object_set(id, "name".into(), s("AbortError"));
                        rt.object_set(id, "code".into(), s("ABORT_ERR"));
                        Value::Object(id)
                    }
                    _ => Value::Undefined,
                };
                let destroy = rt.object_get(stream_id, "destroy");
                if rt.is_callable(&destroy) {
                    let _ = rt.call_function(destroy, Value::Object(stream_id), vec![err]);
                }
                Ok(Value::Undefined)
            });
            if matches!(rt.object_get(sig_id, "aborted"), Value::Boolean(true)) {

                rt.enqueue_host_phase_rooted(
                    HostEnqueuePhase::HostCompletionMacrotask,
                    "addAbortSignal.preAborted",
                    vec![on_abort, stream_id],
                    move |rt| {
                        let _ =
                            rt.call_function(Value::Object(on_abort), Value::Undefined, Vec::new());
                        Ok(())
                    },
                );
            } else {
                let add = rt.object_get(sig_id, "addEventListener");
                if rt.is_callable(&add) {
                    let ev = Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                        "abort".to_string(),
                    )));
                    let _ = rt.call_function(
                        add,
                        Value::Object(sig_id),
                        vec![ev, Value::Object(on_abort)],
                    );
                }
            }
            Ok(Value::Object(stream_id))
        });

        register_method(rt, se, "compose", |rt, args| {
            let streams: Vec<Value> = args
                .iter()
                .filter(|v| matches!(v, Value::Object(_)))
                .cloned()
                .collect();
            if streams.is_empty() {
                return Err(RuntimeError::TypeError(
                    "The \"streams\" argument must be specified".into(),
                ));
            }
            for i in 0..streams.len().saturating_sub(1) {
                if let Value::Object(a) = &streams[i] {
                    let pipe = rt.object_get(*a, "pipe");
                    if rt.is_callable(&pipe) {
                        let _ = rt.call_function(
                            pipe,
                            streams[i].clone(),
                            vec![streams[i + 1].clone()],
                        );
                    }
                }
            }
            let first = streams[0].clone();
            let last = streams[streams.len() - 1].clone();
            let duplex_proto = match rt.global_get("stream") {
                Value::Object(s) => match rt.object_get(s, "Duplex") {
                    Value::Object(d) => match rt.object_get(d, "prototype") {
                        Value::Object(p) => Some(p),
                        _ => None,
                    },
                    _ => None,
                },
                _ => None,
            };
            let composed =
                make_stream_instance(rt, None, None, "Duplex", duplex_proto, None, false, None);

            let first_w = first.clone();
            register_method(rt, composed, "write", move |rt, a| {
                let w = match &first_w {
                    Value::Object(id) => rt.object_get(*id, "write"),
                    _ => Value::Undefined,
                };
                rt.call_function(w, first_w.clone(), a.to_vec())
            });
            let first_e = first.clone();
            register_method(rt, composed, "end", move |rt, a| {
                let e = match &first_e {
                    Value::Object(id) => rt.object_get(*id, "end"),
                    _ => Value::Undefined,
                };
                let _ = rt.call_function(e, first_e.clone(), a.to_vec());
                Ok(rt.current_this())
            });

            if let Value::Object(last_id) = &last {
                let on = rt.object_get(*last_id, "on");
                if rt.is_callable(&on) {
                    let on_data = make_callable(rt, "compose.data", move |rt, a| {
                        let chunk = a.first().cloned().unwrap_or(Value::Undefined);
                        let push = rt.object_get(composed, "push");
                        if rt.is_callable(&push) {
                            let _ = rt.call_function(push, Value::Object(composed), vec![chunk]);
                        }
                        Ok(Value::Undefined)
                    });
                    let on_end = make_callable(rt, "compose.end", move |rt, _a| {
                        let push = rt.object_get(composed, "push");
                        if rt.is_callable(&push) {
                            let _ =
                                rt.call_function(push, Value::Object(composed), vec![Value::Null]);
                        }
                        Ok(Value::Undefined)
                    });
                    let on_err = make_callable(rt, "compose.err", move |rt, a| {
                        let err = a.first().cloned().unwrap_or(Value::Undefined);
                        stream_emit(rt, composed, "error", vec![err]);
                        Ok(Value::Undefined)
                    });
                    for (ev, cb) in [("data", on_data), ("end", on_end), ("error", on_err)] {
                        let evv = Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                            ev.to_string(),
                        )));
                        let _ = rt.call_function(
                            on.clone(),
                            last.clone(),
                            vec![evv, Value::Object(cb)],
                        );
                    }
                }
            }
            Ok(Value::Object(composed))
        });
        if let Value::Object(sp) = rt.global_get("stream_promises") {
            rt.object_set(se, "promises".into(), Value::Object(sp));
        }
    }
}

pub fn wire_event_emitter_alias(rt: &mut Runtime) {
    if let (Value::Object(se), Value::Object(ee)) =
        (rt.global_get("stream"), rt.global_get("__cruft_events"))
    {
        rt.obj_mut(se)
            .set_own_internal("EventEmitter".into(), Value::Object(ee));
        let _ = ee;

        let user_ee_proto = match rt.global_get("events") {
            Value::Object(e) => match rt.object_get(e, "prototype") {
                Value::Object(p) => Some(p),
                _ => None,
            },
            _ => None,
        };
        if let (Value::Object(stream_ctor), Some(ee_proto)) =
            (rt.object_get(se, "Stream"), user_ee_proto)
        {
            if let Value::Object(stream_proto) = rt.object_get(stream_ctor, "prototype") {
                if !matches!(rt.obj(stream_proto).proto, Some(p) if p == ee_proto) {
                    rt.set_object_prototype_internal(stream_proto, Some(ee_proto));
                }
            }
        }
    }
}

fn install_stream_web(rt: &mut Runtime) {
    rt.materialize_lazy_global("ReadableStream");
    rt.materialize_lazy_global("CompressionStream");
    let web = new_object(rt);
    const CLASSES: &[&str] = &[
        "ReadableStream",
        "WritableStream",
        "TransformStream",
        "ByteLengthQueuingStrategy",
        "CountQueuingStrategy",
        "ReadableStreamDefaultController",
        "ReadableStreamDefaultReader",
        "ReadableStreamBYOBReader",
        "ReadableStreamBYOBRequest",
        "ReadableByteStreamController",
        "WritableStreamDefaultController",
        "WritableStreamDefaultWriter",
        "TransformStreamDefaultController",
        "TextEncoderStream",
        "TextDecoderStream",
        "CompressionStream",
        "DecompressionStream",
    ];
    for name in CLASSES {
        let g = rt.global_get(name);
        if !matches!(g, Value::Undefined) {
            rt.object_set(web, (*name).to_string(), g);
        }
    }
    rt.define_global_property("stream_web", Value::Object(web));
}

fn pipeline_is_stream(rt: &mut Runtime, v: &Value) -> bool {
    if let Value::Object(o) = v {
        rt.is_callable(&rt.object_get(*o, "on")) || rt.is_callable(&rt.object_get(*o, "pipe"))
    } else {
        false
    }
}

fn pipeline_drain_async_iter(
    rt: &mut Runtime,
    promise: rusty_js_runtime::ObjectRef,
    iter: rusty_js_runtime::ObjectRef,
) {
    let next = rt.object_get(iter, "next");
    if !rt.is_callable(&next) {
        rusty_js_runtime::promise::resolve_promise(rt, promise, Value::Undefined);
        return;
    }
    let step = match rt.call_function(next, Value::Object(iter), Vec::new()) {
        Ok(v) => v,
        Err(e) => {
            let ev = match e {
                RuntimeError::Thrown(v) => v,
                _ => Value::Undefined,
            };
            rusty_js_runtime::promise::reject_promise(rt, promise, ev);
            return;
        }
    };
    let then = match &step {
        Value::Object(pid) => rt.object_get(*pid, "then"),
        _ => Value::Undefined,
    };
    if !rt.is_callable(&then) {

        rusty_js_runtime::promise::resolve_promise(rt, promise, Value::Undefined);
        return;
    }
    let on_ful = make_callable(rt, "pipeline.drain", move |rt, a| {
        let result = a.first().cloned().unwrap_or(Value::Undefined);
        let done = match &result {
            Value::Object(rid) => matches!(rt.object_get(*rid, "done"), Value::Boolean(true)),
            _ => true,
        };
        if done {
            rusty_js_runtime::promise::resolve_promise(rt, promise, Value::Undefined);
        } else {
            pipeline_drain_async_iter(rt, promise, iter);
        }
        Ok(Value::Undefined)
    });
    let on_rej = make_callable(rt, "pipeline.drain.err", move |rt, a| {
        rusty_js_runtime::promise::reject_promise(
            rt,
            promise,
            a.first().cloned().unwrap_or(Value::Undefined),
        );
        Ok(Value::Undefined)
    });
    let _ = rt.call_function(
        then,
        step,
        vec![Value::Object(on_ful), Value::Object(on_rej)],
    );
}

fn pipeline_pump_iter_to_stream(
    rt: &mut Runtime,
    promise: rusty_js_runtime::ObjectRef,
    iter: rusty_js_runtime::ObjectRef,
    target: rusty_js_runtime::ObjectRef,
) {
    let next = rt.object_get(iter, "next");
    if !rt.is_callable(&next) {
        return;
    }
    let step = match rt.call_function(next, Value::Object(iter), Vec::new()) {
        Ok(v) => v,
        Err(e) => {
            let ev = match e {
                RuntimeError::Thrown(v) => v,
                _ => Value::Undefined,
            };
            rusty_js_runtime::promise::reject_promise(rt, promise, ev);
            return;
        }
    };
    let then = match &step {
        Value::Object(pid) => rt.object_get(*pid, "then"),
        _ => Value::Undefined,
    };
    if !rt.is_callable(&then) {
        return;
    }
    let on_ful = make_callable(rt, "pipeline.pump", move |rt, a| {
        let result = a.first().cloned().unwrap_or(Value::Undefined);
        let (value, done) = match &result {
            Value::Object(rid) => (
                rt.object_get(*rid, "value"),
                matches!(rt.object_get(*rid, "done"), Value::Boolean(true)),
            ),
            _ => (Value::Undefined, true),
        };
        if done {
            let end = rt.object_get(target, "end");
            if rt.is_callable(&end) {
                let _ = rt.call_function(end, Value::Object(target), Vec::new());
            }
        } else {
            let write = rt.object_get(target, "write");
            if rt.is_callable(&write) {
                let _ = rt.call_function(write, Value::Object(target), vec![value]);
            }
            pipeline_pump_iter_to_stream(rt, promise, iter, target);
        }
        Ok(Value::Undefined)
    });
    let on_rej = make_callable(rt, "pipeline.pump.err", move |rt, a| {
        rusty_js_runtime::promise::reject_promise(
            rt,
            promise,
            a.first().cloned().unwrap_or(Value::Undefined),
        );
        Ok(Value::Undefined)
    });
    let _ = rt.call_function(
        then,
        step,
        vec![Value::Object(on_ful), Value::Object(on_rej)],
    );
}

fn pipeline_settle(rt: &mut Runtime, promise: rusty_js_runtime::ObjectRef, current: Value) {

    if let Value::Object(cid) = &current {
        let then = rt.object_get(*cid, "then");
        if rt.is_callable(&then) {
            let prom = promise;
            let on_ful = make_callable(rt, "pipeline.value", move |rt, a| {
                rusty_js_runtime::promise::resolve_promise(
                    rt,
                    prom,
                    a.first().cloned().unwrap_or(Value::Undefined),
                );
                Ok(Value::Undefined)
            });
            let on_rej = make_callable(rt, "pipeline.value.err", move |rt, a| {
                rusty_js_runtime::promise::reject_promise(
                    rt,
                    prom,
                    a.first().cloned().unwrap_or(Value::Undefined),
                );
                Ok(Value::Undefined)
            });
            let _ = rt.call_function(
                then,
                current.clone(),
                vec![Value::Object(on_ful), Value::Object(on_rej)],
            );
            return;
        }

        if rt.is_callable(&rt.object_get(*cid, "next")) {
            pipeline_drain_async_iter(rt, promise, *cid);
            return;
        }

        let on = rt.object_get(*cid, "on");
        if rt.is_callable(&on) {
            let prom = promise;
            let on_done = make_callable(rt, "pipeline.resolve", move |rt, _a| {
                rusty_js_runtime::promise::resolve_promise(rt, prom, Value::Undefined);
                Ok(Value::Undefined)
            });
            for ev_name in ["finish", "end", "close"] {
                let ev = Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                    ev_name.to_string(),
                )));
                let _ = rt.call_function(
                    on.clone(),
                    current.clone(),
                    vec![ev, Value::Object(on_done)],
                );
            }
            return;
        }
    }
    rusty_js_runtime::promise::resolve_promise(rt, promise, Value::Undefined);
}

fn install_stream_promises(rt: &mut Runtime) {
    let sp = new_object(rt);

    register_method(rt, sp, "pipeline", |rt, args| {
        let promise = rusty_js_runtime::promise::new_promise(rt);

        let stages: Vec<Value> = args
            .iter()
            .filter(|v| rt.is_callable(v) || pipeline_is_stream(rt, v))
            .cloned()
            .collect();
        if stages.is_empty() {
            rusty_js_runtime::promise::resolve_promise(rt, promise, Value::Undefined);
            return Ok(Value::Object(promise));
        }

        for st in &stages {
            if pipeline_is_stream(rt, st) {
                if let Value::Object(s) = st {
                    let prom = promise;
                    let on_err = make_callable(rt, "pipeline.reject", move |rt, a| {
                        let e = a.first().cloned().unwrap_or(Value::Undefined);
                        rusty_js_runtime::promise::reject_promise(rt, prom, e);
                        Ok(Value::Undefined)
                    });
                    let on = rt.object_get(*s, "on");
                    let ev = Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                        "error".to_string(),
                    )));
                    let _ =
                        rt.call_function(on, Value::Object(*s), vec![ev, Value::Object(on_err)]);
                }
            }
        }

        let mut current = stages[0].clone();
        for stage in stages.iter().skip(1) {
            if rt.is_callable(stage) {
                match rt.call_function(stage.clone(), Value::Undefined, vec![current.clone()]) {
                    Ok(v) => current = v,
                    Err(e) => {
                        let ev = match e {
                            RuntimeError::Thrown(v) => v,
                            _ => Value::Undefined,
                        };
                        rusty_js_runtime::promise::reject_promise(rt, promise, ev);
                        return Ok(Value::Object(promise));
                    }
                }
            } else {

                if let (Value::Object(cur), Value::Object(tgt)) = (&current, stage) {
                    let pipe = rt.object_get(*cur, "pipe");
                    if rt.is_callable(&pipe) {
                        let _ = rt.call_function(pipe, current.clone(), vec![stage.clone()]);
                    } else if rt.is_callable(&rt.object_get(*cur, "next")) {
                        pipeline_pump_iter_to_stream(rt, promise, *cur, *tgt);
                    }
                }
                current = stage.clone();
            }
        }

        pipeline_settle(rt, promise, current);
        Ok(Value::Object(promise))
    });

    register_method(rt, sp, "finished", |rt, args| {
        let promise = rusty_js_runtime::promise::new_promise(rt);
        if let Some(Value::Object(s)) = args.first() {

            let prom = promise;
            let on_err = make_callable(rt, "finished.reject", move |rt, a| {
                rusty_js_runtime::promise::reject_promise(
                    rt,
                    prom,
                    a.first().cloned().unwrap_or(Value::Undefined),
                );
                Ok(Value::Undefined)
            });
            let prom = promise;
            let on_done = make_callable(rt, "finished.resolve", move |rt, _a| {
                rusty_js_runtime::promise::resolve_promise(rt, prom, Value::Undefined);
                Ok(Value::Undefined)
            });
            let on = rt.object_get(*s, "on");
            if rt.is_callable(&on) {
                for ev_name in ["error", "end", "close", "finish"] {
                    let ev = Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                        ev_name.to_string(),
                    )));
                    let listener = if ev_name == "error" {
                        Value::Object(on_err)
                    } else {
                        Value::Object(on_done)
                    };
                    let _ = rt.call_function(on.clone(), Value::Object(*s), vec![ev, listener]);
                }
            }
        } else {
            rusty_js_runtime::promise::resolve_promise(rt, promise, Value::Undefined);
        }
        Ok(Value::Object(promise))
    });

    rt.define_global_property("stream_promises", Value::Object(sp));
}

fn consume_chunk_bytes(rt: &Runtime, v: &Value) -> Vec<u8> {
    match v {
        Value::String(s) => s.as_bytes().to_vec(),
        Value::Object(id) => {
            let len = match rt.object_get(*id, "length") {
                Value::Number(n) => n as usize,
                _ => 0,
            };
            (0..len)
                .map(|i| match rt.object_get(*id, &i.to_string()) {
                    Value::Number(n) => n as u8,
                    _ => 0,
                })
                .collect()
        }
        _ => Vec::new(),
    }
}

fn consume_finalize(rt: &mut Runtime, acc: rusty_js_runtime::ObjectRef, mode: &str) -> Value {
    let n = rt.array_length(acc);
    let mut bytes = Vec::new();
    for i in 0..n {
        bytes.extend(consume_chunk_bytes(rt, &rt.object_get(acc, &i.to_string())));
    }
    let text = || {
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
            String::from_utf8_lossy(&bytes).into_owned(),
        )))
    };
    match mode {
        "text" => text(),
        "json" => {
            let s = String::from_utf8_lossy(&bytes).into_owned();
            let json = rt.global_get("JSON");
            if let Value::Object(j) = json {
                let parse = rt.object_get(j, "parse");
                if rt.is_callable(&parse) {
                    let arg = Value::String(Rc::new(rusty_js_runtime::value::JsString::from(s)));
                    return rt
                        .call_function(parse, json, vec![arg])
                        .unwrap_or(Value::Undefined);
                }
            }
            Value::Undefined
        }
        "bytes" => Value::Object(rt.alloc_uint8_array_from_bytes(&bytes)),
        "arrayBuffer" => {
            let u8a = rt.alloc_uint8_array_from_bytes(&bytes);
            rt.object_get(u8a, "buffer")
        }
        "blob" => {
            let u8a = rt.alloc_uint8_array_from_bytes(&bytes);
            let blob_ctor = rt.global_get("Blob");
            if rt.is_callable(&blob_ctor) {
                let parts = rt.alloc_object(RtObject::new_array());
                rt.object_set(parts, "0".into(), Value::Object(u8a));
                rt.object_set(parts, "length".into(), Value::Number(1.0));
                return rt
                    .construct(blob_ctor, vec![Value::Object(parts)])
                    .unwrap_or(Value::Undefined);
            }
            Value::Undefined
        }

        _ => crate::net::net_buffer_from_bytes(rt, &bytes),
    }
}

fn install_stream_consumers(rt: &mut Runtime) {
    let consumers = new_object(rt);
    for mode in ["text", "json", "buffer", "bytes", "arrayBuffer", "blob"] {
        register_method(rt, consumers, mode, move |rt, args| {
            let stream = match args.first() {
                Some(Value::Object(s)) => *s,
                _ => {
                    return Err(RuntimeError::TypeError(
                        "stream/consumers: argument must be a stream".into(),
                    ))
                }
            };
            let promise = rusty_js_runtime::promise::new_promise(rt);
            let acc = rt.alloc_object(RtObject::new_array());
            rt.object_set(acc, "length".into(), Value::Number(0.0));

            let on_data = make_callable(rt, "__consume_data", move |rt, a| {
                let chunk = a.first().cloned().unwrap_or(Value::Undefined);
                let len = rt.array_length(acc);
                rt.object_set(acc, len.to_string(), chunk);
                rt.object_set(acc, "length".into(), Value::Number((len + 1) as f64));
                Ok(Value::Undefined)
            });

            let m = mode.to_string();
            let on_end = make_callable(rt, "__consume_end", move |rt, _a| {
                let result = consume_finalize(rt, acc, &m);
                rusty_js_runtime::promise::resolve_promise(rt, promise, result);
                Ok(Value::Undefined)
            });

            let on_err = make_callable(rt, "__consume_err", move |rt, a| {
                let e = a.first().cloned().unwrap_or(Value::Undefined);
                rusty_js_runtime::promise::reject_promise(rt, promise, e);
                Ok(Value::Undefined)
            });
            let on = rt.object_get(stream, "on");
            if rt.is_callable(&on) {

                let s = Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                    "error".to_string(),
                )));
                rt.call_function(
                    on.clone(),
                    Value::Object(stream),
                    vec![s, Value::Object(on_err)],
                )?;
                let s = Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                    "end".to_string(),
                )));
                rt.call_function(
                    on.clone(),
                    Value::Object(stream),
                    vec![s, Value::Object(on_end)],
                )?;
                let s = Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                    "data".to_string(),
                )));
                rt.call_function(on, Value::Object(stream), vec![s, Value::Object(on_data)])?;
            }
            Ok(Value::Object(promise))
        });
    }
    rt.define_global_property("stream_consumers", Value::Object(consumers));
}
