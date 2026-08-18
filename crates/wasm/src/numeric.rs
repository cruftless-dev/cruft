
use crate::interp::{as_f32, as_f64, as_i32, as_i64};
use crate::WasmValue;

fn pop(s: &mut Vec<WasmValue>) -> Result<WasmValue, String> {
    s.pop()
        .ok_or_else(|| "numeric: stack underflow".to_string())
}

fn pi32(s: &mut Vec<WasmValue>) -> Result<i32, String> {
    as_i32(pop(s)?)
}
fn pi64(s: &mut Vec<WasmValue>) -> Result<i64, String> {
    as_i64(pop(s)?)
}
fn pf32(s: &mut Vec<WasmValue>) -> Result<f32, String> {
    as_f32(pop(s)?)
}
fn pf64(s: &mut Vec<WasmValue>) -> Result<f64, String> {
    as_f64(pop(s)?)
}

fn b(x: bool) -> WasmValue {
    WasmValue::I32(if x { 1 } else { 0 })
}

fn f32_nearest(x: f32) -> f32 {

    let r = x.round();
    if (x - x.trunc()).abs() == 0.5 {
        let e = 2.0 * (x / 2.0).round();
        e
    } else {
        r
    }
}
fn f64_nearest(x: f64) -> f64 {
    let r = x.round();
    if (x - x.trunc()).abs() == 0.5 {
        2.0 * (x / 2.0).round()
    } else {
        r
    }
}

fn trunc_to_i32_s(x: f64) -> Result<i32, String> {
    if x.is_nan() {
        return Err("invalid conversion to integer (NaN)".to_string());
    }
    let t = x.trunc();
    if t < i32::MIN as f64 || t > i32::MAX as f64 {
        return Err("integer overflow".to_string());
    }
    Ok(t as i32)
}
fn trunc_to_i32_u(x: f64) -> Result<i32, String> {
    if x.is_nan() {
        return Err("invalid conversion to integer (NaN)".to_string());
    }
    let t = x.trunc();
    if t < 0.0 || t > u32::MAX as f64 {
        return Err("integer overflow".to_string());
    }
    Ok(t as u32 as i32)
}
fn trunc_to_i64_s(x: f64) -> Result<i64, String> {
    if x.is_nan() {
        return Err("invalid conversion to integer (NaN)".to_string());
    }
    let t = x.trunc();
    if t < i64::MIN as f64 || t >= 9223372036854775808.0 {
        return Err("integer overflow".to_string());
    }
    Ok(t as i64)
}
fn trunc_to_i64_u(x: f64) -> Result<i64, String> {
    if x.is_nan() {
        return Err("invalid conversion to integer (NaN)".to_string());
    }
    let t = x.trunc();
    if t < 0.0 || t >= 18446744073709551616.0 {
        return Err("integer overflow".to_string());
    }
    Ok(t as u64 as i64)
}

