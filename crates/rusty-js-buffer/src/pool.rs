
use std::cell::RefCell;
use std::rc::Rc;

pub const DEFAULT_POOL_SIZE: usize = 8192;

#[derive(Clone)]
pub struct PooledBuffer {
    block: Rc<RefCell<Vec<u8>>>,
    offset: usize,
    len: usize,
}

impl PooledBuffer {
    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn to_vec(&self) -> Vec<u8> {
        self.block.borrow()[self.offset..self.offset + self.len].to_vec()
    }

    pub fn copy_at(&self, at: usize, src: &[u8]) {
        if at >= self.len {
            return;
        }
        let n = src.len().min(self.len - at);
        self.block.borrow_mut()[self.offset + at..self.offset + at + n].copy_from_slice(&src[..n]);
    }

    pub fn zero_range(&self, from: usize, to: usize) {
        let to = to.min(self.len);
        if from < to {
            for x in &mut self.block.borrow_mut()[self.offset + from..self.offset + to] {
                *x = 0;
            }
        }
    }

    pub fn shares_block_with(&self, other: &PooledBuffer) -> bool {
        Rc::ptr_eq(&self.block, &other.block)
    }

    pub fn standalone(bytes: Vec<u8>) -> PooledBuffer {
        let len = bytes.len();
        PooledBuffer {
            block: Rc::new(RefCell::new(bytes)),
            offset: 0,
            len,
        }
    }

    pub fn get(&self, index: i64) -> Option<u8> {
        if index < 0 {
            return None;
        }
        let idx = index as usize;
        if idx >= self.len {
            return None;
        }
        Some(self.block.borrow()[self.offset + idx])
    }

    pub fn set(&self, index: i64, value: i32) -> Option<u8> {
        if index < 0 {
            return None;
        }
        let idx = index as usize;
        if idx >= self.len {
            return None;
        }
        let byte = (value & 0xFF) as u8;
        self.block.borrow_mut()[self.offset + idx] = byte;
        Some(byte)
    }

    pub fn slice(&self, start: i64, end: Option<i64>) -> PooledBuffer {
        let len = self.len as i64;
        let s = normalize_bound(start, len);
        let e = normalize_bound(end.unwrap_or(len), len).max(s);
        PooledBuffer {
            block: Rc::clone(&self.block),
            offset: self.offset + s as usize,
            len: (e - s) as usize,
        }
    }

    pub fn subarray(&self, start: i64, end: Option<i64>) -> PooledBuffer {
        self.slice(start, end)
    }

    pub fn fill_byte(&self, value: u8, offset: usize, end: usize) {
        let start = offset.min(self.len);
        let stop = end.clamp(start, self.len);
        if start < stop {
            for b in &mut self.block.borrow_mut()[self.offset + start..self.offset + stop] {
                *b = value;
            }
        }
    }

    pub fn fill_bytes(&self, pattern: &[u8], offset: usize, end: usize) {
        if pattern.is_empty() {
            return;
        }
        let start = offset.min(self.len);
        let stop = end.clamp(start, self.len);
        let mut block = self.block.borrow_mut();
        for (k, b) in block[self.offset + start..self.offset + stop]
            .iter_mut()
            .enumerate()
        {
            *b = pattern[k % pattern.len()];
        }
    }

    pub fn write(&self, src: &[u8], offset: usize, length: Option<usize>) -> usize {
        let off = offset.min(self.len);
        let remaining = self.len - off;
        let n = src.len().min(remaining).min(length.unwrap_or(remaining));
        if n == 0 {
            return 0;
        }
        self.block.borrow_mut()[self.offset + off..self.offset + off + n]
            .copy_from_slice(&src[..n]);
        n
    }

    pub fn copy(
        &self,
        target: &PooledBuffer,
        target_start: usize,
        source_start: usize,
        source_end: usize,
    ) -> usize {
        let src_start = source_start.min(self.len);
        let src_end = source_end.clamp(src_start, self.len);
        let tgt_start = target_start.min(target.len);
        let n = (src_end - src_start).min(target.len - tgt_start);
        if n == 0 {
            return 0;
        }
        let snapshot: Vec<u8> = {
            let block = self.block.borrow();
            block[self.offset + src_start..self.offset + src_start + n].to_vec()
        };
        target.block.borrow_mut()[target.offset + tgt_start..target.offset + tgt_start + n]
            .copy_from_slice(&snapshot);
        n
    }
}

