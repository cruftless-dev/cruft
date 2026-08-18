
use crate::register::{
    make_callable, new_object, register_method, register_method_internal, set_constant,
};
use rusty_js_runtime::value::ObjectRef;
use rusty_js_runtime::{HostEnqueuePhase, Runtime, RuntimeError, Value};
use std::rc::Rc;

fn sha256(data: &[u8]) -> [u8; 32] {
    let h = sha256_core(
        data,
        &[
            0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
            0x5be0cd19,
        ],
    );
    let mut out = [0u8; 32];
    for i in 0..8 {
        out[i * 4..i * 4 + 4].copy_from_slice(&h[i].to_be_bytes());
    }
    out
}

fn sha224(data: &[u8]) -> [u8; 28] {
    let h = sha256_core(
        data,
        &[
            0xc1059ed8, 0x367cd507, 0x3070dd17, 0xf70e5939, 0xffc00b31, 0x68581511, 0x64f98fa7,
            0xbefa4fa4,
        ],
    );
    let mut out = [0u8; 28];
    for i in 0..7 {
        out[i * 4..i * 4 + 4].copy_from_slice(&h[i].to_be_bytes());
    }
    out
}

fn sha256_core(data: &[u8], iv: &[u32; 8]) -> [u32; 8] {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut h: [u32; 8] = *iv;
    let bit_len = (data.len() as u64).wrapping_mul(8);
    let mut padded = data.to_vec();
    padded.push(0x80);
    while padded.len() % 64 != 56 {
        padded.push(0);
    }
    padded.extend_from_slice(&bit_len.to_be_bytes());
    for chunk in padded.chunks_exact(64) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([
                chunk[i * 4],
                chunk[i * 4 + 1],
                chunk[i * 4 + 2],
                chunk[i * 4 + 3],
            ]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let (mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh) =
            (h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7]);
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ (!e & g);
            let t1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let mj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(mj);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
        h[5] = h[5].wrapping_add(f);
        h[6] = h[6].wrapping_add(g);
        h[7] = h[7].wrapping_add(hh);
    }
    h
}

fn sha1(data: &[u8]) -> [u8; 20] {
    let mut h: [u32; 5] = [0x67452301, 0xEFCDAB89, 0x98BADCFE, 0x10325476, 0xC3D2E1F0];
    let bit_len = (data.len() as u64).wrapping_mul(8);
    let mut padded = data.to_vec();
    padded.push(0x80);
    while padded.len() % 64 != 56 {
        padded.push(0);
    }
    padded.extend_from_slice(&bit_len.to_be_bytes());
    for chunk in padded.chunks_exact(64) {
        let mut w = [0u32; 80];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([
                chunk[i * 4],
                chunk[i * 4 + 1],
                chunk[i * 4 + 2],
                chunk[i * 4 + 3],
            ]);
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }
        let (mut a, mut b, mut c, mut d, mut e) = (h[0], h[1], h[2], h[3], h[4]);
        for i in 0..80 {
            let (f, k) = if i < 20 {
                ((b & c) | (!b & d), 0x5A827999)
            } else if i < 40 {
                (b ^ c ^ d, 0x6ED9EBA1)
            } else if i < 60 {
                ((b & c) | (b & d) | (c & d), 0x8F1BBCDC)
            } else {
                (b ^ c ^ d, 0xCA62C1D6)
            };
            let t = a
                .rotate_left(5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(w[i]);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = t;
        }
        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
    }
    let mut out = [0u8; 20];
    for i in 0..5 {
        out[i * 4..i * 4 + 4].copy_from_slice(&h[i].to_be_bytes());
    }
    out
}

fn md5(data: &[u8]) -> [u8; 16] {
    const S: [u32; 64] = [
        7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 5, 9, 14, 20, 5, 9, 14, 20, 5,
        9, 14, 20, 5, 9, 14, 20, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 6, 10,
        15, 21, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21,
    ];
    const K: [u32; 64] = [
        0xd76aa478, 0xe8c7b756, 0x242070db, 0xc1bdceee, 0xf57c0faf, 0x4787c62a, 0xa8304613,
        0xfd469501, 0x698098d8, 0x8b44f7af, 0xffff5bb1, 0x895cd7be, 0x6b901122, 0xfd987193,
        0xa679438e, 0x49b40821, 0xf61e2562, 0xc040b340, 0x265e5a51, 0xe9b6c7aa, 0xd62f105d,
        0x02441453, 0xd8a1e681, 0xe7d3fbc8, 0x21e1cde6, 0xc33707d6, 0xf4d50d87, 0x455a14ed,
        0xa9e3e905, 0xfcefa3f8, 0x676f02d9, 0x8d2a4c8a, 0xfffa3942, 0x8771f681, 0x6d9d6122,
        0xfde5380c, 0xa4beea44, 0x4bdecfa9, 0xf6bb4b60, 0xbebfbc70, 0x289b7ec6, 0xeaa127fa,
        0xd4ef3085, 0x04881d05, 0xd9d4d039, 0xe6db99e5, 0x1fa27cf8, 0xc4ac5665, 0xf4292244,
        0x432aff97, 0xab9423a7, 0xfc93a039, 0x655b59c3, 0x8f0ccc92, 0xffeff47d, 0x85845dd1,
        0x6fa87e4f, 0xfe2ce6e0, 0xa3014314, 0x4e0811a1, 0xf7537e82, 0xbd3af235, 0x2ad7d2bb,
        0xeb86d391,
    ];
    let mut h = [0x67452301u32, 0xefcdab89, 0x98badcfe, 0x10325476];
    let bit_len = (data.len() as u64).wrapping_mul(8);
    let mut padded = data.to_vec();
    padded.push(0x80);
    while padded.len() % 64 != 56 {
        padded.push(0);
    }
    padded.extend_from_slice(&bit_len.to_le_bytes());
    for chunk in padded.chunks_exact(64) {
        let mut m = [0u32; 16];
        for i in 0..16 {
            m[i] = u32::from_le_bytes([
                chunk[i * 4],
                chunk[i * 4 + 1],
                chunk[i * 4 + 2],
                chunk[i * 4 + 3],
            ]);
        }
        let (mut a, mut b, mut c, mut d) = (h[0], h[1], h[2], h[3]);
        for i in 0..64 {
            let (f, g) = if i < 16 {
                ((b & c) | (!b & d), i)
            } else if i < 32 {
                ((d & b) | (!d & c), (5 * i + 1) % 16)
            } else if i < 48 {
                (b ^ c ^ d, (3 * i + 5) % 16)
            } else {
                (c ^ (b | !d), (7 * i) % 16)
            };
            let t = a.wrapping_add(f).wrapping_add(K[i]).wrapping_add(m[g]);
            a = d;
            d = c;
            c = b;
            b = b.wrapping_add(t.rotate_left(S[i]));
        }
        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
    }
    let mut out = [0u8; 16];
    for i in 0..4 {
        out[i * 4..i * 4 + 4].copy_from_slice(&h[i].to_le_bytes());
    }
    out
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0xf) as usize] as char);
    }
    out
}

fn base64_encode(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    let mut i = 0;
    while i + 2 < bytes.len() {
        let b = ((bytes[i] as u32) << 16) | ((bytes[i + 1] as u32) << 8) | (bytes[i + 2] as u32);
        out.push(T[((b >> 18) & 0x3f) as usize] as char);
        out.push(T[((b >> 12) & 0x3f) as usize] as char);
        out.push(T[((b >> 6) & 0x3f) as usize] as char);
        out.push(T[(b & 0x3f) as usize] as char);
        i += 3;
    }
    let rem = bytes.len() - i;
    if rem == 1 {
        let b = (bytes[i] as u32) << 16;
        out.push(T[((b >> 18) & 0x3f) as usize] as char);
        out.push(T[((b >> 12) & 0x3f) as usize] as char);
        out.push_str("==");
    } else if rem == 2 {
        let b = ((bytes[i] as u32) << 16) | ((bytes[i + 1] as u32) << 8);
        out.push(T[((b >> 18) & 0x3f) as usize] as char);
        out.push(T[((b >> 12) & 0x3f) as usize] as char);
        out.push(T[((b >> 6) & 0x3f) as usize] as char);
        out.push('=');
    }
    out
}

fn base64url_encode(bytes: &[u8]) -> String {
    let mut s = base64_encode(bytes);
    s = s.replace('+', "-").replace('/', "_");
    while s.ends_with('=') {
        s.pop();
    }
    s
}

fn base64url_decode(s: &str) -> Vec<u8> {
    let mut t = s.replace('-', "+").replace('_', "/");
    while t.len() % 4 != 0 {
        t.push('=');
    }
    rusty_js_basen::decode_base64(&t).unwrap_or_default()
}

fn ripemd160(data: &[u8]) -> [u8; 20] {
    #[inline]
    fn f(j: usize, x: u32, y: u32, z: u32) -> u32 {
        match j {
            0..=15 => x ^ y ^ z,
            16..=31 => (x & y) | (!x & z),
            32..=47 => (x | !y) ^ z,
            48..=63 => (x & z) | (y & !z),
            _ => x ^ (y | !z),
        }
    }
    const KL: [u32; 5] = [
        0x0000_0000,
        0x5A82_7999,
        0x6ED9_EBA1,
        0x8F1B_BCDC,
        0xA953_FD4E,
    ];
    const KR: [u32; 5] = [
        0x50A2_8BE6,
        0x5C4D_D124,
        0x6D70_3EF3,
        0x7A6D_76E9,
        0x0000_0000,
    ];
    const RL: [usize; 80] = [
        0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 7, 4, 13, 1, 10, 6, 15, 3, 12, 0, 9,
        5, 2, 14, 11, 8, 3, 10, 14, 4, 9, 15, 8, 1, 2, 7, 0, 6, 13, 11, 5, 12, 1, 9, 11, 10, 0, 8,
        12, 4, 13, 3, 7, 15, 14, 5, 6, 2, 4, 0, 5, 9, 7, 12, 2, 10, 14, 1, 3, 8, 11, 6, 15, 13,
    ];
    const RR: [usize; 80] = [
        5, 14, 7, 0, 9, 2, 11, 4, 13, 6, 15, 8, 1, 10, 3, 12, 6, 11, 3, 7, 0, 13, 5, 10, 14, 15, 8,
        12, 4, 9, 1, 2, 15, 5, 1, 3, 7, 14, 6, 9, 11, 8, 12, 2, 10, 0, 4, 13, 8, 6, 4, 1, 3, 11,
        15, 0, 5, 12, 2, 13, 9, 7, 10, 14, 12, 15, 10, 4, 1, 5, 8, 7, 6, 2, 13, 14, 0, 3, 9, 11,
    ];
    const SL: [u32; 80] = [
        11, 14, 15, 12, 5, 8, 7, 9, 11, 13, 14, 15, 6, 7, 9, 8, 7, 6, 8, 13, 11, 9, 7, 15, 7, 12,
        15, 9, 11, 7, 13, 12, 11, 13, 6, 7, 14, 9, 13, 15, 14, 8, 13, 6, 5, 12, 7, 5, 11, 12, 14,
        15, 14, 15, 9, 8, 9, 14, 5, 6, 8, 6, 5, 12, 9, 15, 5, 11, 6, 8, 13, 12, 5, 12, 13, 14, 11,
        8, 5, 6,
    ];
    const SR: [u32; 80] = [
        8, 9, 9, 11, 13, 15, 15, 5, 7, 7, 8, 11, 14, 14, 12, 6, 9, 13, 15, 7, 12, 8, 9, 11, 7, 7,
        12, 7, 6, 15, 13, 11, 9, 7, 15, 11, 8, 6, 6, 14, 12, 13, 5, 14, 13, 13, 7, 5, 15, 5, 8, 11,
        14, 14, 6, 14, 6, 9, 12, 9, 12, 5, 15, 8, 8, 5, 12, 9, 12, 5, 14, 6, 8, 13, 6, 5, 15, 13,
        11, 11,
    ];
    let mut h: [u32; 5] = [
        0x6745_2301,
        0xEFCD_AB89,
        0x98BA_DCFE,
        0x1032_5476,
        0xC3D2_E1F0,
    ];
    let mut msg = data.to_vec();
    let bitlen = (data.len() as u64).wrapping_mul(8);
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bitlen.to_le_bytes());
    for chunk in msg.chunks_exact(64) {
        let mut x = [0u32; 16];
        for (i, w) in x.iter_mut().enumerate() {
            *w = u32::from_le_bytes([
                chunk[i * 4],
                chunk[i * 4 + 1],
                chunk[i * 4 + 2],
                chunk[i * 4 + 3],
            ]);
        }
        let (mut al, mut bl, mut cl, mut dl, mut el) = (h[0], h[1], h[2], h[3], h[4]);
        let (mut ar, mut br, mut cr, mut dr, mut er) = (h[0], h[1], h[2], h[3], h[4]);
        for j in 0..80 {
            let round = j / 16;
            let t = al
                .wrapping_add(f(j, bl, cl, dl))
                .wrapping_add(x[RL[j]])
                .wrapping_add(KL[round])
                .rotate_left(SL[j])
                .wrapping_add(el);
            al = el;
            el = dl;
            dl = cl.rotate_left(10);
            cl = bl;
            bl = t;
            let t2 = ar
                .wrapping_add(f(79 - j, br, cr, dr))
                .wrapping_add(x[RR[j]])
                .wrapping_add(KR[round])
                .rotate_left(SR[j])
                .wrapping_add(er);
            ar = er;
            er = dr;
            dr = cr.rotate_left(10);
            cr = br;
            br = t2;
        }
        let t = h[1].wrapping_add(cl).wrapping_add(dr);
        h[1] = h[2].wrapping_add(dl).wrapping_add(er);
        h[2] = h[3].wrapping_add(el).wrapping_add(ar);
        h[3] = h[4].wrapping_add(al).wrapping_add(br);
        h[4] = h[0].wrapping_add(bl).wrapping_add(cr);
        h[0] = t;
    }
    let mut out = [0u8; 20];
    for (i, hv) in h.iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&hv.to_le_bytes());
    }
    out
}

fn keccak_f1600(a: &mut [u64; 25]) {
    const RC: [u64; 24] = [
        0x0000_0000_0000_0001,
        0x0000_0000_0000_8082,
        0x8000_0000_0000_808a,
        0x8000_0000_8000_8000,
        0x0000_0000_0000_808b,
        0x0000_0000_8000_0001,
        0x8000_0000_8000_8081,
        0x8000_0000_0000_8009,
        0x0000_0000_0000_008a,
        0x0000_0000_0000_0088,
        0x0000_0000_8000_8009,
        0x0000_0000_8000_000a,
        0x0000_0000_8000_808b,
        0x8000_0000_0000_008b,
        0x8000_0000_0000_8089,
        0x8000_0000_0000_8003,
        0x8000_0000_0000_8002,
        0x8000_0000_0000_0080,
        0x0000_0000_0000_800a,
        0x8000_0000_8000_000a,
        0x8000_0000_8000_8081,
        0x8000_0000_0000_8080,
        0x0000_0000_8000_0001,
        0x8000_0000_8000_8008,
    ];
    const RHO: [u32; 24] = [
        1, 3, 6, 10, 15, 21, 28, 36, 45, 55, 2, 14, 27, 41, 56, 8, 25, 43, 62, 18, 39, 61, 20, 44,
    ];
    const PI: [usize; 24] = [
        10, 7, 11, 17, 18, 3, 5, 16, 8, 21, 24, 4, 15, 23, 19, 13, 12, 2, 20, 14, 22, 9, 6, 1,
    ];
    for rc in RC.iter() {

        let mut c = [0u64; 5];
        for x in 0..5 {
            c[x] = a[x] ^ a[x + 5] ^ a[x + 10] ^ a[x + 15] ^ a[x + 20];
        }
        for x in 0..5 {
            let d = c[(x + 4) % 5] ^ c[(x + 1) % 5].rotate_left(1);
            for y in 0..5 {
                a[x + 5 * y] ^= d;
            }
        }

        let mut last = a[1];
        for i in 0..24 {
            let j = PI[i];
            let tmp = a[j];
            a[j] = last.rotate_left(RHO[i]);
            last = tmp;
        }

        for y in 0..5 {
            let row = [
                a[5 * y],
                a[5 * y + 1],
                a[5 * y + 2],
                a[5 * y + 3],
                a[5 * y + 4],
            ];
            for x in 0..5 {
                a[x + 5 * y] = row[x] ^ ((!row[(x + 1) % 5]) & row[(x + 2) % 5]);
            }
        }

        a[0] ^= *rc;
    }
}

fn sha3(data: &[u8], rate_bytes: usize, out_len: usize) -> Vec<u8> {
    keccak_xof(data, rate_bytes, out_len, 0x06)
}

fn shake(data: &[u8], rate_bytes: usize, out_len: usize) -> Vec<u8> {
    keccak_xof(data, rate_bytes, out_len, 0x1f)
}

fn keccak_xof(data: &[u8], rate_bytes: usize, out_len: usize, dom: u8) -> Vec<u8> {
    let mut buf = data.to_vec();
    let padlen = rate_bytes - (buf.len() % rate_bytes);
    if padlen == 1 {
        buf.push(dom | 0x80);
    } else {
        buf.push(dom);
        buf.extend(std::iter::repeat(0u8).take(padlen - 2));
        buf.push(0x80);
    }
    let mut state = [0u64; 25];
    for block in buf.chunks_exact(rate_bytes) {
        for i in 0..(rate_bytes / 8) {
            let lane = u64::from_le_bytes([
                block[i * 8],
                block[i * 8 + 1],
                block[i * 8 + 2],
                block[i * 8 + 3],
                block[i * 8 + 4],
                block[i * 8 + 5],
                block[i * 8 + 6],
                block[i * 8 + 7],
            ]);
            state[i] ^= lane;
        }
        keccak_f1600(&mut state);
    }
    let mut out = Vec::with_capacity(out_len);
    'squeeze: loop {
        for i in 0..(rate_bytes / 8) {
            for b in state[i].to_le_bytes() {
                out.push(b);
                if out.len() == out_len {
                    break 'squeeze;
                }
            }
        }
        keccak_f1600(&mut state);
    }
    out
}

