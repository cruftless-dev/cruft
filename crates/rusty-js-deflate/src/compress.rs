
use crate::{adler32, crc32, deflate_stored};

const LENGTH_BASE: [u16; 29] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131,
    163, 195, 227, 258,
];
const LENGTH_EXTRA: [u8; 29] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
];
const DIST_BASE: [u16; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
    2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
];
const DIST_EXTRA: [u8; 30] = [
    0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13,
    13,
];

const CLCL_ORDER: [usize; 19] = [
    16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
];

const MIN_MATCH: usize = 3;
const MAX_MATCH: usize = 258;
const WSIZE: usize = 32768;
const HASH_BITS: usize = 15;
const HASH_SIZE: usize = 1 << HASH_BITS;
const MAX_CHAIN: usize = 256;

#[derive(Clone, Copy)]
enum Sym {
    Lit(u8),
    Match { len: u16, dist: u16 },
}

#[inline]
fn hash3(data: &[u8], i: usize) -> usize {
    let a = data[i] as usize;
    let b = data[i + 1] as usize;
    let c = data[i + 2] as usize;
    ((a << 10) ^ (b << 5) ^ c) & (HASH_SIZE - 1)
}

fn lz77(data: &[u8]) -> Vec<Sym> {
    let n = data.len();
    let mut syms = Vec::new();
    if n == 0 {
        return syms;
    }
    let mut head = vec![-1i32; HASH_SIZE];
    let mut prev = vec![-1i32; n];

    let insert = |i: usize, head: &mut [i32], prev: &mut [i32]| {
        if i + MIN_MATCH <= n {
            let h = hash3(data, i);
            prev[i] = head[h];
            head[h] = i as i32;
        }
    };

    let mut i = 0;
    while i < n {
        let mut best_len = 0usize;
        let mut best_dist = 0usize;
        if i + MIN_MATCH <= n {
            let h = hash3(data, i);
            let mut cand = head[h];
            let max_len = std::cmp::min(MAX_MATCH, n - i);
            let mut chain = MAX_CHAIN;
            while cand >= 0 && chain > 0 {
                let c = cand as usize;
                if i - c > WSIZE {
                    break;
                }

                if best_len == 0 || data[c + best_len] == data[i + best_len] {
                    let mut l = 0usize;
                    while l < max_len && data[c + l] == data[i + l] {
                        l += 1;
                    }
                    if l > best_len {
                        best_len = l;
                        best_dist = i - c;
                        if l >= max_len {
                            break;
                        }
                    }
                }
                cand = prev[c];
                chain -= 1;
            }
        }
        if best_len >= MIN_MATCH {
            syms.push(Sym::Match {
                len: best_len as u16,
                dist: best_dist as u16,
            });
            let end = i + best_len;
            while i < end {
                insert(i, &mut head, &mut prev);
                i += 1;
            }
        } else {
            syms.push(Sym::Lit(data[i]));
            insert(i, &mut head, &mut prev);
            i += 1;
        }
    }
    syms
}

struct BitWriter {
    out: Vec<u8>,
    bitbuf: u32,
    bitcnt: u32,
}

impl BitWriter {
    fn new() -> Self {
        BitWriter {
            out: Vec::new(),
            bitbuf: 0,
            bitcnt: 0,
        }
    }

    #[inline]
    fn write_bits(&mut self, val: u32, n: u32) {
        debug_assert!(n >= 1 && n <= 16);
        self.bitbuf |= (val & ((1u32 << n) - 1)) << self.bitcnt;
        self.bitcnt += n;
        while self.bitcnt >= 8 {
            self.out.push((self.bitbuf & 0xFF) as u8);
            self.bitbuf >>= 8;
            self.bitcnt -= 8;
        }
    }

    #[inline]
    fn write_huff(&mut self, code: u16, len: u8) {
        if len == 0 {
            return;
        }
        let mut c = code;
        let mut rev = 0u32;
        for _ in 0..len {
            rev = (rev << 1) | (c & 1) as u32;
            c >>= 1;
        }
        self.write_bits(rev, len as u32);
    }

    fn finish(mut self) -> Vec<u8> {
        if self.bitcnt > 0 {
            self.out.push((self.bitbuf & 0xFF) as u8);
        }
        self.out
    }
}

