
use crate::Buffer;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NumError {

    OutOfBounds,

    ValueOutOfRange,

    ByteLengthOutOfRange,
}

type R<T> = Result<T, NumError>;

impl Buffer {
    #[inline]
    fn rd_bounds(&self, offset: usize, size: usize) -> R<usize> {
        let end = offset.checked_add(size).ok_or(NumError::OutOfBounds)?;
        if end > self.as_bytes().len() {
            return Err(NumError::OutOfBounds);
        }
        Ok(end)
    }

    #[inline]
    fn slice(&self, offset: usize, size: usize) -> R<&[u8]> {
        let end = self.rd_bounds(offset, size)?;
        Ok(&self.as_bytes()[offset..end])
    }

    pub fn read_u8(&self, o: usize) -> R<u8> {
        Ok(self.slice(o, 1)?[0])
    }
    pub fn read_i8(&self, o: usize) -> R<i8> {
        Ok(self.read_u8(o)? as i8)
    }
    pub fn read_u16_be(&self, o: usize) -> R<u16> {
        let s = self.slice(o, 2)?;
        Ok(u16::from_be_bytes([s[0], s[1]]))
    }
    pub fn read_u16_le(&self, o: usize) -> R<u16> {
        let s = self.slice(o, 2)?;
        Ok(u16::from_le_bytes([s[0], s[1]]))
    }
    pub fn read_i16_be(&self, o: usize) -> R<i16> {
        Ok(self.read_u16_be(o)? as i16)
    }
    pub fn read_i16_le(&self, o: usize) -> R<i16> {
        Ok(self.read_u16_le(o)? as i16)
    }
    pub fn read_u32_be(&self, o: usize) -> R<u32> {
        let s = self.slice(o, 4)?;
        Ok(u32::from_be_bytes([s[0], s[1], s[2], s[3]]))
    }
    pub fn read_u32_le(&self, o: usize) -> R<u32> {
        let s = self.slice(o, 4)?;
        Ok(u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
    }
    pub fn read_i32_be(&self, o: usize) -> R<i32> {
        Ok(self.read_u32_be(o)? as i32)
    }
    pub fn read_i32_le(&self, o: usize) -> R<i32> {
        Ok(self.read_u32_le(o)? as i32)
    }

    pub fn read_big_u64_be(&self, o: usize) -> R<u64> {
        let s = self.slice(o, 8)?;
        Ok(u64::from_be_bytes(s.try_into().unwrap()))
    }
    pub fn read_big_u64_le(&self, o: usize) -> R<u64> {
        let s = self.slice(o, 8)?;
        Ok(u64::from_le_bytes(s.try_into().unwrap()))
    }
    pub fn read_big_i64_be(&self, o: usize) -> R<i64> {
        Ok(self.read_big_u64_be(o)? as i64)
    }
    pub fn read_big_i64_le(&self, o: usize) -> R<i64> {
        Ok(self.read_big_u64_le(o)? as i64)
    }

    pub fn read_float_be(&self, o: usize) -> R<f32> {
        let s = self.slice(o, 4)?;
        Ok(f32::from_be_bytes([s[0], s[1], s[2], s[3]]))
    }
    pub fn read_float_le(&self, o: usize) -> R<f32> {
        let s = self.slice(o, 4)?;
        Ok(f32::from_le_bytes([s[0], s[1], s[2], s[3]]))
    }
    pub fn read_double_be(&self, o: usize) -> R<f64> {
        let s = self.slice(o, 8)?;
        Ok(f64::from_be_bytes(s.try_into().unwrap()))
    }
    pub fn read_double_le(&self, o: usize) -> R<f64> {
        let s = self.slice(o, 8)?;
        Ok(f64::from_le_bytes(s.try_into().unwrap()))
    }

    fn check_var_len(byte_length: usize) -> R<()> {
        if (1..=6).contains(&byte_length) {
            Ok(())
        } else {
            Err(NumError::ByteLengthOutOfRange)
        }
    }

    pub fn read_uint_be(&self, o: usize, byte_length: usize) -> R<u64> {
        Self::check_var_len(byte_length)?;
        let s = self.slice(o, byte_length)?;
        let mut v: u64 = 0;
        for &b in s {
            v = (v << 8) | b as u64;
        }
        Ok(v)
    }
    pub fn read_uint_le(&self, o: usize, byte_length: usize) -> R<u64> {
        Self::check_var_len(byte_length)?;
        let s = self.slice(o, byte_length)?;
        let mut v: u64 = 0;
        for &b in s.iter().rev() {
            v = (v << 8) | b as u64;
        }
        Ok(v)
    }
    pub fn read_int_be(&self, o: usize, byte_length: usize) -> R<i64> {
        Ok(sign_extend(self.read_uint_be(o, byte_length)?, byte_length))
    }
    pub fn read_int_le(&self, o: usize, byte_length: usize) -> R<i64> {
        Ok(sign_extend(self.read_uint_le(o, byte_length)?, byte_length))
    }

    fn wr_bytes(&mut self, o: usize, src: &[u8]) -> R<usize> {
        let end = self.rd_bounds(o, src.len())?;
        self.bytes_mut()[o..end].copy_from_slice(src);
        Ok(end)
    }

    pub fn write_u8(&mut self, v: i128, o: usize) -> R<usize> {
        in_range(v, 0, u8::MAX as i128)?;
        self.wr_bytes(o, &[(v as u8)])
    }
    pub fn write_i8(&mut self, v: i128, o: usize) -> R<usize> {
        in_range(v, i8::MIN as i128, i8::MAX as i128)?;
        self.wr_bytes(o, &[(v as i8 as u8)])
    }
    pub fn write_u16_be(&mut self, v: i128, o: usize) -> R<usize> {
        in_range(v, 0, u16::MAX as i128)?;
        self.wr_bytes(o, &(v as u16).to_be_bytes())
    }
    pub fn write_u16_le(&mut self, v: i128, o: usize) -> R<usize> {
        in_range(v, 0, u16::MAX as i128)?;
        self.wr_bytes(o, &(v as u16).to_le_bytes())
    }
    pub fn write_i16_be(&mut self, v: i128, o: usize) -> R<usize> {
        in_range(v, i16::MIN as i128, i16::MAX as i128)?;
        self.wr_bytes(o, &(v as i16).to_be_bytes())
    }
    pub fn write_i16_le(&mut self, v: i128, o: usize) -> R<usize> {
        in_range(v, i16::MIN as i128, i16::MAX as i128)?;
        self.wr_bytes(o, &(v as i16).to_le_bytes())
    }
    pub fn write_u32_be(&mut self, v: i128, o: usize) -> R<usize> {
        in_range(v, 0, u32::MAX as i128)?;
        self.wr_bytes(o, &(v as u32).to_be_bytes())
    }
    pub fn write_u32_le(&mut self, v: i128, o: usize) -> R<usize> {
        in_range(v, 0, u32::MAX as i128)?;
        self.wr_bytes(o, &(v as u32).to_le_bytes())
    }
    pub fn write_i32_be(&mut self, v: i128, o: usize) -> R<usize> {
        in_range(v, i32::MIN as i128, i32::MAX as i128)?;
        self.wr_bytes(o, &(v as i32).to_be_bytes())
    }
    pub fn write_i32_le(&mut self, v: i128, o: usize) -> R<usize> {
        in_range(v, i32::MIN as i128, i32::MAX as i128)?;
        self.wr_bytes(o, &(v as i32).to_le_bytes())
    }

    pub fn write_big_u64_be(&mut self, v: i128, o: usize) -> R<usize> {
        in_range(v, 0, u64::MAX as i128)?;
        self.wr_bytes(o, &(v as u64).to_be_bytes())
    }
    pub fn write_big_u64_le(&mut self, v: i128, o: usize) -> R<usize> {
        in_range(v, 0, u64::MAX as i128)?;
        self.wr_bytes(o, &(v as u64).to_le_bytes())
    }
    pub fn write_big_i64_be(&mut self, v: i128, o: usize) -> R<usize> {
        in_range(v, i64::MIN as i128, i64::MAX as i128)?;
        self.wr_bytes(o, &(v as i64).to_be_bytes())
    }
    pub fn write_big_i64_le(&mut self, v: i128, o: usize) -> R<usize> {
        in_range(v, i64::MIN as i128, i64::MAX as i128)?;
        self.wr_bytes(o, &(v as i64).to_le_bytes())
    }

    pub fn write_float_be(&mut self, v: f64, o: usize) -> R<usize> {
        self.wr_bytes(o, &(v as f32).to_be_bytes())
    }
    pub fn write_float_le(&mut self, v: f64, o: usize) -> R<usize> {
        self.wr_bytes(o, &(v as f32).to_le_bytes())
    }
    pub fn write_double_be(&mut self, v: f64, o: usize) -> R<usize> {
        self.wr_bytes(o, &v.to_be_bytes())
    }
    pub fn write_double_le(&mut self, v: f64, o: usize) -> R<usize> {
        self.wr_bytes(o, &v.to_le_bytes())
    }

    pub fn write_uint_be(&mut self, v: i128, o: usize, byte_length: usize) -> R<usize> {
        Self::check_var_len(byte_length)?;
        in_range(v, 0, (1i128 << (8 * byte_length)) - 1)?;
        let full = (v as u64).to_be_bytes();
        let src = &full[8 - byte_length..];
        self.wr_bytes(o, src)
    }
    pub fn write_uint_le(&mut self, v: i128, o: usize, byte_length: usize) -> R<usize> {
        Self::check_var_len(byte_length)?;
        in_range(v, 0, (1i128 << (8 * byte_length)) - 1)?;
        let full = (v as u64).to_le_bytes();
        let src = &full[..byte_length];
        self.wr_bytes(o, src)
    }
    pub fn write_int_be(&mut self, v: i128, o: usize, byte_length: usize) -> R<usize> {
        Self::check_var_len(byte_length)?;
        let bound = 1i128 << (8 * byte_length - 1);
        in_range(v, -bound, bound - 1)?;

        let u = (v & ((1i128 << (8 * byte_length)) - 1)) as u64;
        let full = u.to_be_bytes();
        self.wr_bytes(o, &full[8 - byte_length..])
    }
    pub fn write_int_le(&mut self, v: i128, o: usize, byte_length: usize) -> R<usize> {
        Self::check_var_len(byte_length)?;
        let bound = 1i128 << (8 * byte_length - 1);
        in_range(v, -bound, bound - 1)?;
        let u = (v & ((1i128 << (8 * byte_length)) - 1)) as u64;
        let full = u.to_le_bytes();
        self.wr_bytes(o, &full[..byte_length])
    }
}

#[inline]
fn in_range(v: i128, lo: i128, hi: i128) -> R<()> {
    if v < lo || v > hi {
        Err(NumError::ValueOutOfRange)
    } else {
        Ok(())
    }
}

#[inline]
fn sign_extend(u: u64, byte_length: usize) -> i64 {
    let bits = 8 * byte_length;
    if bits >= 64 {
        return u as i64;
    }
    let sign_bit = 1u64 << (bits - 1);
    if u & sign_bit != 0 {
        (u as i64) - (1i64 << bits)
    } else {
        u as i64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_fixed_width_be_le() {
        let b = Buffer::from_bytes(&[0x01, 0x02, 0x03, 0x04]);
        assert_eq!(b.read_u8(0), Ok(1));
        assert_eq!(b.read_i8(0), Ok(1));
        assert_eq!(b.read_u16_be(0), Ok(0x0102));
        assert_eq!(b.read_u16_le(0), Ok(0x0201));
        assert_eq!(b.read_u32_be(0), Ok(0x01020304));
        assert_eq!(b.read_u32_le(0), Ok(0x04030201));
    }

    #[test]
    fn read_signed_negative() {
        let b = Buffer::from_bytes(&[0xff, 0xfe]);
        assert_eq!(b.read_i8(0), Ok(-1));
        assert_eq!(b.read_i16_be(0), Ok(-2));
    }

    #[test]
    fn read_out_of_bounds_is_rangeerror() {
        let b = Buffer::from_bytes(&[0x01]);
        assert_eq!(b.read_u16_be(0), Err(NumError::OutOfBounds));
        assert_eq!(b.read_u8(1), Err(NumError::OutOfBounds));
    }

    #[test]
    fn read_write_64bit_roundtrip() {
        let mut b = Buffer::alloc(8);
        assert_eq!(b.write_big_u64_be(0x0102030405060708, 0), Ok(8));
        assert_eq!(b.read_big_u64_be(0), Ok(0x0102030405060708));
        let mut c = Buffer::alloc(8);
        assert_eq!(c.write_big_i64_le(-2, 0), Ok(8));
        assert_eq!(c.read_big_i64_le(0), Ok(-2));
    }

    #[test]
    fn float_double_roundtrip() {
        let mut b = Buffer::alloc(4);
        b.write_float_be(1.5, 0).unwrap();
        assert_eq!(b.read_float_be(0), Ok(1.5));
        let mut d = Buffer::alloc(8);
        d.write_double_le(3.141592653589793, 0).unwrap();
        assert_eq!(d.read_double_le(0), Ok(3.141592653589793));
    }

    #[test]
    fn writer_value_out_of_range_is_error() {
        let mut b = Buffer::alloc(4);
        assert_eq!(b.write_u8(256, 0), Err(NumError::ValueOutOfRange));
        assert_eq!(b.write_u8(-1, 0), Err(NumError::ValueOutOfRange));
        assert_eq!(b.write_i8(128, 0), Err(NumError::ValueOutOfRange));

        assert_eq!(b.write_u8(255, 0), Ok(1));
        assert_eq!(b.as_bytes()[0], 255);
    }

    #[test]
    fn writer_offset_out_of_bounds_is_error() {
        let mut b = Buffer::alloc(2);
        assert_eq!(b.write_u32_be(1, 0), Err(NumError::OutOfBounds));
        assert_eq!(b.write_u8(1, 2), Err(NumError::OutOfBounds));
    }

    #[test]
    fn variable_length_uint_be_le() {
        let b = Buffer::from_bytes(&[0x12, 0x34, 0x56]);
        assert_eq!(b.read_uint_be(0, 3), Ok(0x123456));
        assert_eq!(b.read_uint_le(0, 3), Ok(0x563412));
        let mut w = Buffer::alloc(3);
        assert_eq!(w.write_uint_be(0xAABBCC, 0, 3), Ok(3));
        assert_eq!(w.as_bytes(), &[0xAA, 0xBB, 0xCC]);
        assert_eq!(w.write_uint_le(0xAABBCC, 0, 3), Ok(3));
        assert_eq!(w.as_bytes(), &[0xCC, 0xBB, 0xAA]);
    }

    #[test]
    fn variable_length_int_sign_extend_and_write() {

        let b = Buffer::from_bytes(&[0xFF, 0xFF, 0xFF]);
        assert_eq!(b.read_int_be(0, 3), Ok(-1));
        assert_eq!(b.read_int_le(0, 3), Ok(-1));
        let mut w = Buffer::alloc(3);
        assert_eq!(w.write_int_be(-2, 0, 3), Ok(3));
        assert_eq!(w.read_int_be(0, 3), Ok(-2));
        assert_eq!(w.write_int_le(-2, 0, 3), Ok(3));
        assert_eq!(w.read_int_le(0, 3), Ok(-2));
    }

    #[test]
    fn variable_length_bad_byte_length_is_error() {
        let b = Buffer::from_bytes(&[0, 0, 0, 0, 0, 0, 0]);
        assert_eq!(b.read_uint_be(0, 0), Err(NumError::ByteLengthOutOfRange));
        assert_eq!(b.read_uint_be(0, 7), Err(NumError::ByteLengthOutOfRange));
    }

    #[test]
    fn variable_length_value_range_enforced() {
        let mut b = Buffer::alloc(3);

        assert_eq!(
            b.write_uint_be(0x1000000, 0, 3),
            Err(NumError::ValueOutOfRange)
        );
        assert_eq!(
            b.write_int_be(0x800000, 0, 3),
            Err(NumError::ValueOutOfRange)
        );
        assert_eq!(b.write_int_be(-0x800000, 0, 3), Ok(3));
    }
}