fn blake2b512(data: &[u8]) -> [u8; 64] {
    const IV: [u64; 8] = [
        0x6a09e667f3bcc908,
        0xbb67ae8584caa73b,
        0x3c6ef372fe94f82b,
        0xa54ff53a5f1d36f1,
        0x510e527fade682d1,
        0x9b05688c2b3e6c1f,
        0x1f83d9abfb41bd6b,
        0x5be0cd19137e2179,
    ];
    const SIGMA: [[usize; 16]; 12] = [
        [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
        [14, 10, 4, 8, 9, 15, 13, 6, 1, 12, 0, 2, 11, 7, 5, 3],
        [11, 8, 12, 0, 5, 2, 15, 13, 10, 14, 3, 6, 7, 1, 9, 4],
        [7, 9, 3, 1, 13, 12, 11, 14, 2, 6, 5, 10, 4, 0, 15, 8],
        [9, 0, 5, 7, 2, 4, 10, 15, 14, 1, 11, 12, 6, 8, 3, 13],
        [2, 12, 6, 10, 0, 11, 8, 3, 4, 13, 7, 5, 15, 14, 1, 9],
        [12, 5, 1, 15, 14, 13, 4, 10, 0, 7, 6, 3, 9, 2, 8, 11],
        [13, 11, 7, 14, 12, 1, 3, 9, 5, 0, 15, 4, 8, 6, 2, 10],
        [6, 15, 14, 9, 11, 3, 0, 8, 12, 2, 13, 7, 1, 4, 10, 5],
        [10, 2, 8, 4, 7, 6, 1, 5, 15, 11, 9, 14, 3, 12, 13, 0],
        [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
        [14, 10, 4, 8, 9, 15, 13, 6, 1, 12, 0, 2, 11, 7, 5, 3],
    ];
    #[inline]
    fn g(v: &mut [u64; 16], a: usize, b: usize, c: usize, d: usize, x: u64, y: u64) {
        v[a] = v[a].wrapping_add(v[b]).wrapping_add(x);
        v[d] = (v[d] ^ v[a]).rotate_right(32);
        v[c] = v[c].wrapping_add(v[d]);
        v[b] = (v[b] ^ v[c]).rotate_right(24);
        v[a] = v[a].wrapping_add(v[b]).wrapping_add(y);
        v[d] = (v[d] ^ v[a]).rotate_right(16);
        v[c] = v[c].wrapping_add(v[d]);
        v[b] = (v[b] ^ v[c]).rotate_right(63);
    }
    fn compress(h: &mut [u64; 8], block: &[u8; 128], t: u128, last: bool) {
        let mut m = [0u64; 16];
        for (i, w) in m.iter_mut().enumerate() {
            *w = u64::from_le_bytes(block[i * 8..i * 8 + 8].try_into().unwrap());
        }
        let mut v = [0u64; 16];
        v[..8].copy_from_slice(h);
        v[8..].copy_from_slice(&IV);
        v[12] ^= t as u64;
        v[13] ^= (t >> 64) as u64;
        if last {
            v[14] ^= 0xffff_ffff_ffff_ffff;
        }
        for s in SIGMA.iter() {
            g(&mut v, 0, 4, 8, 12, m[s[0]], m[s[1]]);
            g(&mut v, 1, 5, 9, 13, m[s[2]], m[s[3]]);
            g(&mut v, 2, 6, 10, 14, m[s[4]], m[s[5]]);
            g(&mut v, 3, 7, 11, 15, m[s[6]], m[s[7]]);
            g(&mut v, 0, 5, 10, 15, m[s[8]], m[s[9]]);
            g(&mut v, 1, 6, 11, 12, m[s[10]], m[s[11]]);
            g(&mut v, 2, 7, 8, 13, m[s[12]], m[s[13]]);
            g(&mut v, 3, 4, 9, 14, m[s[14]], m[s[15]]);
        }
        for i in 0..8 {
            h[i] ^= v[i] ^ v[i + 8];
        }
    }
    let mut h = IV;

    h[0] ^= 0x0101_0000 ^ 64;
    let ll = data.len();
    if ll == 0 {
        compress(&mut h, &[0u8; 128], 0, true);
    } else {
        let non_last = (ll - 1) / 128;
        for i in 0..non_last {
            let mut block = [0u8; 128];
            block.copy_from_slice(&data[i * 128..i * 128 + 128]);
            compress(&mut h, &block, ((i as u128) + 1) * 128, false);
        }
        let start = non_last * 128;
        let mut block = [0u8; 128];
        block[..ll - start].copy_from_slice(&data[start..]);
        compress(&mut h, &block, ll as u128, true);
    }
    let mut out = [0u8; 64];
    for (i, word) in h.iter().enumerate() {
        out[i * 8..i * 8 + 8].copy_from_slice(&word.to_le_bytes());
    }
    out
}

fn blake2s256(data: &[u8]) -> [u8; 32] {
    const IV: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    const SIGMA: [[usize; 16]; 10] = [
        [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
        [14, 10, 4, 8, 9, 15, 13, 6, 1, 12, 0, 2, 11, 7, 5, 3],
        [11, 8, 12, 0, 5, 2, 15, 13, 10, 14, 3, 6, 7, 1, 9, 4],
        [7, 9, 3, 1, 13, 12, 11, 14, 2, 6, 5, 10, 4, 0, 15, 8],
        [9, 0, 5, 7, 2, 4, 10, 15, 14, 1, 11, 12, 6, 8, 3, 13],
        [2, 12, 6, 10, 0, 11, 8, 3, 4, 13, 7, 5, 15, 14, 1, 9],
        [12, 5, 1, 15, 14, 13, 4, 10, 0, 7, 6, 3, 9, 2, 8, 11],
        [13, 11, 7, 14, 12, 1, 3, 9, 5, 0, 15, 4, 8, 6, 2, 10],
        [6, 15, 14, 9, 11, 3, 0, 8, 12, 2, 13, 7, 1, 4, 10, 5],
        [10, 2, 8, 4, 7, 6, 1, 5, 15, 11, 9, 14, 3, 12, 13, 0],
    ];
    #[inline]
    fn g(v: &mut [u32; 16], a: usize, b: usize, c: usize, d: usize, x: u32, y: u32) {
        v[a] = v[a].wrapping_add(v[b]).wrapping_add(x);
        v[d] = (v[d] ^ v[a]).rotate_right(16);
        v[c] = v[c].wrapping_add(v[d]);
        v[b] = (v[b] ^ v[c]).rotate_right(12);
        v[a] = v[a].wrapping_add(v[b]).wrapping_add(y);
        v[d] = (v[d] ^ v[a]).rotate_right(8);
        v[c] = v[c].wrapping_add(v[d]);
        v[b] = (v[b] ^ v[c]).rotate_right(7);
    }
    fn compress(h: &mut [u32; 8], block: &[u8; 64], t: u64, last: bool) {
        let mut m = [0u32; 16];
        for (i, w) in m.iter_mut().enumerate() {
            *w = u32::from_le_bytes(block[i * 4..i * 4 + 4].try_into().unwrap());
        }
        let mut v = [0u32; 16];
        v[..8].copy_from_slice(h);
        v[8..].copy_from_slice(&IV);
        v[12] ^= t as u32;
        v[13] ^= (t >> 32) as u32;
        if last {
            v[14] ^= 0xffff_ffff;
        }
        for s in SIGMA.iter() {
            g(&mut v, 0, 4, 8, 12, m[s[0]], m[s[1]]);
            g(&mut v, 1, 5, 9, 13, m[s[2]], m[s[3]]);
            g(&mut v, 2, 6, 10, 14, m[s[4]], m[s[5]]);
            g(&mut v, 3, 7, 11, 15, m[s[6]], m[s[7]]);
            g(&mut v, 0, 5, 10, 15, m[s[8]], m[s[9]]);
            g(&mut v, 1, 6, 11, 12, m[s[10]], m[s[11]]);
            g(&mut v, 2, 7, 8, 13, m[s[12]], m[s[13]]);
            g(&mut v, 3, 4, 9, 14, m[s[14]], m[s[15]]);
        }
        for i in 0..8 {
            h[i] ^= v[i] ^ v[i + 8];
        }
    }
    let mut h = IV;

    h[0] ^= 0x0101_0000 ^ 32;
    let ll = data.len();
    if ll == 0 {
        compress(&mut h, &[0u8; 64], 0, true);
    } else {
        let non_last = (ll - 1) / 64;
        for i in 0..non_last {
            let mut block = [0u8; 64];
            block.copy_from_slice(&data[i * 64..i * 64 + 64]);
            compress(&mut h, &block, ((i as u64) + 1) * 64, false);
        }
        let start = non_last * 64;
        let mut block = [0u8; 64];
        block[..ll - start].copy_from_slice(&data[start..]);
        compress(&mut h, &block, ll as u64, true);
    }
    let mut out = [0u8; 32];
    for (i, word) in h.iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&word.to_le_bytes());
    }
    out
}

fn sha512_core(data: &[u8], iv: &[u64; 8]) -> [u64; 8] {
    const K: [u64; 80] = [
        0x428a2f98d728ae22,
        0x7137449123ef65cd,
        0xb5c0fbcfec4d3b2f,
        0xe9b5dba58189dbbc,
        0x3956c25bf348b538,
        0x59f111f1b605d019,
        0x923f82a4af194f9b,
        0xab1c5ed5da6d8118,
        0xd807aa98a3030242,
        0x12835b0145706fbe,
        0x243185be4ee4b28c,
        0x550c7dc3d5ffb4e2,
        0x72be5d74f27b896f,
        0x80deb1fe3b1696b1,
        0x9bdc06a725c71235,
        0xc19bf174cf692694,
        0xe49b69c19ef14ad2,
        0xefbe4786384f25e3,
        0x0fc19dc68b8cd5b5,
        0x240ca1cc77ac9c65,
        0x2de92c6f592b0275,
        0x4a7484aa6ea6e483,
        0x5cb0a9dcbd41fbd4,
        0x76f988da831153b5,
        0x983e5152ee66dfab,
        0xa831c66d2db43210,
        0xb00327c898fb213f,
        0xbf597fc7beef0ee4,
        0xc6e00bf33da88fc2,
        0xd5a79147930aa725,
        0x06ca6351e003826f,
        0x142929670a0e6e70,
        0x27b70a8546d22ffc,
        0x2e1b21385c26c926,
        0x4d2c6dfc5ac42aed,
        0x53380d139d95b3df,
        0x650a73548baf63de,
        0x766a0abb3c77b2a8,
        0x81c2c92e47edaee6,
        0x92722c851482353b,
        0xa2bfe8a14cf10364,
        0xa81a664bbc423001,
        0xc24b8b70d0f89791,
        0xc76c51a30654be30,
        0xd192e819d6ef5218,
        0xd69906245565a910,
        0xf40e35855771202a,
        0x106aa07032bbd1b8,
        0x19a4c116b8d2d0c8,
        0x1e376c085141ab53,
        0x2748774cdf8eeb99,
        0x34b0bcb5e19b48a8,
        0x391c0cb3c5c95a63,
        0x4ed8aa4ae3418acb,
        0x5b9cca4f7763e373,
        0x682e6ff3d6b2b8a3,
        0x748f82ee5defb2fc,
        0x78a5636f43172f60,
        0x84c87814a1f0ab72,
        0x8cc702081a6439ec,
        0x90befffa23631e28,
        0xa4506cebde82bde9,
        0xbef9a3f7b2c67915,
        0xc67178f2e372532b,
        0xca273eceea26619c,
        0xd186b8c721c0c207,
        0xeada7dd6cde0eb1e,
        0xf57d4f7fee6ed178,
        0x06f067aa72176fba,
        0x0a637dc5a2c898a6,
        0x113f9804bef90dae,
        0x1b710b35131c471b,
        0x28db77f523047d84,
        0x32caab7b40c72493,
        0x3c9ebe0a15c9bebc,
        0x431d67c49c100d4c,
        0x4cc5d4becb3e42b6,
        0x597f299cfc657e2a,
        0x5fcb6fab3ad6faec,
        0x6c44198c4a475817,
    ];
    let mut h: [u64; 8] = *iv;
    let bit_len = (data.len() as u128).wrapping_mul(8);
    let mut padded = data.to_vec();
    padded.push(0x80);
    while padded.len() % 128 != 112 {
        padded.push(0);
    }
    padded.extend_from_slice(&bit_len.to_be_bytes());
    for chunk in padded.chunks_exact(128) {
        let mut w = [0u64; 80];
        for (i, word) in w.iter_mut().take(16).enumerate() {
            *word = u64::from_be_bytes(chunk[i * 8..i * 8 + 8].try_into().unwrap());
        }
        for i in 16..80 {
            let s0 = w[i - 15].rotate_right(1) ^ w[i - 15].rotate_right(8) ^ (w[i - 15] >> 7);
            let s1 = w[i - 2].rotate_right(19) ^ w[i - 2].rotate_right(61) ^ (w[i - 2] >> 6);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let (mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh) =
            (h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7]);
        for i in 0..80 {
            let s1 = e.rotate_right(14) ^ e.rotate_right(18) ^ e.rotate_right(41);
            let ch = (e & f) ^ (!e & g);
            let t1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(28) ^ a.rotate_right(34) ^ a.rotate_right(39);
            let mj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(mj);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
        h[5] = h[5].wrapping_add(f);
        h[6] = h[6].wrapping_add(g);
        h[7] = h[7].wrapping_add(hh);
    }
    h
}

fn sha512_256(data: &[u8]) -> [u8; 32] {
    let h = sha512_core(
        data,
        &[
            0x22312194fc2bf72c,
            0x9f555fa3c84c64c2,
            0x2393b86b6f53b151,
            0x963877195940eabd,
            0x96283ee2a88effe3,
            0xbe5e1e2553863992,
            0x2b0199fc2c85b8aa,
            0x0eb72ddc81c52ca2,
        ],
    );
    let mut out = [0u8; 32];
    for i in 0..4 {
        out[i * 8..i * 8 + 8].copy_from_slice(&h[i].to_be_bytes());
    }
    out
}

fn sha512_224(data: &[u8]) -> [u8; 28] {
    let h = sha512_core(
        data,
        &[
            0x8c3d37c819544da2,
            0x73e1996689dcd4d6,
            0x1dfab7ae32ff9c82,
            0x679dd514582f9fcf,
            0x0f6d2b697bd44da8,
            0x77e36f7304c48942,
            0x3f9d85a86a1d36c8,
            0x1112e6ad91d692a1,
        ],
    );
    let mut out = [0u8; 28];

    for i in 0..3 {
        out[i * 8..i * 8 + 8].copy_from_slice(&h[i].to_be_bytes());
    }
    out[24..28].copy_from_slice(&h[3].to_be_bytes()[..4]);
    out
}

fn sm3(data: &[u8]) -> [u8; 32] {
    #[inline]
    fn p0(x: u32) -> u32 {
        x ^ x.rotate_left(9) ^ x.rotate_left(17)
    }
    #[inline]
    fn p1(x: u32) -> u32 {
        x ^ x.rotate_left(15) ^ x.rotate_left(23)
    }
    let mut v: [u32; 8] = [
        0x7380166f, 0x4914b2b9, 0x172442d7, 0xda8a0600, 0xa96f30bc, 0x163138aa, 0xe38dee4d,
        0xb0fb0e4e,
    ];
    let bit_len = (data.len() as u64).wrapping_mul(8);
    let mut padded = data.to_vec();
    padded.push(0x80);
    while padded.len() % 64 != 56 {
        padded.push(0);
    }
    padded.extend_from_slice(&bit_len.to_be_bytes());
    for chunk in padded.chunks_exact(64) {
        let mut w = [0u32; 68];
        for (i, word) in w.iter_mut().take(16).enumerate() {
            *word = u32::from_be_bytes(chunk[i * 4..i * 4 + 4].try_into().unwrap());
        }
        for j in 16..68 {
            w[j] = p1(w[j - 16] ^ w[j - 9] ^ w[j - 3].rotate_left(15))
                ^ w[j - 13].rotate_left(7)
                ^ w[j - 6];
        }
        let (mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h) =
            (v[0], v[1], v[2], v[3], v[4], v[5], v[6], v[7]);
        for j in 0..64 {
            let t = if j < 16 { 0x79cc4519u32 } else { 0x7a879d8au32 };
            let ss1 = a
                .rotate_left(12)
                .wrapping_add(e)
                .wrapping_add(t.rotate_left((j % 32) as u32))
                .rotate_left(7);
            let ss2 = ss1 ^ a.rotate_left(12);
            let (ff, gg) = if j < 16 {
                (a ^ b ^ c, e ^ f ^ g)
            } else {
                ((a & b) | (a & c) | (b & c), (e & f) | (!e & g))
            };
            let wpj = w[j] ^ w[j + 4];
            let tt1 = ff.wrapping_add(d).wrapping_add(ss2).wrapping_add(wpj);
            let tt2 = gg.wrapping_add(h).wrapping_add(ss1).wrapping_add(w[j]);
            d = c;
            c = b.rotate_left(9);
            b = a;
            a = tt1;
            h = g;
            g = f.rotate_left(19);
            f = e;
            e = p0(tt2);
        }
        v[0] ^= a;
        v[1] ^= b;
        v[2] ^= c;
        v[3] ^= d;
        v[4] ^= e;
        v[5] ^= f;
        v[6] ^= g;
        v[7] ^= h;
    }
    let mut out = [0u8; 32];
    for i in 0..8 {
        out[i * 4..i * 4 + 4].copy_from_slice(&v[i].to_be_bytes());
    }
    out
}

fn digest_bytes(alg: &str, data: &[u8]) -> Vec<u8> {
    match normalize_digest(alg).as_str() {
        "sha256" => sha256(data).to_vec(),
        "sha224" => sha224(data).to_vec(),
        "sha1" => sha1(data).to_vec(),
        "md5" => md5(data).to_vec(),
        "sha384" => rusty_web_crypto::digest_sha384(data).to_vec(),
        "sha512" => rusty_web_crypto::digest_sha512(data).to_vec(),

        "ripemd160" | "rmd160" | "ripemd" => ripemd160(data).to_vec(),

        "sha3256" => sha3(data, 136, 32),
        "sha3512" => sha3(data, 72, 64),
        "sha3384" => sha3(data, 104, 48),
        "sha3224" => sha3(data, 144, 28),
        "blake2b512" => blake2b512(data).to_vec(),
        "blake2s256" => blake2s256(data).to_vec(),

        "md5sha1" => {
            let mut v = md5(data).to_vec();
            v.extend_from_slice(&sha1(data));
            v
        }
        "sha512256" => sha512_256(data).to_vec(),
        "sha512224" => sha512_224(data).to_vec(),
        "sm3" => sm3(data).to_vec(),
        _ => sha256(data).to_vec(),
    }
}

fn digest_bytes_len(alg: &str, data: &[u8], out_len: Option<usize>) -> Vec<u8> {
    match normalize_digest(alg).as_str() {

        "shake128" => shake(data, 168, out_len.unwrap_or(16)),
        "shake256" => shake(data, 136, out_len.unwrap_or(32)),
        _ => digest_bytes(alg, data),
    }
}

fn normalize_digest(alg: &str) -> String {
    alg.to_ascii_lowercase().replace('-', "")
}

fn digest_supported(alg: &str) -> bool {
    matches!(
        normalize_digest(alg).as_str(),
        "sha1"
            | "sha224"
            | "sha256"
            | "sha384"
            | "sha512"
            | "md5"
            | "ripemd160"
            | "rmd160"
            | "ripemd"
            | "sha3224"
            | "sha3256"
            | "sha3384"
            | "sha3512"
            | "shake128"
            | "shake256"
            | "blake2b512"
            | "blake2s256"
            | "md5sha1"
            | "sha512256"
            | "sha512224"
            | "sm3"
    )
}

fn value_string(v: Option<&Value>) -> String {
    match v {
        Some(Value::String(s)) => s.as_str().to_string(),
        Some(Value::Number(n)) => {
            if n.fract() == 0.0 {
                (*n as i64).to_string()
            } else {
                n.to_string()
            }
        }
        Some(Value::Boolean(b)) => b.to_string(),
        _ => String::new(),
    }
}

fn object_len(rt: &mut Runtime, id: ObjectRef) -> usize {
    match rt.object_get(id, "length") {
        Value::Number(n) if n.is_finite() && n > 0.0 => n as usize,
        _ => 0,
    }
}

fn extract_bytes(rt: &mut Runtime, v: &Value) -> Vec<u8> {
    match v {
        Value::String(s) => s.as_bytes().to_vec(),
        Value::Object(id) => {

            if let Some(rec) = rt.array_buffers.get(id) {
                return rec.data.clone();
            }

            if let Value::String(hex) = rt.object_get(*id, "__keybytes") {
                let h = hex.as_bytes();
                let mut out = Vec::with_capacity(h.len() / 2);
                let mut i = 0;
                while i + 1 < h.len() {
                    let hi = (h[i] as char).to_digit(16);
                    let lo = (h[i + 1] as char).to_digit(16);
                    if let (Some(hi), Some(lo)) = (hi, lo) {
                        out.push(((hi << 4) | lo) as u8);
                    }
                    i += 2;
                }
                return out;
            }
            let len = object_len(rt, *id);
            let mut out = Vec::with_capacity(len);
            for i in 0..len {
                match rt.object_get(*id, &i.to_string()) {
                    Value::Number(n) => out.push(n as u8),
                    _ => {}
                }
            }
            out
        }
        _ => Vec::new(),
    }
}

fn make_buffer(rt: &mut Runtime, bytes: &[u8]) -> Value {

    crate::node_stubs::intrinsic_buffer_from_bytes(rt, bytes)
}

fn make_arraybuffer(rt: &mut Runtime, bytes: &[u8]) -> Value {
    let mut ab = rusty_js_runtime::value::Object::new_ordinary();
    ab.proto = rt.array_buffer_prototype;
    let ab_id = rt.alloc_object(ab);
    rt.array_buffers.insert(
        ab_id,
        rusty_js_runtime::interp::ArrayBufferRecord {
            byte_length: bytes.len(),
            max_byte_length: bytes.len(),
            backing_epoch: 0,
            data: bytes.to_vec(),
            detached: false,
            untransferable: false,
            shared: None,
        },
    );
    Value::Object(ab_id)
}

fn push_bytes(rt: &mut Runtime, id: ObjectRef, bytes: &[u8]) {
    let cur_len = object_len(rt, id);
    for (i, b) in bytes.iter().enumerate() {
        rt.object_set(id, (cur_len + i).to_string(), Value::Number(*b as f64));
    }
    rt.object_set(
        id,
        "length".into(),
        Value::Number((cur_len + bytes.len()) as f64),
    );
}

fn set_bytes_prop(rt: &mut Runtime, id: ObjectRef, prop: &str, bytes: &[u8]) {
    let v = make_buffer(rt, bytes);
    rt.object_set(id, prop.into(), v);
}

fn get_bytes_prop(rt: &mut Runtime, id: ObjectRef, prop: &str) -> Vec<u8> {
    let v = rt.object_get(id, prop);
    extract_bytes(rt, &v)
}

fn crypto_error(msg: impl Into<String>) -> RuntimeError {
    RuntimeError::TypeError(msg.into())
}

fn sign_digest_supported(algo: &str) -> bool {
    matches!(
        algo.to_ascii_lowercase().replace("rsa-", "").as_str(),
        "sha1" | "sha256" | "sha384" | "sha512"
    )
}

fn chacha20_block(key: &[u8; 32], nonce: &[u8; 12], counter: u32) -> [u8; 64] {
    #[inline]
    fn qr(s: &mut [u32; 16], a: usize, b: usize, c: usize, d: usize) {
        s[a] = s[a].wrapping_add(s[b]);
        s[d] = (s[d] ^ s[a]).rotate_left(16);
        s[c] = s[c].wrapping_add(s[d]);
        s[b] = (s[b] ^ s[c]).rotate_left(12);
        s[a] = s[a].wrapping_add(s[b]);
        s[d] = (s[d] ^ s[a]).rotate_left(8);
        s[c] = s[c].wrapping_add(s[d]);
        s[b] = (s[b] ^ s[c]).rotate_left(7);
    }
    let mut state = [0u32; 16];
    state[0] = 0x6170_7865;
    state[1] = 0x3320_646e;
    state[2] = 0x7962_2d32;
    state[3] = 0x6b20_6574;
    for i in 0..8 {
        state[4 + i] = u32::from_le_bytes(key[i * 4..i * 4 + 4].try_into().unwrap());
    }
    state[12] = counter;
    for i in 0..3 {
        state[13 + i] = u32::from_le_bytes(nonce[i * 4..i * 4 + 4].try_into().unwrap());
    }
    let mut w = state;
    for _ in 0..10 {
        qr(&mut w, 0, 4, 8, 12);
        qr(&mut w, 1, 5, 9, 13);
        qr(&mut w, 2, 6, 10, 14);
        qr(&mut w, 3, 7, 11, 15);
        qr(&mut w, 0, 5, 10, 15);
        qr(&mut w, 1, 6, 11, 12);
        qr(&mut w, 2, 7, 8, 13);
        qr(&mut w, 3, 4, 9, 14);
    }
    let mut out = [0u8; 64];
    for i in 0..16 {
        let v = w[i].wrapping_add(state[i]);
        out[i * 4..i * 4 + 4].copy_from_slice(&v.to_le_bytes());
    }
    out
}

fn chacha20_xor(key: &[u8; 32], nonce: &[u8; 12], counter0: u32, data: &[u8]) -> Vec<u8> {
    let mut out = data.to_vec();
    let mut counter = counter0;
    for chunk in out.chunks_mut(64) {
        let ks = chacha20_block(key, nonce, counter);
        for (b, k) in chunk.iter_mut().zip(ks.iter()) {
            *b ^= *k;
        }
        counter = counter.wrapping_add(1);
    }
    out
}

fn poly1305_mac(msg: &[u8], otk: &[u8; 32]) -> [u8; 16] {
    use rusty_web_crypto::BigUInt;

    let mut p_be = [0xffu8; 17];
    p_be[0] = 0x03;
    p_be[16] = 0xfb;
    let p = BigUInt::from_be_bytes(&p_be);

    let mut rb = [0u8; 16];
    rb.copy_from_slice(&otk[0..16]);
    rb[3] &= 15;
    rb[7] &= 15;
    rb[11] &= 15;
    rb[15] &= 15;
    rb[4] &= 252;
    rb[8] &= 252;
    rb[12] &= 252;
    rb.reverse();
    let r = BigUInt::from_be_bytes(&rb);
    let mut sb = [0u8; 16];
    sb.copy_from_slice(&otk[16..32]);
    sb.reverse();
    let s = BigUInt::from_be_bytes(&sb);
    let mut acc = BigUInt::zero();
    for chunk in msg.chunks(16) {

        let mut blk = chunk.to_vec();
        blk.push(0x01);
        blk.reverse();
        let n = BigUInt::from_be_bytes(&blk);
        acc = acc.add(&n).modulo(&p);
        acc = acc.mul(&r).modulo(&p);
    }
    acc = acc.add(&s);

    let be = acc.to_be_bytes(32);
    let mut tag = [0u8; 16];
    tag.copy_from_slice(&be[16..32]);
    tag.reverse();
    tag
}

fn chacha20_poly1305_seal(
    key: &[u8; 32],
    nonce: &[u8; 12],
    aad: &[u8],
    pt: &[u8],
) -> (Vec<u8>, [u8; 16]) {
    let otk_block = chacha20_block(key, nonce, 0);
    let mut otk = [0u8; 32];
    otk.copy_from_slice(&otk_block[0..32]);
    let ct = chacha20_xor(key, nonce, 1, pt);
    let tag = poly1305_mac(&aead_mac_data(aad, &ct), &otk);
    (ct, tag)
}

fn chacha20_poly1305_open(
    key: &[u8; 32],
    nonce: &[u8; 12],
    aad: &[u8],
    ct: &[u8],
    tag: &[u8],
) -> Option<Vec<u8>> {
    let otk_block = chacha20_block(key, nonce, 0);
    let mut otk = [0u8; 32];
    otk.copy_from_slice(&otk_block[0..32]);
    let expected = poly1305_mac(&aead_mac_data(aad, ct), &otk);
    if tag.len() != 16 || expected != tag {
        return None;
    }
    Some(chacha20_xor(key, nonce, 1, ct))
}

fn aead_mac_data(aad: &[u8], ct: &[u8]) -> Vec<u8> {
    let mut m = Vec::with_capacity(aad.len() + ct.len() + 32);
    m.extend_from_slice(aad);
    while m.len() % 16 != 0 {
        m.push(0);
    }
    m.extend_from_slice(ct);
    while m.len() % 16 != 0 {
        m.push(0);
    }
    m.extend_from_slice(&(aad.len() as u64).to_le_bytes());
    m.extend_from_slice(&(ct.len() as u64).to_le_bytes());
    m
}

fn cipher_supported(algo: &str) -> bool {
    matches!(
        algo.to_ascii_lowercase().as_str(),
        "aes-128-cbc"
            | "aes-192-cbc"
            | "aes-256-cbc"
            | "aes-128-ctr"
            | "aes-192-ctr"
            | "aes-256-ctr"
            | "aes-128-gcm"
            | "aes-192-gcm"
            | "aes-256-gcm"
            | "aes-128-ecb"
            | "aes-192-ecb"
            | "aes-256-ecb"
            | "aes-128-cfb"
            | "aes-192-cfb"
            | "aes-256-cfb"
            | "aes-128-ofb"
            | "aes-192-ofb"
            | "aes-256-ofb"
            | "chacha20-poly1305"
            | "chacha20"
    )
}

fn cipher_key_len(algo: &str) -> usize {
    if algo.contains("128") {
        16
    } else if algo.contains("192") {
        24
    } else {
        32
    }
}

fn cipher_iv_len(algo: &str) -> Option<usize> {
    if algo.contains("gcm") {
        None
    } else if algo.contains("ecb") {
        Some(0)
    } else if algo == "chacha20-poly1305" {
        Some(12)
    } else {

        Some(16)
    }
}

fn hash_finalized_error(rt: &mut Runtime) -> RuntimeError {
    coded_crypto_error(
        rt,
        "Error",
        "ERR_CRYPTO_HASH_FINALIZED",
        "Digest already called",
    )
}

fn hash_is_finalized(rt: &Runtime, id: ObjectRef) -> bool {
    matches!(rt.object_get(id, "__finalized__"), Value::Boolean(true))
}

fn coded_crypto_error(rt: &mut Runtime, kind: &str, code: &str, msg: &str) -> RuntimeError {
    match rusty_js_runtime::intrinsics::make_error_instance(rt, kind, msg) {
        Some(id) => {
            rt.object_set(
                id,
                "code".into(),
                Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                    code.to_string(),
                ))),
            );
            RuntimeError::Thrown(Value::Object(id))
        }
        None => RuntimeError::TypeError(msg.to_string()),
    }
}

fn validate_cipher_key_iv(
    rt: &mut Runtime,
    algo: &str,
    key: &[u8],
    iv: &[u8],
) -> Result<(), RuntimeError> {
    if key.len() != cipher_key_len(algo) {
        return Err(coded_crypto_error(
            rt,
            "RangeError",
            "ERR_CRYPTO_INVALID_KEYLEN",
            "Invalid key length",
        ));
    }
    let iv_ok = match cipher_iv_len(algo) {
        Some(n) => iv.len() == n,
        None => !iv.is_empty(),
    };
    if !iv_ok {
        return Err(coded_crypto_error(
            rt,
            "TypeError",
            "ERR_CRYPTO_INVALID_IV",
            "Invalid initialization vector",
        ));
    }
    Ok(())
}

fn unknown_cipher_error(rt: &mut Runtime) -> RuntimeError {
    match rusty_js_runtime::intrinsics::make_error_instance(rt, "Error", "Unknown cipher") {
        Some(id) => {
            rt.object_set(
                id,
                "code".into(),
                Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                    "ERR_CRYPTO_UNKNOWN_CIPHER",
                ))),
            );
            RuntimeError::Thrown(Value::Object(id))
        }
        None => RuntimeError::TypeError("Unknown cipher".to_string()),
    }
}

fn invalid_digest_error_msg(rt: &mut Runtime, msg: &str) -> RuntimeError {
    match rusty_js_runtime::intrinsics::make_error_instance(rt, "TypeError", msg) {
        Some(id) => {
            rt.object_set(
                id,
                "code".into(),
                Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                    "ERR_CRYPTO_INVALID_DIGEST",
                ))),
            );
            RuntimeError::Thrown(Value::Object(id))
        }
        None => RuntimeError::TypeError(msg.to_string()),
    }
}

fn invalid_digest_error(rt: &mut Runtime, name: &str) -> RuntimeError {
    let msg = format!("Invalid digest: {name}");
    match rusty_js_runtime::intrinsics::make_error_instance(rt, "TypeError", &msg) {
        Some(id) => {
            rt.object_set(
                id,
                "code".into(),
                Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                    "ERR_CRYPTO_INVALID_DIGEST",
                ))),
            );
            RuntimeError::Thrown(Value::Object(id))
        }
        None => RuntimeError::TypeError(msg),
    }
}

fn fill_random(bytes: &mut [u8]) -> Result<(), RuntimeError> {
    rusty_web_crypto::get_random_values(bytes)
        .map_err(|e| crypto_error(format!("node:crypto random: {e}")))
}

fn digest_fn_and_len(
    digest: &str,
) -> Result<(fn(&[u8]) -> Vec<u8>, usize, &'static str), RuntimeError> {
    match normalize_digest(digest).as_str() {
        "sha1" => Ok((|d| rusty_web_crypto::digest_sha1(d).to_vec(), 20, "SHA-1")),
        "sha256" => Ok((
            |d| rusty_web_crypto::digest_sha256(d).to_vec(),
            32,
            "SHA-256",
        )),
        "sha384" => Ok((
            |d| rusty_web_crypto::digest_sha384(d).to_vec(),
            48,
            "SHA-384",
        )),
        "sha512" => Ok((
            |d| rusty_web_crypto::digest_sha512(d).to_vec(),
            64,
            "SHA-512",
        )),
        _ => Err(crypto_error("node:crypto: unsupported digest")),
    }
}

fn pbkdf2(
    password: &[u8],
    salt: &[u8],
    iterations: usize,
    key_len: usize,
    digest: &str,
) -> Result<Vec<u8>, RuntimeError> {
    let iterations = u32::try_from(iterations)
        .map_err(|_| crypto_error("node:crypto pbkdf2Sync: iterations exceed u32"))?;
    match normalize_digest(digest).as_str() {
        "sha1" => Ok(rusty_web_crypto::pbkdf2_hmac_sha1(
            password, salt, iterations, key_len,
        )),
        "sha256" => Ok(rusty_web_crypto::pbkdf2_hmac_sha256(
            password, salt, iterations, key_len,
        )),
        "sha384" => Ok(rusty_web_crypto::pbkdf2_hmac_sha384(
            password, salt, iterations, key_len,
        )),
        "sha512" => Ok(rusty_web_crypto::pbkdf2_hmac_sha512(
            password, salt, iterations, key_len,
        )),
        _ => Err(crypto_error("node:crypto pbkdf2Sync: unsupported digest")),
    }
}

fn cipher_encode_output(rt: &mut Runtime, bytes: &[u8], enc_arg: Option<&Value>) -> Value {
    let enc = match enc_arg {
        Some(Value::String(s)) => s.as_str().to_ascii_lowercase(),
        _ => String::new(),
    };
    let s = |txt: String| Value::String(Rc::new(rusty_js_runtime::value::JsString::from(txt)));
    match enc.as_str() {
        "hex" => s(hex_encode(bytes)),
        "base64" => s(base64_encode(bytes)),
        "base64url" => s(base64url_encode(bytes)),
        "latin1" | "binary" => s(bytes.iter().map(|&b| b as char).collect::<String>()),
        "utf8" | "utf-8" | "ucs2" | "utf16le" => s(String::from_utf8_lossy(bytes).into_owned()),
        _ => make_buffer(rt, bytes),
    }
}

fn hex_str_to_bytes(s: &str) -> Vec<u8> {
    let h = s.as_bytes();
    let mut out = Vec::with_capacity(h.len() / 2);
    let mut i = 0;
    while i + 1 < h.len() {
        if let (Some(hi), Some(lo)) = ((h[i] as char).to_digit(16), (h[i + 1] as char).to_digit(16))
        {
            out.push(((hi << 4) | lo) as u8);
        }
        i += 2;
    }
    out
}

fn base64_str_to_bytes(s: &str) -> Vec<u8> {
    let val = |c: u8| -> Option<u8> {
        match c {
            b'A'..=b'Z' => Some(c - b'A'),
            b'a'..=b'z' => Some(c - b'a' + 26),
            b'0'..=b'9' => Some(c - b'0' + 52),
            b'+' | b'-' => Some(62),
            b'/' | b'_' => Some(63),
            _ => None,
        }
    };
    let (mut bits, mut nbits, mut out) = (0u32, 0u32, Vec::new());
    for &c in s.as_bytes() {
        if let Some(v) = val(c) {
            bits = (bits << 6) | v as u32;
            nbits += 6;
            if nbits >= 8 {
                nbits -= 8;
                out.push((bits >> nbits) as u8);
            }
        }
    }
    out
}

fn apply_gcm_auth_tag_length(
    rt: &mut Runtime,
    cipher: &Value,
    algo: &str,
    opts: Option<&Value>,
) -> Result<(), RuntimeError> {
    let Value::Object(cid) = cipher else {
        return Ok(());
    };
    if !algo.contains("gcm") {
        return Ok(());
    }
    let len = match opts {
        Some(Value::Object(o)) => match rt.object_get(*o, "authTagLength") {
            Value::Number(n) if n.is_finite() && n >= 0.0 => n as usize,
            _ => return Ok(()),
        },
        _ => return Ok(()),
    };
    if !matches!(len, 4 | 8 | 12 | 13 | 14 | 15 | 16) {
        return Err(coded_crypto_error(
            rt,
            "TypeError",
            "ERR_CRYPTO_INVALID_AUTH_TAG",
            &format!("Invalid authentication tag length: {len}"),
        ));
    }
    rt.object_set(*cid, "__authtaglen__".into(), Value::Number(len as f64));
    Ok(())
}

