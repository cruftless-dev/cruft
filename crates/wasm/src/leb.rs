
pub struct Reader<'a> {
    pub bytes: &'a [u8],
    pub pos: usize,
}

impl<'a> Reader<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        Reader { bytes, pos: 0 }
    }

    #[allow(dead_code)]
    pub fn remaining(&self) -> usize {
        self.bytes.len().saturating_sub(self.pos)
    }

    pub fn eof(&self) -> bool {
        self.pos >= self.bytes.len()
    }

    pub fn byte(&mut self) -> Result<u8, String> {
        if self.pos >= self.bytes.len() {
            return Err("unexpected end of input".to_string());
        }
        let b = self.bytes[self.pos];
        self.pos += 1;
        Ok(b)
    }

    pub fn bytes_n(&mut self, n: usize) -> Result<&'a [u8], String> {
        if self.pos + n > self.bytes.len() {
            return Err("unexpected end of input (bytes_n)".to_string());
        }
        let s = &self.bytes[self.pos..self.pos + n];
        self.pos += n;
        Ok(s)
    }

    pub fn u32(&mut self) -> Result<u32, String> {
        let v = self.u64_inner(32)?;
        if v > u32::MAX as u64 {
            return Err("LEB128 u32 overflow".to_string());
        }
        Ok(v as u32)
    }

    #[allow(dead_code)]
    pub fn u64(&mut self) -> Result<u64, String> {
        self.u64_inner(64)
    }

    fn u64_inner(&mut self, max_bits: u32) -> Result<u64, String> {
        let mut result: u64 = 0;
        let mut shift: u32 = 0;
        loop {
            let b = self.byte()?;
            if shift >= max_bits {

                if (b & 0x7f) != 0 && shift < 64 {

                } else if shift >= 64 {
                    return Err("LEB128 u overflow".to_string());
                }
            }
            result |= ((b & 0x7f) as u64) << shift;
            shift += 7;
            if (b & 0x80) == 0 {
                break;
            }
            if shift >= 64 {
                return Err("LEB128 u too long".to_string());
            }
        }
        Ok(result)
    }

    pub fn i32(&mut self) -> Result<i32, String> {
        Ok(self.i64_inner(32)? as i32)
    }

    pub fn i64(&mut self) -> Result<i64, String> {
        self.i64_inner(64)
    }

    fn i64_inner(&mut self, max_bits: u32) -> Result<i64, String> {
        let mut result: i64 = 0;
        let mut shift: u32 = 0;
        let mut byte;
        loop {
            byte = self.byte()?;
            result |= ((byte & 0x7f) as i64) << shift;
            shift += 7;
            if (byte & 0x80) == 0 {
                break;
            }
            if shift >= 64 {
                return Err("LEB128 s too long".to_string());
            }
        }
        let _ = max_bits;
        if shift < 64 && (byte & 0x40) != 0 {
            result |= -1i64 << shift;
        }
        Ok(result)
    }

    pub fn f32(&mut self) -> Result<f32, String> {
        let b = self.bytes_n(4)?;
        Ok(f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    pub fn f64(&mut self) -> Result<f64, String> {
        let b = self.bytes_n(8)?;
        Ok(f64::from_le_bytes([
            b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
        ]))
    }

    pub fn name(&mut self) -> Result<String, String> {
        let len = self.u32()? as usize;
        let bytes = self.bytes_n(len)?;
        String::from_utf8(bytes.to_vec()).map_err(|_| "invalid UTF-8 name".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::Reader;

    #[test]
    fn u32_rejects_out_of_range_value() {
        let mut r = Reader::new(&[0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x01]);
        let err = r.u32().expect_err("u32 LEB above u32::MAX must reject");
        assert!(err.contains("u32 overflow"), "{err}");
    }

    #[test]
    fn u32_accepts_max_value() {
        let mut r = Reader::new(&[0xff, 0xff, 0xff, 0xff, 0x0f]);
        assert_eq!(r.u32().expect("u32 max should decode"), u32::MAX);
    }
}