pub fn exec_num(op: u8, s: &mut Vec<WasmValue>) -> Result<(), String> {
    match op {

        0x45 => {
            let a = pi32(s)?;
            s.push(b(a == 0));
        }
        0x46 => {
            let y = pi32(s)?;
            let x = pi32(s)?;
            s.push(b(x == y));
        }
        0x47 => {
            let y = pi32(s)?;
            let x = pi32(s)?;
            s.push(b(x != y));
        }
        0x48 => {
            let y = pi32(s)?;
            let x = pi32(s)?;
            s.push(b(x < y));
        }
        0x49 => {
            let y = pi32(s)? as u32;
            let x = pi32(s)? as u32;
            s.push(b(x < y));
        }
        0x4a => {
            let y = pi32(s)?;
            let x = pi32(s)?;
            s.push(b(x > y));
        }
        0x4b => {
            let y = pi32(s)? as u32;
            let x = pi32(s)? as u32;
            s.push(b(x > y));
        }
        0x4c => {
            let y = pi32(s)?;
            let x = pi32(s)?;
            s.push(b(x <= y));
        }
        0x4d => {
            let y = pi32(s)? as u32;
            let x = pi32(s)? as u32;
            s.push(b(x <= y));
        }
        0x4e => {
            let y = pi32(s)?;
            let x = pi32(s)?;
            s.push(b(x >= y));
        }
        0x4f => {
            let y = pi32(s)? as u32;
            let x = pi32(s)? as u32;
            s.push(b(x >= y));
        }

        0x50 => {
            let a = pi64(s)?;
            s.push(b(a == 0));
        }
        0x51 => {
            let y = pi64(s)?;
            let x = pi64(s)?;
            s.push(b(x == y));
        }
        0x52 => {
            let y = pi64(s)?;
            let x = pi64(s)?;
            s.push(b(x != y));
        }
        0x53 => {
            let y = pi64(s)?;
            let x = pi64(s)?;
            s.push(b(x < y));
        }
        0x54 => {
            let y = pi64(s)? as u64;
            let x = pi64(s)? as u64;
            s.push(b(x < y));
        }
        0x55 => {
            let y = pi64(s)?;
            let x = pi64(s)?;
            s.push(b(x > y));
        }
        0x56 => {
            let y = pi64(s)? as u64;
            let x = pi64(s)? as u64;
            s.push(b(x > y));
        }
        0x57 => {
            let y = pi64(s)?;
            let x = pi64(s)?;
            s.push(b(x <= y));
        }
        0x58 => {
            let y = pi64(s)? as u64;
            let x = pi64(s)? as u64;
            s.push(b(x <= y));
        }
        0x59 => {
            let y = pi64(s)?;
            let x = pi64(s)?;
            s.push(b(x >= y));
        }
        0x5a => {
            let y = pi64(s)? as u64;
            let x = pi64(s)? as u64;
            s.push(b(x >= y));
        }

        0x5b => {
            let y = pf32(s)?;
            let x = pf32(s)?;
            s.push(b(x == y));
        }
        0x5c => {
            let y = pf32(s)?;
            let x = pf32(s)?;
            s.push(b(x != y));
        }
        0x5d => {
            let y = pf32(s)?;
            let x = pf32(s)?;
            s.push(b(x < y));
        }
        0x5e => {
            let y = pf32(s)?;
            let x = pf32(s)?;
            s.push(b(x > y));
        }
        0x5f => {
            let y = pf32(s)?;
            let x = pf32(s)?;
            s.push(b(x <= y));
        }
        0x60 => {
            let y = pf32(s)?;
            let x = pf32(s)?;
            s.push(b(x >= y));
        }

        0x61 => {
            let y = pf64(s)?;
            let x = pf64(s)?;
            s.push(b(x == y));
        }
        0x62 => {
            let y = pf64(s)?;
            let x = pf64(s)?;
            s.push(b(x != y));
        }
        0x63 => {
            let y = pf64(s)?;
            let x = pf64(s)?;
            s.push(b(x < y));
        }
        0x64 => {
            let y = pf64(s)?;
            let x = pf64(s)?;
            s.push(b(x > y));
        }
        0x65 => {
            let y = pf64(s)?;
            let x = pf64(s)?;
            s.push(b(x <= y));
        }
        0x66 => {
            let y = pf64(s)?;
            let x = pf64(s)?;
            s.push(b(x >= y));
        }

        0x67 => {
            let x = pi32(s)?;
            s.push(WasmValue::I32(x.leading_zeros() as i32));
        }
        0x68 => {
            let x = pi32(s)?;
            s.push(WasmValue::I32(x.trailing_zeros() as i32));
        }
        0x69 => {
            let x = pi32(s)?;
            s.push(WasmValue::I32(x.count_ones() as i32));
        }
        0x6a => {
            let y = pi32(s)?;
            let x = pi32(s)?;
            s.push(WasmValue::I32(x.wrapping_add(y)));
        }
        0x6b => {
            let y = pi32(s)?;
            let x = pi32(s)?;
            s.push(WasmValue::I32(x.wrapping_sub(y)));
        }
        0x6c => {
            let y = pi32(s)?;
            let x = pi32(s)?;
            s.push(WasmValue::I32(x.wrapping_mul(y)));
        }
        0x6d => {
            let y = pi32(s)?;
            let x = pi32(s)?;
            if y == 0 {
                return Err("integer divide by zero".to_string());
            }
            if x == i32::MIN && y == -1 {
                return Err("integer overflow".to_string());
            }
            s.push(WasmValue::I32(x.wrapping_div(y)));
        }
        0x6e => {
            let y = pi32(s)? as u32;
            let x = pi32(s)? as u32;
            if y == 0 {
                return Err("integer divide by zero".to_string());
            }
            s.push(WasmValue::I32((x / y) as i32));
        }
        0x6f => {
            let y = pi32(s)?;
            let x = pi32(s)?;
            if y == 0 {
                return Err("integer divide by zero".to_string());
            }
            s.push(WasmValue::I32(x.wrapping_rem(y)));
        }
        0x70 => {
            let y = pi32(s)? as u32;
            let x = pi32(s)? as u32;
            if y == 0 {
                return Err("integer divide by zero".to_string());
            }
            s.push(WasmValue::I32((x % y) as i32));
        }
        0x71 => {
            let y = pi32(s)?;
            let x = pi32(s)?;
            s.push(WasmValue::I32(x & y));
        }
        0x72 => {
            let y = pi32(s)?;
            let x = pi32(s)?;
            s.push(WasmValue::I32(x | y));
        }
        0x73 => {
            let y = pi32(s)?;
            let x = pi32(s)?;
            s.push(WasmValue::I32(x ^ y));
        }
        0x74 => {
            let y = pi32(s)?;
            let x = pi32(s)?;
            s.push(WasmValue::I32(x.wrapping_shl(y as u32)));
        }
        0x75 => {
            let y = pi32(s)?;
            let x = pi32(s)?;
            s.push(WasmValue::I32(x.wrapping_shr(y as u32)));
        }
        0x76 => {
            let y = pi32(s)? as u32;
            let x = pi32(s)? as u32;
            s.push(WasmValue::I32((x.wrapping_shr(y)) as i32));
        }
        0x77 => {
            let y = pi32(s)? as u32;
            let x = pi32(s)?;
            s.push(WasmValue::I32(x.rotate_left(y & 31)));
        }
        0x78 => {
            let y = pi32(s)? as u32;
            let x = pi32(s)?;
            s.push(WasmValue::I32(x.rotate_right(y & 31)));
        }

        0x79 => {
            let x = pi64(s)?;
            s.push(WasmValue::I64(x.leading_zeros() as i64));
        }
        0x7a => {
            let x = pi64(s)?;
            s.push(WasmValue::I64(x.trailing_zeros() as i64));
        }
        0x7b => {
            let x = pi64(s)?;
            s.push(WasmValue::I64(x.count_ones() as i64));
        }
        0x7c => {
            let y = pi64(s)?;
            let x = pi64(s)?;
            s.push(WasmValue::I64(x.wrapping_add(y)));
        }
        0x7d => {
            let y = pi64(s)?;
            let x = pi64(s)?;
            s.push(WasmValue::I64(x.wrapping_sub(y)));
        }
        0x7e => {
            let y = pi64(s)?;
            let x = pi64(s)?;
            s.push(WasmValue::I64(x.wrapping_mul(y)));
        }
        0x7f => {
            let y = pi64(s)?;
            let x = pi64(s)?;
            if y == 0 {
                return Err("integer divide by zero".to_string());
            }
            if x == i64::MIN && y == -1 {
                return Err("integer overflow".to_string());
            }
            s.push(WasmValue::I64(x.wrapping_div(y)));
        }
        0x80 => {
            let y = pi64(s)? as u64;
            let x = pi64(s)? as u64;
            if y == 0 {
                return Err("integer divide by zero".to_string());
            }
            s.push(WasmValue::I64((x / y) as i64));
        }
        0x81 => {
            let y = pi64(s)?;
            let x = pi64(s)?;
            if y == 0 {
                return Err("integer divide by zero".to_string());
            }
            s.push(WasmValue::I64(x.wrapping_rem(y)));
        }
        0x82 => {
            let y = pi64(s)? as u64;
            let x = pi64(s)? as u64;
            if y == 0 {
                return Err("integer divide by zero".to_string());
            }
            s.push(WasmValue::I64((x % y) as i64));
        }
        0x83 => {
            let y = pi64(s)?;
            let x = pi64(s)?;
            s.push(WasmValue::I64(x & y));
        }
        0x84 => {
            let y = pi64(s)?;
            let x = pi64(s)?;
            s.push(WasmValue::I64(x | y));
        }
        0x85 => {
            let y = pi64(s)?;
            let x = pi64(s)?;
            s.push(WasmValue::I64(x ^ y));
        }
        0x86 => {
            let y = pi64(s)?;
            let x = pi64(s)?;
            s.push(WasmValue::I64(x.wrapping_shl(y as u32)));
        }
        0x87 => {
            let y = pi64(s)?;
            let x = pi64(s)?;
            s.push(WasmValue::I64(x.wrapping_shr(y as u32)));
        }
        0x88 => {
            let y = pi64(s)? as u64;
            let x = pi64(s)? as u64;
            s.push(WasmValue::I64((x.wrapping_shr(y as u32)) as i64));
        }
        0x89 => {
            let y = pi64(s)? as u64;
            let x = pi64(s)?;
            s.push(WasmValue::I64(x.rotate_left((y & 63) as u32)));
        }
        0x8a => {
            let y = pi64(s)? as u64;
            let x = pi64(s)?;
            s.push(WasmValue::I64(x.rotate_right((y & 63) as u32)));
        }

        0x8b => {
            let x = pf32(s)?;
            s.push(WasmValue::F32(x.abs()));
        }
        0x8c => {
            let x = pf32(s)?;
            s.push(WasmValue::F32(-x));
        }
        0x8d => {
            let x = pf32(s)?;
            s.push(WasmValue::F32(x.ceil()));
        }
        0x8e => {
            let x = pf32(s)?;
            s.push(WasmValue::F32(x.floor()));
        }
        0x8f => {
            let x = pf32(s)?;
            s.push(WasmValue::F32(x.trunc()));
        }
        0x90 => {
            let x = pf32(s)?;
            s.push(WasmValue::F32(f32_nearest(x)));
        }
        0x91 => {
            let x = pf32(s)?;
            s.push(WasmValue::F32(x.sqrt()));
        }
        0x92 => {
            let y = pf32(s)?;
            let x = pf32(s)?;
            s.push(WasmValue::F32(x + y));
        }
        0x93 => {
            let y = pf32(s)?;
            let x = pf32(s)?;
            s.push(WasmValue::F32(x - y));
        }
        0x94 => {
            let y = pf32(s)?;
            let x = pf32(s)?;
            s.push(WasmValue::F32(x * y));
        }
        0x95 => {
            let y = pf32(s)?;
            let x = pf32(s)?;
            s.push(WasmValue::F32(x / y));
        }
        0x96 => {
            let y = pf32(s)?;
            let x = pf32(s)?;
            s.push(WasmValue::F32(wasm_fmin_f32(x, y)));
        }
        0x97 => {
            let y = pf32(s)?;
            let x = pf32(s)?;
            s.push(WasmValue::F32(wasm_fmax_f32(x, y)));
        }
        0x98 => {
            let y = pf32(s)?;
            let x = pf32(s)?;
            s.push(WasmValue::F32(x.copysign(y)));
        }

        0x99 => {
            let x = pf64(s)?;
            s.push(WasmValue::F64(x.abs()));
        }
        0x9a => {
            let x = pf64(s)?;
            s.push(WasmValue::F64(-x));
        }
        0x9b => {
            let x = pf64(s)?;
            s.push(WasmValue::F64(x.ceil()));
        }
        0x9c => {
            let x = pf64(s)?;
            s.push(WasmValue::F64(x.floor()));
        }
        0x9d => {
            let x = pf64(s)?;
            s.push(WasmValue::F64(x.trunc()));
        }
        0x9e => {
            let x = pf64(s)?;
            s.push(WasmValue::F64(f64_nearest(x)));
        }
        0x9f => {
            let x = pf64(s)?;
            s.push(WasmValue::F64(x.sqrt()));
        }
        0xa0 => {
            let y = pf64(s)?;
            let x = pf64(s)?;
            s.push(WasmValue::F64(x + y));
        }
        0xa1 => {
            let y = pf64(s)?;
            let x = pf64(s)?;
            s.push(WasmValue::F64(x - y));
        }
        0xa2 => {
            let y = pf64(s)?;
            let x = pf64(s)?;
            s.push(WasmValue::F64(x * y));
        }
        0xa3 => {
            let y = pf64(s)?;
            let x = pf64(s)?;
            s.push(WasmValue::F64(x / y));
        }
        0xa4 => {
            let y = pf64(s)?;
            let x = pf64(s)?;
            s.push(WasmValue::F64(wasm_fmin_f64(x, y)));
        }
        0xa5 => {
            let y = pf64(s)?;
            let x = pf64(s)?;
            s.push(WasmValue::F64(wasm_fmax_f64(x, y)));
        }
        0xa6 => {
            let y = pf64(s)?;
            let x = pf64(s)?;
            s.push(WasmValue::F64(x.copysign(y)));
        }

        0xa7 => {

            let x = pi64(s)?;
            s.push(WasmValue::I32(x as i32));
        }
        0xa8 => {
            let x = pf32(s)?;
            s.push(WasmValue::I32(trunc_to_i32_s(x as f64)?));
        }
        0xa9 => {
            let x = pf32(s)?;
            s.push(WasmValue::I32(trunc_to_i32_u(x as f64)?));
        }
        0xaa => {
            let x = pf64(s)?;
            s.push(WasmValue::I32(trunc_to_i32_s(x)?));
        }
        0xab => {
            let x = pf64(s)?;
            s.push(WasmValue::I32(trunc_to_i32_u(x)?));
        }
        0xac => {

            let x = pi32(s)?;
            s.push(WasmValue::I64(x as i64));
        }
        0xad => {

            let x = pi32(s)? as u32;
            s.push(WasmValue::I64(x as u64 as i64));
        }
        0xae => {
            let x = pf32(s)?;
            s.push(WasmValue::I64(trunc_to_i64_s(x as f64)?));
        }
        0xaf => {
            let x = pf32(s)?;
            s.push(WasmValue::I64(trunc_to_i64_u(x as f64)?));
        }
        0xb0 => {
            let x = pf64(s)?;
            s.push(WasmValue::I64(trunc_to_i64_s(x)?));
        }
        0xb1 => {
            let x = pf64(s)?;
            s.push(WasmValue::I64(trunc_to_i64_u(x)?));
        }
        0xb2 => {

            let x = pi32(s)?;
            s.push(WasmValue::F32(x as f32));
        }
        0xb3 => {
            let x = pi32(s)? as u32;
            s.push(WasmValue::F32(x as f32));
        }
        0xb4 => {
            let x = pi64(s)?;
            s.push(WasmValue::F32(x as f32));
        }
        0xb5 => {
            let x = pi64(s)? as u64;
            s.push(WasmValue::F32(x as f32));
        }
        0xb6 => {

            let x = pf64(s)?;
            s.push(WasmValue::F32(x as f32));
        }
        0xb7 => {
            let x = pi32(s)?;
            s.push(WasmValue::F64(x as f64));
        }
        0xb8 => {
            let x = pi32(s)? as u32;
            s.push(WasmValue::F64(x as f64));
        }
        0xb9 => {
            let x = pi64(s)?;
            s.push(WasmValue::F64(x as f64));
        }
        0xba => {
            let x = pi64(s)? as u64;
            s.push(WasmValue::F64(x as f64));
        }
        0xbb => {

            let x = pf32(s)?;
            s.push(WasmValue::F64(x as f64));
        }
        0xbc => {

            let x = pf32(s)?;
            s.push(WasmValue::I32(x.to_bits() as i32));
        }
        0xbd => {

            let x = pf64(s)?;
            s.push(WasmValue::I64(x.to_bits() as i64));
        }
        0xbe => {

            let x = pi32(s)?;
            s.push(WasmValue::F32(f32::from_bits(x as u32)));
        }
        0xbf => {

            let x = pi64(s)?;
            s.push(WasmValue::F64(f64::from_bits(x as u64)));
        }

        0xc0 => {

            let x = pi32(s)?;
            s.push(WasmValue::I32((x as u8) as i8 as i32));
        }
        0xc1 => {

            let x = pi32(s)?;
            s.push(WasmValue::I32((x as u16) as i16 as i32));
        }
        0xc2 => {

            let x = pi64(s)?;
            s.push(WasmValue::I64((x as u8) as i8 as i64));
        }
        0xc3 => {

            let x = pi64(s)?;
            s.push(WasmValue::I64((x as u16) as i16 as i64));
        }
        0xc4 => {

            let x = pi64(s)?;
            s.push(WasmValue::I64((x as u32) as i32 as i64));
        }
        other => return Err(format!("unsupported numeric opcode 0x{:02x}", other)),
    }
    Ok(())
}