fn cipher_decode_input(rt: &mut Runtime, data: Option<&Value>, enc: Option<&Value>) -> Vec<u8> {
    if let (Some(Value::String(sv)), Some(Value::String(ienc))) = (data, enc) {
        match ienc.as_str().to_ascii_lowercase().as_str() {
            "hex" => return hex_str_to_bytes(sv.as_str()),
            "base64" | "base64url" => return base64_str_to_bytes(sv.as_str()),
            "latin1" | "binary" => return sv.as_str().chars().map(|c| c as u8).collect(),
            "ascii" => {
                return sv
                    .as_str()
                    .chars()
                    .map(|c| (c as u32 & 0x7f) as u8)
                    .collect()
            }
            "utf16le" | "ucs2" | "utf-16le" | "ucs-2" => {
                return sv
                    .as_str()
                    .encode_utf16()
                    .flat_map(|u| [(u & 0xff) as u8, (u >> 8) as u8])
                    .collect()
            }
            "utf8" | "utf-8" | "" => return sv.as_bytes().to_vec(),
            _ => {}
        }
    }
    extract_bytes(rt, data.unwrap_or(&Value::Undefined))
}

fn build_cipher(rt: &mut Runtime, algo: &str, key: &[u8], iv: &[u8], decrypt: bool) -> Value {
    let id = new_object(rt);
    rt.object_set(
        id,
        "__cruft_crypto_kind".into(),
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
            if decrypt { "decipher" } else { "cipher" },
        ))),
    );
    rt.object_set(
        id,
        "__algo__".into(),
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
            algo.to_string(),
        ))),
    );
    rt.object_set(id, "__decrypt__".into(), Value::Boolean(decrypt));
    set_bytes_prop(rt, id, "__key__", key);
    set_bytes_prop(rt, id, "__iv__", iv);

    {
        let acc = new_object(rt);
        rt.object_set(acc, "length".into(), Value::Number(0.0));
        rt.object_set(id, "__buf__".into(), Value::Object(acc));
    }
    set_bytes_prop(rt, id, "__aad__", &[]);
    set_bytes_prop(rt, id, "__tag__", &[]);
    register_method(rt, id, "update", |rt, args| {
        let this = match rt.current_this() {
            Value::Object(id) => id,
            _ => return Ok(Value::Undefined),
        };

        let data = cipher_decode_input(rt, args.first(), args.get(1));
        let buf = match rt.object_get(this, "__buf__") {
            Value::Object(id) => id,
            _ => return Err(crypto_error("node:crypto cipher: missing state")),
        };
        push_bytes(rt, buf, &data);
        Ok(cipher_encode_output(rt, &[], args.get(2)))
    });
    register_method(rt, id, "setAAD", |rt, args| {
        let this = match rt.current_this() {
            Value::Object(id) => id,
            _ => return Ok(Value::Undefined),
        };
        let aad = extract_bytes(rt, args.first().unwrap_or(&Value::Undefined));
        set_bytes_prop(rt, this, "__aad__", &aad);
        Ok(Value::Object(this))
    });

    register_method(rt, id, "setAutoPadding", |rt, args| {
        let this = match rt.current_this() {
            Value::Object(id) => id,
            _ => return Ok(Value::Undefined),
        };
        let auto = !matches!(args.first(), Some(Value::Boolean(false)));
        rt.object_set(this, "__autopad__".into(), Value::Boolean(auto));
        Ok(Value::Object(this))
    });
    register_method(rt, id, "setAuthTag", |rt, args| {
        let this = match rt.current_this() {
            Value::Object(id) => id,
            _ => return Ok(Value::Undefined),
        };
        let tag = extract_bytes(rt, args.first().unwrap_or(&Value::Undefined));

        let expected = match rt.object_get(this, "__authtaglen__") {
            Value::Number(n) if n.is_finite() && n >= 0.0 => n as usize,
            _ => 16,
        };
        if tag.len() != expected {
            return Err(coded_crypto_error(
                rt,
                "TypeError",
                "ERR_CRYPTO_INVALID_AUTH_TAG",
                &format!("Invalid authentication tag length: {}", tag.len()),
            ));
        }
        set_bytes_prop(rt, this, "__tag__", &tag);
        Ok(Value::Object(this))
    });
    register_method(rt, id, "getAuthTag", |rt, _args| {
        let this = match rt.current_this() {
            Value::Object(id) => id,
            _ => return Ok(Value::Undefined),
        };

        if !matches!(
            rt.object_get(this, "__gcm_tag_ready__"),
            Value::Boolean(true)
        ) {
            return Err(coded_crypto_error(
                rt,
                "Error",
                "ERR_CRYPTO_INVALID_STATE",
                "Invalid state for operation getAuthTag",
            ));
        }
        let mut tag = get_bytes_prop(rt, this, "__tag__");

        if let Value::Number(n) = rt.object_get(this, "__authtaglen__") {
            let len = n as usize;
            if len < tag.len() {
                tag.truncate(len);
            }
        }
        Ok(make_buffer(rt, &tag))
    });
    register_method(rt, id, "final", |rt, args| {
        let this = match rt.current_this() {
            Value::Object(id) => id,
            _ => return Ok(Value::Undefined),
        };
        let out_enc = args.first().cloned();
        let algo = match rt.object_get(this, "__algo__") {
            Value::String(s) => s.as_str().to_ascii_lowercase(),
            _ => String::new(),
        };
        let decrypt = matches!(rt.object_get(this, "__decrypt__"), Value::Boolean(true));
        let key = get_bytes_prop(rt, this, "__key__");
        let iv = get_bytes_prop(rt, this, "__iv__");
        let aad = get_bytes_prop(rt, this, "__aad__");
        let mut data = get_bytes_prop(rt, this, "__buf__");
        let out = if algo.contains("gcm") {
            if decrypt {

                let tag = get_bytes_prop(rt, this, "__tag__");
                match rusty_web_crypto::aes_gcm_decrypt_split(&key, &iv, &aad, &data, &tag) {
                    Ok(pt) => Ok(pt),
                    Err(_) => {

                        let msg = "Unsupported state or unable to authenticate data";
                        return Err(
                            match rusty_js_runtime::intrinsics::make_error_instance(
                                rt, "Error", msg,
                            ) {
                                Some(eid) => RuntimeError::Thrown(Value::Object(eid)),
                                None => RuntimeError::TypeError(msg.to_string()),
                            },
                        );
                    }
                }
            } else {
                let mut combined = rusty_web_crypto::aes_gcm_encrypt(&key, &iv, &aad, &data)
                    .map_err(|e| crypto_error(format!("node:crypto cipher: {e}")))?;
                let tag = combined.split_off(combined.len().saturating_sub(16));
                set_bytes_prop(rt, this, "__tag__", &tag);
                rt.object_set(this, "__gcm_tag_ready__".into(), Value::Boolean(true));
                Ok(combined)
            }
        } else if algo == "chacha20-poly1305" {

            let k: [u8; 32] = match key.as_slice().try_into() {
                Ok(a) => a,
                Err(_) => return Err(crypto_error("chacha20-poly1305: key must be 32 bytes")),
            };
            let n: [u8; 12] = match iv.as_slice().try_into() {
                Ok(a) => a,
                Err(_) => return Err(crypto_error("chacha20-poly1305: nonce must be 12 bytes")),
            };
            if decrypt {
                let tag = get_bytes_prop(rt, this, "__tag__");
                match chacha20_poly1305_open(&k, &n, &aad, &data, &tag) {
                    Some(pt) => Ok(pt),
                    None => {

                        let msg = "Unsupported state or unable to authenticate data";
                        return Err(
                            match rusty_js_runtime::intrinsics::make_error_instance(
                                rt, "Error", msg,
                            ) {
                                Some(eid) => RuntimeError::Thrown(Value::Object(eid)),
                                None => RuntimeError::TypeError(msg.to_string()),
                            },
                        );
                    }
                }
            } else {
                let (ct, tag) = chacha20_poly1305_seal(&k, &n, &aad, &data);
                set_bytes_prop(rt, this, "__tag__", &tag);
                rt.object_set(this, "__gcm_tag_ready__".into(), Value::Boolean(true));
                Ok(ct)
            }
        } else if algo == "chacha20" {

            let k: [u8; 32] = match key.as_slice().try_into() {
                Ok(a) => a,
                Err(_) => return Err(crypto_error("chacha20: key must be 32 bytes")),
            };
            if iv.len() != 16 {
                return Err(crypto_error("chacha20: iv must be 16 bytes"));
            }
            let counter = u32::from_le_bytes([iv[0], iv[1], iv[2], iv[3]]);
            let mut nonce = [0u8; 12];
            nonce.copy_from_slice(&iv[4..16]);
            Ok(chacha20_xor(&k, &nonce, counter, &data))
        } else if algo.contains("ecb") {

            let auto_pad = !matches!(rt.object_get(this, "__autopad__"), Value::Boolean(false));
            if decrypt {
                if data.len() % 16 != 0 {
                    return Err(coded_crypto_error(
                        rt,
                        "Error",
                        "ERR_OSSL_WRONG_FINAL_BLOCK_LENGTH",
                        "error:1C80006B:Provider routines::wrong final block length",
                    ));
                }
                let mut out = Vec::with_capacity(data.len());
                for blk in data.chunks_exact(16) {
                    let b: [u8; 16] = blk.try_into().unwrap();
                    out.extend_from_slice(&rusty_web_crypto::aes_decrypt_block_with_key(&key, &b));
                }
                if auto_pad {

                    let bad = out.is_empty() || {
                        let p = out[out.len() - 1] as usize;
                        p == 0
                            || p > 16
                            || p > out.len()
                            || out[out.len() - p..].iter().any(|&b| b as usize != p)
                    };
                    if bad {
                        return Err(coded_crypto_error(
                            rt,
                            "Error",
                            "ERR_OSSL_BAD_DECRYPT",
                            "error:1C800064:Provider routines::bad decrypt",
                        ));
                    }
                    let p = out[out.len() - 1] as usize;
                    out.truncate(out.len() - p);
                }
                Ok(out)
            } else {
                let mut buf = data.clone();
                if auto_pad {
                    let pad = 16 - (buf.len() % 16);
                    buf.extend(std::iter::repeat(pad as u8).take(pad));
                } else if buf.len() % 16 != 0 {
                    return Err(coded_crypto_error(
                        rt,
                        "Error",
                        "ERR_OSSL_WRONG_FINAL_BLOCK_LENGTH",
                        "error:1C80006B:Provider routines::wrong final block length",
                    ));
                }
                let mut out = Vec::with_capacity(buf.len());
                for blk in buf.chunks_exact(16) {
                    let b: [u8; 16] = blk.try_into().unwrap();
                    out.extend_from_slice(&rusty_web_crypto::aes_encrypt_block_with_key(&key, &b));
                }
                Ok(out)
            }
        } else if algo.contains("cfb") {

            let mut fb = [0u8; 16];
            if iv.len() != 16 {
                return Err(crypto_error("aes-cfb: iv must be 16 bytes"));
            }
            fb.copy_from_slice(&iv);
            let mut out = Vec::with_capacity(data.len());
            for chunk in data.chunks(16) {
                let ks = rusty_web_crypto::aes_encrypt_block_with_key(&key, &fb);
                let mut cblk = [0u8; 16];
                for (i, &b) in chunk.iter().enumerate() {
                    let x = b ^ ks[i];
                    out.push(x);

                    cblk[i] = if decrypt { b } else { x };
                }
                if chunk.len() == 16 {
                    fb = cblk;
                }
            }
            Ok(out)
        } else if algo.contains("ofb") {

            if iv.len() != 16 {
                return Err(crypto_error("aes-ofb: iv must be 16 bytes"));
            }
            let mut fb = [0u8; 16];
            fb.copy_from_slice(&iv);
            let mut out = Vec::with_capacity(data.len());
            for chunk in data.chunks(16) {
                fb = rusty_web_crypto::aes_encrypt_block_with_key(&key, &fb);
                for (i, &b) in chunk.iter().enumerate() {
                    out.push(b ^ fb[i]);
                }
            }
            Ok(out)
        } else if algo.contains("ctr") {
            rusty_web_crypto::aes_ctr_xor_with_key(&key, &iv, 64, &data)
        } else if algo.contains("cbc") {
            if decrypt {

                let auto_pad = !matches!(rt.object_get(this, "__autopad__"), Value::Boolean(false));
                if !auto_pad {
                    rusty_web_crypto::aes_cbc_decrypt_no_pad(&key, &iv, &data)
                } else {

                    match rusty_web_crypto::aes_cbc_decrypt(&key, &iv, &data) {
                        Ok(v) => Ok(v),
                        Err(_) => {
                            return Err(coded_crypto_error(
                                rt,
                                "Error",
                                "ERR_OSSL_BAD_DECRYPT",
                                "error:1C800064:Provider routines::bad decrypt",
                            ))
                        }
                    }
                }
            } else {

                let auto_pad = !matches!(rt.object_get(this, "__autopad__"), Value::Boolean(false));
                if !auto_pad {
                    if data.len() % 16 != 0 {
                        return Err(coded_crypto_error(
                            rt,
                            "Error",
                            "ERR_OSSL_WRONG_FINAL_BLOCK_LENGTH",
                            "error:1C80006B:Provider routines::wrong final block length",
                        ));
                    }
                    rusty_web_crypto::aes_cbc_encrypt_no_pad(&key, &iv, &data)
                } else {
                    rusty_web_crypto::aes_cbc_encrypt(&key, &iv, &data)
                }
            }
        } else {
            Err("unsupported cipher".to_string())
        }
        .map_err(|e| crypto_error(format!("node:crypto cipher: {e}")))?;
        Ok(cipher_encode_output(rt, &out, out_enc.as_ref()))
    });
    Value::Object(id)
}

fn key_bytes(rt: &mut Runtime, key: &Value, prop: &str) -> Vec<u8> {
    match key {
        Value::Object(id) => get_bytes_prop(rt, *id, prop),
        _ => Vec::new(),
    }
}

fn rsa_sign(
    rt: &mut Runtime,
    digest: &str,
    data: &[u8],
    key: &Value,
    pss: bool,
) -> Result<Vec<u8>, RuntimeError> {
    let n = key_bytes(rt, key, "n");
    let d = key_bytes(rt, key, "d");
    if n.is_empty() || d.is_empty() {
        return Err(crypto_error(
            "node:crypto sign: RSA key requires n and d byte properties",
        ));
    }
    let (hash_fn, hlen, hash_name) = digest_fn_and_len(digest)?;
    if pss {
        let mut salt = vec![0u8; hlen];
        fill_random(&mut salt)?;
        rusty_web_crypto::rsa_pss_sign(&n, &d, data, &salt, hash_fn, hlen)
            .map_err(|e| crypto_error(format!("node:crypto sign: {e}")))
    } else {
        let hash = hash_fn(data);
        rusty_web_crypto::rsa_pkcs1_v15_sign(&n, &d, &hash, hash_name)
            .map_err(|e| crypto_error(format!("node:crypto sign: {e}")))
    }
}

fn rsa_verify(
    rt: &mut Runtime,
    digest: &str,
    data: &[u8],
    key: &Value,
    sig: &[u8],
    pss: bool,
) -> Result<bool, RuntimeError> {
    let n = key_bytes(rt, key, "n");
    let e = key_bytes(rt, key, "e");
    if n.is_empty() || e.is_empty() {
        return Err(crypto_error(
            "node:crypto verify: RSA key requires n and e byte properties",
        ));
    }
    let (hash_fn, hlen, hash_name) = digest_fn_and_len(digest)?;
    let ok = if pss {
        rusty_web_crypto::rsa_pss_verify(&n, &e, data, sig, hlen, hash_fn, hlen)
    } else {
        let hash = hash_fn(data);
        rusty_web_crypto::rsa_pkcs1_v15_verify(&n, &e, &hash, sig, hash_name)
    };
    Ok(ok.is_ok())
}

fn install_hash_object(rt: &mut Runtime, alg: &str) -> rusty_js_runtime::ObjectRef {
    let hash = new_object(rt);
    rt.object_set(
        hash,
        "__cruft_crypto_kind".into(),
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from("hash"))),
    );
    set_constant(
        rt,
        hash,
        "algorithm",
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
            alg.to_string(),
        ))),
    );

    let buf = rt.alloc_object(rusty_js_runtime::value::Object::new_array());
    rt.object_set(buf, "length".into(), Value::Number(0.0));
    rt.object_set(hash, "__buf__".into(), Value::Object(buf));
    register_method(rt, hash, "update", |rt, args| {
        let this_id = match rt.current_this() {
            Value::Object(id) => id,
            _ => return Ok(Value::Undefined),
        };
        if hash_is_finalized(rt, this_id) {
            return Err(hash_finalized_error(rt));
        }
        let buf_id = match rt.object_get(this_id, "__buf__") {
            Value::Object(id) => id,
            _ => return Ok(rt.current_this()),
        };

        let bytes = cipher_decode_input(rt, args.first(), args.get(1));
        let cur_len = match rt.object_get(buf_id, "length") {
            Value::Number(n) => n as usize,
            _ => 0,
        };
        for (i, b) in bytes.iter().enumerate() {
            rt.object_set(buf_id, (cur_len + i).to_string(), Value::Number(*b as f64));
        }
        rt.object_set(
            buf_id,
            "length".into(),
            Value::Number((cur_len + bytes.len()) as f64),
        );
        Ok(rt.current_this())
    });
    register_method(rt, hash, "digest", |rt, args| {
        let this_id = match rt.current_this() {
            Value::Object(id) => id,
            _ => return Ok(Value::Undefined),
        };
        if hash_is_finalized(rt, this_id) {
            return Err(hash_finalized_error(rt));
        }
        rt.object_set(this_id, "__finalized__".into(), Value::Boolean(true));
        let alg = match rt.object_get(this_id, "algorithm") {
            Value::String(s) => (*s).clone(),
            _ => "sha256".into(),
        };
        let buf_id = match rt.object_get(this_id, "__buf__") {
            Value::Object(id) => id,
            _ => return Ok(Value::Undefined),
        };
        let len = match rt.object_get(buf_id, "length") {
            Value::Number(n) => n as usize,
            _ => 0,
        };
        let mut bytes = Vec::with_capacity(len);
        for i in 0..len {
            if let Value::Number(n) = rt.object_get(buf_id, &i.to_string()) {
                bytes.push(n as u8);
            }
        }
        let out_len = match rt.object_get(this_id, "__hash_outlen") {
            Value::Number(n) if n.is_finite() && n >= 0.0 => Some(n as usize),
            _ => None,
        };
        let d = digest_bytes_len(&alg, &bytes, out_len);
        let enc = match args.first() {
            Some(Value::String(s)) => s.as_str().to_string(),
            _ => "".to_string(),
        };
        match enc.as_str() {
            "hex" => Ok(Value::String(Rc::new(
                rusty_js_runtime::value::JsString::from(hex_encode(&d)),
            ))),
            "base64" => Ok(Value::String(Rc::new(
                rusty_js_runtime::value::JsString::from(base64_encode(&d)),
            ))),
            "base64url" => Ok(Value::String(Rc::new(
                rusty_js_runtime::value::JsString::from(base64url_encode(&d)),
            ))),

            "latin1" | "binary" => Ok(Value::String(Rc::new(
                rusty_js_runtime::value::JsString::from(
                    d.iter().map(|&b| b as char).collect::<String>(),
                ),
            ))),
            "" => {

                Ok(make_buffer(rt, &d))
            }
            _ => Ok(Value::String(Rc::new(
                rusty_js_runtime::value::JsString::from(hex_encode(&d)),
            ))),
        }
    });

    register_method(rt, hash, "copy", |rt, _args| {
        let this_id = match rt.current_this() {
            Value::Object(id) => id,
            _ => return Ok(Value::Undefined),
        };
        if hash_is_finalized(rt, this_id) {
            return Err(hash_finalized_error(rt));
        }
        let alg = match rt.object_get(this_id, "algorithm") {
            Value::String(s) => (*s).as_str().to_string(),
            _ => "sha256".to_string(),
        };
        let new_hash = install_hash_object(rt, &alg);
        if let (Value::Object(src_buf), Value::Object(dst_buf)) = (
            rt.object_get(this_id, "__buf__"),
            rt.object_get(new_hash, "__buf__"),
        ) {
            let len = match rt.object_get(src_buf, "length") {
                Value::Number(n) => n as usize,
                _ => 0,
            };
            for i in 0..len {
                let b = rt.object_get(src_buf, &i.to_string());
                rt.object_set(dst_buf, i.to_string(), b);
            }
            rt.object_set(dst_buf, "length".into(), Value::Number(len as f64));
        }

        if let Value::Number(n) = rt.object_get(this_id, "__hash_outlen") {
            rt.object_set(new_hash, "__hash_outlen".into(), Value::Number(n));
        }
        Ok(Value::Object(new_hash))
    });
    hash
}

