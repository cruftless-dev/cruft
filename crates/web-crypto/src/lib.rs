
fn zeroize_bytes(bytes: &mut [u8]) {
    for byte in bytes {
        unsafe {
            std::ptr::write_volatile(byte, 0);
        }
    }
    std::sync::atomic::compiler_fence(std::sync::atomic::Ordering::SeqCst);
}

fn zeroize_words(words: &mut [u32]) {
    for word in words {
        unsafe {
            std::ptr::write_volatile(word, 0);
        }
    }
    std::sync::atomic::compiler_fence(std::sync::atomic::Ordering::SeqCst);
}

struct SecretWords(Vec<u32>);

impl std::ops::Deref for SecretWords {
    type Target = [u32];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Drop for SecretWords {
    fn drop(&mut self) {
        zeroize_words(&mut self.0);
    }
}

pub fn get_random_values(buf: &mut [u8]) -> std::io::Result<()> {
    fill_platform_random(buf)
}

#[cfg(unix)]
fn fill_platform_random(buf: &mut [u8]) -> std::io::Result<()> {
    use std::io::Read;
    let mut f = std::fs::File::open("/dev/urandom")?;
    f.read_exact(buf)
}

#[cfg(windows)]
fn fill_platform_random(buf: &mut [u8]) -> std::io::Result<()> {
    use std::ffi::c_void;
    use std::io::{Error, ErrorKind};

    const BCRYPT_USE_SYSTEM_PREFERRED_RNG: u32 = 0x0000_0002;

    #[link(name = "bcrypt")]
    extern "system" {
        fn BCryptGenRandom(
            hAlgorithm: *mut c_void,
            pbBuffer: *mut u8,
            cbBuffer: u32,
            dwFlags: u32,
        ) -> i32;
    }

    for chunk in buf.chunks_mut(u32::MAX as usize) {
        let status = unsafe {
            BCryptGenRandom(
                std::ptr::null_mut(),
                chunk.as_mut_ptr(),
                chunk.len() as u32,
                BCRYPT_USE_SYSTEM_PREFERRED_RNG,
            )
        };
        if status < 0 {
            return Err(Error::new(
                ErrorKind::Other,
                format!("BCryptGenRandom failed with NTSTATUS 0x{status:08x}"),
            ));
        }
    }
    Ok(())
}

#[cfg(not(any(unix, windows)))]
fn fill_platform_random(_buf: &mut [u8]) -> std::io::Result<()> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "no platform CSPRNG is wired for this target",
    ))
}

#[cfg(test)]
mod random_source_tests {
    use super::get_random_values;

    #[test]
    fn random_source_fills_requested_buffer() {
        let mut bytes = [0u8; 32];
        get_random_values(&mut bytes).expect("platform CSPRNG");
        assert!(bytes.iter().any(|&b| b != 0));
    }

    #[cfg(windows)]
    #[test]
    fn windows_random_source_is_wired() {
        let mut bytes = [0u8; 64];
        get_random_values(&mut bytes).expect("BCryptGenRandom");
        assert!(bytes.iter().any(|&b| b != 0));
    }
}

pub fn random_uuid_v4() -> String {
    let mut bytes = [0u8; 16];
    get_random_values(&mut bytes).expect("random source");

    bytes[6] = (bytes[6] & 0x0F) | 0x40;
    bytes[8] = (bytes[8] & 0x3F) | 0x80;
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        bytes[0], bytes[1], bytes[2], bytes[3],
        bytes[4], bytes[5],
        bytes[6], bytes[7],
        bytes[8], bytes[9],
        bytes[10], bytes[11], bytes[12], bytes[13], bytes[14], bytes[15],
    )
}

pub fn timing_safe_equal(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff: u8 = 0;
    for i in 0..a.len() {
        diff |= a[i] ^ b[i];
    }
    diff == 0
}

const SHA256_K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

const SHA256_H0: [u32; 8] = [
    0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
];