pub fn exec_trunc_sat(sub: u32, s: &mut Vec<WasmValue>) -> Result<(), String> {
    match sub {
        0 => {

            let x = pf32(s)? as f64;
            s.push(WasmValue::I32(sat_i32_s(x)));
        }
        1 => {

            let x = pf32(s)? as f64;
            s.push(WasmValue::I32(sat_i32_u(x) as i32));
        }
        2 => {

            let x = pf64(s)?;
            s.push(WasmValue::I32(sat_i32_s(x)));
        }
        3 => {

            let x = pf64(s)?;
            s.push(WasmValue::I32(sat_i32_u(x) as i32));
        }
        4 => {

            let x = pf32(s)? as f64;
            s.push(WasmValue::I64(sat_i64_s(x)));
        }
        5 => {

            let x = pf32(s)? as f64;
            s.push(WasmValue::I64(sat_i64_u(x) as i64));
        }
        6 => {

            let x = pf64(s)?;
            s.push(WasmValue::I64(sat_i64_s(x)));
        }
        7 => {

            let x = pf64(s)?;
            s.push(WasmValue::I64(sat_i64_u(x) as i64));
        }
        other => return Err(format!("unsupported trunc_sat subopcode {}", other)),
    }
    Ok(())
}

fn sat_i32_s(x: f64) -> i32 {
    if x.is_nan() {
        return 0;
    }
    let t = x.trunc();
    if t < i32::MIN as f64 {
        i32::MIN
    } else if t > i32::MAX as f64 {
        i32::MAX
    } else {
        t as i32
    }
}