fn canonical_codes(lengths: &[u8]) -> Vec<u16> {
    let max_bits = *lengths.iter().max().unwrap_or(&0);
    let mut codes = vec![0u16; lengths.len()];
    if max_bits == 0 {
        return codes;
    }
    let mut bl_count = vec![0u16; max_bits as usize + 1];
    for &l in lengths {
        if l > 0 {
            bl_count[l as usize] += 1;
        }
    }
    let mut next_code = vec![0u16; max_bits as usize + 1];
    let mut code = 0u16;
    for bits in 1..=max_bits as usize {
        code = (code + bl_count[bits - 1]) << 1;
        next_code[bits] = code;
    }
    for (i, &l) in lengths.iter().enumerate() {
        if l > 0 {
            codes[i] = next_code[l as usize];
            next_code[l as usize] += 1;
        }
    }
    codes
}

fn length_limited_huffman(freqs: &[u32], maxlen: u8) -> Vec<u8> {
    let n = freqs.len();
    let mut lengths = vec![0u8; n];
    let symbols: Vec<usize> = (0..n).filter(|&i| freqs[i] > 0).collect();
    let m = symbols.len();
    if m == 0 {
        return lengths;
    }
    if m == 1 {

        let s = symbols[0];
        lengths[s] = 1;
        let filler = if s == 0 { 1 } else { 0 };
        lengths[filler] = 1;
        return lengths;
    }

    let mut syms = symbols.clone();
    syms.sort_by_key(|&s| freqs[s]);
    let leaves: Vec<(u64, Vec<usize>)> = syms.iter().map(|&s| (freqs[s] as u64, vec![s])).collect();

    let mut current: Vec<(u64, Vec<usize>)> = leaves.clone();
    for _ in 1..maxlen {

        let mut packages: Vec<(u64, Vec<usize>)> = Vec::with_capacity(current.len() / 2);
        let mut k = 0;
        while k + 1 < current.len() {
            let mut mem = current[k].1.clone();
            mem.extend_from_slice(&current[k + 1].1);
            packages.push((current[k].0 + current[k + 1].0, mem));
            k += 2;
        }

        let mut merged: Vec<(u64, Vec<usize>)> = Vec::with_capacity(leaves.len() + packages.len());
        let (mut a, mut b) = (0usize, 0usize);
        while a < leaves.len() || b < packages.len() {
            if b >= packages.len() || (a < leaves.len() && leaves[a].0 <= packages[b].0) {
                merged.push(leaves[a].clone());
                a += 1;
            } else {
                merged.push(packages[b].clone());
                b += 1;
            }
        }
        current = merged;
    }

    let take = 2 * m - 2;
    let mut counts = vec![0u32; n];
    for item in current.iter().take(take) {
        for &s in &item.1 {
            counts[s] += 1;
        }
    }
    for s in 0..n {
        lengths[s] = counts[s] as u8;
    }
    lengths
}

#[inline]
fn length_sym(len: u16) -> (usize, u32, u8) {
    let mut idx = 0;
    for j in 0..29 {
        if LENGTH_BASE[j] <= len {
            idx = j;
        } else {
            break;
        }
    }
    (
        257 + idx,
        (len - LENGTH_BASE[idx]) as u32,
        LENGTH_EXTRA[idx],
    )
}

#[inline]
fn dist_sym(dist: u16) -> (usize, u32, u8) {
    let mut idx = 0;
    for j in 0..30 {
        if DIST_BASE[j] <= dist {
            idx = j;
        } else {
            break;
        }
    }
    (idx, (dist - DIST_BASE[idx]) as u32, DIST_EXTRA[idx])
}

fn fixed_litlen_lengths() -> Vec<u8> {
    let mut l = vec![0u8; 288];
    for s in l.iter_mut().take(144) {
        *s = 8;
    }
    for s in l.iter_mut().take(256).skip(144) {
        *s = 9;
    }
    for s in l.iter_mut().take(280).skip(256) {
        *s = 7;
    }
    for s in l.iter_mut().take(288).skip(280) {
        *s = 8;
    }
    l
}

fn fixed_dist_lengths() -> Vec<u8> {
    vec![5u8; 30]
}

fn rle_code_lengths(all: &[u8]) -> Vec<(u8, u32, u8)> {
    let mut out: Vec<(u8, u32, u8)> = Vec::new();
    let n = all.len();
    let mut i = 0;
    while i < n {
        let cur = all[i];
        let mut run = 1;
        while i + run < n && all[i + run] == cur {
            run += 1;
        }
        if cur == 0 {
            let mut r = run;
            while r >= 11 {
                let c = r.min(138);
                out.push((18, (c - 11) as u32, 7));
                r -= c;
            }
            while r >= 3 {
                let c = r.min(10);
                out.push((17, (c - 3) as u32, 3));
                r -= c;
            }
            for _ in 0..r {
                out.push((0, 0, 0));
            }
        } else {
            out.push((cur, 0, 0));
            let mut r = run - 1;
            while r >= 3 {
                let c = r.min(6);
                out.push((16, (c - 3) as u32, 2));
                r -= c;
            }
            for _ in 0..r {
                out.push((cur, 0, 0));
            }
        }
        i += run;
    }
    out
}

