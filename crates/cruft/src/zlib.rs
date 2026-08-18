
use crate::press::{self, PressFormat};
use crate::register::{make_callable, make_subclassable, new_object, register_method};
use rusty_js_deflate::{CompressionFormat, StreamCodec};
use rusty_js_runtime::value::{Object as RtObject, ObjectRef};
use rusty_js_runtime::{HostEnqueuePhase, Runtime, RuntimeError, Value};
use std::rc::Rc;

const ZLIB_LISTENERS_SLOT: &str = "__zlib_listeners";
const ZLIB_INPUT_SLOT: &str = "__zlib_input";
const ZLIB_FORMAT_SLOT: &str = "__zlib_format";
const ZLIB_DECODE_SLOT: &str = "__zlib_decode";
const ZLIB_BROTLI_SLOT: &str = "__zlib_brotli";
const ZLIB_PIPES_SLOT: &str = "__zlib_pipes";
const ZLIB_CHUNKS_SLOT: &str = "__zlib_chunks";
const ZLIB_ENDED_SLOT: &str = "__zlib_ended";

const ZLIB_BR_QUALITY_SLOT: &str = "__zlib_br_quality";
const ZLIB_BR_LGWIN_SLOT: &str = "__zlib_br_lgwin";
const ZLIB_BR_MODE_SLOT: &str = "__zlib_br_mode";
const ZLIB_BR_SIZEHINT_SLOT: &str = "__zlib_br_sizehint";
const ZLIB_BR_LARGEWIN_SLOT: &str = "__zlib_br_largewin";
const ZLIB_MIN_CHUNK: usize = 64;
const ZLIB_DEFAULT_MAX_OUTPUT: usize = 64 * 1024 * 1024;
const ZLIB_STREAM_MAX_INPUT: usize = 16 * 1024 * 1024;
const ZLIB_STREAM_MAX_RATIO: usize = 128;

fn brotli_param_num(rt: &mut Runtime, params: ObjectRef, key: &str) -> Option<f64> {
    match rt.object_get(params, key) {
        Value::Number(n) => Some(n),
        _ => None,
    }
}

fn parse_brotli_params(rt: &mut Runtime, opts: Option<&Value>) -> rusty_compression::BrotliParams {
    let mut p = rusty_compression::BrotliParams::default();
    if let Some(Value::Object(o)) = opts {
        if let Value::Object(params) = rt.object_get(*o, "params") {
            if let Some(v) = brotli_param_num(rt, params, "1") {
                p.quality = v.max(0.0) as u32;
            }
            if let Some(v) = brotli_param_num(rt, params, "2") {
                p.lgwin = v.max(0.0) as u32;
            }
            if let Some(v) = brotli_param_num(rt, params, "0") {
                p.mode = v.max(0.0) as u32;
            }
            if let Some(v) = brotli_param_num(rt, params, "5") {
                p.size_hint = v.max(0.0) as usize;
            }
            if let Some(v) = brotli_param_num(rt, params, "6") {
                p.large_window = v != 0.0;
            }
        }
    }
    p
}

fn zstd_compress_raw(input: &[u8]) -> Vec<u8> {
    let mut out = vec![0x28u8, 0xB5, 0x2F, 0xFD];
    let l = input.len() as u64;

    if l < 256 {
        out.push(0x20);
        out.push(l as u8);
    } else if l < 65536 + 256 {
        out.push(0x60);
        let v = (l - 256) as u16;
        out.extend_from_slice(&v.to_le_bytes());
    } else if l < 0x1_0000_0000 {
        out.push(0xA0);
        out.extend_from_slice(&(l as u32).to_le_bytes());
    } else {
        out.push(0xE0);
        out.extend_from_slice(&l.to_le_bytes());
    }
    const MAX_BLOCK: usize = 128 * 1024;
    let push_block = |out: &mut Vec<u8>, chunk: &[u8], last: bool| {

        let bh: u32 = ((chunk.len() as u32) << 3) | (last as u32);
        out.push((bh & 0xFF) as u8);
        out.push(((bh >> 8) & 0xFF) as u8);
        out.push(((bh >> 16) & 0xFF) as u8);
        out.extend_from_slice(chunk);
    };
    if input.is_empty() {
        push_block(&mut out, &[], true);
    } else {
        let mut i = 0;
        while i < input.len() {
            let end = (i + MAX_BLOCK).min(input.len());
            push_block(&mut out, &input[i..end], end == input.len());
            i = end;
        }
    }
    out
}

fn zstd_decompress_raw(input: &[u8]) -> Result<Vec<u8>, String> {
    if input.len() < 5 || input[0..4] != [0x28, 0xB5, 0x2F, 0xFD] {
        return Err("zstd: invalid frame magic".into());
    }
    let mut pos = 4usize;
    let fhd = input[pos];
    pos += 1;
    let single_segment = (fhd & 0x20) != 0;
    let dict_id_flag = fhd & 0x03;
    let fcs_flag = fhd >> 6;
    if !single_segment {
        pos += 1;
    }
    pos += match dict_id_flag {
        0 => 0,
        1 => 1,
        2 => 2,
        _ => 4,
    };
    pos += match fcs_flag {
        0 => usize::from(single_segment),
        1 => 2,
        2 => 4,
        _ => 8,
    };
    let mut out = Vec::new();
    loop {
        if pos + 3 > input.len() {
            return Err("zstd: truncated block header".into());
        }
        let bh =
            (input[pos] as u32) | ((input[pos + 1] as u32) << 8) | ((input[pos + 2] as u32) << 16);
        pos += 3;
        let last = (bh & 1) != 0;
        let btype = (bh >> 1) & 3;
        let bsize = (bh >> 3) as usize;
        match btype {
            0 => {
                if pos + bsize > input.len() {
                    return Err("zstd: truncated raw block".into());
                }
                out.extend_from_slice(&input[pos..pos + bsize]);
                pos += bsize;
            }
            1 => {
                if pos >= input.len() {
                    return Err("zstd: truncated RLE block".into());
                }
                let b = input[pos];
                pos += 1;
                out.extend(std::iter::repeat(b).take(bsize));
            }
            2 => return Err("zstd: compressed (FSE/Huffman) blocks are not yet supported".into()),
            _ => return Err("zstd: reserved block type".into()),
        }
        if last {
            break;
        }
    }
    Ok(out)
}

fn zstd_compress_res(input: &[u8]) -> Result<Vec<u8>, String> {
    Ok(zstd_compress_raw(input))
}

fn stub(name: &'static str) -> impl Fn(&mut Runtime, &[Value]) -> Result<Value, RuntimeError> {
    move |_rt, _args| {
        Err(RuntimeError::Thrown(Value::String(Rc::new(
            rusty_js_runtime::value::JsString::from(format!(
                "TypeError: node:zlib.{name} not yet implemented (Tier-Ω.5.y stub)"
            )),
        ))))
    }
}

fn zlib_arg_type_error(rt: &mut Runtime, value: &Value) -> RuntimeError {
    let received = match value {
        Value::Null => "null".to_string(),
        Value::Undefined => "undefined".to_string(),
        Value::Number(_) => format!(
            "type number ({})",
            rusty_js_runtime::abstract_ops::to_string(value).as_str()
        ),
        Value::Boolean(b) => format!("type boolean ({b})"),
        Value::Object(id) => {
            let ctor = match rt.object_get(*id, "constructor") {
                Value::Object(c) => match rt.object_get(c, "name") {
                    Value::String(s) => s.as_str().to_string(),
                    _ => "Object".to_string(),
                },
                _ => "Object".to_string(),
            };
            format!("an instance of {ctor}")
        }
        other => format!(
            "type {}",
            rusty_js_runtime::abstract_ops::to_string(other).as_str()
        ),
    };
    let msg = format!(
        "The \"buffer\" argument must be of type string or an instance of Buffer, \
         TypedArray, DataView, or ArrayBuffer. Received {received}"
    );
    let ctor = rt.global_get("TypeError");
    if let Ok(Value::Object(id)) = rt.construct(
        ctor,
        vec![Value::String(Rc::new(
            rusty_js_runtime::value::JsString::from(msg.clone()),
        ))],
    ) {
        rt.object_set(
            id,
            "code".into(),
            Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                "ERR_INVALID_ARG_TYPE",
            ))),
        );
        return RuntimeError::Thrown(Value::Object(id));
    }
    RuntimeError::TypeError(msg)
}

fn bytes_from_value(rt: &mut Runtime, value: &Value) -> Result<Vec<u8>, RuntimeError> {
    match value {
        Value::String(s) => Ok(s.as_bytes().to_vec()),
        Value::Object(id) => {

            let is_view = rt.typed_array_views.contains_key(id) || rt.obj(*id).is_buffer;
            if is_view {
                let len = match rt.object_get(*id, "length") {
                    Value::Number(n) if n >= 0.0 => n as usize,
                    _ => 0,
                };
                let mut bytes = Vec::with_capacity(len);
                for i in 0..len {
                    let b = match rt.object_get(*id, &i.to_string()) {
                        Value::Number(n) => n as u8,
                        Value::String(s) if !s.is_empty() => s.as_bytes()[0],
                        _ => 0,
                    };
                    bytes.push(b);
                }
                return Ok(bytes);
            }
            if let Some(rec) = rt.array_buffers.get(id) {
                return Ok(rec.data.clone());
            }
            Err(zlib_arg_type_error(rt, value))
        }
        _ => Err(zlib_arg_type_error(rt, value)),
    }
}

