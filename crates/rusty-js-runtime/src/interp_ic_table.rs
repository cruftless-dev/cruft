
use crate::value::Value;
use crate::Runtime;
use std::rc::Rc;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    OnceLock,
};

pub struct IhiEntry {
    pub key: &'static str,
    pub receiver: IhiReceiverKind,

    pub arity: Option<u8>,
    pub cached_id_field: IhiCachedField,

    pub fast: fn(rt: &mut Runtime, recv: &Value, args: &[Value]) -> Option<Value>,
}

unsafe impl Sync for IhiEntry {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IhiReceiverKind {
    String,
    #[allow(dead_code)]
    Array,
    #[allow(dead_code)]
    Number,

    Buffer,

    RegExp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IhiCachedField {

    RegexpExec,
    StringCharCodeAt,
    StringCharAt,

    StringToLowerCase,

    StringTrim,

    StringIndexOf,

    ArrayIndexOf,

    ObjectKeys,

    StringCodePointAt,

    StringToUpperCase,

    StringStartsWith,

    StringEndsWith,

    StringIncludes,

    BufferGeneric,

    BufferWriteUInt8,

    BufferWriteInt8,

    BufferReadUInt8,

    BufferReadInt8,

    BufferWriteUInt16BE,

    BufferWriteUInt16LE,

    BufferWriteInt16BE,

    BufferWriteInt16LE,

    BufferReadUInt16BE,

    BufferReadUInt16LE,

    BufferReadInt16BE,

    BufferReadInt16LE,

    BufferWriteUInt32BE,

    BufferReadUInt32BE,

    BufferWriteUInt32LE,

    BufferWriteInt32BE,

    BufferWriteInt32LE,

    BufferReadUInt32LE,

    BufferReadInt32BE,

    BufferReadInt32LE,
    BufferWriteFloatBE,
    BufferWriteFloatLE,
    BufferWriteDoubleBE,
    BufferWriteDoubleLE,
    BufferReadFloatBE,
    BufferReadFloatLE,
    BufferReadDoubleBE,
    BufferReadDoubleLE,
    BufferWriteBigUInt64BE,
    BufferWriteBigUInt64LE,
    BufferWriteBigInt64BE,
    BufferWriteBigInt64LE,
    BufferReadBigUInt64BE,
    BufferReadBigUInt64LE,
    BufferReadBigInt64BE,
    BufferReadBigInt64LE,
}

fn fast_string_char_code_at(_rt: &mut Runtime, recv: &Value, args: &[Value]) -> Option<Value> {
    if args.len() != 1 {
        return None;
    }
    if let Value::String(s) = recv {
        let pos = &args[0];
        let i_n = match pos {
            Value::Undefined => 0.0,
            Value::Number(n) => *n,
            _ => f64::NAN,
        };
        if i_n.is_finite() && i_n >= 0.0 {
            let i = i_n as usize;
            if let Some(unit) = s.latin1_code_unit_at(i) {
                return Some(match unit {
                    Some(unit) => Value::Number(unit as f64),
                    None => Value::Number(f64::NAN),
                });
            }
            let bytes = s.as_bytes();
            let result = if s.is_ascii() {
                if i < bytes.len() {
                    Value::Number(bytes[i] as f64)
                } else {
                    Value::Number(f64::NAN)
                }
            } else {

                match s.code_unit_at(i) {
                    Some(unit) => Value::Number(unit as f64),
                    None => Value::Number(f64::NAN),
                }
            };
            return Some(result);
        }

        None
    } else {
        None
    }
}

pub(crate) const fn generated_string_char_code_at_ihi_entry() -> IhiEntry {
    crate::native_api_manifest_generated::generated_string_char_code_at_ihi_entry(
        fast_string_char_code_at,
    )
}

fn fast_string_char_at(_rt: &mut Runtime, recv: &Value, args: &[Value]) -> Option<Value> {
    if args.len() != 1 {
        return None;
    }
    let Value::String(s) = recv else {
        return None;
    };
    let i_n = match &args[0] {
        Value::Undefined => 0.0,
        Value::Number(n) => *n,
        _ => return None,
    };
    let pos_int = if i_n.is_nan() { 0.0 } else { i_n.trunc() };
    let out = if pos_int < 0.0 || !pos_int.is_finite() {
        crate::value::JsString::from(String::new())
    } else {
        s.code_unit_as_string(pos_int as usize)
            .unwrap_or_else(|| crate::value::JsString::from(String::new()))
    };
    Some(Value::String(Rc::new(out)))
}

fn fast_string_code_point_at(_rt: &mut Runtime, recv: &Value, args: &[Value]) -> Option<Value> {
    if args.len() != 1 {
        return None;
    }
    if let Value::String(s) = recv {
        let pos = &args[0];
        let i_n = match pos {
            Value::Undefined => 0.0,
            Value::Number(n) => *n,
            _ => return None,
        };
        if i_n.is_infinite() || i_n < 0.0 {
            return Some(Value::Undefined);
        }
        let i = if i_n.is_nan() { 0 } else { i_n as usize };
        let first = s.code_unit_at(i)?;
        if (0xD800..=0xDBFF).contains(&first) {
            if let Some(second) = s.code_unit_at(i + 1) {
                if (0xDC00..=0xDFFF).contains(&second) {
                    let high = (first as u32) - 0xD800;
                    let low = (second as u32) - 0xDC00;
                    return Some(Value::Number((0x10000 + ((high << 10) | low)) as f64));
                }
            }
        }
        Some(Value::Number(first as f64))
    } else {
        None
    }
}

fn fast_string_to_lower_case(_rt: &mut Runtime, recv: &Value, args: &[Value]) -> Option<Value> {
    if !args.is_empty() {
        return None;
    }
    if let Value::String(s) = recv {
        if s.is_ascii() {
            let bytes = s.as_bytes();
            let mut out = Vec::with_capacity(bytes.len());
            for &b in bytes {
                out.push(if (b'A'..=b'Z').contains(&b) {
                    b + 32
                } else {
                    b
                });
            }

            let lowered = unsafe { String::from_utf8_unchecked(out) };
            return Some(Value::String(std::rc::Rc::new(
                crate::value::JsString::from(lowered),
            )));
        }
        if s.code_unit_len() == 1 {
            if let Some(ch) = s.as_wellformed().and_then(|s| s.chars().next()) {
                let lowered: String = ch.to_lowercase().collect();
                return Some(Value::String(std::rc::Rc::new(
                    crate::value::JsString::from(lowered),
                )));
            }
        }

        None
    } else {
        None
    }
}

fn fast_string_trim(_rt: &mut Runtime, recv: &Value, args: &[Value]) -> Option<Value> {
    if !args.is_empty() {
        return None;
    }
    if let Value::String(s) = recv {
        let bytes = s.as_bytes();
        let is_ws = |b: u8| matches!(b, b' ' | b'\t' | b'\n' | b'\r' | 0x0B | 0x0C);

        if !s.is_ascii() {
            return None;
        }
        let mut start = 0;
        while start < bytes.len() && is_ws(bytes[start]) {
            start += 1;
        }
        let mut end = bytes.len();
        while end > start && is_ws(bytes[end - 1]) {
            end -= 1;
        }
        if start == 0 && end == bytes.len() {

            return Some(Value::String(s.clone()));
        }
        let trimmed = unsafe { std::str::from_utf8_unchecked(&bytes[start..end]) }.to_owned();
        Some(Value::String(std::rc::Rc::new(
            crate::value::JsString::from(trimmed),
        )))
    } else {
        None
    }
}

fn fast_string_index_of_1(_rt: &mut Runtime, recv: &Value, args: &[Value]) -> Option<Value> {
    if args.len() != 1 {
        return None;
    }
    if let (Value::String(s), Value::String(needle)) = (recv, &args[0]) {
        if s.is_ascii() && needle.is_ascii() {
            let s_bytes = s.as_bytes();
            let n_bytes = needle.as_bytes();
            if n_bytes.is_empty() {
                return Some(Value::Number(0.0));
            }
            if n_bytes.len() > s_bytes.len() {
                return Some(Value::Number(-1.0));
            }
            match s_bytes.windows(n_bytes.len()).position(|w| w == n_bytes) {
                Some(p) => Some(Value::Number(p as f64)),
                None => Some(Value::Number(-1.0)),
            }
        } else {

            None
        }
    } else {
        None
    }
}

fn fast_array_index_of_1(rt: &mut Runtime, recv: &Value, args: &[Value]) -> Option<Value> {
    if args.len() != 1 {
        return None;
    }
    let Value::Object(id) = recv else {
        return None;
    };
    let store_len = {
        let obj = rt.obj(*id);
        if !matches!(obj.internal_kind, crate::value::InternalKind::Array) || !obj.array_dense {
            return None;
        }
        obj.array_store_len()
    };
    if store_len != rt.array_length(*id) as usize {
        return None;
    }
    let needle = &args[0];
    for i in 0..store_len {
        let candidate = rt.obj(*id).array_store_get(i);
        if crate::abstract_ops::is_strictly_equal(&candidate, needle) {
            return Some(Value::Number(i as f64));
        }
    }
    Some(Value::Number(-1.0))
}

fn fast_object_keys_1(rt: &mut Runtime, _recv: &Value, args: &[Value]) -> Option<Value> {
    if args.len() != 1 {
        return None;
    }
    let Value::Object(id) = args.first()? else {
        return None;
    };
    match rt.proxy_target_handler_checked(*id) {
        Ok(Some(_)) | Err(_) => return None,
        Ok(None) => {}
    }
    crate::generated::object_keys(rt, Value::Undefined, args).ok()
}

fn fast_string_to_upper_case(_rt: &mut Runtime, recv: &Value, args: &[Value]) -> Option<Value> {
    if !args.is_empty() {
        return None;
    }
    if let Value::String(s) = recv {
        if s.is_ascii() {
            let bytes = s.as_bytes();
            let mut out = Vec::with_capacity(bytes.len());
            for &b in bytes {
                out.push(if (b'a'..=b'z').contains(&b) {
                    b - 32
                } else {
                    b
                });
            }
            let upper = unsafe { String::from_utf8_unchecked(out) };
            return Some(Value::String(std::rc::Rc::new(
                crate::value::JsString::from(upper),
            )));
        }
        if s.code_unit_len() == 1 {
            if let Some(ch) = s.as_wellformed().and_then(|s| s.chars().next()) {
                let upper: String = ch.to_uppercase().collect();
                return Some(Value::String(std::rc::Rc::new(
                    crate::value::JsString::from(upper),
                )));
            }
        }
        None
    } else {
        None
    }
}

fn fast_string_starts_with(_rt: &mut Runtime, recv: &Value, args: &[Value]) -> Option<Value> {
    if args.len() != 1 {
        return None;
    }
    if let (Value::String(s), Value::String(prefix)) = (recv, &args[0]) {
        if s.is_ascii() && prefix.is_ascii() {
            let s_bytes = s.as_bytes();
            let p_bytes = prefix.as_bytes();
            if p_bytes.len() > s_bytes.len() {
                return Some(Value::Boolean(false));
            }
            return Some(Value::Boolean(&s_bytes[..p_bytes.len()] == p_bytes));
        }
        None
    } else {
        None
    }
}

fn fast_string_ends_with(_rt: &mut Runtime, recv: &Value, args: &[Value]) -> Option<Value> {
    if args.len() != 1 {
        return None;
    }
    if let (Value::String(s), Value::String(suffix)) = (recv, &args[0]) {
        if s.is_ascii() && suffix.is_ascii() {
            let s_bytes = s.as_bytes();
            let f_bytes = suffix.as_bytes();
            if f_bytes.len() > s_bytes.len() {
                return Some(Value::Boolean(false));
            }
            let off = s_bytes.len() - f_bytes.len();
            return Some(Value::Boolean(&s_bytes[off..] == f_bytes));
        }
        None
    } else {
        None
    }
}

fn fast_string_includes(_rt: &mut Runtime, recv: &Value, args: &[Value]) -> Option<Value> {
    if args.len() != 1 {
        return None;
    }
    if let (Value::String(s), Value::String(needle)) = (recv, &args[0]) {
        if s.is_ascii() && needle.is_ascii() {
            let s_bytes = s.as_bytes();
            let n_bytes = needle.as_bytes();
            if n_bytes.is_empty() {
                return Some(Value::Boolean(true));
            }
            if n_bytes.len() > s_bytes.len() {
                return Some(Value::Boolean(false));
            }
            return Some(Value::Boolean(
                s_bytes.windows(n_bytes.len()).any(|w| w == n_bytes),
            ));
        }
        None
    } else {
        None
    }
}

fn buffer_view_of(rt: &Runtime, recv: &Value) -> Option<(rusty_js_gc::ObjectId, usize)> {
    if let Value::Object(id) = recv {
        let v = rt.typed_array_views.get(id)?;
        Some((v.buffer, v.byte_offset))
    } else {
        None
    }
}

macro_rules! buf_write {
    ($fn:ident, $ty:ty, $w:literal, $be:expr) => {
        fn $fn(rt: &mut Runtime, recv: &Value, args: &[Value]) -> Option<Value> {
            record_buffer_ihi_probe(stringify!($fn), "call");
            if args.len() != 2 {
                record_buffer_ihi_probe(stringify!($fn), "bail-arity");
                return None;
            }
            let value = match args[0] {
                Value::Number(n)
                    if n.fract() == 0.0 && n >= <$ty>::MIN as f64 && n <= <$ty>::MAX as f64 =>
                {
                    n as $ty
                }
                _ => {
                    record_buffer_ihi_probe(stringify!($fn), "bail-value");
                    return None;
                }
            };
            let offset = match args[1] {
                Value::Number(n) if n >= 0.0 && n.fract() == 0.0 => n as usize,
                Value::Undefined => 0,
                _ => {
                    record_buffer_ihi_probe(stringify!($fn), "bail-offset");
                    return None;
                }
            };
            let (buf_id, base) = match buffer_view_of(rt, recv) {
                Some(view) => view,
                None => {
                    record_buffer_ihi_probe(stringify!($fn), "bail-receiver");
                    return None;
                }
            };
            let ab = match rt.array_buffers.get_mut(&buf_id) {
                Some(ab) => ab,
                None => {
                    record_buffer_ihi_probe(stringify!($fn), "bail-backing");
                    return None;
                }
            };
            if ab.detached {
                record_buffer_ihi_probe(stringify!($fn), "bail-detached");
                return None;
            }
            let abs = match base.checked_add(offset) {
                Some(abs) => abs,
                None => {
                    record_buffer_ihi_probe(stringify!($fn), "bail-overflow");
                    return None;
                }
            };
            if abs.checked_add($w).map_or(true, |end| end > ab.byte_length) {
                record_buffer_ihi_probe(stringify!($fn), "bail-oob");
                return None;
            }
            let bytes = if $be {
                value.to_be_bytes()
            } else {
                value.to_le_bytes()
            };
            ab.data[abs..abs + $w].copy_from_slice(&bytes);
            record_buffer_ihi_probe(stringify!($fn), "hit");
            Some(Value::Number((offset + $w) as f64))
        }
    };
}
macro_rules! buf_read {
    ($fn:ident, $ty:ty, $w:literal, $be:expr) => {
        fn $fn(rt: &mut Runtime, recv: &Value, args: &[Value]) -> Option<Value> {
            if args.len() != 1 {
                return None;
            }
            let offset = match args[0] {
                Value::Number(n) if n >= 0.0 && n.fract() == 0.0 => n as usize,
                Value::Undefined => 0,
                _ => return None,
            };
            let (buf_id, base) = buffer_view_of(rt, recv)?;
            let ab = rt.array_buffers.get(&buf_id)?;
            if ab.detached {
                return None;
            }
            let abs = base.checked_add(offset)?;
            if abs.checked_add($w)? > ab.byte_length {
                return None;
            }
            let mut tmp = [0u8; $w];
            tmp.copy_from_slice(&ab.data[abs..abs + $w]);
            let v = if $be {
                <$ty>::from_be_bytes(tmp)
            } else {
                <$ty>::from_le_bytes(tmp)
            };
            Some(Value::Number(v as f64))
        }
    };
}

macro_rules! buf_write_float {
    ($fn:ident, $ty:ty, $w:literal, $be:expr) => {
        fn $fn(rt: &mut Runtime, recv: &Value, args: &[Value]) -> Option<Value> {
            if args.len() != 2 {
                return None;
            }
            let value = match args[0] {
                Value::Number(n) => n as $ty,
                _ => return None,
            };
            let offset = match args[1] {
                Value::Number(n) if n >= 0.0 && n.fract() == 0.0 => n as usize,
                Value::Undefined => 0,
                _ => return None,
            };
            let (buf_id, base) = buffer_view_of(rt, recv)?;
            let ab = rt.array_buffers.get_mut(&buf_id)?;
            if ab.detached {
                return None;
            }
            let abs = base.checked_add(offset)?;
            if abs.checked_add($w)? > ab.byte_length {
                return None;
            }
            let bytes = if $be {
                value.to_be_bytes()
            } else {
                value.to_le_bytes()
            };
            ab.data[abs..abs + $w].copy_from_slice(&bytes);
            Some(Value::Number((offset + $w) as f64))
        }
    };
}

buf_write!(fast_buffer_write_u8, u8, 1, true);
buf_write!(fast_buffer_write_u16be, u16, 2, true);
buf_write!(fast_buffer_write_u16le, u16, 2, false);
buf_write!(fast_buffer_write_u32be, u32, 4, true);
buf_write!(fast_buffer_write_u32le, u32, 4, false);
buf_write!(fast_buffer_write_i8, i8, 1, true);
buf_write!(fast_buffer_write_i16be, i16, 2, true);
buf_write!(fast_buffer_write_i16le, i16, 2, false);
buf_write!(fast_buffer_write_i32be, i32, 4, true);
buf_write!(fast_buffer_write_i32le, i32, 4, false);
buf_write_float!(fast_buffer_write_f32be, f32, 4, true);
buf_write_float!(fast_buffer_write_f32le, f32, 4, false);
buf_write_float!(fast_buffer_write_f64be, f64, 8, true);
buf_write_float!(fast_buffer_write_f64le, f64, 8, false);

buf_read!(fast_buffer_read_u8, u8, 1, true);
buf_read!(fast_buffer_read_u16be, u16, 2, true);
buf_read!(fast_buffer_read_u16le, u16, 2, false);
buf_read!(fast_buffer_read_u32be, u32, 4, true);
buf_read!(fast_buffer_read_u32le, u32, 4, false);
buf_read!(fast_buffer_read_i8, i8, 1, true);
buf_read!(fast_buffer_read_i16be, i16, 2, true);
buf_read!(fast_buffer_read_i16le, i16, 2, false);
buf_read!(fast_buffer_read_i32be, i32, 4, true);
buf_read!(fast_buffer_read_i32le, i32, 4, false);
buf_read!(fast_buffer_read_f32be, f32, 4, true);
buf_read!(fast_buffer_read_f32le, f32, 4, false);
buf_read!(fast_buffer_read_f64be, f64, 8, true);
buf_read!(fast_buffer_read_f64le, f64, 8, false);

macro_rules! buf_write_big {
    ($fn:ident, $ty:ty, $be:expr) => {
        fn $fn(rt: &mut Runtime, recv: &Value, args: &[Value]) -> Option<Value> {
            if args.len() != 2 {
                return None;
            }
            let value_i128 = match &args[0] {
                Value::BigInt(big) => big.to_decimal().parse::<i128>().ok()?,
                _ => return None,
            };
            let value: $ty = value_i128.try_into().ok()?;
            let offset = match args[1] {
                Value::Number(n) if n >= 0.0 && n.fract() == 0.0 => n as usize,
                Value::Undefined => 0,
                _ => return None,
            };
            let (buf_id, base) = buffer_view_of(rt, recv)?;
            let ab = rt.array_buffers.get_mut(&buf_id)?;
            if ab.detached {
                return None;
            }
            let abs = base.checked_add(offset)?;
            if abs.checked_add(8)? > ab.byte_length {
                return None;
            }
            let bytes = if $be {
                value.to_be_bytes()
            } else {
                value.to_le_bytes()
            };
            ab.data[abs..abs + 8].copy_from_slice(&bytes);
            Some(Value::Number((offset + 8) as f64))
        }
    };
}

macro_rules! buf_read_big {
    ($fn:ident, $ty:ty, $ctor:ident, $be:expr) => {
        fn $fn(rt: &mut Runtime, recv: &Value, args: &[Value]) -> Option<Value> {
            if args.len() != 1 {
                return None;
            }
            let offset = match args[0] {
                Value::Number(n) if n >= 0.0 && n.fract() == 0.0 => n as usize,
                Value::Undefined => 0,
                _ => return None,
            };
            let (buf_id, base) = buffer_view_of(rt, recv)?;
            let ab = rt.array_buffers.get(&buf_id)?;
            if ab.detached {
                return None;
            }
            let abs = base.checked_add(offset)?;
            if abs.checked_add(8)? > ab.byte_length {
                return None;
            }
            let mut tmp = [0u8; 8];
            tmp.copy_from_slice(&ab.data[abs..abs + 8]);
            let value = if $be {
                <$ty>::from_be_bytes(tmp)
            } else {
                <$ty>::from_le_bytes(tmp)
            };
            Some(Value::BigInt(Rc::new(crate::bigint::JsBigInt::$ctor(
                value,
            ))))
        }
    };
}

buf_write_big!(fast_buffer_write_big_u64be, u64, true);
buf_write_big!(fast_buffer_write_big_u64le, u64, false);
buf_write_big!(fast_buffer_write_big_i64be, i64, true);
buf_write_big!(fast_buffer_write_big_i64le, i64, false);
buf_read_big!(fast_buffer_read_big_u64be, u64, from_u64, true);
buf_read_big!(fast_buffer_read_big_u64le, u64, from_u64, false);
buf_read_big!(fast_buffer_read_big_i64be, i64, from_i64, true);
buf_read_big!(fast_buffer_read_big_i64le, i64, from_i64, false);

fn record_buffer_ihi_probe(method: &'static str, outcome: &'static str) {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    if !*ENABLED.get_or_init(|| {
        std::env::var("CRUFT_BUFFER_IHI_COUNTERS")
            .map(|v| v != "0")
            .unwrap_or(false)
    }) {
        return;
    }
    static CALLS: AtomicU64 = AtomicU64::new(0);
    let calls = CALLS.fetch_add(1, Ordering::Relaxed) + 1;
    if calls <= 16 || calls.is_power_of_two() {
        eprintln!("[buffer-ihi] calls={calls} method={method} outcome={outcome}");
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CachedDispatch {
    NotCached,
    NoMatch,
    Entry(u8),
}

pub static IHI_TABLE: &[IhiEntry] = &[

    IhiEntry {
        key: "exec",
        receiver: IhiReceiverKind::RegExp,
        arity: Some(1),
        cached_id_field: IhiCachedField::RegexpExec,
        fast: crate::regexp::ihi_fast_regexp_exec,
    },
    generated_string_char_code_at_ihi_entry(),
    IhiEntry {
        key: "charAt",
        receiver: IhiReceiverKind::String,
        arity: Some(1),
        cached_id_field: IhiCachedField::StringCharAt,
        fast: fast_string_char_at,
    },
    crate::native_api_manifest_generated::generated_string_to_lower_case_ihi_entry(
        fast_string_to_lower_case,
    ),
    crate::native_api_manifest_generated::generated_string_trim_ihi_entry(fast_string_trim),
    crate::native_api_manifest_generated::generated_string_index_of_ihi_entry(
        fast_string_index_of_1,
    ),
    IhiEntry {
        key: "indexOf",
        receiver: IhiReceiverKind::Array,
        arity: Some(1),
        cached_id_field: IhiCachedField::ArrayIndexOf,
        fast: fast_array_index_of_1,
    },
    IhiEntry {
        key: "keys",
        receiver: IhiReceiverKind::Array,
        arity: Some(1),
        cached_id_field: IhiCachedField::ObjectKeys,
        fast: fast_object_keys_1,
    },
    crate::native_api_manifest_generated::generated_string_code_point_at_ihi_entry(
        fast_string_code_point_at,
    ),
    crate::native_api_manifest_generated::generated_string_to_upper_case_ihi_entry(
        fast_string_to_upper_case,
    ),
    crate::native_api_manifest_generated::generated_string_starts_with_ihi_entry(
        fast_string_starts_with,
    ),
    crate::native_api_manifest_generated::generated_string_ends_with_ihi_entry(
        fast_string_ends_with,
    ),
    crate::native_api_manifest_generated::generated_string_includes_ihi_entry(fast_string_includes),

    crate::native_api_manifest_generated::generated_buffer_write_u_int8_ihi_entry(
        fast_buffer_write_u8,
    ),
    crate::native_api_manifest_generated::generated_buffer_write_u_int16_be_ihi_entry(
        fast_buffer_write_u16be,
    ),
    crate::native_api_manifest_generated::generated_buffer_write_u_int16_le_ihi_entry(
        fast_buffer_write_u16le,
    ),
    crate::native_api_manifest_generated::generated_buffer_write_u_int32_be_ihi_entry(
        fast_buffer_write_u32be,
    ),
    crate::native_api_manifest_generated::generated_buffer_write_u_int32_le_ihi_entry(
        fast_buffer_write_u32le,
    ),
    crate::native_api_manifest_generated::generated_buffer_write_int8_ihi_entry(
        fast_buffer_write_i8,
    ),
    crate::native_api_manifest_generated::generated_buffer_write_int16_be_ihi_entry(
        fast_buffer_write_i16be,
    ),
    crate::native_api_manifest_generated::generated_buffer_write_int16_le_ihi_entry(
        fast_buffer_write_i16le,
    ),
    crate::native_api_manifest_generated::generated_buffer_write_int32_be_ihi_entry(
        fast_buffer_write_i32be,
    ),
    crate::native_api_manifest_generated::generated_buffer_write_int32_le_ihi_entry(
        fast_buffer_write_i32le,
    ),
    crate::native_api_manifest_generated::generated_buffer_read_u_int8_ihi_entry(
        fast_buffer_read_u8,
    ),
    crate::native_api_manifest_generated::generated_buffer_read_u_int16_be_ihi_entry(
        fast_buffer_read_u16be,
    ),
    crate::native_api_manifest_generated::generated_buffer_read_u_int16_le_ihi_entry(
        fast_buffer_read_u16le,
    ),
    crate::native_api_manifest_generated::generated_buffer_read_u_int32_be_ihi_entry(
        fast_buffer_read_u32be,
    ),
    crate::native_api_manifest_generated::generated_buffer_read_u_int32_le_ihi_entry(
        fast_buffer_read_u32le,
    ),
    crate::native_api_manifest_generated::generated_buffer_read_int8_ihi_entry(fast_buffer_read_i8),
    crate::native_api_manifest_generated::generated_buffer_read_int16_be_ihi_entry(
        fast_buffer_read_i16be,
    ),
    crate::native_api_manifest_generated::generated_buffer_read_int16_le_ihi_entry(
        fast_buffer_read_i16le,
    ),
    crate::native_api_manifest_generated::generated_buffer_read_int32_be_ihi_entry(
        fast_buffer_read_i32be,
    ),
    crate::native_api_manifest_generated::generated_buffer_read_int32_le_ihi_entry(
        fast_buffer_read_i32le,
    ),
    crate::native_api_manifest_generated::generated_buffer_read_float_le_ihi_entry(
        fast_buffer_read_f32le,
    ),
    crate::native_api_manifest_generated::generated_buffer_read_float_be_ihi_entry(
        fast_buffer_read_f32be,
    ),
    crate::native_api_manifest_generated::generated_buffer_read_double_le_ihi_entry(
        fast_buffer_read_f64le,
    ),
    crate::native_api_manifest_generated::generated_buffer_read_double_be_ihi_entry(
        fast_buffer_read_f64be,
    ),
    crate::native_api_manifest_generated::generated_buffer_write_float_le_ihi_entry(
        fast_buffer_write_f32le,
    ),
    crate::native_api_manifest_generated::generated_buffer_write_float_be_ihi_entry(
        fast_buffer_write_f32be,
    ),
    crate::native_api_manifest_generated::generated_buffer_write_double_le_ihi_entry(
        fast_buffer_write_f64le,
    ),
    crate::native_api_manifest_generated::generated_buffer_write_double_be_ihi_entry(
        fast_buffer_write_f64be,
    ),
    crate::native_api_manifest_generated::generated_buffer_read_big_uint64_le_ihi_entry(
        fast_buffer_read_big_u64le,
    ),
    crate::native_api_manifest_generated::generated_buffer_read_big_uint64_be_ihi_entry(
        fast_buffer_read_big_u64be,
    ),
    crate::native_api_manifest_generated::generated_buffer_read_big_int64_le_ihi_entry(
        fast_buffer_read_big_i64le,
    ),
    crate::native_api_manifest_generated::generated_buffer_read_big_int64_be_ihi_entry(
        fast_buffer_read_big_i64be,
    ),
    crate::native_api_manifest_generated::generated_buffer_write_big_uint64_le_ihi_entry(
        fast_buffer_write_big_u64le,
    ),
    crate::native_api_manifest_generated::generated_buffer_write_big_uint64_be_ihi_entry(
        fast_buffer_write_big_u64be,
    ),
    crate::native_api_manifest_generated::generated_buffer_write_big_int64_le_ihi_entry(
        fast_buffer_write_big_i64le,
    ),
    crate::native_api_manifest_generated::generated_buffer_write_big_int64_be_ihi_entry(
        fast_buffer_write_big_i64be,
    ),
];

pub fn lookup(key: &str, receiver: IhiReceiverKind, arity: u8) -> Option<&'static IhiEntry> {
    IHI_TABLE
        .iter()
        .find(|e| e.key == key && e.receiver == receiver && e.arity == Some(arity))
}

pub fn receiver_kind_of(rt: &Runtime, v: &Value) -> IhiReceiverKind {
    match v {
        Value::String(_) => IhiReceiverKind::String,
        Value::Object(id) => {

            if rt.typed_array_views.contains_key(id) {
                IhiReceiverKind::Buffer
            } else if matches!(
                rt.obj(*id).internal_kind,
                crate::value::InternalKind::RegExp(_)
            ) {
                IhiReceiverKind::RegExp
            } else {
                IhiReceiverKind::Array
            }
        }
        Value::Number(_) => IhiReceiverKind::Number,

        _ => IhiReceiverKind::Number,
    }
}