pub fn digest_sha256(data: &[u8]) -> [u8; 32] {
    let mut h = SHA256_H0;
    let mut padded: Vec<u8> = data.to_vec();
    let bit_len = (data.len() as u64) * 8;
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
                .wrapping_add(SHA256_K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
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

    let mut out = [0u8; 32];
    for i in 0..8 {
        out[i * 4..i * 4 + 4].copy_from_slice(&h[i].to_be_bytes());
    }
    out
}

pub fn digest_sha256_hex(data: &[u8]) -> String {
    let bytes = digest_sha256(data);
    let mut s = String::with_capacity(64);
    for b in &bytes {
        s.push_str(&format!("{:02x}", b));
    }
    s
}

const SHA1_H0: [u32; 5] = [0x67452301, 0xefcdab89, 0x98badcfe, 0x10325476, 0xc3d2e1f0];

pub fn digest_sha1(data: &[u8]) -> [u8; 20] {
    let mut h = SHA1_H0;
    let mut padded: Vec<u8> = data.to_vec();
    let bit_len = (data.len() as u64) * 8;
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
                ((b & c) | (!b & d), 0x5a827999_u32)
            } else if i < 40 {
                (b ^ c ^ d, 0x6ed9eba1_u32)
            } else if i < 60 {
                ((b & c) | (b & d) | (c & d), 0x8f1bbcdc_u32)
            } else {
                (b ^ c ^ d, 0xca62c1d6_u32)
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

pub fn digest_sha1_hex(data: &[u8]) -> String {
    let bytes = digest_sha1(data);
    let mut s = String::with_capacity(40);
    for b in &bytes {
        s.push_str(&format!("{:02x}", b));
    }
    s
}

pub fn hmac_sha1(key: &[u8], message: &[u8]) -> [u8; 20] {
    const BLOCK: usize = 64;
    let mut key_pad = [0u8; BLOCK];
    if key.len() > BLOCK {
        let hashed = digest_sha1(key);
        key_pad[..20].copy_from_slice(&hashed);
    } else {
        key_pad[..key.len()].copy_from_slice(key);
    }
    let mut ipad = [0u8; BLOCK];
    let mut opad = [0u8; BLOCK];
    for i in 0..BLOCK {
        ipad[i] = key_pad[i] ^ 0x36;
        opad[i] = key_pad[i] ^ 0x5C;
    }
    let mut inner_input = Vec::with_capacity(BLOCK + message.len());
    inner_input.extend_from_slice(&ipad);
    inner_input.extend_from_slice(message);
    let inner = digest_sha1(&inner_input);
    let mut outer_input = Vec::with_capacity(BLOCK + 20);
    outer_input.extend_from_slice(&opad);
    outer_input.extend_from_slice(&inner);
    let out = digest_sha1(&outer_input);
    zeroize_bytes(&mut key_pad);
    zeroize_bytes(&mut ipad);
    zeroize_bytes(&mut opad);
    zeroize_bytes(&mut inner_input);
    zeroize_bytes(&mut outer_input);
    out
}

pub fn hmac_sha256(key: &[u8], message: &[u8]) -> [u8; 32] {
    const BLOCK: usize = 64;
    let mut key_pad = [0u8; BLOCK];
    if key.len() > BLOCK {
        let hashed = digest_sha256(key);
        key_pad[..32].copy_from_slice(&hashed);
    } else {
        key_pad[..key.len()].copy_from_slice(key);
    }
    let mut ipad = [0u8; BLOCK];
    let mut opad = [0u8; BLOCK];
    for i in 0..BLOCK {
        ipad[i] = key_pad[i] ^ 0x36;
        opad[i] = key_pad[i] ^ 0x5C;
    }
    let mut inner_input = Vec::with_capacity(BLOCK + message.len());
    inner_input.extend_from_slice(&ipad);
    inner_input.extend_from_slice(message);
    let inner = digest_sha256(&inner_input);
    let mut outer_input = Vec::with_capacity(BLOCK + 32);
    outer_input.extend_from_slice(&opad);
    outer_input.extend_from_slice(&inner);
    let out = digest_sha256(&outer_input);
    zeroize_bytes(&mut key_pad);
    zeroize_bytes(&mut ipad);
    zeroize_bytes(&mut opad);
    zeroize_bytes(&mut inner_input);
    zeroize_bytes(&mut outer_input);
    out
}

const SHA512_K: [u64; 80] = [
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

const SHA512_H0: [u64; 8] = [
    0x6a09e667f3bcc908,
    0xbb67ae8584caa73b,
    0x3c6ef372fe94f82b,
    0xa54ff53a5f1d36f1,
    0x510e527fade682d1,
    0x9b05688c2b3e6c1f,
    0x1f83d9abfb41bd6b,
    0x5be0cd19137e2179,
];

const SHA384_H0: [u64; 8] = [
    0xcbbb9d5dc1059ed8,
    0x629a292a367cd507,
    0x9159015a3070dd17,
    0x152fecd8f70e5939,
    0x67332667ffc00b31,
    0x8eb44a8768581511,
    0xdb0c2e0d64f98fa7,
    0x47b5481dbefa4fa4,
];

fn sha512_compress(h: &mut [u64; 8], data: &[u8]) {

    let mut padded: Vec<u8> = data.to_vec();
    let bit_len_lo = (data.len() as u128) * 8;
    padded.push(0x80);
    while padded.len() % 128 != 112 {
        padded.push(0);
    }

    padded.extend_from_slice(&bit_len_lo.to_be_bytes());

    for chunk in padded.chunks_exact(128) {
        let mut w = [0u64; 80];
        for i in 0..16 {
            w[i] = u64::from_be_bytes([
                chunk[i * 8],
                chunk[i * 8 + 1],
                chunk[i * 8 + 2],
                chunk[i * 8 + 3],
                chunk[i * 8 + 4],
                chunk[i * 8 + 5],
                chunk[i * 8 + 6],
                chunk[i * 8 + 7],
            ]);
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
                .wrapping_add(SHA512_K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(28) ^ a.rotate_right(34) ^ a.rotate_right(39);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
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
}

pub fn digest_sha512(data: &[u8]) -> [u8; 64] {
    let mut h = SHA512_H0;
    sha512_compress(&mut h, data);
    let mut out = [0u8; 64];
    for i in 0..8 {
        out[i * 8..i * 8 + 8].copy_from_slice(&h[i].to_be_bytes());
    }
    out
}

pub fn digest_sha384(data: &[u8]) -> [u8; 48] {
    let mut h = SHA384_H0;
    sha512_compress(&mut h, data);
    let mut out = [0u8; 48];

    for i in 0..6 {
        out[i * 8..i * 8 + 8].copy_from_slice(&h[i].to_be_bytes());
    }
    out
}

pub fn digest_sha512_hex(data: &[u8]) -> String {
    let bytes = digest_sha512(data);
    let mut s = String::with_capacity(128);
    for b in &bytes {
        s.push_str(&format!("{:02x}", b));
    }
    s
}

pub fn digest_sha384_hex(data: &[u8]) -> String {
    let bytes = digest_sha384(data);
    let mut s = String::with_capacity(96);
    for b in &bytes {
        s.push_str(&format!("{:02x}", b));
    }
    s
}

pub fn hmac_sha512(key: &[u8], message: &[u8]) -> [u8; 64] {
    const BLOCK: usize = 128;
    let mut key_pad = [0u8; BLOCK];
    if key.len() > BLOCK {
        let hashed = digest_sha512(key);
        key_pad[..64].copy_from_slice(&hashed);
    } else {
        key_pad[..key.len()].copy_from_slice(key);
    }
    let mut ipad = [0u8; BLOCK];
    let mut opad = [0u8; BLOCK];
    for i in 0..BLOCK {
        ipad[i] = key_pad[i] ^ 0x36;
        opad[i] = key_pad[i] ^ 0x5C;
    }
    let mut inner_input = Vec::with_capacity(BLOCK + message.len());
    inner_input.extend_from_slice(&ipad);
    inner_input.extend_from_slice(message);
    let inner = digest_sha512(&inner_input);
    let mut outer_input = Vec::with_capacity(BLOCK + 64);
    outer_input.extend_from_slice(&opad);
    outer_input.extend_from_slice(&inner);
    let out = digest_sha512(&outer_input);
    zeroize_bytes(&mut key_pad);
    zeroize_bytes(&mut ipad);
    zeroize_bytes(&mut opad);
    zeroize_bytes(&mut inner_input);
    zeroize_bytes(&mut outer_input);
    out
}

pub fn hmac_sha384(key: &[u8], message: &[u8]) -> [u8; 48] {
    const BLOCK: usize = 128;
    let mut key_pad = [0u8; BLOCK];
    if key.len() > BLOCK {
        let hashed = digest_sha384(key);
        key_pad[..48].copy_from_slice(&hashed);
    } else {
        key_pad[..key.len()].copy_from_slice(key);
    }
    let mut ipad = [0u8; BLOCK];
    let mut opad = [0u8; BLOCK];
    for i in 0..BLOCK {
        ipad[i] = key_pad[i] ^ 0x36;
        opad[i] = key_pad[i] ^ 0x5C;
    }
    let mut inner_input = Vec::with_capacity(BLOCK + message.len());
    inner_input.extend_from_slice(&ipad);
    inner_input.extend_from_slice(message);
    let inner = digest_sha384(&inner_input);
    let mut outer_input = Vec::with_capacity(BLOCK + 48);
    outer_input.extend_from_slice(&opad);
    outer_input.extend_from_slice(&inner);
    let out = digest_sha384(&outer_input);
    zeroize_bytes(&mut key_pad);
    zeroize_bytes(&mut ipad);
    zeroize_bytes(&mut opad);
    zeroize_bytes(&mut inner_input);
    zeroize_bytes(&mut outer_input);
    out
}

fn pbkdf2_inner<F, const H: usize>(
    prf: F,
    password: &[u8],
    salt: &[u8],
    iterations: u32,
    dk_len: usize,
) -> Vec<u8>
where
    F: Fn(&[u8], &[u8]) -> [u8; H],
{
    if iterations == 0 || dk_len == 0 {
        return Vec::new();
    }
    let l = (dk_len + H - 1) / H;
    let mut out = Vec::with_capacity(l * H);
    let mut salt_with_index = Vec::with_capacity(salt.len() + 4);
    for i in 1..=l {
        salt_with_index.clear();
        salt_with_index.extend_from_slice(salt);
        salt_with_index.extend_from_slice(&(i as u32).to_be_bytes());
        let mut u = prf(password, &salt_with_index);
        let mut t = u;
        for _ in 1..iterations {
            u = prf(password, &u);
            for k in 0..H {
                t[k] ^= u[k];
            }
        }
        out.extend_from_slice(&t);
    }
    out.truncate(dk_len);
    out
}

pub fn pbkdf2_hmac_sha1(password: &[u8], salt: &[u8], iterations: u32, dk_len: usize) -> Vec<u8> {
    pbkdf2_inner::<_, 20>(hmac_sha1, password, salt, iterations, dk_len)
}

pub fn pbkdf2_hmac_sha256(password: &[u8], salt: &[u8], iterations: u32, dk_len: usize) -> Vec<u8> {
    pbkdf2_inner::<_, 32>(hmac_sha256, password, salt, iterations, dk_len)
}

pub fn pbkdf2_hmac_sha384(password: &[u8], salt: &[u8], iterations: u32, dk_len: usize) -> Vec<u8> {
    pbkdf2_inner::<_, 48>(hmac_sha384, password, salt, iterations, dk_len)
}

pub fn pbkdf2_hmac_sha512(password: &[u8], salt: &[u8], iterations: u32, dk_len: usize) -> Vec<u8> {
    pbkdf2_inner::<_, 64>(hmac_sha512, password, salt, iterations, dk_len)
}

fn salsa20_8(block: &mut [u8; 64]) {
    let mut x = [0u32; 16];
    for (i, w) in x.iter_mut().enumerate() {
        *w = u32::from_le_bytes([
            block[i * 4],
            block[i * 4 + 1],
            block[i * 4 + 2],
            block[i * 4 + 3],
        ]);
    }
    let orig = x;
    macro_rules! r {
        ($a:expr,$b:expr,$n:expr) => {
            $a ^= $b.rotate_left($n);
        };
    }
    for _ in 0..4 {

        r!(x[4], x[0].wrapping_add(x[12]), 7);
        r!(x[8], x[4].wrapping_add(x[0]), 9);
        r!(x[12], x[8].wrapping_add(x[4]), 13);
        r!(x[0], x[12].wrapping_add(x[8]), 18);
        r!(x[9], x[5].wrapping_add(x[1]), 7);
        r!(x[13], x[9].wrapping_add(x[5]), 9);
        r!(x[1], x[13].wrapping_add(x[9]), 13);
        r!(x[5], x[1].wrapping_add(x[13]), 18);
        r!(x[14], x[10].wrapping_add(x[6]), 7);
        r!(x[2], x[14].wrapping_add(x[10]), 9);
        r!(x[6], x[2].wrapping_add(x[14]), 13);
        r!(x[10], x[6].wrapping_add(x[2]), 18);
        r!(x[3], x[15].wrapping_add(x[11]), 7);
        r!(x[7], x[3].wrapping_add(x[15]), 9);
        r!(x[11], x[7].wrapping_add(x[3]), 13);
        r!(x[15], x[11].wrapping_add(x[7]), 18);

        r!(x[1], x[0].wrapping_add(x[3]), 7);
        r!(x[2], x[1].wrapping_add(x[0]), 9);
        r!(x[3], x[2].wrapping_add(x[1]), 13);
        r!(x[0], x[3].wrapping_add(x[2]), 18);
        r!(x[6], x[5].wrapping_add(x[4]), 7);
        r!(x[7], x[6].wrapping_add(x[5]), 9);
        r!(x[4], x[7].wrapping_add(x[6]), 13);
        r!(x[5], x[4].wrapping_add(x[7]), 18);
        r!(x[11], x[10].wrapping_add(x[9]), 7);
        r!(x[8], x[11].wrapping_add(x[10]), 9);
        r!(x[9], x[8].wrapping_add(x[11]), 13);
        r!(x[10], x[9].wrapping_add(x[8]), 18);
        r!(x[12], x[15].wrapping_add(x[14]), 7);
        r!(x[13], x[12].wrapping_add(x[15]), 9);
        r!(x[14], x[13].wrapping_add(x[12]), 13);
        r!(x[15], x[14].wrapping_add(x[13]), 18);
    }
    for i in 0..16 {
        let v = x[i].wrapping_add(orig[i]);
        block[i * 4..i * 4 + 4].copy_from_slice(&v.to_le_bytes());
    }
}

fn block_mix(b: &[u8], r: usize) -> Vec<u8> {
    let mut x = [0u8; 64];
    x.copy_from_slice(&b[(2 * r - 1) * 64..2 * r * 64]);
    let mut out = vec![0u8; 128 * r];
    for i in 0..2 * r {
        for j in 0..64 {
            x[j] ^= b[i * 64 + j];
        }
        salsa20_8(&mut x);

        let dst = if i % 2 == 0 {
            (i / 2) * 64
        } else {
            (r + i / 2) * 64
        };
        out[dst..dst + 64].copy_from_slice(&x);
    }
    out
}

fn ro_mix(b: &mut [u8], n: usize, r: usize) {
    let mut v: Vec<Vec<u8>> = Vec::with_capacity(n);
    let mut x = b.to_vec();
    for _ in 0..n {
        v.push(x.clone());
        x = block_mix(&x, r);
    }
    for _ in 0..n {

        let j = {
            let off = (2 * r - 1) * 64;
            u32::from_le_bytes([x[off], x[off + 1], x[off + 2], x[off + 3]]) as usize % n
        };
        for k in 0..128 * r {
            x[k] ^= v[j][k];
        }
        x = block_mix(&x, r);
    }
    b.copy_from_slice(&x);
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScryptError {
    InvalidParams(String),
    MemoryLimitExceeded,
}

pub fn scrypt(
    password: &[u8],
    salt: &[u8],
    n: u32,
    r: u32,
    p: u32,
    dk_len: usize,
    max_mem: u64,
) -> Result<Vec<u8>, ScryptError> {
    if n < 2 || (n & (n - 1)) != 0 {
        return Err(ScryptError::InvalidParams(
            "N must be a power of two > 1".into(),
        ));
    }
    if r == 0 || p == 0 {
        return Err(ScryptError::InvalidParams("r and p must be >= 1".into()));
    }

    let mem = 128u64
        .checked_mul(n as u64)
        .and_then(|v| v.checked_mul(r as u64));
    match mem {
        Some(m) if m <= max_mem => {}
        _ => return Err(ScryptError::MemoryLimitExceeded),
    }
    let r_us = r as usize;
    let p_us = p as usize;

    let mut b = pbkdf2_hmac_sha256(password, salt, 1, p_us * 128 * r_us);

    for i in 0..p_us {
        let start = i * 128 * r_us;
        let mut block = b[start..start + 128 * r_us].to_vec();
        ro_mix(&mut block, n as usize, r_us);
        b[start..start + 128 * r_us].copy_from_slice(&block);
    }

    Ok(pbkdf2_hmac_sha256(password, &b, 1, dk_len))
}

fn hkdf_inner<F, const H: usize>(
    prf: F,
    ikm: &[u8],
    salt: &[u8],
    info: &[u8],
    length: usize,
) -> Result<Vec<u8>, String>
where
    F: Fn(&[u8], &[u8]) -> [u8; H],
{

    if length > 255 * H {
        return Err(format!(
            "HKDF: length {} exceeds 255 * HashLen ({})",
            length,
            255 * H
        ));
    }

    let zero_salt = vec![0u8; H];
    let prk = if salt.is_empty() {
        prf(&zero_salt, ikm)
    } else {
        prf(salt, ikm)
    };

    let n = (length + H - 1) / H;
    let mut okm = Vec::with_capacity(n * H);
    let mut prev: Vec<u8> = Vec::new();
    for i in 1..=n {
        let mut buf = Vec::with_capacity(prev.len() + info.len() + 1);
        buf.extend_from_slice(&prev);
        buf.extend_from_slice(info);
        buf.push(i as u8);
        let t = prf(&prk, &buf);
        prev = t.to_vec();
        okm.extend_from_slice(&t);
    }
    okm.truncate(length);
    Ok(okm)
}

pub fn hkdf_sha1(ikm: &[u8], salt: &[u8], info: &[u8], length: usize) -> Result<Vec<u8>, String> {
    hkdf_inner::<_, 20>(hmac_sha1, ikm, salt, info, length)
}
pub fn hkdf_sha256(ikm: &[u8], salt: &[u8], info: &[u8], length: usize) -> Result<Vec<u8>, String> {
    hkdf_inner::<_, 32>(hmac_sha256, ikm, salt, info, length)
}
pub fn hkdf_sha384(ikm: &[u8], salt: &[u8], info: &[u8], length: usize) -> Result<Vec<u8>, String> {
    hkdf_inner::<_, 48>(hmac_sha384, ikm, salt, info, length)
}
pub fn hkdf_sha512(ikm: &[u8], salt: &[u8], info: &[u8], length: usize) -> Result<Vec<u8>, String> {
    hkdf_inner::<_, 64>(hmac_sha512, ikm, salt, info, length)
}

const AES_SBOX: [u8; 256] = [
    0x63, 0x7c, 0x77, 0x7b, 0xf2, 0x6b, 0x6f, 0xc5, 0x30, 0x01, 0x67, 0x2b, 0xfe, 0xd7, 0xab, 0x76,
    0xca, 0x82, 0xc9, 0x7d, 0xfa, 0x59, 0x47, 0xf0, 0xad, 0xd4, 0xa2, 0xaf, 0x9c, 0xa4, 0x72, 0xc0,
    0xb7, 0xfd, 0x93, 0x26, 0x36, 0x3f, 0xf7, 0xcc, 0x34, 0xa5, 0xe5, 0xf1, 0x71, 0xd8, 0x31, 0x15,
    0x04, 0xc7, 0x23, 0xc3, 0x18, 0x96, 0x05, 0x9a, 0x07, 0x12, 0x80, 0xe2, 0xeb, 0x27, 0xb2, 0x75,
    0x09, 0x83, 0x2c, 0x1a, 0x1b, 0x6e, 0x5a, 0xa0, 0x52, 0x3b, 0xd6, 0xb3, 0x29, 0xe3, 0x2f, 0x84,
    0x53, 0xd1, 0x00, 0xed, 0x20, 0xfc, 0xb1, 0x5b, 0x6a, 0xcb, 0xbe, 0x39, 0x4a, 0x4c, 0x58, 0xcf,
    0xd0, 0xef, 0xaa, 0xfb, 0x43, 0x4d, 0x33, 0x85, 0x45, 0xf9, 0x02, 0x7f, 0x50, 0x3c, 0x9f, 0xa8,
    0x51, 0xa3, 0x40, 0x8f, 0x92, 0x9d, 0x38, 0xf5, 0xbc, 0xb6, 0xda, 0x21, 0x10, 0xff, 0xf3, 0xd2,
    0xcd, 0x0c, 0x13, 0xec, 0x5f, 0x97, 0x44, 0x17, 0xc4, 0xa7, 0x7e, 0x3d, 0x64, 0x5d, 0x19, 0x73,
    0x60, 0x81, 0x4f, 0xdc, 0x22, 0x2a, 0x90, 0x88, 0x46, 0xee, 0xb8, 0x14, 0xde, 0x5e, 0x0b, 0xdb,
    0xe0, 0x32, 0x3a, 0x0a, 0x49, 0x06, 0x24, 0x5c, 0xc2, 0xd3, 0xac, 0x62, 0x91, 0x95, 0xe4, 0x79,
    0xe7, 0xc8, 0x37, 0x6d, 0x8d, 0xd5, 0x4e, 0xa9, 0x6c, 0x56, 0xf4, 0xea, 0x65, 0x7a, 0xae, 0x08,
    0xba, 0x78, 0x25, 0x2e, 0x1c, 0xa6, 0xb4, 0xc6, 0xe8, 0xdd, 0x74, 0x1f, 0x4b, 0xbd, 0x8b, 0x8a,
    0x70, 0x3e, 0xb5, 0x66, 0x48, 0x03, 0xf6, 0x0e, 0x61, 0x35, 0x57, 0xb9, 0x86, 0xc1, 0x1d, 0x9e,
    0xe1, 0xf8, 0x98, 0x11, 0x69, 0xd9, 0x8e, 0x94, 0x9b, 0x1e, 0x87, 0xe9, 0xce, 0x55, 0x28, 0xdf,
    0x8c, 0xa1, 0x89, 0x0d, 0xbf, 0xe6, 0x42, 0x68, 0x41, 0x99, 0x2d, 0x0f, 0xb0, 0x54, 0xbb, 0x16,
];

const AES_RCON: [u8; 11] = [
    0x00, 0x01, 0x02, 0x04, 0x08, 0x10, 0x20, 0x40, 0x80, 0x1b, 0x36,
];

fn aes_xtime(x: u8) -> u8 {
    (x << 1) ^ if x & 0x80 != 0 { 0x1b } else { 0x00 }
}

fn aes_sub_word(w: u32) -> u32 {
    let b = w.to_be_bytes();
    u32::from_be_bytes([
        AES_SBOX[b[0] as usize],
        AES_SBOX[b[1] as usize],
        AES_SBOX[b[2] as usize],
        AES_SBOX[b[3] as usize],
    ])
}

fn aes_key_expansion(key: &[u8]) -> SecretWords {
    let nk = key.len() / 4;
    let nr = nk + 6;
    let total = 4 * (nr + 1);
    let mut w = Vec::with_capacity(total);
    for i in 0..nk {
        w.push(u32::from_be_bytes([
            key[4 * i],
            key[4 * i + 1],
            key[4 * i + 2],
            key[4 * i + 3],
        ]));
    }
    for i in nk..total {
        let mut t = w[i - 1];
        if i % nk == 0 {
            t = aes_sub_word(t.rotate_left(8)) ^ ((AES_RCON[i / nk] as u32) << 24);
        } else if nk > 6 && i % nk == 4 {
            t = aes_sub_word(t);
        }
        w.push(w[i - nk] ^ t);
    }
    SecretWords(w)
}

fn aes_add_round_key(state: &mut [u8; 16], w: &[u32]) {
    for c in 0..4 {
        let k = w[c].to_be_bytes();
        for r in 0..4 {
            state[r * 4 + c] ^= k[r];
        }
    }
}

fn aes_sub_bytes(state: &mut [u8; 16]) {
    for b in state.iter_mut() {
        *b = AES_SBOX[*b as usize];
    }
}

fn aes_shift_rows(state: &mut [u8; 16]) {

    let s = *state;
    for r in 1..4 {
        for c in 0..4 {
            state[r * 4 + c] = s[r * 4 + (c + r) % 4];
        }
    }
}

fn aes_mix_columns(state: &mut [u8; 16]) {
    for c in 0..4 {
        let s0 = state[c];
        let s1 = state[4 + c];
        let s2 = state[8 + c];
        let s3 = state[12 + c];
        let t = s0 ^ s1 ^ s2 ^ s3;
        state[c] ^= t ^ aes_xtime(s0 ^ s1);
        state[4 + c] ^= t ^ aes_xtime(s1 ^ s2);
        state[8 + c] ^= t ^ aes_xtime(s2 ^ s3);
        state[12 + c] ^= t ^ aes_xtime(s3 ^ s0);
    }
}

fn aes_encrypt_block(block: &[u8; 16], w: &[u32]) -> [u8; 16] {
    let nr = w.len() / 4 - 1;
    let mut state = [0u8; 16];
    for c in 0..4 {
        for r in 0..4 {
            state[r * 4 + c] = block[4 * c + r];
        }
    }
    aes_add_round_key(&mut state, &w[0..4]);
    for round in 1..nr {
        aes_sub_bytes(&mut state);
        aes_shift_rows(&mut state);
        aes_mix_columns(&mut state);
        aes_add_round_key(&mut state, &w[4 * round..4 * round + 4]);
    }
    aes_sub_bytes(&mut state);
    aes_shift_rows(&mut state);
    aes_add_round_key(&mut state, &w[4 * nr..4 * nr + 4]);
    let mut out = [0u8; 16];
    for c in 0..4 {
        for r in 0..4 {
            out[4 * c + r] = state[r * 4 + c];
        }
    }
    out
}

pub fn aes_encrypt_block_with_key(key: &[u8], block: &[u8; 16]) -> [u8; 16] {
    assert!(
        key.len() == 16 || key.len() == 24 || key.len() == 32,
        "AES key must be 16/24/32 bytes"
    );
    let w = aes_key_expansion(key);
    aes_encrypt_block(block, &w)
}

#[cfg(test)]
fn te_tables() -> &'static [[u32; 256]; 4] {
    static T: std::sync::OnceLock<[[u32; 256]; 4]> = std::sync::OnceLock::new();
    T.get_or_init(|| {
        let mut te = [[0u32; 256]; 4];
        for x in 0..256 {
            let s = AES_SBOX[x] as u32;
            let s2 = aes_xtime(AES_SBOX[x]) as u32;
            let s3 = s2 ^ s;

            let t0 = (s2 << 24) | (s << 16) | (s << 8) | s3;
            te[0][x] = t0;
            te[1][x] = t0.rotate_right(8);
            te[2][x] = t0.rotate_right(16);
            te[3][x] = t0.rotate_right(24);
        }
        te
    })
}

#[cfg(test)]
fn aes_encrypt_block_fast(block: &[u8; 16], w: &[u32]) -> [u8; 16] {
    let te = te_tables();
    let (te0, te1, te2, te3) = (&te[0], &te[1], &te[2], &te[3]);
    let nr = w.len() / 4 - 1;
    let mut s0 = u32::from_be_bytes([block[0], block[1], block[2], block[3]]) ^ w[0];
    let mut s1 = u32::from_be_bytes([block[4], block[5], block[6], block[7]]) ^ w[1];
    let mut s2 = u32::from_be_bytes([block[8], block[9], block[10], block[11]]) ^ w[2];
    let mut s3 = u32::from_be_bytes([block[12], block[13], block[14], block[15]]) ^ w[3];
    for round in 1..nr {
        let rk = &w[4 * round..];
        let t0 = te0[(s0 >> 24) as usize]
            ^ te1[((s1 >> 16) & 0xff) as usize]
            ^ te2[((s2 >> 8) & 0xff) as usize]
            ^ te3[(s3 & 0xff) as usize]
            ^ rk[0];
        let t1 = te0[(s1 >> 24) as usize]
            ^ te1[((s2 >> 16) & 0xff) as usize]
            ^ te2[((s3 >> 8) & 0xff) as usize]
            ^ te3[(s0 & 0xff) as usize]
            ^ rk[1];
        let t2 = te0[(s2 >> 24) as usize]
            ^ te1[((s3 >> 16) & 0xff) as usize]
            ^ te2[((s0 >> 8) & 0xff) as usize]
            ^ te3[(s1 & 0xff) as usize]
            ^ rk[2];
        let t3 = te0[(s3 >> 24) as usize]
            ^ te1[((s0 >> 16) & 0xff) as usize]
            ^ te2[((s1 >> 8) & 0xff) as usize]
            ^ te3[(s2 & 0xff) as usize]
            ^ rk[3];
        s0 = t0;
        s1 = t1;
        s2 = t2;
        s3 = t3;
    }

    let rk = &w[4 * nr..];
    let sb = |b: u32, sh: u32| AES_SBOX[((b >> sh) & 0xff) as usize] as u32;
    let o0 = ((sb(s0, 24) << 24) | (sb(s1, 16) << 16) | (sb(s2, 8) << 8) | sb(s3, 0)) ^ rk[0];
    let o1 = ((sb(s1, 24) << 24) | (sb(s2, 16) << 16) | (sb(s3, 8) << 8) | sb(s0, 0)) ^ rk[1];
    let o2 = ((sb(s2, 24) << 24) | (sb(s3, 16) << 16) | (sb(s0, 8) << 8) | sb(s1, 0)) ^ rk[2];
    let o3 = ((sb(s3, 24) << 24) | (sb(s0, 16) << 16) | (sb(s1, 8) << 8) | sb(s2, 0)) ^ rk[3];
    let mut out = [0u8; 16];
    out[0..4].copy_from_slice(&o0.to_be_bytes());
    out[4..8].copy_from_slice(&o1.to_be_bytes());
    out[8..12].copy_from_slice(&o2.to_be_bytes());
    out[12..16].copy_from_slice(&o3.to_be_bytes());
    out
}

#[derive(Clone, Debug)]
pub struct BigUInt(Vec<u32>);

impl BigUInt {
    fn zeroize_limbs(&mut self) {
        zeroize_words(&mut self.0);
    }

    pub fn zero() -> Self {
        BigUInt(vec![0])
    }
    pub fn one() -> Self {
        BigUInt(vec![1])
    }

    pub fn limbs(&self) -> &[u32] {
        &self.0
    }

    pub fn from_limbs(limbs: Vec<u32>) -> Self {
        let mut r = BigUInt(limbs);
        r.trim();
        r
    }

    pub fn from_be_bytes(b: &[u8]) -> Self {

        let n_limbs = (b.len() + 3) / 4;
        let mut limbs = vec![0u32; n_limbs];
        for (i, byte) in b.iter().rev().enumerate() {
            limbs[i / 4] |= (*byte as u32) << ((i % 4) * 8);
        }
        let mut r = BigUInt(limbs);
        r.trim();
        r
    }

    pub fn to_be_bytes(&self, len: usize) -> Vec<u8> {
        let mut out = vec![0u8; len];
        for i in 0..len {
            let limb = self.0.get(i / 4).copied().unwrap_or(0);
            let byte = (limb >> ((i % 4) * 8)) & 0xff;
            out[len - 1 - i] = byte as u8;
        }
        out
    }

    fn trim(&mut self) {
        while self.0.len() > 1 && *self.0.last().unwrap() == 0 {
            self.0.pop();
        }
    }

    pub fn is_zero(&self) -> bool {
        self.0.iter().all(|&l| l == 0)
    }

    pub fn bit_len(&self) -> usize {
        for i in (0..self.0.len()).rev() {
            if self.0[i] != 0 {
                return i * 32 + (32 - self.0[i].leading_zeros() as usize);
            }
        }
        0
    }

    pub fn bit(&self, i: usize) -> bool {
        let limb = i / 32;
        let bit = i % 32;
        self.0.get(limb).copied().unwrap_or(0) & (1u32 << bit) != 0
    }

    pub fn cmp(&self, other: &BigUInt) -> std::cmp::Ordering {
        use std::cmp::Ordering;

        let la = self.0.len();
        let lb = other.0.len();
        let la_eff = (0..la)
            .rev()
            .find(|&i| self.0[i] != 0)
            .map(|i| i + 1)
            .unwrap_or(0);
        let lb_eff = (0..lb)
            .rev()
            .find(|&i| other.0[i] != 0)
            .map(|i| i + 1)
            .unwrap_or(0);
        if la_eff != lb_eff {
            return la_eff.cmp(&lb_eff);
        }
        for i in (0..la_eff).rev() {
            match self.0[i].cmp(&other.0[i]) {
                Ordering::Equal => continue,
                ord => return ord,
            }
        }
        Ordering::Equal
    }

    pub fn add(&self, other: &BigUInt) -> BigUInt {
        let n = self.0.len().max(other.0.len()) + 1;
        let mut out = vec![0u32; n];
        let mut carry: u64 = 0;
        for i in 0..n {
            let a = self.0.get(i).copied().unwrap_or(0) as u64;
            let b = other.0.get(i).copied().unwrap_or(0) as u64;
            let sum = a + b + carry;
            out[i] = (sum & 0xffffffff) as u32;
            carry = sum >> 32;
        }
        let mut r = BigUInt(out);
        r.trim();
        r
    }

    pub fn sub(&self, other: &BigUInt) -> BigUInt {
        let n = self.0.len();
        let mut out = vec![0u32; n];
        let mut borrow: i64 = 0;
        for i in 0..n {
            let a = self.0[i] as i64;
            let b = other.0.get(i).copied().unwrap_or(0) as i64;
            let diff = a - b - borrow;
            if diff < 0 {
                out[i] = (diff + (1i64 << 32)) as u32;
                borrow = 1;
            } else {
                out[i] = diff as u32;
                borrow = 0;
            }
        }
        let mut r = BigUInt(out);
        r.trim();
        r
    }

    pub fn mul(&self, other: &BigUInt) -> BigUInt {
        const KARATSUBA_THRESHOLD: usize = 24;
        let n = std::cmp::max(self.0.len(), other.0.len());
        if n < KARATSUBA_THRESHOLD || self.0.len() < 2 || other.0.len() < 2 {
            return self.mul_schoolbook(other);
        }

        let split = (n + 1) / 2;
        let a_lo_limbs = self.0.iter().take(split).cloned().collect::<Vec<u32>>();
        let a_hi_limbs = self.0.iter().skip(split).cloned().collect::<Vec<u32>>();
        let b_lo_limbs = other.0.iter().take(split).cloned().collect::<Vec<u32>>();
        let b_hi_limbs = other.0.iter().skip(split).cloned().collect::<Vec<u32>>();
        let a_lo = BigUInt::from_limbs(a_lo_limbs);
        let a_hi = BigUInt::from_limbs(a_hi_limbs);
        let b_lo = BigUInt::from_limbs(b_lo_limbs);
        let b_hi = BigUInt::from_limbs(b_hi_limbs);

        let z0 = a_lo.mul(&b_lo);
        let z2 = a_hi.mul(&b_hi);
        let a_sum = a_lo.add(&a_hi);
        let b_sum = b_lo.add(&b_hi);
        let z1_full = a_sum.mul(&b_sum);
        let z1 = z1_full.sub(&z0).sub(&z2);

        let z1_shifted = z1.shl_limbs(split);
        let z2_shifted = z2.shl_limbs(2 * split);
        z0.add(&z1_shifted).add(&z2_shifted)
    }

    fn shl_limbs(&self, k: usize) -> BigUInt {
        if self.is_zero() {
            return BigUInt::zero();
        }
        let mut out = vec![0u32; k + self.0.len()];
        out[k..].copy_from_slice(&self.0);
        let mut r = BigUInt(out);
        r.trim();
        r
    }

    fn mul_schoolbook(&self, other: &BigUInt) -> BigUInt {

        let an = self.0.len();
        let bn = other.0.len();
        let n = an + bn;
        let mut limbs = vec![0u32; n + 1];
        let mut acc: u128 = 0;
        for k in 0..n {
            let i_min = if k >= bn { k - bn + 1 } else { 0 };
            let i_max = std::cmp::min(k + 1, an);
            for i in i_min..i_max {
                let j = k - i;
                acc += (self.0[i] as u128) * (other.0[j] as u128);
            }
            limbs[k] = acc as u32;
            acc >>= 32;
        }
        limbs[n] = acc as u32;

        let mut r = BigUInt(limbs);
        r.trim();
        r
    }

    fn shl1(&self) -> BigUInt {
        let mut out = vec![0u32; self.0.len() + 1];
        let mut carry: u32 = 0;
        for (i, &l) in self.0.iter().enumerate() {
            out[i] = (l << 1) | carry;
            carry = l >> 31;
        }
        out[self.0.len()] = carry;
        let mut r = BigUInt(out);
        r.trim();
        r
    }

    pub fn divmod(&self, divisor: &BigUInt) -> (BigUInt, BigUInt) {
        use std::cmp::Ordering;
        assert!(!divisor.is_zero(), "BigUInt divmod by zero");
        let bits = self.bit_len();
        let mut q_limbs = vec![0u32; (bits + 31) / 32 + 1];
        let mut r = BigUInt::zero();
        for i in (0..bits).rev() {
            r = r.shl1();
            if self.bit(i) {

                if r.0.is_empty() {
                    r.0.push(0);
                }
                r.0[0] |= 1;
            }
            if r.cmp(divisor) != Ordering::Less {
                r = r.sub(divisor);
                q_limbs[i / 32] |= 1u32 << (i % 32);
            }
        }
        let mut q = BigUInt(q_limbs);
        q.trim();
        r.trim();
        (q, r)
    }

    pub fn modulo(&self, m: &BigUInt) -> BigUInt {
        self.divmod(m).1
    }

    pub fn mod_pow(&self, e: &BigUInt, m: &BigUInt) -> BigUInt {
        if m.cmp(&BigUInt::one()) == std::cmp::Ordering::Equal {
            return BigUInt::zero();
        }
        let mut result = BigUInt::one();
        let mut base = self.modulo(m);
        let bits = e.bit_len();
        for i in 0..bits {
            if e.bit(i) {
                result = result.mul(&base).modulo(m);
            }
            base = base.mul(&base).modulo(m);
        }
        result
    }
}

impl Drop for BigUInt {
    fn drop(&mut self) {
        self.zeroize_limbs();
    }
}

pub fn rsaep(n: &BigUInt, e: &BigUInt, m: &BigUInt) -> Result<BigUInt, String> {
    if m.cmp(n) != std::cmp::Ordering::Less {
        return Err("RSAEP: message representative out of range".to_string());
    }

    Ok(mod_pow_mont(m, e, n))
}

pub fn rsadp(n: &BigUInt, d: &BigUInt, c: &BigUInt) -> Result<BigUInt, String> {
    if c.cmp(n) != std::cmp::Ordering::Less {
        return Err("RSADP: ciphertext representative out of range".to_string());
    }

    Ok(mod_pow_mont(c, d, n))
}

fn digest_info_prefix(hash_name: &str) -> Result<&'static [u8], String> {

    match hash_name {
        "SHA-1" => Ok(&[
            0x30, 0x21, 0x30, 0x09, 0x06, 0x05, 0x2b, 0x0e, 0x03, 0x02, 0x1a, 0x05, 0x00, 0x04,
            0x14,
        ]),
        "SHA-256" => Ok(&[
            0x30, 0x31, 0x30, 0x0d, 0x06, 0x09, 0x60, 0x86, 0x48, 0x01, 0x65, 0x03, 0x04, 0x02,
            0x01, 0x05, 0x00, 0x04, 0x20,
        ]),
        "SHA-384" => Ok(&[
            0x30, 0x41, 0x30, 0x0d, 0x06, 0x09, 0x60, 0x86, 0x48, 0x01, 0x65, 0x03, 0x04, 0x02,
            0x02, 0x05, 0x00, 0x04, 0x30,
        ]),
        "SHA-512" => Ok(&[
            0x30, 0x51, 0x30, 0x0d, 0x06, 0x09, 0x60, 0x86, 0x48, 0x01, 0x65, 0x03, 0x04, 0x02,
            0x03, 0x05, 0x00, 0x04, 0x40,
        ]),
        other => Err(format!("PKCS1-v1_5: unsupported hash {}", other)),
    }
}

fn emsa_pkcs1_v1_5_encode(hash: &[u8], em_len: usize, hash_name: &str) -> Result<Vec<u8>, String> {
    let prefix = digest_info_prefix(hash_name)?;
    let t_len = prefix.len() + hash.len();
    if em_len < t_len + 11 {
        return Err("PKCS1-v1_5: intended encoded message length too short".into());
    }
    let ps_len = em_len - t_len - 3;
    let mut em = Vec::with_capacity(em_len);
    em.push(0x00);
    em.push(0x01);
    em.extend(std::iter::repeat(0xffu8).take(ps_len));
    em.push(0x00);
    em.extend_from_slice(prefix);
    em.extend_from_slice(hash);
    debug_assert_eq!(em.len(), em_len);
    Ok(em)
}

pub fn rsa_pkcs1_v15_sign(
    n_bytes: &[u8],
    d_bytes: &[u8],
    hash: &[u8],
    hash_name: &str,
) -> Result<Vec<u8>, String> {
    let k = n_bytes.len();
    let em = emsa_pkcs1_v1_5_encode(hash, k, hash_name)?;
    let n = BigUInt::from_be_bytes(n_bytes);
    let d = BigUInt::from_be_bytes(d_bytes);
    let m_int = BigUInt::from_be_bytes(&em);
    let s_int = rsadp(&n, &d, &m_int)?;
    Ok(s_int.to_be_bytes(k))
}

pub fn rsa_pkcs1_v15_verify(
    n_bytes: &[u8],
    e_bytes: &[u8],
    hash: &[u8],
    signature: &[u8],
    hash_name: &str,
) -> Result<(), String> {
    let k = n_bytes.len();
    if signature.len() != k {
        return Err("PKCS1-v1_5: signature length mismatch".into());
    }
    let n = BigUInt::from_be_bytes(n_bytes);
    let e = BigUInt::from_be_bytes(e_bytes);
    let s_int = BigUInt::from_be_bytes(signature);
    let m_int = rsaep(&n, &e, &s_int)?;
    let em_recovered = m_int.to_be_bytes(k);
    let em_expected = emsa_pkcs1_v1_5_encode(hash, k, hash_name)?;
    if !timing_safe_equal(&em_recovered, &em_expected) {
        return Err("PKCS1-v1_5: signature verification failed".into());
    }
    Ok(())
}

pub fn rsa_private_encrypt_pkcs1(
    n_bytes: &[u8],
    d_bytes: &[u8],
    msg: &[u8],
) -> Result<Vec<u8>, String> {
    let k = n_bytes.len();
    if k < 11 || msg.len() > k - 11 {
        return Err("privateEncrypt: message too long for RSA key size".into());
    }
    let ps_len = k - msg.len() - 3;
    let mut em = Vec::with_capacity(k);
    em.push(0x00);
    em.push(0x01);
    em.extend(std::iter::repeat(0xFFu8).take(ps_len));
    em.push(0x00);
    em.extend_from_slice(msg);
    let n = BigUInt::from_be_bytes(n_bytes);
    let d = BigUInt::from_be_bytes(d_bytes);
    let m_int = BigUInt::from_be_bytes(&em);
    let s = rsadp(&n, &d, &m_int)?;
    Ok(s.to_be_bytes(k))
}

pub fn rsa_public_decrypt_pkcs1(
    n_bytes: &[u8],
    e_bytes: &[u8],
    ct: &[u8],
) -> Result<Vec<u8>, String> {
    let k = n_bytes.len();
    let n = BigUInt::from_be_bytes(n_bytes);
    let e = BigUInt::from_be_bytes(e_bytes);
    let c = BigUInt::from_be_bytes(ct);
    let em = rsaep(&n, &e, &c)?.to_be_bytes(k);
    if em.len() != k || em[0] != 0x00 || em[1] != 0x01 {
        return Err("publicDecrypt: invalid PKCS#1 type-1 block".into());
    }
    let mut i = 2;
    while i < em.len() && em[i] == 0xFF {
        i += 1;
    }

    if i < 10 || i >= em.len() || em[i] != 0x00 {
        return Err("publicDecrypt: invalid PKCS#1 padding".into());
    }
    Ok(em[i + 1..].to_vec())
}

fn biguint_u32(v: u32) -> BigUInt {
    BigUInt::from_limbs(vec![v])
}

fn biguint_gcd(a: &BigUInt, b: &BigUInt) -> BigUInt {
    let mut x = a.clone();
    let mut y = b.clone();
    while !y.is_zero() {
        let (_, r) = x.divmod(&y);
        x = y;
        y = r;
    }
    x
}

fn biguint_mod_inverse(a: &BigUInt, m: &BigUInt) -> Option<BigUInt> {
    let zero = BigUInt::zero();
    if m.cmp(&BigUInt::one()) != std::cmp::Ordering::Greater {
        return None;
    }
    let mut t = zero.clone();
    let mut new_t = BigUInt::one();
    let mut r = m.clone();
    let mut new_r = a.modulo(m);
    while !new_r.is_zero() {
        let (q, _) = r.divmod(&new_r);

        let qn = q.mul(&new_t).modulo(m);
        let next_t = if t.cmp(&qn) != std::cmp::Ordering::Less {
            t.sub(&qn)
        } else {

            m.sub(&qn.sub(&t).modulo(m))
        };
        t = new_t;
        new_t = next_t.modulo(m);

        let qr = q.mul(&new_r);
        let next_r = r.sub(&qr);
        r = new_r;
        new_r = next_r;
    }
    if r.cmp(&BigUInt::one()) != std::cmp::Ordering::Equal {
        return None;
    }
    Some(t.modulo(m))
}

fn is_probable_prime(n: &BigUInt, rounds: usize) -> bool {
    let one = BigUInt::one();
    let two = biguint_u32(2);
    match n.cmp(&two) {
        std::cmp::Ordering::Less => return false,
        std::cmp::Ordering::Equal => return true,
        _ => {}
    }

    if n.limbs()[0] & 1 == 0 {
        return false;
    }

    for p in [3u32, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37] {
        let bp = biguint_u32(p);
        if n.cmp(&bp) == std::cmp::Ordering::Equal {
            return true;
        }
        if n.modulo(&bp).is_zero() {
            return false;
        }
    }

    let n_minus_1 = n.sub(&one);
    let mut d = n_minus_1.clone();
    let mut s = 0usize;
    while d.limbs()[0] & 1 == 0 {
        let (q, _) = d.divmod(&two);
        d = q;
        s += 1;
    }
    let byte_len = n.to_be_bytes((n.bit_len() + 7) / 8).len().max(1);
    'witness: for _ in 0..rounds {

        let mut buf = vec![0u8; byte_len];
        if get_random_values(&mut buf).is_err() {
            return false;
        }
        let mut a = BigUInt::from_be_bytes(&buf).modulo(&n_minus_1);
        if a.cmp(&two) == std::cmp::Ordering::Less {
            a = two.clone();
        }
        let mut x = mod_pow_mont(&a, &d, n);
        if x.cmp(&one) == std::cmp::Ordering::Equal
            || x.cmp(&n_minus_1) == std::cmp::Ordering::Equal
        {
            continue 'witness;
        }
        for _ in 0..s.saturating_sub(1) {
            x = mod_pow_mont(&x, &two, n);
            if x.cmp(&n_minus_1) == std::cmp::Ordering::Equal {
                continue 'witness;
            }
        }
        return false;
    }
    true
}

fn random_prime(bits: usize) -> Result<BigUInt, String> {
    let byte_len = (bits + 7) / 8;
    loop {
        let mut buf = vec![0u8; byte_len];
        get_random_values(&mut buf).map_err(|e| format!("rng: {e}"))?;

        buf[0] |= 0b1100_0000;
        let last = byte_len - 1;
        buf[last] |= 1;
        let cand = BigUInt::from_be_bytes(&buf);
        if is_probable_prime(&cand, 40) {
            return Ok(cand);
        }
    }
}

pub fn is_probable_prime_be(bytes: &[u8], rounds: usize) -> bool {
    is_probable_prime(&BigUInt::from_be_bytes(bytes), rounds)
}

pub fn generate_safe_prime(bits: usize) -> Result<Vec<u8>, String> {
    let one = BigUInt::one();
    let two = biguint_u32(2);
    loop {
        let p_bytes = generate_dh_prime(bits)?;
        let p = BigUInt::from_be_bytes(&p_bytes);
        let (q, _) = p.sub(&one).divmod(&two);
        if is_probable_prime(&q, 40) {
            return Ok(p_bytes);
        }
    }
}

pub fn generate_dh_prime(bits: usize) -> Result<Vec<u8>, String> {
    if bits < 8 {
        return Err(format!("prime length {bits} is too small"));
    }
    let byte_len = (bits + 7) / 8;
    let p = random_prime(bits)?;
    Ok(p.to_be_bytes(byte_len))
}

pub fn rsa_generate_keypair(
    modulus_bits: usize,
    e: &BigUInt,
) -> Result<(Vec<u8>, Vec<u8>, Vec<u8>), String> {
    if modulus_bits < 512 || modulus_bits % 2 != 0 {
        return Err(format!(
            "rsa_generate_keypair: modulus_bits must be even and >= 512, got {modulus_bits}"
        ));
    }
    let one = BigUInt::one();
    let half = modulus_bits / 2;
    for _ in 0..200 {
        let p = random_prime(half)?;
        let q = random_prime(half)?;
        if p.cmp(&q) == std::cmp::Ordering::Equal {
            continue;
        }
        let n = p.mul(&q);
        if n.bit_len() != modulus_bits {
            continue;
        }

        let phi = p.sub(&one).mul(&q.sub(&one));
        if biguint_gcd(e, &phi).cmp(&one) != std::cmp::Ordering::Equal {
            continue;
        }
        let d = match biguint_mod_inverse(e, &phi) {
            Some(d) => d,
            None => continue,
        };
        let n_len = (modulus_bits + 7) / 8;
        let e_len = ((e.bit_len() + 7) / 8).max(1);
        return Ok((
            n.to_be_bytes(n_len),
            e.to_be_bytes(e_len),
            d.to_be_bytes(n_len),
        ));
    }
    Err("rsa_generate_keypair: exhausted attempts generating valid primes".into())
}

#[allow(clippy::type_complexity)]
pub fn rsa_generate_keypair_crt(
    modulus_bits: usize,
    e: &BigUInt,
) -> Result<
    (
        Vec<u8>,
        Vec<u8>,
        Vec<u8>,
        Vec<u8>,
        Vec<u8>,
        Vec<u8>,
        Vec<u8>,
        Vec<u8>,
    ),
    String,
> {
    if modulus_bits < 512 || modulus_bits % 2 != 0 {
        return Err(format!(
            "rsa_generate_keypair_crt: modulus_bits must be even and >= 512, got {modulus_bits}"
        ));
    }
    let one = BigUInt::one();
    let half = modulus_bits / 2;
    for _ in 0..200 {
        let mut p = random_prime(half)?;
        let mut q = random_prime(half)?;
        if p.cmp(&q) == std::cmp::Ordering::Equal {
            continue;
        }

        if p.cmp(&q) == std::cmp::Ordering::Less {
            std::mem::swap(&mut p, &mut q);
        }
        let n = p.mul(&q);
        if n.bit_len() != modulus_bits {
            continue;
        }
        let phi = p.sub(&one).mul(&q.sub(&one));
        if biguint_gcd(e, &phi).cmp(&one) != std::cmp::Ordering::Equal {
            continue;
        }
        let d = match biguint_mod_inverse(e, &phi) {
            Some(d) => d,
            None => continue,
        };
        let dp = d.modulo(&p.sub(&one));
        let dq = d.modulo(&q.sub(&one));
        let qinv = match biguint_mod_inverse(&q, &p) {
            Some(v) => v,
            None => continue,
        };
        let n_len = (modulus_bits + 7) / 8;
        let h_len = (half + 7) / 8;
        let e_len = ((e.bit_len() + 7) / 8).max(1);
        return Ok((
            n.to_be_bytes(n_len),
            e.to_be_bytes(e_len),
            d.to_be_bytes(n_len),
            p.to_be_bytes(h_len),
            q.to_be_bytes(h_len),
            dp.to_be_bytes(h_len),
            dq.to_be_bytes(h_len),
            qinv.to_be_bytes(h_len),
        ));
    }
    Err("rsa_generate_keypair_crt: exhausted attempts generating valid primes".into())
}

pub fn ec_public_from_private(c: &Curve, d_bytes: &[u8]) -> Option<(Vec<u8>, Vec<u8>)> {
    let d = BigUInt::from_be_bytes(d_bytes);
    match ec_scalar_mul(c, &d, &c.g) {
        P256Point::Affine { x, y } => {
            Some((x.to_be_bytes(c.coord_bytes), y.to_be_bytes(c.coord_bytes)))
        }
        _ => None,
    }
}

fn p25519() -> BigUInt {
    let mut b = [0xffu8; 32];
    b[0] = 0x7f;
    b[31] = 0xed;
    BigUInt::from_be_bytes(&b)
}

#[inline]
fn fe_add(a: &BigUInt, b: &BigUInt, p: &BigUInt) -> BigUInt {
    a.add(b).modulo(p)
}
#[inline]
fn fe_sub(a: &BigUInt, b: &BigUInt, p: &BigUInt) -> BigUInt {

    a.add(p).sub(b).modulo(p)
}
#[inline]
fn fe_mul(a: &BigUInt, b: &BigUInt, p: &BigUInt) -> BigUInt {
    a.mul(b).modulo(p)
}

pub fn x25519(k_in: &[u8], u_in: &[u8]) -> Vec<u8> {
    let p = p25519();

    let mut k = [0u8; 32];
    let n = k_in.len().min(32);
    k[..n].copy_from_slice(&k_in[..n]);
    k[0] &= 248;
    k[31] &= 127;
    k[31] |= 64;

    let mut ub = [0u8; 32];
    let m = u_in.len().min(32);
    ub[..m].copy_from_slice(&u_in[..m]);
    ub[31] &= 127;
    ub.reverse();
    let x1 = BigUInt::from_be_bytes(&ub).modulo(&p);

    let a24 = BigUInt::from_be_bytes(&[0x01, 0xDB, 0x41]);
    let mut x2 = BigUInt::one();
    let mut z2 = BigUInt::zero();
    let mut x3 = x1.clone();
    let mut z3 = BigUInt::one();
    let mut swap: u8 = 0;
    for t in (0..255usize).rev() {
        let kt = (k[t >> 3] >> (t & 7)) & 1;
        swap ^= kt;
        if swap == 1 {
            std::mem::swap(&mut x2, &mut x3);
            std::mem::swap(&mut z2, &mut z3);
        }
        swap = kt;
        let a = fe_add(&x2, &z2, &p);
        let aa = fe_mul(&a, &a, &p);
        let b = fe_sub(&x2, &z2, &p);
        let bb = fe_mul(&b, &b, &p);
        let e = fe_sub(&aa, &bb, &p);
        let c = fe_add(&x3, &z3, &p);
        let d = fe_sub(&x3, &z3, &p);
        let da = fe_mul(&d, &a, &p);
        let cb = fe_mul(&c, &b, &p);
        let s = fe_add(&da, &cb, &p);
        x3 = fe_mul(&s, &s, &p);
        let diff = fe_sub(&da, &cb, &p);
        let diff2 = fe_mul(&diff, &diff, &p);
        z3 = fe_mul(&x1, &diff2, &p);
        x2 = fe_mul(&aa, &bb, &p);
        let t3 = fe_mul(&a24, &e, &p);
        let t4 = fe_add(&aa, &t3, &p);
        z2 = fe_mul(&e, &t4, &p);
    }
    if swap == 1 {
        std::mem::swap(&mut x2, &mut x3);
        std::mem::swap(&mut z2, &mut z3);
    }

    let two = BigUInt::from_be_bytes(&[2]);
    let zinv = z2.mod_pow(&p.sub(&two), &p);
    let mut out = fe_mul(&x2, &zinv, &p).to_be_bytes(32);
    out.reverse();
    out
}

pub fn x25519_base(k: &[u8]) -> Vec<u8> {
    let mut nine = [0u8; 32];
    nine[0] = 9;
    x25519(k, &nine)
}

fn p448() -> BigUInt {
    let mut bytes = vec![0xffu8; 56];
    bytes[27] = 0xfe;
    BigUInt::from_be_bytes(&bytes)
}

pub fn x448(k_in: &[u8], u_in: &[u8]) -> Vec<u8> {
    let p = p448();
    let mut k = [0u8; 56];
    let n = k_in.len().min(56);
    k[..n].copy_from_slice(&k_in[..n]);
    k[0] &= 252;
    k[55] |= 128;
    let mut ub = [0u8; 56];
    let m = u_in.len().min(56);
    ub[..m].copy_from_slice(&u_in[..m]);
    ub.reverse();
    let x1 = BigUInt::from_be_bytes(&ub).modulo(&p);

    let a24 = BigUInt::from_be_bytes(&[0x98, 0xA9]);
    let mut x2 = BigUInt::one();
    let mut z2 = BigUInt::zero();
    let mut x3 = x1.clone();
    let mut z3 = BigUInt::one();
    let mut swap: u8 = 0;
    for t in (0..448usize).rev() {
        let kt = (k[t >> 3] >> (t & 7)) & 1;
        swap ^= kt;
        if swap == 1 {
            std::mem::swap(&mut x2, &mut x3);
            std::mem::swap(&mut z2, &mut z3);
        }
        swap = kt;
        let a = fe_add(&x2, &z2, &p);
        let aa = fe_mul(&a, &a, &p);
        let b = fe_sub(&x2, &z2, &p);
        let bb = fe_mul(&b, &b, &p);
        let e = fe_sub(&aa, &bb, &p);
        let c = fe_add(&x3, &z3, &p);
        let d = fe_sub(&x3, &z3, &p);
        let da = fe_mul(&d, &a, &p);
        let cb = fe_mul(&c, &b, &p);
        let s = fe_add(&da, &cb, &p);
        x3 = fe_mul(&s, &s, &p);
        let diff = fe_sub(&da, &cb, &p);
        let diff2 = fe_mul(&diff, &diff, &p);
        z3 = fe_mul(&x1, &diff2, &p);
        x2 = fe_mul(&aa, &bb, &p);
        let t3 = fe_mul(&a24, &e, &p);
        let t4 = fe_add(&aa, &t3, &p);
        z2 = fe_mul(&e, &t4, &p);
    }
    if swap == 1 {
        std::mem::swap(&mut x2, &mut x3);
        std::mem::swap(&mut z2, &mut z3);
    }
    let two = BigUInt::from_be_bytes(&[2]);
    let zinv = z2.mod_pow(&p.sub(&two), &p);
    let mut out = fe_mul(&x2, &zinv, &p).to_be_bytes(56);
    out.reverse();
    out
}

pub fn x448_base(k: &[u8]) -> Vec<u8> {
    let mut five = [0u8; 56];
    five[0] = 5;
    x448(k, &five)
}

#[derive(Clone)]
struct EdPt {
    x: BigUInt,
    y: BigUInt,
    z: BigUInt,
    t: BigUInt,
}

fn ed_l() -> BigUInt {
    let l_be: [u8; 32] = [
        0x10, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x14, 0xde, 0xf9, 0xde, 0xa2, 0xf7,
        0x9c, 0xd6, 0x58, 0x12, 0x63, 0x1a, 0x5c, 0xf5, 0xd3, 0xed,
    ];
    BigUInt::from_be_bytes(&l_be)
}

fn modp_inv(a: &BigUInt, p: &BigUInt) -> BigUInt {
    a.mod_pow(&p.sub(&BigUInt::from_be_bytes(&[2])), p)
}

fn ed_d(p: &BigUInt) -> BigUInt {
    let num = p.sub(&BigUInt::from_be_bytes(&[0x01, 0xDB, 0x41]));
    let den = BigUInt::from_be_bytes(&[0x01, 0xDB, 0x42]);
    fe_mul(&num, &modp_inv(&den, p), p)
}

fn ed_sqrtm1(p: &BigUInt) -> BigUInt {
    let mut exp = [0xffu8; 32];
    exp[0] = 0x1f;
    exp[31] = 0xfb;
    BigUInt::from_be_bytes(&[2]).mod_pow(&BigUInt::from_be_bytes(&exp), p)
}

fn be_is_odd(a: &BigUInt) -> bool {
    a.to_be_bytes(32)[31] & 1 == 1
}

fn neg_mod(a: &BigUInt, p: &BigUInt) -> BigUInt {
    fe_sub(&BigUInt::zero(), a, p)
}

fn ed_recover_x(y: &BigUInt, sign: u8, p: &BigUInt, d: &BigUInt) -> Option<BigUInt> {
    let one = BigUInt::one();
    let y2 = fe_mul(y, y, p);
    let u = fe_sub(&y2, &one, p);
    let v = fe_add(&fe_mul(d, &y2, p), &one, p);

    let v3 = fe_mul(&fe_mul(&v, &v, p), &v, p);
    let v7 = fe_mul(&fe_mul(&v3, &v3, p), &v, p);
    let mut exp = [0xffu8; 32];
    exp[0] = 0x0f;
    exp[31] = 0xfd;
    let pw = fe_mul(&u, &v7, p).mod_pow(&BigUInt::from_be_bytes(&exp), p);
    let mut x = fe_mul(&fe_mul(&u, &v3, p), &pw, p);

    let vx2 = fe_mul(&v, &fe_mul(&x, &x, p), p);
    if vx2.cmp(&u) != std::cmp::Ordering::Equal {
        if vx2.cmp(&neg_mod(&u, p)) == std::cmp::Ordering::Equal {
            x = fe_mul(&x, &ed_sqrtm1(p), p);
        } else {
            return None;
        }
    }
    if x.cmp(&BigUInt::zero()) == std::cmp::Ordering::Equal && sign == 1 {
        return None;
    }
    if (be_is_odd(&x) as u8) != sign {
        x = neg_mod(&x, p);
    }
    Some(x)
}

fn ed_identity() -> EdPt {
    EdPt {
        x: BigUInt::zero(),
        y: BigUInt::one(),
        z: BigUInt::one(),
        t: BigUInt::zero(),
    }
}

fn ed_base(p: &BigUInt, d: &BigUInt) -> EdPt {
    let by = fe_mul(
        &BigUInt::from_be_bytes(&[4]),
        &modp_inv(&BigUInt::from_be_bytes(&[5]), p),
        p,
    );
    let bx = ed_recover_x(&by, 0, p, d).expect("base point");
    let t = fe_mul(&bx, &by, p);
    EdPt {
        x: bx,
        y: by,
        z: BigUInt::one(),
        t,
    }
}

fn ed_add(p1: &EdPt, p2: &EdPt, p: &BigUInt, d: &BigUInt) -> EdPt {
    let a = fe_mul(&fe_sub(&p1.y, &p1.x, p), &fe_sub(&p2.y, &p2.x, p), p);
    let b = fe_mul(&fe_add(&p1.y, &p1.x, p), &fe_add(&p2.y, &p2.x, p), p);
    let two_d = fe_add(d, d, p);
    let c = fe_mul(&fe_mul(&p1.t, &two_d, p), &p2.t, p);
    let dd = fe_mul(&fe_add(&p1.z, &p1.z, p), &p2.z, p);
    let e = fe_sub(&b, &a, p);
    let f = fe_sub(&dd, &c, p);
    let g = fe_add(&dd, &c, p);
    let h = fe_add(&b, &a, p);
    EdPt {
        x: fe_mul(&e, &f, p),
        y: fe_mul(&g, &h, p),
        t: fe_mul(&e, &h, p),
        z: fe_mul(&f, &g, p),
    }
}

fn ed_scalarmult(k: &BigUInt, point: &EdPt, p: &BigUInt, d: &BigUInt) -> EdPt {
    let mut q = ed_identity();
    let kb = k.to_be_bytes(32);
    for byte in kb.iter() {
        for bit in (0..8).rev() {
            q = ed_add(&q, &q, p, d);
            if (byte >> bit) & 1 == 1 {
                q = ed_add(&q, point, p, d);
            }
        }
    }
    q
}

fn ed_encode(pt: &EdPt, p: &BigUInt) -> Vec<u8> {
    let zinv = modp_inv(&pt.z, p);
    let x = fe_mul(&pt.x, &zinv, p);
    let y = fe_mul(&pt.y, &zinv, p);
    let mut out = y.to_be_bytes(32);
    out.reverse();
    if be_is_odd(&x) {
        out[31] |= 0x80;
    }
    out
}

fn ed_decode(bytes: &[u8], p: &BigUInt, d: &BigUInt) -> Option<EdPt> {
    if bytes.len() != 32 {
        return None;
    }
    let sign = (bytes[31] >> 7) & 1;
    let mut yb = [0u8; 32];
    yb.copy_from_slice(bytes);
    yb[31] &= 0x7f;
    yb.reverse();
    let y = BigUInt::from_be_bytes(&yb);
    if y.cmp(p) != std::cmp::Ordering::Less {
        return None;
    }
    let x = ed_recover_x(&y, sign, p, d)?;
    let t = fe_mul(&x, &y, p);
    Some(EdPt {
        x,
        y,
        z: BigUInt::one(),
        t,
    })
}

fn ed_points_equal(p1: &EdPt, p2: &EdPt, p: &BigUInt) -> bool {
    let xz = fe_mul(&p1.x, &p2.z, p);
    let zx = fe_mul(&p2.x, &p1.z, p);
    let yz = fe_mul(&p1.y, &p2.z, p);
    let zy = fe_mul(&p2.y, &p1.z, p);
    xz.cmp(&zx) == std::cmp::Ordering::Equal && yz.cmp(&zy) == std::cmp::Ordering::Equal
}

fn le_decode(bytes: &[u8]) -> BigUInt {
    let mut b = bytes.to_vec();
    b.reverse();
    BigUInt::from_be_bytes(&b)
}

fn ed_clamp_scalar(h: &[u8]) -> BigUInt {
    let mut a = [0u8; 32];
    a.copy_from_slice(&h[0..32]);
    a[0] &= 248;
    a[31] &= 127;
    a[31] |= 64;
    le_decode(&a)
}

pub fn ed25519_public_key(seed: &[u8]) -> Vec<u8> {
    let p = p25519();
    let d = ed_d(&p);
    let h = digest_sha512(seed);
    let a = ed_clamp_scalar(&h);
    ed_encode(&ed_scalarmult(&a, &ed_base(&p, &d), &p, &d), &p)
}

pub fn ed25519_sign(seed: &[u8], msg: &[u8]) -> Vec<u8> {
    let p = p25519();
    let d = ed_d(&p);
    let l = ed_l();
    let base = ed_base(&p, &d);
    let h = digest_sha512(seed);
    let a = ed_clamp_scalar(&h);
    let aenc = ed_encode(&ed_scalarmult(&a, &base, &p, &d), &p);
    let prefix = &h[32..64];
    let mut rin = prefix.to_vec();
    rin.extend_from_slice(msg);
    let r = le_decode(&digest_sha512(&rin)).modulo(&l);
    let renc = ed_encode(&ed_scalarmult(&r, &base, &p, &d), &p);
    let mut kin = renc.clone();
    kin.extend_from_slice(&aenc);
    kin.extend_from_slice(msg);
    let k = le_decode(&digest_sha512(&kin)).modulo(&l);
    let s = r.add(&k.mul(&a)).modulo(&l);
    let mut senc = s.to_be_bytes(32);
    senc.reverse();
    let mut sig = renc;
    sig.extend_from_slice(&senc);
    sig
}

pub fn ed25519_verify(pubkey: &[u8], msg: &[u8], sig: &[u8]) -> bool {
    if sig.len() != 64 || pubkey.len() != 32 {
        return false;
    }
    let p = p25519();
    let d = ed_d(&p);
    let l = ed_l();
    let base = ed_base(&p, &d);
    let renc = &sig[0..32];
    let s = le_decode(&sig[32..64]);
    if s.cmp(&l) != std::cmp::Ordering::Less {
        return false;
    }
    let a_pt = match ed_decode(pubkey, &p, &d) {
        Some(v) => v,
        None => return false,
    };
    let r_pt = match ed_decode(renc, &p, &d) {
        Some(v) => v,
        None => return false,
    };
    let mut kin = renc.to_vec();
    kin.extend_from_slice(pubkey);
    kin.extend_from_slice(msg);
    let k = le_decode(&digest_sha512(&kin)).modulo(&l);
    let sb = ed_scalarmult(&s, &base, &p, &d);
    let ka = ed_scalarmult(&k, &a_pt, &p, &d);
    let rka = ed_add(&r_pt, &ka, &p, &d);
    ed_points_equal(&sb, &rka, &p)
}

fn ed448_d(p: &BigUInt) -> BigUInt {
    p.sub(&BigUInt::from_be_bytes(&[0x98, 0xA9]))
}

fn ed448_l() -> BigUInt {
    BigUInt::from_be_bytes(&[
        0x3f, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x7c, 0xca,
        0x23, 0xe9, 0xc4, 0x4e, 0xdb, 0x49, 0xae, 0xd6, 0x36, 0x90, 0x21, 0x6c, 0xc2, 0x72, 0x8d,
        0xc5, 0x8f, 0x55, 0x23, 0x78, 0xc2, 0x92, 0xab, 0x58, 0x44, 0xf3,
    ])
}

fn ed448_base(p: &BigUInt) -> EdPt {
    let gx = BigUInt::from_be_bytes(&[
        0x4f, 0x19, 0x70, 0xc6, 0x6b, 0xed, 0x0d, 0xed, 0x22, 0x1d, 0x15, 0xa6, 0x22, 0xbf, 0x36,
        0xda, 0x9e, 0x14, 0x65, 0x70, 0x47, 0x0f, 0x17, 0x67, 0xea, 0x6d, 0xe3, 0x24, 0xa3, 0xd3,
        0xa4, 0x64, 0x12, 0xae, 0x1a, 0xf7, 0x2a, 0xb6, 0x65, 0x11, 0x43, 0x3b, 0x80, 0xe1, 0x8b,
        0x00, 0x93, 0x8e, 0x26, 0x26, 0xa8, 0x2b, 0xc7, 0x0c, 0xc0, 0x5e,
    ]);
    let gy = BigUInt::from_be_bytes(&[
        0x69, 0x3f, 0x46, 0x71, 0x6e, 0xb6, 0xbc, 0x24, 0x88, 0x76, 0x20, 0x37, 0x56, 0xc9, 0xc7,
        0x62, 0x4b, 0xea, 0x73, 0x73, 0x6c, 0xa3, 0x98, 0x40, 0x87, 0x78, 0x9c, 0x1e, 0x05, 0xa0,
        0xc2, 0xd7, 0x3a, 0xd3, 0xff, 0x1c, 0xe6, 0x7c, 0x39, 0xc4, 0xfd, 0xbd, 0x13, 0x2c, 0x4e,
        0xd7, 0xc8, 0xad, 0x98, 0x08, 0x79, 0x5b, 0xf2, 0x30, 0xfa, 0x14,
    ]);
    let t = fe_mul(&gx, &gy, p);
    EdPt {
        x: gx,
        y: gy,
        z: BigUInt::one(),
        t,
    }
}

fn ed448_recover_x(y: &BigUInt, sign: u8, p: &BigUInt, d: &BigUInt) -> Option<BigUInt> {
    let one = BigUInt::one();
    let y2 = fe_mul(y, y, p);
    let u = fe_sub(&y2, &one, p);
    let v = fe_sub(&fe_mul(d, &y2, p), &one, p);
    let w = fe_mul(&u, &modp_inv(&v, p), p);
    let exp = p.add(&one).divmod(&BigUInt::from_be_bytes(&[4])).0;
    let mut x = w.mod_pow(&exp, p);
    if fe_mul(&x, &x, p).cmp(&w) != std::cmp::Ordering::Equal {
        return None;
    }
    if x.cmp(&BigUInt::zero()) == std::cmp::Ordering::Equal && sign == 1 {
        return None;
    }
    if (be_is_odd(&x) as u8) != sign {
        x = neg_mod(&x, p);
    }
    Some(x)
}

fn ed448_add(p1: &EdPt, p2: &EdPt, p: &BigUInt, d: &BigUInt) -> EdPt {
    let aa = fe_mul(&p1.x, &p2.x, p);
    let bb = fe_mul(&p1.y, &p2.y, p);
    let cc = fe_mul(&fe_mul(&p1.t, d, p), &p2.t, p);
    let dd = fe_mul(&p1.z, &p2.z, p);
    let e = fe_sub(
        &fe_mul(&fe_add(&p1.x, &p1.y, p), &fe_add(&p2.x, &p2.y, p), p),
        &fe_add(&aa, &bb, p),
        p,
    );
    let f = fe_sub(&dd, &cc, p);
    let g = fe_add(&dd, &cc, p);
    let h = fe_sub(&bb, &aa, p);
    EdPt {
        x: fe_mul(&e, &f, p),
        y: fe_mul(&g, &h, p),
        t: fe_mul(&e, &h, p),
        z: fe_mul(&f, &g, p),
    }
}

fn ed448_scalarmult(k: &BigUInt, point: &EdPt, p: &BigUInt, d: &BigUInt) -> EdPt {
    let mut q = ed_identity();
    for byte in k.to_be_bytes(57).iter() {
        for bit in (0..8).rev() {
            q = ed448_add(&q, &q, p, d);
            if (byte >> bit) & 1 == 1 {
                q = ed448_add(&q, point, p, d);
            }
        }
    }
    q
}

fn ed448_encode(pt: &EdPt, p: &BigUInt) -> Vec<u8> {
    let zinv = modp_inv(&pt.z, p);
    let x = fe_mul(&pt.x, &zinv, p);
    let y = fe_mul(&pt.y, &zinv, p);
    let mut out = y.to_be_bytes(57);
    out.reverse();
    if be_is_odd(&x) {
        out[56] |= 0x80;
    }
    out
}

fn ed448_decode(bytes: &[u8], p: &BigUInt, d: &BigUInt) -> Option<EdPt> {
    if bytes.len() != 57 {
        return None;
    }
    let sign = (bytes[56] >> 7) & 1;
    let mut yb = [0u8; 57];
    yb.copy_from_slice(bytes);
    yb[56] &= 0x7f;
    yb.reverse();
    let y = BigUInt::from_be_bytes(&yb);
    if y.cmp(p) != std::cmp::Ordering::Less {
        return None;
    }
    let x = ed448_recover_x(&y, sign, p, d)?;
    let t = fe_mul(&x, &y, p);
    Some(EdPt {
        x,
        y,
        z: BigUInt::one(),
        t,
    })
}

fn ed448_dom4() -> Vec<u8> {
    let mut v = b"SigEd448".to_vec();
    v.push(0x00);
    v.push(0x00);
    v
}

fn ed448_clamp(h: &[u8]) -> BigUInt {
    let mut a = [0u8; 57];
    a.copy_from_slice(&h[0..57]);
    a[0] &= 0xfc;
    a[56] = 0;
    a[55] |= 0x80;
    le_decode(&a)
}

pub fn ed448_public_key(seed: &[u8], shake: &dyn Fn(&[u8], usize) -> Vec<u8>) -> Vec<u8> {
    let p = p448();
    let d = ed448_d(&p);
    let h = shake(seed, 114);
    let s = ed448_clamp(&h);
    ed448_encode(&ed448_scalarmult(&s, &ed448_base(&p), &p, &d), &p)
}

pub fn ed448_sign(seed: &[u8], msg: &[u8], shake: &dyn Fn(&[u8], usize) -> Vec<u8>) -> Vec<u8> {
    let p = p448();
    let d = ed448_d(&p);
    let l = ed448_l();
    let base = ed448_base(&p);
    let h = shake(seed, 114);
    let s = ed448_clamp(&h);
    let aenc = ed448_encode(&ed448_scalarmult(&s, &base, &p, &d), &p);
    let prefix = &h[57..114];
    let dom = ed448_dom4();
    let mut rin = dom.clone();
    rin.extend_from_slice(prefix);
    rin.extend_from_slice(msg);
    let r = le_decode(&shake(&rin, 114)).modulo(&l);
    let renc = ed448_encode(&ed448_scalarmult(&r, &base, &p, &d), &p);
    let mut kin = dom.clone();
    kin.extend_from_slice(&renc);
    kin.extend_from_slice(&aenc);
    kin.extend_from_slice(msg);
    let k = le_decode(&shake(&kin, 114)).modulo(&l);
    let sval = r.add(&k.mul(&s)).modulo(&l);
    let mut senc = sval.to_be_bytes(57);
    senc.reverse();
    let mut sig = renc;
    sig.extend_from_slice(&senc);
    sig
}

pub fn ed448_verify(
    pubkey: &[u8],
    msg: &[u8],
    sig: &[u8],
    shake: &dyn Fn(&[u8], usize) -> Vec<u8>,
) -> bool {
    if sig.len() != 114 || pubkey.len() != 57 {
        return false;
    }
    let p = p448();
    let d = ed448_d(&p);
    let l = ed448_l();
    let base = ed448_base(&p);
    let renc = &sig[0..57];
    let sval = le_decode(&sig[57..114]);
    if sval.cmp(&l) != std::cmp::Ordering::Less {
        return false;
    }
    let a_pt = match ed448_decode(pubkey, &p, &d) {
        Some(v) => v,
        None => return false,
    };
    let r_pt = match ed448_decode(renc, &p, &d) {
        Some(v) => v,
        None => return false,
    };
    let dom = ed448_dom4();
    let mut kin = dom;
    kin.extend_from_slice(renc);
    kin.extend_from_slice(pubkey);
    kin.extend_from_slice(msg);
    let k = le_decode(&shake(&kin, 114)).modulo(&l);
    let sb = ed448_scalarmult(&sval, &base, &p, &d);
    let ka = ed448_scalarmult(&k, &a_pt, &p, &d);
    let rka = ed448_add(&r_pt, &ka, &p, &d);
    ed_points_equal(&sb, &rka, &p)
}

#[cfg(test)]
mod ed25519_tests {
    use super::*;

    fn hx(s: &str) -> Vec<u8> {
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
            .collect()
    }

    #[test]
    fn rfc8032_test1_pubkey_and_sig() {
        let seed = hx("9d61b19deffebc3a6b8b8e5b1d6a8c3c6e1d8c5e0e8e8c5e0e8e8c5e0e8e8c5e");

        let pk = ed25519_public_key(&seed);
        let sig = ed25519_sign(&seed, &[]);
        assert_eq!(sig.len(), 64);
        assert!(ed25519_verify(&pk, &[], &sig));

        assert!(!ed25519_verify(&pk, b"x", &sig));
    }

    #[test]
    fn sign_verify_roundtrip_and_tamper() {
        let seed = hx("833fe62409237b9d62ec77587520911e9a759cec1d19755b7da901b96dca3d42");
        let pk = ed25519_public_key(&seed);
        let msg = b"the quick brown fox";
        let sig = ed25519_sign(&seed, msg);
        assert!(ed25519_verify(&pk, msg, &sig));
        assert!(!ed25519_verify(&pk, b"the quick brown FOX", &sig));

        let pk2 = ed25519_public_key(&hx(
            "0000000000000000000000000000000000000000000000000000000000000001",
        ));
        assert!(!ed25519_verify(&pk2, msg, &sig));
    }
}

#[cfg(test)]
mod x25519_tests {
    use super::x25519;

    fn hx(s: &str) -> Vec<u8> {
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
            .collect()
    }

    #[test]
    fn rfc7748_section_5_2_vector_1() {
        let k = hx("a546e36bf0527c9d3b16154b82465edd62144c0ac1fc5a18506a2244ba449ac4");
        let u = hx("e6db6867583030db3594c1a424b15f7c726624ec26b3353b10a903a6d0ab1c4c");
        let out = x25519(&k, &u);
        assert_eq!(
            out,
            hx("c3da55379de9c6908e94ea4df28d084f32eccf03491c71f754b4075577a28552")
        );
    }

    #[test]
    fn rfc7748_section_5_2_vector_2() {
        let k = hx("4b66e9d4d1b4673c5ad22691957d6af5c11b6421e0ea01d42ca4169e7918ba0d");
        let u = hx("e5210f12786811d3f4b7959d0538ae2c31dbe7106fc03c3efc4cd549c715a493");
        let out = x25519(&k, &u);
        assert_eq!(
            out,
            hx("95cbde9476e8907d7aade45cb4b873f88b595a68799fa152e6f8f7647aac7957")
        );
    }
}

fn p256_p() -> BigUInt {
    BigUInt::from_be_bytes(&[
        0xff, 0xff, 0xff, 0xff, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0xff,
    ])
}
fn p256_n() -> BigUInt {
    BigUInt::from_be_bytes(&[
        0xff, 0xff, 0xff, 0xff, 0x00, 0x00, 0x00, 0x00, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0xbc, 0xe6, 0xfa, 0xad, 0xa7, 0x17, 0x9e, 0x84, 0xf3, 0xb9, 0xca, 0xc2, 0xfc, 0x63,
        0x25, 0x51,
    ])
}
fn p256_b() -> BigUInt {
    BigUInt::from_be_bytes(&[
        0x5a, 0xc6, 0x35, 0xd8, 0xaa, 0x3a, 0x93, 0xe7, 0xb3, 0xeb, 0xbd, 0x55, 0x76, 0x98, 0x86,
        0xbc, 0x65, 0x1d, 0x06, 0xb0, 0xcc, 0x53, 0xb0, 0xf6, 0x3b, 0xce, 0x3c, 0x3e, 0x27, 0xd2,
        0x60, 0x4b,
    ])
}
pub fn p256_g() -> P256Point {
    P256Point::Affine {
        x: BigUInt::from_be_bytes(&[
            0x6b, 0x17, 0xd1, 0xf2, 0xe1, 0x2c, 0x42, 0x47, 0xf8, 0xbc, 0xe6, 0xe5, 0x63, 0xa4,
            0x40, 0xf2, 0x77, 0x03, 0x7d, 0x81, 0x2d, 0xeb, 0x33, 0xa0, 0xf4, 0xa1, 0x39, 0x45,
            0xd8, 0x98, 0xc2, 0x96,
        ]),
        y: BigUInt::from_be_bytes(&[
            0x4f, 0xe3, 0x42, 0xe2, 0xfe, 0x1a, 0x7f, 0x9b, 0x8e, 0xe7, 0xeb, 0x4a, 0x7c, 0x0f,
            0x9e, 0x16, 0x2b, 0xce, 0x33, 0x57, 0x6b, 0x31, 0x5e, 0xce, 0xcb, 0xb6, 0x40, 0x68,
            0x37, 0xbf, 0x51, 0xf5,
        ]),
    }
}

#[derive(Clone, Debug)]
pub enum P256Point {
    Identity,
    Affine { x: BigUInt, y: BigUInt },
}

fn mod_add(a: &BigUInt, b: &BigUInt, m: &BigUInt) -> BigUInt {
    a.add(b).modulo(m)
}
fn mod_sub(a: &BigUInt, b: &BigUInt, m: &BigUInt) -> BigUInt {
    use std::cmp::Ordering;
    if a.cmp(b) != Ordering::Less {
        a.sub(b)
    } else {
        m.add(a).sub(b).modulo(m)
    }
}
fn mod_mul(a: &BigUInt, b: &BigUInt, m: &BigUInt) -> BigUInt {
    a.mul(b).modulo(m)
}

#[inline]
fn fe_add_p(a: &BigUInt, b: &BigUInt, p: &BigUInt) -> BigUInt {
    let s = a.add(b);
    if s.cmp(p) != std::cmp::Ordering::Less {
        s.sub(p)
    } else {
        s
    }
}
#[inline]
fn fe_sub_p(a: &BigUInt, b: &BigUInt, p: &BigUInt) -> BigUInt {
    if a.cmp(b) != std::cmp::Ordering::Less {
        a.sub(b)
    } else {
        p.add(a).sub(b)
    }
}
fn mod_inv_fermat(a: &BigUInt, p: &BigUInt) -> BigUInt {

    biguint_mod_inverse(a, p).expect("mod_inv_fermat: input must be invertible")
}

fn mod_inv_public_fermat(a: &BigUInt, p: &BigUInt) -> BigUInt {
    let two = BigUInt::from_be_bytes(&[2]);
    a.mod_pow(&p.sub(&two), p)
}

fn hmac_for_len(key: &[u8], message: &[u8], len: usize) -> Vec<u8> {
    match len {
        48 => hmac_sha384(key, message).to_vec(),
        64 => hmac_sha512(key, message).to_vec(),
        _ => hmac_sha256(key, message).to_vec(),
    }
}

fn ecdsa_hash_scalar(hash: &[u8], n: &BigUInt) -> BigUInt {
    let n_bits = n.bit_len();
    let hash_bits = hash.len() * 8;
    if hash_bits <= n_bits {
        return BigUInt::from_be_bytes(hash).modulo(n);
    }
    let nbytes = (n_bits + 7) / 8;
    let mut top = hash[..nbytes].to_vec();
    let extra = nbytes * 8 - n_bits;
    if extra > 0 {
        let mut carry = 0u8;
        for byte in top.iter_mut() {
            let new_carry = *byte & ((1u8 << extra) - 1);
            *byte = (*byte >> extra) | (carry << (8 - extra));
            carry = new_carry;
        }
    }
    BigUInt::from_be_bytes(&top).modulo(n)
}

fn deterministic_ecdsa_nonce(c: &Curve, d_bytes: &[u8], hash: &[u8]) -> Result<Vec<u8>, String> {
    use std::cmp::Ordering;
    let d = BigUInt::from_be_bytes(d_bytes);
    if d.is_zero() || d.cmp(&c.n) != Ordering::Less {
        return Err("ECDSA: private key out of range".into());
    }
    let hlen = match hash.len() {
        48 | 64 => hash.len(),
        _ => 32,
    };
    let x = d.to_be_bytes(c.coord_bytes);
    let h1 = ecdsa_hash_scalar(hash, &c.n).to_be_bytes(c.coord_bytes);
    let mut v = vec![0x01; hlen];
    let mut k = vec![0x00; hlen];

    let mut seed = Vec::with_capacity(v.len() + 1 + x.len() + h1.len());
    seed.extend_from_slice(&v);
    seed.push(0x00);
    seed.extend_from_slice(&x);
    seed.extend_from_slice(&h1);
    k = hmac_for_len(&k, &seed, hlen);
    v = hmac_for_len(&k, &v, hlen);

    seed.clear();
    seed.extend_from_slice(&v);
    seed.push(0x01);
    seed.extend_from_slice(&x);
    seed.extend_from_slice(&h1);
    k = hmac_for_len(&k, &seed, hlen);
    v = hmac_for_len(&k, &v, hlen);

    loop {
        let mut t = Vec::with_capacity(c.coord_bytes);
        while t.len() < c.coord_bytes {
            v = hmac_for_len(&k, &v, hlen);
            t.extend_from_slice(&v);
        }
        t.truncate(c.coord_bytes);
        let excess_bits = c.coord_bytes * 8 - c.n.bit_len();
        if excess_bits > 0 {
            t[0] &= 0xffu8 >> excess_bits;
        }
        let candidate = BigUInt::from_be_bytes(&t);
        if !candidate.is_zero() && candidate.cmp(&c.n) == Ordering::Less {
            return Ok(t);
        }
        let mut retry = Vec::with_capacity(v.len() + 1);
        retry.extend_from_slice(&v);
        retry.push(0x00);
        k = hmac_for_len(&k, &retry, hlen);
        v = hmac_for_len(&k, &v, hlen);
    }
}

static P256_R_SQ_MOD_P: OnceLock<BigUInt> = OnceLock::new();
fn p256_r_sq() -> &'static BigUInt {
    P256_R_SQ_MOD_P.get_or_init(|| {

        let mut bytes = vec![0u8; 65];
        bytes[0] = 1;
        BigUInt::from_be_bytes(&bytes).modulo(&p256_p())
    })
}

fn p256_redc(mut t: Vec<u32>) -> BigUInt {

    while t.len() < 17 {
        t.push(0);
    }
    let p = p256_p();
    let p_limbs = p.limbs();
    debug_assert_eq!(p_limbs.len(), 8, "p256_p must be 8 limbs");

    for i in 0..8 {
        let u = t[i] as u64;
        if u == 0 {
            continue;
        }
        let mut carry: u64 = 0;
        for j in 0..8 {
            let prod = u * (p_limbs[j] as u64);
            let sum = (t[i + j] as u64) + (prod & 0xFFFF_FFFF) + (carry & 0xFFFF_FFFF);
            t[i + j] = sum as u32;
            carry = (sum >> 32) + (prod >> 32) + (carry >> 32);
        }

        let mut k = 8;
        while carry > 0 && (i + k) < t.len() {
            let sum = (t[i + k] as u64) + carry;
            t[i + k] = sum as u32;
            carry = sum >> 32;
            k += 1;
        }
    }

    let limbs: Vec<u32> = t[8..].to_vec();
    let mut result = BigUInt::from_limbs(limbs.clone());

    use std::cmp::Ordering;
    if result.cmp(&p) != Ordering::Less {
        result = result.sub(&p);
    }
    result
}

pub fn p256_mont_mul(am: &BigUInt, bm: &BigUInt) -> BigUInt {
    let product = am.mul(bm);
    p256_redc(product.limbs().to_vec())
}

pub fn p256_to_mont(a: &BigUInt) -> BigUInt {
    p256_mont_mul(a, p256_r_sq())
}

pub fn p256_from_mont(am: &BigUInt) -> BigUInt {
    p256_redc(am.limbs().to_vec())
}

fn p256_solinas_reduce(t_limbs: &[u32]) -> BigUInt {

    let mut t = t_limbs.to_vec();
    while t.len() < 16 {
        t.push(0);
    }
    let p = p256_p();

    let s1 = BigUInt::from_limbs(t[0..8].to_vec());

    let s2 = BigUInt::from_limbs(vec![0, 0, 0, t[11], t[12], t[13], t[14], t[15]]);

    let s3 = BigUInt::from_limbs(vec![0, 0, 0, t[12], t[13], t[14], t[15], 0]);

    let s4 = BigUInt::from_limbs(vec![t[8], t[9], t[10], 0, 0, 0, t[14], t[15]]);

    let s5 = BigUInt::from_limbs(vec![t[9], t[10], t[11], t[13], t[14], t[15], t[13], t[8]]);

    let s6 = BigUInt::from_limbs(vec![t[11], t[12], t[13], 0, 0, 0, t[8], t[10]]);

    let s7 = BigUInt::from_limbs(vec![t[12], t[13], t[14], t[15], 0, 0, t[9], t[11]]);

    let s8 = BigUInt::from_limbs(vec![t[13], t[14], t[15], t[8], t[9], t[10], 0, t[12]]);

    let s9 = BigUInt::from_limbs(vec![t[14], t[15], 0, t[9], t[10], t[11], 0, t[13]]);

    let r = mod_add(&s1, &s2, &p);
    let r = mod_add(&r, &s2, &p);
    let r = mod_add(&r, &s3, &p);
    let r = mod_add(&r, &s3, &p);
    let r = mod_add(&r, &s4, &p);
    let r = mod_add(&r, &s5, &p);
    let r = mod_sub(&r, &s6, &p);
    let r = mod_sub(&r, &s7, &p);
    let r = mod_sub(&r, &s8, &p);
    let r = mod_sub(&r, &s9, &p);
    r
}

pub fn p256_mod_mul_solinas(a: &BigUInt, b: &BigUInt) -> BigUInt {
    let product = a.mul(b);
    p256_solinas_reduce(product.limbs())
}

fn p256_solinas_reduce_v2(t_in: &[u32]) -> BigUInt {
    let mut t = [0u32; 16];
    for (i, &l) in t_in.iter().take(16).enumerate() {
        t[i] = l;
    }
    let g = |i: usize| t[i] as i64;

    let mut col = [
        g(0) + g(8) + g(9) - g(11) - g(12) - g(13) - g(14),
        g(1) + g(9) + g(10) - g(12) - g(13) - g(14) - g(15),
        g(2) + g(10) + g(11) - g(13) - g(14) - g(15),
        g(3) + 2 * g(11) + 2 * g(12) + g(13) - g(8) - g(9) - g(15),
        g(4) + 2 * g(12) + 2 * g(13) + g(14) - g(9) - g(10),
        g(5) + 2 * g(13) + 2 * g(14) + g(15) - g(10) - g(11),
        g(6) + 2 * g(14) + 2 * g(15) + g(14) + g(13) - g(8) - g(9),
        g(7) + 2 * g(15) + g(15) + g(8) - g(10) - g(11) - g(12) - g(13),
        0i64,
    ];

    for i in 0..8 {
        let lo = (col[i] as i64).rem_euclid(1i64 << 32);
        let hi = (col[i] - lo) >> 32;
        col[i] = lo;
        col[i + 1] += hi;
    }

    let p = p256_p();
    let mut limbs8 = [0u32; 8];
    for i in 0..8 {
        limbs8[i] = col[i] as u32;
    }
    let mut result = BigUInt::from_limbs(limbs8.to_vec());

    use std::cmp::Ordering;

    let extra = col[8];
    if extra > 0 {

        let c_2_256_mod_p = BigUInt::from_be_bytes(&[
            0x00, 0x00, 0x00, 0x00, 0xff, 0xff, 0xff, 0xfe, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
            0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x01,
        ]);
        for _ in 0..extra {
            result = result.add(&c_2_256_mod_p);
        }
    } else if extra < 0 {

        for _ in 0..(-extra) {
            result = result.add(&p);
        }

        let c_2_256_mod_p = BigUInt::from_be_bytes(&[
            0x00, 0x00, 0x00, 0x00, 0xff, 0xff, 0xff, 0xfe, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
            0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x01,
        ]);
        for _ in 0..(-extra) {
            result = result.sub(&c_2_256_mod_p);
        }
    }

    while result.cmp(&p) != Ordering::Less {
        result = result.sub(&p);
    }
    result
}

pub fn p256_mod_mul_solinas_v2(a: &BigUInt, b: &BigUInt) -> BigUInt {
    let product = a.mul(b);
    p256_solinas_reduce_v2(product.limbs())
}

type Fp = [u64; 4];

#[inline]
fn fp_pack_u32(limbs: &[u32]) -> Fp {
    let mut w = [0u32; 8];
    for (i, &l) in limbs.iter().take(8).enumerate() {
        w[i] = l;
    }
    let mut r = [0u64; 4];
    for i in 0..4 {
        r[i] = w[2 * i] as u64 | (w[2 * i + 1] as u64) << 32;
    }
    r
}

fn p256_p_arr() -> &'static Fp {
    static A: OnceLock<Fp> = OnceLock::new();
    A.get_or_init(|| fp_pack_u32(p256_p().limbs()))
}

fn p256_c_2_256_arr() -> &'static Fp {
    static A: OnceLock<Fp> = OnceLock::new();
    A.get_or_init(|| {
        let c = BigUInt::from_be_bytes(&[
            0x00, 0x00, 0x00, 0x00, 0xff, 0xff, 0xff, 0xfe, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
            0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x01,
        ]);
        fp_pack_u32(c.limbs())
    })
}

fn p256_neg_c_arr() -> &'static Fp {
    static A: OnceLock<Fp> = OnceLock::new();
    A.get_or_init(|| fp_sub(p256_p_arr(), p256_c_2_256_arr()))
}

