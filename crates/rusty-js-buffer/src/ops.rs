
use crate::Buffer;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SwapError;

impl Buffer {

    fn swap_chunks(&mut self, chunk: usize) -> Result<(), SwapError> {
        if self.len() % chunk != 0 {
            return Err(SwapError);
        }
        let bytes = self.bytes_mut();
        for c in bytes.chunks_exact_mut(chunk) {
            c.reverse();
        }
        Ok(())
    }

    pub fn swap16(&mut self) -> Result<(), SwapError> {
        self.swap_chunks(2)
    }

    pub fn swap32(&mut self) -> Result<(), SwapError> {
        self.swap_chunks(4)
    }

    pub fn swap64(&mut self) -> Result<(), SwapError> {
        self.swap_chunks(8)
    }

    pub fn equals(&self, other: &Buffer) -> bool {
        self.as_bytes() == other.as_bytes()
    }

    pub fn compare(&self, other: &Buffer) -> i32 {
        Self::compare_bytes(self.as_bytes(), other.as_bytes())
    }

    pub fn compare_range(
        &self,
        other: &Buffer,
        target_start: usize,
        target_end: usize,
        source_start: usize,
        source_end: usize,
    ) -> i32 {
        let ss = source_start.min(self.len());
        let se = source_end.clamp(ss, self.len());
        let ts = target_start.min(other.len());
        let te = target_end.clamp(ts, other.len());
        Self::compare_bytes(&self.as_bytes()[ss..se], &other.as_bytes()[ts..te])
    }

    fn compare_bytes(a: &[u8], b: &[u8]) -> i32 {
        use std::cmp::Ordering::*;
        match a.cmp(b) {
            Less => -1,
            Equal => 0,
            Greater => 1,
        }
    }

    fn norm_offset(byte_offset: i64, len: usize) -> usize {
        if byte_offset < 0 {
            (len as i64 + byte_offset).max(0) as usize
        } else {
            (byte_offset as usize).min(len)
        }
    }

    pub fn index_of(&self, needle: &[u8], byte_offset: i64) -> Option<usize> {
        let hay = self.as_bytes();
        let start = Self::norm_offset(byte_offset, hay.len());
        if needle.is_empty() {
            return Some(start.min(hay.len()));
        }
        if start > hay.len() || needle.len() > hay.len().saturating_sub(start) {
            return None;
        }
        (start..=hay.len().saturating_sub(needle.len()))
            .find(|&i| i >= start && hay[i..i + needle.len()] == *needle)
    }

    pub fn last_index_of(&self, needle: &[u8], byte_offset: i64) -> Option<usize> {
        let hay = self.as_bytes();
        if needle.is_empty() {
            return Some(Self::norm_offset(byte_offset, hay.len()));
        }
        if needle.len() > hay.len() {
            return None;
        }
        let last_start = hay.len() - needle.len();
        let bound = Self::norm_offset(byte_offset, hay.len()).min(last_start);
        (0..=bound)
            .rev()
            .find(|&i| hay[i..i + needle.len()] == *needle)
    }

    pub fn includes(&self, needle: &[u8], byte_offset: i64) -> bool {
        self.index_of(needle, byte_offset).is_some()
    }

    pub fn to_json_data(&self) -> Vec<u8> {
        self.as_bytes().to_vec()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn swap16_32_64_reverse_chunks() {
        let mut a = Buffer::from_bytes(&[1, 2, 3, 4]);
        a.swap16().unwrap();
        assert_eq!(a.as_bytes(), &[2, 1, 4, 3]);
        let mut b = Buffer::from_bytes(&[1, 2, 3, 4]);
        b.swap32().unwrap();
        assert_eq!(b.as_bytes(), &[4, 3, 2, 1]);
        let mut c = Buffer::from_bytes(&[1, 2, 3, 4, 5, 6, 7, 8]);
        c.swap64().unwrap();
        assert_eq!(c.as_bytes(), &[8, 7, 6, 5, 4, 3, 2, 1]);
    }

    #[test]
    fn swap_bad_length_is_error() {
        assert_eq!(Buffer::from_bytes(&[1, 2, 3]).swap16(), Err(SwapError));
        assert_eq!(
            Buffer::from_bytes(&[1, 2, 3, 4, 5]).swap32(),
            Err(SwapError)
        );
        assert_eq!(Buffer::from_bytes(&[1; 4]).swap64(), Err(SwapError));

        assert_eq!(Buffer::alloc(0).swap16(), Ok(()));
    }

    #[test]
    fn equals_and_compare() {
        let a = Buffer::from_bytes(&[1, 2, 3]);
        let b = Buffer::from_bytes(&[1, 2, 3]);
        let c = Buffer::from_bytes(&[1, 2, 4]);
        assert!(a.equals(&b));
        assert!(!a.equals(&c));
        assert_eq!(a.compare(&b), 0);
        assert_eq!(a.compare(&c), -1);
        assert_eq!(c.compare(&a), 1);

        assert_eq!(Buffer::from_bytes(&[1, 2]).compare(&a), -1);
    }

    #[test]
    fn compare_with_ranges() {
        let a = Buffer::from_bytes(&[1, 2, 3, 4]);
        let b = Buffer::from_bytes(&[0, 2, 3, 9]);

        assert_eq!(a.compare_range(&b, 1, 3, 1, 3), 0);
    }

    #[test]
    fn index_of_byte_and_buffer() {
        let b = Buffer::from_bytes(b"abcabc");
        assert_eq!(b.index_of(b"bc", 0), Some(1));
        assert_eq!(b.index_of(b"bc", 2), Some(4));
        assert_eq!(b.index_of(b"x", 0), None);
        assert_eq!(
            b.index_of(&[b'a'], -4),
            Some(3),
            "negative offset from end (start=2)"
        );
        assert_eq!(
            b.index_of(&[b'a'], -2),
            None,
            "negative offset start=4, no 'a' after"
        );
        assert_eq!(b.index_of(b"", 3), Some(3), "empty needle → clamped offset");
        assert!(b.includes(b"cab", 0));
        assert!(!b.includes(b"cba", 0));
    }

    #[test]
    fn last_index_of_reverse() {
        let b = Buffer::from_bytes(b"abcabc");
        assert_eq!(b.last_index_of(b"bc", b.len() as i64), Some(4));
        assert_eq!(b.last_index_of(b"bc", 3), Some(1), "bounded before offset");
        assert_eq!(b.last_index_of(&[b'a'], b.len() as i64), Some(3));
        assert_eq!(b.last_index_of(b"x", b.len() as i64), None);
    }

    #[test]
    fn to_json_data_is_byte_array() {
        assert_eq!(Buffer::from_bytes(&[1, 2, 3]).to_json_data(), vec![1, 2, 3]);
    }
}