fn buffer_from_bytes(rt: &mut Runtime, bytes: &[u8]) -> Value {
    let id = rt.alloc_uint8_array_from_bytes(bytes);
    rt.obj_mut(id).is_buffer = true;
    rt.obj_mut(id)
        .set_own_internal("__is_buffer".into(), Value::Boolean(true));
    rt.obj_mut(id)
        .set_own_internal("__is_buffer__".into(), Value::Boolean(true));
    if let Value::Object(buffer_ctor) = rt.global_get("Buffer") {
        if let Value::Object(buffer_proto) = rt.object_get(buffer_ctor, "prototype") {
            rt.obj_mut(id).proto = Some(buffer_proto);
        }
    }
    Value::Object(id)
}

fn buffer_array_from_chunks(rt: &mut Runtime, chunks: &[Vec<u8>]) -> Value {
    let arr = rt.alloc_object(RtObject::new_array());
    for (i, chunk) in chunks.iter().enumerate() {
        let buf = buffer_from_bytes(rt, chunk);
        rt.object_set(arr, i.to_string(), buf);
    }
    rt.object_set(arr, "length".into(), Value::Number(chunks.len() as f64));
    Value::Object(arr)
}

fn numeric_option(rt: &mut Runtime, opts: Option<&Value>, name: &str) -> Option<f64> {
    match opts {
        Some(Value::Object(id)) => match rt.object_get(*id, name) {
            Value::Number(n) if n.is_finite() => Some(n),
            _ => None,
        },
        _ => None,
    }
}

fn zlib_chunk_size(rt: &mut Runtime, opts: Option<&Value>) -> usize {
    numeric_option(rt, opts, "chunkSize")
        .filter(|n| *n >= 0.0)
        .map(|n| n as usize)
        .unwrap_or(16 * 1024)
        .max(ZLIB_MIN_CHUNK)
}

fn write_bytes_into_buffer(rt: &mut Runtime, out: ObjectRef, offset: usize, bytes: &[u8]) {
    for (i, byte) in bytes.iter().enumerate() {
        rt.object_set(out, (offset + i).to_string(), Value::Number(*byte as f64));
    }
}

fn zlib_decode_error(rt: &mut Runtime, method: &str, err: impl std::fmt::Display) -> RuntimeError {
    RuntimeError::Thrown(zlib_error_value(rt, method, err))
}

fn zlib_decode_limit_for_input(input_len: usize) -> usize {
    ZLIB_DEFAULT_MAX_OUTPUT.min(
        input_len
            .saturating_mul(ZLIB_STREAM_MAX_RATIO)
            .max(ZLIB_MIN_CHUNK),
    )
}

fn check_zlib_stream_input_len(input_len: usize) -> Result<(), String> {
    if input_len > ZLIB_STREAM_MAX_INPUT {
        Err(format!(
            "compressed input exceeded node:zlib stream limit: {input_len} > {ZLIB_STREAM_MAX_INPUT}"
        ))
    } else {
        Ok(())
    }
}

fn check_zlib_decode_ratio(input_len: usize, output_len: usize) -> Result<(), String> {
    let max = zlib_decode_limit_for_input(input_len);
    if output_len > max {
        Err(format!(
            "decompressed output exceeded node:zlib ratio limit: {output_len} > {max}"
        ))
    } else {
        Ok(())
    }
}

fn finish_zlib_codec_with_policy(
    codec: StreamCodec,
    input_len: usize,
    decode: bool,
) -> Result<Vec<u8>, String> {
    let out = if decode {
        codec
            .finish_with_limit(zlib_decode_limit_for_input(input_len))
            .map_err(|err| err.to_string())?
    } else {
        codec.finish().map_err(|err| err.to_string())?
    };
    if decode {
        check_zlib_decode_ratio(input_len, out.len())?;
    }
    Ok(out)
}

fn zlib_error_value(rt: &mut Runtime, method: &str, message: impl std::fmt::Display) -> Value {

    let raw = format!("{message}");
    let is_truncation = raw == "unexpected end of file";

    let is_node_header_verbatim = matches!(
        raw.as_str(),
        "incorrect header check" | "unknown compression method" | "invalid window size"
    );
    let (code, errno) = if is_truncation {
        ("Z_BUF_ERROR", -5.0)
    } else {
        ("Z_DATA_ERROR", -3.0)
    };
    let msg = if is_node_header_verbatim {
        raw.clone()
    } else {
        format!("node:zlib.{method} failed: {message}")
    };
    match rusty_js_runtime::intrinsics::make_error_instance(rt, "Error", &msg) {
        Some(id) => {
            rt.object_set(
                id,
                "code".into(),
                Value::String(Rc::new(rusty_js_runtime::value::JsString::from(code))),
            );
            rt.object_set(id, "errno".into(), Value::Number(errno));
            Value::Object(id)
        }
        None => Value::String(Rc::new(rusty_js_runtime::value::JsString::from(msg))),
    }
}

fn register_sync_method(
    rt: &mut Runtime,
    host: rusty_js_runtime::ObjectRef,
    name: &'static str,
    op: fn(&[u8]) -> Result<Vec<u8>, String>,
) {
    register_method(rt, host, name, move |rt, args| {
        let info = zlib_options_info(rt, args);
        let input = bytes_from_value(rt, &args.first().cloned().unwrap_or(Value::Undefined))?;
        let out = op(&input).map_err(|err| zlib_decode_error(rt, name, err))?;
        if info {
            Ok(zlib_info_result(
                rt,
                host,
                &out,
                zlib_engine_class(name, true),
                zlib_stream_format_for_method(name),
                true,
                name.starts_with("brotli"),
            ))
        } else {
            Ok(buffer_from_bytes(rt, &out))
        }
    });
}

fn register_sync_encoder(
    rt: &mut Runtime,
    host: rusty_js_runtime::ObjectRef,
    name: &'static str,
    op: fn(&[u8]) -> Result<Vec<u8>, String>,
) {
    register_method(rt, host, name, move |rt, args| {
        let info = zlib_options_info(rt, args);
        let input = bytes_from_value(rt, &args.first().cloned().unwrap_or(Value::Undefined))?;
        let out = op(&input).map_err(|err| zlib_decode_error(rt, name, err))?;
        if info {
            Ok(zlib_info_result(
                rt,
                host,
                &out,
                zlib_engine_class(name, false),
                zlib_stream_format_for_method(name),
                false,
                name.starts_with("brotli"),
            ))
        } else {
            Ok(buffer_from_bytes(rt, &out))
        }
    });
}

fn gzip_header_error(bytes: &[u8]) -> Option<&'static str> {
    if bytes.len() >= 2 && (bytes[0] != 0x1f || bytes[1] != 0x8b) {
        return Some("incorrect header check");
    }
    if bytes.len() >= 3 && bytes[2] != 8 {
        return Some("unknown compression method");
    }
    None
}

fn zlib_header_error(bytes: &[u8]) -> Option<&'static str> {
    if bytes.len() >= 2 {
        let cmf = bytes[0];
        let flg = bytes[1];
        if ((cmf as u16) * 256 + flg as u16) % 31 != 0 {
            return Some("incorrect header check");
        }
        if (cmf & 0x0f) != 8 {
            return Some("unknown compression method");
        }
        if (cmf >> 4) > 7 {
            return Some("invalid window size");
        }
    }
    None
}

fn squeeze_gzip(bytes: &[u8]) -> Result<Vec<u8>, String> {
    press::squeeze_bytes(bytes, PressFormat::Gzip)
}

fn juice_gzip(bytes: &[u8]) -> Result<Vec<u8>, String> {
    if let Some(e) = gzip_header_error(bytes) {
        return Err(e.to_string());
    }
    press::juice_bytes_with_limit(bytes, PressFormat::Gzip, ZLIB_DEFAULT_MAX_OUTPUT)
}

fn juice_unzip_auto(bytes: &[u8]) -> Result<Vec<u8>, String> {
    if bytes.starts_with(&[0x1f, 0x8b]) {
        juice_gzip(bytes)
    } else {
        juice_zlib(bytes)
    }
}

fn squeeze_zlib(bytes: &[u8]) -> Result<Vec<u8>, String> {
    press::squeeze_bytes(bytes, PressFormat::Zlib)
}

fn juice_zlib(bytes: &[u8]) -> Result<Vec<u8>, String> {
    if let Some(e) = zlib_header_error(bytes) {
        return Err(e.to_string());
    }
    press::juice_bytes_with_limit(bytes, PressFormat::Zlib, ZLIB_DEFAULT_MAX_OUTPUT)
}