#[inline]
fn fp_geq(a: &Fp, b: &Fp) -> bool {
    for i in (0..4).rev() {
        if a[i] != b[i] {
            return a[i] > b[i];
        }
    }
    true
}

#[inline]
fn fp_is_zero(a: &Fp) -> bool {
    a.iter().all(|&l| l == 0)
}

#[inline]
fn fp_one() -> Fp {
    [1, 0, 0, 0]
}

#[inline]
fn fp_add(a: &Fp, b: &Fp) -> Fp {
    let mut r = [0u64; 4];
    let mut carry: u128 = 0;
    for i in 0..4 {
        let t = a[i] as u128 + b[i] as u128 + carry;
        r[i] = t as u64;
        carry = t >> 64;
    }

    if carry == 1 || fp_geq(&r, p256_p_arr()) {
        let p = p256_p_arr();
        let mut borrow: i128 = 0;
        for i in 0..4 {
            let t = r[i] as i128 - p[i] as i128 - borrow;
            if t < 0 {
                r[i] = (t + (1i128 << 64)) as u64;
                borrow = 1;
            } else {
                r[i] = t as u64;
                borrow = 0;
            }
        }
    }
    r
}

#[inline]
fn fp_sub(a: &Fp, b: &Fp) -> Fp {
    let mut r = [0u64; 4];
    let mut borrow: i128 = 0;
    for i in 0..4 {
        let t = a[i] as i128 - b[i] as i128 - borrow;
        if t < 0 {
            r[i] = (t + (1i128 << 64)) as u64;
            borrow = 1;
        } else {
            r[i] = t as u64;
            borrow = 0;
        }
    }
    if borrow == 1 {

        let p = p256_p_arr();
        let mut carry: u128 = 0;
        for i in 0..4 {
            let t = r[i] as u128 + p[i] as u128 + carry;
            r[i] = t as u64;
            carry = t >> 64;
        }
    }
    r
}