fn normalize_bound(i: i64, len: i64) -> i64 {
    if i < 0 {
        (len + i).max(0)
    } else {
        i.min(len)
    }
}

pub struct BufferPool {
    pool_size: usize,
    block: Rc<RefCell<Vec<u8>>>,
    cursor: usize,

    pub pool_blocks_created: u64,

    pub pooled_allocations: u64,

    pub standalone_allocations: u64,
}

impl Default for BufferPool {
    fn default() -> Self {
        Self::with_pool_size(DEFAULT_POOL_SIZE)
    }
}

impl BufferPool {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_pool_size(pool_size: usize) -> Self {
        let pool_size = pool_size.max(1);
        BufferPool {
            pool_size,
            block: Rc::new(RefCell::new(vec![0u8; pool_size])),
            cursor: 0,
            pool_blocks_created: 1,
            pooled_allocations: 0,
            standalone_allocations: 0,
        }
    }

    pub fn pool_size(&self) -> usize {
        self.pool_size
    }

    fn poolable(&self, size: usize) -> bool {
        size <= self.pool_size >> 1
    }

    pub fn alloc_unsafe(&mut self, size: usize) -> PooledBuffer {
        if !self.poolable(size) {
            self.standalone_allocations += 1;
            return PooledBuffer {
                block: Rc::new(RefCell::new(vec![0u8; size])),
                offset: 0,
                len: size,
            };
        }
        if self.cursor + size > self.pool_size {

            self.block = Rc::new(RefCell::new(vec![0u8; self.pool_size]));
            self.cursor = 0;
            self.pool_blocks_created += 1;
        }
        let offset = self.cursor;
        self.cursor += size;
        self.pooled_allocations += 1;
        PooledBuffer {
            block: Rc::clone(&self.block),
            offset,
            len: size,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pooled_allocs_share_block_alloc_elision() {

        let mut p = BufferPool::with_pool_size(8192);
        let a = p.alloc_unsafe(4096);
        let b = p.alloc_unsafe(4096);
        assert!(
            a.shares_block_with(&b),
            "two half-pool allocs must share a block"
        );
        assert_eq!(p.pool_blocks_created, 1);
        assert_eq!(p.pooled_allocations, 2);
    }

    #[test]
    fn pool_rotation_one_block_per_two_half_allocs() {

        let mut p = BufferPool::with_pool_size(8192);
        for _ in 0..6 {
            let _ = p.alloc_unsafe(4096);
        }
        assert_eq!(p.pool_blocks_created, 3, "6 half-pool allocs → 3 blocks");
        assert_eq!(p.pooled_allocations, 6);
        assert_eq!(p.standalone_allocations, 0);
    }

    #[test]
    fn over_half_pool_is_standalone() {
        let mut p = BufferPool::with_pool_size(8192);
        let big = p.alloc_unsafe(4097);
        let small = p.alloc_unsafe(10);
        assert!(
            !big.shares_block_with(&small),
            "standalone must not share pool block"
        );
        assert_eq!(p.standalone_allocations, 1);
        assert_eq!(p.pooled_allocations, 1);
        assert_eq!(big.len(), 4097);
    }

    #[test]
    fn view_copy_and_readback() {
        let mut p = BufferPool::with_pool_size(64);
        let v = p.alloc_unsafe(8);
        v.copy_at(0, &[1, 2, 3, 4]);
        v.copy_at(4, &[5, 6, 7, 8]);
        assert_eq!(v.to_vec(), vec![1, 2, 3, 4, 5, 6, 7, 8]);
    }

    #[test]
    fn rotation_keeps_old_view_valid() {

        let mut p = BufferPool::with_pool_size(16);
        let a = p.alloc_unsafe(8);
        a.copy_at(0, &[9; 8]);

        let _b = p.alloc_unsafe(8);
        let _c = p.alloc_unsafe(8);
        assert_eq!(a.to_vec(), vec![9; 8], "old view survives rotation via Rc");
    }
}