fn squeeze_deflate(bytes: &[u8]) -> Result<Vec<u8>, String> {
    press::squeeze_bytes(bytes, PressFormat::Deflate)
}

fn juice_deflate(bytes: &[u8]) -> Result<Vec<u8>, String> {
    press::juice_bytes_with_limit(bytes, PressFormat::Deflate, ZLIB_DEFAULT_MAX_OUTPUT)
}

fn juice_brotli(bytes: &[u8]) -> Result<Vec<u8>, String> {
    press::juice_bytes_with_limit(bytes, PressFormat::Brotli, ZLIB_DEFAULT_MAX_OUTPUT)
}

fn trailing_callback(rt: &Runtime, args: &[Value]) -> Option<Value> {
    args.last().cloned().filter(|v| rt.is_callable(v))
}

fn callback_roots(callback: &Option<Value>) -> Vec<ObjectRef> {
    match callback {
        Some(Value::Object(o)) => vec![*o],
        _ => Vec::new(),
    }
}

fn call_node_callback(rt: &mut Runtime, callback: Option<Value>, args: Vec<Value>) {
    if let Some(cb) = callback {
        let _ = rt.call_function(cb, Value::Undefined, args);
    }
}

fn zlib_options_info(rt: &mut Runtime, args: &[Value]) -> bool {
    matches!(
        args.get(1),
        Some(Value::Object(opts)) if matches!(rt.object_get(*opts, "info"), Value::Boolean(true))
    )
}

fn zlib_stream_format_for_method(name: &str) -> &'static str {
    match name {
        "deflate" | "inflate" | "deflateSync" | "inflateSync" => "zlib",
        "deflateRaw" | "inflateRaw" | "deflateRawSync" | "inflateRawSync" => "deflate-raw",
        "gzip" | "gunzip" | "gzipSync" | "gunzipSync" => "gzip",
        "unzip" | "unzipSync" => "auto",
        "brotliCompress" | "brotliDecompress" | "brotliCompressSync" | "brotliDecompressSync" => {
            "brotli"
        }
        _ => "zlib",
    }
}

fn zlib_class_for(format: &str, decode: bool, brotli: bool) -> &'static str {
    match (format, decode, brotli) {
        ("gzip", false, _) => "Gzip",
        ("gzip", true, _) => "Gunzip",
        ("deflate", false, _) => "Deflate",
        ("deflate", true, _) => "Inflate",
        ("deflate-raw", false, _) => "DeflateRaw",
        ("deflate-raw", true, _) => "InflateRaw",
        ("auto", _, _) => "Unzip",
        ("brotli", false, true) => "BrotliCompress",
        ("brotli", true, true) => "BrotliDecompress",
        _ => "Zlib",
    }
}

fn zlib_link_stream_class(
    rt: &mut Runtime,
    stream: ObjectRef,
    format: &str,
    decode: bool,
    brotli: bool,
) {
    let class = zlib_class_for(format, decode, brotli);
    let z = match rt.global_get("zlib") {
        Value::Object(z) => z,
        _ => return,
    };
    let ctor = match rt.object_get(z, class) {
        Value::Object(c) => c,
        _ => return,
    };
    let class_proto = match rt.object_get(ctor, "prototype") {
        Value::Object(p) => p,
        _ => return,
    };

    if let Value::Object(se) = rt.global_get("stream") {
        if let Value::Object(tctor) = rt.object_get(se, "Transform") {
            if let Value::Object(tproto) = rt.object_get(tctor, "prototype") {
                if !matches!(rt.obj(class_proto).proto, Some(p) if p == tproto) {
                    rt.set_object_prototype_internal(class_proto, Some(tproto));
                }
            }
        }
    }
    rt.set_object_prototype_internal(stream, Some(class_proto));
}

fn zlib_engine_class(name: &str, decode: bool) -> &'static str {
    match (name, decode) {
        ("gzip" | "gzipSync", false) => "Gzip",
        ("gunzip" | "gunzipSync", true) => "Gunzip",
        ("unzip" | "unzipSync", true) => "Unzip",
        ("deflate" | "deflateSync", false) => "Deflate",
        ("inflate" | "inflateSync", true) => "Inflate",
        ("deflateRaw" | "deflateRawSync", false) => "DeflateRaw",
        ("inflateRaw" | "inflateRawSync", true) => "InflateRaw",
        ("brotliCompress" | "brotliCompressSync", false) => "BrotliCompress",
        ("brotliDecompress" | "brotliDecompressSync", true) => "BrotliDecompress",
        _ => "Zlib",
    }
}

fn zlib_info_result(
    rt: &mut Runtime,
    host: ObjectRef,
    bytes: &[u8],
    class_name: &str,
    format: &'static str,
    decode: bool,
    brotli: bool,
) -> Value {
    let out = new_object(rt);
    let buffer = buffer_from_bytes(rt, bytes);
    let engine = make_zlib_stream(rt, format, decode, brotli);
    if let Value::Object(ctor) = rt.object_get(host, class_name) {
        if let Value::Object(proto) = rt.object_get(ctor, "prototype") {
            rt.set_object_prototype_internal(engine, Some(proto));
        }
    }
    rt.object_set(out, "buffer".into(), buffer);
    rt.object_set(out, "engine".into(), Value::Object(engine));
    Value::Object(out)
}

fn register_async_encoder(
    rt: &mut Runtime,
    host: ObjectRef,
    name: &'static str,
    op: fn(&[u8]) -> Result<Vec<u8>, String>,
) {
    register_method(rt, host, name, move |rt, args| {
        let callback = trailing_callback(rt, args);
        let info = zlib_options_info(rt, args);
        let input = bytes_from_value(rt, &args.first().cloned().unwrap_or(Value::Undefined))?;
        let roots = callback_roots(&callback);
        rt.enqueue_host_phase_rooted(
            HostEnqueuePhase::HostCompletionMacrotask,
            "zlib async encode completion",
            roots,
            move |rt| {
                match op(&input) {
                    Ok(bytes) => {
                        let out = if info {
                            zlib_info_result(
                                rt,
                                host,
                                &bytes,
                                zlib_engine_class(name, false),
                                zlib_stream_format_for_method(name),
                                false,
                                name.starts_with("brotli"),
                            )
                        } else {
                            buffer_from_bytes(rt, &bytes)
                        };
                        call_node_callback(rt, callback, vec![Value::Null, out]);
                    }
                    Err(err) => {
                        let ev = zlib_error_value(rt, name, err);
                        call_node_callback(rt, callback, vec![ev]);
                    }
                }
                Ok(())
            },
        );
        Ok(Value::Undefined)
    });
}

fn register_async_decoder(
    rt: &mut Runtime,
    host: ObjectRef,
    name: &'static str,
    op: fn(&[u8]) -> Result<Vec<u8>, String>,
) {
    register_method(rt, host, name, move |rt, args| {
        let callback = trailing_callback(rt, args);
        let info = zlib_options_info(rt, args);
        let input = bytes_from_value(rt, &args.first().cloned().unwrap_or(Value::Undefined))?;
        let roots = callback_roots(&callback);
        rt.enqueue_host_phase_rooted(
            HostEnqueuePhase::HostCompletionMacrotask,
            "zlib async decode completion",
            roots,
            move |rt| {
                match op(&input) {
                    Ok(bytes) => {
                        let out = if info {
                            zlib_info_result(
                                rt,
                                host,
                                &bytes,
                                zlib_engine_class(name, true),
                                zlib_stream_format_for_method(name),
                                true,
                                name.starts_with("brotli"),
                            )
                        } else {
                            buffer_from_bytes(rt, &bytes)
                        };
                        call_node_callback(rt, callback, vec![Value::Null, out]);
                    }
                    Err(err) => {
                        let ev = zlib_error_value(rt, name, err);
                        call_node_callback(rt, callback, vec![ev]);
                    }
                }
                Ok(())
            },
        );
        Ok(Value::Undefined)
    });
}

fn value_to_string_lossy(v: &Value) -> String {
    rusty_js_runtime::abstract_ops::to_string(v)
        .as_str()
        .to_string()
}