fn block_frequencies(syms: &[Sym]) -> ([u32; 288], [u32; 30]) {
    let mut ll = [0u32; 288];
    let mut d = [0u32; 30];
    for s in syms {
        match s {
            Sym::Lit(b) => ll[*b as usize] += 1,
            Sym::Match { len, dist } => {
                let (lc, _, _) = length_sym(*len);
                ll[lc] += 1;
                let (dc, _, _) = dist_sym(*dist);
                d[dc] += 1;
            }
        }
    }
    ll[256] += 1;
    (ll, d)
}

fn emit_symbols(
    bw: &mut BitWriter,
    syms: &[Sym],
    ll_codes: &[u16],
    ll_lens: &[u8],
    d_codes: &[u16],
    d_lens: &[u8],
) {
    for s in syms {
        match s {
            Sym::Lit(b) => bw.write_huff(ll_codes[*b as usize], ll_lens[*b as usize]),
            Sym::Match { len, dist } => {
                let (lc, lev, leb) = length_sym(*len);
                bw.write_huff(ll_codes[lc], ll_lens[lc]);
                if leb > 0 {
                    bw.write_bits(lev, leb as u32);
                }
                let (dc, dev, deb) = dist_sym(*dist);
                bw.write_huff(d_codes[dc], d_lens[dc]);
                if deb > 0 {
                    bw.write_bits(dev, deb as u32);
                }
            }
        }
    }
    bw.write_huff(ll_codes[256], ll_lens[256]);
}

fn encode_fixed(syms: &[Sym]) -> Vec<u8> {
    let mut bw = BitWriter::new();
    bw.write_bits(1, 1);
    bw.write_bits(1, 2);
    let ll_lens = fixed_litlen_lengths();
    let d_lens = fixed_dist_lengths();
    let ll_codes = canonical_codes(&ll_lens);
    let d_codes = canonical_codes(&d_lens);
    emit_symbols(&mut bw, syms, &ll_codes, &ll_lens, &d_codes, &d_lens);
    bw.finish()
}

fn encode_dynamic(syms: &[Sym]) -> Vec<u8> {
    let (ll_freq, d_freq) = block_frequencies(syms);
    let ll_lens = length_limited_huffman(&ll_freq, 15);
    let mut d_lens = length_limited_huffman(&d_freq, 15);
    if d_lens.iter().all(|&x| x == 0) {

        d_lens[0] = 1;
        d_lens[1] = 1;
    }

    let mut bw = BitWriter::new();
    bw.write_bits(1, 1);
    bw.write_bits(2, 2);

    let mut hlit = 286;
    while hlit > 257 && ll_lens[hlit - 1] == 0 {
        hlit -= 1;
    }
    let mut hdist = 30;
    while hdist > 1 && d_lens[hdist - 1] == 0 {
        hdist -= 1;
    }

    let mut combined: Vec<u8> = Vec::with_capacity(hlit + hdist);
    combined.extend_from_slice(&ll_lens[0..hlit]);
    combined.extend_from_slice(&d_lens[0..hdist]);
    let rle = rle_code_lengths(&combined);

    let mut cl_freq = [0u32; 19];
    for (sym, _, _) in &rle {
        cl_freq[*sym as usize] += 1;
    }
    let cl_lens = length_limited_huffman(&cl_freq, 7);
    let cl_codes = canonical_codes(&cl_lens);

    let mut hclen = 19;
    while hclen > 4 && cl_lens[CLCL_ORDER[hclen - 1]] == 0 {
        hclen -= 1;
    }

    bw.write_bits((hlit - 257) as u32, 5);
    bw.write_bits((hdist - 1) as u32, 5);
    bw.write_bits((hclen - 4) as u32, 4);
    for j in 0..hclen {
        bw.write_bits(cl_lens[CLCL_ORDER[j]] as u32, 3);
    }
    for (sym, extra, ebits) in &rle {
        bw.write_huff(cl_codes[*sym as usize], cl_lens[*sym as usize]);
        if *ebits > 0 {
            bw.write_bits(*extra, *ebits as u32);
        }
    }

    let ll_codes = canonical_codes(&ll_lens);
    let d_codes = canonical_codes(&d_lens);
    emit_symbols(&mut bw, syms, &ll_codes, &ll_lens, &d_codes, &d_lens);
    bw.finish()
}