pub fn install(rt: &mut Runtime) {
    let crypto = new_object(rt);

    register_method(rt, crypto, "createHash", |rt, args| {
        let alg = match args.first() {
            Some(Value::String(s)) => s.as_str().to_string(),
            _ => "sha256".to_string(),
        };

        if !digest_supported(&alg) {
            return Err(RuntimeError::Thrown(
                match rusty_js_runtime::intrinsics::make_error_instance(
                    rt,
                    "Error",
                    "Digest method not supported",
                ) {
                    Some(id) => Value::Object(id),
                    None => {
                        return Err(RuntimeError::TypeError(
                            "Digest method not supported".into(),
                        ))
                    }
                },
            ));
        }
        let hash = install_hash_object(rt, &alg);

        if let Some(Value::Object(opts)) = args.get(1) {
            if let Value::Number(n) = rt.object_get(*opts, "outputLength") {
                if n.is_finite() && n >= 0.0 {
                    rt.object_set(hash, "__hash_outlen".into(), Value::Number(n));
                }
            }
        }
        Ok(Value::Object(hash))
    });

    register_method(rt, crypto, "createHmac", |rt, args| {
        let alg = match args.first() {
            Some(Value::String(s)) => s.as_str().to_string(),
            _ => "sha256".to_string(),
        };

        if !digest_supported(&alg) {
            let msg = format!("Invalid digest: {alg}");
            return Err(RuntimeError::Thrown(
                match rusty_js_runtime::intrinsics::make_error_instance(rt, "TypeError", &msg) {
                    Some(id) => {
                        rt.object_set(
                            id,
                            "code".into(),
                            Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                                "ERR_CRYPTO_INVALID_DIGEST",
                            ))),
                        );
                        Value::Object(id)
                    }
                    None => return Err(RuntimeError::TypeError(msg)),
                },
            ));
        }
        let key_v = args.get(1).cloned().unwrap_or(Value::Undefined);
        let key_bytes = extract_bytes(rt, &key_v);
        let hmac = new_object(rt);
        set_constant(
            rt,
            hmac,
            "algorithm",
            Value::String(Rc::new(rusty_js_runtime::value::JsString::from(alg))),
        );

        let key_obj = rt.alloc_object(rusty_js_runtime::value::Object::new_array());
        for (i, b) in key_bytes.iter().enumerate() {
            rt.object_set(key_obj, i.to_string(), Value::Number(*b as f64));
        }
        rt.object_set(
            key_obj,
            "length".into(),
            Value::Number(key_bytes.len() as f64),
        );
        rt.object_set(
            hmac,
            "__cruft_crypto_kind".into(),
            Value::String(Rc::new(rusty_js_runtime::value::JsString::from("hmac"))),
        );
        rt.object_set(hmac, "__key__".into(), Value::Object(key_obj));
        let buf = rt.alloc_object(rusty_js_runtime::value::Object::new_array());
        rt.object_set(buf, "length".into(), Value::Number(0.0));
        rt.object_set(hmac, "__buf__".into(), Value::Object(buf));
        register_method(rt, hmac, "update", |rt, args| {
            let this_id = match rt.current_this() {
                Value::Object(id) => id,
                _ => return Ok(Value::Undefined),
            };
            if hash_is_finalized(rt, this_id) {
                return Err(hash_finalized_error(rt));
            }
            let buf_id = match rt.object_get(this_id, "__buf__") {
                Value::Object(id) => id,
                _ => return Ok(rt.current_this()),
            };

            let bytes = cipher_decode_input(rt, args.first(), args.get(1));
            let cur_len = match rt.object_get(buf_id, "length") {
                Value::Number(n) => n as usize,
                _ => 0,
            };
            for (i, b) in bytes.iter().enumerate() {
                rt.object_set(buf_id, (cur_len + i).to_string(), Value::Number(*b as f64));
            }
            rt.object_set(
                buf_id,
                "length".into(),
                Value::Number((cur_len + bytes.len()) as f64),
            );
            Ok(rt.current_this())
        });
        register_method(rt, hmac, "digest", |rt, args| {
            let this_id = match rt.current_this() {
                Value::Object(id) => id,
                _ => return Ok(Value::Undefined),
            };

            rt.object_set(this_id, "__finalized__".into(), Value::Boolean(true));
            let alg = match rt.object_get(this_id, "algorithm") {
                Value::String(s) => (*s).clone(),
                _ => "sha256".into(),
            };
            let key_id = match rt.object_get(this_id, "__key__") {
                Value::Object(id) => id,
                _ => return Ok(Value::Undefined),
            };
            let buf_id = match rt.object_get(this_id, "__buf__") {
                Value::Object(id) => id,
                _ => return Ok(Value::Undefined),
            };

            let k_len = match rt.object_get(key_id, "length") {
                Value::Number(n) => n as usize,
                _ => 0,
            };
            let mut key: Vec<u8> = (0..k_len)
                .map(|i| match rt.object_get(key_id, &i.to_string()) {
                    Value::Number(n) => n as u8,
                    _ => 0,
                })
                .collect();
            let m_len = match rt.object_get(buf_id, "length") {
                Value::Number(n) => n as usize,
                _ => 0,
            };
            let msg: Vec<u8> = (0..m_len)
                .map(|i| match rt.object_get(buf_id, &i.to_string()) {
                    Value::Number(n) => n as u8,
                    _ => 0,
                })
                .collect();

            let block_size: usize = match normalize_digest(&alg).as_str() {
                "sha384" | "sha512" | "sha512224" | "sha512256" => 128,
                "sha3224" => 144,
                "sha3256" => 136,
                "sha3384" => 104,
                "sha3512" => 72,
                _ => 64,
            };
            if key.len() > block_size {
                key = digest_bytes(&alg, &key);
            }
            while key.len() < block_size {
                key.push(0);
            }
            let ipad: Vec<u8> = key.iter().map(|b| b ^ 0x36).collect();
            let opad: Vec<u8> = key.iter().map(|b| b ^ 0x5c).collect();
            let mut inner = ipad;
            inner.extend_from_slice(&msg);
            let inner_hash = digest_bytes(&alg, &inner);
            let mut outer = opad;
            outer.extend_from_slice(&inner_hash);
            let d = digest_bytes(&alg, &outer);
            let enc = match args.first() {
                Some(Value::String(s)) => s.as_str().to_string(),
                _ => "".to_string(),
            };
            match enc.as_str() {
                "hex" => Ok(Value::String(Rc::new(
                    rusty_js_runtime::value::JsString::from(hex_encode(&d)),
                ))),
                "base64" => Ok(Value::String(Rc::new(
                    rusty_js_runtime::value::JsString::from(base64_encode(&d)),
                ))),
                "base64url" => Ok(Value::String(Rc::new(
                    rusty_js_runtime::value::JsString::from(base64url_encode(&d)),
                ))),
                "latin1" | "binary" => Ok(Value::String(Rc::new(
                    rusty_js_runtime::value::JsString::from(
                        d.iter().map(|&b| b as char).collect::<String>(),
                    ),
                ))),
                _ => {

                    Ok(make_buffer(rt, &d))
                }
            }
        });
        Ok(Value::Object(hmac))
    });

    fn crypto_random_bytes(
        rt: &mut Runtime,
        args: &[Value],
        task_label: &'static str,
    ) -> Result<Value, RuntimeError> {
        let n = match args.first() {
            Some(Value::Number(n)) => *n as usize,
            _ => 0,
        };
        let mut bytes_vec = vec![0u8; n];
        fill_random(&mut bytes_vec)?;
        let out = make_buffer(rt, &bytes_vec);
        if let Some(cb) = args.get(1).cloned().filter(|v| rt.is_callable(v)) {
            let cb_args = vec![Value::Null, out];
            let roots = crate::timer::roots_for_callback(&cb, &cb_args);
            rt.enqueue_host_phase_rooted(
                HostEnqueuePhase::HostCompletionMacrotask,
                task_label,
                roots,
                move |rt| {
                    let _ = rt.call_function(cb, Value::Undefined, cb_args);
                    Ok(())
                },
            );
            Ok(Value::Undefined)
        } else {
            Ok(out)
        }
    }

    register_method(rt, crypto, "randomBytes", |rt, args| {
        crypto_random_bytes(rt, args, "crypto.randomBytes callback")
    });

    register_method(rt, crypto, "pseudoRandomBytes", |rt, args| {
        crypto_random_bytes(rt, args, "crypto.pseudoRandomBytes callback")
    });

    register_method(rt, crypto, "randomUUID", |_rt, _args| {

        let mut b = [0u8; 16];
        fill_random(&mut b)?;
        b[6] = (b[6] & 0x0f) | 0x40;
        b[8] = (b[8] & 0x3f) | 0x80;
        let s = format!(
            "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
            b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7], b[8], b[9], b[10], b[11], b[12], b[13], b[14], b[15]
        );
        Ok(Value::String(Rc::new(
            rusty_js_runtime::value::JsString::from(s),
        )))
    });

    let webcrypto = new_object(rt);
    register_method(rt, webcrypto, "getRandomValues", |rt, args| {
        let id = match args.first() {
            Some(Value::Object(id)) => *id,
            _ => {
                return Err(RuntimeError::TypeError(
                    "crypto.getRandomValues: argument must be a typed array".into(),
                ))
            }
        };
        let length = match rt.object_get(id, &"length".to_string()) {
            Value::Number(n) => n as usize,
            _ => 0,
        };
        let mut bytes = vec![0u8; length];
        fill_random(&mut bytes)?;
        for (i, b) in bytes.iter().enumerate() {
            rt.object_set(id, i.to_string(), Value::Number(*b as f64));
        }
        Ok(Value::Object(id))
    });
    register_method(rt, webcrypto, "randomUUID", |_rt, _args| {
        let mut b = [0u8; 16];
        fill_random(&mut b)?;
        b[6] = (b[6] & 0x0f) | 0x40;
        b[8] = (b[8] & 0x3f) | 0x80;
        let s = format!(
            "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
            b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7], b[8], b[9], b[10], b[11], b[12], b[13], b[14], b[15]
        );
        Ok(Value::String(Rc::new(
            rusty_js_runtime::value::JsString::from(s),
        )))
    });
    rt.object_set(crypto, "webcrypto".into(), Value::Object(webcrypto));

    register_method(rt, crypto, "getRandomValues", |rt, args| {
        let id = match args.first() {
            Some(Value::Object(id)) => *id,
            _ => {
                return Err(RuntimeError::TypeError(
                    "crypto.getRandomValues: argument must be a typed array".into(),
                ))
            }
        };
        let length = match rt.object_get(id, &"length".to_string()) {
            Value::Number(n) => n as usize,
            _ => 0,
        };
        let mut bytes = vec![0u8; length];
        fill_random(&mut bytes)?;
        for (i, b) in bytes.iter().enumerate() {
            rt.object_set(id, i.to_string(), Value::Number(*b as f64));
        }
        Ok(Value::Object(id))
    });

    register_method(rt, crypto, "randomFillSync", |rt, args| {
        let id = match args.first() {
            Some(Value::Object(id)) => *id,
            _ => {
                return Err(crypto_error(
                    "node:crypto randomFillSync: buffer object required",
                ))
            }
        };
        let len = object_len(rt, id);
        let offset = match args.get(1) {
            Some(Value::Number(n)) if *n >= 0.0 => *n as usize,
            _ => 0,
        };
        let size = match args.get(2) {
            Some(Value::Number(n)) if *n >= 0.0 => *n as usize,
            _ => len.saturating_sub(offset),
        };
        if offset > len || size > len.saturating_sub(offset) {
            return Err(crypto_error(
                "node:crypto randomFillSync: range out of bounds",
            ));
        }
        let mut bytes = vec![0u8; size];
        fill_random(&mut bytes)?;
        for (i, b) in bytes.iter().enumerate() {
            rt.object_set(id, (offset + i).to_string(), Value::Number(*b as f64));
        }
        Ok(Value::Object(id))
    });

    register_method(rt, crypto, "createCipheriv", |rt, args| {
        let algo = value_string(args.first()).to_ascii_lowercase();
        if !cipher_supported(&algo) {
            return Err(unknown_cipher_error(rt));
        }
        let key = extract_bytes(rt, args.get(1).unwrap_or(&Value::Undefined));
        let iv = extract_bytes(rt, args.get(2).unwrap_or(&Value::Undefined));
        validate_cipher_key_iv(rt, &algo, &key, &iv)?;
        let cipher = build_cipher(rt, &algo, &key, &iv, false);
        apply_gcm_auth_tag_length(rt, &cipher, &algo, args.get(3))?;
        Ok(cipher)
    });
    register_method(rt, crypto, "createDecipheriv", |rt, args| {
        let algo = value_string(args.first()).to_ascii_lowercase();
        if !cipher_supported(&algo) {
            return Err(unknown_cipher_error(rt));
        }
        let key = extract_bytes(rt, args.get(1).unwrap_or(&Value::Undefined));
        let iv = extract_bytes(rt, args.get(2).unwrap_or(&Value::Undefined));
        validate_cipher_key_iv(rt, &algo, &key, &iv)?;
        let cipher = build_cipher(rt, &algo, &key, &iv, true);
        apply_gcm_auth_tag_length(rt, &cipher, &algo, args.get(3))?;
        Ok(cipher)
    });
    register_method(rt, crypto, "pbkdf2Sync", |rt, args| {
        let password = extract_bytes(rt, args.first().unwrap_or(&Value::Undefined));
        let salt = extract_bytes(rt, args.get(1).unwrap_or(&Value::Undefined));
        let iterations = match args.get(2) {
            Some(Value::Number(n)) if *n > 0.0 => *n as usize,
            _ => {
                return Err(crypto_error(
                    "node:crypto pbkdf2Sync: iterations must be positive",
                ))
            }
        };
        let key_len = match args.get(3) {
            Some(Value::Number(n)) if *n >= 0.0 => *n as usize,
            _ => {
                return Err(crypto_error(
                    "node:crypto pbkdf2Sync: keylen must be non-negative",
                ))
            }
        };
        let digest = value_string(args.get(4));

        if !matches!(
            normalize_digest(&digest).as_str(),
            "sha1" | "sha256" | "sha384" | "sha512"
        ) {
            return Err(invalid_digest_error(rt, &digest));
        }
        Ok(make_buffer(
            rt,
            &pbkdf2(&password, &salt, iterations, key_len, &digest)?,
        ))
    });
    register_method(rt, crypto, "scryptSync", |rt, args| {

        let password = extract_bytes(rt, args.first().unwrap_or(&Value::Undefined));
        let salt = extract_bytes(rt, args.get(1).unwrap_or(&Value::Undefined));
        let key_len = match args.get(2) {
            Some(Value::Number(n)) if *n >= 0.0 => *n as usize,
            _ => {
                return Err(crypto_error(
                    "node:crypto scryptSync: keylen must be non-negative",
                ))
            }
        };
        let opt_u32 = |rt: &mut Runtime, id: ObjectRef, names: &[&str], default: u32| -> u32 {
            for name in names {
                if let Value::Number(n) = rt.object_get(id, name) {
                    if n.is_finite() && n > 0.0 {
                        return n as u32;
                    }
                }
            }
            default
        };
        let opt_u64 = |rt: &mut Runtime, id: ObjectRef, name: &str, default: u64| -> u64 {
            match rt.object_get(id, name) {
                Value::Number(n) if n.is_finite() && n > 0.0 => n as u64,
                _ => default,
            }
        };
        let (n, r, p, max_mem) = match args.get(3) {
            Some(Value::Object(id)) => {
                let id = *id;
                (
                    opt_u32(rt, id, &["N", "cost"], 16384),
                    opt_u32(rt, id, &["r", "blockSize"], 8),
                    opt_u32(rt, id, &["p", "parallelization"], 1),
                    opt_u64(rt, id, "maxmem", 32 << 20),
                )
            }
            _ => (16384, 8, 1, 32 << 20),
        };

        let dk = match rusty_web_crypto::scrypt(&password, &salt, n, r, p, key_len, max_mem) {
            Ok(dk) => dk,
            Err(_) => {
                return Err(coded_crypto_error(
                    rt,
                    "RangeError",
                    "ERR_CRYPTO_INVALID_SCRYPT_PARAMS",
                    "Invalid scrypt params",
                ))
            }
        };
        Ok(make_buffer(rt, &dk))
    });

    fn build_asym_key_object(
        rt: &mut Runtime,
        pem: &str,
        ktype: &str,
        details: Value,
    ) -> rusty_js_runtime::ObjectRef {
        let ko = new_object(rt);

        let proto_global = if ktype == "public" {
            "__cruft_PublicKeyObject_proto"
        } else {
            "__cruft_PrivateKeyObject_proto"
        };
        if let Value::Object(p) = rt.global_get(proto_global) {
            rt.obj_mut(ko).proto = Some(p);
        }

        rt.obj_mut(ko)
            .set_own_internal("__cruft_key_object".into(), Value::Boolean(true));

        rt.obj_mut(ko)
            .set_own_internal("type".into(), crypto_sval(ktype));
        rt.obj_mut(ko)
            .set_own_internal("key".into(), crypto_sval(pem));
        rt.obj_mut(ko)
            .set_own_internal("__pem".into(), crypto_sval(pem));
        rt.obj_mut(ko)
            .set_own_internal("asymmetricKeyType".into(), crypto_sval(key_type_of(pem)));
        rt.obj_mut(ko)
            .set_own_internal("asymmetricKeyDetails".into(), details);
        register_method_internal(rt, ko, "export", |rt, a| key_object_export(rt, a));
        ko
    }

    fn keypair_result_value(
        rt: &mut Runtime,
        opts: &Value,
        enc_field: &str,
        pem: &str,
        ktype: &str,
        details: Value,
    ) -> Value {
        let has_enc = matches!(opts, Value::Object(id)
            if matches!(rt.object_get(*id, enc_field), Value::Object(_)));
        if has_enc {
            crypto_sval(pem)
        } else {
            Value::Object(build_asym_key_object(rt, pem, ktype, details))
        }
    }

    fn ec_key_details(rt: &mut Runtime) -> Value {
        let o = new_object(rt);
        rt.object_set(o, "namedCurve".into(), crypto_sval("prime256v1"));
        Value::Object(o)
    }
    fn rsa_key_details(rt: &mut Runtime, modulus_len: usize, pub_exp: u64) -> Value {
        let o = new_object(rt);
        rt.object_set(o, "modulusLength".into(), Value::Number(modulus_len as f64));
        rt.object_set(
            o,
            "publicExponent".into(),
            Value::BigInt(Rc::new(rusty_js_runtime::bigint::JsBigInt::from_u64(
                pub_exp,
            ))),
        );
        Value::Object(o)
    }

    fn key_details_from_pem(rt: &mut Runtime, pem: &str) -> Value {
        match key_type_of(pem) {
            "rsa" => {

                let ne = parse_rsa_pub(pem)
                    .or_else(|| parse_rsa_priv(pem).map(|c| (c[0].clone(), c[1].clone())));
                match ne {
                    Some((n, e)) => {
                        let bits = if n.is_empty() {
                            0
                        } else {
                            n.len() * 8 - n[0].leading_zeros() as usize
                        };
                        let exp = e.iter().fold(0u64, |a, &b| (a << 8) | b as u64);
                        rsa_key_details(rt, bits, exp)
                    }
                    None => Value::Object(new_object(rt)),
                }
            }
            "ec" => ec_key_details(rt),

            _ => Value::Object(new_object(rt)),
        }
    }
    register_method(rt, crypto, "generateKeyPairSync", |rt, args| {
        let kind = value_string(args.first()).to_ascii_lowercase();
        let opts = args.get(1).cloned().unwrap_or(Value::Undefined);
        let modulus_len = match &opts {
            Value::Object(id) => match rt.object_get(*id, "modulusLength") {
                Value::Number(n) if n > 0.0 => n as usize,
                _ => 2048,
            },
            _ => 2048,
        };
        let pub_exp = match &opts {
            Value::Object(id) => match rt.object_get(*id, "publicExponent") {
                Value::Number(n) if n > 0.0 => n as u64,
                _ => 65537,
            },
            _ => 65537,
        };
        if kind == "ed25519" {
            let mut seed = vec![0u8; 32];
            let _ = rusty_web_crypto::get_random_values(&mut seed);
            let pubkey = rusty_web_crypto::ed25519_public_key(&seed);
            let (priv_pem, pub_pem) = ed25519_keypair_to_pem(&seed, &pubkey);
            let result = new_object(rt);
            let _result_root = rt.push_temporary_value_roots(&[Value::Object(result)]);
            let d1 = Value::Object(new_object(rt));
            let d2 = Value::Object(new_object(rt));
            let pubv = keypair_result_value(rt, &opts, "publicKeyEncoding", &pub_pem, "public", d1);
            let privv =
                keypair_result_value(rt, &opts, "privateKeyEncoding", &priv_pem, "private", d2);
            rt.object_set(result, "publicKey".into(), pubv);
            rt.object_set(result, "privateKey".into(), privv);
            return Ok(Value::Object(result));
        }
        if kind == "x25519" {
            let mut scalar = vec![0u8; 32];
            let _ = rusty_web_crypto::get_random_values(&mut scalar);

            let pubkey = rusty_web_crypto::x25519_base(&scalar);
            let (priv_pem, pub_pem) = x25519_keypair_to_pem(&scalar, &pubkey);
            let result = new_object(rt);
            let _result_root = rt.push_temporary_value_roots(&[Value::Object(result)]);
            let d1 = Value::Object(new_object(rt));
            let d2 = Value::Object(new_object(rt));
            let pubv = keypair_result_value(rt, &opts, "publicKeyEncoding", &pub_pem, "public", d1);
            let privv =
                keypair_result_value(rt, &opts, "privateKeyEncoding", &priv_pem, "private", d2);
            rt.object_set(result, "publicKey".into(), pubv);
            rt.object_set(result, "privateKey".into(), privv);
            return Ok(Value::Object(result));
        }
        if kind == "ec" {
            let curve = match &opts {
                Value::Object(id) => {
                    value_string(Some(&rt.object_get(*id, "namedCurve"))).to_ascii_lowercase()
                }
                _ => String::new(),
            };
            if !matches!(curve.as_str(), "prime256v1" | "secp256r1" | "p-256") {
                return Err(crypto_error(
                    "generateKeyPairSync: ec namedCurve must be prime256v1/secp256r1/P-256 (R1)",
                ));
            }
            let d = ecdh_gen_scalar();
            let point = ecdh_pubkey(&d);
            let (priv_pem, pub_pem) = ec_keypair_to_pem(&d, &point[1..33], &point[33..65]);
            let result = new_object(rt);
            let _result_root = rt.push_temporary_value_roots(&[Value::Object(result)]);
            let d1 = ec_key_details(rt);
            let d2 = ec_key_details(rt);
            let pubv = keypair_result_value(rt, &opts, "publicKeyEncoding", &pub_pem, "public", d1);
            let privv =
                keypair_result_value(rt, &opts, "privateKeyEncoding", &priv_pem, "private", d2);
            rt.object_set(result, "publicKey".into(), pubv);
            rt.object_set(result, "privateKey".into(), privv);
            return Ok(Value::Object(result));
        }
        if kind != "rsa" {
            return Err(crypto_error(
                "generateKeyPairSync: 'rsa'/'ec'/'ed25519'/'x25519' supported",
            ));
        }
        if !matches!(modulus_len, 1024 | 2048 | 3072 | 4096) {
            return Err(crypto_error(
                "generateKeyPairSync: rsa modulusLength must be 1024/2048/3072/4096",
            ));
        }
        let mut e_bytes = pub_exp.to_be_bytes().to_vec();
        while e_bytes.len() > 1 && e_bytes[0] == 0 {
            e_bytes.remove(0);
        }
        let e_big = rusty_web_crypto::BigUInt::from_be_bytes(&e_bytes);
        let (n, e, d, pp, q, dp, dq, qinv) =
            rusty_web_crypto::rsa_generate_keypair_crt(modulus_len, &e_big)
                .map_err(|err| crypto_error(format!("generateKeyPairSync: {err}")))?;
        let (priv_pem, pub_pem) = rsa_keypair_to_pem(&n, &e, &d, &pp, &q, &dp, &dq, &qinv);
        let result = new_object(rt);
        let _result_root = rt.push_temporary_value_roots(&[Value::Object(result)]);
        let d1 = rsa_key_details(rt, modulus_len, pub_exp);
        let d2 = rsa_key_details(rt, modulus_len, pub_exp);
        let pubv = keypair_result_value(rt, &opts, "publicKeyEncoding", &pub_pem, "public", d1);
        let privv = keypair_result_value(rt, &opts, "privateKeyEncoding", &priv_pem, "private", d2);
        rt.object_set(result, "publicKey".into(), pubv);
        rt.object_set(result, "privateKey".into(), privv);
        Ok(Value::Object(result))
    });

    register_method(rt, crypto, "diffieHellman", |rt, args| {
        let opts =
            match args.first() {
                Some(Value::Object(id)) => *id,
                _ => return Err(crypto_error(
                    "crypto.diffieHellman: options object with {privateKey, publicKey} required",
                )),
            };
        let priv_v = rt.object_get(opts, "privateKey");
        let pub_v = rt.object_get(opts, "publicKey");
        let priv_pem = key_as_pem(rt, &priv_v)
            .ok_or_else(|| crypto_error("crypto.diffieHellman: privateKey is not a key"))?;
        let pub_pem = key_as_pem(rt, &pub_v)
            .ok_or_else(|| crypto_error("crypto.diffieHellman: publicKey is not a key"))?;
        match key_alg_oid(&priv_pem).as_deref() {
            Some(o) if o == X25519_OID => {
                let scalar = parse_ed25519_priv(&priv_pem).ok_or_else(|| {
                    crypto_error("crypto.diffieHellman: X25519 private key parse failed")
                })?;
                let peer = parse_ed25519_pub(&pub_pem).ok_or_else(|| {
                    crypto_error("crypto.diffieHellman: X25519 public key parse failed")
                })?;
                Ok(make_buffer(rt, &rusty_web_crypto::x25519(&scalar, &peer)))
            }
            Some(o) if o == EC_PUBKEY_OID => {

                let d = rusty_tls::parse_ec_p256_private_key_pem(&priv_pem).map_err(|_| {
                    crypto_error("crypto.diffieHellman: EC private key parse failed (P-256 only)")
                })?;
                let (x, y) = parse_ec_pub(&pub_pem).ok_or_else(|| {
                    crypto_error("crypto.diffieHellman: EC public key parse failed (P-256 only)")
                })?;
                let secret = rusty_web_crypto::ecdh(&rusty_web_crypto::curve_p256(), &d, &x, &y)
                    .map_err(|e| crypto_error(format!("crypto.diffieHellman: {e}")))?;
                Ok(make_buffer(rt, &secret))
            }
            _ => Err(crypto_error(
                "crypto.diffieHellman: only X25519 and EC P-256 keys supported (RSA/other-curve follow-on)",
            )),
        }
    });
    register_method(rt, crypto, "sign", |rt, args| {
        let digest = value_string(args.first());
        let data = extract_bytes(rt, args.get(1).unwrap_or(&Value::Undefined));
        let key = args.get(2).cloned().unwrap_or(Value::Undefined);
        let ieee_p1363 = wants_ieee_p1363(rt, &key);

        if let Some(pem) = key_as_pem(rt, &key) {
            if key_alg_oid(&pem).as_deref() == Some(&ED25519_OID) {

                ed25519_reject_digest(rt, args.first())?;
                let seed = parse_ed25519_priv(&pem)
                    .ok_or_else(|| crypto_error("sign: Ed25519 key parse failed"))?;
                return Ok(make_buffer(
                    rt,
                    &rusty_web_crypto::ed25519_sign(&seed, &data),
                ));
            }
            if let Ok((n, d)) = rusty_tls::parse_rsa_private_key_pem(&pem) {
                let dg = if digest.is_empty() {
                    "sha256".to_string()
                } else {
                    digest.clone()
                };
                if is_pss_padding(rt, &key) {

                    let (hash_fn, hlen, _) = digest_fn_and_len(&dg)?;
                    let slen = pss_salt_len(rt, &key, &n, hlen);
                    let mut salt = vec![0u8; slen];
                    fill_random(&mut salt)?;
                    let sig = rusty_web_crypto::rsa_pss_sign(&n, &d, &data, &salt, hash_fn, hlen)
                        .map_err(|e| crypto_error(format!("sign: {e}")))?;
                    return Ok(make_buffer(rt, &sig));
                }
                let (h, hn) = sign_digest(&dg, &data);
                let sig = rusty_web_crypto::rsa_pkcs1_v15_sign(&n, &d, &h, &hn)
                    .map_err(|e| crypto_error(format!("sign: {e}")))?;
                return Ok(make_buffer(rt, &sig));
            }
            if let Ok(d) = rusty_tls::parse_ec_p256_private_key_pem(&pem) {
                let raw = rusty_web_crypto::ecdsa_p256_sha256_sign_deterministic(&d, &data)
                    .map_err(|e| crypto_error(format!("sign: {e}")))?;

                let out = if ieee_p1363 {
                    raw
                } else {
                    ecdsa_raw_to_der(&raw)
                };
                return Ok(make_buffer(rt, &out));
            }
        }
        let is_rsa = match &key {
            Value::Object(id) => !matches!(rt.object_get(*id, "n"), Value::Undefined),
            _ => false,
        };
        if is_rsa {
            let pss = match &key {
                Value::Object(id) => value_string(Some(&rt.object_get(*id, "padding")))
                    .to_ascii_lowercase()
                    .contains("pss"),
                _ => false,
            };
            let sig = rsa_sign(rt, &digest, &data, &key, pss)?;
            return Ok(make_buffer(rt, &sig));
        }
        let d = key_bytes(rt, &key, "d");
        if d.is_empty() {
            return Err(crypto_error("node:crypto sign: EC private key requires d"));
        }
        let hash = digest_bytes(&digest, &data);
        let sig =
            rusty_web_crypto::ecdsa_sign_deterministic(&rusty_web_crypto::curve_p256(), &d, &hash)
                .map_err(|e| crypto_error(format!("node:crypto sign: {e}")))?;
        Ok(make_buffer(rt, &sig))
    });
    register_method(rt, crypto, "verify", |rt, args| {

        let digest = value_string(args.first());
        let data = extract_bytes(rt, args.get(1).unwrap_or(&Value::Undefined));
        let key = args.get(2).cloned().unwrap_or(Value::Undefined);
        let ieee_p1363 = wants_ieee_p1363(rt, &key);
        let sig = extract_bytes(rt, args.get(3).unwrap_or(&Value::Undefined));
        if let Some(pem) = key_as_pem(rt, &key) {
            if key_alg_oid(&pem).as_deref() == Some(&ED25519_OID) {

                ed25519_reject_digest(rt, args.first())?;
                let pk = parse_ed25519_pub(&pem)
                    .ok_or_else(|| crypto_error("verify: Ed25519 key parse failed"))?;
                return Ok(Value::Boolean(rusty_web_crypto::ed25519_verify(
                    &pk, &data, &sig,
                )));
            }
            if let Some((n, e)) = parse_rsa_pub(&pem) {
                let dg = if digest.is_empty() {
                    "sha256".to_string()
                } else {
                    digest.clone()
                };
                if is_pss_padding(rt, &key) {
                    let (hash_fn, hlen, _) = digest_fn_and_len(&dg)?;
                    let slen = pss_salt_len(rt, &key, &n, hlen);
                    return Ok(Value::Boolean(
                        rusty_web_crypto::rsa_pss_verify(&n, &e, &data, &sig, slen, hash_fn, hlen)
                            .is_ok(),
                    ));
                }
                let (h, hn) = sign_digest(&dg, &data);
                return Ok(Value::Boolean(
                    rusty_web_crypto::rsa_pkcs1_v15_verify(&n, &e, &h, &sig, &hn).is_ok(),
                ));
            }
            if let Some((x, y)) = parse_ec_pub(&pem) {

                let raw = if ieee_p1363 {
                    Some(sig.clone())
                } else {
                    ecdsa_der_to_raw(&sig)
                };
                return Ok(Value::Boolean(match raw {
                    Some(raw) => {
                        rusty_web_crypto::ecdsa_p256_sha256_verify(&x, &y, &data, &raw).is_ok()
                    }
                    None => false,
                }));
            }
        }
        let is_rsa = match &key {
            Value::Object(id) => !matches!(rt.object_get(*id, "n"), Value::Undefined),
            _ => false,
        };
        if is_rsa {
            let pss = match &key {
                Value::Object(id) => value_string(Some(&rt.object_get(*id, "padding")))
                    .to_ascii_lowercase()
                    .contains("pss"),
                _ => false,
            };
            return Ok(Value::Boolean(rsa_verify(
                rt, &digest, &data, &key, &sig, pss,
            )?));
        }
        let qx = key_bytes(rt, &key, "qx");
        let qy = key_bytes(rt, &key, "qy");
        if qx.is_empty() || qy.is_empty() {
            return Err(crypto_error(
                "node:crypto verify: EC public key requires qx/qy",
            ));
        }
        let hash = digest_bytes(&digest, &data);
        let ok =
            rusty_web_crypto::ecdsa_verify(&rusty_web_crypto::curve_p256(), &qx, &qy, &hash, &sig)
                .is_ok();
        Ok(Value::Boolean(ok))
    });

    register_method(rt, crypto, "getHashes", |rt, _args| {

        let arr = rt.alloc_object(rusty_js_runtime::value::Object::new_array());
        let _arr_root = rt.push_temporary_value_roots(&[Value::Object(arr)]);
        let names = [

            "sha1",
            "sha224",
            "sha256",
            "sha384",
            "sha512",
            "md5",
            "ripemd160",
            "rmd160",
            "sha3-224",
            "sha3-256",
            "sha3-384",
            "sha3-512",
            "shake128",
            "shake256",
            "blake2b512",
            "blake2s256",
            "md5-sha1",
            "sha512-256",
            "sha512-224",
            "sm3",
        ];
        for (i, name) in names.iter().enumerate() {
            rt.object_set(
                arr,
                i.to_string(),
                Value::String(std::rc::Rc::new(rusty_js_runtime::value::JsString::from(
                    *name,
                ))),
            );
        }
        rt.object_set(arr, "length".into(), Value::Number(names.len() as f64));
        Ok(Value::Object(arr))
    });
    register_method(rt, crypto, "getCiphers", |rt, _args| {
        let arr = rt.alloc_object(rusty_js_runtime::value::Object::new_array());
        let _arr_root = rt.push_temporary_value_roots(&[Value::Object(arr)]);
        for (i, name) in [
            "aes-128-cbc",
            "aes-192-cbc",
            "aes-256-cbc",
            "aes-128-ctr",
            "aes-192-ctr",
            "aes-256-ctr",
            "aes-128-gcm",
            "aes-192-gcm",
            "aes-256-gcm",
            "aes-128-ecb",
            "aes-192-ecb",
            "aes-256-ecb",
            "aes-128-cfb",
            "aes-192-cfb",
            "aes-256-cfb",
            "aes-128-ofb",
            "aes-192-ofb",
            "aes-256-ofb",
            "chacha20-poly1305",
            "chacha20",
        ]
        .iter()
        .enumerate()
        {
            rt.object_set(
                arr,
                i.to_string(),
                Value::String(std::rc::Rc::new(rusty_js_runtime::value::JsString::from(
                    *name,
                ))),
            );
        }
        rt.object_set(arr, "length".into(), Value::Number(20.0));
        Ok(Value::Object(arr))
    });
    register_method(rt, crypto, "getCurves", |rt, _args| {

        let arr = rt.alloc_object(rusty_js_runtime::value::Object::new_array());
        let _arr_root = rt.push_temporary_value_roots(&[Value::Object(arr)]);
        for (i, name) in ["prime256v1", "secp384r1", "secp521r1"].iter().enumerate() {
            rt.object_set(
                arr,
                i.to_string(),
                Value::String(Rc::new(rusty_js_runtime::value::JsString::from(*name))),
            );
        }
        rt.object_set(arr, "length".into(), Value::Number(3.0));
        Ok(Value::Object(arr))
    });
    register_method(rt, crypto, "timingSafeEqual", |rt, args| {

        let a = extract_bytes(rt, &args.first().cloned().unwrap_or(Value::Undefined));
        let b = extract_bytes(rt, &args.get(1).cloned().unwrap_or(Value::Undefined));
        if a.len() != b.len() {
            return Err(coded_crypto_error(
                rt,
                "RangeError",
                "ERR_CRYPTO_TIMING_SAFE_EQUAL_LENGTH",
                "Input buffers must have the same byte length",
            ));
        }
        let mut diff = 0u8;
        for i in 0..a.len() {
            diff |= a[i] ^ b[i];
        }
        Ok(Value::Boolean(diff == 0))
    });

    if let Value::Object(runtime_crypto) = rt.global_get("crypto") {
        let subtle = rt.object_get(runtime_crypto, "subtle");
        if !matches!(subtle, Value::Undefined) {
            rt.object_set(crypto, "subtle".into(), subtle.clone());
            rt.object_set(webcrypto, "subtle".into(), subtle);
        }
    }

    set_constant(rt, crypto, "default", Value::Object(crypto));

    pub fn b64_to_bytes(s: &str) -> Vec<u8> {
        fn val(c: u8) -> Option<u8> {
            match c {
                b'A'..=b'Z' => Some(c - b'A'),
                b'a'..=b'z' => Some(c - b'a' + 26),
                b'0'..=b'9' => Some(c - b'0' + 52),
                b'+' => Some(62),
                b'/' => Some(63),
                _ => None,
            }
        }
        let mut out = Vec::new();
        let mut acc = 0u32;
        let mut bits = 0u32;
        for &c in s.as_bytes() {
            if let Some(v) = val(c) {
                acc = (acc << 6) | v as u32;
                bits += 6;
                if bits >= 8 {
                    bits -= 8;
                    out.push((acc >> bits) as u8);
                }
            }
        }
        out
    }

    fn pss_sign(
        n: &[u8],
        d: &[u8],
        data: &[u8],
        salt_len: usize,
        hname: &str,
    ) -> Result<Vec<u8>, String> {
        use rusty_web_crypto as w;
        let mut salt = vec![0u8; salt_len];
        let _ = w::get_random_values(&mut salt);
        match hname {
            "SHA-1" => w::rsa_pss_sign(n, d, data, &salt, |m| w::digest_sha1(m).to_vec(), 20),
            "SHA-384" => w::rsa_pss_sign(n, d, data, &salt, |m| w::digest_sha384(m).to_vec(), 48),
            "SHA-512" => w::rsa_pss_sign(n, d, data, &salt, |m| w::digest_sha512(m).to_vec(), 64),
            _ => w::rsa_pss_sign(n, d, data, &salt, |m| w::digest_sha256(m).to_vec(), 32),
        }
    }
    fn pss_verify(n: &[u8], e: &[u8], data: &[u8], sig: &[u8], s_len: usize, hname: &str) -> bool {
        use rusty_web_crypto as w;
        match hname {
            "SHA-1" => {
                w::rsa_pss_verify(n, e, data, sig, s_len, |m| w::digest_sha1(m).to_vec(), 20)
            }
            "SHA-384" => {
                w::rsa_pss_verify(n, e, data, sig, s_len, |m| w::digest_sha384(m).to_vec(), 48)
            }
            "SHA-512" => {
                w::rsa_pss_verify(n, e, data, sig, s_len, |m| w::digest_sha512(m).to_vec(), 64)
            }
            _ => w::rsa_pss_verify(n, e, data, sig, s_len, |m| w::digest_sha256(m).to_vec(), 32),
        }
        .is_ok()
    }
    fn is_pss(rt: &Runtime, key_arg: &Value) -> bool {
        matches!(key_arg, Value::Object(id) if matches!(rt.object_get(*id, "padding"), Value::Number(n) if n as i64 == 6))
    }
    fn salt_len_of(rt: &Runtime, key_arg: &Value, default: usize) -> usize {
        match key_arg {
            Value::Object(id) => match rt.object_get(*id, "saltLength") {
                Value::Number(n) if n >= 0.0 => n as usize,
                _ => default,
            },
            _ => default,
        }
    }

    fn key_type_of(pem: &str) -> &'static str {
        match key_alg_oid(pem).as_deref() {
            Some(o) if o == ED25519_OID => "ed25519",
            Some(o) if o == X25519_OID => "x25519",
            Some(o) if o == EC_PUBKEY_OID => "ec",
            _ => "rsa",
        }
    }

    const ED25519_OID: [u64; 4] = [1, 3, 101, 112];
    const X25519_OID: [u64; 4] = [1, 3, 101, 110];

    fn x25519_keypair_to_pem(scalar: &[u8], pubkey: &[u8]) -> (String, String) {
        use rusty_asn1_der as der;
        let alg_id = der::enc_sequence(&[der::enc_oid(&X25519_OID)]);
        let pkcs8 = der::enc_sequence(&[
            der::enc_integer_small(0),
            alg_id.clone(),
            der::enc_octet_string(&der::enc_octet_string(scalar)),
        ]);
        let spki = der::enc_sequence(&[alg_id, der::enc_bit_string(pubkey)]);
        (
            pem_wrap("PRIVATE KEY", &pkcs8),
            pem_wrap("PUBLIC KEY", &spki),
        )
    }

    fn key_alg_oid(pem: &str) -> Option<Vec<u64>> {
        let der = pem_to_der(pem);
        let outer = rusty_asn1_der::parse_single(&der).ok()?;
        let mut rd = rusty_asn1_der::DerReader::new(outer.content);
        let first = rd.read_tlv().ok()?;

        let alg = if first.tag == rusty_asn1_der::TAG_INTEGER {
            rd.read_tag(rusty_asn1_der::TAG_SEQUENCE).ok()?
        } else {
            first
        };
        let mut ar = rusty_asn1_der::DerReader::new(alg.content);
        ar.read_tlv().ok().and_then(|oid| oid.as_oid().ok())
    }

    fn parse_ed25519_priv(pem: &str) -> Option<Vec<u8>> {
        let der = pem_to_der(pem);
        let outer = rusty_asn1_der::parse_single(&der).ok()?;
        let mut rd = rusty_asn1_der::DerReader::new(outer.content);
        rd.read_tag(rusty_asn1_der::TAG_INTEGER).ok()?;
        rd.read_tag(rusty_asn1_der::TAG_SEQUENCE).ok()?;
        let pk = rd.read_tag(rusty_asn1_der::TAG_OCTET_STRING).ok()?;
        let inner = rusty_asn1_der::parse_single(pk.content).ok()?;
        Some(inner.content.to_vec())
    }

    fn parse_ed25519_pub(pem: &str) -> Option<Vec<u8>> {
        let der = pem_to_der(pem);
        let outer = rusty_asn1_der::parse_single(&der).ok()?;
        let mut rd = rusty_asn1_der::DerReader::new(outer.content);
        rd.read_tag(rusty_asn1_der::TAG_SEQUENCE).ok()?;
        let bitstr = rd.read_tag(rusty_asn1_der::TAG_BIT_STRING).ok()?;
        Some(bitstr.content[1..].to_vec())
    }
    fn ed25519_keypair_to_pem(seed: &[u8], pubkey: &[u8]) -> (String, String) {
        use rusty_asn1_der as der;
        let alg_id = der::enc_sequence(&[der::enc_oid(&ED25519_OID)]);
        let pkcs8 = der::enc_sequence(&[
            der::enc_integer_small(0),
            alg_id.clone(),
            der::enc_octet_string(&der::enc_octet_string(seed)),
        ]);
        let spki = der::enc_sequence(&[alg_id, der::enc_bit_string(pubkey)]);
        (
            pem_wrap("PRIVATE KEY", &pkcs8),
            pem_wrap("PUBLIC KEY", &spki),
        )
    }

    fn wants_ieee_p1363(rt: &mut Runtime, v: &Value) -> bool {
        matches!(v, Value::Object(id)
            if matches!(rt.object_get(*id, "dsaEncoding"), Value::String(ref s) if s.as_str() == "ieee-p1363"))
    }

    fn is_pss_padding(rt: &mut Runtime, v: &Value) -> bool {
        matches!(v, Value::Object(id)
            if matches!(rt.object_get(*id, "padding"), Value::Number(n) if n == 6.0))
    }

    fn pss_max_salt_len(n: &[u8], hlen: usize) -> usize {
        let mut mod_bits = n.len() * 8;
        for &b in n {
            if b == 0 {
                mod_bits -= 8;
            } else {
                mod_bits -= b.leading_zeros() as usize;
                break;
            }
        }
        let em_len = (mod_bits.saturating_sub(1) + 7) / 8;
        em_len.saturating_sub(hlen + 2)
    }

    fn pss_salt_len(rt: &mut Runtime, key: &Value, n: &[u8], hlen: usize) -> usize {
        let requested = match key {
            Value::Object(id) => match rt.object_get(*id, "saltLength") {
                Value::Number(x) if x.is_finite() => Some(x as i64),
                _ => None,
            },
            _ => None,
        };
        match requested {
            Some(-1) => hlen,
            Some(v) if v >= 0 => v as usize,
            _ => pss_max_salt_len(n, hlen),
        }
    }

    fn ed25519_reject_digest(rt: &mut Runtime, alg: Option<&Value>) -> Result<(), RuntimeError> {
        match alg {
            None | Some(Value::Undefined) | Some(Value::Null) => Ok(()),
            _ => Err(coded_crypto_error(
                rt,
                "Error",
                "ERR_OSSL_INVALID_DIGEST",
                "error:1C80007A:Provider routines::invalid digest",
            )),
        }
    }

    fn key_as_pem(rt: &mut Runtime, v: &Value) -> Option<String> {
        match v {
            Value::String(s) if s.as_str().contains("BEGIN") => Some(s.as_str().to_string()),
            Value::Object(id) => {

                if let Value::String(s) = rt.object_get(*id, "__pem") {
                    if s.as_str().contains("BEGIN") {
                        return Some(s.as_str().to_string());
                    }
                }
                match rt.object_get(*id, "key") {
                    Value::String(s) if s.as_str().contains("BEGIN") => {
                        Some(s.as_str().to_string())
                    }

                    nested @ Value::Object(_) => key_as_pem(rt, &nested),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    const EC_PUBKEY_OID: [u64; 6] = [1, 2, 840, 10045, 2, 1];
    const P256_CURVE_OID: [u64; 7] = [1, 2, 840, 10045, 3, 1, 7];
    fn ec_keypair_to_pem(d: &[u8], x: &[u8], y: &[u8]) -> (String, String) {
        use rusty_asn1_der as der;
        let mut point = vec![0x04u8];
        point.extend_from_slice(x);
        point.extend_from_slice(y);
        let alg_id =
            der::enc_sequence(&[der::enc_oid(&EC_PUBKEY_OID), der::enc_oid(&P256_CURVE_OID)]);

        let sec1 = der::enc_sequence(&[
            der::enc_integer_small(1),
            der::enc_octet_string(d),
            der::enc_context_constructed(1, &der::enc_bit_string(&point)),
        ]);
        let pkcs8 = der::enc_sequence(&[
            der::enc_integer_small(0),
            alg_id.clone(),
            der::enc_octet_string(&sec1),
        ]);
        let spki = der::enc_sequence(&[alg_id, der::enc_bit_string(&point)]);
        (
            pem_wrap("PRIVATE KEY", &pkcs8),
            pem_wrap("PUBLIC KEY", &spki),
        )
    }

    fn parse_ec_pub(pem: &str) -> Option<(Vec<u8>, Vec<u8>)> {
        let der = pem_to_der(pem);
        let outer = rusty_asn1_der::parse_single(&der).ok()?;
        if outer.tag != rusty_asn1_der::TAG_SEQUENCE {
            return None;
        }
        let mut rd = rusty_asn1_der::DerReader::new(outer.content);
        rd.read_tag(rusty_asn1_der::TAG_SEQUENCE).ok()?;
        let bitstr = rd.read_tag(rusty_asn1_der::TAG_BIT_STRING).ok()?;
        let pt = &bitstr.content[1..];
        if pt.len() < 65 || pt[0] != 0x04 {
            return None;
        }
        Some((pt[1..33].to_vec(), pt[33..65].to_vec()))
    }

    fn ecdsa_raw_to_der(raw: &[u8]) -> Vec<u8> {
        use rusty_asn1_der as der;
        der::enc_sequence(&[
            der::enc_integer_unsigned(&raw[..32]),
            der::enc_integer_unsigned(&raw[32..64]),
        ])
    }

    fn ecdsa_der_to_raw(sig: &[u8]) -> Option<Vec<u8>> {
        let seq = rusty_asn1_der::parse_single(sig).ok()?;
        if seq.tag != rusty_asn1_der::TAG_SEQUENCE {
            return None;
        }
        let mut rd = rusty_asn1_der::DerReader::new(seq.content);
        let r = strip_int_sign(rd.read_tag(rusty_asn1_der::TAG_INTEGER).ok()?.content);
        let s = strip_int_sign(rd.read_tag(rusty_asn1_der::TAG_INTEGER).ok()?.content);
        let pad = |v: &[u8]| {
            let mut o = vec![0u8; 32];
            let n = v.len().min(32);
            o[32 - n..].copy_from_slice(&v[v.len() - n..]);
            o
        };
        let mut out = pad(&r);
        out.extend_from_slice(&pad(&s));
        Some(out)
    }

    const RSA_OID: [u64; 7] = [1, 2, 840, 113549, 1, 1, 1];
    fn pem_wrap(label: &str, der: &[u8]) -> String {
        let b64 = base64_encode(der);
        let mut body = String::new();
        let mut i = 0;
        while i < b64.len() {
            let end = (i + 64).min(b64.len());
            body.push_str(&b64[i..end]);
            body.push('\n');
            i = end;
        }
        format!("-----BEGIN {label}-----\n{body}-----END {label}-----\n")
    }
    #[allow(clippy::too_many_arguments)]
    fn rsa_keypair_to_pem(
        n: &[u8],
        e: &[u8],
        d: &[u8],
        p: &[u8],
        q: &[u8],
        dp: &[u8],
        dq: &[u8],
        qinv: &[u8],
    ) -> (String, String) {
        use rusty_asn1_der as der;

        let rsa_priv = der::enc_sequence(&[
            der::enc_integer_small(0),
            der::enc_integer_unsigned(n),
            der::enc_integer_unsigned(e),
            der::enc_integer_unsigned(d),
            der::enc_integer_unsigned(p),
            der::enc_integer_unsigned(q),
            der::enc_integer_unsigned(dp),
            der::enc_integer_unsigned(dq),
            der::enc_integer_unsigned(qinv),
        ]);
        let alg_id = der::enc_sequence(&[der::enc_oid(&RSA_OID), der::enc_null()]);
        let pkcs8 = der::enc_sequence(&[
            der::enc_integer_small(0),
            alg_id.clone(),
            der::enc_octet_string(&rsa_priv),
        ]);

        let rsa_pub =
            der::enc_sequence(&[der::enc_integer_unsigned(n), der::enc_integer_unsigned(e)]);
        let spki = der::enc_sequence(&[alg_id, der::enc_bit_string(&rsa_pub)]);
        (
            pem_wrap("PRIVATE KEY", &pkcs8),
            pem_wrap("PUBLIC KEY", &spki),
        )
    }

    fn ecdh_curve(name: &str) -> Option<rusty_web_crypto::Curve> {
        match name {
            "prime256v1" | "secp256r1" | "p-256" | "P-256" => Some(rusty_web_crypto::curve_p256()),
            "secp384r1" | "p-384" | "P-384" => Some(rusty_web_crypto::curve_p384()),
            "secp521r1" | "p-521" | "P-521" => Some(rusty_web_crypto::curve_p521()),
            _ => None,
        }
    }
    fn ecdh_pubkey_curve(c: &rusty_web_crypto::Curve, d_bytes: &[u8]) -> Vec<u8> {
        let d = rusty_web_crypto::BigUInt::from_be_bytes(d_bytes);
        match rusty_web_crypto::ec_scalar_mul(c, &d, &c.g) {
            rusty_web_crypto::P256Point::Affine { x, y } => {
                let mut o = vec![0x04u8];
                o.extend_from_slice(&x.to_be_bytes(c.coord_bytes));
                o.extend_from_slice(&y.to_be_bytes(c.coord_bytes));
                o
            }
            _ => Vec::new(),
        }
    }
    fn ecdh_gen_d(c: &rusty_web_crypto::Curve) -> Vec<u8> {
        rusty_web_crypto::ec_generate_keypair(c).0
    }

    fn strip_int_sign(b: &[u8]) -> Vec<u8> {
        let mut i = 0;
        while i + 1 < b.len() && b[i] == 0 {
            i += 1;
        }
        b[i..].to_vec()
    }

    fn key_pem(rt: &mut Runtime, v: &Value) -> String {
        match v {
            Value::String(s) => s.as_str().to_string(),
            Value::Object(id) => {

                match rt.object_get(*id, "key") {
                    Value::String(s) => s.as_str().to_string(),
                    nested @ Value::Object(_) => key_pem(rt, &nested),
                    _ => match rt.object_get(*id, "__pem") {
                        Value::String(s) => s.as_str().to_string(),
                        _ => String::new(),
                    },
                }
            }
            _ => String::new(),
        }
    }
    fn pem_to_der(pem: &str) -> Vec<u8> {
        let b64: String = pem
            .lines()
            .filter(|l| !l.contains("-----"))
            .flat_map(|l| l.chars())
            .filter(|c| !c.is_whitespace())
            .collect();
        b64_to_bytes(&b64)
    }

    fn key_object_export(rt: &mut Runtime, args: &[Value]) -> Result<Value, RuntimeError> {
        let this = match rt.current_this() {
            Value::Object(t) => t,
            _ => return Ok(Value::Undefined),
        };
        let pem = match rt.object_get(this, "__pem") {
            Value::String(s) => s.as_str().to_string(),
            _ => return Ok(Value::Undefined),
        };

        let opts = match args.first() {
            Some(Value::Object(id)) => *id,
            _ => {
                return Err(coded_crypto_error(
                    rt,
                    "TypeError",
                    "ERR_INVALID_ARG_TYPE",
                    "The \"options\" argument must be of type object.",
                ))
            }
        };
        let format = match rt.object_get(opts, "format") {
            Value::String(s) => s.as_str().to_string(),
            _ => "pem".to_string(),
        };
        if format == "der" {
            let der = pem_to_der(&pem);
            return Ok(make_buffer(rt, &der));
        }
        if format == "jwk" {
            let is_public = matches!(rt.object_get(this, "type"), Value::String(ref s) if s.as_str() == "public");
            if is_public {
                if let Some(jwk) = key_public_jwk(rt, &pem, true) {
                    return Ok(jwk);
                }
            } else if let Some(jwk) = key_private_jwk(rt, &pem) {
                return Ok(jwk);
            }

            return Err(coded_crypto_error(
                rt,
                "Error",
                "ERR_CRYPTO_JWK_UNSUPPORTED_KEY",
                "Unsupported JWK Key Type.",
            ));
        }
        Ok(Value::String(Rc::new(
            rusty_js_runtime::value::JsString::from(pem),
        )))
    }

    fn key_public_jwk(rt: &mut Runtime, pem: &str, is_public: bool) -> Option<Value> {
        if !is_public {
            return None;
        }
        let jwk = new_object(rt);
        match key_type_of(pem) {
            "rsa" => {
                let (n, e) = parse_rsa_pub(pem)?;
                rt.object_set(jwk, "kty".into(), crypto_sval("RSA"));
                rt.object_set(jwk, "n".into(), crypto_sval(&base64url_encode(&n)));
                rt.object_set(jwk, "e".into(), crypto_sval(&base64url_encode(&e)));
            }
            "ec" => {
                let (x, y) = parse_ec_pub(pem)?;
                rt.object_set(jwk, "kty".into(), crypto_sval("EC"));
                rt.object_set(jwk, "x".into(), crypto_sval(&base64url_encode(&x)));
                rt.object_set(jwk, "y".into(), crypto_sval(&base64url_encode(&y)));
                rt.object_set(jwk, "crv".into(), crypto_sval("P-256"));
            }
            "ed25519" => {
                let x = parse_ed25519_pub(pem)?;
                rt.object_set(jwk, "crv".into(), crypto_sval("Ed25519"));
                rt.object_set(jwk, "x".into(), crypto_sval(&base64url_encode(&x)));
                rt.object_set(jwk, "kty".into(), crypto_sval("OKP"));
            }
            "x25519" => {
                let x = parse_ed25519_pub(pem)?;
                rt.object_set(jwk, "crv".into(), crypto_sval("X25519"));
                rt.object_set(jwk, "x".into(), crypto_sval(&base64url_encode(&x)));
                rt.object_set(jwk, "kty".into(), crypto_sval("OKP"));
            }
            _ => return None,
        }
        Some(Value::Object(jwk))
    }

    fn parse_rsa_priv(pem: &str) -> Option<[Vec<u8>; 8]> {
        let der = pem_to_der(pem);
        let outer = rusty_asn1_der::parse_single(&der).ok()?;
        let mut rd = rusty_asn1_der::DerReader::new(outer.content);
        rd.read_tag(rusty_asn1_der::TAG_INTEGER).ok()?;
        rd.read_tag(rusty_asn1_der::TAG_SEQUENCE).ok()?;
        let pk = rd.read_tag(rusty_asn1_der::TAG_OCTET_STRING).ok()?;

        let inner = rusty_asn1_der::parse_single(pk.content).ok()?;
        let mut ir = rusty_asn1_der::DerReader::new(inner.content);
        ir.read_tag(rusty_asn1_der::TAG_INTEGER).ok()?;
        let mut next = || -> Option<Vec<u8>> {
            Some(strip_int_sign(
                ir.read_tag(rusty_asn1_der::TAG_INTEGER).ok()?.content,
            ))
        };
        Some([
            next()?,
            next()?,
            next()?,
            next()?,
            next()?,
            next()?,
            next()?,
            next()?,
        ])
    }

    fn key_private_jwk(rt: &mut Runtime, pem: &str) -> Option<Value> {
        if key_type_of(pem) != "rsa" {
            return None;
        }
        let [n, e, d, p, q, dp, dq, qi] = parse_rsa_priv(pem)?;
        let jwk = new_object(rt);

        rt.object_set(jwk, "kty".into(), crypto_sval("RSA"));
        for (k, v) in [
            ("n", &n),
            ("e", &e),
            ("d", &d),
            ("p", &p),
            ("q", &q),
            ("dp", &dp),
            ("dq", &dq),
            ("qi", &qi),
        ] {
            rt.object_set(jwk, k.into(), crypto_sval(&base64url_encode(v)));
        }
        Some(Value::Object(jwk))
    }

    fn jwk_to_pem(rt: &Runtime, jwk: ObjectRef, want: &str) -> Option<String> {
        let field = |k: &str| -> Option<Vec<u8>> {
            match rt.object_get(jwk, k) {
                Value::String(s) => Some(base64url_decode(s.as_str())),
                _ => None,
            }
        };
        let kty = match rt.object_get(jwk, "kty") {
            Value::String(s) => s.as_str().to_string(),
            _ => return None,
        };
        match kty.as_str() {
            "RSA" => {
                let n = field("n")?;
                let e = field("e")?;

                if let (Some(d), Some(p), Some(q), Some(dp), Some(dq), Some(qi)) = (
                    field("d"),
                    field("p"),
                    field("q"),
                    field("dp"),
                    field("dq"),
                    field("qi"),
                ) {
                    let (priv_pem, pub_pem) = rsa_keypair_to_pem(&n, &e, &d, &p, &q, &dp, &dq, &qi);
                    return Some(if want == "private" { priv_pem } else { pub_pem });
                }

                use rusty_asn1_der as der;
                let rsa_pub = der::enc_sequence(&[
                    der::enc_integer_unsigned(&n),
                    der::enc_integer_unsigned(&e),
                ]);
                let alg_id = der::enc_sequence(&[der::enc_oid(&RSA_OID), der::enc_null()]);
                let spki = der::enc_sequence(&[alg_id, der::enc_bit_string(&rsa_pub)]);
                Some(pem_wrap("PUBLIC KEY", &spki))
            }
            "EC" => {
                let x = field("x")?;
                let y = field("y")?;
                let d = field("d").unwrap_or_default();
                let (priv_pem, pub_pem) = ec_keypair_to_pem(&d, &x, &y);
                Some(if want == "private" && !d.is_empty() {
                    priv_pem
                } else {
                    pub_pem
                })
            }
            "OKP" => {
                let x = field("x")?;
                let crv = match rt.object_get(jwk, "crv") {
                    Value::String(s) => s.as_str().to_string(),
                    _ => String::new(),
                };
                let d = field("d").unwrap_or_default();
                let build = if crv == "X25519" {
                    x25519_keypair_to_pem
                } else {
                    ed25519_keypair_to_pem
                };
                let (priv_pem, pub_pem) = build(&d, &x);
                Some(if want == "private" && !d.is_empty() {
                    priv_pem
                } else {
                    pub_pem
                })
            }
            _ => None,
        }
    }

    fn parse_rsa_pub(pem: &str) -> Option<(Vec<u8>, Vec<u8>)> {
        let der = pem_to_der(pem);
        let outer = rusty_asn1_der::parse_single(&der).ok()?;
        if outer.tag != rusty_asn1_der::TAG_SEQUENCE {
            return None;
        }
        let mut rd = rusty_asn1_der::DerReader::new(outer.content);
        let first = rd.read_tlv().ok()?;

        if first.tag == rusty_asn1_der::TAG_SEQUENCE {
            let bitstr = rd.read_tag(rusty_asn1_der::TAG_BIT_STRING).ok()?;
            let spk = &bitstr.content[1..];
            let rsaseq = rusty_asn1_der::parse_single(spk).ok()?;
            let mut r2 = rusty_asn1_der::DerReader::new(rsaseq.content);
            let n = r2.read_tag(rusty_asn1_der::TAG_INTEGER).ok()?;
            let e = r2.read_tag(rusty_asn1_der::TAG_INTEGER).ok()?;
            return Some((strip_int_sign(n.content), strip_int_sign(e.content)));
        }

        if first.tag == rusty_asn1_der::TAG_INTEGER {
            let e = rd.read_tag(rusty_asn1_der::TAG_INTEGER).ok()?;
            return Some((strip_int_sign(first.content), strip_int_sign(e.content)));
        }
        None
    }

    fn sign_digest(algo: &str, data: &[u8]) -> (Vec<u8>, String) {
        let a = algo.to_ascii_lowercase().replace("rsa-", "");
        match a.as_str() {
            "sha1" => (rusty_web_crypto::digest_sha1(data).to_vec(), "SHA-1".into()),
            "sha384" => (
                rusty_web_crypto::digest_sha384(data).to_vec(),
                "SHA-384".into(),
            ),
            "sha512" => (
                rusty_web_crypto::digest_sha512(data).to_vec(),
                "SHA-512".into(),
            ),
            _ => (
                rusty_web_crypto::digest_sha256(data).to_vec(),
                "SHA-256".into(),
            ),
        }
    }
    fn sign_data_of(rt: &Runtime, obj: rusty_js_runtime::value::ObjectRef) -> Vec<u8> {
        match rt.object_get(obj, "__sign_data") {
            Value::String(s) => hex_decode(s.as_str()),
            _ => Vec::new(),
        }
    }

    fn oid_short(oid: &str) -> &'static str {
        match oid {
            "2.5.4.3" => "CN",
            "2.5.4.6" => "C",
            "2.5.4.7" => "L",
            "2.5.4.8" => "ST",
            "2.5.4.10" => "O",
            "2.5.4.11" => "OU",
            "2.5.4.5" => "serialNumber",
            "1.2.840.113549.1.9.1" => "emailAddress",
            _ => "",
        }
    }

    fn host_matches(pattern: &str, name: &str) -> bool {
        if let Some(rest) = pattern.strip_prefix("*.") {
            match name.strip_suffix(rest).and_then(|p| p.strip_suffix('.')) {
                Some(label) => !label.is_empty() && !label.contains('.'),
                None => false,
            }
        } else {
            pattern == name
        }
    }
    fn format_dn(dn: &rusty_x509::DistinguishedName) -> String {
        dn.attributes
            .iter()
            .map(|(oid, val)| {
                let k = oid_short(oid);
                if k.is_empty() {
                    format!("{oid}={val}")
                } else {
                    format!("{k}={val}")
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
    fn colon_hex(bytes: &[u8]) -> String {
        bytes
            .iter()
            .map(|b| format!("{b:02X}"))
            .collect::<Vec<_>>()
            .join(":")
    }

    fn format_x509_time(bytes: &[u8], tag: u8) -> String {
        let s: String = bytes.iter().map(|&b| b as char).collect();
        let digits: Vec<char> = s.chars().filter(|c| c.is_ascii_digit()).collect();
        let (year, rest): (i32, &[char]) = if tag == 0x18 && digits.len() >= 14 {
            (
                digits[0..4].iter().collect::<String>().parse().unwrap_or(0),
                &digits[4..],
            )
        } else if digits.len() >= 12 {
            let yy: i32 = digits[0..2].iter().collect::<String>().parse().unwrap_or(0);
            (if yy < 50 { 2000 + yy } else { 1900 + yy }, &digits[2..])
        } else {
            return s;
        };
        let g = |a: usize, b: usize| -> i32 {
            rest[a..b].iter().collect::<String>().parse().unwrap_or(0)
        };
        let (mo, da, hh, mi, ss) = (g(0, 2), g(2, 4), g(4, 6), g(6, 8), g(8, 10));
        const M: [&str; 12] = [
            "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
        ];
        let mon = M.get((mo.max(1) - 1) as usize).copied().unwrap_or("Jan");
        format!("{mon} {da:>2} {hh:02}:{mi:02}:{ss:02} {year} GMT")
    }
    fn make_x509(rt: &mut Runtime, der: &[u8]) -> Result<Value, RuntimeError> {
        let cert = rusty_x509::parse_certificate(der)
            .map_err(|e| RuntimeError::TypeError(format!("X509Certificate: parse: {e:?}")))?;
        let obj = new_object(rt);
        rt.object_set(
            obj,
            "subject".into(),
            crypto_sval(&format_dn(&cert.subject)),
        );
        rt.object_set(obj, "issuer".into(), crypto_sval(&format_dn(&cert.issuer)));
        rt.object_set(
            obj,
            "serialNumber".into(),
            crypto_sval(
                &cert
                    .serial_number
                    .iter()
                    .map(|b| format!("{b:02X}"))
                    .collect::<String>(),
            ),
        );
        rt.object_set(
            obj,
            "fingerprint".into(),
            crypto_sval(&colon_hex(&rusty_web_crypto::digest_sha1(der))),
        );
        rt.object_set(
            obj,
            "fingerprint256".into(),
            crypto_sval(&colon_hex(&rusty_web_crypto::digest_sha256(der))),
        );
        rt.object_set(
            obj,
            "fingerprint512".into(),
            crypto_sval(&colon_hex(&rusty_web_crypto::digest_sha512(der))),
        );
        rt.object_set(
            obj,
            "validFrom".into(),
            crypto_sval(&format_x509_time(
                &cert.validity.not_before,
                cert.validity.not_before_tag,
            )),
        );
        rt.object_set(
            obj,
            "validTo".into(),
            crypto_sval(&format_x509_time(
                &cert.validity.not_after,
                cert.validity.not_after_tag,
            )),
        );
        let raw = make_buffer(rt, der);
        rt.object_set(obj, "raw".into(), raw);
        let pem = format!(
            "-----BEGIN CERTIFICATE-----\n{}-----END CERTIFICATE-----\n",
            {
                let b64 = base64_encode(der);
                let mut o = String::new();
                let mut i = 0;
                while i < b64.len() {
                    let e = (i + 64).min(b64.len());
                    o.push_str(&b64[i..e]);
                    o.push('\n');
                    i = e;
                }
                o
            }
        );
        rt.object_set(obj, "__pem".into(), crypto_sval(&pem));

        let spki_pem = pem_wrap("PUBLIC KEY", &cert.subject_public_key_info.raw_der);
        let details = Value::Object(new_object(rt));
        let pk = build_asym_key_object(rt, &spki_pem, "public", details);
        rt.object_set(obj, "publicKey".into(), Value::Object(pk));

        let ca = cert
            .extensions
            .iter()
            .find(|e| e.oid == rusty_x509::OID_BASIC_CONSTRAINTS)
            .map(|e| {
                rusty_asn1_der::parse_single(&e.value)
                    .ok()
                    .and_then(|seq| rusty_asn1_der::DerReader::new(seq.content).read_tlv().ok())
                    .map(|tlv| {
                        tlv.tag == rusty_asn1_der::TAG_BOOLEAN && tlv.content.first() == Some(&0xff)
                    })
                    .unwrap_or(false)
            })
            .unwrap_or(false);
        rt.object_set(obj, "ca".into(), Value::Boolean(ca));

        rt.object_set(
            obj,
            "__tbs".into(),
            crypto_sval(&hex_encode(&cert.tbs_certificate)),
        );
        rt.object_set(
            obj,
            "__sig".into(),
            crypto_sval(&hex_encode(&cert.signature_value)),
        );
        rt.object_set(
            obj,
            "__sigalg".into(),
            crypto_sval(&cert.signature_algorithm.oid),
        );

        let san_names = cert.subject_alt_names().unwrap_or_default();
        let san_dns: Vec<String> = san_names
            .iter()
            .filter_map(|n| match n {
                rusty_x509::GeneralName::DnsName(d) => Some(d.clone()),
                _ => None,
            })
            .collect();

        if !san_names.is_empty() {
            let fmt_ip = |b: &[u8]| -> String {
                if b.len() == 4 {
                    format!("IP Address:{}.{}.{}.{}", b[0], b[1], b[2], b[3])
                } else {
                    let parts: Vec<String> = b
                        .chunks(2)
                        .map(|c| format!("{:x}", ((c[0] as u16) << 8) | c[1] as u16))
                        .collect();
                    format!("IP Address:{}", parts.join(":"))
                }
            };
            let san_str = san_names
                .iter()
                .map(|n| match n {
                    rusty_x509::GeneralName::DnsName(d) => format!("DNS:{d}"),
                    rusty_x509::GeneralName::IpAddress(ip) => fmt_ip(ip),
                    rusty_x509::GeneralName::Other { tag: 0x81, value } => {
                        format!("email:{}", String::from_utf8_lossy(value))
                    }
                    rusty_x509::GeneralName::Other { tag: 0x86, value } => {
                        format!("URI:{}", String::from_utf8_lossy(value))
                    }
                    rusty_x509::GeneralName::Other { tag, .. } => format!("othername:<{tag}>"),
                })
                .collect::<Vec<_>>()
                .join(", ");
            rt.object_set(obj, "subjectAltName".into(), crypto_sval(&san_str));
        }
        let cn = cert
            .subject
            .attributes
            .iter()
            .find(|(oid, _)| oid == "2.5.4.3")
            .map(|(_, v)| v.clone())
            .unwrap_or_default();
        rt.object_set(obj, "__san_dns".into(), crypto_sval(&san_dns.join("\n")));
        rt.object_set(obj, "__cn".into(), crypto_sval(&cn));

        let san_email: Vec<String> = san_names
            .iter()
            .filter_map(|n| match n {
                rusty_x509::GeneralName::Other { tag: 0x81, value } => {
                    Some(String::from_utf8_lossy(value).to_string())
                }
                _ => None,
            })
            .collect();
        rt.object_set(
            obj,
            "__san_email".into(),
            crypto_sval(&san_email.join("\n")),
        );

        rt.object_set(
            obj,
            "__issuer_der".into(),
            crypto_sval(&hex_encode(&cert.issuer.raw_der)),
        );
        rt.object_set(
            obj,
            "__subject_der".into(),
            crypto_sval(&hex_encode(&cert.subject.raw_der)),
        );

        if let Some(eku) = cert
            .extensions
            .iter()
            .find(|e| e.oid == rusty_x509::OID_EXTENDED_KEY_USAGE)
        {
            if let Ok(seq) = rusty_asn1_der::parse_single(&eku.value) {
                let mut rd = rusty_asn1_der::DerReader::new(seq.content);
                let mut oids: Vec<String> = Vec::new();
                while let Ok(tlv) = rd.read_tlv() {
                    if tlv.tag == rusty_asn1_der::TAG_OID {
                        if let Ok(o) = tlv.as_oid() {
                            oids.push(
                                o.iter()
                                    .map(|n| n.to_string())
                                    .collect::<Vec<_>>()
                                    .join("."),
                            );
                        }
                    }
                }
                if !oids.is_empty() {
                    let arr = rt.alloc_object(rusty_js_runtime::value::Object::new_array());
                    for (i, s) in oids.iter().enumerate() {
                        rt.object_set(arr, i.to_string(), crypto_sval(s));
                    }
                    rt.object_set(arr, "length".into(), Value::Number(oids.len() as f64));
                    rt.object_set(obj, "keyUsage".into(), Value::Object(arr));
                }
            }
        }

        register_method(rt, obj, "verify", |rt, args| {
            let this = match rt.current_this() {
                Value::Object(t) => t,
                _ => return Ok(Value::Boolean(false)),
            };
            let tbs = match rt.object_get(this, "__tbs") {
                Value::String(s) => hex_decode(s.as_str()),
                _ => return Ok(Value::Boolean(false)),
            };
            let sig = match rt.object_get(this, "__sig") {
                Value::String(s) => hex_decode(s.as_str()),
                _ => Vec::new(),
            };
            let sigalg = match rt.object_get(this, "__sigalg") {
                Value::String(s) => s.as_str().to_string(),
                _ => String::new(),
            };
            let key = args.first().cloned().unwrap_or(Value::Undefined);
            let pem = match key_as_pem(rt, &key) {
                Some(p) => p,
                None => return Ok(Value::Boolean(false)),
            };
            if key_alg_oid(&pem).as_deref() == Some(&ED25519_OID) {
                let pk = match parse_ed25519_pub(&pem) {
                    Some(p) => p,
                    None => return Ok(Value::Boolean(false)),
                };
                return Ok(Value::Boolean(rusty_web_crypto::ed25519_verify(
                    &pk, &tbs, &sig,
                )));
            }
            if let Some((n, e)) = parse_rsa_pub(&pem) {
                let dg = match sigalg.as_str() {
                    "1.2.840.113549.1.1.12" => "sha384",
                    "1.2.840.113549.1.1.13" => "sha512",
                    _ => "sha256",
                };
                let (h, hn) = sign_digest(dg, &tbs);
                return Ok(Value::Boolean(
                    rusty_web_crypto::rsa_pkcs1_v15_verify(&n, &e, &h, &sig, &hn).is_ok(),
                ));
            }
            if let Some((x, y)) = parse_ec_pub(&pem) {
                return Ok(Value::Boolean(match ecdsa_der_to_raw(&sig) {
                    Some(raw) => {
                        rusty_web_crypto::ecdsa_p256_sha256_verify(&x, &y, &tbs, &raw).is_ok()
                    }
                    None => false,
                }));
            }
            Ok(Value::Boolean(false))
        });
        register_method(rt, obj, "toString", |rt, _a| {
            Ok(rt.object_get(
                match rt.current_this() {
                    Value::Object(t) => t,
                    _ => return Ok(Value::Undefined),
                },
                "__pem",
            ))
        });
        register_method(rt, obj, "toLegacyObject", |rt, _a| Ok(rt.current_this()));
        register_method(rt, obj, "checkHost", |rt, args| {
            let this = match rt.current_this() {
                Value::Object(t) => t,
                _ => return Ok(Value::Undefined),
            };
            let name = value_string(args.first()).to_ascii_lowercase();
            if name.is_empty() {
                return Ok(Value::Undefined);
            }
            let san = match rt.object_get(this, "__san_dns") {
                Value::String(s) => s.as_str().to_string(),
                _ => String::new(),
            };
            let cn = match rt.object_get(this, "__cn") {
                Value::String(s) => s.as_str().to_string(),
                _ => String::new(),
            };

            let candidates: Vec<String> = if !san.is_empty() {
                san.lines().map(|l| l.to_string()).collect()
            } else if !cn.is_empty() {
                vec![cn]
            } else {
                Vec::new()
            };
            for pat in candidates {
                if host_matches(&pat.to_ascii_lowercase(), &name) {

                    return Ok(crypto_sval(&pat));
                }
            }
            Ok(Value::Undefined)
        });
        register_method(rt, obj, "checkEmail", |rt, args| {
            let this = match rt.current_this() {
                Value::Object(t) => t,
                _ => return Ok(Value::Undefined),
            };
            let email = value_string(args.first());
            if email.is_empty() {
                return Ok(Value::Undefined);
            }

            let stash = match rt.object_get(this, "__san_email") {
                Value::String(s) => s.as_str().to_string(),
                _ => String::new(),
            };
            for e in stash.lines() {
                if e == email {
                    return Ok(crypto_sval(e));
                }
            }
            Ok(Value::Undefined)
        });

        register_method(rt, obj, "checkIssued", |rt, args| {
            let this = match rt.current_this() {
                Value::Object(t) => t,
                _ => return Ok(Value::Boolean(false)),
            };
            let other = match args.first() {
                Some(Value::Object(o)) => *o,
                _ => return Ok(Value::Boolean(false)),
            };
            let my_issuer = match rt.object_get(this, "__issuer_der") {
                Value::String(s) => s.as_str().to_string(),
                _ => String::new(),
            };
            let other_subject = match rt.object_get(other, "__subject_der") {
                Value::String(s) => s.as_str().to_string(),
                _ => String::new(),
            };
            Ok(Value::Boolean(
                !my_issuer.is_empty() && my_issuer == other_subject,
            ))
        });
        Ok(Value::Object(obj))
    }

    fn dh_named_group(name: &str) -> Option<(&'static str, &'static str)> {
        match name {
        "modp1" => Some(("ffffffffffffffffc90fdaa22168c234c4c6628b80dc1cd129024e088a67cc74020bbea63b139b22514a08798e3404ddef9519b3cd3a431b302b0a6df25f14374fe1356d6d51c245e485b576625e7ec6f44c42e9a63a3620ffffffffffffffff", "02")),
        "modp2" => Some(("ffffffffffffffffc90fdaa22168c234c4c6628b80dc1cd129024e088a67cc74020bbea63b139b22514a08798e3404ddef9519b3cd3a431b302b0a6df25f14374fe1356d6d51c245e485b576625e7ec6f44c42e9a637ed6b0bff5cb6f406b7edee386bfb5a899fa5ae9f24117c4b1fe649286651ece65381ffffffffffffffff", "02")),
        "modp5" => Some(("ffffffffffffffffc90fdaa22168c234c4c6628b80dc1cd129024e088a67cc74020bbea63b139b22514a08798e3404ddef9519b3cd3a431b302b0a6df25f14374fe1356d6d51c245e485b576625e7ec6f44c42e9a637ed6b0bff5cb6f406b7edee386bfb5a899fa5ae9f24117c4b1fe649286651ece45b3dc2007cb8a163bf0598da48361c55d39a69163fa8fd24cf5f83655d23dca3ad961c62f356208552bb9ed529077096966d670c354e4abc9804f1746c08ca237327ffffffffffffffff", "02")),
        "modp14" => Some(("ffffffffffffffffc90fdaa22168c234c4c6628b80dc1cd129024e088a67cc74020bbea63b139b22514a08798e3404ddef9519b3cd3a431b302b0a6df25f14374fe1356d6d51c245e485b576625e7ec6f44c42e9a637ed6b0bff5cb6f406b7edee386bfb5a899fa5ae9f24117c4b1fe649286651ece45b3dc2007cb8a163bf0598da48361c55d39a69163fa8fd24cf5f83655d23dca3ad961c62f356208552bb9ed529077096966d670c354e4abc9804f1746c08ca18217c32905e462e36ce3be39e772c180e86039b2783a2ec07a28fb5c55df06f4c52c9de2bcbf6955817183995497cea956ae515d2261898fa051015728e5a8aacaa68ffffffffffffffff", "02")),
        "modp15" => Some(("ffffffffffffffffc90fdaa22168c234c4c6628b80dc1cd129024e088a67cc74020bbea63b139b22514a08798e3404ddef9519b3cd3a431b302b0a6df25f14374fe1356d6d51c245e485b576625e7ec6f44c42e9a637ed6b0bff5cb6f406b7edee386bfb5a899fa5ae9f24117c4b1fe649286651ece45b3dc2007cb8a163bf0598da48361c55d39a69163fa8fd24cf5f83655d23dca3ad961c62f356208552bb9ed529077096966d670c354e4abc9804f1746c08ca18217c32905e462e36ce3be39e772c180e86039b2783a2ec07a28fb5c55df06f4c52c9de2bcbf6955817183995497cea956ae515d2261898fa051015728e5a8aaac42dad33170d04507a33a85521abdf1cba64ecfb850458dbef0a8aea71575d060c7db3970f85a6e1e4c7abf5ae8cdb0933d71e8c94e04a25619dcee3d2261ad2ee6bf12ffa06d98a0864d87602733ec86a64521f2b18177b200cbbe117577a615d6c770988c0bad946e208e24fa074e5ab3143db5bfce0fd108e4b82d120a93ad2caffffffffffffffff", "02")),
        "modp16" => Some(("ffffffffffffffffc90fdaa22168c234c4c6628b80dc1cd129024e088a67cc74020bbea63b139b22514a08798e3404ddef9519b3cd3a431b302b0a6df25f14374fe1356d6d51c245e485b576625e7ec6f44c42e9a637ed6b0bff5cb6f406b7edee386bfb5a899fa5ae9f24117c4b1fe649286651ece45b3dc2007cb8a163bf0598da48361c55d39a69163fa8fd24cf5f83655d23dca3ad961c62f356208552bb9ed529077096966d670c354e4abc9804f1746c08ca18217c32905e462e36ce3be39e772c180e86039b2783a2ec07a28fb5c55df06f4c52c9de2bcbf6955817183995497cea956ae515d2261898fa051015728e5a8aaac42dad33170d04507a33a85521abdf1cba64ecfb850458dbef0a8aea71575d060c7db3970f85a6e1e4c7abf5ae8cdb0933d71e8c94e04a25619dcee3d2261ad2ee6bf12ffa06d98a0864d87602733ec86a64521f2b18177b200cbbe117577a615d6c770988c0bad946e208e24fa074e5ab3143db5bfce0fd108e4b82d120a92108011a723c12a787e6d788719a10bdba5b2699c327186af4e23c1a946834b6150bda2583e9ca2ad44ce8dbbbc2db04de8ef92e8efc141fbecaa6287c59474e6bc05d99b2964fa090c3a2233ba186515be7ed1f612970cee2d7afb81bdd762170481cd0069127d5b05aa993b4ea988d8fddc186ffb7dc90a6c08f4df435c934063199ffffffffffffffff", "02")),
        "modp17" => Some(("ffffffffffffffffc90fdaa22168c234c4c6628b80dc1cd129024e088a67cc74020bbea63b139b22514a08798e3404ddef9519b3cd3a431b302b0a6df25f14374fe1356d6d51c245e485b576625e7ec6f44c42e9a637ed6b0bff5cb6f406b7edee386bfb5a899fa5ae9f24117c4b1fe649286651ece45b3dc2007cb8a163bf0598da48361c55d39a69163fa8fd24cf5f83655d23dca3ad961c62f356208552bb9ed529077096966d670c354e4abc9804f1746c08ca18217c32905e462e36ce3be39e772c180e86039b2783a2ec07a28fb5c55df06f4c52c9de2bcbf6955817183995497cea956ae515d2261898fa051015728e5a8aaac42dad33170d04507a33a85521abdf1cba64ecfb850458dbef0a8aea71575d060c7db3970f85a6e1e4c7abf5ae8cdb0933d71e8c94e04a25619dcee3d2261ad2ee6bf12ffa06d98a0864d87602733ec86a64521f2b18177b200cbbe117577a615d6c770988c0bad946e208e24fa074e5ab3143db5bfce0fd108e4b82d120a92108011a723c12a787e6d788719a10bdba5b2699c327186af4e23c1a946834b6150bda2583e9ca2ad44ce8dbbbc2db04de8ef92e8efc141fbecaa6287c59474e6bc05d99b2964fa090c3a2233ba186515be7ed1f612970cee2d7afb81bdd762170481cd0069127d5b05aa993b4ea988d8fddc186ffb7dc90a6c08f4df435c93402849236c3fab4d27c7026c1d4dcb2602646dec9751e763dba37bdf8ff9406ad9e530ee5db382f413001aeb06a53ed9027d831179727b0865a8918da3edbebcf9b14ed44ce6cbaced4bb1bdb7f1447e6cc254b332051512bd7af426fb8f401378cd2bf5983ca01c64b92ecf032ea15d1721d03f482d7ce6e74fef6d55e702f46980c82b5a84031900b1c9e59e7c97fbec7e8f323a97a7e36cc88be0f1d45b7ff585ac54bd407b22b4154aacc8f6d7ebf48e1d814cc5ed20f8037e0a79715eef29be32806a1d58bb7c5da76f550aa3d8a1fbff0eb19ccb1a313d55cda56c9ec2ef29632387fe8d76e3c0468043e8f663f4860ee12bf2d5b0b7474d6e694f91e6dcc4024ffffffffffffffff", "02")),
        "modp18" => Some(("ffffffffffffffffc90fdaa22168c234c4c6628b80dc1cd129024e088a67cc74020bbea63b139b22514a08798e3404ddef9519b3cd3a431b302b0a6df25f14374fe1356d6d51c245e485b576625e7ec6f44c42e9a637ed6b0bff5cb6f406b7edee386bfb5a899fa5ae9f24117c4b1fe649286651ece45b3dc2007cb8a163bf0598da48361c55d39a69163fa8fd24cf5f83655d23dca3ad961c62f356208552bb9ed529077096966d670c354e4abc9804f1746c08ca18217c32905e462e36ce3be39e772c180e86039b2783a2ec07a28fb5c55df06f4c52c9de2bcbf6955817183995497cea956ae515d2261898fa051015728e5a8aaac42dad33170d04507a33a85521abdf1cba64ecfb850458dbef0a8aea71575d060c7db3970f85a6e1e4c7abf5ae8cdb0933d71e8c94e04a25619dcee3d2261ad2ee6bf12ffa06d98a0864d87602733ec86a64521f2b18177b200cbbe117577a615d6c770988c0bad946e208e24fa074e5ab3143db5bfce0fd108e4b82d120a92108011a723c12a787e6d788719a10bdba5b2699c327186af4e23c1a946834b6150bda2583e9ca2ad44ce8dbbbc2db04de8ef92e8efc141fbecaa6287c59474e6bc05d99b2964fa090c3a2233ba186515be7ed1f612970cee2d7afb81bdd762170481cd0069127d5b05aa993b4ea988d8fddc186ffb7dc90a6c08f4df435c93402849236c3fab4d27c7026c1d4dcb2602646dec9751e763dba37bdf8ff9406ad9e530ee5db382f413001aeb06a53ed9027d831179727b0865a8918da3edbebcf9b14ed44ce6cbaced4bb1bdb7f1447e6cc254b332051512bd7af426fb8f401378cd2bf5983ca01c64b92ecf032ea15d1721d03f482d7ce6e74fef6d55e702f46980c82b5a84031900b1c9e59e7c97fbec7e8f323a97a7e36cc88be0f1d45b7ff585ac54bd407b22b4154aacc8f6d7ebf48e1d814cc5ed20f8037e0a79715eef29be32806a1d58bb7c5da76f550aa3d8a1fbff0eb19ccb1a313d55cda56c9ec2ef29632387fe8d76e3c0468043e8f663f4860ee12bf2d5b0b7474d6e694f91e6dbe115974a3926f12fee5e438777cb6a932df8cd8bec4d073b931ba3bc832b68d9dd300741fa7bf8afc47ed2576f6936ba424663aab639c5ae4f5683423b4742bf1c978238f16cbe39d652de3fdb8befc848ad922222e04a4037c0713eb57a81a23f0c73473fc646cea306b4bcbc8862f8385ddfa9d4b7fa2c087e879683303ed5bdd3a062b3cf5b3a278a66d2a13f83f44f82ddf310ee074ab6a364597e899a0255dc164f31cc50846851df9ab48195ded7ea1b1d510bd7ee74d73faf36bc31ecfa268359046f4eb879f924009438b481c6cd7889a002ed5ee382bc9190da6fc026e479558e4475677e9aa9e3050e2765694dfc81f56e880b96e7160c980dd98edd3dfffffffffffffffff", "02")),
        _ => None,
    }
    }
    fn dh_mod_pow(base: &[u8], exp: &[u8], modulus: &[u8]) -> Vec<u8> {
        let b = rusty_web_crypto::BigUInt::from_be_bytes(base);
        let e = rusty_web_crypto::BigUInt::from_be_bytes(exp);
        let m = rusty_web_crypto::BigUInt::from_be_bytes(modulus);
        rusty_web_crypto::mod_pow_mont(&b, &e, &m).to_be_bytes(modulus.len())
    }
    fn dh_get(rt: &Runtime, obj: rusty_js_runtime::value::ObjectRef, key: &str) -> Vec<u8> {
        match rt.object_get(obj, key) {
            Value::String(s) => hex_decode(s.as_str()),
            _ => Vec::new(),
        }
    }

    fn dh_make(
        rt: &mut Runtime,
        prime: Vec<u8>,
        gen: Vec<u8>,
    ) -> rusty_js_runtime::value::ObjectRef {
        let obj = new_object(rt);
        rt.object_set(obj, "__dh_p".into(), crypto_sval(&hex_encode(&prime)));
        rt.object_set(obj, "__dh_g".into(), crypto_sval(&hex_encode(&gen)));

        rt.object_set(obj, "verifyError".into(), Value::Number(0.0));
        register_method(rt, obj, "generateKeys", |rt, args| {
            let this = match rt.current_this() {
                Value::Object(t) => t,
                _ => return Ok(Value::Undefined),
            };
            let p = dh_get(rt, this, "__dh_p");
            let g = dh_get(rt, this, "__dh_g");
            let mut priv_b = vec![0u8; p.len()];
            let _ = rusty_web_crypto::get_random_values(&mut priv_b);

            priv_b[0] &= 0x7f;
            rt.object_set(this, "__dh_priv".into(), crypto_sval(&hex_encode(&priv_b)));
            let pubk = dh_mod_pow(&g, &priv_b, &p);
            let enc = args.first().and_then(|v| {
                if let Value::String(s) = v {
                    Some(s.as_str().to_string())
                } else {
                    None
                }
            });
            Ok(ecdh_output(rt, &pubk, enc.as_deref()))
        });
        register_method(rt, obj, "getPublicKey", |rt, args| {
            let this = match rt.current_this() {
                Value::Object(t) => t,
                _ => return Ok(Value::Undefined),
            };
            let (p, g, pr) = (
                dh_get(rt, this, "__dh_p"),
                dh_get(rt, this, "__dh_g"),
                dh_get(rt, this, "__dh_priv"),
            );
            let pubk = dh_mod_pow(&g, &pr, &p);
            let enc = args.first().and_then(|v| {
                if let Value::String(s) = v {
                    Some(s.as_str().to_string())
                } else {
                    None
                }
            });
            Ok(ecdh_output(rt, &pubk, enc.as_deref()))
        });
        register_method(rt, obj, "getPrivateKey", |rt, args| {
            let this = match rt.current_this() {
                Value::Object(t) => t,
                _ => return Ok(Value::Undefined),
            };
            let pr = dh_get(rt, this, "__dh_priv");
            let enc = args.first().and_then(|v| {
                if let Value::String(s) = v {
                    Some(s.as_str().to_string())
                } else {
                    None
                }
            });
            Ok(ecdh_output(rt, &pr, enc.as_deref()))
        });
        register_method(rt, obj, "setPrivateKey", |rt, args| {
            let this = match rt.current_this() {
                Value::Object(t) => t,
                _ => return Ok(Value::Undefined),
            };
            let enc = args.get(1).and_then(|v| {
                if let Value::String(s) = v {
                    Some(s.as_str().to_string())
                } else {
                    None
                }
            });
            let pr = ecdh_input(
                rt,
                &args.first().cloned().unwrap_or(Value::Undefined),
                enc.as_deref(),
            );
            rt.object_set(this, "__dh_priv".into(), crypto_sval(&hex_encode(&pr)));
            Ok(rt.current_this())
        });
        register_method(rt, obj, "computeSecret", |rt, args| {
            let this = match rt.current_this() {
                Value::Object(t) => t,
                _ => return Ok(Value::Undefined),
            };
            let other_is_str = matches!(args.first(), Some(Value::String(_)));
            let in_enc = if other_is_str {
                args.get(1).and_then(|v| {
                    if let Value::String(s) = v {
                        Some(s.as_str().to_string())
                    } else {
                        None
                    }
                })
            } else {
                None
            };
            let out_enc = if other_is_str {
                args.get(2)
            } else {
                args.get(1)
            }
            .and_then(|v| {
                if let Value::String(s) = v {
                    Some(s.as_str().to_string())
                } else {
                    None
                }
            });
            let other = ecdh_input(
                rt,
                &args.first().cloned().unwrap_or(Value::Undefined),
                in_enc.as_deref(),
            );
            let (p, pr) = (dh_get(rt, this, "__dh_p"), dh_get(rt, this, "__dh_priv"));
            let secret = dh_mod_pow(&other, &pr, &p);
            Ok(ecdh_output(rt, &secret, out_enc.as_deref()))
        });
        register_method(rt, obj, "getPrime", |rt, args| {
            let this = match rt.current_this() {
                Value::Object(t) => t,
                _ => return Ok(Value::Undefined),
            };
            let p = dh_get(rt, this, "__dh_p");
            let enc = args.first().and_then(|v| {
                if let Value::String(s) = v {
                    Some(s.as_str().to_string())
                } else {
                    None
                }
            });
            Ok(ecdh_output(rt, &p, enc.as_deref()))
        });
        register_method(rt, obj, "getGenerator", |rt, args| {
            let this = match rt.current_this() {
                Value::Object(t) => t,
                _ => return Ok(Value::Undefined),
            };
            let g = dh_get(rt, this, "__dh_g");
            let enc = args.first().and_then(|v| {
                if let Value::String(s) = v {
                    Some(s.as_str().to_string())
                } else {
                    None
                }
            });
            Ok(ecdh_output(rt, &g, enc.as_deref()))
        });
        register_method(rt, obj, "setPublicKey", |rt, _a| Ok(rt.current_this()));
        obj
    }

    fn hex_decode(s: &str) -> Vec<u8> {
        let s: String = s.chars().filter(|c| !c.is_whitespace()).collect();
        (0..s.len() / 2)
            .filter_map(|i| u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).ok())
            .collect()
    }
    fn p256_order() -> rusty_web_crypto::BigUInt {
        rusty_web_crypto::BigUInt::from_be_bytes(&[
            0xff, 0xff, 0xff, 0xff, 0x00, 0x00, 0x00, 0x00, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
            0xff, 0xff, 0xbc, 0xe6, 0xfa, 0xad, 0xa7, 0x17, 0x9e, 0x84, 0xf3, 0xb9, 0xca, 0xc2,
            0xfc, 0x63, 0x25, 0x51,
        ])
    }
    fn ecdh_gen_scalar() -> Vec<u8> {
        use std::cmp::Ordering;
        let n = p256_order();
        let zero = rusty_web_crypto::BigUInt::from_be_bytes(&[0]);
        loop {
            let mut bytes = vec![0u8; 32];
            let _ = rusty_web_crypto::get_random_values(&mut bytes);
            let d = rusty_web_crypto::BigUInt::from_be_bytes(&bytes);
            if d.cmp(&zero) == Ordering::Greater && d.cmp(&n) == Ordering::Less {
                return bytes;
            }
        }
    }

    fn ecdh_pubkey(d_bytes: &[u8]) -> Vec<u8> {
        let d = rusty_web_crypto::BigUInt::from_be_bytes(d_bytes);
        match rusty_web_crypto::p256_scalar_mul_base_solinas(&d) {
            rusty_web_crypto::P256Point::Affine { x, y } => {
                let mut out = vec![0x04u8];
                out.extend_from_slice(&x.to_be_bytes(32));
                out.extend_from_slice(&y.to_be_bytes(32));
                out
            }
            _ => Vec::new(),
        }
    }

    fn ecdh_input(rt: &mut Runtime, v: &Value, enc: Option<&str>) -> Vec<u8> {
        match v {

            Value::String(s) => match enc {
                Some("base64") => b64_to_bytes(s.as_str()),
                Some("base64url") => b64_to_bytes(&s.as_str().replace('-', "+").replace('_', "/")),
                Some("latin1") | Some("binary") => s.as_str().chars().map(|c| c as u8).collect(),
                Some("utf8") | Some("utf-8") => s.as_str().as_bytes().to_vec(),
                _ => hex_decode(s.as_str()),
            },
            _ => extract_bytes(rt, v),
        }
    }

    fn ecdh_output(rt: &mut Runtime, bytes: &[u8], enc: Option<&str>) -> Value {
        match enc {
            Some("hex") => Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                hex_encode(bytes),
            ))),
            Some("base64") => Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                base64_encode(bytes),
            ))),
            Some("latin1") | Some("binary") => {
                Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                    bytes.iter().map(|&b| b as char).collect::<String>(),
                )))
            }
            _ => make_buffer(rt, bytes),
        }
    }
    fn ecdh_d_of(rt: &Runtime, obj: rusty_js_runtime::value::ObjectRef) -> Vec<u8> {
        match rt.object_get(obj, "__ecdh_d") {
            Value::String(s) => hex_decode(s.as_str()),
            _ => Vec::new(),
        }
    }

    fn crypto_sval(s: &str) -> Value {
        Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
            s.to_string(),
        )))
    }

    fn build_secret_key(rt: &mut Runtime, bytes: &[u8]) -> ObjectRef {
        let ko = new_object(rt);

        if let Value::Object(p) = rt.global_get("__cruft_SecretKeyObject_proto") {
            rt.obj_mut(ko).proto = Some(p);
        }

        rt.obj_mut(ko)
            .set_own_internal("__cruft_key_object".into(), Value::Boolean(true));

        rt.obj_mut(ko)
            .set_own_internal("type".into(), crypto_sval("secret"));
        rt.obj_mut(ko)
            .set_own_internal("__keybytes".into(), crypto_sval(&hex_encode(bytes)));
        rt.obj_mut(ko)
            .set_own_internal("symmetricKeySize".into(), Value::Number(bytes.len() as f64));
        register_method_internal(rt, ko, "export", |rt, _a| {
            let this = match rt.current_this() {
                Value::Object(t) => t,
                _ => return Ok(Value::Undefined),
            };
            let b = match rt.object_get(this, "__keybytes") {
                Value::String(s) => hex_decode(s.as_str()),
                _ => Vec::new(),
            };
            Ok(make_buffer(rt, &b))
        });
        ko
    }

    for (async_name, sync_name) in [
        ("pbkdf2", "pbkdf2Sync"),
        ("scrypt", "scryptSync"),
        ("randomFill", "randomFillSync"),
        ("hkdf", "hkdfSync"),
        ("generateKey", "generateKeySync"),
    ] {
        let sname = sync_name.to_string();
        register_method(rt, crypto, async_name, move |rt, args| {
            let cb = args.iter().rev().find(|v| rt.is_callable(v)).cloned();
            let pass: Vec<Value> = args
                .iter()
                .filter(|v| !rt.is_callable(v))
                .cloned()
                .collect();
            let cg = match rt.global_get("crypto") {
                Value::Object(i) => i,
                _ => return Ok(Value::Undefined),
            };
            let sync_fn = rt.object_get(cg, &sname);
            if !rt.is_callable(&sync_fn) {
                if let Some(cb) = cb {
                    let err = crypto_sval(&format!("crypto.{sname}: not implemented"));
                    let cb_args = vec![err];
                    let roots = crate::timer::roots_for_callback(&cb, &cb_args);
                    rt.enqueue_host_phase_rooted(
                        HostEnqueuePhase::HostCompletionMacrotask,
                        "crypto async callback",
                        roots,
                        move |rt| {
                            let _ = rt.call_function(cb, Value::Undefined, cb_args);
                            Ok(())
                        },
                    );
                }
                return Ok(Value::Undefined);
            }
            let mut roots = vec![Value::Object(cg), sync_fn.clone()];
            roots.extend(pass.iter().cloned());
            if let Some(cb) = &cb {
                roots.push(cb.clone());
            }
            let _sync_roots = rt.push_temporary_value_roots(&roots);
            match rt.call_function(sync_fn, Value::Object(cg), pass) {
                Ok(v) => {
                    if let Some(cb) = cb {
                        let cb_args = vec![Value::Null, v];
                        let roots = crate::timer::roots_for_callback(&cb, &cb_args);
                        rt.enqueue_host_phase_rooted(
                            HostEnqueuePhase::HostCompletionMacrotask,
                            "crypto async callback",
                            roots,
                            move |rt| {
                                let _ = rt.call_function(cb, Value::Undefined, cb_args);
                                Ok(())
                            },
                        );
                    }
                }
                Err(e) => {
                    if let Some(cb) = cb {
                        let err = crypto_sval(&format!("{e:?}"));
                        let cb_args = vec![err];
                        let roots = crate::timer::roots_for_callback(&cb, &cb_args);
                        rt.enqueue_host_phase_rooted(
                            HostEnqueuePhase::HostCompletionMacrotask,
                            "crypto async callback",
                            roots,
                            move |rt| {
                                let _ = rt.call_function(cb, Value::Undefined, cb_args);
                                Ok(())
                            },
                        );
                    }
                }
            }
            Ok(Value::Undefined)
        });
    }

    if !rt.is_callable(&rt.object_get(crypto, "hkdfSync")) {
        register_method(rt, crypto, "hkdfSync", |rt, args| {
            let digest = value_string(args.first());
            let ikm = extract_bytes(rt, &args.get(1).cloned().unwrap_or(Value::Undefined));
            let salt = extract_bytes(rt, &args.get(2).cloned().unwrap_or(Value::Undefined));
            let info = extract_bytes(rt, &args.get(3).cloned().unwrap_or(Value::Undefined));
            let keylen = match args.get(4) {
                Some(Value::Number(n)) if n.is_finite() && *n >= 0.0 => *n as usize,
                Some(v) => rusty_js_runtime::abstract_ops::to_number(v) as usize,
                None => return Err(crypto_error("node:crypto hkdfSync: keylen is required")),
            };

            let hlen = match normalize_digest(&digest).as_str() {
                "sha1" => 20usize,
                "sha256" => 32,
                "sha384" => 48,
                "sha512" => 64,
                _ => 0,
            };
            if hlen > 0 && keylen > 255 * hlen {
                return Err(coded_crypto_error(
                    rt,
                    "RangeError",
                    "ERR_CRYPTO_INVALID_KEYLEN",
                    "Invalid key length",
                ));
            }
            let out = match normalize_digest(&digest).as_str() {
                "sha1" => rusty_web_crypto::hkdf_sha1(&ikm, &salt, &info, keylen),
                "sha256" => rusty_web_crypto::hkdf_sha256(&ikm, &salt, &info, keylen),
                "sha384" => rusty_web_crypto::hkdf_sha384(&ikm, &salt, &info, keylen),
                "sha512" => rusty_web_crypto::hkdf_sha512(&ikm, &salt, &info, keylen),
                _ => Err("unsupported digest".into()),
            }
            .map_err(|e| crypto_error(format!("node:crypto hkdfSync: {e}")))?;
            Ok(make_buffer(rt, &out))
        });
    }

    register_method(rt, crypto, "hash", |rt, args| {
        let cg = match rt.global_get("crypto") {
            Value::Object(i) => i,
            _ => return Ok(Value::Undefined),
        };
        let ch = rt.object_get(cg, "createHash");
        let algo = args.first().cloned().unwrap_or(Value::Undefined);
        let h = rt.call_function(ch, Value::Object(cg), vec![algo])?;
        if let Value::Object(hid) = h {
            let upd = rt.object_get(hid, "update");
            let data = args.get(1).cloned().unwrap_or(Value::Undefined);
            rt.call_function(upd, Value::Object(hid), vec![data])?;
            let dig = rt.object_get(hid, "digest");
            let enc = args.get(2).cloned().unwrap_or_else(|| crypto_sval("hex"));
            return rt.call_function(dig, Value::Object(hid), vec![enc]);
        }
        Ok(Value::Undefined)
    });

    register_method(rt, crypto, "randomInt", |rt, args| {
        let nums: Vec<f64> = args
            .iter()
            .filter_map(|v| {
                if let Value::Number(n) = v {
                    Some(*n)
                } else {
                    None
                }
            })
            .collect();
        let (min, max) = if nums.len() >= 2 {
            (nums[0], nums[1])
        } else {
            (0.0, *nums.first().unwrap_or(&0.0))
        };

        if max <= min {
            let fmt = |n: f64| {
                if n.fract() == 0.0 {
                    (n as i64).to_string()
                } else {
                    n.to_string()
                }
            };
            let msg = format!(
                "The value of \"max\" is out of range. It must be greater than the value of \"min\" ({}). Received {}",
                fmt(min),
                fmt(max)
            );
            return Err(coded_crypto_error(
                rt,
                "RangeError",
                "ERR_OUT_OF_RANGE",
                &msg,
            ));
        }
        let range = (max - min).max(1.0);
        let cg = match rt.global_get("crypto") {
            Value::Object(i) => i,
            _ => return Ok(Value::Undefined),
        };
        let rb = rt.object_get(cg, "randomBytes");
        let _random_roots = rt.push_temporary_value_roots(&[Value::Object(cg), rb.clone()]);
        let buf = rt.call_function(rb, Value::Object(cg), vec![Value::Number(6.0)])?;
        let _buf_root = rt.push_temporary_value_roots(&[buf.clone()]);
        let mut acc = 0f64;
        if let Value::Object(bid) = buf {
            for i in 0..6 {
                if let Value::Number(b) = rt.object_get(bid, &i.to_string()) {
                    acc = acc * 256.0 + b;
                }
            }
        }
        let v = min + (acc % range).floor();
        let cb = args.iter().rev().find(|v| rt.is_callable(v)).cloned();
        if let Some(cb) = cb {
            let result = Value::Number(v);
            let _call_roots = rt.push_temporary_value_roots(&[cb.clone(), result.clone()]);
            let _ = rt.call_function(cb, Value::Undefined, vec![Value::Null, result]);
            return Ok(Value::Undefined);
        }
        Ok(Value::Number(v))
    });

    {
        let u = rt.object_get(crypto, "randomUUID");
        if rt.is_callable(&u) {
            set_constant(rt, crypto, "randomUUIDv7", u);
        }
    }

    register_method(rt, crypto, "getFips", |_rt, _a| Ok(Value::Number(0.0)));
    register_method(rt, crypto, "setFips", |_rt, _a| Ok(Value::Undefined));
    register_method(rt, crypto, "setEngine", |_rt, _a| Ok(Value::Undefined));
    register_method(rt, crypto, "secureHeapUsed", |rt, _a| {
        let o = new_object(rt);
        for k in ["total", "min", "used"] {
            rt.object_set(o, k.to_string(), Value::Number(0.0));
        }
        rt.object_set(o, "utilization".into(), Value::Number(0.0));
        Ok(Value::Object(o))
    });
    register_method(rt, crypto, "getCipherInfo", |rt, args| {

        #[allow(clippy::type_complexity)]
        const T: &[(&str, &str, u32, &str, Option<u32>, u32, Option<u32>)] = &[
            (
                "aes-128-cbc",
                "aes-128-cbc",
                419,
                "cbc",
                Some(16),
                16,
                Some(16),
            ),
            (
                "aes-192-cbc",
                "aes-192-cbc",
                423,
                "cbc",
                Some(16),
                24,
                Some(16),
            ),
            (
                "aes-256-cbc",
                "aes-256-cbc",
                427,
                "cbc",
                Some(16),
                32,
                Some(16),
            ),
            (
                "aes-128-ctr",
                "aes-128-ctr",
                904,
                "ctr",
                Some(1),
                16,
                Some(16),
            ),
            (
                "aes-192-ctr",
                "aes-192-ctr",
                905,
                "ctr",
                Some(1),
                24,
                Some(16),
            ),
            (
                "aes-256-ctr",
                "aes-256-ctr",
                906,
                "ctr",
                Some(1),
                32,
                Some(16),
            ),
            (
                "aes-128-gcm",
                "id-aes128-gcm",
                895,
                "gcm",
                Some(1),
                16,
                Some(12),
            ),
            (
                "aes-192-gcm",
                "id-aes192-gcm",
                898,
                "gcm",
                Some(1),
                24,
                Some(12),
            ),
            (
                "aes-256-gcm",
                "id-aes256-gcm",
                901,
                "gcm",
                Some(1),
                32,
                Some(12),
            ),
            ("aes-128-ecb", "aes-128-ecb", 418, "ecb", Some(16), 16, None),
            ("aes-192-ecb", "aes-192-ecb", 422, "ecb", Some(16), 24, None),
            ("aes-256-ecb", "aes-256-ecb", 426, "ecb", Some(16), 32, None),
            (
                "aes-128-cfb",
                "aes-128-cfb",
                421,
                "cfb",
                Some(1),
                16,
                Some(16),
            ),
            (
                "aes-192-cfb",
                "aes-192-cfb",
                425,
                "cfb",
                Some(1),
                24,
                Some(16),
            ),
            (
                "aes-256-cfb",
                "aes-256-cfb",
                429,
                "cfb",
                Some(1),
                32,
                Some(16),
            ),
            (
                "aes-128-ofb",
                "aes-128-ofb",
                420,
                "ofb",
                Some(1),
                16,
                Some(16),
            ),
            (
                "aes-192-ofb",
                "aes-192-ofb",
                424,
                "ofb",
                Some(1),
                24,
                Some(16),
            ),
            (
                "aes-256-ofb",
                "aes-256-ofb",
                428,
                "ofb",
                Some(1),
                32,
                Some(16),
            ),
            (
                "chacha20-poly1305",
                "chacha20-poly1305",
                1018,
                "stream",
                None,
                32,
                Some(12),
            ),
            ("chacha20", "chacha20", 1019, "stream", None, 32, Some(16)),
        ];
        let found = match args.first() {
            Some(Value::String(s)) => {
                let n = s.as_str().to_ascii_lowercase();
                T.iter().find(|e| e.0 == n)
            }
            Some(Value::Number(nid)) => {
                let nid = *nid as u32;
                T.iter().find(|e| e.2 == nid)
            }
            _ => None,
        };
        let e = match found {
            Some(e) => e,
            None => return Ok(Value::Undefined),
        };
        let s = |txt: &str| Value::String(Rc::new(rusty_js_runtime::value::JsString::from(txt)));
        let o = new_object(rt);

        rt.object_set(o, "mode".into(), s(e.3));
        rt.object_set(o, "name".into(), s(e.1));
        rt.object_set(o, "nid".into(), Value::Number(e.2 as f64));
        rt.object_set(o, "keyLength".into(), Value::Number(e.5 as f64));
        if let Some(bs) = e.4 {
            rt.object_set(o, "blockSize".into(), Value::Number(bs as f64));
        }
        if let Some(iv) = e.6 {
            rt.object_set(o, "ivLength".into(), Value::Number(iv as f64));
        }
        Ok(Value::Object(o))
    });

    {
        let c = new_object(rt);
        for (k, v) in [
            ("RSA_PKCS1_PADDING", 1.0),
            ("RSA_NO_PADDING", 3.0),
            ("RSA_PKCS1_OAEP_PADDING", 4.0),
            ("RSA_X931_PADDING", 5.0),
            ("RSA_PKCS1_PSS_PADDING", 6.0),
            ("RSA_PSS_SALTLEN_DIGEST", -1.0),
            ("RSA_PSS_SALTLEN_MAX_SIGN", -2.0),
            ("RSA_PSS_SALTLEN_AUTO", -2.0),
            ("POINT_CONVERSION_COMPRESSED", 2.0),
            ("POINT_CONVERSION_UNCOMPRESSED", 4.0),
            ("POINT_CONVERSION_HYBRID", 6.0),

            ("SSL_OP_ALL", 2147485776.0),
            ("SSL_OP_ALLOW_NO_DHE_KEX", 1024.0),
            ("SSL_OP_ALLOW_UNSAFE_LEGACY_RENEGOTIATION", 262144.0),
            ("SSL_OP_CIPHER_SERVER_PREFERENCE", 4194304.0),
            ("SSL_OP_CISCO_ANYCONNECT", 32768.0),
            ("SSL_OP_COOKIE_EXCHANGE", 8192.0),
            ("SSL_OP_CRYPTOPRO_TLSEXT_BUG", 2147483648.0),
            ("SSL_OP_DONT_INSERT_EMPTY_FRAGMENTS", 2048.0),
            ("SSL_OP_LEGACY_SERVER_CONNECT", 4.0),
            ("SSL_OP_NO_COMPRESSION", 131072.0),
            ("SSL_OP_NO_ENCRYPT_THEN_MAC", 524288.0),
            ("SSL_OP_NO_QUERY_MTU", 4096.0),
            ("SSL_OP_NO_RENEGOTIATION", 1073741824.0),
            ("SSL_OP_NO_SESSION_RESUMPTION_ON_RENEGOTIATION", 65536.0),
            ("SSL_OP_NO_SSLv2", 0.0),
            ("SSL_OP_NO_SSLv3", 33554432.0),
            ("SSL_OP_NO_TICKET", 16384.0),
            ("SSL_OP_NO_TLSv1", 67108864.0),
            ("SSL_OP_NO_TLSv1_1", 268435456.0),
            ("SSL_OP_NO_TLSv1_2", 134217728.0),
            ("SSL_OP_NO_TLSv1_3", 536870912.0),
            ("SSL_OP_PRIORITIZE_CHACHA", 2097152.0),
            ("SSL_OP_TLS_ROLLBACK_BUG", 8388608.0),

            ("ENGINE_METHOD_RSA", 1.0),
            ("ENGINE_METHOD_DSA", 2.0),
            ("ENGINE_METHOD_DH", 4.0),
            ("ENGINE_METHOD_RAND", 8.0),
            ("ENGINE_METHOD_EC", 2048.0),
            ("ENGINE_METHOD_CIPHERS", 64.0),
            ("ENGINE_METHOD_DIGESTS", 128.0),
            ("ENGINE_METHOD_PKEY_METHS", 512.0),
            ("ENGINE_METHOD_PKEY_ASN1_METHS", 1024.0),
            ("ENGINE_METHOD_ALL", 65535.0),
            ("ENGINE_METHOD_NONE", 0.0),

            ("DH_CHECK_P_NOT_SAFE_PRIME", 2.0),
            ("DH_CHECK_P_NOT_PRIME", 1.0),
            ("DH_UNABLE_TO_CHECK_GENERATOR", 4.0),
            ("DH_NOT_SUITABLE_GENERATOR", 8.0),

            ("TLS1_VERSION", 769.0),
            ("TLS1_1_VERSION", 770.0),
            ("TLS1_2_VERSION", 771.0),
            ("TLS1_3_VERSION", 772.0),
        ] {
            rt.object_set(c, k.to_string(), Value::Number(v));
        }

        let cipher_list = "TLS_AES_256_GCM_SHA384:TLS_CHACHA20_POLY1305_SHA256:TLS_AES_128_GCM_SHA256:ECDHE-RSA-AES128-GCM-SHA256:ECDHE-ECDSA-AES128-GCM-SHA256:ECDHE-RSA-AES256-GCM-SHA384:ECDHE-ECDSA-AES256-GCM-SHA384:DHE-RSA-AES128-GCM-SHA256:ECDHE-RSA-AES128-SHA256:DHE-RSA-AES128-SHA256:ECDHE-RSA-AES256-SHA384:DHE-RSA-AES256-SHA384:ECDHE-RSA-AES256-SHA256:DHE-RSA-AES256-SHA256:HIGH:!aNULL:!eNULL:!EXPORT:!DES:!RC4:!MD5:!PSK:!SRP:!CAMELLIA";
        for key in ["defaultCoreCipherList", "defaultCipherList"] {
            rt.object_set(
                c,
                key.to_string(),
                Value::String(Rc::new(rusty_js_runtime::value::JsString::from(
                    cipher_list,
                ))),
            );
        }
        set_constant(rt, crypto, "constants", Value::Object(c));
    }

    register_method(rt, crypto, "generateKeyPair", |rt, args| {
        let cb = args.iter().rev().find(|v| rt.is_callable(v)).cloned();
        let pass: Vec<Value> = args
            .iter()
            .filter(|v| !rt.is_callable(v))
            .cloned()
            .collect();
        let cg = match rt.global_get("crypto") {
            Value::Object(i) => i,
            _ => return Ok(Value::Undefined),
        };
        let sync_fn = rt.object_get(cg, "generateKeyPairSync");
        let mut roots = vec![Value::Object(cg), sync_fn.clone()];
        roots.extend(pass.iter().cloned());
        if let Some(cb) = &cb {
            roots.push(cb.clone());
        }
        let _sync_roots = rt.push_temporary_value_roots(&roots);
        match rt.call_function(sync_fn, Value::Object(cg), pass) {
            Ok(Value::Object(res)) => {
                let pub_k = rt.object_get(res, "publicKey");
                let priv_k = rt.object_get(res, "privateKey");
                if let Some(cb) = cb {
                    let _call_roots = rt.push_temporary_value_roots(&[
                        cb.clone(),
                        Value::Object(res),
                        pub_k.clone(),
                        priv_k.clone(),
                    ]);
                    let _ =
                        rt.call_function(cb, Value::Undefined, vec![Value::Null, pub_k, priv_k]);
                }
            }
            Err(e) => {
                if let Some(cb) = cb {
                    let err = crypto_sval(&format!("{e:?}"));
                    let _call_roots = rt.push_temporary_value_roots(&[cb.clone(), err.clone()]);
                    let _ = rt.call_function(cb, Value::Undefined, vec![err]);
                }
            }
            _ => {}
        }
        Ok(Value::Undefined)
    });

    register_method(rt, crypto, "publicEncrypt", |rt, args| {
        let pem = key_pem(rt, &args.first().cloned().unwrap_or(Value::Undefined));
        let msg = extract_bytes(rt, &args.get(1).cloned().unwrap_or(Value::Undefined));
        let (n, e) = parse_rsa_pub(&pem).ok_or_else(|| {
            RuntimeError::TypeError("publicEncrypt: RSA public key parse failed".into())
        })?;
        let mut seed = vec![0u8; 20];
        let _ = rusty_web_crypto::get_random_values(&mut seed);
        let ct = rusty_web_crypto::rsa_oaep_encrypt(
            &n,
            &e,
            &msg,
            &[],
            &seed,
            |m| rusty_web_crypto::digest_sha1(m).to_vec(),
            20,
        )
        .map_err(|e| RuntimeError::TypeError(format!("publicEncrypt: {e}")))?;
        Ok(make_buffer(rt, &ct))
    });
    register_method(rt, crypto, "privateDecrypt", |rt, args| {
        let pem = key_pem(rt, &args.first().cloned().unwrap_or(Value::Undefined));
        let ct = extract_bytes(rt, &args.get(1).cloned().unwrap_or(Value::Undefined));
        let (n, d) = rusty_tls::parse_rsa_private_key_pem(&pem)
            .map_err(|e| RuntimeError::TypeError(format!("privateDecrypt: key parse: {e:?}")))?;
        let pt = rusty_web_crypto::rsa_oaep_decrypt(
            &n,
            &d,
            &ct,
            &[],
            |m| rusty_web_crypto::digest_sha1(m).to_vec(),
            20,
        )
        .map_err(|e| RuntimeError::TypeError(format!("privateDecrypt: {e}")))?;
        Ok(make_buffer(rt, &pt))
    });

    register_method(rt, crypto, "privateEncrypt", |rt, args| {
        let pem = key_pem(rt, &args.first().cloned().unwrap_or(Value::Undefined));
        let msg = extract_bytes(rt, &args.get(1).cloned().unwrap_or(Value::Undefined));
        let (n, d) = rusty_tls::parse_rsa_private_key_pem(&pem)
            .map_err(|e| RuntimeError::TypeError(format!("privateEncrypt: key parse: {e:?}")))?;
        let ct = rusty_web_crypto::rsa_private_encrypt_pkcs1(&n, &d, &msg)
            .map_err(|e| RuntimeError::TypeError(format!("privateEncrypt: {e}")))?;
        Ok(make_buffer(rt, &ct))
    });
    register_method(rt, crypto, "publicDecrypt", |rt, args| {
        let pem = key_pem(rt, &args.first().cloned().unwrap_or(Value::Undefined));
        let ct = extract_bytes(rt, &args.get(1).cloned().unwrap_or(Value::Undefined));
        let (n, e) = parse_rsa_pub(&pem).ok_or_else(|| {
            RuntimeError::TypeError("publicDecrypt: RSA public key parse failed".into())
        })?;
        let pt = rusty_web_crypto::rsa_public_decrypt_pkcs1(&n, &e, &ct)
            .map_err(|e| RuntimeError::TypeError(format!("publicDecrypt: {e}")))?;
        Ok(make_buffer(rt, &pt))
    });

    for (name, ktype) in [
        ("createPrivateKey", "private"),
        ("createPublicKey", "public"),
    ] {
        register_method(rt, crypto, name, move |rt, args| {
            let (pem, from_string) = match args.first() {
                Some(Value::String(s)) => (s.as_str().to_string(), true),
                Some(Value::Object(id)) => {

                    let is_jwk = matches!(rt.object_get(*id, "format"), Value::String(ref s) if s.as_str() == "jwk");
                    let p = match rt.object_get(*id, "key") {
                        Value::Object(jwk) if is_jwk => {
                            jwk_to_pem(rt, jwk, ktype).unwrap_or_default()
                        }
                        Value::String(s) => s.as_str().to_string(),
                        _ => match rt.object_get(*id, "__pem") {
                            Value::String(s) => s.as_str().to_string(),
                            _ => String::new(),
                        },
                    };
                    (p, false)
                }
                _ => (String::new(), false),
            };

            if from_string {
                let has_pem = pem.contains("-----BEGIN");
                let ok = has_pem
                    && if ktype == "private" {
                        pem.contains("PRIVATE KEY")
                    } else {
                        pem.contains("PUBLIC KEY")
                            || pem.contains("PRIVATE KEY")
                            || pem.contains("CERTIFICATE")
                    };
                if !ok {
                    return Err(RuntimeError::TypeError(format!(
                        "error:1E08010C:DECODER routines::unsupported ({name}: not valid key material)"
                    )));
                }
            }
            let ko = new_object(rt);
            let proto_global = if ktype == "public" {
                "__cruft_PublicKeyObject_proto"
            } else {
                "__cruft_PrivateKeyObject_proto"
            };
            if let Value::Object(p) = rt.global_get(proto_global) {
                rt.obj_mut(ko).proto = Some(p);
            }

            rt.obj_mut(ko)
                .set_own_internal("__cruft_key_object".into(), Value::Boolean(true));

            rt.obj_mut(ko)
                .set_own_internal("type".into(), crypto_sval(ktype));
            rt.obj_mut(ko)
                .set_own_internal("key".into(), crypto_sval(&pem));
            rt.obj_mut(ko)
                .set_own_internal("__pem".into(), crypto_sval(&pem));
            rt.obj_mut(ko)
                .set_own_internal("asymmetricKeyType".into(), crypto_sval(key_type_of(&pem)));

            let details = if pem.is_empty() {
                Value::Undefined
            } else {
                key_details_from_pem(rt, &pem)
            };
            rt.obj_mut(ko)
                .set_own_internal("asymmetricKeyDetails".into(), details);
            register_method_internal(rt, ko, "export", |rt, a| key_object_export(rt, a));
            Ok(Value::Object(ko))
        });
    }
    register_method(rt, crypto, "createSecretKey", |rt, args| {
        let bytes = extract_bytes(rt, &args.first().cloned().unwrap_or(Value::Undefined));
        Ok(Value::Object(build_secret_key(rt, &bytes)))
    });

    register_method(rt, crypto, "generateKeySync", |rt, args| {
        let typ = value_string(args.first());
        let length_bits: i64 = match args.get(1) {
            Some(Value::Object(o)) => match rt.object_get(*o, "length") {
                Value::Number(n) if n.is_finite() => n as i64,
                _ => -1,
            },
            _ => -1,
        };
        let nbytes = match typ.to_ascii_lowercase().as_str() {
            "aes" => {
                if !matches!(length_bits, 128 | 192 | 256) {
                    return Err(coded_crypto_error(
                        rt,
                        "TypeError",
                        "ERR_INVALID_ARG_VALUE",
                        &format!(
                            "The property 'options.length' must be one of: 128, 192, 256. Received {length_bits}"
                        ),
                    ));
                }
                (length_bits / 8) as usize
            }
            "hmac" => {
                if length_bits < 8 || length_bits > 2_147_483_647 {
                    return Err(coded_crypto_error(
                        rt,
                        "RangeError",
                        "ERR_OUT_OF_RANGE",
                        &format!(
                            "The value of \"options.length\" is out of range. It must be >= 8 && <= 2147483647. Received {length_bits}"
                        ),
                    ));
                }
                (length_bits / 8) as usize
            }
            _ => {
                return Err(coded_crypto_error(
                    rt,
                    "TypeError",
                    "ERR_INVALID_ARG_VALUE",
                    &format!("The argument 'type' must be a supported key type. Received '{typ}'"),
                ))
            }
        };
        let mut bytes = vec![0u8; nbytes];
        let _ = rusty_web_crypto::get_random_values(&mut bytes);
        Ok(Value::Object(build_secret_key(rt, &bytes)))
    });

    register_method(rt, crypto, "createSign", |rt, args| {
        let algo = match args.first() {
            Some(Value::String(s)) => s.as_str().to_string(),
            _ => "sha256".to_string(),
        };

        if !sign_digest_supported(&algo) {
            return Err(invalid_digest_error_msg(rt, "Invalid digest"));
        }
        let obj = new_object(rt);
        rt.object_set(obj, "__sign_algo".into(), crypto_sval(&algo));
        rt.object_set(obj, "__sign_data".into(), crypto_sval(""));
        register_method(rt, obj, "update", |rt, args| {
            let this = match rt.current_this() {
                Value::Object(t) => t,
                _ => return Ok(Value::Undefined),
            };
            let mut cur = sign_data_of(rt, this);
            cur.extend_from_slice(&extract_bytes(
                rt,
                &args.first().cloned().unwrap_or(Value::Undefined),
            ));
            rt.object_set(this, "__sign_data".into(), crypto_sval(&hex_encode(&cur)));
            Ok(rt.current_this())
        });

        register_method(rt, obj, "write", |rt, args| {
            if let Value::Object(this) = rt.current_this() {
                let mut cur = sign_data_of(rt, this);
                cur.extend_from_slice(&extract_bytes(
                    rt,
                    &args.first().cloned().unwrap_or(Value::Undefined),
                ));
                rt.object_set(this, "__sign_data".into(), crypto_sval(&hex_encode(&cur)));
            }
            Ok(Value::Boolean(true))
        });
        register_method(rt, obj, "end", |rt, args| {
            if let Value::Object(this) = rt.current_this() {
                if let Some(v) = args.first() {
                    if !rt.is_callable(v) {
                        let mut cur = sign_data_of(rt, this);
                        cur.extend_from_slice(&extract_bytes(rt, v));
                        rt.object_set(this, "__sign_data".into(), crypto_sval(&hex_encode(&cur)));
                    }
                }
            }
            Ok(rt.current_this())
        });
        register_method(rt, obj, "sign", |rt, args| {
            let this = match rt.current_this() {
                Value::Object(t) => t,
                _ => return Ok(Value::Undefined),
            };
            let algo = match rt.object_get(this, "__sign_algo") {
                Value::String(s) => s.as_str().to_string(),
                _ => "sha256".into(),
            };
            let data = sign_data_of(rt, this);
            let pem = key_pem(rt, &args.first().cloned().unwrap_or(Value::Undefined));
            let enc = args.get(1).and_then(|v| {
                if let Value::String(s) = v {
                    Some(s.as_str().to_string())
                } else {
                    None
                }
            });
            let key_arg = args.first().cloned().unwrap_or(Value::Undefined);
            let sig = if let Ok((n, d)) = rusty_tls::parse_rsa_private_key_pem(&pem) {
                let (digest, hname) = sign_digest(&algo, &data);
                if is_pss(rt, &key_arg) {
                    let slen = salt_len_of(rt, &key_arg, digest.len());
                    pss_sign(&n, &d, &data, slen, &hname)
                        .map_err(|e| RuntimeError::TypeError(format!("sign(PSS): {e}")))?
                } else {
                    rusty_web_crypto::rsa_pkcs1_v15_sign(&n, &d, &digest, &hname)
                        .map_err(|e| RuntimeError::TypeError(format!("sign: {e}")))?
                }
            } else if let Ok(d) = rusty_tls::parse_ec_p256_private_key_pem(&pem) {
                let raw = rusty_web_crypto::ecdsa_p256_sha256_sign_deterministic(&d, &data)
                    .map_err(|e| RuntimeError::TypeError(format!("sign(ECDSA): {e}")))?;
                ecdsa_raw_to_der(&raw)
            } else {
                return Err(RuntimeError::TypeError(
                    "sign: unsupported key (RSA + EC P-256 supported)".into(),
                ));
            };
            Ok(ecdh_output(rt, &sig, enc.as_deref()))
        });
        Ok(Value::Object(obj))
    });

    register_method(rt, crypto, "createVerify", |rt, args| {
        let algo = match args.first() {
            Some(Value::String(s)) => s.as_str().to_string(),
            _ => "sha256".to_string(),
        };
        if !sign_digest_supported(&algo) {
            return Err(invalid_digest_error_msg(rt, "Invalid digest"));
        }
        let obj = new_object(rt);
        rt.object_set(obj, "__sign_algo".into(), crypto_sval(&algo));
        rt.object_set(obj, "__sign_data".into(), crypto_sval(""));
        register_method(rt, obj, "update", |rt, args| {
            let this = match rt.current_this() {
                Value::Object(t) => t,
                _ => return Ok(Value::Undefined),
            };
            let mut cur = sign_data_of(rt, this);
            cur.extend_from_slice(&extract_bytes(
                rt,
                &args.first().cloned().unwrap_or(Value::Undefined),
            ));
            rt.object_set(this, "__sign_data".into(), crypto_sval(&hex_encode(&cur)));
            Ok(rt.current_this())
        });

        register_method(rt, obj, "write", |rt, args| {
            if let Value::Object(this) = rt.current_this() {
                let mut cur = sign_data_of(rt, this);
                cur.extend_from_slice(&extract_bytes(
                    rt,
                    &args.first().cloned().unwrap_or(Value::Undefined),
                ));
                rt.object_set(this, "__sign_data".into(), crypto_sval(&hex_encode(&cur)));
            }
            Ok(Value::Boolean(true))
        });
        register_method(rt, obj, "end", |rt, args| {
            if let Value::Object(this) = rt.current_this() {
                if let Some(v) = args.first() {
                    if !rt.is_callable(v) {
                        let mut cur = sign_data_of(rt, this);
                        cur.extend_from_slice(&extract_bytes(rt, v));
                        rt.object_set(this, "__sign_data".into(), crypto_sval(&hex_encode(&cur)));
                    }
                }
            }
            Ok(rt.current_this())
        });
        register_method(rt, obj, "verify", |rt, args| {
            let this = match rt.current_this() {
                Value::Object(t) => t,
                _ => return Ok(Value::Boolean(false)),
            };
            let algo = match rt.object_get(this, "__sign_algo") {
                Value::String(s) => s.as_str().to_string(),
                _ => "sha256".into(),
            };
            let data = sign_data_of(rt, this);
            let pem = key_pem(rt, &args.first().cloned().unwrap_or(Value::Undefined));

            let sig = {
                let v = args.get(1).cloned().unwrap_or(Value::Undefined);
                let enc = args.get(2).and_then(|e| {
                    if let Value::String(s) = e {
                        Some(s.as_str().to_string())
                    } else {
                        None
                    }
                });
                ecdh_input(rt, &v, enc.as_deref())
            };
            let key_arg = args.first().cloned().unwrap_or(Value::Undefined);
            let ok = if let Some((n, e)) = parse_rsa_pub(&pem) {
                let (digest, hname) = sign_digest(&algo, &data);
                if is_pss(rt, &key_arg) {
                    let slen = salt_len_of(rt, &key_arg, digest.len());
                    pss_verify(&n, &e, &data, &sig, slen, &hname)
                } else {
                    rusty_web_crypto::rsa_pkcs1_v15_verify(&n, &e, &digest, &sig, &hname).is_ok()
                }
            } else if let Some((x, y)) = parse_ec_pub(&pem) {
                match ecdsa_der_to_raw(&sig) {
                    Some(raw) => {
                        rusty_web_crypto::ecdsa_p256_sha256_verify(&x, &y, &data, &raw).is_ok()
                    }
                    None => false,
                }
            } else {
                return Err(RuntimeError::TypeError(
                    "verify: public key parse failed (RSA + EC P-256 supported)".into(),
                ));
            };
            Ok(Value::Boolean(ok))
        });
        Ok(Value::Object(obj))
    });

    register_method(rt, crypto, "getDiffieHellman", |rt, args| {
        let name = match args.first() {
            Some(Value::String(s)) => s.as_str().to_string(),
            _ => {
                return Err(RuntimeError::TypeError(
                    "getDiffieHellman: group name required".into(),
                ))
            }
        };
        match dh_named_group(&name) {
            Some((p, g)) => Ok(Value::Object(dh_make(rt, hex_decode(p), hex_decode(g)))),
            None => Err(RuntimeError::TypeError(format!(
                "getDiffieHellman: unknown group '{name}'"
            ))),
        }
    });
    register_method(rt, crypto, "createDiffieHellmanGroup", |rt, args| {
        let name = match args.first() {
            Some(Value::String(s)) => s.as_str().to_string(),
            _ => {
                return Err(RuntimeError::TypeError(
                    "createDiffieHellmanGroup: name required".into(),
                ))
            }
        };
        match dh_named_group(&name) {
            Some((p, g)) => Ok(Value::Object(dh_make(rt, hex_decode(p), hex_decode(g)))),
            None => Err(RuntimeError::TypeError(format!(
                "createDiffieHellmanGroup: unknown group '{name}'"
            ))),
        }
    });
    register_method(rt, crypto, "createDiffieHellman", |rt, args| {

        let prime = match args.first() {
            Some(Value::Number(n)) => {

                match rusty_web_crypto::generate_dh_prime(*n as usize) {
                    Ok(bytes) => bytes,
                    Err(e) => {
                        return Err(RuntimeError::TypeError(format!(
                            "createDiffieHellman(primeLength): {e}"
                        )))
                    }
                }
            }
            Some(v) => {
                let enc = args.get(1).and_then(|e| {
                    if let Value::String(s) = e {
                        Some(s.as_str().to_string())
                    } else {
                        None
                    }
                });
                ecdh_input(rt, &v.clone(), enc.as_deref())
            }
            None => {
                return Err(RuntimeError::TypeError(
                    "createDiffieHellman: prime required".into(),
                ))
            }
        };
        let gen = match args
            .iter()
            .skip(1)
            .find(|v| matches!(v, Value::Object(_)) || matches!(v, Value::Number(_)))
        {
            Some(Value::Number(n)) => vec![*n as u8],
            Some(v) => extract_bytes(rt, &v.clone()),
            _ => vec![2u8],
        };
        Ok(Value::Object(dh_make(rt, prime, gen)))
    });

    {
        let ctor = make_callable(rt, "X509Certificate", |rt, args| {
            let der = match args.first() {
                Some(Value::String(s)) if s.as_str().contains("BEGIN CERTIFICATE") => {
                    pem_to_der(s.as_str())
                }
                Some(Value::String(s)) => s.as_bytes().to_vec(),
                Some(v) => {
                    let bytes = extract_bytes(rt, v);
                    let text = String::from_utf8_lossy(&bytes);
                    if text.contains("BEGIN CERTIFICATE") {
                        pem_to_der(&text)
                    } else {
                        bytes
                    }
                }
                None => {
                    return Err(RuntimeError::TypeError(
                        "X509Certificate: input required".into(),
                    ))
                }
            };
            make_x509(rt, &der)
        });
        let pr = new_object(rt);
        rt.object_set(pr, "constructor".into(), Value::Object(ctor));
        rt.object_set(ctor, "prototype".into(), Value::Object(pr));
        set_constant(rt, crypto, "X509Certificate", Value::Object(ctor));
    }

    register_method(rt, crypto, "createECDH", |rt, args| {
        let curve = match args.first() {
            Some(Value::String(s)) => s.as_str().to_string(),
            _ => "prime256v1".to_string(),
        };
        if ecdh_curve(&curve).is_none() {
            return Err(RuntimeError::TypeError(format!(
                "createECDH: curve '{curve}' not supported (P-256/384/521)"
            )));
        }
        let obj = new_object(rt);
        rt.object_set(obj, "__ecdh_curve".into(), crypto_sval(&curve));
        register_method(rt, obj, "generateKeys", |rt, args| {
            let this = match rt.current_this() {
                Value::Object(t) => t,
                _ => return Ok(Value::Undefined),
            };
            let cn = match rt.object_get(this, "__ecdh_curve") {
                Value::String(s) => s.as_str().to_string(),
                _ => "prime256v1".to_string(),
            };
            let c = ecdh_curve(&cn).unwrap_or_else(rusty_web_crypto::curve_p256);
            let d = ecdh_gen_d(&c);
            rt.object_set(this, "__ecdh_d".into(), crypto_sval(&hex_encode(&d)));
            let pubk = ecdh_pubkey_curve(&c, &d);
            let enc = args.first().and_then(|v| {
                if let Value::String(s) = v {
                    Some(s.as_str().to_string())
                } else {
                    None
                }
            });
            Ok(ecdh_output(rt, &pubk, enc.as_deref()))
        });
        register_method(rt, obj, "getPublicKey", |rt, args| {
            let this = match rt.current_this() {
                Value::Object(t) => t,
                _ => return Ok(Value::Undefined),
            };
            let cn = match rt.object_get(this, "__ecdh_curve") {
                Value::String(s) => s.as_str().to_string(),
                _ => "prime256v1".to_string(),
            };
            let c = ecdh_curve(&cn).unwrap_or_else(rusty_web_crypto::curve_p256);
            let pubk = ecdh_pubkey_curve(&c, &ecdh_d_of(rt, this));
            let enc = args.first().and_then(|v| {
                if let Value::String(s) = v {
                    Some(s.as_str().to_string())
                } else {
                    None
                }
            });
            Ok(ecdh_output(rt, &pubk, enc.as_deref()))
        });
        register_method(rt, obj, "getPrivateKey", |rt, args| {
            let this = match rt.current_this() {
                Value::Object(t) => t,
                _ => return Ok(Value::Undefined),
            };
            let d = ecdh_d_of(rt, this);
            let enc = args.first().and_then(|v| {
                if let Value::String(s) = v {
                    Some(s.as_str().to_string())
                } else {
                    None
                }
            });
            Ok(ecdh_output(rt, &d, enc.as_deref()))
        });
        register_method(rt, obj, "setPrivateKey", |rt, args| {
            let this = match rt.current_this() {
                Value::Object(t) => t,
                _ => return Ok(Value::Undefined),
            };
            let enc = args.get(1).and_then(|v| {
                if let Value::String(s) = v {
                    Some(s.as_str().to_string())
                } else {
                    None
                }
            });
            let d = ecdh_input(
                rt,
                &args.first().cloned().unwrap_or(Value::Undefined),
                enc.as_deref(),
            );
            rt.object_set(this, "__ecdh_d".into(), crypto_sval(&hex_encode(&d)));
            Ok(rt.current_this())
        });
        register_method(rt, obj, "computeSecret", |rt, args| {
            let this = match rt.current_this() {
                Value::Object(t) => t,
                _ => return Ok(Value::Undefined),
            };
            let other = args.first().cloned().unwrap_or(Value::Undefined);
            let other_is_str = matches!(other, Value::String(_));
            let in_enc = if other_is_str {
                args.get(1).and_then(|v| {
                    if let Value::String(s) = v {
                        Some(s.as_str().to_string())
                    } else {
                        None
                    }
                })
            } else {
                None
            };
            let out_enc = if other_is_str {
                args.get(2)
            } else {
                args.get(1)
            }
            .and_then(|v| {
                if let Value::String(s) = v {
                    Some(s.as_str().to_string())
                } else {
                    None
                }
            });
            let pub_bytes = ecdh_input(rt, &other, in_enc.as_deref());
            let cn = match rt.object_get(this, "__ecdh_curve") {
                Value::String(s) => s.as_str().to_string(),
                _ => "prime256v1".to_string(),
            };
            let c = ecdh_curve(&cn).unwrap_or_else(rusty_web_crypto::curve_p256);
            let cb = c.coord_bytes;
            if pub_bytes.len() < 1 + 2 * cb || pub_bytes[0] != 0x04 {
                return Err(RuntimeError::TypeError(
                    "computeSecret: invalid public key".into(),
                ));
            }
            let d = ecdh_d_of(rt, this);
            match rusty_web_crypto::ecdh(
                &c,
                &d,
                &pub_bytes[1..1 + cb],
                &pub_bytes[1 + cb..1 + 2 * cb],
            ) {
                Ok(secret) => Ok(ecdh_output(rt, &secret, out_enc.as_deref())),
                Err(e) => Err(RuntimeError::TypeError(format!("computeSecret: {e}"))),
            }
        });
        register_method(rt, obj, "setPublicKey", |rt, _a| Ok(rt.current_this()));
        Ok(Value::Object(obj))
    });

    for cls in [
        "Hash",
        "Hmac",
        "Cipheriv",
        "Decipheriv",
        "Sign",
        "Verify",
        "KeyObject",
        "Certificate",
        "DiffieHellman",
        "DiffieHellmanGroup",
        "ECDH",
    ] {
        let c = make_callable(rt, cls, |rt, _a| Ok(rt.current_this()));
        let proto = new_object(rt);
        rt.object_set(proto, "constructor".into(), Value::Object(c));
        rt.object_set(c, "prototype".into(), Value::Object(proto));
        set_constant(rt, crypto, cls, Value::Object(c));
    }

    {
        let key_object_proto = match rt.object_get(crypto, "KeyObject") {
            Value::Object(ko) => match rt.object_get(ko, "prototype") {
                Value::Object(p) => Some(p),
                _ => None,
            },
            _ => None,
        };
        for (cls, global_name) in [
            ("PublicKeyObject", "__cruft_PublicKeyObject_proto"),
            ("PrivateKeyObject", "__cruft_PrivateKeyObject_proto"),
        ] {
            let c = make_callable(rt, cls, |rt, _a| Ok(rt.current_this()));
            let proto = new_object(rt);
            if let Some(kp) = key_object_proto {
                rt.obj_mut(proto).proto = Some(kp);
            }
            rt.object_set(proto, "constructor".into(), Value::Object(c));

            register_method(rt, proto, "equals", |rt, args| {
                let this = match rt.current_this() {
                    Value::Object(t) => t,
                    _ => return Ok(Value::Boolean(false)),
                };
                let other = match args.first() {
                    Some(Value::Object(o)) => *o,
                    _ => return Ok(Value::Boolean(false)),
                };
                let same_type = rt.object_get(this, "type") == rt.object_get(other, "type");
                let same_pem = rt.object_get(this, "__pem") == rt.object_get(other, "__pem");
                Ok(Value::Boolean(same_type && same_pem))
            });
            rt.object_set(c, "prototype".into(), Value::Object(proto));
            rt.define_global_property(global_name, Value::Object(proto));
        }

        {
            let c = make_callable(rt, "SecretKeyObject", |rt, _a| Ok(rt.current_this()));
            let proto = new_object(rt);
            if let Some(kp) = key_object_proto {
                rt.obj_mut(proto).proto = Some(kp);
            }
            rt.object_set(proto, "constructor".into(), Value::Object(c));
            register_method(rt, proto, "equals", |rt, args| {
                let this = match rt.current_this() {
                    Value::Object(t) => t,
                    _ => return Ok(Value::Boolean(false)),
                };
                let other = match args.first() {
                    Some(Value::Object(o)) => *o,
                    _ => return Ok(Value::Boolean(false)),
                };
                let same_type = rt.object_get(this, "type") == rt.object_get(other, "type");
                let same_key =
                    rt.object_get(this, "__keybytes") == rt.object_get(other, "__keybytes");
                Ok(Value::Boolean(same_type && same_key))
            });
            rt.object_set(c, "prototype".into(), Value::Object(proto));
            rt.define_global_property("__cruft_SecretKeyObject_proto", Value::Object(proto));
        }
    }

    fn check_prime_bytes(rt: &mut Runtime, args: &[Value]) -> (Vec<u8>, usize) {
        let candidate = args.first().cloned().unwrap_or(Value::Undefined);
        let bytes = match &candidate {
            Value::BigInt(b) => b.to_be_bytes(),
            other => extract_bytes(rt, other),
        };
        let checks = match args.get(1) {
            Some(Value::Object(id)) => match rt.object_get(*id, "checks") {
                Value::Number(n) if n.is_finite() && n >= 1.0 => n as usize,
                _ => 0,
            },
            _ => 0,
        };

        let rounds = if checks == 0 { 64 } else { checks };
        (bytes, rounds)
    }
    register_method(rt, crypto, "checkPrimeSync", |rt, args| {
        let (bytes, rounds) = check_prime_bytes(rt, args);
        Ok(Value::Boolean(rusty_web_crypto::is_probable_prime_be(
            &bytes, rounds,
        )))
    });
    register_method(rt, crypto, "checkPrime", |rt, args| {

        let cb = args.iter().rev().find(|v| rt.is_callable(v)).cloned();
        let (bytes, rounds) = check_prime_bytes(rt, args);
        let result = rusty_web_crypto::is_probable_prime_be(&bytes, rounds);
        if let Some(cb) = cb {
            let roots: Vec<_> = match &cb {
                Value::Object(id) => vec![*id],
                _ => vec![],
            };
            rt.enqueue_host_phase_rooted(
                HostEnqueuePhase::HostCompletionMacrotask,
                "crypto.checkPrime",
                roots,
                move |rt| {

                    let _ = rt.call_function(
                        cb,
                        Value::Undefined,
                        vec![Value::Undefined, Value::Boolean(result)],
                    );
                    Ok(())
                },
            );
        }
        Ok(Value::Undefined)
    });

    fn generate_prime_result(rt: &mut Runtime, args: &[Value]) -> Result<Value, RuntimeError> {
        let size = match args.first() {
            Some(Value::Number(n)) if n.is_finite() && *n >= 1.0 => *n as usize,
            _ => {
                return Err(coded_crypto_error(
                    rt,
                    "TypeError",
                    "ERR_INVALID_ARG_TYPE",
                    "The \"size\" argument must be of type number.",
                ))
            }
        };
        let (safe, bigint_out, has_add) = match args.get(1) {
            Some(Value::Object(id)) => (
                matches!(rt.object_get(*id, "safe"), Value::Boolean(true)),
                matches!(rt.object_get(*id, "bigint"), Value::Boolean(true)),
                !matches!(rt.object_get(*id, "add"), Value::Undefined),
            ),
            _ => (false, false, false),
        };
        if has_add {
            return Err(crypto_error(
                "generatePrime: the {add, rem} options are not yet supported",
            ));
        }
        let bytes = if safe {
            rusty_web_crypto::generate_safe_prime(size)
        } else {
            rusty_web_crypto::generate_dh_prime(size)
        }
        .map_err(|e| crypto_error(format!("generatePrime: {e}")))?;
        if bigint_out {
            Ok(Value::BigInt(std::rc::Rc::new(
                rusty_js_runtime::bigint::JsBigInt::from_be_bytes(&bytes),
            )))
        } else {
            Ok(make_arraybuffer(rt, &bytes))
        }
    }
    register_method(rt, crypto, "generatePrimeSync", |rt, args| {
        generate_prime_result(rt, args)
    });
    register_method(rt, crypto, "generatePrime", |rt, args| {

        let cb = args.iter().rev().find(|v| rt.is_callable(v)).cloned();
        let prime = generate_prime_result(rt, args)?;
        if let Some(cb) = cb {
            let roots: Vec<_> = match &cb {
                Value::Object(id) => vec![*id],
                _ => vec![],
            };
            rt.enqueue_host_phase_rooted(
                HostEnqueuePhase::HostCompletionMacrotask,
                "crypto.generatePrime",
                roots,
                move |rt| {
                    let _ = rt.call_function(cb, Value::Undefined, vec![Value::Undefined, prime]);
                    Ok(())
                },
            );
        }
        Ok(Value::Undefined)
    });

    for name in ["encapsulate", "decapsulate", "argon2", "argon2Sync"] {
        let nm = name;
        register_method(rt, crypto, name, move |_rt, _a| {
            Err(RuntimeError::TypeError(format!(
                "crypto.{nm}: not yet implemented (CRYPTO-SURF deep follow-up)"
            )))
        });
    }

    rt.define_global_property("crypto", Value::Object(crypto));

    rt.materialize_lazy_global("crypto");
}