fn install_zlib_emitter(rt: &mut Runtime, obj: ObjectRef) {
    let registry = new_object(rt);
    rt.set_engine_sentinel(obj, ZLIB_LISTENERS_SLOT, Value::Object(registry));
    let on_impl = |rt: &mut Runtime, args: &[Value]| -> Result<Value, RuntimeError> {
        let this = match rt.current_this() {
            Value::Object(id) => id,
            _ => return Ok(Value::Undefined),
        };
        let event = args.first().map(value_to_string_lossy).unwrap_or_default();
        let listener = args.get(1).cloned().unwrap_or(Value::Undefined);
        if !rt.is_callable(&listener) {
            return Ok(Value::Object(this));
        }
        let registry = match rt.object_get(this, ZLIB_LISTENERS_SLOT) {
            Value::Object(id) => id,
            _ => return Ok(Value::Object(this)),
        };
        let arr = match rt.object_get(registry, &event) {
            Value::Object(id) => id,
            _ => {
                let id = rt.alloc_object(RtObject::new_array());
                rt.object_set(id, "length".into(), Value::Number(0.0));
                rt.object_set(registry, event.clone(), Value::Object(id));
                id
            }
        };
        let len = rt.array_length(arr);
        rt.object_set(arr, len.to_string(), listener);
        rt.object_set(arr, "length".into(), Value::Number((len + 1) as f64));
        Ok(Value::Object(this))
    };
    register_method(rt, obj, "on", on_impl);
    register_method(rt, obj, "once", on_impl);
    register_method(rt, obj, "addListener", on_impl);
    register_method(rt, obj, "removeListener", |rt, _args| Ok(rt.current_this()));
    register_method(rt, obj, "removeAllListeners", |rt, args| {
        let this = match rt.current_this() {
            Value::Object(id) => id,
            _ => return Ok(Value::Undefined),
        };
        let registry = match rt.object_get(this, ZLIB_LISTENERS_SLOT) {
            Value::Object(id) => id,
            _ => return Ok(Value::Object(this)),
        };
        if let Some(event) = args.first() {
            let event = value_to_string_lossy(event);
            let empty = rt.alloc_object(RtObject::new_array());
            rt.object_set(empty, "length".into(), Value::Number(0.0));
            rt.object_set(registry, event, Value::Object(empty));
        } else {
            let empty_registry = rt.alloc_object(RtObject::new_ordinary());
            rt.set_engine_sentinel(
                this,
                ZLIB_LISTENERS_SLOT.into(),
                Value::Object(empty_registry),
            );
        }
        Ok(Value::Object(this))
    });
}

fn zlib_emit(rt: &mut Runtime, obj: ObjectRef, event: &str, args: Vec<Value>) {
    if event == "data" {
        if let Some(chunk) = args.first() {
            if let Value::Object(chunks) = rt.object_get(obj, ZLIB_CHUNKS_SLOT) {
                let len = rt.array_length(chunks);
                rt.object_set(chunks, len.to_string(), chunk.clone());
                rt.object_set(chunks, "length".into(), Value::Number((len + 1) as f64));
            }
        }
    } else if event == "end" {
        rt.object_set(obj, ZLIB_ENDED_SLOT.into(), Value::Boolean(true));
    }
    let registry = match rt.object_get(obj, ZLIB_LISTENERS_SLOT) {
        Value::Object(id) => id,
        _ => return,
    };
    let arr = match rt.object_get(registry, event) {
        Value::Object(id) => id,
        _ => return,
    };
    let len = rt.array_length(arr);
    for i in 0..len {
        let cb = rt.object_get(arr, &i.to_string());
        if rt.is_callable(&cb) {
            let _ = rt.call_function(cb, Value::Object(obj), args.clone());
        }
    }
}

fn append_stream_input(rt: &mut Runtime, stream: ObjectRef, bytes: &[u8]) -> Result<(), String> {
    let mut input = match rt.object_get(stream, ZLIB_INPUT_SLOT) {
        Value::Object(id) => bytes_from_value(rt, &Value::Object(id)).unwrap_or_default(),
        _ => Vec::new(),
    };
    let next_len = input
        .len()
        .checked_add(bytes.len())
        .ok_or_else(|| "compressed input length overflow".to_string())?;
    check_zlib_stream_input_len(next_len)?;
    input.extend_from_slice(bytes);
    let buf = buffer_from_bytes(rt, &input);
    rt.object_set(stream, ZLIB_INPUT_SLOT.into(), buf);
    Ok(())
}

fn finish_stream(rt: &mut Runtime, stream: ObjectRef) {
    let input = match rt.object_get(stream, ZLIB_INPUT_SLOT) {
        Value::Object(id) => bytes_from_value(rt, &Value::Object(id)).unwrap_or_default(),
        _ => Vec::new(),
    };
    let brotli = matches!(
        rt.object_get(stream, ZLIB_BROTLI_SLOT),
        Value::Boolean(true)
    );
    let decode = matches!(
        rt.object_get(stream, ZLIB_DECODE_SLOT),
        Value::Boolean(true)
    );
    let is_zstd = matches!(
        rt.object_get(stream, ZLIB_FORMAT_SLOT),
        Value::String(ref s) if s.as_str() == "zstd"
    );
    let out = if is_zstd {

        if decode {
            zstd_decompress_raw(&input)
        } else {
            zstd_compress_res(&input)
        }
    } else if brotli {

        if decode {
            press::juice_bytes_with_limit(
                &input,
                PressFormat::Brotli,
                zlib_decode_limit_for_input(input.len()),
            )
            .and_then(|out| {
                check_zlib_decode_ratio(input.len(), out.len())?;
                Ok(out)
            })
        } else {

            let num = |rt: &mut Runtime, slot: &str, dflt: f64| match rt.object_get(stream, slot) {
                Value::Number(n) => n,
                _ => dflt,
            };
            let params = rusty_compression::BrotliParams {
                quality: num(rt, ZLIB_BR_QUALITY_SLOT, 11.0) as u32,
                lgwin: num(rt, ZLIB_BR_LGWIN_SLOT, 22.0) as u32,
                mode: num(rt, ZLIB_BR_MODE_SLOT, 0.0) as u32,
                size_hint: num(rt, ZLIB_BR_SIZEHINT_SLOT, 0.0) as usize,
                large_window: matches!(
                    rt.object_get(stream, ZLIB_BR_LARGEWIN_SLOT),
                    Value::Boolean(true)
                ),
            };
            press::squeeze_bytes_with_brotli_params(&input, PressFormat::Brotli, Some(&params))
        }
    } else {
        let format = stream_compression_format(rt, stream, &input);
        let mut codec = StreamCodec::new(format, decode);
        codec
            .push(&input)
            .map_err(|err| err.to_string())
            .and_then(|_| finish_zlib_codec_with_policy(codec, input.len(), decode))
    };
    match out {
        Ok(bytes) => {
            if !bytes.is_empty() {
                let chunk = buffer_from_bytes(rt, &bytes);
                zlib_emit(rt, stream, "data", vec![chunk.clone()]);
                let pipes = match rt.object_get(stream, ZLIB_PIPES_SLOT) {
                    Value::Object(id) => id,
                    _ => return,
                };
                let len = rt.array_length(pipes);
                for i in 0..len {
                    let dst = rt.object_get(pipes, &i.to_string());
                    if let Value::Object(dst_id) = dst {
                        let write = rt.object_get(dst_id, "write");
                        if rt.is_callable(&write) {
                            let _ =
                                rt.call_function(write, Value::Object(dst_id), vec![chunk.clone()]);
                        }
                    }
                }
            }
            zlib_emit(rt, stream, "end", Vec::new());
            zlib_emit(rt, stream, "close", Vec::new());
            let pipes = match rt.object_get(stream, ZLIB_PIPES_SLOT) {
                Value::Object(id) => id,
                _ => return,
            };
            let len = rt.array_length(pipes);
            for i in 0..len {
                let dst = rt.object_get(pipes, &i.to_string());
                if let Value::Object(dst_id) = dst {
                    let end = rt.object_get(dst_id, "end");
                    if rt.is_callable(&end) {
                        let _ = rt.call_function(end, Value::Object(dst_id), Vec::new());
                    }
                }
            }
        }
        Err(err) => {
            let ev = zlib_error_value(rt, "stream", err);
            zlib_emit(rt, stream, "error", vec![ev])
        }
    }
}

fn stream_compression_format(
    rt: &mut Runtime,
    stream: ObjectRef,
    input: &[u8],
) -> CompressionFormat {
    match rt.object_get(stream, ZLIB_FORMAT_SLOT) {
        Value::String(s) if s.as_str() == "gzip" => CompressionFormat::Gzip,
        Value::String(s) if s.as_str() == "deflate-raw" => CompressionFormat::Raw,
        Value::String(s) if s.as_str() == "auto" && input.starts_with(&[0x1f, 0x8b]) => {
            CompressionFormat::Gzip
        }
        _ => CompressionFormat::Zlib,
    }
}

fn compression_format_from_label(label: &str) -> CompressionFormat {
    match label {
        "gzip" => CompressionFormat::Gzip,
        "deflate-raw" => CompressionFormat::Raw,
        _ => CompressionFormat::Zlib,
    }
}

