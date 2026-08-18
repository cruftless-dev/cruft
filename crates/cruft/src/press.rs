
use crate::register::{new_object, register_method};
use rusty_js_runtime::value::JsString;
use rusty_js_runtime::{Runtime, RuntimeError, Value};
use std::rc::Rc;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PressFormat {
    Gzip,
    Zlib,
    Deflate,
    Brotli,
}

impl PressFormat {
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "gzip" => Some(Self::Gzip),
            "zlib" => Some(Self::Zlib),
            "deflate" => Some(Self::Deflate),
            "brotli" => Some(Self::Brotli),
            _ => None,
        }
    }
}

pub fn squeeze_bytes_with_brotli_params(
    bytes: &[u8],
    format: PressFormat,
    brotli_params: Option<&rusty_compression::BrotliParams>,
) -> Result<Vec<u8>, String> {
    match format {
        PressFormat::Gzip => Ok(rusty_compression::gzip_deflate_node_default(bytes)),
        PressFormat::Zlib => Ok(rusty_compression::zlib_deflate_stored(bytes)),
        PressFormat::Deflate => Ok(rusty_compression::deflate_best(bytes)),
        PressFormat::Brotli => match brotli_params {
            Some(params) => rusty_compression::brotli_encode_params(bytes, params)
                .map_err(|err| err.to_string()),
            None => rusty_compression::brotli_encode(bytes, 11, 22).map_err(|err| err.to_string()),
        },
    }
}

pub fn squeeze_bytes(bytes: &[u8], format: PressFormat) -> Result<Vec<u8>, String> {
    squeeze_bytes_with_brotli_params(bytes, format, None)
}

pub fn juice_bytes(bytes: &[u8], format: PressFormat) -> Result<Vec<u8>, String> {
    juice_bytes_with_limit(bytes, format, rusty_compression::MAX_OUTPUT)
}

pub fn juice_bytes_with_limit(
    bytes: &[u8],
    format: PressFormat,
    max_output: usize,
) -> Result<Vec<u8>, String> {
    match format {
        PressFormat::Gzip => {
            rusty_compression::gunzip_with_limit(bytes, max_output).map_err(|err| err.to_string())
        }
        PressFormat::Zlib => rusty_compression::zlib_inflate_with_limit(bytes, max_output)
            .map_err(|err| err.to_string()),
        PressFormat::Deflate => {
            rusty_compression::inflate_with_limit(bytes, max_output).map_err(|err| err.to_string())
        }
        PressFormat::Brotli => rusty_compression::brotli_decode_with_limit(bytes, max_output)
            .map_err(|err| err.to_string()),
    }
}

fn bytes_from_value(rt: &mut Runtime, value: &Value) -> Result<Vec<u8>, RuntimeError> {
    match value {
        Value::String(s) => Ok(s.as_bytes().to_vec()),
        Value::Object(id) => {
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
            Ok(bytes)
        }
        Value::Undefined | Value::Null => Err(RuntimeError::TypeError(
            "cruft:press: bytes must be a byte array or string".into(),
        )),
        other => {
            let s = rusty_js_runtime::abstract_ops::to_string(other);
            Ok(s.as_bytes().to_vec())
        }
    }
}

fn bytes_to_uint8_array(rt: &mut Runtime, bytes: &[u8]) -> Value {
    Value::Object(rt.alloc_uint8_array_from_bytes(bytes))
}

fn format_from_options(
    rt: &mut Runtime,
    opts: Option<&Value>,
) -> Result<PressFormat, RuntimeError> {
    let Some(Value::Object(o)) = opts else {
        return Err(RuntimeError::TypeError(
            "cruft:press: options.as must name a press format".into(),
        ));
    };
    match rt.object_get(*o, "as") {
        Value::String(s) => PressFormat::parse(s.as_str()).ok_or_else(|| {
            RuntimeError::TypeError(format!("cruft:press: unsupported format {}", s.as_str()))
        }),
        _ => Err(RuntimeError::TypeError(
            "cruft:press: options.as must name a press format".into(),
        )),
    }
}

fn press_error(method: &str, err: impl std::fmt::Display) -> RuntimeError {
    RuntimeError::Thrown(Value::String(Rc::new(JsString::from(format!(
        "Error: cruft:press.{method} failed: {err}"
    )))))
}

pub fn install(rt: &mut Runtime) {
    let ns = new_object(rt);

    register_method(rt, ns, "squeeze", |rt, args| {
        let bytes = bytes_from_value(rt, &args.first().cloned().unwrap_or(Value::Undefined))?;
        let format = format_from_options(rt, args.get(1))?;
        let out = squeeze_bytes(&bytes, format).map_err(|err| press_error("squeeze", err))?;
        Ok(bytes_to_uint8_array(rt, &out))
    });

    register_method(rt, ns, "juice", |rt, args| {
        let bytes = bytes_from_value(rt, &args.first().cloned().unwrap_or(Value::Undefined))?;
        let format = format_from_options(rt, args.get(1))?;
        let out = juice_bytes(&bytes, format).map_err(|err| press_error("juice", err))?;
        Ok(bytes_to_uint8_array(rt, &out))
    });

    rt.define_global_property("__cruft_press", Value::Object(ns));
}