#[inline]
fn fp_cond_sub_p(mut r: Fp) -> Fp {
    if fp_geq(&r, p256_p_arr()) {
        let p = p256_p_arr();
        let mut borrow: i128 = 0;
        for i in 0..4 {
            let t = r[i] as i128 - p[i] as i128 - borrow;
            if t < 0 {
                r[i] = (t + (1i128 << 64)) as u64;
                borrow = 1;
            } else {
                r[i] = t as u64;
                borrow = 0;
            }
        }
    }
    r
}

#[inline]
fn fp_mul_raw(a: &Fp, b: &Fp) -> [u64; 8] {
    let mut r = [0u64; 8];
    for i in 0..4 {
        let mut carry: u128 = 0;
        for j in 0..4 {
            let cur = r[i + j] as u128 + a[i] as u128 * b[j] as u128 + carry;
            r[i + j] = cur as u64;
            carry = cur >> 64;
        }
        r[i + 4] = carry as u64;
    }
    r
}

fn fp_reduce(prod: &[u64; 8]) -> Fp {

    let mut t = [0u32; 16];
    for i in 0..8 {
        t[2 * i] = prod[i] as u32;
        t[2 * i + 1] = (prod[i] >> 32) as u32;
    }
    let g = |i: usize| t[i] as i64;

    let mut col = [
        g(0) + g(8) + g(9) - g(11) - g(12) - g(13) - g(14),
        g(1) + g(9) + g(10) - g(12) - g(13) - g(14) - g(15),
        g(2) + g(10) + g(11) - g(13) - g(14) - g(15),
        g(3) + 2 * g(11) + 2 * g(12) + g(13) - g(8) - g(9) - g(15),
        g(4) + 2 * g(12) + 2 * g(13) + g(14) - g(9) - g(10),
        g(5) + 2 * g(13) + 2 * g(14) + g(15) - g(10) - g(11),
        g(6) + 2 * g(14) + 2 * g(15) + g(14) + g(13) - g(8) - g(9),
        g(7) + 2 * g(15) + g(15) + g(8) - g(10) - g(11) - g(12) - g(13),
        0i64,
    ];
    for i in 0..8 {
        let lo = col[i].rem_euclid(1i64 << 32);
        let hi = (col[i] - lo) >> 32;
        col[i] = lo;
        col[i + 1] += hi;
    }
    let mut r32 = [0u32; 8];
    for i in 0..8 {
        r32[i] = col[i] as u32;
    }
    let extra = col[8];

    let mut acc = fp_cond_sub_p(fp_pack_u32(&r32));
    if extra > 0 {
        let c = p256_c_2_256_arr();
        for _ in 0..extra {
            acc = fp_add(&acc, c);
        }
    } else if extra < 0 {
        let nc = p256_neg_c_arr();
        for _ in 0..(-extra) {
            acc = fp_add(&acc, nc);
        }
    }
    acc
}

#[inline]
fn fp_mul(a: &Fp, b: &Fp) -> Fp {
    fp_reduce(&fp_mul_raw(a, b))
}

#[inline]
fn fp_from_big(a: &BigUInt) -> Fp {
    fp_pack_u32(a.limbs())
}

#[inline]
fn fp_to_big(a: &Fp) -> BigUInt {
    let mut limbs = vec![0u32; 8];
    for i in 0..4 {
        limbs[2 * i] = a[i] as u32;
        limbs[2 * i + 1] = (a[i] >> 32) as u32;
    }
    BigUInt::from_limbs(limbs)
}

#[doc(hidden)]
pub fn __fp_add_big(a: &BigUInt, b: &BigUInt) -> BigUInt {
    fp_to_big(&fp_add(&fp_from_big(a), &fp_from_big(b)))
}
#[doc(hidden)]
pub fn __fp_sub_big(a: &BigUInt, b: &BigUInt) -> BigUInt {
    fp_to_big(&fp_sub(&fp_from_big(a), &fp_from_big(b)))
}
#[doc(hidden)]
pub fn __fp_mul_big(a: &BigUInt, b: &BigUInt) -> BigUInt {
    fp_to_big(&fp_mul(&fp_from_big(a), &fp_from_big(b)))
}

#[derive(Clone)]
struct JacFp {
    x: Fp,
    y: Fp,
    z: Fp,
}

impl JacFp {
    fn identity() -> Self {
        JacFp {
            x: [0u64; 4],
            y: [0u64; 4],
            z: [0u64; 4],
        }
    }
    fn is_identity(&self) -> bool {
        fp_is_zero(&self.z)
    }
}

fn jac_double_fp(j: &JacFp) -> JacFp {
    if j.is_identity() {
        return j.clone();
    }
    if fp_is_zero(&j.y) {
        return JacFp::identity();
    }
    let delta = fp_mul(&j.z, &j.z);
    let gamma = fp_mul(&j.y, &j.y);
    let beta = fp_mul(&j.x, &gamma);
    let x_minus_d = fp_sub(&j.x, &delta);
    let x_plus_d = fp_add(&j.x, &delta);
    let xm_xp = fp_mul(&x_minus_d, &x_plus_d);
    let alpha = {
        let v2 = fp_add(&xm_xp, &xm_xp);
        fp_add(&v2, &xm_xp)
    };
    let alpha2 = fp_mul(&alpha, &alpha);
    let beta2 = fp_add(&beta, &beta);
    let beta4 = fp_add(&beta2, &beta2);
    let beta8 = fp_add(&beta4, &beta4);
    let x3 = fp_sub(&alpha2, &beta8);
    let y_plus_z = fp_add(&j.y, &j.z);
    let z3 = fp_sub(&fp_sub(&fp_mul(&y_plus_z, &y_plus_z), &gamma), &delta);
    let four_beta_minus_x3 = fp_sub(&beta4, &x3);
    let gamma2 = fp_mul(&gamma, &gamma);
    let g2_2 = fp_add(&gamma2, &gamma2);
    let g2_4 = fp_add(&g2_2, &g2_2);
    let g2_8 = fp_add(&g2_4, &g2_4);
    let y3 = fp_sub(&fp_mul(&alpha, &four_beta_minus_x3), &g2_8);
    JacFp {
        x: x3,
        y: y3,
        z: z3,
    }
}

fn jac_add_affine_fp(j: &JacFp, ax: &Fp, ay: &Fp) -> JacFp {
    if j.is_identity() {
        return JacFp {
            x: *ax,
            y: *ay,
            z: fp_one(),
        };
    }
    let z1z1 = fp_mul(&j.z, &j.z);
    let u2 = fp_mul(ax, &z1z1);
    let z1_cubed = fp_mul(&j.z, &z1z1);
    let s2 = fp_mul(ay, &z1_cubed);
    if u2 == j.x {
        if s2 == j.y {
            return jac_double_fp(j);
        }
        return JacFp::identity();
    }
    let h = fp_sub(&u2, &j.x);
    let r = fp_sub(&s2, &j.y);
    let h2 = fp_mul(&h, &h);
    let h3 = fp_mul(&h2, &h);
    let x1_h2 = fp_mul(&j.x, &h2);
    let two_x1_h2 = fp_add(&x1_h2, &x1_h2);
    let r2 = fp_mul(&r, &r);
    let x3 = fp_sub(&fp_sub(&r2, &h3), &two_x1_h2);
    let y3 = fp_sub(&fp_mul(&r, &fp_sub(&x1_h2, &x3)), &fp_mul(&j.y, &h3));
    let z3 = fp_mul(&j.z, &h);
    JacFp {
        x: x3,
        y: y3,
        z: z3,
    }
}

fn jac_to_affine_fp(j: &JacFp) -> P256Point {
    if j.is_identity() {
        return P256Point::Identity;
    }

    let two = BigUInt::from_be_bytes(&[2]);
    let p_minus_2 = p256_p().sub(&two);
    let mut result = fp_one();
    let mut base = j.z;
    let bits = p_minus_2.bit_len();
    for i in 0..bits {
        if p_minus_2.bit(i) {
            result = fp_mul(&result, &base);
        }
        base = fp_mul(&base, &base);
    }
    let z_inv = result;
    let z_inv2 = fp_mul(&z_inv, &z_inv);
    let z_inv3 = fp_mul(&z_inv2, &z_inv);
    P256Point::Affine {
        x: fp_to_big(&fp_mul(&j.x, &z_inv2)),
        y: fp_to_big(&fp_mul(&j.y, &z_inv3)),
    }
}

pub fn p256_scalar_mul_fp(k: &BigUInt, pt: &P256Point) -> P256Point {
    let bits = k.bit_len();
    if bits == 0 {
        return P256Point::Identity;
    }
    let (ax, ay) = match pt {
        P256Point::Identity => return P256Point::Identity,
        P256Point::Affine { x, y } => (fp_from_big(x), fp_from_big(y)),
    };
    let mut result = JacFp::identity();
    for i in (0..bits).rev() {
        result = jac_double_fp(&result);
        if k.bit(i) {
            result = jac_add_affine_fp(&result, &ax, &ay);
        }
    }
    jac_to_affine_fp(&result)
}

fn p384_p() -> BigUInt {
    BigUInt::from_be_bytes(&hex_to_bytes(
        "fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffeffffffff0000000000000000ffffffff"))
}

static P384_C_2_384: OnceLock<BigUInt> = OnceLock::new();
fn p384_c_2_384_mod_p() -> &'static BigUInt {
    P384_C_2_384.get_or_init(|| {

        let mut bytes = vec![0u8; 49];
        bytes[0] = 1;
        BigUInt::from_be_bytes(&bytes).modulo(&p384_p())
    })
}

fn p384_solinas_reduce(t_in: &[u32]) -> BigUInt {
    let mut t = [0u32; 24];
    for (i, &l) in t_in.iter().take(24).enumerate() {
        t[i] = l;
    }
    let g = |i: usize| t[i] as i64;

    let mut col: [i64; 13] = [

        g(0) + g(12) + g(21) + g(20) - g(23),

        g(1) + g(13) + g(22) + g(23) - g(12) - g(20),

        g(2) + g(14) + g(23) - g(13) - g(21),

        g(3) + g(15) + g(12) + g(20) + g(21) - g(14) - g(22) - g(23),

        g(4) + 2 * g(21) + g(16) + g(13) + g(12) + g(20) + g(22) - g(15) - 2 * g(23),

        g(5) + 2 * g(22) + g(17) + g(14) + g(13) + g(21) + g(23) - g(16),

        g(6) + 2 * g(23) + g(18) + g(15) + g(14) + g(22) - g(17),

        g(7) + g(19) + g(16) + g(15) + g(23) - g(18),

        g(8) + g(20) + g(17) + g(16) - g(19),

        g(9) + g(21) + g(18) + g(17) - g(20),

        g(10) + g(22) + g(19) + g(18) - g(21),

        g(11) + g(23) + g(20) + g(19) - g(22),
        0i64,
    ];

    for i in 0..12 {
        let lo = col[i].rem_euclid(1i64 << 32);
        let hi = (col[i] - lo) >> 32;
        col[i] = lo;
        col[i + 1] += hi;
    }

    let p = p384_p();
    let mut limbs12 = vec![0u32; 12];
    for i in 0..12 {
        limbs12[i] = col[i] as u32;
    }
    let mut result = BigUInt::from_limbs(limbs12);

    use std::cmp::Ordering;
    let extra = col[12];
    let c = p384_c_2_384_mod_p();
    if extra > 0 {
        for _ in 0..extra {
            result = result.add(c);
        }
    } else if extra < 0 {
        for _ in 0..(-extra) {
            result = result.add(&p);
        }
        for _ in 0..(-extra) {
            result = result.sub(c);
        }
    }

    while result.cmp(&p) != Ordering::Less {
        result = result.sub(&p);
    }
    result
}

pub fn p384_mod_mul_solinas(a: &BigUInt, b: &BigUInt) -> BigUInt {
    let product = a.mul(b);
    p384_solinas_reduce(product.limbs())
}

#[inline(always)]
fn sol_mul_p384(a: &BigUInt, b: &BigUInt) -> BigUInt {
    p384_mod_mul_solinas(a, b)
}

fn jac_double_solinas_p384(j: &JacPoint) -> JacPoint {
    let p = p384_p();
    if j.is_identity() {
        return j.clone();
    }
    if j.y.is_zero() {
        return JacPoint::identity();
    }
    let delta = sol_mul_p384(&j.z, &j.z);
    let gamma = sol_mul_p384(&j.y, &j.y);
    let beta = sol_mul_p384(&j.x, &gamma);
    let x_minus_d = fe_sub_p(&j.x, &delta, &p);
    let x_plus_d = fe_add_p(&j.x, &delta, &p);
    let xm_xp = sol_mul_p384(&x_minus_d, &x_plus_d);
    let alpha = {
        let v2 = fe_add_p(&xm_xp, &xm_xp, &p);
        fe_add_p(&v2, &xm_xp, &p)
    };
    let alpha2 = sol_mul_p384(&alpha, &alpha);
    let beta2 = fe_add_p(&beta, &beta, &p);
    let beta4 = fe_add_p(&beta2, &beta2, &p);
    let beta8 = fe_add_p(&beta4, &beta4, &p);
    let x3 = fe_sub_p(&alpha2, &beta8, &p);
    let y_plus_z = fe_add_p(&j.y, &j.z, &p);
    let z3 = fe_sub_p(
        &fe_sub_p(&sol_mul_p384(&y_plus_z, &y_plus_z), &gamma, &p),
        &delta,
        &p,
    );
    let four_beta_minus_x3 = fe_sub_p(&beta4, &x3, &p);
    let gamma2 = sol_mul_p384(&gamma, &gamma);
    let g2_2 = fe_add_p(&gamma2, &gamma2, &p);
    let g2_4 = fe_add_p(&g2_2, &g2_2, &p);
    let g2_8 = fe_add_p(&g2_4, &g2_4, &p);
    let y3 = fe_sub_p(&sol_mul_p384(&alpha, &four_beta_minus_x3), &g2_8, &p);
    JacPoint {
        x: x3,
        y: y3,
        z: z3,
    }
}

fn jac_add_affine_solinas_p384(j: &JacPoint, a: &P256Point) -> JacPoint {
    use std::cmp::Ordering;
    let p = p384_p();
    let (ax, ay) = match a {
        P256Point::Identity => return j.clone(),
        P256Point::Affine { x, y } => (x, y),
    };
    if j.is_identity() {
        return JacPoint {
            x: ax.clone(),
            y: ay.clone(),
            z: BigUInt::one(),
        };
    }
    let z1z1 = sol_mul_p384(&j.z, &j.z);
    let u2 = sol_mul_p384(ax, &z1z1);
    let z1_cubed = sol_mul_p384(&j.z, &z1z1);
    let s2 = sol_mul_p384(ay, &z1_cubed);
    if u2.cmp(&j.x) == Ordering::Equal {
        if s2.cmp(&j.y) == Ordering::Equal {
            return jac_double_solinas_p384(j);
        }
        return JacPoint::identity();
    }
    let h = fe_sub_p(&u2, &j.x, &p);
    let r = fe_sub_p(&s2, &j.y, &p);
    let h2 = sol_mul_p384(&h, &h);
    let h3 = sol_mul_p384(&h2, &h);
    let x1_h2 = sol_mul_p384(&j.x, &h2);
    let two_x1_h2 = fe_add_p(&x1_h2, &x1_h2, &p);
    let r2 = sol_mul_p384(&r, &r);
    let x3 = fe_sub_p(&fe_sub_p(&r2, &h3, &p), &two_x1_h2, &p);
    let y3 = fe_sub_p(
        &sol_mul_p384(&r, &fe_sub_p(&x1_h2, &x3, &p)),
        &sol_mul_p384(&j.y, &h3),
        &p,
    );
    let z3 = sol_mul_p384(&j.z, &h);
    JacPoint {
        x: x3,
        y: y3,
        z: z3,
    }
}

fn jac_to_affine_solinas_p384(j: &JacPoint) -> P256Point {
    if j.is_identity() {
        return P256Point::Identity;
    }
    let p = p384_p();
    let two = BigUInt::from_be_bytes(&[2]);
    let p_minus_2 = p.sub(&two);
    let mut result = BigUInt::one();
    let mut base = j.z.clone();
    let bits = p_minus_2.bit_len();
    for i in 0..bits {
        if p_minus_2.bit(i) {
            result = sol_mul_p384(&result, &base);
        }
        base = sol_mul_p384(&base, &base);
    }
    let z_inv = result;
    let z_inv2 = sol_mul_p384(&z_inv, &z_inv);
    let z_inv3 = sol_mul_p384(&z_inv2, &z_inv);
    P256Point::Affine {
        x: sol_mul_p384(&j.x, &z_inv2),
        y: sol_mul_p384(&j.y, &z_inv3),
    }
}

pub fn p384_scalar_mul_solinas(k: &BigUInt, pt: &P256Point) -> P256Point {
    let bits = k.bit_len();
    if bits == 0 {
        return P256Point::Identity;
    }
    if matches!(pt, P256Point::Identity) {
        return P256Point::Identity;
    }
    let mut result = JacPoint::identity();
    for i in (0..bits).rev() {
        result = jac_double_solinas_p384(&result);
        if k.bit(i) {
            result = jac_add_affine_solinas_p384(&result, pt);
        }
    }
    jac_to_affine_solinas_p384(&result)
}

#[inline(always)]
fn sol_mul(a: &BigUInt, b: &BigUInt) -> BigUInt {
    p256_mod_mul_solinas_v2(a, b)
}

fn jac_double_solinas(j: &JacPoint) -> JacPoint {
    let p = p256_p();
    if j.is_identity() {
        return j.clone();
    }
    if j.y.is_zero() {
        return JacPoint::identity();
    }
    let delta = sol_mul(&j.z, &j.z);
    let gamma = sol_mul(&j.y, &j.y);
    let beta = sol_mul(&j.x, &gamma);
    let x_minus_d = fe_sub_p(&j.x, &delta, &p);
    let x_plus_d = fe_add_p(&j.x, &delta, &p);
    let xm_xp = sol_mul(&x_minus_d, &x_plus_d);

    let alpha = {
        let v2 = fe_add_p(&xm_xp, &xm_xp, &p);
        fe_add_p(&v2, &xm_xp, &p)
    };
    let alpha2 = sol_mul(&alpha, &alpha);
    let beta2 = fe_add_p(&beta, &beta, &p);
    let beta4 = fe_add_p(&beta2, &beta2, &p);
    let beta8 = fe_add_p(&beta4, &beta4, &p);
    let x3 = fe_sub_p(&alpha2, &beta8, &p);
    let y_plus_z = fe_add_p(&j.y, &j.z, &p);
    let z3 = fe_sub_p(
        &fe_sub_p(&sol_mul(&y_plus_z, &y_plus_z), &gamma, &p),
        &delta,
        &p,
    );
    let four_beta_minus_x3 = fe_sub_p(&beta4, &x3, &p);
    let gamma2 = sol_mul(&gamma, &gamma);
    let g2_2 = fe_add_p(&gamma2, &gamma2, &p);
    let g2_4 = fe_add_p(&g2_2, &g2_2, &p);
    let g2_8 = fe_add_p(&g2_4, &g2_4, &p);
    let y3 = fe_sub_p(&sol_mul(&alpha, &four_beta_minus_x3), &g2_8, &p);
    JacPoint {
        x: x3,
        y: y3,
        z: z3,
    }
}

fn jac_add_affine_solinas(j: &JacPoint, a: &P256Point) -> JacPoint {
    use std::cmp::Ordering;
    let p = p256_p();
    let (ax, ay) = match a {
        P256Point::Identity => return j.clone(),
        P256Point::Affine { x, y } => (x, y),
    };
    if j.is_identity() {

        return JacPoint {
            x: ax.clone(),
            y: ay.clone(),
            z: BigUInt::one(),
        };
    }
    let z1z1 = sol_mul(&j.z, &j.z);
    let u2 = sol_mul(ax, &z1z1);
    let z1_cubed = sol_mul(&j.z, &z1z1);
    let s2 = sol_mul(ay, &z1_cubed);
    if u2.cmp(&j.x) == Ordering::Equal {
        if s2.cmp(&j.y) == Ordering::Equal {
            return jac_double_solinas(j);
        }
        return JacPoint::identity();
    }
    let h = fe_sub_p(&u2, &j.x, &p);
    let r = fe_sub_p(&s2, &j.y, &p);
    let h2 = sol_mul(&h, &h);
    let h3 = sol_mul(&h2, &h);
    let x1_h2 = sol_mul(&j.x, &h2);
    let two_x1_h2 = fe_add_p(&x1_h2, &x1_h2, &p);
    let r2 = sol_mul(&r, &r);
    let x3 = fe_sub_p(&fe_sub_p(&r2, &h3, &p), &two_x1_h2, &p);
    let y3 = fe_sub_p(
        &sol_mul(&r, &fe_sub_p(&x1_h2, &x3, &p)),
        &sol_mul(&j.y, &h3),
        &p,
    );
    let z3 = sol_mul(&j.z, &h);
    JacPoint {
        x: x3,
        y: y3,
        z: z3,
    }
}