fn sat_i32_u(x: f64) -> u32 {
    if x.is_nan() {
        return 0;
    }
    let t = x.trunc();
    if t < 0.0 {
        0
    } else if t > u32::MAX as f64 {
        u32::MAX
    } else {
        t as u32
    }
}

fn sat_i64_s(x: f64) -> i64 {
    if x.is_nan() {
        return 0;
    }
    let t = x.trunc();
    if t < i64::MIN as f64 {
        i64::MIN
    } else if t >= i64::MAX as f64 {

        i64::MAX
    } else {
        t as i64
    }
}

fn sat_i64_u(x: f64) -> u64 {
    if x.is_nan() {
        return 0;
    }
    let t = x.trunc();
    if t < 0.0 {
        0
    } else if t >= u64::MAX as f64 {
        u64::MAX
    } else {
        t as u64
    }
}

fn wasm_fmin_f32(x: f32, y: f32) -> f32 {
    if x.is_nan() || y.is_nan() {
        return f32::NAN;
    }
    if x == 0.0 && y == 0.0 {

        if x.is_sign_negative() || y.is_sign_negative() {
            return -0.0;
        }
        return 0.0;
    }
    if x < y {
        x
    } else {
        y
    }
}
fn wasm_fmax_f32(x: f32, y: f32) -> f32 {
    if x.is_nan() || y.is_nan() {
        return f32::NAN;
    }
    if x == 0.0 && y == 0.0 {
        if x.is_sign_positive() || y.is_sign_positive() {
            return 0.0;
        }
        return -0.0;
    }
    if x > y {
        x
    } else {
        y
    }
}
fn wasm_fmin_f64(x: f64, y: f64) -> f64 {
    if x.is_nan() || y.is_nan() {
        return f64::NAN;
    }
    if x == 0.0 && y == 0.0 {
        if x.is_sign_negative() || y.is_sign_negative() {
            return -0.0;
        }
        return 0.0;
    }
    if x < y {
        x
    } else {
        y
    }
}
fn wasm_fmax_f64(x: f64, y: f64) -> f64 {
    if x.is_nan() || y.is_nan() {
        return f64::NAN;
    }
    if x == 0.0 && y == 0.0 {
        if x.is_sign_positive() || y.is_sign_positive() {
            return 0.0;
        }
        return -0.0;
    }
    if x > y {
        x
    } else {
        y
    }
}