pub fn compressed_deflate(data: &[u8]) -> Vec<u8> {
    let syms = lz77(data);
    let fixed = encode_fixed(&syms);
    let dynamic = encode_dynamic(&syms);
    let stored = deflate_stored(data);

    let mut best = fixed;
    if dynamic.len() < best.len() {
        best = dynamic;
    }
    if stored.len() < best.len() {
        best = stored;
    }
    best
}

pub fn compressed_zlib_deflate(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    out.push(0x78);
    out.push(0x9C);
    out.extend_from_slice(&compressed_deflate(data));
    let a = adler32(data);
    out.push((a >> 24) as u8);
    out.push((a >> 16) as u8);
    out.push((a >> 8) as u8);
    out.push(a as u8);
    out
}

pub fn compressed_gzip_deflate(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&[0x1f, 0x8b, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xff]);
    out.extend_from_slice(&compressed_deflate(data));
    let c = crc32(data);
    out.push(c as u8);
    out.push((c >> 8) as u8);
    out.push((c >> 16) as u8);
    out.push((c >> 24) as u8);
    let isize_le = data.len() as u32;
    out.push(isize_le as u8);
    out.push((isize_le >> 8) as u8);
    out.push((isize_le >> 16) as u8);
    out.push((isize_le >> 24) as u8);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{gunzip, inflate, zlib_inflate};

    fn rt(data: &[u8]) {
        let enc = compressed_deflate(data);
        let dec = inflate(&enc).expect("inflate round-trip");
        assert_eq!(dec, data, "raw round-trip mismatch (len {})", data.len());

        let zenc = compressed_zlib_deflate(data);
        let zdec = zlib_inflate(&zenc).expect("zlib round-trip");
        assert_eq!(zdec, data, "zlib round-trip mismatch");

        let genc = compressed_gzip_deflate(data);
        let gdec = gunzip(&genc).expect("gzip round-trip");
        assert_eq!(gdec, data, "gzip round-trip mismatch");
    }

    #[test]
    fn empty_input() {
        rt(b"");
    }

    #[test]
    fn single_byte() {
        rt(b"A");
        rt(&[0u8]);
        rt(&[0xFFu8]);
    }

    #[test]
    fn short_inputs_use_fixed() {

        let data = b"hello";
        let enc = compressed_deflate(data);
        let btype = (enc[0] >> 1) & 0x03;
        assert_eq!(btype, 1, "small input should select fixed Huffman");
        rt(data);
    }

    #[test]
    fn repeated_pattern_high_ratio() {
        let data = vec![b'x'; 10000];
        let enc = compressed_deflate(&data);
        assert!(
            enc.len() < data.len() / 10,
            "high-redundancy input should compress >10x, got {} -> {}",
            data.len(),
            enc.len()
        );
        rt(&data);
    }

    #[test]
    fn large_input_uses_dynamic() {

        let mut data = Vec::new();
        for i in 0..4000u32 {
            data.push((i % 64) as u8 + 32);
        }
        let enc = compressed_deflate(&data);
        let btype = (enc[0] >> 1) & 0x03;
        assert_eq!(btype, 2, "large input should select dynamic Huffman");
        assert!(enc.len() < data.len(), "should compress");
        rt(&data);
    }

    #[test]
    fn varied_lengths_round_trip() {
        for n in [2usize, 3, 7, 31, 100, 258, 259, 600, 1500, 3000, 70000] {
            let mut data = Vec::with_capacity(n);
            let mut x = 0x12345678u32;
            for _ in 0..n {

                x = x.wrapping_mul(1103515245).wrapping_add(12345);
                data.push((x >> 16) as u8);
            }
            rt(&data);
        }
    }

    #[test]
    fn lz77_back_references() {

        let unit = b"The quick brown fox jumps over the lazy dog. ";
        let mut data = Vec::new();
        for _ in 0..50 {
            data.extend_from_slice(unit);
        }
        let enc = compressed_deflate(&data);
        assert!(
            enc.len() < data.len() / 4,
            "repeated text should compress >4x"
        );
        rt(&data);
    }

    #[test]
    fn incompressible_falls_back_to_stored_bound() {

        let mut data = Vec::with_capacity(5000);
        let mut x = 0xDEADBEEFu32;
        for _ in 0..5000 {
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            data.push(x as u8);
        }
        let enc = compressed_deflate(&data);

        assert!(
            enc.len() <= data.len() + 5,
            "must not expand past stored bound"
        );
        rt(&data);
    }
}