fn jac_to_affine_solinas(j: &JacPoint) -> P256Point {
    if j.is_identity() {
        return P256Point::Identity;
    }

    let p = p256_p();
    let two = BigUInt::from_be_bytes(&[2]);
    let p_minus_2 = p.sub(&two);
    let mut result = BigUInt::one();
    let mut base = j.z.clone();
    let bits = p_minus_2.bit_len();
    for i in 0..bits {
        if p_minus_2.bit(i) {
            result = sol_mul(&result, &base);
        }
        base = sol_mul(&base, &base);
    }
    let z_inv = result;
    let z_inv2 = sol_mul(&z_inv, &z_inv);
    let z_inv3 = sol_mul(&z_inv2, &z_inv);
    P256Point::Affine {
        x: sol_mul(&j.x, &z_inv2),
        y: sol_mul(&j.y, &z_inv3),
    }
}

pub fn p256_scalar_mul_solinas(k: &BigUInt, pt: &P256Point) -> P256Point {
    let bits = k.bit_len();
    if bits == 0 {
        return P256Point::Identity;
    }
    if matches!(pt, P256Point::Identity) {
        return P256Point::Identity;
    }
    let mut result = JacPoint::identity();
    for i in (0..bits).rev() {
        result = jac_double_solinas(&result);
        if k.bit(i) {
            result = jac_add_affine_solinas(&result, pt);
        }
    }
    jac_to_affine_solinas(&result)
}

pub fn p256_scalar_mul_base_solinas(k: &BigUInt) -> P256Point {
    let bits = k.bit_len();
    if bits == 0 {
        return P256Point::Identity;
    }
    let table = p256_base_table();
    let mut result = JacPoint::identity();
    for i in 0..bits {
        if k.bit(i) {
            result = jac_add_affine_solinas(&result, &table[i]);
        }
    }
    jac_to_affine_solinas(&result)
}

pub struct MontCtx {
    p: BigUInt,
    k: usize,
    m_prime: u32,
    r_sq_mod_p: BigUInt,
}

impl MontCtx {

    pub fn for_modulus(p: &BigUInt) -> Self {
        let p_limbs = p.limbs();
        assert!(!p_limbs.is_empty(), "MontCtx: modulus is zero");
        assert!(p_limbs[0] & 1 == 1, "MontCtx: modulus must be odd");
        let k = p_limbs.len();

        let p0 = p_limbs[0];
        let mut x: u32 = 1;
        for _ in 0..6 {
            x = x.wrapping_mul(2u32.wrapping_sub(p0.wrapping_mul(x)));
        }
        let m_prime = 0u32.wrapping_sub(x);

        let mut r_sq_bytes = vec![0u8; 8 * k + 1];
        r_sq_bytes[0] = 1;
        let r_sq = BigUInt::from_be_bytes(&r_sq_bytes).modulo(p);
        MontCtx {
            p: p.clone(),
            k,
            m_prime,
            r_sq_mod_p: r_sq,
        }
    }
}

pub fn mont_redc(mut t: Vec<u32>, ctx: &MontCtx) -> BigUInt {
    while t.len() < 2 * ctx.k + 1 {
        t.push(0);
    }
    let p_limbs = ctx.p.limbs();
    let m_prime = ctx.m_prime as u64;
    for i in 0..ctx.k {

        let u = ((t[i] as u64).wrapping_mul(m_prime)) & 0xFFFF_FFFF;
        if u == 0 {
            continue;
        }
        let mut carry: u64 = 0;
        for j in 0..ctx.k {
            let prod = u * (p_limbs[j] as u64);
            let sum = (t[i + j] as u64) + (prod & 0xFFFF_FFFF) + (carry & 0xFFFF_FFFF);
            t[i + j] = sum as u32;
            carry = (sum >> 32) + (prod >> 32) + (carry >> 32);
        }
        let mut kk = ctx.k;
        while carry > 0 && (i + kk) < t.len() {
            let sum = (t[i + kk] as u64) + carry;
            t[i + kk] = sum as u32;
            carry = sum >> 32;
            kk += 1;
        }
    }
    let mut result = BigUInt::from_limbs(t[ctx.k..].to_vec());
    use std::cmp::Ordering;
    if result.cmp(&ctx.p) != Ordering::Less {
        result = result.sub(&ctx.p);
    }
    result
}

pub fn mont_mul(am: &BigUInt, bm: &BigUInt, ctx: &MontCtx) -> BigUInt {

    mont_mul_cios_u128(am, bm, ctx)
}

fn mont_mul_cios_u128(am: &BigUInt, bm: &BigUInt, ctx: &MontCtx) -> BigUInt {
    let k = ctx.k;
    let p_limbs = ctx.p.limbs();
    let m_prime = ctx.m_prime as u64;
    let mut t = vec![0u32; 2 * k + 2];

    let a_limbs = am.limbs();
    let b_limbs = bm.limbs();

    for i in 0..k {
        let b_i = (*b_limbs.get(i).unwrap_or(&0)) as u128;

        let mut acc: u128 = 0;
        for j in 0..k {
            let a_j = (*a_limbs.get(j).unwrap_or(&0)) as u128;
            acc += (t[i + j] as u128) + a_j * b_i;
            t[i + j] = acc as u32;
            acc >>= 32;
        }
        let mut kk = k;
        while acc > 0 && i + kk < t.len() {
            acc += t[i + kk] as u128;
            t[i + kk] = acc as u32;
            acc >>= 32;
            kk += 1;
        }

        let u = ((t[i] as u64).wrapping_mul(m_prime)) & 0xFFFF_FFFF;

        let u128_u = u as u128;
        let mut acc: u128 = 0;
        for j in 0..k {
            let p_j = p_limbs[j] as u128;
            acc += (t[i + j] as u128) + p_j * u128_u;
            t[i + j] = acc as u32;
            acc >>= 32;
        }
        let mut kk = k;
        while acc > 0 && i + kk < t.len() {
            acc += t[i + kk] as u128;
            t[i + kk] = acc as u32;
            acc >>= 32;
            kk += 1;
        }
    }

    let mut result = BigUInt::from_limbs(t[k..].to_vec());
    use std::cmp::Ordering;
    if result.cmp(&ctx.p) != Ordering::Less {
        result = result.sub(&ctx.p);
    }
    result
}

#[allow(dead_code)]
fn mont_mul_cios(am: &BigUInt, bm: &BigUInt, ctx: &MontCtx) -> BigUInt {
    let k = ctx.k;
    let p_limbs = ctx.p.limbs();
    let m_prime = ctx.m_prime as u64;
    let mut t = vec![0u32; 2 * k + 2];

    let a_limbs = am.limbs();
    let b_limbs = bm.limbs();

    for i in 0..k {
        let b_i = (*b_limbs.get(i).unwrap_or(&0)) as u64;

        let mut carry: u64 = 0;
        for j in 0..k {
            let a_j = (*a_limbs.get(j).unwrap_or(&0)) as u64;
            let s = (t[i + j] as u64) + a_j * b_i + carry;
            t[i + j] = s as u32;
            carry = s >> 32;
        }
        let mut kk = k;
        while carry > 0 && i + kk < t.len() {
            let s = (t[i + kk] as u64) + carry;
            t[i + kk] = s as u32;
            carry = s >> 32;
            kk += 1;
        }

        let u = ((t[i] as u64).wrapping_mul(m_prime)) & 0xFFFF_FFFF;

        let mut carry: u64 = 0;
        for j in 0..k {
            let p_j = p_limbs[j] as u64;
            let s = (t[i + j] as u64) + u * p_j + carry;
            t[i + j] = s as u32;
            carry = s >> 32;
        }
        let mut kk = k;
        while carry > 0 && i + kk < t.len() {
            let s = (t[i + kk] as u64) + carry;
            t[i + kk] = s as u32;
            carry = s >> 32;
            kk += 1;
        }

    }

    let mut result = BigUInt::from_limbs(t[k..].to_vec());
    use std::cmp::Ordering;
    if result.cmp(&ctx.p) != Ordering::Less {
        result = result.sub(&ctx.p);
    }
    result
}

pub fn mont_to(a: &BigUInt, ctx: &MontCtx) -> BigUInt {
    mont_mul(a, &ctx.r_sq_mod_p, ctx)
}

pub fn mont_from(am: &BigUInt, ctx: &MontCtx) -> BigUInt {
    mont_redc(am.limbs().to_vec(), ctx)
}

pub fn mod_pow_mont(base: &BigUInt, e: &BigUInt, m: &BigUInt) -> BigUInt {
    let ctx = MontCtx::for_modulus(m);
    let base_mont = mont_to(&base.modulo(m), &ctx);
    let one_mont = mont_to(&BigUInt::one(), &ctx);
    let mut result = one_mont;
    let mut b = base_mont;
    let bits = e.bit_len();
    for i in 0..bits {
        if e.bit(i) {
            result = mont_mul(&result, &b, &ctx);
        }
        b = mont_mul(&b, &b, &ctx);
    }
    mont_from(&result, &ctx)
}

static P256_MONT_ONE: OnceLock<BigUInt> = OnceLock::new();
fn p256_mont_one() -> &'static BigUInt {
    P256_MONT_ONE.get_or_init(|| p256_to_mont(&BigUInt::one()))
}

fn jacpoint_from_affine_mont(a: &P256Point) -> JacPoint {
    match a {
        P256Point::Identity => JacPoint::identity(),
        P256Point::Affine { x, y } => JacPoint {
            x: x.clone(),
            y: y.clone(),
            z: p256_mont_one().clone(),
        },
    }
}

fn p256_mont_mul_by_small(am: &BigUInt, k: u32) -> BigUInt {
    let p = p256_p();
    let mut acc = am.clone();
    for _ in 1..k {
        acc = mod_add(&acc, am, &p);
    }
    acc
}

fn p256_mont_pow(am: &BigUInt, e: &BigUInt) -> BigUInt {

    let one_mont = p256_to_mont(&BigUInt::one());
    let mut result = one_mont;
    let mut base = am.clone();
    let bits = e.bit_len();
    for i in 0..bits {
        if e.bit(i) {
            result = p256_mont_mul(&result, &base);
        }
        base = p256_mont_mul(&base, &base);
    }
    result
}

pub fn p256_mont_inv(am: &BigUInt) -> BigUInt {
    let p = p256_p();
    let two = BigUInt::from_be_bytes(&[2]);
    let p_minus_2 = p.sub(&two);
    p256_mont_pow(am, &p_minus_2)
}

fn p256_jac_double_mont(j: &JacPoint) -> JacPoint {
    let p = p256_p();
    if j.is_identity() {
        return j.clone();
    }
    if j.y.is_zero() {
        return JacPoint::identity();
    }
    let delta = p256_mont_mul(&j.z, &j.z);
    let gamma = p256_mont_mul(&j.y, &j.y);
    let beta = p256_mont_mul(&j.x, &gamma);
    let x_minus_d = mod_sub(&j.x, &delta, &p);
    let x_plus_d = mod_add(&j.x, &delta, &p);
    let xm_xp = p256_mont_mul(&x_minus_d, &x_plus_d);
    let alpha = p256_mont_mul_by_small(&xm_xp, 3);
    let alpha2 = p256_mont_mul(&alpha, &alpha);
    let eight_beta = p256_mont_mul_by_small(&beta, 8);
    let x3 = mod_sub(&alpha2, &eight_beta, &p);
    let y_plus_z = mod_add(&j.y, &j.z, &p);
    let z3 = mod_sub(
        &mod_sub(&p256_mont_mul(&y_plus_z, &y_plus_z), &gamma, &p),
        &delta,
        &p,
    );
    let four_beta = p256_mont_mul_by_small(&beta, 4);
    let four_beta_minus_x3 = mod_sub(&four_beta, &x3, &p);
    let gamma2 = p256_mont_mul(&gamma, &gamma);
    let eight_gamma2 = p256_mont_mul_by_small(&gamma2, 8);
    let y3 = mod_sub(
        &p256_mont_mul(&alpha, &four_beta_minus_x3),
        &eight_gamma2,
        &p,
    );
    JacPoint {
        x: x3,
        y: y3,
        z: z3,
    }
}

fn p256_jac_add_affine_mont(j: &JacPoint, a: &P256Point) -> JacPoint {
    use std::cmp::Ordering;
    let p = p256_p();
    let (ax, ay) = match a {
        P256Point::Identity => return j.clone(),
        P256Point::Affine { x, y } => (x, y),
    };
    if j.is_identity() {
        return jacpoint_from_affine_mont(a);
    }
    let z1z1 = p256_mont_mul(&j.z, &j.z);
    let u2 = p256_mont_mul(ax, &z1z1);
    let z1_cubed = p256_mont_mul(&j.z, &z1z1);
    let s2 = p256_mont_mul(ay, &z1_cubed);
    if u2.cmp(&j.x) == Ordering::Equal {
        if s2.cmp(&j.y) == Ordering::Equal {
            return p256_jac_double_mont(j);
        }
        return JacPoint::identity();
    }
    let h = mod_sub(&u2, &j.x, &p);
    let r = mod_sub(&s2, &j.y, &p);
    let h2 = p256_mont_mul(&h, &h);
    let h3 = p256_mont_mul(&h2, &h);
    let x1_h2 = p256_mont_mul(&j.x, &h2);
    let two_x1_h2 = p256_mont_mul_by_small(&x1_h2, 2);
    let r2 = p256_mont_mul(&r, &r);
    let x3 = mod_sub(&mod_sub(&r2, &h3, &p), &two_x1_h2, &p);
    let y3 = mod_sub(
        &p256_mont_mul(&r, &mod_sub(&x1_h2, &x3, &p)),
        &p256_mont_mul(&j.y, &h3),
        &p,
    );
    let z3 = p256_mont_mul(&j.z, &h);
    JacPoint {
        x: x3,
        y: y3,
        z: z3,
    }
}

fn p256_jac_to_affine_mont(j: &JacPoint) -> P256Point {
    if j.is_identity() {
        return P256Point::Identity;
    }
    let z_inv_m = p256_mont_inv(&j.z);
    let z_inv2_m = p256_mont_mul(&z_inv_m, &z_inv_m);
    let z_inv3_m = p256_mont_mul(&z_inv2_m, &z_inv_m);
    let x_m = p256_mont_mul(&j.x, &z_inv2_m);
    let y_m = p256_mont_mul(&j.y, &z_inv3_m);
    P256Point::Affine {
        x: p256_from_mont(&x_m),
        y: p256_from_mont(&y_m),
    }
}

fn p256_affine_to_mont(p: &P256Point) -> P256Point {
    match p {
        P256Point::Identity => P256Point::Identity,
        P256Point::Affine { x, y } => P256Point::Affine {
            x: p256_to_mont(x),
            y: p256_to_mont(y),
        },
    }
}

static P256_BASE_TABLE_MONT: OnceLock<Vec<P256Point>> = OnceLock::new();
fn p256_base_table_mont() -> &'static [P256Point] {
    P256_BASE_TABLE_MONT.get_or_init(|| p256_base_table().iter().map(p256_affine_to_mont).collect())
}

pub fn p256_scalar_mul_base_mont(k: &BigUInt) -> P256Point {
    let bits = k.bit_len();
    if bits == 0 {
        return P256Point::Identity;
    }
    let table = p256_base_table_mont();
    let mut result = JacPoint::identity();
    for i in 0..bits {
        if k.bit(i) {
            result = p256_jac_add_affine_mont(&result, &table[i]);
        }
    }
    p256_jac_to_affine_mont(&result)
}

static MONT_CTX_P256: OnceLock<MontCtx> = OnceLock::new();
static MONT_CTX_P384: OnceLock<MontCtx> = OnceLock::new();
static MONT_CTX_P521: OnceLock<MontCtx> = OnceLock::new();

fn mont_ctx_for_curve(c: &Curve) -> &'static MontCtx {
    match c.coord_bytes {
        32 => MONT_CTX_P256.get_or_init(|| MontCtx::for_modulus(&c.p)),
        48 => MONT_CTX_P384.get_or_init(|| MontCtx::for_modulus(&c.p)),
        66 => MONT_CTX_P521.get_or_init(|| MontCtx::for_modulus(&c.p)),
        _ => panic!(
            "mont_ctx_for_curve: unsupported coord_bytes {}",
            c.coord_bytes
        ),
    }
}

fn jacpoint_from_affine_mont_g(ctx: &MontCtx, a: &P256Point) -> JacPoint {
    match a {
        P256Point::Identity => JacPoint::identity(),
        P256Point::Affine { x, y } => JacPoint {
            x: x.clone(),
            y: y.clone(),
            z: mont_to(&BigUInt::one(), ctx),
        },
    }
}

fn jac_double_mont_g(ctx: &MontCtx, j: &JacPoint) -> JacPoint {
    if j.is_identity() {
        return j.clone();
    }
    if j.y.is_zero() {
        return JacPoint::identity();
    }
    let p = &ctx.p;
    let delta = mont_mul(&j.z, &j.z, ctx);
    let gamma = mont_mul(&j.y, &j.y, ctx);
    let beta = mont_mul(&j.x, &gamma, ctx);
    let x_minus_d = mod_sub(&j.x, &delta, p);
    let x_plus_d = mod_add(&j.x, &delta, p);
    let xm_xp = mont_mul(&x_minus_d, &x_plus_d, ctx);

    let alpha = {
        let v2 = mod_add(&xm_xp, &xm_xp, p);
        mod_add(&v2, &xm_xp, p)
    };
    let alpha2 = mont_mul(&alpha, &alpha, ctx);

    let beta2 = mod_add(&beta, &beta, p);
    let beta4 = mod_add(&beta2, &beta2, p);
    let beta8 = mod_add(&beta4, &beta4, p);
    let x3 = mod_sub(&alpha2, &beta8, p);
    let y_plus_z = mod_add(&j.y, &j.z, p);
    let z3 = mod_sub(
        &mod_sub(&mont_mul(&y_plus_z, &y_plus_z, ctx), &gamma, p),
        &delta,
        p,
    );
    let four_beta_minus_x3 = mod_sub(&beta4, &x3, p);
    let gamma2 = mont_mul(&gamma, &gamma, ctx);
    let g2_2 = mod_add(&gamma2, &gamma2, p);
    let g2_4 = mod_add(&g2_2, &g2_2, p);
    let g2_8 = mod_add(&g2_4, &g2_4, p);
    let y3 = mod_sub(&mont_mul(&alpha, &four_beta_minus_x3, ctx), &g2_8, p);
    JacPoint {
        x: x3,
        y: y3,
        z: z3,
    }
}

fn jac_add_affine_mont_g(ctx: &MontCtx, j: &JacPoint, a_mont: &P256Point) -> JacPoint {
    use std::cmp::Ordering;
    let p = &ctx.p;
    let (ax, ay) = match a_mont {
        P256Point::Identity => return j.clone(),
        P256Point::Affine { x, y } => (x, y),
    };
    if j.is_identity() {
        return jacpoint_from_affine_mont_g(ctx, a_mont);
    }
    let z1z1 = mont_mul(&j.z, &j.z, ctx);
    let u2 = mont_mul(ax, &z1z1, ctx);
    let z1_cubed = mont_mul(&j.z, &z1z1, ctx);
    let s2 = mont_mul(ay, &z1_cubed, ctx);
    if u2.cmp(&j.x) == Ordering::Equal {
        if s2.cmp(&j.y) == Ordering::Equal {
            return jac_double_mont_g(ctx, j);
        }
        return JacPoint::identity();
    }
    let h = mod_sub(&u2, &j.x, p);
    let r = mod_sub(&s2, &j.y, p);
    let h2 = mont_mul(&h, &h, ctx);
    let h3 = mont_mul(&h2, &h, ctx);
    let x1_h2 = mont_mul(&j.x, &h2, ctx);
    let two_x1_h2 = mod_add(&x1_h2, &x1_h2, p);
    let r2 = mont_mul(&r, &r, ctx);
    let x3 = mod_sub(&mod_sub(&r2, &h3, p), &two_x1_h2, p);
    let y3 = mod_sub(
        &mont_mul(&r, &mod_sub(&x1_h2, &x3, p), ctx),
        &mont_mul(&j.y, &h3, ctx),
        p,
    );
    let z3 = mont_mul(&j.z, &h, ctx);
    JacPoint {
        x: x3,
        y: y3,
        z: z3,
    }
}

fn jac_to_affine_mont_g(ctx: &MontCtx, j: &JacPoint) -> P256Point {
    if j.is_identity() {
        return P256Point::Identity;
    }

    let two = BigUInt::from_be_bytes(&[2]);
    let p_minus_2 = ctx.p.sub(&two);

    let one_mont = mont_to(&BigUInt::one(), ctx);
    let mut z_inv_m = one_mont;
    let mut base = j.z.clone();
    let bits = p_minus_2.bit_len();
    for i in 0..bits {
        if p_minus_2.bit(i) {
            z_inv_m = mont_mul(&z_inv_m, &base, ctx);
        }
        base = mont_mul(&base, &base, ctx);
    }
    let z_inv2_m = mont_mul(&z_inv_m, &z_inv_m, ctx);
    let z_inv3_m = mont_mul(&z_inv2_m, &z_inv_m, ctx);
    let x_m = mont_mul(&j.x, &z_inv2_m, ctx);
    let y_m = mont_mul(&j.y, &z_inv3_m, ctx);
    P256Point::Affine {
        x: mont_from(&x_m, ctx),
        y: mont_from(&y_m, ctx),
    }
}

pub fn ec_scalar_mul_mont_g(c: &Curve, k: &BigUInt, pt_std: &P256Point) -> P256Point {
    let bits = k.bit_len();
    if bits == 0 {
        return P256Point::Identity;
    }
    if matches!(pt_std, P256Point::Identity) {
        return P256Point::Identity;
    }
    let ctx = mont_ctx_for_curve(c);
    let pt_mont = match pt_std {
        P256Point::Affine { x, y } => P256Point::Affine {
            x: mont_to(x, ctx),
            y: mont_to(y, ctx),
        },
        P256Point::Identity => unreachable!(),
    };
    let mut result = JacPoint::identity();
    for i in (0..bits).rev() {
        result = jac_double_mont_g(ctx, &result);
        if k.bit(i) {
            result = jac_add_affine_mont_g(ctx, &result, &pt_mont);
        }
    }
    jac_to_affine_mont_g(ctx, &result)
}

pub fn p256_scalar_mul_mont(k: &BigUInt, q_std: &P256Point) -> P256Point {
    let bits = k.bit_len();
    if bits == 0 {
        return P256Point::Identity;
    }
    if matches!(q_std, P256Point::Identity) {
        return P256Point::Identity;
    }
    let q_mont = p256_affine_to_mont(q_std);
    let mut result = JacPoint::identity();
    for i in (0..bits).rev() {
        result = p256_jac_double_mont(&result);
        if k.bit(i) {
            result = p256_jac_add_affine_mont(&result, &q_mont);
        }
    }
    p256_jac_to_affine_mont(&result)
}

fn p256_double(pt: &P256Point) -> P256Point {
    let p = p256_p();
    let three = BigUInt::from_be_bytes(&[3]);
    let two = BigUInt::from_be_bytes(&[2]);
    match pt {
        P256Point::Identity => P256Point::Identity,
        P256Point::Affine { x, y } => {
            if y.is_zero() {
                return P256Point::Identity;
            }

            let x2 = mod_mul(x, x, &p);
            let three_x2 = mod_mul(&three, &x2, &p);
            let three_x2_plus_a = mod_sub(&three_x2, &three, &p);
            let two_y = mod_mul(&two, y, &p);
            let inv = mod_inv_fermat(&two_y, &p);
            let lambda = mod_mul(&three_x2_plus_a, &inv, &p);

            let lambda2 = mod_mul(&lambda, &lambda, &p);
            let two_x = mod_mul(&two, x, &p);
            let x3 = mod_sub(&lambda2, &two_x, &p);

            let x_minus_x3 = mod_sub(x, &x3, &p);
            let lambda_diff = mod_mul(&lambda, &x_minus_x3, &p);
            let y3 = mod_sub(&lambda_diff, y, &p);
            P256Point::Affine { x: x3, y: y3 }
        }
    }
}

fn p256_add(p1: &P256Point, p2: &P256Point) -> P256Point {
    use std::cmp::Ordering;
    let p = p256_p();
    match (p1, p2) {
        (P256Point::Identity, q) | (q, P256Point::Identity) => q.clone(),
        (P256Point::Affine { x: x1, y: y1 }, P256Point::Affine { x: x2, y: y2 }) => {
            if x1.cmp(x2) == Ordering::Equal {

                if y1.cmp(y2) == Ordering::Equal {
                    return p256_double(p1);
                }

                return P256Point::Identity;
            }

            let dy = mod_sub(y2, y1, &p);
            let dx = mod_sub(x2, x1, &p);
            let inv = mod_inv_fermat(&dx, &p);
            let lambda = mod_mul(&dy, &inv, &p);

            let lambda2 = mod_mul(&lambda, &lambda, &p);
            let x3 = mod_sub(&mod_sub(&lambda2, x1, &p), x2, &p);

            let x1_minus_x3 = mod_sub(x1, &x3, &p);
            let lambda_diff = mod_mul(&lambda, &x1_minus_x3, &p);
            let y3 = mod_sub(&lambda_diff, y1, &p);
            P256Point::Affine { x: x3, y: y3 }
        }
    }
}

pub fn p256_scalar_mul(k: &BigUInt, pt: &P256Point) -> P256Point {

    p256_scalar_mul_fp(k, pt)
}

#[allow(dead_code)]
fn p256_scalar_mul_affine(k: &BigUInt, pt: &P256Point) -> P256Point {
    let mut result = P256Point::Identity;
    let mut addend = pt.clone();
    let bits = k.bit_len();
    for i in 0..bits {
        if k.bit(i) {
            result = p256_add(&result, &addend);
        }
        addend = p256_double(&addend);
    }
    result
}

pub fn ecdsa_p256_sha256_sign(
    d_bytes: &[u8],
    message: &[u8],
    nonce_k: &[u8],
) -> Result<Vec<u8>, String> {
    let n = p256_n();
    let d = BigUInt::from_be_bytes(d_bytes);
    let k = BigUInt::from_be_bytes(nonce_k);
    use std::cmp::Ordering;
    if k.is_zero() || k.cmp(&n) != Ordering::Less {
        return Err("ECDSA: nonce_k out of range".into());
    }
    if d.is_zero() || d.cmp(&n) != Ordering::Less {
        return Err("ECDSA: private key out of range".into());
    }
    let e_bytes = digest_sha256(message);
    let e = BigUInt::from_be_bytes(&e_bytes);

    let e_red = e.modulo(&n);
    let g = p256_g();
    let r_pt = p256_scalar_mul(&k, &g);
    let x1 = match &r_pt {
        P256Point::Affine { x, .. } => x.clone(),
        P256Point::Identity => return Err("ECDSA: k*G is identity".into()),
    };
    let r = x1.modulo(&n);
    if r.is_zero() {
        return Err("ECDSA: r=0 — retry with new k".into());
    }
    let k_inv = mod_inv_public_fermat(&k, &n);
    let rd = mod_mul(&r, &d, &n);
    let e_plus_rd = mod_add(&e_red, &rd, &n);
    let s = mod_mul(&k_inv, &e_plus_rd, &n);
    if s.is_zero() {
        return Err("ECDSA: s=0 — retry with new k".into());
    }
    let mut out = Vec::with_capacity(64);
    out.extend_from_slice(&r.to_be_bytes(32));
    out.extend_from_slice(&s.to_be_bytes(32));
    Ok(out)
}

pub fn ecdsa_p256_sha256_sign_deterministic(
    d_bytes: &[u8],
    message: &[u8],
) -> Result<Vec<u8>, String> {
    let e_bytes = digest_sha256(message);
    let nonce = deterministic_ecdsa_nonce(&curve_p256(), d_bytes, &e_bytes)?;
    ecdsa_p256_sha256_sign(d_bytes, message, &nonce)
}

pub fn ecdsa_p256_sha256_verify(
    qx_bytes: &[u8],
    qy_bytes: &[u8],
    message: &[u8],
    signature: &[u8],
) -> Result<(), String> {
    use std::cmp::Ordering;
    if signature.len() != 64 {
        return Err("ECDSA: signature must be 64 bytes".into());
    }
    let n = p256_n();
    let one = BigUInt::one();
    let r = BigUInt::from_be_bytes(&signature[..32]);
    let s = BigUInt::from_be_bytes(&signature[32..]);
    if r.cmp(&one) == Ordering::Less || r.cmp(&n) != Ordering::Less {
        return Err("ECDSA: r out of range".into());
    }
    if s.cmp(&one) == Ordering::Less || s.cmp(&n) != Ordering::Less {
        return Err("ECDSA: s out of range".into());
    }
    let qx = BigUInt::from_be_bytes(qx_bytes);
    let qy = BigUInt::from_be_bytes(qy_bytes);

    let p = p256_p();
    let three = BigUInt::from_be_bytes(&[3]);
    let lhs = mod_mul(&qy, &qy, &p);
    let x3 = mod_mul(&mod_mul(&qx, &qx, &p), &qx, &p);
    let neg3x = mod_mul(&three, &qx, &p);
    let rhs = mod_sub(&mod_add(&x3, &p256_b(), &p), &neg3x, &p);
    if lhs.cmp(&rhs) != Ordering::Equal {
        return Err("ECDSA: public key not on curve".into());
    }
    let q = P256Point::Affine { x: qx, y: qy };
    let e = BigUInt::from_be_bytes(&digest_sha256(message)).modulo(&n);
    let w = mod_inv_fermat(&s, &n);
    let u1 = mod_mul(&e, &w, &n);
    let u2 = mod_mul(&r, &w, &n);
    let p1 = p256_scalar_mul(&u1, &p256_g());
    let p2 = p256_scalar_mul(&u2, &q);
    let r_pt = p256_add(&p1, &p2);
    let x1 = match r_pt {
        P256Point::Affine { x, .. } => x,
        P256Point::Identity => return Err("ECDSA: u1*G + u2*Q is identity".into()),
    };
    if x1.modulo(&n).cmp(&r) == Ordering::Equal {
        Ok(())
    } else {
        Err("ECDSA: signature mismatch".into())
    }
}