fn install_zlib_receiver_internals(
    rt: &mut Runtime,
    stream: ObjectRef,
    format: &'static str,
    decode: bool,
    brotli: bool,
    opts: Option<&Value>,
) {
    let chunk_size = zlib_chunk_size(rt, opts);
    install_zlib_emitter(rt, stream);
    let out_buffer = buffer_from_bytes(rt, &vec![0; chunk_size]);
    rt.object_set(
        stream,
        "_chunkSize".into(),
        Value::Number(chunk_size as f64),
    );
    rt.object_set(stream, "_outOffset".into(), Value::Number(0.0));
    rt.object_set(stream, "_offset".into(), Value::Number(0.0));
    rt.object_set(stream, "_outBuffer".into(), out_buffer.clone());
    rt.object_set(stream, "_buffer".into(), out_buffer);
    rt.object_set(stream, "_hadError".into(), Value::Boolean(false));
    rt.object_set(stream, "_finishFlushFlag".into(), Value::Number(4.0));
    if let Some(max_length) = numeric_option(rt, opts, "maxLength") {
        rt.object_set(stream, "_maxLength".into(), Value::Number(max_length));
    }

    let handle = new_object(rt);
    rt.obj_mut(handle)
        .set_own_internal("__zlib_handle__".into(), Value::Boolean(true));
    register_method(rt, handle, "writeSync", move |rt, args| {
        let input = args
            .get(1)
            .map(|value| bytes_from_value(rt, value))
            .transpose()?
            .unwrap_or_default();
        let in_off = match args.get(2) {
            Some(Value::Number(n)) if *n >= 0.0 => *n as usize,
            _ => 0,
        };
        let in_len = match args.get(3) {
            Some(Value::Number(n)) if *n >= 0.0 => *n as usize,
            _ => input.len().saturating_sub(in_off),
        };
        let out = match args.get(4) {
            Some(Value::Object(id)) => Some(*id),
            _ => None,
        };
        let out_off = match args.get(5) {
            Some(Value::Number(n)) if *n >= 0.0 => *n as usize,
            _ => 0,
        };
        let out_len = match args.get(6) {
            Some(Value::Number(n)) if *n >= 0.0 => *n as usize,
            _ => 0,
        };
        let start = in_off.min(input.len());
        let end = start.saturating_add(in_len).min(input.len());
        let input = &input[start..end];
        let decoded = if brotli {
            if decode {
                check_zlib_stream_input_len(input.len()).and_then(|_| {
                    press::juice_bytes_with_limit(
                        input,
                        PressFormat::Brotli,
                        zlib_decode_limit_for_input(input.len()),
                    )
                    .and_then(|out| {
                        check_zlib_decode_ratio(input.len(), out.len())?;
                        Ok(out)
                    })
                })
            } else {
                press::squeeze_bytes(input, PressFormat::Brotli)
            }
        } else {
            (if decode {
                check_zlib_stream_input_len(input.len())
            } else {
                Ok(())
            })
            .and_then(|_| {
                let mut codec = StreamCodec::new(compression_format_from_label(format), decode);
                codec
                    .push(input)
                    .map_err(|err| err.to_string())
                    .and_then(|_| finish_zlib_codec_with_policy(codec, input.len(), decode))
            })
        }
        .map_err(|err| zlib_decode_error(rt, "writeSync", err))?;
        let have = decoded.len().min(out_len);
        if let Some(out) = out {
            write_bytes_into_buffer(rt, out, out_off, &decoded[..have]);
        }
        let res = rt.alloc_object(RtObject::new_array());
        rt.object_set(res, "0".into(), Value::Number(0.0));
        rt.object_set(
            res,
            "1".into(),
            Value::Number(out_len.saturating_sub(have) as f64),
        );
        rt.object_set(res, "length".into(), Value::Number(2.0));
        Ok(Value::Object(res))
    });
    for m in ["write", "close", "init", "params", "reset"] {
        register_method(rt, handle, m, |_rt, _args| Ok(Value::Undefined));
    }
    rt.object_set(handle, "onerror".into(), Value::Null);
    let zlib_ctor = make_callable(rt, "Zlib", |_rt, _args| Ok(Value::Undefined));
    make_subclassable(rt, zlib_ctor, None);
    rt.object_set(handle, "constructor".into(), Value::Object(zlib_ctor));
    rt.object_set(stream, "_handle".into(), Value::Object(handle));
}

fn make_zlib_stream(
    rt: &mut Runtime,
    format: &'static str,
    decode: bool,
    brotli: bool,
) -> ObjectRef {
    crate::stream::ensure_installed(rt);
    let stream = new_object(rt);
    rt.obj_mut(stream)
        .set_own_internal("__zlib_stream__".into(), Value::Boolean(true));
    install_zlib_emitter(rt, stream);
    let pipes = rt.alloc_object(RtObject::new_array());
    rt.object_set(pipes, "length".into(), Value::Number(0.0));
    rt.object_set(stream, ZLIB_PIPES_SLOT.into(), Value::Object(pipes));
    let chunks = rt.alloc_object(RtObject::new_array());
    rt.object_set(chunks, "length".into(), Value::Number(0.0));
    rt.object_set(stream, ZLIB_CHUNKS_SLOT.into(), Value::Object(chunks));
    rt.object_set(stream, ZLIB_ENDED_SLOT.into(), Value::Boolean(false));
    let empty = buffer_from_bytes(rt, &[]);
    rt.object_set(stream, ZLIB_INPUT_SLOT.into(), empty);
    rt.object_set(
        stream,
        ZLIB_FORMAT_SLOT.into(),
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
            format.to_string(),
        ))),
    );
    rt.object_set(stream, ZLIB_DECODE_SLOT.into(), Value::Boolean(decode));
    rt.object_set(stream, ZLIB_BROTLI_SLOT.into(), Value::Boolean(brotli));
    install_zlib_receiver_internals(rt, stream, format, decode, brotli, None);

    register_method(rt, stream, "write", |rt, args| {
        let this = match rt.current_this() {
            Value::Object(id) => id,
            _ => return Ok(Value::Boolean(false)),
        };
        let bytes = match args.first() {
            Some(v) => bytes_from_value(rt, v)?,
            None => Vec::new(),
        };
        let prior = match rt.object_get(this, "bytesWritten") {
            Value::Number(n) => n,
            _ => 0.0,
        };
        rt.object_set(
            this,
            "bytesWritten".into(),
            Value::Number(prior + bytes.len() as f64),
        );
        append_stream_input(rt, this, &bytes).map_err(|err| zlib_decode_error(rt, "write", err))?;
        Ok(Value::Boolean(true))
    });
    register_method(rt, stream, "_processChunk", |rt, args| {
        let this = match rt.current_this() {
            Value::Object(id) => id,
            _ => return Ok(buffer_array_from_chunks(rt, &[])),
        };
        let bytes = match args.first() {
            Some(v) => bytes_from_value(rt, v)?,
            None => Vec::new(),
        };
        append_stream_input(rt, this, &bytes)
            .map_err(|err| zlib_decode_error(rt, "_processChunk", err))?;
        let flush_flag = match args.get(1) {
            Some(Value::Number(n)) => *n as i32,
            _ => 0,
        };
        if flush_flag != 4 {
            return Ok(buffer_array_from_chunks(rt, &[]));
        }
        let input = match rt.object_get(this, ZLIB_INPUT_SLOT) {
            Value::Object(id) => bytes_from_value(rt, &Value::Object(id)).unwrap_or_default(),
            _ => Vec::new(),
        };
        let brotli = matches!(rt.object_get(this, ZLIB_BROTLI_SLOT), Value::Boolean(true));
        let decode = matches!(rt.object_get(this, ZLIB_DECODE_SLOT), Value::Boolean(true));
        let out = if brotli {
            if decode {
                press::juice_bytes_with_limit(
                    &input,
                    PressFormat::Brotli,
                    zlib_decode_limit_for_input(input.len()),
                )
                .and_then(|out| {
                    check_zlib_decode_ratio(input.len(), out.len())?;
                    Ok(out)
                })
            } else {
                let num = |rt: &mut Runtime, slot: &str, dflt: f64| match rt.object_get(this, slot)
                {
                    Value::Number(n) => n,
                    _ => dflt,
                };
                let params = rusty_compression::BrotliParams {
                    quality: num(rt, ZLIB_BR_QUALITY_SLOT, 11.0) as u32,
                    lgwin: num(rt, ZLIB_BR_LGWIN_SLOT, 22.0) as u32,
                    mode: num(rt, ZLIB_BR_MODE_SLOT, 0.0) as u32,
                    size_hint: num(rt, ZLIB_BR_SIZEHINT_SLOT, 0.0) as usize,
                    large_window: matches!(
                        rt.object_get(this, ZLIB_BR_LARGEWIN_SLOT),
                        Value::Boolean(true)
                    ),
                };
                press::squeeze_bytes_with_brotli_params(&input, PressFormat::Brotli, Some(&params))
            }
        } else {
            let format = stream_compression_format(rt, this, &input);
            let mut codec = StreamCodec::new(format, decode);
            codec
                .push(&input)
                .map_err(|err| err.to_string())
                .and_then(|_| finish_zlib_codec_with_policy(codec, input.len(), decode))
        };
        match out {
            Ok(bytes) => {
                let empty = buffer_from_bytes(rt, &[]);
                rt.object_set(this, ZLIB_INPUT_SLOT.into(), empty);
                if bytes.is_empty() {
                    Ok(buffer_array_from_chunks(rt, &[]))
                } else {
                    Ok(buffer_array_from_chunks(rt, &[bytes]))
                }
            }
            Err(err) => Err(zlib_decode_error(rt, "_processChunk", err)),
        }
    });
    register_method(rt, stream, "end", |rt, args| {
        let this = match rt.current_this() {
            Value::Object(id) => id,
            _ => return Ok(Value::Undefined),
        };
        if let Some(v) = args.first() {
            if !rt.is_callable(v) {
                let bytes = bytes_from_value(rt, v)?;
                append_stream_input(rt, this, &bytes)
                    .map_err(|err| zlib_decode_error(rt, "end", err))?;
            }
        }
        rt.enqueue_host_phase_rooted(
            HostEnqueuePhase::HostCompletionMacrotask,
            "zlib stream finish",
            vec![this],
            move |rt| {
                finish_stream(rt, this);
                Ok(())
            },
        );
        Ok(Value::Undefined)
    });
    register_method(rt, stream, "pipe", |rt, args| {
        let this = match rt.current_this() {
            Value::Object(id) => id,
            _ => return Ok(Value::Undefined),
        };
        let dst = args.first().cloned().unwrap_or(Value::Undefined);
        if matches!(dst, Value::Object(_)) {
            let pipes = match rt.object_get(this, ZLIB_PIPES_SLOT) {
                Value::Object(id) => id,
                _ => return Ok(dst),
            };
            let len = rt.array_length(pipes);
            rt.object_set(pipes, len.to_string(), dst.clone());
            rt.object_set(pipes, "length".into(), Value::Number((len + 1) as f64));
        }
        Ok(dst)
    });
    register_method(rt, stream, "toArray", |rt, _args| {
        let this = match rt.current_this() {
            Value::Object(id) => id,
            _ => return Ok(Value::Undefined),
        };
        let promise = rusty_js_runtime::promise::new_promise(rt);
        let chunks = match rt.object_get(this, ZLIB_CHUNKS_SLOT) {
            Value::Object(id) => id,
            _ => {
                let id = rt.alloc_object(RtObject::new_array());
                rt.object_set(id, "length".into(), Value::Number(0.0));
                id
            }
        };
        if matches!(rt.object_get(this, ZLIB_ENDED_SLOT), Value::Boolean(true)) {
            rusty_js_runtime::promise::resolve_promise(rt, promise, Value::Object(chunks));
            return Ok(Value::Object(promise));
        }
        let resolve_on_end = crate::register::make_callable_rooted(
            rt,
            "zlibToArrayResolve",
            vec![promise, chunks],
            move |rt, _args| {
                rusty_js_runtime::promise::resolve_promise(rt, promise, Value::Object(chunks));
                Ok(Value::Undefined)
            },
        );
        let on = rt.object_get(this, "once");
        if rt.is_callable(&on) {
            let _ = rt.call_function(
                on,
                Value::Object(this),
                vec![
                    Value::String(Rc::new(rusty_js_runtime::value::JsString::from("end"))),
                    Value::Object(resolve_on_end),
                ],
            );
        }
        Ok(Value::Object(promise))
    });
    register_method(rt, stream, "destroy", |rt, _args| Ok(rt.current_this()));
    register_method(rt, stream, "pause", |rt, _args| Ok(rt.current_this()));
    register_method(rt, stream, "resume", |rt, _args| Ok(rt.current_this()));

    rt.object_set(stream, "bytesWritten".into(), Value::Number(0.0));
    register_method(rt, stream, "flush", zlib_stream_trailing_cb);
    register_method(rt, stream, "params", zlib_stream_trailing_cb);
    register_method(rt, stream, "reset", |rt, _args| Ok(rt.current_this()));
    register_method(rt, stream, "close", |rt, args| {
        let this = match rt.current_this() {
            Value::Object(id) => id,
            _ => return Ok(Value::Undefined),
        };
        zlib_stream_trailing_cb(rt, args)?;
        rt.enqueue_host_phase_rooted(
            HostEnqueuePhase::HostCompletionMacrotask,
            "zlib stream close",
            vec![this],
            move |rt| {
                zlib_emit(rt, this, "close", Vec::new());
                Ok(())
            },
        );
        Ok(Value::Undefined)
    });

    crate::stream::install_async_iterator(rt, stream);

    zlib_link_stream_class(rt, stream, format, decode, brotli);

    stream
}