#[derive(Clone)]
pub struct Curve {
    pub p: BigUInt,
    pub n: BigUInt,
    pub b: BigUInt,

    pub a: BigUInt,
    pub g: P256Point,
    pub coord_bytes: usize,
}

fn curve_a_minus3(p: &BigUInt) -> BigUInt {
    mod_sub(p, &BigUInt::from_be_bytes(&[3]), p)
}

pub fn curve_p256() -> Curve {
    let p = p256_p();
    Curve {
        a: curve_a_minus3(&p),
        p,
        n: p256_n(),
        b: p256_b(),
        g: p256_g(),
        coord_bytes: 32,
    }
}

pub fn curve_secp256k1() -> Curve {
    let p = BigUInt::from_be_bytes(&hex_to_bytes(
        "fffffffffffffffffffffffffffffffffffffffffffffffffffffffefffffc2f",
    ));
    let n = BigUInt::from_be_bytes(&hex_to_bytes(
        "fffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd0364141",
    ));
    let b = BigUInt::from_be_bytes(&[7]);
    let gx = BigUInt::from_be_bytes(&hex_to_bytes(
        "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798",
    ));
    let gy = BigUInt::from_be_bytes(&hex_to_bytes(
        "483ada7726a3c4655da4fbfc0e1108a8fd17b448a68554199c47d08ffb10d4b8",
    ));
    Curve {
        a: BigUInt::from_be_bytes(&[0]),
        p,
        n,
        b,
        g: P256Point::Affine { x: gx, y: gy },
        coord_bytes: 32,
    }
}

pub fn curve_secp224r1() -> Curve {
    let p = BigUInt::from_be_bytes(&hex_to_bytes(
        "ffffffffffffffffffffffffffffffff000000000000000000000001",
    ));
    let n = BigUInt::from_be_bytes(&hex_to_bytes(
        "ffffffffffffffffffffffffffff16a2e0b8f03e13dd29455c5c2a3d",
    ));
    let b = BigUInt::from_be_bytes(&hex_to_bytes(
        "b4050a850c04b3abf54132565044b0b7d7bfd8ba270b39432355ffb4",
    ));
    let gx = BigUInt::from_be_bytes(&hex_to_bytes(
        "b70e0cbd6bb4bf7f321390b94a03c1d356c21122343280d6115c1d21",
    ));
    let gy = BigUInt::from_be_bytes(&hex_to_bytes(
        "bd376388b5f723fb4c22dfe6cd4375a05a07476444d5819985007e34",
    ));
    Curve {
        a: curve_a_minus3(&p),
        p,
        n,
        b,
        g: P256Point::Affine { x: gx, y: gy },
        coord_bytes: 28,
    }
}

pub fn curve_prime192v1() -> Curve {
    let p = BigUInt::from_be_bytes(&hex_to_bytes(
        "fffffffffffffffffffffffffffffffeffffffffffffffff",
    ));
    let n = BigUInt::from_be_bytes(&hex_to_bytes(
        "ffffffffffffffffffffffff99def836146bc9b1b4d22831",
    ));
    let b = BigUInt::from_be_bytes(&hex_to_bytes(
        "64210519e59c80e70fa7e9ab72243049feb8deecc146b9b1",
    ));
    let gx = BigUInt::from_be_bytes(&hex_to_bytes(
        "188da80eb03090f67cbf20eb43a18800f4ff0afd82ff1012",
    ));
    let gy = BigUInt::from_be_bytes(&hex_to_bytes(
        "07192b95ffc8da78631011ed6b24cdd573f977a11e794811",
    ));
    Curve {
        a: curve_a_minus3(&p),
        p,
        n,
        b,
        g: P256Point::Affine { x: gx, y: gy },
        coord_bytes: 24,
    }
}

pub fn curve_brainpoolp256r1() -> Curve {
    let hx = |s| BigUInt::from_be_bytes(&hex_to_bytes(s));
    Curve {
        p: hx("a9fb57dba1eea9bc3e660a909d838d726e3bf623d52620282013481d1f6e5377"),
        n: hx("a9fb57dba1eea9bc3e660a909d838d718c397aa3b561a6f7901e0e82974856a7"),
        a: hx("7d5a0975fc2c3057eef67530417affe7fb8055c126dc5c6ce94a4b44f330b5d9"),
        b: hx("26dc5c6ce94a4b44f330b5d9bbd77cbf958416295cf7e1ce6bccdc18ff8c07b6"),
        g: P256Point::Affine {
            x: hx("8bd2aeb9cb7e57cb2c4b482ffc81b7afb9de27e1e3bd23c23a4453bd9ace3262"),
            y: hx("547ef835c3dac4fd97f8461a14611dc9c27745132ded8e545c1d54c72f046997"),
        },
        coord_bytes: 32,
    }
}

pub fn curve_brainpoolp384r1() -> Curve {
    let hx = |s| BigUInt::from_be_bytes(&hex_to_bytes(s));
    Curve {
        p: hx("8cb91e82a3386d280f5d6f7e50e641df152f7109ed5456b412b1da197fb71123acd3a729901d1a71874700133107ec53"),
        n: hx("8cb91e82a3386d280f5d6f7e50e641df152f7109ed5456b31f166e6cac0425a7cf3ab6af6b7fc3103b883202e9046565"),
        a: hx("7bc382c63d8c150c3c72080ace05afa0c2bea28e4fb22787139165efba91f90f8aa5814a503ad4eb04a8c7dd22ce2826"),
        b: hx("04a8c7dd22ce28268b39b55416f0447c2fb77de107dcd2a62e880ea53eeb62d57cb4390295dbc9943ab78696fa504c11"),
        g: P256Point::Affine {
            x: hx("1d1c64f068cf45ffa2a63a81b7c13f6b8847a3e77ef14fe3db7fcafe0cbd10e8e826e03436d646aaef87b2e247d4af1e"),
            y: hx("8abe1d7520f9c2a45cb1eb8e95cfd55262b70b29feec5864e19c054ff99129280e4646217791811142820341263c5315"),
        },
        coord_bytes: 48,
    }
}

pub fn curve_brainpoolp512r1() -> Curve {
    let hx = |s| BigUInt::from_be_bytes(&hex_to_bytes(s));
    Curve {
        p: hx("aadd9db8dbe9c48b3fd4e6ae33c9fc07cb308db3b3c9d20ed6639cca703308717d4d9b009bc66842aecda12ae6a380e62881ff2f2d82c68528aa6056583a48f3"),
        n: hx("aadd9db8dbe9c48b3fd4e6ae33c9fc07cb308db3b3c9d20ed6639cca70330870553e5c414ca92619418661197fac10471db1d381085ddaddb58796829ca90069"),
        a: hx("7830a3318b603b89e2327145ac234cc594cbdd8d3df91610a83441caea9863bc2ded5d5aa8253aa10a2ef1c98b9ac8b57f1117a72bf2c7b9e7c1ac4d77fc94ca"),
        b: hx("3df91610a83441caea9863bc2ded5d5aa8253aa10a2ef1c98b9ac8b57f1117a72bf2c7b9e7c1ac4d77fc94cadc083e67984050b75ebae5dd2809bd638016f723"),
        g: P256Point::Affine {
            x: hx("81aee4bdd82ed9645a21322e9c4c6a9385ed9f70b5d916c1b43b62eef4d0098eff3b1f78e2d0d48d50d1687b93b97d5f7c6d5047406a5e688b352209bcb9f822"),
            y: hx("7dde385d566332ecc0eabfa9cf7822fdf209f70024a57b1aa000c55b881f8111b2dcde494a5f485e5bca4bd88a2763aed1ca2b2fa8f0540678cd1e0f3ad80892"),
        },
        coord_bytes: 64,
    }
}

pub fn curve_p384() -> Curve {

    let p = BigUInt::from_be_bytes(&hex_to_bytes(
        "fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffeffffffff0000000000000000ffffffff"));
    let n = BigUInt::from_be_bytes(&hex_to_bytes(
        "ffffffffffffffffffffffffffffffffffffffffffffffffc7634d81f4372ddf581a0db248b0a77aecec196accc52973"));
    let b = BigUInt::from_be_bytes(&hex_to_bytes(
        "b3312fa7e23ee7e4988e056be3f82d19181d9c6efe8141120314088f5013875ac656398d8a2ed19d2a85c8edd3ec2aef"));
    let gx = BigUInt::from_be_bytes(&hex_to_bytes(
        "aa87ca22be8b05378eb1c71ef320ad746e1d3b628ba79b9859f741e082542a385502f25dbf55296c3a545e3872760ab7"));
    let gy = BigUInt::from_be_bytes(&hex_to_bytes(
        "3617de4a96262c6f5d9e98bf9292dc29f8f41dbd289a147ce9da3113b5f0b8c00a60b1ce1d7e819d7a431d7c90ea0e5f"));
    Curve {
        a: curve_a_minus3(&p),
        p,
        n,
        b,
        g: P256Point::Affine { x: gx, y: gy },
        coord_bytes: 48,
    }
}

pub fn curve_p521() -> Curve {

    let p = BigUInt::from_be_bytes(&hex_to_bytes(
        "01ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"));
    let n = BigUInt::from_be_bytes(&hex_to_bytes(
        "01fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffa51868783bf2f966b7fcc0148f709a5d03bb5c9b8899c47aebb6fb71e91386409"));
    let b = BigUInt::from_be_bytes(&hex_to_bytes(
        "0051953eb9618e1c9a1f929a21a0b68540eea2da725b99b315f3b8b489918ef109e156193951ec7e937b1652c0bd3bb1bf073573df883d2c34f1ef451fd46b503f00"));
    let gx = BigUInt::from_be_bytes(&hex_to_bytes(
        "00c6858e06b70404e9cd9e3ecb662395b4429c648139053fb521f828af606b4d3dbaa14b5e77efe75928fe1dc127a2ffa8de3348b3c1856a429bf97e7e31c2e5bd66"));
    let gy = BigUInt::from_be_bytes(&hex_to_bytes(
        "011839296a789a3bc0045c8a5fb42c7d1bd998f54449579b446817afbd17273e662c97ee72995ef42640c550b9013fad0761353c7086a272c24088be94769fd16650"));
    Curve {
        a: curve_a_minus3(&p),
        p,
        n,
        b,
        g: P256Point::Affine { x: gx, y: gy },
        coord_bytes: 66,
    }
}

fn hex_to_bytes(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

fn ec_double(c: &Curve, pt: &P256Point) -> P256Point {
    let p = &c.p;
    let three = BigUInt::from_be_bytes(&[3]);
    let two = BigUInt::from_be_bytes(&[2]);
    match pt {
        P256Point::Identity => P256Point::Identity,
        P256Point::Affine { x, y } => {
            if y.is_zero() {
                return P256Point::Identity;
            }
            let x2 = mod_mul(x, x, p);
            let three_x2 = mod_mul(&three, &x2, p);

            let three_x2_plus_a = mod_add(&three_x2, &c.a, p);
            let two_y = mod_mul(&two, y, p);
            let inv = mod_inv_fermat(&two_y, p);
            let lambda = mod_mul(&three_x2_plus_a, &inv, p);
            let lambda2 = mod_mul(&lambda, &lambda, p);
            let two_x = mod_mul(&two, x, p);
            let x3 = mod_sub(&lambda2, &two_x, p);
            let x_minus_x3 = mod_sub(x, &x3, p);
            let lambda_diff = mod_mul(&lambda, &x_minus_x3, p);
            let y3 = mod_sub(&lambda_diff, y, p);
            P256Point::Affine { x: x3, y: y3 }
        }
    }
}

fn ec_add(c: &Curve, p1: &P256Point, p2: &P256Point) -> P256Point {
    use std::cmp::Ordering;
    let p = &c.p;
    match (p1, p2) {
        (P256Point::Identity, q) | (q, P256Point::Identity) => q.clone(),
        (P256Point::Affine { x: x1, y: y1 }, P256Point::Affine { x: x2, y: y2 }) => {
            if x1.cmp(x2) == Ordering::Equal {
                if y1.cmp(y2) == Ordering::Equal {
                    return ec_double(c, p1);
                }
                return P256Point::Identity;
            }
            let dy = mod_sub(y2, y1, p);
            let dx = mod_sub(x2, x1, p);
            let inv = mod_inv_fermat(&dx, p);
            let lambda = mod_mul(&dy, &inv, p);
            let lambda2 = mod_mul(&lambda, &lambda, p);
            let x3 = mod_sub(&mod_sub(&lambda2, x1, p), x2, p);
            let x1_minus_x3 = mod_sub(x1, &x3, p);
            let lambda_diff = mod_mul(&lambda, &x1_minus_x3, p);
            let y3 = mod_sub(&lambda_diff, y1, p);
            P256Point::Affine { x: x3, y: y3 }
        }
    }
}

#[derive(Clone)]
struct JacPoint {
    x: BigUInt,
    y: BigUInt,
    z: BigUInt,
}

impl JacPoint {
    fn identity() -> Self {
        JacPoint {
            x: BigUInt::one(),
            y: BigUInt::one(),
            z: BigUInt::from_be_bytes(&[]),
        }
    }
    fn is_identity(&self) -> bool {
        self.z.is_zero()
    }
    fn from_affine(pt: &P256Point) -> Self {
        match pt {
            P256Point::Identity => Self::identity(),
            P256Point::Affine { x, y } => JacPoint {
                x: x.clone(),
                y: y.clone(),
                z: BigUInt::one(),
            },
        }
    }
}

fn jac_double(c: &Curve, j: &JacPoint) -> JacPoint {
    let p = &c.p;
    if j.is_identity() {
        return j.clone();
    }
    if j.y.is_zero() {
        return JacPoint::identity();
    }
    let three = BigUInt::from_be_bytes(&[3]);
    let four = BigUInt::from_be_bytes(&[4]);
    let eight = BigUInt::from_be_bytes(&[8]);
    let delta = mod_mul(&j.z, &j.z, p);
    let gamma = mod_mul(&j.y, &j.y, p);
    let beta = mod_mul(&j.x, &gamma, p);
    let x_minus_d = mod_sub(&j.x, &delta, p);
    let x_plus_d = mod_add(&j.x, &delta, p);
    let alpha = mod_mul(&three, &mod_mul(&x_minus_d, &x_plus_d, p), p);
    let alpha2 = mod_mul(&alpha, &alpha, p);
    let x3 = mod_sub(&alpha2, &mod_mul(&eight, &beta, p), p);
    let y_plus_z = mod_add(&j.y, &j.z, p);
    let z3 = mod_sub(
        &mod_sub(&mod_mul(&y_plus_z, &y_plus_z, p), &gamma, p),
        &delta,
        p,
    );
    let four_beta_minus_x3 = mod_sub(&mod_mul(&four, &beta, p), &x3, p);
    let gamma2 = mod_mul(&gamma, &gamma, p);
    let y3 = mod_sub(
        &mod_mul(&alpha, &four_beta_minus_x3, p),
        &mod_mul(&eight, &gamma2, p),
        p,
    );
    JacPoint {
        x: x3,
        y: y3,
        z: z3,
    }
}

fn jac_add_affine(c: &Curve, j: &JacPoint, a: &P256Point) -> JacPoint {
    use std::cmp::Ordering;
    let p = &c.p;
    let (ax, ay) = match a {
        P256Point::Identity => return j.clone(),
        P256Point::Affine { x, y } => (x, y),
    };
    if j.is_identity() {
        return JacPoint::from_affine(a);
    }
    let z1z1 = mod_mul(&j.z, &j.z, p);
    let u2 = mod_mul(ax, &z1z1, p);
    let z1_cubed = mod_mul(&j.z, &z1z1, p);
    let s2 = mod_mul(ay, &z1_cubed, p);
    if u2.cmp(&j.x) == Ordering::Equal {
        if s2.cmp(&j.y) == Ordering::Equal {
            return jac_double(c, j);
        }
        return JacPoint::identity();
    }
    let h = mod_sub(&u2, &j.x, p);
    let r = mod_sub(&s2, &j.y, p);
    let h2 = mod_mul(&h, &h, p);
    let h3 = mod_mul(&h2, &h, p);
    let x1_h2 = mod_mul(&j.x, &h2, p);
    let two = BigUInt::from_be_bytes(&[2]);
    let r2 = mod_mul(&r, &r, p);
    let x3 = mod_sub(&mod_sub(&r2, &h3, p), &mod_mul(&two, &x1_h2, p), p);
    let y3 = mod_sub(
        &mod_mul(&r, &mod_sub(&x1_h2, &x3, p), p),
        &mod_mul(&j.y, &h3, p),
        p,
    );
    let z3 = mod_mul(&j.z, &h, p);
    JacPoint {
        x: x3,
        y: y3,
        z: z3,
    }
}

fn jac_to_affine(c: &Curve, j: &JacPoint) -> P256Point {
    let p = &c.p;
    if j.is_identity() {
        return P256Point::Identity;
    }
    let z_inv = mod_inv_fermat(&j.z, p);
    let z_inv2 = mod_mul(&z_inv, &z_inv, p);
    let z_inv3 = mod_mul(&z_inv2, &z_inv, p);
    P256Point::Affine {
        x: mod_mul(&j.x, &z_inv2, p),
        y: mod_mul(&j.y, &z_inv3, p),
    }
}

fn jac_negate(c: &Curve, j: &JacPoint) -> JacPoint {
    if j.is_identity() {
        return j.clone();
    }
    JacPoint {
        x: j.x.clone(),
        y: mod_sub(&c.p, &j.y, &c.p),
        z: j.z.clone(),
    }
}

fn affine_negate(c: &Curve, p: &P256Point) -> P256Point {
    match p {
        P256Point::Identity => P256Point::Identity,
        P256Point::Affine { x, y } => P256Point::Affine {
            x: x.clone(),
            y: mod_sub(&c.p, y, &c.p),
        },
    }
}

fn wnaf(k: &BigUInt, w: u32) -> Vec<i32> {
    assert!(w >= 2 && w <= 8);
    let pow_w = 1i32 << w;
    let mask = (pow_w - 1) as u32;
    let half = pow_w >> 1;

    let mut limbs: Vec<u32> = k.limbs().to_vec();
    let mut digits = Vec::new();
    loop {

        if limbs.iter().all(|&l| l == 0) {
            break;
        }
        let lsb = limbs[0] & 1;
        if lsb == 1 {

            let low_w = (limbs[0] & mask) as i32;
            let d = if low_w >= half { low_w - pow_w } else { low_w };
            digits.push(d);

            if d > 0 {
                sub_u32_inplace(&mut limbs, d as u32);
            } else {
                add_u32_inplace(&mut limbs, (-d) as u32);
            }
        } else {
            digits.push(0);
        }

        shr1_inplace(&mut limbs);
    }
    digits
}

fn add_u32_inplace(limbs: &mut Vec<u32>, x: u32) {
    let mut carry = x as u64;
    let mut i = 0;
    while carry != 0 {
        if i >= limbs.len() {
            limbs.push(0);
        }
        let s = limbs[i] as u64 + carry;
        limbs[i] = (s & 0xFFFF_FFFF) as u32;
        carry = s >> 32;
        i += 1;
    }
}

fn sub_u32_inplace(limbs: &mut Vec<u32>, x: u32) {

    let mut borrow = x as i64;
    let mut i = 0;
    while borrow != 0 {
        let s = limbs[i] as i64 - borrow;
        if s < 0 {
            limbs[i] = (s + (1i64 << 32)) as u32;
            borrow = 1;
        } else {
            limbs[i] = s as u32;
            borrow = 0;
        }
        i += 1;
        if i >= limbs.len() {
            break;
        }
    }
    while limbs.len() > 1 && *limbs.last().unwrap() == 0 {
        limbs.pop();
    }
}

fn shr1_inplace(limbs: &mut Vec<u32>) {
    let mut carry = 0u32;
    for i in (0..limbs.len()).rev() {
        let next_carry = limbs[i] & 1;
        limbs[i] = (limbs[i] >> 1) | (carry << 31);
        carry = next_carry;
    }
    while limbs.len() > 1 && *limbs.last().unwrap() == 0 {
        limbs.pop();
    }
}

fn batch_mod_inv(values: &[BigUInt], p: &BigUInt) -> Vec<BigUInt> {
    let n = values.len();
    if n == 0 {
        return Vec::new();
    }

    let mut prefix: Vec<BigUInt> = Vec::with_capacity(n);
    prefix.push(values[0].clone());
    for i in 1..n {
        prefix.push(mod_mul(&prefix[i - 1], &values[i], p));
    }

    let mut inv_acc = mod_inv_fermat(&prefix[n - 1], p);

    let mut inverses: Vec<BigUInt> = vec![BigUInt::zero(); n];
    for i in (1..n).rev() {

        inverses[i] = mod_mul(&inv_acc, &prefix[i - 1], p);

        inv_acc = mod_mul(&inv_acc, &values[i], p);
    }
    inverses[0] = inv_acc;
    inverses
}

fn jac_to_affine_batch(c: &Curve, jacs: &[JacPoint]) -> Vec<P256Point> {

    let zs: Vec<BigUInt> = jacs
        .iter()
        .filter(|j| !j.is_identity())
        .map(|j| j.z.clone())
        .collect();
    let z_invs = batch_mod_inv(&zs, &c.p);
    let p = &c.p;
    let mut out: Vec<P256Point> = Vec::with_capacity(jacs.len());
    let mut zi = 0;
    for j in jacs {
        if j.is_identity() {
            out.push(P256Point::Identity);
        } else {
            let z_inv = &z_invs[zi];
            zi += 1;
            let z_inv2 = mod_mul(z_inv, z_inv, p);
            let z_inv3 = mod_mul(&z_inv2, z_inv, p);
            out.push(P256Point::Affine {
                x: mod_mul(&j.x, &z_inv2, p),
                y: mod_mul(&j.y, &z_inv3, p),
            });
        }
    }
    out
}

#[allow(dead_code)]
pub fn ec_scalar_mul_affine_generic(c: &Curve, k: &BigUInt, pt: &P256Point) -> P256Point {
    let mut result = P256Point::Identity;
    let mut addend = pt.clone();
    let bits = k.bit_len();
    for i in 0..bits {
        if k.bit(i) {
            result = ec_add(c, &result, &addend);
        }
        addend = ec_double(c, &addend);
    }
    result
}

fn jac_double_generic(c: &Curve, j: &JacPoint) -> JacPoint {
    let p = &c.p;
    if j.is_identity() || j.y.is_zero() {
        return JacPoint::identity();
    }
    let three = BigUInt::from_be_bytes(&[3]);
    let four = BigUInt::from_be_bytes(&[4]);
    let eight = BigUInt::from_be_bytes(&[8]);
    let delta = mod_mul(&j.z, &j.z, p);
    let gamma = mod_mul(&j.y, &j.y, p);
    let beta = mod_mul(&j.x, &gamma, p);
    let xx = mod_mul(&j.x, &j.x, p);
    let delta2 = mod_mul(&delta, &delta, p);
    let alpha = mod_add(&mod_mul(&three, &xx, p), &mod_mul(&c.a, &delta2, p), p);
    let alpha2 = mod_mul(&alpha, &alpha, p);
    let x3 = mod_sub(&alpha2, &mod_mul(&eight, &beta, p), p);
    let y_plus_z = mod_add(&j.y, &j.z, p);
    let z3 = mod_sub(
        &mod_sub(&mod_mul(&y_plus_z, &y_plus_z, p), &gamma, p),
        &delta,
        p,
    );
    let four_beta_minus_x3 = mod_sub(&mod_mul(&four, &beta, p), &x3, p);
    let gamma2 = mod_mul(&gamma, &gamma, p);
    let y3 = mod_sub(
        &mod_mul(&alpha, &four_beta_minus_x3, p),
        &mod_mul(&eight, &gamma2, p),
        p,
    );
    JacPoint {
        x: x3,
        y: y3,
        z: z3,
    }
}

fn jac_add_affine_generic(c: &Curve, j: &JacPoint, a: &P256Point) -> JacPoint {
    use std::cmp::Ordering;
    let p = &c.p;
    let (ax, ay) = match a {
        P256Point::Identity => return j.clone(),
        P256Point::Affine { x, y } => (x, y),
    };
    if j.is_identity() {
        return JacPoint::from_affine(a);
    }
    let z1z1 = mod_mul(&j.z, &j.z, p);
    let u2 = mod_mul(ax, &z1z1, p);
    let z1_cubed = mod_mul(&j.z, &z1z1, p);
    let s2 = mod_mul(ay, &z1_cubed, p);
    if u2.cmp(&j.x) == Ordering::Equal {
        if s2.cmp(&j.y) == Ordering::Equal {
            return jac_double_generic(c, j);
        }
        return JacPoint::identity();
    }
    let h = mod_sub(&u2, &j.x, p);
    let r = mod_sub(&s2, &j.y, p);
    let h2 = mod_mul(&h, &h, p);
    let h3 = mod_mul(&h2, &h, p);
    let x1_h2 = mod_mul(&j.x, &h2, p);
    let two = BigUInt::from_be_bytes(&[2]);
    let r2 = mod_mul(&r, &r, p);
    let x3 = mod_sub(&mod_sub(&r2, &h3, p), &mod_mul(&two, &x1_h2, p), p);
    let y3 = mod_sub(
        &mod_mul(&r, &mod_sub(&x1_h2, &x3, p), p),
        &mod_mul(&j.y, &h3, p),
        p,
    );
    let z3 = mod_mul(&j.z, &h, p);
    JacPoint {
        x: x3,
        y: y3,
        z: z3,
    }
}

pub fn ec_scalar_mul_jac_generic(c: &Curve, k: &BigUInt, pt: &P256Point) -> P256Point {
    let bits = k.bit_len();
    if bits == 0 || matches!(pt, P256Point::Identity) {
        return P256Point::Identity;
    }
    let mut r = JacPoint::identity();
    for i in (0..bits).rev() {
        r = jac_double_generic(c, &r);
        if k.bit(i) {
            r = jac_add_affine_generic(c, &r, pt);
        }
    }
    jac_to_affine(c, &r)
}

pub fn ec_scalar_mul(c: &Curve, k: &BigUInt, pt: &P256Point) -> P256Point {

    if c.a.cmp(&curve_a_minus3(&c.p)) != std::cmp::Ordering::Equal
        || !matches!(c.coord_bytes, 32 | 48 | 66)
    {
        return ec_scalar_mul_jac_generic(c, k, pt);
    }

    if c.coord_bytes == 48 && c.b.to_be_bytes(48) == curve_p384().b.to_be_bytes(48) {
        return p384_scalar_mul_solinas(k, pt);
    }

    return ec_scalar_mul_mont_g(c, k, pt);
    #[allow(unreachable_code, dead_code, unused_variables)]

    let bits = k.bit_len();
    if bits == 0 {
        return P256Point::Identity;
    }
    if matches!(pt, P256Point::Identity) {
        return P256Point::Identity;
    }

    const W: u32 = 4;
    let n_entries = 1usize << (W - 1);

    let mut odd_jac: Vec<JacPoint> = Vec::with_capacity(n_entries);
    odd_jac.push(JacPoint::from_affine(pt));

    let two_p_j = jac_double(c, &odd_jac[0]);

    let two_p_aff = jac_to_affine(c, &two_p_j);
    let mut prev = odd_jac[0].clone();
    for _ in 1..n_entries {
        prev = jac_add_affine(c, &prev, &two_p_aff);
        odd_jac.push(prev.clone());
    }

    let odd_aff = jac_to_affine_batch(c, &odd_jac);

    let digits = wnaf(k, W);

    let mut result = JacPoint::identity();
    for &d in digits.iter().rev() {

        result = jac_double(c, &result);
        if d != 0 {
            let idx = (d.abs() as usize - 1) / 2;
            let entry = if d > 0 {
                odd_aff[idx].clone()
            } else {
                affine_negate(c, &odd_aff[idx])
            };
            result = jac_add_affine(c, &result, &entry);
        }
    }
    jac_to_affine(c, &result)
}

use std::sync::OnceLock;

mod p256_base_table;

static P256_BASE_TABLE: OnceLock<Vec<P256Point>> = OnceLock::new();

fn p256_base_table() -> &'static [P256Point] {
    P256_BASE_TABLE.get_or_init(|| p256_base_table::p256_base_table_baked())
}

pub fn p256_scalar_mul_base(k: &BigUInt) -> P256Point {
    let c = curve_p256();
    let bits = k.bit_len();
    if bits == 0 {
        return P256Point::Identity;
    }
    let table = p256_base_table();
    let mut result = JacPoint::identity();
    for i in 0..bits {
        if k.bit(i) {
            result = jac_add_affine(&c, &result, &table[i]);
        }
    }
    jac_to_affine(&c, &result)
}

pub fn ec_generate_keypair(c: &Curve) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    let mut d_bytes = vec![0u8; c.coord_bytes];
    loop {
        get_random_values(&mut d_bytes).expect("platform CSPRNG");
        let d = BigUInt::from_be_bytes(&d_bytes);
        use std::cmp::Ordering;
        if d.cmp(&BigUInt::from_be_bytes(&[0])) == Ordering::Greater
            && d.cmp(&c.n) == Ordering::Less
        {
            let q = ec_scalar_mul(c, &d, &c.g);
            if let P256Point::Affine { x, y } = q {
                let x_bytes = x.to_be_bytes(c.coord_bytes);
                let y_bytes = y.to_be_bytes(c.coord_bytes);
                return (d_bytes, x_bytes, y_bytes);
            }

        }
    }
}

fn on_curve(c: &Curve, x: &BigUInt, y: &BigUInt) -> bool {
    use std::cmp::Ordering;
    let p = &c.p;
    let lhs = mod_mul(y, y, p);
    let x3 = mod_mul(&mod_mul(x, x, p), x, p);

    let ax = mod_mul(&c.a, x, p);
    let rhs = mod_add(&mod_add(&x3, &ax, p), &c.b, p);
    lhs.cmp(&rhs) == Ordering::Equal
}