fn zlib_stream_trailing_cb(
    rt: &mut Runtime,
    args: &[Value],
) -> Result<Value, rusty_js_runtime::RuntimeError> {
    if let Some(cb) = args.iter().rev().find(|v| rt.is_callable(v)).cloned() {
        let roots: Vec<_> = match &cb {
            Value::Object(id) => vec![*id],
            _ => vec![],
        };
        rt.enqueue_host_phase_rooted(
            HostEnqueuePhase::HostCompletionMacrotask,
            "zlib stream flush cb",
            roots,
            move |rt| {
                let _ = rt.call_function(cb, Value::Undefined, vec![]);
                Ok(())
            },
        );
    }
    Ok(Value::Undefined)
}

fn register_stream_constructor(
    rt: &mut Runtime,
    host: ObjectRef,
    name: &'static str,
    format: &'static str,
    decode: bool,
    brotli: bool,
) {
    register_method(rt, host, name, move |rt, _args| {
        Ok(Value::Object(make_zlib_stream(rt, format, decode, brotli)))
    });
}

pub fn install(rt: &mut Runtime) {
    let z = new_object(rt);
    register_async_encoder(rt, z, "deflate", squeeze_zlib);
    register_async_decoder(rt, z, "inflate", juice_zlib);
    register_async_encoder(rt, z, "gzip", squeeze_gzip);
    register_async_decoder(rt, z, "gunzip", juice_gzip);

    register_method(rt, z, "brotliCompress", move |rt, args| {
        let callback = trailing_callback(rt, args);
        let info = zlib_options_info(rt, args);
        let input = bytes_from_value(rt, &args.first().cloned().unwrap_or(Value::Undefined))?;

        let opts = args
            .get(1)
            .filter(|v| matches!(v, Value::Object(_)) && !rt.is_callable(v))
            .cloned();
        let params = parse_brotli_params(rt, opts.as_ref());
        let roots = callback_roots(&callback);
        rt.enqueue_host_phase_rooted(
            HostEnqueuePhase::HostCompletionMacrotask,
            "zlib brotli async compress",
            roots,
            move |rt| {
                match press::squeeze_bytes_with_brotli_params(
                    &input,
                    PressFormat::Brotli,
                    Some(&params),
                ) {
                    Ok(bytes) => {
                        let out = if info {
                            zlib_info_result(rt, z, &bytes, "BrotliCompress", "brotli", false, true)
                        } else {
                            buffer_from_bytes(rt, &bytes)
                        };
                        call_node_callback(rt, callback, vec![Value::Null, out]);
                    }
                    Err(err) => {
                        let ev = zlib_error_value(rt, "brotliCompress", err);
                        call_node_callback(rt, callback, vec![ev]);
                    }
                }
                Ok(())
            },
        );
        Ok(Value::Undefined)
    });
    register_async_decoder(rt, z, "brotliDecompress", juice_brotli);
    register_sync_encoder(rt, z, "deflateSync", squeeze_zlib);
    register_sync_encoder(rt, z, "deflateRawSync", squeeze_deflate);
    register_sync_encoder(rt, z, "gzipSync", squeeze_gzip);
    register_sync_method(rt, z, "inflateSync", juice_zlib);
    register_sync_method(rt, z, "inflateRawSync", juice_deflate);
    register_sync_method(rt, z, "gunzipSync", juice_gzip);
    register_sync_method(rt, z, "brotliDecompressSync", juice_brotli);

    register_method(rt, z, "brotliCompressSync", move |rt, args| {
        let info = zlib_options_info(rt, args);
        let input = bytes_from_value(rt, &args.first().cloned().unwrap_or(Value::Undefined))?;
        let params = parse_brotli_params(rt, args.get(1));
        let out =
            press::squeeze_bytes_with_brotli_params(&input, PressFormat::Brotli, Some(&params))
                .map_err(|err| zlib_decode_error(rt, "brotliCompressSync", err))?;
        if info {
            Ok(zlib_info_result(
                rt,
                z,
                &out,
                "BrotliCompress",
                "brotli",
                false,
                true,
            ))
        } else {
            Ok(buffer_from_bytes(rt, &out))
        }
    });
    register_async_encoder(rt, z, "deflateRaw", squeeze_deflate);
    register_async_decoder(rt, z, "inflateRaw", juice_deflate);
    register_stream_constructor(rt, z, "createDeflate", "deflate", false, false);
    register_stream_constructor(rt, z, "createInflate", "deflate", true, false);
    register_stream_constructor(rt, z, "createGzip", "gzip", false, false);
    register_stream_constructor(rt, z, "createGunzip", "gzip", true, false);
    register_stream_constructor(rt, z, "createUnzip", "auto", true, false);
    register_stream_constructor(rt, z, "createDeflateRaw", "deflate-raw", false, false);
    register_stream_constructor(rt, z, "createInflateRaw", "deflate-raw", true, false);

    register_method(rt, z, "createBrotliCompress", |rt, args| {
        let params = parse_brotli_params(rt, args.first());
        let stream = make_zlib_stream(rt, "brotli", false, true);
        rt.object_set(
            stream,
            ZLIB_BR_QUALITY_SLOT.into(),
            Value::Number(params.quality as f64),
        );
        rt.object_set(
            stream,
            ZLIB_BR_LGWIN_SLOT.into(),
            Value::Number(params.lgwin as f64),
        );
        rt.object_set(
            stream,
            ZLIB_BR_MODE_SLOT.into(),
            Value::Number(params.mode as f64),
        );
        rt.object_set(
            stream,
            ZLIB_BR_SIZEHINT_SLOT.into(),
            Value::Number(params.size_hint as f64),
        );
        rt.object_set(
            stream,
            ZLIB_BR_LARGEWIN_SLOT.into(),
            Value::Boolean(params.large_window),
        );
        Ok(Value::Object(stream))
    });
    register_stream_constructor(rt, z, "createBrotliDecompress", "brotli", true, true);

    for (name, format, decode, brotli) in [
        ("Zlib", "zlib", false, false),
        ("Deflate", "zlib", false, false),
        ("Inflate", "zlib", true, false),
        ("DeflateRaw", "deflate-raw", false, false),
        ("InflateRaw", "deflate-raw", true, false),
        ("Gzip", "gzip", false, false),
        ("Gunzip", "gzip", true, false),
        ("Unzip", "auto", true, false),
        ("BrotliCompress", "brotli", false, true),
        ("BrotliDecompress", "brotli", true, true),
    ] {
        register_method(rt, z, name, move |rt, args| {
            if let Value::Object(receiver) = rt.current_this() {
                install_zlib_receiver_internals(rt, receiver, format, decode, brotli, args.first());
            }
            let stream = make_zlib_stream(rt, format, decode, brotli);
            if let Value::Object(ctor) = rt.object_get(z, name) {
                if let Value::Object(proto) = rt.object_get(ctor, "prototype") {
                    rt.set_object_prototype_internal(stream, Some(proto));
                }
            }
            Ok(Value::Object(stream))
        });
        if let Value::Object(ctor) = rt.object_get(z, name) {
            let proto = new_object(rt);
            rt.set_own_frozen_property(ctor, "prototype".into(), Value::Object(proto));
            rt.obj_mut(proto)
                .set_own_internal("constructor".into(), Value::Object(ctor));
        }
    }

    let constants = new_object(rt);
    let pairs: &[(&str, f64)] = &[

        ("Z_NO_FLUSH", 0.0),
        ("Z_PARTIAL_FLUSH", 1.0),
        ("Z_SYNC_FLUSH", 2.0),
        ("Z_FULL_FLUSH", 3.0),
        ("Z_FINISH", 4.0),
        ("Z_BLOCK", 5.0),
        ("Z_TREES", 6.0),

        ("Z_OK", 0.0),
        ("Z_STREAM_END", 1.0),
        ("Z_NEED_DICT", 2.0),
        ("Z_ERRNO", -1.0),
        ("Z_STREAM_ERROR", -2.0),
        ("Z_DATA_ERROR", -3.0),
        ("Z_MEM_ERROR", -4.0),
        ("Z_BUF_ERROR", -5.0),
        ("Z_VERSION_ERROR", -6.0),

        ("Z_NO_COMPRESSION", 0.0),
        ("Z_BEST_SPEED", 1.0),
        ("Z_BEST_COMPRESSION", 9.0),
        ("Z_DEFAULT_COMPRESSION", -1.0),

        ("Z_FILTERED", 1.0),
        ("Z_HUFFMAN_ONLY", 2.0),
        ("Z_RLE", 3.0),
        ("Z_FIXED", 4.0),
        ("Z_DEFAULT_STRATEGY", 0.0),

        ("Z_BINARY", 0.0),
        ("Z_TEXT", 1.0),
        ("Z_ASCII", 1.0),
        ("Z_UNKNOWN", 2.0),
        ("Z_DEFLATED", 8.0),

        ("DEFLATE", 1.0),
        ("INFLATE", 2.0),
        ("GZIP", 3.0),
        ("GUNZIP", 4.0),
        ("DEFLATERAW", 5.0),
        ("INFLATERAW", 6.0),
        ("UNZIP", 7.0),
        ("BROTLI_DECODE", 8.0),
        ("BROTLI_ENCODE", 9.0),

        ("Z_DEFAULT_WINDOWBITS", 15.0),
        ("Z_MIN_WINDOWBITS", 8.0),
        ("Z_MAX_WINDOWBITS", 15.0),
        ("Z_MIN_CHUNK", 64.0),
        ("Z_MAX_CHUNK", f64::INFINITY),
        ("Z_DEFAULT_CHUNK", 16384.0),
        ("Z_MIN_MEMLEVEL", 1.0),
        ("Z_MAX_MEMLEVEL", 9.0),
        ("Z_DEFAULT_MEMLEVEL", 8.0),
        ("Z_MIN_LEVEL", -1.0),
        ("Z_MAX_LEVEL", 9.0),
        ("Z_DEFAULT_LEVEL", -1.0),

        ("BROTLI_OPERATION_PROCESS", 0.0),
        ("BROTLI_OPERATION_FLUSH", 1.0),
        ("BROTLI_OPERATION_FINISH", 2.0),
        ("BROTLI_OPERATION_EMIT_METADATA", 3.0),

        ("BROTLI_PARAM_MODE", 0.0),
        ("BROTLI_MODE_GENERIC", 0.0),
        ("BROTLI_MODE_TEXT", 1.0),
        ("BROTLI_MODE_FONT", 2.0),
        ("BROTLI_DEFAULT_MODE", 0.0),
        ("BROTLI_PARAM_QUALITY", 1.0),
        ("BROTLI_MIN_QUALITY", 0.0),
        ("BROTLI_MAX_QUALITY", 11.0),
        ("BROTLI_DEFAULT_QUALITY", 11.0),
        ("BROTLI_PARAM_LGWIN", 2.0),
        ("BROTLI_MIN_WINDOW_BITS", 10.0),
        ("BROTLI_MAX_WINDOW_BITS", 24.0),
        ("BROTLI_LARGE_MAX_WINDOW_BITS", 30.0),
        ("BROTLI_DEFAULT_WINDOW", 22.0),
        ("BROTLI_PARAM_LGBLOCK", 3.0),
        ("BROTLI_MIN_INPUT_BLOCK_BITS", 16.0),
        ("BROTLI_MAX_INPUT_BLOCK_BITS", 24.0),
        ("BROTLI_PARAM_DISABLE_LITERAL_CONTEXT_MODELING", 4.0),
        ("BROTLI_PARAM_SIZE_HINT", 5.0),
        ("BROTLI_PARAM_LARGE_WINDOW", 6.0),
        ("BROTLI_PARAM_NPOSTFIX", 7.0),
        ("BROTLI_PARAM_NDIRECT", 8.0),

        ("BROTLI_DECODER_RESULT_ERROR", 0.0),
        ("BROTLI_DECODER_RESULT_SUCCESS", 1.0),
        ("BROTLI_DECODER_RESULT_NEEDS_MORE_INPUT", 2.0),
        ("BROTLI_DECODER_RESULT_NEEDS_MORE_OUTPUT", 3.0),
        ("BROTLI_DECODER_PARAM_DISABLE_RING_BUFFER_REALLOCATION", 0.0),
        ("BROTLI_DECODER_PARAM_LARGE_WINDOW", 1.0),

        ("BROTLI_DECODER_NO_ERROR", 0.0),
        ("BROTLI_DECODER_SUCCESS", 1.0),
        ("BROTLI_DECODER_NEEDS_MORE_INPUT", 2.0),
        ("BROTLI_DECODER_NEEDS_MORE_OUTPUT", 3.0),
        ("BROTLI_DECODER_ERROR_FORMAT_EXUBERANT_NIBBLE", -1.0),
        ("BROTLI_DECODER_ERROR_FORMAT_RESERVED", -2.0),
        ("BROTLI_DECODER_ERROR_FORMAT_EXUBERANT_META_NIBBLE", -3.0),
        ("BROTLI_DECODER_ERROR_FORMAT_SIMPLE_HUFFMAN_ALPHABET", -4.0),
        ("BROTLI_DECODER_ERROR_FORMAT_SIMPLE_HUFFMAN_SAME", -5.0),
        ("BROTLI_DECODER_ERROR_FORMAT_CL_SPACE", -6.0),
        ("BROTLI_DECODER_ERROR_FORMAT_HUFFMAN_SPACE", -7.0),
        ("BROTLI_DECODER_ERROR_FORMAT_CONTEXT_MAP_REPEAT", -8.0),
        ("BROTLI_DECODER_ERROR_FORMAT_BLOCK_LENGTH_1", -9.0),
        ("BROTLI_DECODER_ERROR_FORMAT_BLOCK_LENGTH_2", -10.0),
        ("BROTLI_DECODER_ERROR_FORMAT_TRANSFORM", -11.0),
        ("BROTLI_DECODER_ERROR_FORMAT_DICTIONARY", -12.0),
        ("BROTLI_DECODER_ERROR_FORMAT_WINDOW_BITS", -13.0),
        ("BROTLI_DECODER_ERROR_FORMAT_PADDING_1", -14.0),
        ("BROTLI_DECODER_ERROR_FORMAT_PADDING_2", -15.0),
        ("BROTLI_DECODER_ERROR_FORMAT_DISTANCE", -16.0),
        ("BROTLI_DECODER_ERROR_DICTIONARY_NOT_SET", -19.0),
        ("BROTLI_DECODER_ERROR_INVALID_ARGUMENTS", -20.0),
        ("BROTLI_DECODER_ERROR_ALLOC_CONTEXT_MODES", -21.0),
        ("BROTLI_DECODER_ERROR_ALLOC_TREE_GROUPS", -22.0),
        ("BROTLI_DECODER_ERROR_ALLOC_CONTEXT_MAP", -25.0),
        ("BROTLI_DECODER_ERROR_ALLOC_RING_BUFFER_1", -26.0),
        ("BROTLI_DECODER_ERROR_ALLOC_RING_BUFFER_2", -27.0),
        ("BROTLI_DECODER_ERROR_ALLOC_BLOCK_TYPE_TREES", -30.0),
        ("BROTLI_DECODER_ERROR_UNREACHABLE", -31.0),

        ("ZLIB_VERNUM", 4880.0),
        ("ZSTD_CLEVEL_DEFAULT", 3.0),
        ("ZSTD_COMPRESS", 10.0),
        ("ZSTD_DECOMPRESS", 11.0),
        ("ZSTD_fast", 1.0),
        ("ZSTD_dfast", 2.0),
        ("ZSTD_greedy", 3.0),
        ("ZSTD_lazy", 4.0),
        ("ZSTD_lazy2", 5.0),
        ("ZSTD_btlazy2", 6.0),
        ("ZSTD_btopt", 7.0),
        ("ZSTD_btultra", 8.0),
        ("ZSTD_btultra2", 9.0),
        ("ZSTD_c_compressionLevel", 100.0),
        ("ZSTD_c_windowLog", 101.0),
        ("ZSTD_c_hashLog", 102.0),
        ("ZSTD_c_chainLog", 103.0),
        ("ZSTD_c_searchLog", 104.0),
        ("ZSTD_c_minMatch", 105.0),
        ("ZSTD_c_targetLength", 106.0),
        ("ZSTD_c_strategy", 107.0),
        ("ZSTD_c_enableLongDistanceMatching", 160.0),
        ("ZSTD_c_ldmHashLog", 161.0),
        ("ZSTD_c_ldmMinMatch", 162.0),
        ("ZSTD_c_ldmBucketSizeLog", 163.0),
        ("ZSTD_c_ldmHashRateLog", 164.0),
        ("ZSTD_c_contentSizeFlag", 200.0),
        ("ZSTD_c_checksumFlag", 201.0),
        ("ZSTD_c_dictIDFlag", 202.0),
        ("ZSTD_c_nbWorkers", 400.0),
        ("ZSTD_c_jobSize", 401.0),
        ("ZSTD_c_overlapLog", 402.0),
        ("ZSTD_d_windowLogMax", 100.0),
        ("ZSTD_e_continue", 0.0),
        ("ZSTD_e_flush", 1.0),
        ("ZSTD_e_end", 2.0),
        ("ZSTD_error_no_error", 0.0),
        ("ZSTD_error_GENERIC", 1.0),
        ("ZSTD_error_prefix_unknown", 10.0),
        ("ZSTD_error_version_unsupported", 12.0),
        ("ZSTD_error_frameParameter_unsupported", 14.0),
        ("ZSTD_error_frameParameter_windowTooLarge", 16.0),
        ("ZSTD_error_corruption_detected", 20.0),
        ("ZSTD_error_checksum_wrong", 22.0),
        ("ZSTD_error_literals_headerWrong", 24.0),
        ("ZSTD_error_dictionary_corrupted", 30.0),
        ("ZSTD_error_dictionary_wrong", 32.0),
        ("ZSTD_error_dictionaryCreation_failed", 34.0),
        ("ZSTD_error_parameter_unsupported", 40.0),
        ("ZSTD_error_parameter_combination_unsupported", 41.0),
        ("ZSTD_error_parameter_outOfBound", 42.0),
        ("ZSTD_error_tableLog_tooLarge", 44.0),
        ("ZSTD_error_maxSymbolValue_tooLarge", 46.0),
        ("ZSTD_error_maxSymbolValue_tooSmall", 48.0),
        ("ZSTD_error_stabilityCondition_notRespected", 50.0),
        ("ZSTD_error_stage_wrong", 60.0),
        ("ZSTD_error_init_missing", 62.0),
        ("ZSTD_error_memory_allocation", 64.0),
        ("ZSTD_error_workSpace_tooSmall", 66.0),
        ("ZSTD_error_dstSize_tooSmall", 70.0),
        ("ZSTD_error_srcSize_wrong", 72.0),
        ("ZSTD_error_dstBuffer_null", 74.0),
        ("ZSTD_error_noForwardProgress_destFull", 80.0),
        ("ZSTD_error_noForwardProgress_inputEmpty", 82.0),
    ];
    let codes = new_object(rt);
    for (name, val) in pairs {
        rt.object_set(constants, name.to_string(), Value::Number(*val));
        if name.starts_with("Z_") {
            rt.object_set(codes, name.to_string(), Value::Number(*val));
        }
    }
    let _ = rt.object_freeze_via(&Value::Object(constants));
    let _ = rt.object_freeze_via(&Value::Object(codes));
    rt.set_own_frozen_property(z, "constants".into(), Value::Object(constants));
    rt.set_own_frozen_property(z, "codes".into(), Value::Object(codes));

    register_method(rt, z, "crc32", |rt, args| {
        let bytes: Vec<u8> = match args.first() {
            Some(Value::String(s)) => s.as_bytes().to_vec(),
            Some(Value::Object(id)) => {
                let len = rt.array_length(*id);
                (0..len)
                    .map(|i| {
                        if let Value::Number(n) = rt.object_get(*id, &i.to_string()) {
                            n as u8
                        } else {
                            0
                        }
                    })
                    .collect()
            }
            _ => Vec::new(),
        };
        Ok(Value::Number(rusty_compression::crc32(&bytes) as f64))
    });
    register_method(rt, z, "ZstdCompress", stub("ZstdCompress"));
    register_method(rt, z, "ZstdDecompress", stub("ZstdDecompress"));
    register_stream_constructor(rt, z, "createZstdCompress", "zstd", false, false);
    register_stream_constructor(rt, z, "createZstdDecompress", "zstd", true, false);
    register_async_decoder(rt, z, "unzip", juice_unzip_auto);
    register_sync_method(rt, z, "unzipSync", juice_unzip_auto);
    register_async_encoder(rt, z, "zstdCompress", zstd_compress_res);
    register_method(rt, z, "zstdCompressSync", |rt, args| {
        let input = bytes_from_value(rt, &args.first().cloned().unwrap_or(Value::Undefined))?;
        Ok(buffer_from_bytes(rt, &zstd_compress_raw(&input)))
    });
    register_async_decoder(rt, z, "zstdDecompress", zstd_decompress_raw);
    register_method(rt, z, "zstdDecompressSync", |rt, args| {
        let input = bytes_from_value(rt, &args.first().cloned().unwrap_or(Value::Undefined))?;
        match zstd_decompress_raw(&input) {
            Ok(out) => Ok(buffer_from_bytes(rt, &out)),
            Err(msg) => Err(RuntimeError::TypeError(format!(
                "node:zlib.zstdDecompressSync failed: {msg}"
            ))),
        }
    });
    let _ = rt.delete_own_via(
        &Value::Object(z),
        &Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
            "Zlib".to_string(),
        ))),
    );
    rt.define_global_property("zlib", Value::Object(z));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zlib_stream_input_policy_rejects_over_cap_and_overflow() {
        assert!(check_zlib_stream_input_len(ZLIB_STREAM_MAX_INPUT).is_ok());
        assert!(check_zlib_stream_input_len(ZLIB_STREAM_MAX_INPUT + 1).is_err());
    }

    #[test]
    fn zlib_stream_ratio_policy_scales_from_input() {
        assert_eq!(
            zlib_decode_limit_for_input(1),
            ZLIB_STREAM_MAX_RATIO.min(ZLIB_DEFAULT_MAX_OUTPUT)
        );
        assert!(check_zlib_decode_ratio(10, 10 * ZLIB_STREAM_MAX_RATIO).is_ok());
        assert!(check_zlib_decode_ratio(10, 10 * ZLIB_STREAM_MAX_RATIO + 1).is_err());
        assert!(zlib_decode_limit_for_input(usize::MAX) <= ZLIB_DEFAULT_MAX_OUTPUT);
    }
}