pub fn ecdsa_sign(
    c: &Curve,
    d_bytes: &[u8],
    hash: &[u8],
    nonce_k: &[u8],
) -> Result<Vec<u8>, String> {
    use std::cmp::Ordering;
    let d = BigUInt::from_be_bytes(d_bytes);
    let k = BigUInt::from_be_bytes(nonce_k);
    if k.is_zero() || k.cmp(&c.n) != Ordering::Less {
        return Err("ECDSA: nonce k out of range".into());
    }
    if d.is_zero() || d.cmp(&c.n) != Ordering::Less {
        return Err("ECDSA: private key out of range".into());
    }

    let e = ecdsa_hash_scalar(hash, &c.n);
    let r_pt = ec_scalar_mul(c, &k, &c.g);
    let x1 = match &r_pt {
        P256Point::Affine { x, .. } => x.clone(),
        P256Point::Identity => return Err("ECDSA: k*G is identity".into()),
    };
    let r = x1.modulo(&c.n);
    if r.is_zero() {
        return Err("ECDSA: r=0".into());
    }
    let k_inv = mod_inv_public_fermat(&k, &c.n);
    let rd = mod_mul(&r, &d, &c.n);
    let e_plus_rd = mod_add(&e, &rd, &c.n);
    let s = mod_mul(&k_inv, &e_plus_rd, &c.n);
    if s.is_zero() {
        return Err("ECDSA: s=0".into());
    }
    let mut out = Vec::with_capacity(2 * c.coord_bytes);
    out.extend_from_slice(&r.to_be_bytes(c.coord_bytes));
    out.extend_from_slice(&s.to_be_bytes(c.coord_bytes));
    Ok(out)
}

pub fn ecdsa_sign_deterministic(c: &Curve, d_bytes: &[u8], hash: &[u8]) -> Result<Vec<u8>, String> {
    let nonce = deterministic_ecdsa_nonce(c, d_bytes, hash)?;
    ecdsa_sign(c, d_bytes, hash, &nonce)
}

pub fn ecdsa_verify(
    c: &Curve,
    qx_bytes: &[u8],
    qy_bytes: &[u8],
    hash: &[u8],
    signature: &[u8],
) -> Result<(), String> {
    use std::cmp::Ordering;
    if signature.len() != 2 * c.coord_bytes {
        return Err("ECDSA: signature length mismatch".into());
    }
    let one = BigUInt::one();
    let r = BigUInt::from_be_bytes(&signature[..c.coord_bytes]);
    let s = BigUInt::from_be_bytes(&signature[c.coord_bytes..]);
    if r.cmp(&one) == Ordering::Less || r.cmp(&c.n) != Ordering::Less {
        return Err("ECDSA: r out of range".into());
    }
    if s.cmp(&one) == Ordering::Less || s.cmp(&c.n) != Ordering::Less {
        return Err("ECDSA: s out of range".into());
    }
    let qx = BigUInt::from_be_bytes(qx_bytes);
    let qy = BigUInt::from_be_bytes(qy_bytes);
    if !on_curve(c, &qx, &qy) {
        return Err("ECDSA: public key not on curve".into());
    }
    let q = P256Point::Affine { x: qx, y: qy };
    let dbg_ec = std::env::var("CRUFT_WC_DEBUG").is_ok();
    if dbg_ec {
        eprintln!("[wc-ec] e = hash mod n");
    }
    let e = ecdsa_hash_scalar(hash, &c.n);
    if dbg_ec {
        eprintln!("[wc-ec] → mod_inv_fermat(s, n)");
    }
    let w = mod_inv_fermat(&s, &c.n);
    if dbg_ec {
        eprintln!("[wc-ec]   mod_inv_fermat OK");
    }
    if dbg_ec {
        eprintln!("[wc-ec] → mod_mul(e, w, n) = u1");
    }
    let u1 = mod_mul(&e, &w, &c.n);
    if dbg_ec {
        eprintln!("[wc-ec] → mod_mul(r, w, n) = u2");
    }
    let u2 = mod_mul(&r, &w, &c.n);
    if dbg_ec {
        eprintln!("[wc-ec] → scalar_mul(u1, G) = p1 (Solinas base-table fast path if P-256, else generic)");
    }
    let p1 = if c.coord_bytes == 32 && c.b.cmp(&p256_b()) == std::cmp::Ordering::Equal {

        p256_scalar_mul_base_solinas(&u1)
    } else {
        ec_scalar_mul(c, &u1, &c.g)
    };
    if dbg_ec {
        eprintln!("[wc-ec]   p1 OK");
    }
    if dbg_ec {
        eprintln!("[wc-ec] → scalar_mul(u2, Q) = p2 (Solinas fast path if P-256)");
    }
    let p2 = if c.coord_bytes == 32 && c.b.cmp(&p256_b()) == std::cmp::Ordering::Equal {

        p256_scalar_mul_solinas(&u2, &q)
    } else {
        ec_scalar_mul(c, &u2, &q)
    };
    if dbg_ec {
        eprintln!("[wc-ec]   p2 OK");
    }
    if dbg_ec {
        eprintln!("[wc-ec] → ec_add(p1, p2)");
    }
    let r_pt = ec_add(c, &p1, &p2);
    if dbg_ec {
        eprintln!("[wc-ec]   ec_add OK");
    }
    let x1 = match r_pt {
        P256Point::Affine { x, .. } => x,
        P256Point::Identity => return Err("ECDSA: u1·G + u2·Q is identity".into()),
    };
    if x1.modulo(&c.n).cmp(&r) == Ordering::Equal {
        Ok(())
    } else {
        Err("ECDSA: signature mismatch".into())
    }
}

pub fn ecdh(
    c: &Curve,
    d_bytes: &[u8],
    qx_bytes: &[u8],
    qy_bytes: &[u8],
) -> Result<Vec<u8>, String> {
    use std::cmp::Ordering;
    let d = BigUInt::from_be_bytes(d_bytes);
    if d.is_zero() || d.cmp(&c.n) != Ordering::Less {
        return Err("ECDH: private scalar out of range".into());
    }
    let qx = BigUInt::from_be_bytes(qx_bytes);
    let qy = BigUInt::from_be_bytes(qy_bytes);
    if !on_curve(c, &qx, &qy) {
        return Err("ECDH: peer public key not on curve".into());
    }
    let q = P256Point::Affine { x: qx, y: qy };
    let shared = ec_scalar_mul(c, &d, &q);
    match shared {
        P256Point::Identity => Err("ECDH: derived point is identity".into()),
        P256Point::Affine { x, .. } => Ok(x.to_be_bytes(c.coord_bytes)),
    }
}

pub fn ecdh_p256(d_bytes: &[u8], qx_bytes: &[u8], qy_bytes: &[u8]) -> Result<Vec<u8>, String> {
    use std::cmp::Ordering;
    let n = p256_n();
    let d = BigUInt::from_be_bytes(d_bytes);
    if d.is_zero() || d.cmp(&n) != Ordering::Less {
        return Err("ECDH: private scalar out of range".into());
    }
    let p = p256_p();
    let three = BigUInt::from_be_bytes(&[3]);
    let qx = BigUInt::from_be_bytes(qx_bytes);
    let qy = BigUInt::from_be_bytes(qy_bytes);

    let lhs = mod_mul(&qy, &qy, &p);
    let x3 = mod_mul(&mod_mul(&qx, &qx, &p), &qx, &p);
    let neg3x = mod_mul(&three, &qx, &p);
    let rhs = mod_sub(&mod_add(&x3, &p256_b(), &p), &neg3x, &p);
    if lhs.cmp(&rhs) != Ordering::Equal {
        return Err("ECDH: peer public key not on curve".into());
    }
    let q = P256Point::Affine { x: qx, y: qy };
    let shared = p256_scalar_mul(&d, &q);
    match shared {
        P256Point::Identity => Err("ECDH: derived point is identity (peer key invalid)".into()),
        P256Point::Affine { x, .. } => Ok(x.to_be_bytes(32)),
    }
}

pub fn mgf1<F>(mgf_seed: &[u8], mask_len: usize, hash_fn: F, hlen: usize) -> Vec<u8>
where
    F: Fn(&[u8]) -> Vec<u8>,
{
    let mut t = Vec::with_capacity(mask_len + hlen);
    let n_iters = (mask_len + hlen - 1) / hlen;
    for counter in 0..n_iters {
        let mut input = Vec::with_capacity(mgf_seed.len() + 4);
        input.extend_from_slice(mgf_seed);
        input.extend_from_slice(&(counter as u32).to_be_bytes());
        let h = hash_fn(&input);
        t.extend_from_slice(&h);
    }
    t.truncate(mask_len);
    t
}

pub fn rsa_oaep_encrypt<F: Fn(&[u8]) -> Vec<u8> + Copy>(
    n_bytes: &[u8],
    e_bytes: &[u8],
    message: &[u8],
    label: &[u8],
    seed: &[u8],
    hash_fn: F,
    hlen: usize,
) -> Result<Vec<u8>, String> {
    let n = BigUInt::from_be_bytes(n_bytes);
    let e = BigUInt::from_be_bytes(e_bytes);
    let k = n_bytes.len();
    let k = if k == 0 {
        return Err("RSA-OAEP: empty modulus".into());
    } else {
        k
    };

    if message.len() > k.saturating_sub(2 * hlen + 2) {
        return Err("RSA-OAEP: message too long".into());
    }
    if seed.len() != hlen {
        return Err(format!("RSA-OAEP: seed length must be {}", hlen));
    }

    let l_hash = hash_fn(label);

    let ps_len = k - message.len() - 2 * hlen - 2;
    let mut db = Vec::with_capacity(k - hlen - 1);
    db.extend_from_slice(&l_hash);
    db.extend(std::iter::repeat(0u8).take(ps_len));
    db.push(0x01);
    db.extend_from_slice(message);
    debug_assert_eq!(db.len(), k - hlen - 1);

    let db_mask = mgf1(seed, k - hlen - 1, hash_fn, hlen);

    let masked_db: Vec<u8> = db.iter().zip(db_mask.iter()).map(|(a, b)| a ^ b).collect();

    let seed_mask = mgf1(&masked_db, hlen, hash_fn, hlen);

    let masked_seed: Vec<u8> = seed
        .iter()
        .zip(seed_mask.iter())
        .map(|(a, b)| a ^ b)
        .collect();

    let mut em = Vec::with_capacity(k);
    em.push(0x00);
    em.extend_from_slice(&masked_seed);
    em.extend_from_slice(&masked_db);
    debug_assert_eq!(em.len(), k);

    let m_int = BigUInt::from_be_bytes(&em);
    let c_int = rsaep(&n, &e, &m_int)?;
    Ok(c_int.to_be_bytes(k))
}

pub fn rsa_oaep_decrypt<F: Fn(&[u8]) -> Vec<u8> + Copy>(
    n_bytes: &[u8],
    d_bytes: &[u8],
    ciphertext: &[u8],
    label: &[u8],
    hash_fn: F,
    hlen: usize,
) -> Result<Vec<u8>, String> {
    let n = BigUInt::from_be_bytes(n_bytes);
    let d = BigUInt::from_be_bytes(d_bytes);
    let k = n_bytes.len();
    if ciphertext.len() != k {
        return Err("RSA-OAEP: ciphertext length mismatch".into());
    }
    if k < 2 * hlen + 2 {
        return Err("RSA-OAEP: modulus too small for hash".into());
    }

    let c_int = BigUInt::from_be_bytes(ciphertext);
    let m_int = rsadp(&n, &d, &c_int)?;
    let em = m_int.to_be_bytes(k);

    let l_hash = hash_fn(label);

    let y = em[0];
    let masked_seed = &em[1..1 + hlen];
    let masked_db = &em[1 + hlen..];

    let seed_mask = mgf1(masked_db, hlen, hash_fn, hlen);
    let seed: Vec<u8> = masked_seed
        .iter()
        .zip(seed_mask.iter())
        .map(|(a, b)| a ^ b)
        .collect();

    let db_mask = mgf1(&seed, k - hlen - 1, hash_fn, hlen);
    let db: Vec<u8> = masked_db
        .iter()
        .zip(db_mask.iter())
        .map(|(a, b)| a ^ b)
        .collect();

    let l_hash_prime = &db[..hlen];
    let rest = &db[hlen..];
    let mut seen_sep = 0u8;
    let mut bad_ps = 0u8;
    let mut sep = 0usize;
    for (i, &b) in rest.iter().enumerate() {
        let is_zero = (b == 0x00) as u8;
        let is_one = (b == 0x01) as u8;
        let before_sep = 1u8 ^ seen_sep;
        let first_sep = before_sep & is_one;
        bad_ps |= before_sep & (1u8 ^ is_zero) & (1u8 ^ is_one);
        if first_sep == 1 {
            sep = i;
        }
        seen_sep |= is_one;
    }
    let ok = y == 0x00 && timing_safe_equal(l_hash_prime, &l_hash) && bad_ps == 0 && seen_sep == 1;
    if !ok {
        return Err("RSA-OAEP: decryption error".into());
    }
    Ok(rest[sep + 1..].to_vec())
}

fn emsa_pss_encode<F: Fn(&[u8]) -> Vec<u8> + Copy>(
    message: &[u8],
    em_bits: usize,
    salt: &[u8],
    hash_fn: F,
    hlen: usize,
) -> Result<Vec<u8>, String> {
    let em_len = (em_bits + 7) / 8;
    let s_len = salt.len();
    if em_len < hlen + s_len + 2 {
        return Err("EMSA-PSS-ENCODE: encoding length too short".into());
    }
    let m_hash = hash_fn(message);

    let mut m_prime = Vec::with_capacity(8 + hlen + s_len);
    m_prime.extend_from_slice(&[0u8; 8]);
    m_prime.extend_from_slice(&m_hash);
    m_prime.extend_from_slice(salt);
    let h = hash_fn(&m_prime);

    let mut db = Vec::with_capacity(em_len - hlen - 1);
    db.extend(std::iter::repeat(0u8).take(em_len - s_len - hlen - 2));
    db.push(0x01);
    db.extend_from_slice(salt);
    let db_mask = mgf1(&h, em_len - hlen - 1, hash_fn, hlen);
    let mut masked_db: Vec<u8> = db.iter().zip(db_mask.iter()).map(|(a, b)| a ^ b).collect();

    let unused_bits = 8 * em_len - em_bits;
    if unused_bits > 0 {
        masked_db[0] &= 0xff >> unused_bits;
    }

    let mut em = Vec::with_capacity(em_len);
    em.extend_from_slice(&masked_db);
    em.extend_from_slice(&h);
    em.push(0xbc);
    Ok(em)
}

fn emsa_pss_verify<F: Fn(&[u8]) -> Vec<u8> + Copy>(
    message: &[u8],
    em: &[u8],
    em_bits: usize,
    s_len: usize,
    hash_fn: F,
    hlen: usize,
) -> Result<(), String> {
    let em_len = (em_bits + 7) / 8;
    if em.len() != em_len {
        return Err("EMSA-PSS-VERIFY: EM length mismatch".into());
    }
    if em_len < hlen + s_len + 2 {
        return Err("EMSA-PSS-VERIFY: inconsistent".into());
    }
    if *em.last().unwrap() != 0xbc {
        return Err("EMSA-PSS-VERIFY: missing 0xbc trailer".into());
    }
    let masked_db = &em[..em_len - hlen - 1];
    let h = &em[em_len - hlen - 1..em_len - 1];
    let unused_bits = 8 * em_len - em_bits;
    if unused_bits > 0 {
        let mask: u8 = (0xff_u16 << (8 - unused_bits)) as u8;
        if masked_db[0] & mask != 0 {
            return Err("EMSA-PSS-VERIFY: non-zero leftmost bits".into());
        }
    }
    let db_mask = mgf1(h, em_len - hlen - 1, hash_fn, hlen);
    let mut db: Vec<u8> = masked_db
        .iter()
        .zip(db_mask.iter())
        .map(|(a, b)| a ^ b)
        .collect();
    if unused_bits > 0 {
        db[0] &= 0xff >> unused_bits;
    }

    let ps_len = em_len - hlen - s_len - 2;
    for &b in &db[..ps_len] {
        if b != 0 {
            return Err("EMSA-PSS-VERIFY: non-zero PS".into());
        }
    }
    if db[ps_len] != 0x01 {
        return Err("EMSA-PSS-VERIFY: missing 0x01 separator".into());
    }
    let salt = &db[ps_len + 1..];
    let m_hash = hash_fn(message);
    let mut m_prime = Vec::with_capacity(8 + hlen + salt.len());
    m_prime.extend_from_slice(&[0u8; 8]);
    m_prime.extend_from_slice(&m_hash);
    m_prime.extend_from_slice(salt);
    let h_prime = hash_fn(&m_prime);
    if !timing_safe_equal(h, &h_prime) {
        return Err("EMSA-PSS-VERIFY: H mismatch".into());
    }
    Ok(())
}

pub fn rsa_pss_sign<F: Fn(&[u8]) -> Vec<u8> + Copy>(
    n_bytes: &[u8],
    d_bytes: &[u8],
    message: &[u8],
    salt: &[u8],
    hash_fn: F,
    hlen: usize,
) -> Result<Vec<u8>, String> {
    let k = n_bytes.len();
    let mod_bits = BigUInt::from_be_bytes(n_bytes).bit_len();
    let em = emsa_pss_encode(message, mod_bits - 1, salt, hash_fn, hlen)?;
    let n = BigUInt::from_be_bytes(n_bytes);
    let d = BigUInt::from_be_bytes(d_bytes);
    let m_int = BigUInt::from_be_bytes(&em);
    let s_int = rsadp(&n, &d, &m_int)?;
    Ok(s_int.to_be_bytes(k))
}

pub fn rsa_pss_verify<F: Fn(&[u8]) -> Vec<u8> + Copy>(
    n_bytes: &[u8],
    e_bytes: &[u8],
    message: &[u8],
    signature: &[u8],
    s_len: usize,
    hash_fn: F,
    hlen: usize,
) -> Result<(), String> {
    let k = n_bytes.len();
    if signature.len() != k {
        return Err("RSA-PSS-VERIFY: signature length mismatch".into());
    }
    let n = BigUInt::from_be_bytes(n_bytes);
    let e = BigUInt::from_be_bytes(e_bytes);
    let mod_bits = n.bit_len();
    let s_int = BigUInt::from_be_bytes(signature);
    let m_int = rsaep(&n, &e, &s_int)?;
    let em_len = (mod_bits - 1 + 7) / 8;
    let em = m_int.to_be_bytes(em_len);
    emsa_pss_verify(message, &em, mod_bits - 1, s_len, hash_fn, hlen)
}

const AES_INV_SBOX: [u8; 256] = [
    0x52, 0x09, 0x6a, 0xd5, 0x30, 0x36, 0xa5, 0x38, 0xbf, 0x40, 0xa3, 0x9e, 0x81, 0xf3, 0xd7, 0xfb,
    0x7c, 0xe3, 0x39, 0x82, 0x9b, 0x2f, 0xff, 0x87, 0x34, 0x8e, 0x43, 0x44, 0xc4, 0xde, 0xe9, 0xcb,
    0x54, 0x7b, 0x94, 0x32, 0xa6, 0xc2, 0x23, 0x3d, 0xee, 0x4c, 0x95, 0x0b, 0x42, 0xfa, 0xc3, 0x4e,
    0x08, 0x2e, 0xa1, 0x66, 0x28, 0xd9, 0x24, 0xb2, 0x76, 0x5b, 0xa2, 0x49, 0x6d, 0x8b, 0xd1, 0x25,
    0x72, 0xf8, 0xf6, 0x64, 0x86, 0x68, 0x98, 0x16, 0xd4, 0xa4, 0x5c, 0xcc, 0x5d, 0x65, 0xb6, 0x92,
    0x6c, 0x70, 0x48, 0x50, 0xfd, 0xed, 0xb9, 0xda, 0x5e, 0x15, 0x46, 0x57, 0xa7, 0x8d, 0x9d, 0x84,
    0x90, 0xd8, 0xab, 0x00, 0x8c, 0xbc, 0xd3, 0x0a, 0xf7, 0xe4, 0x58, 0x05, 0xb8, 0xb3, 0x45, 0x06,
    0xd0, 0x2c, 0x1e, 0x8f, 0xca, 0x3f, 0x0f, 0x02, 0xc1, 0xaf, 0xbd, 0x03, 0x01, 0x13, 0x8a, 0x6b,
    0x3a, 0x91, 0x11, 0x41, 0x4f, 0x67, 0xdc, 0xea, 0x97, 0xf2, 0xcf, 0xce, 0xf0, 0xb4, 0xe6, 0x73,
    0x96, 0xac, 0x74, 0x22, 0xe7, 0xad, 0x35, 0x85, 0xe2, 0xf9, 0x37, 0xe8, 0x1c, 0x75, 0xdf, 0x6e,
    0x47, 0xf1, 0x1a, 0x71, 0x1d, 0x29, 0xc5, 0x89, 0x6f, 0xb7, 0x62, 0x0e, 0xaa, 0x18, 0xbe, 0x1b,
    0xfc, 0x56, 0x3e, 0x4b, 0xc6, 0xd2, 0x79, 0x20, 0x9a, 0xdb, 0xc0, 0xfe, 0x78, 0xcd, 0x5a, 0xf4,
    0x1f, 0xdd, 0xa8, 0x33, 0x88, 0x07, 0xc7, 0x31, 0xb1, 0x12, 0x10, 0x59, 0x27, 0x80, 0xec, 0x5f,
    0x60, 0x51, 0x7f, 0xa9, 0x19, 0xb5, 0x4a, 0x0d, 0x2d, 0xe5, 0x7a, 0x9f, 0x93, 0xc9, 0x9c, 0xef,
    0xa0, 0xe0, 0x3b, 0x4d, 0xae, 0x2a, 0xf5, 0xb0, 0xc8, 0xeb, 0xbb, 0x3c, 0x83, 0x53, 0x99, 0x61,
    0x17, 0x2b, 0x04, 0x7e, 0xba, 0x77, 0xd6, 0x26, 0xe1, 0x69, 0x14, 0x63, 0x55, 0x21, 0x0c, 0x7d,
];

fn aes_inv_sub_bytes(state: &mut [u8; 16]) {
    for b in state.iter_mut() {
        *b = AES_INV_SBOX[*b as usize];
    }
}

fn aes_inv_shift_rows(state: &mut [u8; 16]) {
    let s = *state;
    for r in 1..4 {
        for c in 0..4 {

            state[r * 4 + c] = s[r * 4 + (c + 4 - r) % 4];
        }
    }
}

fn gf_mul(mut a: u8, mut b: u8) -> u8 {
    let mut p = 0u8;
    for _ in 0..8 {
        if b & 1 != 0 {
            p ^= a;
        }
        let hi = a & 0x80;
        a <<= 1;
        if hi != 0 {
            a ^= 0x1b;
        }
        b >>= 1;
    }
    p
}

fn aes_inv_mix_columns(state: &mut [u8; 16]) {
    for c in 0..4 {
        let s0 = state[c];
        let s1 = state[4 + c];
        let s2 = state[8 + c];
        let s3 = state[12 + c];
        state[c] = gf_mul(0x0e, s0) ^ gf_mul(0x0b, s1) ^ gf_mul(0x0d, s2) ^ gf_mul(0x09, s3);
        state[4 + c] = gf_mul(0x09, s0) ^ gf_mul(0x0e, s1) ^ gf_mul(0x0b, s2) ^ gf_mul(0x0d, s3);
        state[8 + c] = gf_mul(0x0d, s0) ^ gf_mul(0x09, s1) ^ gf_mul(0x0e, s2) ^ gf_mul(0x0b, s3);
        state[12 + c] = gf_mul(0x0b, s0) ^ gf_mul(0x0d, s1) ^ gf_mul(0x09, s2) ^ gf_mul(0x0e, s3);
    }
}

fn aes_decrypt_block(block: &[u8; 16], w: &[u32]) -> [u8; 16] {
    let nr = w.len() / 4 - 1;
    let mut state = [0u8; 16];
    for c in 0..4 {
        for r in 0..4 {
            state[r * 4 + c] = block[4 * c + r];
        }
    }
    aes_add_round_key(&mut state, &w[4 * nr..4 * nr + 4]);
    for round in (1..nr).rev() {
        aes_inv_shift_rows(&mut state);
        aes_inv_sub_bytes(&mut state);
        aes_add_round_key(&mut state, &w[4 * round..4 * round + 4]);
        aes_inv_mix_columns(&mut state);
    }
    aes_inv_shift_rows(&mut state);
    aes_inv_sub_bytes(&mut state);
    aes_add_round_key(&mut state, &w[0..4]);
    let mut out = [0u8; 16];
    for c in 0..4 {
        for r in 0..4 {
            out[4 * c + r] = state[r * 4 + c];
        }
    }
    out
}

pub fn aes_decrypt_block_with_key(key: &[u8], block: &[u8; 16]) -> [u8; 16] {
    let w = aes_key_expansion(key);
    aes_decrypt_block(block, &w)
}

pub fn aes_cbc_encrypt(key: &[u8], iv: &[u8], plaintext: &[u8]) -> Result<Vec<u8>, String> {
    if key.len() != 16 && key.len() != 24 && key.len() != 32 {
        return Err(format!("AES-CBC: invalid key length {}", key.len()));
    }
    if iv.len() != 16 {
        return Err("AES-CBC: IV must be 16 bytes".to_string());
    }
    let w = aes_key_expansion(key);
    let pad = 16 - (plaintext.len() % 16);
    let mut padded = plaintext.to_vec();
    padded.extend(std::iter::repeat(pad as u8).take(pad));
    let mut prev = [0u8; 16];
    prev.copy_from_slice(iv);
    let mut out = Vec::with_capacity(padded.len());
    for chunk in padded.chunks(16) {
        let mut block = [0u8; 16];
        for i in 0..16 {
            block[i] = chunk[i] ^ prev[i];
        }
        let c = aes_encrypt_block(&block, &w);
        out.extend_from_slice(&c);
        prev = c;
    }
    Ok(out)
}

pub fn aes_cbc_encrypt_no_pad(key: &[u8], iv: &[u8], plaintext: &[u8]) -> Result<Vec<u8>, String> {
    if key.len() != 16 && key.len() != 24 && key.len() != 32 {
        return Err(format!("AES-CBC: invalid key length {}", key.len()));
    }
    if iv.len() != 16 {
        return Err("AES-CBC: IV must be 16 bytes".to_string());
    }
    if plaintext.len() % 16 != 0 {
        return Err("AES-CBC: data not block-aligned".to_string());
    }
    let w = aes_key_expansion(key);
    let mut prev = [0u8; 16];
    prev.copy_from_slice(iv);
    let mut out = Vec::with_capacity(plaintext.len());
    for chunk in plaintext.chunks(16) {
        let mut block = [0u8; 16];
        for i in 0..16 {
            block[i] = chunk[i] ^ prev[i];
        }
        let c = aes_encrypt_block(&block, &w);
        out.extend_from_slice(&c);
        prev = c;
    }
    Ok(out)
}

pub fn aes_cbc_decrypt_no_pad(key: &[u8], iv: &[u8], ciphertext: &[u8]) -> Result<Vec<u8>, String> {
    if key.len() != 16 && key.len() != 24 && key.len() != 32 {
        return Err(format!("AES-CBC: invalid key length {}", key.len()));
    }
    if iv.len() != 16 {
        return Err("AES-CBC: IV must be 16 bytes".to_string());
    }
    if ciphertext.is_empty() || ciphertext.len() % 16 != 0 {
        return Err("AES-CBC: ciphertext must be a positive multiple of 16 bytes".to_string());
    }
    let w = aes_key_expansion(key);
    let mut prev = [0u8; 16];
    prev.copy_from_slice(iv);
    let mut out = Vec::with_capacity(ciphertext.len());
    for chunk in ciphertext.chunks(16) {
        let mut block = [0u8; 16];
        block.copy_from_slice(chunk);
        let d = aes_decrypt_block(&block, &w);
        let mut plain = [0u8; 16];
        for i in 0..16 {
            plain[i] = d[i] ^ prev[i];
        }
        out.extend_from_slice(&plain);
        prev = block;
    }
    Ok(out)
}

pub fn aes_cbc_decrypt(key: &[u8], iv: &[u8], ciphertext: &[u8]) -> Result<Vec<u8>, String> {
    let mut out = aes_cbc_decrypt_no_pad(key, iv, ciphertext)?;

    let pad = *out.last().ok_or("AES-CBC: empty output")? as usize;
    if pad == 0 || pad > 16 {
        return Err("AES-CBC: bad padding".to_string());
    }
    if out.len() < pad {
        return Err("AES-CBC: bad padding".to_string());
    }
    let n = out.len();
    for &b in &out[n - pad..] {
        if b as usize != pad {
            return Err("AES-CBC: bad padding".to_string());
        }
    }
    out.truncate(n - pad);
    Ok(out)
}

pub fn aes_ctr_xor_with_key(
    key: &[u8],
    counter0: &[u8],
    counter_bits: u32,
    data: &[u8],
) -> Result<Vec<u8>, String> {
    if key.len() != 16 && key.len() != 24 && key.len() != 32 {
        return Err(format!("AES-CTR: invalid key length {}", key.len()));
    }
    if counter0.len() != 16 {
        return Err("AES-CTR: counter must be 16 bytes".to_string());
    }
    if counter_bits == 0 || counter_bits > 128 {
        return Err("AES-CTR: length must be in 1..=128".to_string());
    }
    let w = aes_key_expansion(key);
    let mut counter = [0u8; 16];
    counter.copy_from_slice(counter0);
    let mut out = Vec::with_capacity(data.len());
    let total_blocks = (data.len() + 15) / 16;
    let mut block_idx = 0u64;
    for chunk in data.chunks(16) {
        let ks = aes_encrypt_block(&counter, &w);
        for (i, b) in chunk.iter().enumerate() {
            out.push(b ^ ks[i]);
        }
        block_idx += 1;
        if block_idx as usize == total_blocks {
            break;
        }

        counter_inc(&mut counter, counter_bits as usize);
    }
    Ok(out)
}

fn counter_inc(counter: &mut [u8; 16], bits: usize) {

    let mut remaining = bits;
    let mut idx = 15;
    let mut carry: u16 = 1;
    while remaining > 0 && carry != 0 {
        let take = remaining.min(8);
        let mask: u16 = if take == 8 { 0xff } else { (1u16 << take) - 1 };
        let low = (counter[idx] as u16) & mask;
        let high = (counter[idx] as u16) & !mask;
        let sum = low + carry;
        let new_low = sum & mask;
        counter[idx] = (high | new_low) as u8;
        carry = sum >> take;
        remaining -= take;
        if idx == 0 {
            break;
        }
        idx -= 1;
    }
}

const AES_KW_IV: [u8; 8] = [0xa6, 0xa6, 0xa6, 0xa6, 0xa6, 0xa6, 0xa6, 0xa6];

pub fn aes_kw_wrap(kek: &[u8], plaintext: &[u8]) -> Result<Vec<u8>, String> {
    if kek.len() != 16 && kek.len() != 24 && kek.len() != 32 {
        return Err(format!("AES-KW: invalid KEK length {}", kek.len()));
    }
    if plaintext.len() % 8 != 0 || plaintext.is_empty() {
        return Err("AES-KW: plaintext must be a positive multiple of 8 bytes".to_string());
    }
    let n = plaintext.len() / 8;
    let w = aes_key_expansion(kek);
    let mut a = AES_KW_IV;
    let mut r: Vec<[u8; 8]> = (0..n)
        .map(|i| {
            let mut b = [0u8; 8];
            b.copy_from_slice(&plaintext[i * 8..(i + 1) * 8]);
            b
        })
        .collect();
    for j in 0..6 {
        for i in 0..n {
            let mut b = [0u8; 16];
            b[..8].copy_from_slice(&a);
            b[8..].copy_from_slice(&r[i]);
            let enc = aes_encrypt_block(&b, &w);
            a.copy_from_slice(&enc[..8]);
            let t = ((n * j) + i + 1) as u64;
            let t_be = t.to_be_bytes();
            for k in 0..8 {
                a[k] ^= t_be[k];
            }
            r[i].copy_from_slice(&enc[8..]);
        }
    }
    let mut out = Vec::with_capacity(8 * (n + 1));
    out.extend_from_slice(&a);
    for block in &r {
        out.extend_from_slice(block);
    }
    Ok(out)
}

pub fn aes_kw_unwrap(kek: &[u8], ciphertext: &[u8]) -> Result<Vec<u8>, String> {
    if kek.len() != 16 && kek.len() != 24 && kek.len() != 32 {
        return Err(format!("AES-KW: invalid KEK length {}", kek.len()));
    }
    if ciphertext.len() % 8 != 0 || ciphertext.len() < 16 {
        return Err("AES-KW: ciphertext must be a multiple of 8 bytes ≥ 16".to_string());
    }
    let n = ciphertext.len() / 8 - 1;
    let w = aes_key_expansion(kek);
    let mut a = [0u8; 8];
    a.copy_from_slice(&ciphertext[..8]);
    let mut r: Vec<[u8; 8]> = (0..n)
        .map(|i| {
            let mut b = [0u8; 8];
            b.copy_from_slice(&ciphertext[8 + i * 8..8 + (i + 1) * 8]);
            b
        })
        .collect();
    for j in (0..6).rev() {
        for i in (0..n).rev() {
            let t = ((n * j) + i + 1) as u64;
            let t_be = t.to_be_bytes();
            let mut b = [0u8; 16];
            for k in 0..8 {
                b[k] = a[k] ^ t_be[k];
            }
            b[8..].copy_from_slice(&r[i]);
            let dec = aes_decrypt_block(&b, &w);
            a.copy_from_slice(&dec[..8]);
            r[i].copy_from_slice(&dec[8..]);
        }
    }
    if !timing_safe_equal(&a, &AES_KW_IV) {
        return Err("AES-KW: integrity check failed".to_string());
    }
    let mut out = Vec::with_capacity(8 * n);
    for block in &r {
        out.extend_from_slice(block);
    }
    Ok(out)
}

fn gf128_mul(x: [u8; 16], y: [u8; 16]) -> [u8; 16] {

    let xv = u128::from_be_bytes(x);
    let mut v = u128::from_be_bytes(y);
    let mut z = 0u128;
    for i in 0..128 {
        if (xv >> (127 - i)) & 1 == 1 {
            z ^= v;
        }
        let lsb = (v & 1) == 1;
        v >>= 1;
        if lsb {
            v ^= 0xe1u128 << 120;
        }
    }
    z.to_be_bytes()
}

#[cfg(test)]
fn gf128_mul_ref(x: [u8; 16], y: [u8; 16]) -> [u8; 16] {
    let mut z = [0u8; 16];
    let mut v = y;
    for i in 0..128 {
        let bit = (x[i / 8] >> (7 - (i % 8))) & 1;
        if bit == 1 {
            for k in 0..16 {
                z[k] ^= v[k];
            }
        }
        let lsb = v[15] & 1;
        for k in (1..16).rev() {
            v[k] = (v[k] >> 1) | ((v[k - 1] & 1) << 7);
        }
        v[0] >>= 1;
        if lsb == 1 {
            v[0] ^= 0xe1;
        }
    }
    z
}

fn ghash(h: [u8; 16], aad: &[u8], ct: &[u8]) -> [u8; 16] {

    let mut y = [0u8; 16];
    let mut absorb = |chunk: &[u8]| {
        for c in chunk.chunks(16) {
            let mut block = [0u8; 16];
            block[..c.len()].copy_from_slice(c);
            for i in 0..16 {
                y[i] ^= block[i];
            }
            y = gf128_mul(y, h);
        }
    };
    absorb(aad);
    absorb(ct);
    let mut len_block = [0u8; 16];
    len_block[..8].copy_from_slice(&((aad.len() as u64) * 8).to_be_bytes());
    len_block[8..].copy_from_slice(&((ct.len() as u64) * 8).to_be_bytes());
    for i in 0..16 {
        y[i] ^= len_block[i];
    }
    gf128_mul(y, h)
}

fn gcm_j0(h: [u8; 16], iv: &[u8]) -> [u8; 16] {
    if iv.len() == 12 {
        let mut j0 = [0u8; 16];
        j0[..12].copy_from_slice(iv);
        j0[15] = 1;
        return j0;
    }
    let mut y = [0u8; 16];
    for c in iv.chunks(16) {
        let mut block = [0u8; 16];
        block[..c.len()].copy_from_slice(c);
        for i in 0..16 {
            y[i] ^= block[i];
        }
        y = gf128_mul(y, h);
    }
    let mut len_block = [0u8; 16];
    len_block[8..].copy_from_slice(&((iv.len() as u64) * 8).to_be_bytes());
    for i in 0..16 {
        y[i] ^= len_block[i];
    }
    gf128_mul(y, h)
}

fn aes_ctr_xor(w: &[u32], counter0: [u8; 16], data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len());
    let mut counter = counter0;
    for chunk in data.chunks(16) {
        let ks = aes_encrypt_block(&counter, w);
        for (i, b) in chunk.iter().enumerate() {
            out.push(b ^ ks[i]);
        }

        let inc = u32::from_be_bytes([counter[12], counter[13], counter[14], counter[15]])
            .wrapping_add(1);
        counter[12..16].copy_from_slice(&inc.to_be_bytes());
    }
    out
}

pub fn aes_gcm_encrypt(
    key: &[u8],
    iv: &[u8],
    aad: &[u8],
    plaintext: &[u8],
) -> Result<Vec<u8>, String> {
    if key.len() != 16 && key.len() != 24 && key.len() != 32 {
        return Err(format!("AES-GCM: invalid key length {}", key.len()));
    }
    if iv.is_empty() {
        return Err("AES-GCM: IV must be at least 1 byte".to_string());
    }
    let w = aes_key_expansion(key);
    let h = aes_encrypt_block(&[0u8; 16], &w);
    let j0 = gcm_j0(h, iv);
    let mut counter1 = j0;
    let inc = u32::from_be_bytes([counter1[12], counter1[13], counter1[14], counter1[15]])
        .wrapping_add(1);
    counter1[12..16].copy_from_slice(&inc.to_be_bytes());
    let ciphertext = aes_ctr_xor(&w, counter1, plaintext);
    let s = ghash(h, aad, &ciphertext);
    let ej0 = aes_encrypt_block(&j0, &w);
    let mut tag = [0u8; 16];
    for i in 0..16 {
        tag[i] = s[i] ^ ej0[i];
    }
    let mut out = ciphertext;
    out.extend_from_slice(&tag);
    Ok(out)
}

pub fn aes_gcm_decrypt(
    key: &[u8],
    iv: &[u8],
    aad: &[u8],
    ct_and_tag: &[u8],
) -> Result<Vec<u8>, String> {
    if key.len() != 16 && key.len() != 24 && key.len() != 32 {
        return Err(format!("AES-GCM: invalid key length {}", key.len()));
    }
    if ct_and_tag.len() < 16 {
        return Err("AES-GCM: input too short for tag".to_string());
    }
    let (ciphertext, tag) = ct_and_tag.split_at(ct_and_tag.len() - 16);
    aes_gcm_decrypt_split(key, iv, aad, ciphertext, tag)
}

pub fn aes_gcm_decrypt_split(
    key: &[u8],
    iv: &[u8],
    aad: &[u8],
    ciphertext: &[u8],
    tag: &[u8],
) -> Result<Vec<u8>, String> {
    if key.len() != 16 && key.len() != 24 && key.len() != 32 {
        return Err(format!("AES-GCM: invalid key length {}", key.len()));
    }
    if iv.is_empty() {
        return Err("AES-GCM: IV must be at least 1 byte".to_string());
    }
    if tag.is_empty() {
        return Err("AES-GCM: authentication tag mismatch".to_string());
    }
    let w = aes_key_expansion(key);
    let h = aes_encrypt_block(&[0u8; 16], &w);
    let j0 = gcm_j0(h, iv);
    let s = ghash(h, aad, ciphertext);
    let ej0 = aes_encrypt_block(&j0, &w);
    let mut expected_tag = [0u8; 16];
    for i in 0..16 {
        expected_tag[i] = s[i] ^ ej0[i];
    }
    let n = tag.len().min(16);
    if !timing_safe_equal(&expected_tag[..n], &tag[..n]) {
        return Err("AES-GCM: authentication tag mismatch".to_string());
    }
    let mut counter1 = j0;
    let inc = u32::from_be_bytes([counter1[12], counter1[13], counter1[14], counter1[15]])
        .wrapping_add(1);
    counter1[12..16].copy_from_slice(&inc.to_be_bytes());
    Ok(aes_ctr_xor(&w, counter1, ciphertext))
}

pub mod subtle {
    use super::digest_sha256;

    pub fn digest(algorithm: &str, data: &[u8]) -> Result<Vec<u8>, String> {
        match algorithm.to_ascii_uppercase().replace("-", "").as_str() {
            "SHA256" => Ok(digest_sha256(data).to_vec()),
            other => Err(format!("unsupported algorithm: {}", other)),
        }
    }
}

const BLAKE2B_IV: [u64; 8] = [
    0x6a09e667f3bcc908,
    0xbb67ae8584caa73b,
    0x3c6ef372fe94f82b,
    0xa54ff53a5f1d36f1,
    0x510e527fade682d1,
    0x9b05688c2b3e6c1f,
    0x1f83d9abfb41bd6b,
    0x5be0cd19137e2179,
];

const BLAKE2B_SIGMA: [[usize; 16]; 12] = [
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

fn blake2b_mix(v: &mut [u64; 16], a: usize, b: usize, c: usize, d: usize, x: u64, y: u64) {
    v[a] = v[a].wrapping_add(v[b]).wrapping_add(x);
    v[d] = (v[d] ^ v[a]).rotate_right(32);
    v[c] = v[c].wrapping_add(v[d]);
    v[b] = (v[b] ^ v[c]).rotate_right(24);
    v[a] = v[a].wrapping_add(v[b]).wrapping_add(y);
    v[d] = (v[d] ^ v[a]).rotate_right(16);
    v[c] = v[c].wrapping_add(v[d]);
    v[b] = (v[b] ^ v[c]).rotate_right(63);
}

fn blake2b_compress(h: &mut [u64; 8], block: &[u8; 128], t: u128, last: bool) {
    let mut v: [u64; 16] = [0; 16];
    v[..8].copy_from_slice(h);
    v[8..].copy_from_slice(&BLAKE2B_IV);
    v[12] ^= t as u64;
    v[13] ^= (t >> 64) as u64;
    if last {
        v[14] = !v[14];
    }
    let mut m: [u64; 16] = [0; 16];
    for i in 0..16 {
        let off = i * 8;
        m[i] = u64::from_le_bytes([
            block[off],
            block[off + 1],
            block[off + 2],
            block[off + 3],
            block[off + 4],
            block[off + 5],
            block[off + 6],
            block[off + 7],
        ]);
    }
    for round in 0..12 {
        let s = &BLAKE2B_SIGMA[round];
        blake2b_mix(&mut v, 0, 4, 8, 12, m[s[0]], m[s[1]]);
        blake2b_mix(&mut v, 1, 5, 9, 13, m[s[2]], m[s[3]]);
        blake2b_mix(&mut v, 2, 6, 10, 14, m[s[4]], m[s[5]]);
        blake2b_mix(&mut v, 3, 7, 11, 15, m[s[6]], m[s[7]]);
        blake2b_mix(&mut v, 0, 5, 10, 15, m[s[8]], m[s[9]]);
        blake2b_mix(&mut v, 1, 6, 11, 12, m[s[10]], m[s[11]]);
        blake2b_mix(&mut v, 2, 7, 8, 13, m[s[12]], m[s[13]]);
        blake2b_mix(&mut v, 3, 4, 9, 14, m[s[14]], m[s[15]]);
    }
    for i in 0..8 {
        h[i] ^= v[i] ^ v[i + 8];
    }
}

pub fn blake2b(input: &[u8], key: &[u8], out_len: usize) -> Result<Vec<u8>, String> {
    if out_len == 0 || out_len > 64 {
        return Err("blake2b: out_len must be 1..=64".into());
    }
    if key.len() > 64 {
        return Err("blake2b: key length must be 0..=64".into());
    }
    let mut h = BLAKE2B_IV;

    h[0] ^= 0x01010000 | ((key.len() as u64) << 8) | (out_len as u64);

    let mut buf: Vec<u8> = Vec::new();
    if !key.is_empty() {
        let mut padded = [0u8; 128];
        padded[..key.len()].copy_from_slice(key);
        buf.extend_from_slice(&padded);
    }
    buf.extend_from_slice(input);

    let mut t: u128 = 0;
    let mut i = 0;
    while i + 128 < buf.len() {
        let mut block = [0u8; 128];
        block.copy_from_slice(&buf[i..i + 128]);
        t = t.wrapping_add(128);
        blake2b_compress(&mut h, &block, t, false);
        i += 128;
    }

    let remaining = buf.len() - i;
    let mut last_block = [0u8; 128];
    last_block[..remaining].copy_from_slice(&buf[i..]);
    t = t.wrapping_add(remaining as u128);
    blake2b_compress(&mut h, &last_block, t, true);

    let mut out = Vec::with_capacity(out_len);
    for word in h.iter().take((out_len + 7) / 8) {
        for b in word.to_le_bytes() {
            if out.len() < out_len {
                out.push(b);
            }
        }
    }
    out.truncate(out_len);
    Ok(out)
}

const ARGON2_VERSION: u32 = 0x13;
const ARGON2ID_TYPE: u32 = 2;
const ARGON2_BLOCK_SIZE: usize = 1024;
const ARGON2_QWORDS: usize = 128;
const ARGON2_SYNC_POINTS: usize = 4;

#[derive(Debug, Clone)]
pub struct Argon2idParams {
    pub t_cost: u32,
    pub m_kib: u32,
    pub parallelism: u32,
    pub tau: u32,
}

#[derive(Debug)]
pub enum Argon2Error {
    InvalidParam(&'static str),
    Crypto(String),
}
impl std::fmt::Display for Argon2Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Argon2Error::InvalidParam(s) => write!(f, "argon2: {}", s),
            Argon2Error::Crypto(s) => write!(f, "argon2 crypto: {}", s),
        }
    }
}
impl std::error::Error for Argon2Error {}

type Block = [u64; ARGON2_QWORDS];
#[inline]
fn block_zero() -> Block {
    [0u64; ARGON2_QWORDS]
}
fn block_from_bytes(b: &[u8]) -> Block {
    let mut r = block_zero();
    for (i, c) in b.chunks_exact(8).enumerate().take(ARGON2_QWORDS) {
        r[i] = u64::from_le_bytes(c.try_into().unwrap());
    }
    r
}
fn block_to_bytes(b: &Block) -> Vec<u8> {
    let mut o = Vec::with_capacity(ARGON2_BLOCK_SIZE);
    for &w in b {
        o.extend_from_slice(&w.to_le_bytes());
    }
    o
}
fn block_xor(a: &Block, b: &Block) -> Block {
    let mut r = block_zero();
    for i in 0..ARGON2_QWORDS {
        r[i] = a[i] ^ b[i];
    }
    r
}

pub fn argon2_h_prime(input: &[u8], tau: u32) -> Result<Vec<u8>, Argon2Error> {
    let mut tagged = Vec::with_capacity(4 + input.len());
    tagged.extend_from_slice(&tau.to_le_bytes());
    tagged.extend_from_slice(input);
    if tau <= 64 {
        return blake2b(&tagged, &[], tau as usize).map_err(Argon2Error::Crypto);
    }
    let r = ((tau + 31) / 32) as usize - 2;
    let mut out = Vec::with_capacity(tau as usize);
    let mut v = blake2b(&tagged, &[], 64).map_err(Argon2Error::Crypto)?;
    out.extend_from_slice(&v[..32]);
    for _ in 1..r {
        v = blake2b(&v, &[], 64).map_err(Argon2Error::Crypto)?;
        out.extend_from_slice(&v[..32]);
    }
    let final_len = (tau as usize) - 32 * r;
    let vf = blake2b(&v, &[], final_len).map_err(Argon2Error::Crypto)?;
    out.extend_from_slice(&vf);
    Ok(out)
}

#[inline]
fn gb(v: &mut [u64; 16], a: usize, b: usize, c: usize, d: usize) {
    let add = |x: u64, y: u64| {
        let lx = x & 0xFFFFFFFF;
        let ly = y & 0xFFFFFFFF;
        x.wrapping_add(y)
            .wrapping_add(2u64.wrapping_mul(lx).wrapping_mul(ly))
    };
    v[a] = add(v[a], v[b]);
    v[d] = (v[d] ^ v[a]).rotate_right(32);
    v[c] = add(v[c], v[d]);
    v[b] = (v[b] ^ v[c]).rotate_right(24);
    v[a] = add(v[a], v[b]);
    v[d] = (v[d] ^ v[a]).rotate_right(16);
    v[c] = add(v[c], v[d]);
    v[b] = (v[b] ^ v[c]).rotate_right(63);
}

fn permute_p(v: &mut [u64; 16]) {
    gb(v, 0, 4, 8, 12);
    gb(v, 1, 5, 9, 13);
    gb(v, 2, 6, 10, 14);
    gb(v, 3, 7, 11, 15);
    gb(v, 0, 5, 10, 15);
    gb(v, 1, 6, 11, 12);
    gb(v, 2, 7, 8, 13);
    gb(v, 3, 4, 9, 14);
}

fn compress_g(x: &Block, y: &Block) -> Block {
    let r = block_xor(x, y);
    let mut z = r;

    for row in 0..8 {
        let off = row * 16;
        let mut v = [0u64; 16];
        v.copy_from_slice(&z[off..off + 16]);
        permute_p(&mut v);
        z[off..off + 16].copy_from_slice(&v);
    }

    for col in 0..8 {
        let mut v = [0u64; 16];
        for row in 0..8 {
            v[row * 2] = z[row * 16 + col * 2];
            v[row * 2 + 1] = z[row * 16 + col * 2 + 1];
        }
        permute_p(&mut v);
        for row in 0..8 {
            z[row * 16 + col * 2] = v[row * 2];
            z[row * 16 + col * 2 + 1] = v[row * 2 + 1];
        }
    }
    block_xor(&z, &r)
}

fn argon2i_address_block(
    pass: u64,
    lane: u64,
    slice: u64,
    total_blocks: u64,
    total_passes: u64,
    ty: u64,
    counter: u64,
) -> Block {
    let zero = block_zero();
    let mut input = block_zero();
    input[0] = pass;
    input[1] = lane;
    input[2] = slice;
    input[3] = total_blocks;
    input[4] = total_passes;
    input[5] = ty;
    input[6] = counter;

    let first = compress_g(&zero, &input);
    compress_g(&zero, &first)
}

fn map_index(j1: u32, ref_area_size: usize) -> usize {
    let x = ((j1 as u64).wrapping_mul(j1 as u64)) >> 32;
    let y = ((ref_area_size as u64).wrapping_mul(x)) >> 32;
    (ref_area_size as u64).wrapping_sub(1).wrapping_sub(y) as usize
}

pub fn argon2id_hash(
    password: &[u8],
    salt: &[u8],
    params: &Argon2idParams,
) -> Result<Vec<u8>, Argon2Error> {
    if params.parallelism != 1 {
        return Err(Argon2Error::InvalidParam("p=1 only"));
    }
    if params.t_cost < 1 {
        return Err(Argon2Error::InvalidParam("t >= 1"));
    }
    if params.tau < 4 {
        return Err(Argon2Error::InvalidParam("tau >= 4"));
    }
    if salt.len() < 8 {
        return Err(Argon2Error::InvalidParam("salt >= 8"));
    }
    if params.m_kib < 8 {
        return Err(Argon2Error::InvalidParam("m >= 8"));
    }

    let p = params.parallelism;
    let tau = params.tau;
    let t = params.t_cost;

    let m_prime = 4 * p * (params.m_kib / (4 * p));
    let q = (m_prime / p) as usize;
    let segment_length = q / ARGON2_SYNC_POINTS;

    let mut h0_in = Vec::new();
    h0_in.extend_from_slice(&p.to_le_bytes());
    h0_in.extend_from_slice(&tau.to_le_bytes());
    h0_in.extend_from_slice(&params.m_kib.to_le_bytes());
    h0_in.extend_from_slice(&t.to_le_bytes());
    h0_in.extend_from_slice(&ARGON2_VERSION.to_le_bytes());
    h0_in.extend_from_slice(&ARGON2ID_TYPE.to_le_bytes());
    h0_in.extend_from_slice(&(password.len() as u32).to_le_bytes());
    h0_in.extend_from_slice(password);
    h0_in.extend_from_slice(&(salt.len() as u32).to_le_bytes());
    h0_in.extend_from_slice(salt);
    h0_in.extend_from_slice(&0u32.to_le_bytes());
    h0_in.extend_from_slice(&0u32.to_le_bytes());
    let h0 = blake2b(&h0_in, &[], 64).map_err(Argon2Error::Crypto)?;

    let mut mem: Vec<Block> = vec![block_zero(); q];

    let mut ext = h0.clone();
    ext.extend_from_slice(&0u32.to_le_bytes());
    ext.extend_from_slice(&0u32.to_le_bytes());
    mem[0] = block_from_bytes(&argon2_h_prime(&ext, ARGON2_BLOCK_SIZE as u32)?);
    let mut ext = h0.clone();
    ext.extend_from_slice(&1u32.to_le_bytes());
    ext.extend_from_slice(&0u32.to_le_bytes());
    mem[1] = block_from_bytes(&argon2_h_prime(&ext, ARGON2_BLOCK_SIZE as u32)?);

    for pass in 0..t as u64 {
        for slice in 0..ARGON2_SYNC_POINTS as u64 {

            let use_argon2i = pass == 0 && slice < 2;
            let seg_start = (slice as usize) * segment_length;

            let mut addr_block = block_zero();
            let mut addr_counter: u64 = 0;
            if use_argon2i {
                addr_counter = 1;
                addr_block = argon2i_address_block(
                    pass,
                    0,
                    slice,
                    q as u64,
                    t as u64,
                    ARGON2ID_TYPE as u64,
                    addr_counter,
                );
            }

            let start_in_seg = if pass == 0 && slice == 0 { 2 } else { 0 };
            for idx_in_seg in start_in_seg..segment_length {
                let j_abs = seg_start + idx_in_seg;

                if use_argon2i && idx_in_seg > 0 && idx_in_seg % ARGON2_QWORDS == 0 {
                    addr_counter += 1;
                    addr_block = argon2i_address_block(
                        pass,
                        0,
                        slice,
                        q as u64,
                        t as u64,
                        ARGON2ID_TYPE as u64,
                        addr_counter,
                    );
                }
                let prev_idx = if j_abs == 0 { q - 1 } else { j_abs - 1 };
                let prev_block = mem[prev_idx];

                let pseudo = if use_argon2i {
                    addr_block[idx_in_seg % ARGON2_QWORDS]
                } else {
                    prev_block[0]
                };
                let j1 = (pseudo & 0xFFFFFFFF) as u32;

                let ref_area_size: usize = if pass == 0 {
                    j_abs - 1
                } else {

                    q - segment_length + idx_in_seg - 1
                };
                if ref_area_size == 0 {
                    continue;
                }

                let rel = map_index(j1, ref_area_size);
                let ref_index = if pass == 0 {
                    rel
                } else {
                    let start = ((slice as usize + 1) * segment_length) % q;
                    (start + rel) % q
                };
                let ref_block = mem[ref_index];

                let new_block = compress_g(&prev_block, &ref_block);
                if pass == 0 {
                    mem[j_abs] = new_block;
                } else {
                    mem[j_abs] = block_xor(&mem[j_abs], &new_block);
                }
            }
        }
    }

    let c_bytes = block_to_bytes(&mem[q - 1]);
    argon2_h_prime(&c_bytes, tau)
}

pub fn p256_mod_mul_solinas_v3(a: &BigUInt, b: &BigUInt) -> BigUInt {
    let product = a.mul(b);
    let mut t = [0u32; 16];
    for (i, &l) in product.limbs().iter().take(16).enumerate() {
        t[i] = l;
    }
    let mk = |arr: [u32; 8]| BigUInt::from_limbs(arr.to_vec());
    let s1 = mk([t[0], t[1], t[2], t[3], t[4], t[5], t[6], t[7]]);
    let s2 = mk([0, 0, 0, t[11], t[12], t[13], t[14], t[15]]);
    let s3 = mk([0, 0, 0, t[12], t[13], t[14], t[15], 0]);
    let s4 = mk([t[8], t[9], t[10], 0, 0, 0, t[14], t[15]]);
    let s5 = mk([t[9], t[10], t[11], t[13], t[14], t[15], t[13], t[8]]);
    let s6 = mk([t[11], t[12], t[13], 0, 0, 0, t[8], t[10]]);
    let s7 = mk([t[12], t[13], t[14], t[15], 0, 0, t[9], t[11]]);
    let s8 = mk([t[13], t[14], t[15], t[8], t[9], t[10], 0, t[12]]);
    let s9 = mk([t[14], t[15], 0, t[9], t[10], t[11], 0, t[13]]);

    let pos = s1.add(&s2).add(&s2).add(&s3).add(&s3).add(&s4).add(&s5);

    let neg = s6.add(&s7).add(&s8).add(&s9);
    let p = p256_p();
    use std::cmp::Ordering;

    let mut result = if pos.cmp(&neg) != Ordering::Less {
        pos.sub(&neg)
    } else {
        let five_p = p.add(&p).add(&p).add(&p).add(&p);
        pos.add(&five_p).sub(&neg)
    };
    while result.cmp(&p) != Ordering::Less {
        result = result.sub(&p);
    }
    result
}

#[cfg(test)]
mod _lever5_tests {
    use super::*;
    fn lcg(seed: &mut u64) -> u8 {
        *seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (*seed >> 33) as u8
    }
    #[test]
    fn aes_ttable_artifact_matches_reference() {
        let mut s = 0x9E3779B97F4A7C15u64;
        for _ in 0..3000 {
            for keylen in [16usize, 24, 32] {
                let key: Vec<u8> = (0..keylen).map(|_| lcg(&mut s)).collect();
                let w = aes_key_expansion(&key);
                let mut block = [0u8; 16];
                block.iter_mut().for_each(|b| *b = lcg(&mut s));
                assert_eq!(
                    aes_encrypt_block(&block, &w),
                    aes_encrypt_block_fast(&block, &w),
                    "T-table diverged (keylen={keylen})"
                );
            }
        }
    }
    #[test]
    fn zeroize_primitives_wipe_bytes_and_words() {
        let mut bytes = [0xa5u8; 32];
        zeroize_bytes(&mut bytes);
        assert_eq!(bytes, [0u8; 32]);

        let mut words = [0xa5a5_5a5au32, 0xffff_ffff, 0x1234_5678];
        zeroize_words(&mut words);
        assert_eq!(words, [0u32; 3]);

        let mut big = BigUInt(vec![0xfeed_face, 0xdead_beef, 0x1234_5678]);
        big.zeroize_limbs();
        assert_eq!(big.limbs(), &[0, 0, 0]);
    }
    #[test]
    fn gf128_u128_matches_reference() {
        let mut s = 0xDEADBEEF12345678u64;
        for _ in 0..20000 {
            let mut x = [0u8; 16];
            let mut y = [0u8; 16];
            x.iter_mut().for_each(|b| *b = lcg(&mut s));
            y.iter_mut().for_each(|b| *b = lcg(&mut s));
            assert_eq!(gf128_mul(x, y), gf128_mul_ref(x, y), "u128 GHASH diverged");
        }
    }
}

#[cfg(test)]
mod mod_inv_mont_route_tests {
    use super::*;

    fn xorshift64(state: &mut u64) -> u64 {
        let mut x = *state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        *state = x;
        x
    }

    fn random_below(state: &mut u64, m: &BigUInt) -> BigUInt {
        let mut bytes = [0u8; 32];
        for chunk in bytes.chunks_mut(8) {
            chunk.copy_from_slice(&xorshift64(state).to_be_bytes());
        }
        BigUInt::from_be_bytes(&bytes).modulo(m)
    }

    #[test]
    fn mod_inv_fermat_mont_route_differential() {
        let two = BigUInt::from_be_bytes(&[2]);
        let moduli = [curve_p256().p, curve_p256().n, curve_p384().n];
        let mut state: u64 = 0x9E3779B97F4A7C15;
        for m in &moduli {
            let m_minus_2 = m.sub(&two);
            let mut tested = 0;
            while tested < 128 {
                let a = random_below(&mut state, m);
                if a.is_zero() {
                    continue;
                }
                let new = mod_inv_fermat(&a, m);
                let old = mod_pow_mont(&a, &m_minus_2, m);
                assert_eq!(
                    new.to_be_bytes(48),
                    old.to_be_bytes(48),
                    "mod_inv_fermat Mont route diverged from mod_pow"
                );

                let one = BigUInt::one();
                assert_eq!(
                    a.mul(&new).modulo(m).to_be_bytes(48),
                    one.to_be_bytes(48),
                    "inverse property violated"
                );
                tested += 1;
            }
        }
    }
}
