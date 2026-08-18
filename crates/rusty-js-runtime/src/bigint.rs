
use std::cmp::Ordering;

#[derive(Clone, Debug)]
pub struct JsBigInt {

    sign: i8,

    mag: Vec<u32>,
}

impl PartialEq for JsBigInt {
    fn eq(&self, other: &Self) -> bool {
        self.sign == other.sign && self.mag == other.mag
    }
}
impl Eq for JsBigInt {}

impl std::fmt::Display for JsBigInt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.to_decimal())
    }
}

impl JsBigInt {

    pub fn byte_size(&self) -> usize {
        self.mag.len().saturating_mul(std::mem::size_of::<u32>())
    }

    pub fn zero() -> Self {
        JsBigInt {
            sign: 0,
            mag: vec![0],
        }
    }
    pub fn one() -> Self {
        JsBigInt {
            sign: 1,
            mag: vec![1],
        }
    }
    pub fn neg_one() -> Self {
        JsBigInt {
            sign: -1,
            mag: vec![1],
        }
    }

    pub fn is_zero(&self) -> bool {
        self.sign == 0
    }
    pub fn is_negative(&self) -> bool {
        self.sign < 0
    }

    pub fn to_be_bytes(&self) -> Vec<u8> {
        if self.is_zero() {
            return Vec::new();
        }
        let mut out = Vec::with_capacity(self.mag.len() * 4);
        for &limb in self.mag.iter().rev() {
            out.extend_from_slice(&limb.to_be_bytes());
        }
        let first_nz = out.iter().position(|&b| b != 0).unwrap_or(out.len());
        out.drain(..first_nz);
        out
    }

    pub fn from_be_bytes(bytes: &[u8]) -> Self {
        let mut mag: Vec<u32> = Vec::new();
        let mut i = bytes.len();
        while i > 0 {
            let start = i.saturating_sub(4);
            let mut limb = 0u32;
            for &b in &bytes[start..i] {
                limb = (limb << 8) | b as u32;
            }
            mag.push(limb);
            i = start;
        }
        while mag.len() > 1 && *mag.last().unwrap() == 0 {
            mag.pop();
        }
        if mag.is_empty() {
            mag.push(0);
        }
        let sign = if mag.len() == 1 && mag[0] == 0 { 0 } else { 1 };
        JsBigInt { sign, mag }
    }

    pub fn mag_bit_len(&self) -> u32 {
        let n = self.mag.len();
        if n == 0 {
            return 0;
        }
        let top = self.mag[n - 1];
        if top == 0 {
            return ((n - 1) as u32) * 32;
        }
        (n as u32) * 32 - top.leading_zeros()
    }

    pub fn from_i64(v: i64) -> Self {
        if v == 0 {
            return Self::zero();
        }
        let sign = if v < 0 { -1 } else { 1 };
        let u = if v == i64::MIN {
            (i64::MAX as u64) + 1
        } else {
            v.unsigned_abs()
        };
        let lo = (u & 0xffff_ffff) as u32;
        let hi = (u >> 32) as u32;
        let mag = if hi == 0 { vec![lo] } else { vec![lo, hi] };
        JsBigInt { sign, mag }
    }

    pub fn to_u64_wrapping(&self) -> u64 {
        let lo = self.mag.first().copied().unwrap_or(0) as u64;
        let hi = self.mag.get(1).copied().unwrap_or(0) as u64;
        let m = lo | (hi << 32);
        if self.sign < 0 {
            m.wrapping_neg()
        } else {
            m
        }
    }

    pub fn from_u64(v: u64) -> Self {
        if v == 0 {
            return Self::zero();
        }
        let lo = (v & 0xffff_ffff) as u32;
        let hi = (v >> 32) as u32;
        let mag = if hi == 0 { vec![lo] } else { vec![lo, hi] };
        JsBigInt { sign: 1, mag }
    }

    pub fn from_decimal(s: &str) -> Option<Self> {
        let s = s.trim();
        if s.is_empty() {
            return Some(Self::zero());
        }
        let (sign_byte, rest) = match s.as_bytes()[0] {
            b'-' => (-1i8, &s[1..]),
            b'+' => (1i8, &s[1..]),
            _ => (1i8, s),
        };
        if rest.is_empty() {
            return None;
        }

        let (radix, digits): (u32, &str) = if rest.len() >= 2 && rest.as_bytes()[0] == b'0' {
            match rest.as_bytes()[1] {
                b'x' | b'X' => (16, &rest[2..]),
                b'o' | b'O' => (8, &rest[2..]),
                b'b' | b'B' => (2, &rest[2..]),
                _ => (10, rest),
            }
        } else {
            (10, rest)
        };
        if radix != 10 && rest.len() != s.len() {
            return None;
        }
        if digits.is_empty() {
            return None;
        }
        let mut mag = vec![0u32];
        if radix == 10 {
            let bytes = digits.as_bytes();
            if !bytes.iter().all(|b| b.is_ascii_digit()) {
                return None;
            }
            let mut i = 0;
            while i < bytes.len() {
                let take = (bytes.len() - i).min(9);
                let chunk: u32 = std::str::from_utf8(&bytes[i..i + take])
                    .ok()?
                    .parse()
                    .ok()?;
                let mul: u32 = 10u32.pow(take as u32);
                mag_mul_small(&mut mag, mul);
                mag_add_small(&mut mag, chunk);
                i += take;
            }
        } else {
            for c in digits.bytes() {
                let d = match (c, radix) {
                    (b'0'..=b'9', _) => (c - b'0') as u32,
                    (b'a'..=b'f', 16) => (c - b'a' + 10) as u32,
                    (b'A'..=b'F', 16) => (c - b'A' + 10) as u32,
                    _ => return None,
                };
                if d >= radix {
                    return None;
                }
                mag_mul_small(&mut mag, radix);
                mag_add_small(&mut mag, d);
            }
        }
        mag_trim(&mut mag);
        let sign = if mag_is_zero(&mag) { 0 } else { sign_byte };
        Some(JsBigInt { sign, mag })
    }

    pub fn to_decimal(&self) -> String {
        if self.is_zero() {
            return "0".into();
        }
        let mut limbs = self.mag.clone();
        let mut chunks: Vec<u32> = Vec::new();
        while !mag_is_zero(&limbs) {
            let rem = mag_div_small(&mut limbs, 1_000_000_000);
            chunks.push(rem);
        }
        let mut out = String::new();
        if self.sign < 0 {
            out.push('-');
        }
        let last = chunks.pop().unwrap();
        out.push_str(&format!("{}", last));
        for c in chunks.iter().rev() {
            out.push_str(&format!("{:09}", c));
        }
        out
    }

    pub fn to_radix(&self, radix: u32) -> String {
        if radix == 10 {
            return self.to_decimal();
        }
        assert!((2..=36).contains(&radix));
        if self.is_zero() {
            return "0".into();
        }
        let mut limbs = self.mag.clone();
        let mut digits: Vec<u32> = Vec::new();
        while !mag_is_zero(&limbs) {
            let rem = mag_div_small(&mut limbs, radix);
            digits.push(rem);
        }
        let mut out = String::new();
        if self.sign < 0 {
            out.push('-');
        }
        for d in digits.iter().rev() {
            out.push(std::char::from_digit(*d, radix).unwrap());
        }
        out
    }

    pub fn to_f64(&self) -> f64 {
        if self.is_zero() {
            return 0.0;
        }
        let bit_len = self.mag_bit_len();
        let negative = self.sign < 0;
        let sign_bit = if negative { 1u64 << 63 } else { 0 };
        if bit_len <= 53 {
            let mut exact = 0u64;
            for &limb in self.mag.iter().rev() {
                exact = (exact << 32) | limb as u64;
            }
            let n = exact as f64;
            return if negative { -n } else { n };
        }

        let mut exponent = bit_len as i32 - 1;
        if exponent > 1023 {
            return if negative {
                f64::NEG_INFINITY
            } else {
                f64::INFINITY
            };
        }

        let shift = (bit_len - 53) as usize;
        let mut mantissa = mag_shr_to_u64(&self.mag, shift);
        let halfway_bit = shift - 1;
        let round_up = if mag_bit(&self.mag, halfway_bit) {
            mag_any_bits_below(&self.mag, halfway_bit) || (mantissa & 1) == 1
        } else {
            false
        };
        if round_up {
            mantissa += 1;
            if mantissa == (1u64 << 53) {
                mantissa >>= 1;
                exponent += 1;
                if exponent > 1023 {
                    return if negative {
                        f64::NEG_INFINITY
                    } else {
                        f64::INFINITY
                    };
                }
            }
        }

        let biased = (exponent + 1023) as u64;
        let fraction = mantissa & ((1u64 << 52) - 1);
        f64::from_bits(sign_bit | (biased << 52) | fraction)
    }

    pub fn cmp_f64(&self, n: f64) -> Option<Ordering> {
        if n.is_nan() {
            return None;
        }
        if n.is_infinite() {
            return Some(if n > 0.0 {
                Ordering::Less
            } else {
                Ordering::Greater
            });
        }

        let self_f = self.to_f64();
        if self_f.is_finite() && (n.abs() < 1e15) && (self_f.abs() < 1e15) {
            return self_f.partial_cmp(&n);
        }

        if n == 0.0 {
            return Some(match self.sign {
                -1 => Ordering::Less,
                0 => Ordering::Equal,
                _ => Ordering::Greater,
            });
        }
        let floor_n = n.trunc();

        let nb = if floor_n.abs() < (i64::MAX as f64) {
            JsBigInt::from_i64(floor_n as i64)
        } else {
            JsBigInt::from_f64_trunc(floor_n)
        };
        let ord = self.cmp(&nb);

        if ord == Ordering::Equal && (n - floor_n) != 0.0 {
            return Some(if n > floor_n {
                Ordering::Less
            } else {
                Ordering::Greater
            });
        }
        Some(ord)
    }

    pub fn from_f64_trunc(n: f64) -> Self {
        if n == 0.0 || !n.is_finite() {
            return JsBigInt::from_i64(0);
        }
        let neg = n < 0.0;
        let bits = n.abs().to_bits();
        let biased_exp = ((bits >> 52) & 0x7ff) as i64;
        let frac = bits & 0x000f_ffff_ffff_ffff;

        let mantissa = if biased_exp == 0 {
            frac
        } else {
            frac | (1u64 << 52)
        };
        let shift = biased_exp - 1075;
        let mut b = JsBigInt::from_u64(mantissa);
        if shift > 0 {
            b = b.shl(&JsBigInt::from_i64(shift)).unwrap_or(b);
        } else if shift < 0 {
            b = b.shr(&JsBigInt::from_i64(-shift)).unwrap_or(b);
        }
        if neg {
            b = b.neg();
        }
        b
    }

    pub fn cmp(&self, other: &JsBigInt) -> Ordering {
        match self.sign.cmp(&other.sign) {
            Ordering::Equal => {}
            ord => return ord,
        }
        if self.sign == 0 {
            return Ordering::Equal;
        }
        let abs_ord = mag_cmp(&self.mag, &other.mag);
        if self.sign < 0 {
            abs_ord.reverse()
        } else {
            abs_ord
        }
    }

    pub fn neg(&self) -> JsBigInt {
        JsBigInt {
            sign: -self.sign,
            mag: self.mag.clone(),
        }
    }

    pub fn add(&self, other: &JsBigInt) -> JsBigInt {
        if self.is_zero() {
            return other.clone();
        }
        if other.is_zero() {
            return self.clone();
        }
        if self.sign == other.sign {
            let mut mag = mag_add(&self.mag, &other.mag);
            mag_trim(&mut mag);
            JsBigInt {
                sign: self.sign,
                mag,
            }
        } else {

            match mag_cmp(&self.mag, &other.mag) {
                Ordering::Greater => {
                    let mut mag = mag_sub(&self.mag, &other.mag);
                    mag_trim(&mut mag);
                    JsBigInt {
                        sign: self.sign,
                        mag,
                    }
                }
                Ordering::Less => {
                    let mut mag = mag_sub(&other.mag, &self.mag);
                    mag_trim(&mut mag);
                    JsBigInt {
                        sign: other.sign,
                        mag,
                    }
                }
                Ordering::Equal => JsBigInt::zero(),
            }
        }
    }

    pub fn sub(&self, other: &JsBigInt) -> JsBigInt {
        self.add(&other.neg())
    }

    pub fn mul(&self, other: &JsBigInt) -> JsBigInt {
        if self.is_zero() || other.is_zero() {
            return JsBigInt::zero();
        }
        let mut mag = mag_mul(&self.mag, &other.mag);
        mag_trim(&mut mag);
        let sign = if self.sign == other.sign { 1 } else { -1 };
        JsBigInt { sign, mag }
    }

    pub fn divmod(&self, divisor: &JsBigInt) -> Option<(JsBigInt, JsBigInt)> {
        if divisor.is_zero() {
            return None;
        }
        if self.is_zero() {
            return Some((JsBigInt::zero(), JsBigInt::zero()));
        }
        let (q_mag, r_mag) = mag_divmod(&self.mag, &divisor.mag);
        let q_sign = if self.sign == divisor.sign { 1 } else { -1 };
        let q_is_zero = mag_is_zero(&q_mag);
        let r_is_zero = mag_is_zero(&r_mag);
        let q = JsBigInt {
            sign: if q_is_zero { 0 } else { q_sign },
            mag: if q_is_zero { vec![0] } else { q_mag },
        };

        let r = JsBigInt {
            sign: if r_is_zero { 0 } else { self.sign },
            mag: if r_is_zero { vec![0] } else { r_mag },
        };
        Some((q, r))
    }

    pub fn shl(&self, n: &JsBigInt) -> Option<JsBigInt> {
        if n.is_negative() {
            return self.shr(&n.neg());
        }
        if self.is_zero() {
            return Some(JsBigInt::zero());
        }
        let nf = n.to_f64();

        if !nf.is_finite() || nf > (1u64 << 24) as f64 {
            return None;
        }
        let bits = nf as u32;
        let limb_shift = (bits / 32) as usize;
        let bit_shift = bits % 32;
        let mut out = vec![0u32; self.mag.len() + limb_shift + 1];
        for (i, &l) in self.mag.iter().enumerate() {
            let lo = (l as u64) << bit_shift;
            out[i + limb_shift] |= (lo & 0xffff_ffff) as u32;
            out[i + limb_shift + 1] |= (lo >> 32) as u32;
        }
        mag_trim(&mut out);
        Some(JsBigInt {
            sign: self.sign,
            mag: out,
        })
    }

    pub fn shr(&self, n: &JsBigInt) -> Option<JsBigInt> {
        if n.is_negative() {
            return self.shl(&n.neg());
        }
        if self.is_zero() {
            return Some(JsBigInt::zero());
        }
        let nf = n.to_f64();
        if !nf.is_finite() {
            return None;
        }
        let bits = nf as u64;
        if bits >= (self.mag.len() as u64) * 32 + 1 {
            return Some(if self.sign < 0 {
                JsBigInt::neg_one()
            } else {
                JsBigInt::zero()
            });
        }
        let bits = bits as u32;
        let limb_shift = (bits / 32) as usize;
        let bit_shift = bits % 32;
        let mut out: Vec<u32> = self.mag.iter().skip(limb_shift).copied().collect();
        if bit_shift > 0 {
            for i in 0..out.len() {
                let lo = out[i] >> bit_shift;
                let hi = out.get(i + 1).copied().unwrap_or(0) << (32 - bit_shift);
                out[i] = lo | hi;
            }
        }
        mag_trim(&mut out);
        if out.is_empty() || mag_is_zero(&out) {
            return Some(if self.sign < 0 {
                JsBigInt::neg_one()
            } else {
                JsBigInt::zero()
            });
        }

        if self.sign < 0 {
            let mut truncated = false;
            for i in 0..limb_shift {
                if self.mag.get(i).copied().unwrap_or(0) != 0 {
                    truncated = true;
                    break;
                }
            }
            if !truncated && bit_shift > 0 && limb_shift < self.mag.len() {
                if self.mag[limb_shift] & ((1u32 << bit_shift) - 1) != 0 {
                    truncated = true;
                }
            }
            if truncated {
                let bumped = mag_add(&out, &[1]);
                let mut b = bumped;
                mag_trim(&mut b);
                return Some(JsBigInt { sign: -1, mag: b });
            }
        }
        Some(JsBigInt {
            sign: self.sign,
            mag: out,
        })
    }

    pub fn bit_and(&self, other: &JsBigInt) -> JsBigInt {
        if !self.is_negative() && !other.is_negative() {
            let n = self.mag.len().min(other.mag.len());
            let mut out: Vec<u32> = (0..n).map(|i| self.mag[i] & other.mag[i]).collect();
            mag_trim(&mut out);
            let sign = if mag_is_zero(&out) { 0 } else { 1 };
            return JsBigInt {
                sign,
                mag: if out.is_empty() { vec![0] } else { out },
            };
        }

        let max_limbs = self.mag.len().max(other.mag.len()) + 1;
        let a = mag_to_twos(self, max_limbs);
        let b = mag_to_twos(other, max_limbs);
        let r: Vec<u32> = (0..max_limbs).map(|i| a[i] & b[i]).collect();
        twos_to_bigint(r)
    }

    pub fn bit_or(&self, other: &JsBigInt) -> JsBigInt {
        if !self.is_negative() && !other.is_negative() {
            let n = self.mag.len().max(other.mag.len());
            let mut out: Vec<u32> = (0..n)
                .map(|i| {
                    self.mag.get(i).copied().unwrap_or(0) | other.mag.get(i).copied().unwrap_or(0)
                })
                .collect();
            mag_trim(&mut out);
            let sign = if mag_is_zero(&out) { 0 } else { 1 };
            return JsBigInt { sign, mag: out };
        }
        let max_limbs = self.mag.len().max(other.mag.len()) + 1;
        let a = mag_to_twos(self, max_limbs);
        let b = mag_to_twos(other, max_limbs);
        let r: Vec<u32> = (0..max_limbs).map(|i| a[i] | b[i]).collect();
        twos_to_bigint(r)
    }

    pub fn bit_xor(&self, other: &JsBigInt) -> JsBigInt {
        if !self.is_negative() && !other.is_negative() {
            let n = self.mag.len().max(other.mag.len());
            let mut out: Vec<u32> = (0..n)
                .map(|i| {
                    self.mag.get(i).copied().unwrap_or(0) ^ other.mag.get(i).copied().unwrap_or(0)
                })
                .collect();
            mag_trim(&mut out);
            let sign = if mag_is_zero(&out) { 0 } else { 1 };
            return JsBigInt { sign, mag: out };
        }
        let max_limbs = self.mag.len().max(other.mag.len()) + 1;
        let a = mag_to_twos(self, max_limbs);
        let b = mag_to_twos(other, max_limbs);
        let r: Vec<u32> = (0..max_limbs).map(|i| a[i] ^ b[i]).collect();
        twos_to_bigint(r)
    }

    pub fn bit_not(&self) -> JsBigInt {

        self.add(&JsBigInt::one()).neg()
    }

    pub fn pow(&self, exp: &JsBigInt) -> Option<JsBigInt> {
        if exp.is_negative() {
            return None;
        }
        if exp.is_zero() {
            return Some(JsBigInt::one());
        }
        if self.is_zero() {
            return Some(JsBigInt::zero());
        }

        let exp_u = exp.to_f64();
        if !exp_u.is_finite() || exp_u > (1u64 << 20) as f64 {
            return None;
        }
        let mut e = exp_u as u64;
        let mut base = self.clone();
        let mut result = JsBigInt::one();
        while e > 0 {
            if e & 1 == 1 {
                result = result.mul(&base);
            }
            e >>= 1;
            if e > 0 {
                base = base.mul(&base);
            }
        }
        Some(result)
    }
}

fn mag_to_twos(x: &JsBigInt, n_limbs: usize) -> Vec<u32> {
    let mut out = vec![0u32; n_limbs];
    if x.sign >= 0 {
        for (i, &l) in x.mag.iter().enumerate() {
            if i < n_limbs {
                out[i] = l;
            }
        }
    } else {

        for i in 0..n_limbs {
            out[i] = !x.mag.get(i).copied().unwrap_or(0);
        }
        let mut carry: u64 = 1;
        for limb in out.iter_mut() {
            let s = (*limb as u64) + carry;
            *limb = (s & 0xffff_ffff) as u32;
            carry = s >> 32;
            if carry == 0 {
                break;
            }
        }
    }
    out
}

fn twos_to_bigint(r: Vec<u32>) -> JsBigInt {

    let top = *r.last().unwrap_or(&0);
    let is_neg = (top & 0x8000_0000) != 0;
    if !is_neg {
        let mut m = r;
        mag_trim(&mut m);
        let sign = if mag_is_zero(&m) { 0 } else { 1 };
        return JsBigInt { sign, mag: m };
    }

    let n = r.len();
    let mut inv = vec![0u32; n];
    for i in 0..n {
        inv[i] = !r[i];
    }
    let mut carry: u64 = 1;
    for limb in inv.iter_mut() {
        let s = (*limb as u64) + carry;
        *limb = (s & 0xffff_ffff) as u32;
        carry = s >> 32;
        if carry == 0 {
            break;
        }
    }
    mag_trim(&mut inv);
    JsBigInt { sign: -1, mag: inv }
}

fn mag_is_zero(m: &[u32]) -> bool {
    m.iter().all(|&l| l == 0)
}

fn mag_trim(m: &mut Vec<u32>) {
    while m.len() > 1 && *m.last().unwrap() == 0 {
        m.pop();
    }
}

fn mag_cmp(a: &[u32], b: &[u32]) -> Ordering {
    let la = a.iter().rposition(|&l| l != 0).map(|i| i + 1).unwrap_or(0);
    let lb = b.iter().rposition(|&l| l != 0).map(|i| i + 1).unwrap_or(0);
    match la.cmp(&lb) {
        Ordering::Equal => {}
        ord => return ord,
    }
    for i in (0..la).rev() {
        match a[i].cmp(&b[i]) {
            Ordering::Equal => continue,
            ord => return ord,
        }
    }
    Ordering::Equal
}

fn mag_add(a: &[u32], b: &[u32]) -> Vec<u32> {
    let n = a.len().max(b.len()) + 1;
    let mut out = vec![0u32; n];
    let mut carry: u64 = 0;
    for i in 0..n {
        let x = a.get(i).copied().unwrap_or(0) as u64;
        let y = b.get(i).copied().unwrap_or(0) as u64;
        let s = x + y + carry;
        out[i] = (s & 0xffff_ffff) as u32;
        carry = s >> 32;
    }
    out
}

fn mag_sub(a: &[u32], b: &[u32]) -> Vec<u32> {
    let n = a.len();
    let mut out = vec![0u32; n];
    let mut borrow: i64 = 0;
    for i in 0..n {
        let x = a[i] as i64;
        let y = b.get(i).copied().unwrap_or(0) as i64;
        let d = x - y - borrow;
        if d < 0 {
            out[i] = (d + (1i64 << 32)) as u32;
            borrow = 1;
        } else {
            out[i] = d as u32;
            borrow = 0;
        }
    }
    out
}

fn mag_mul(a: &[u32], b: &[u32]) -> Vec<u32> {
    let n = a.len() + b.len();
    let mut acc = vec![0u64; n];
    for i in 0..a.len() {
        for j in 0..b.len() {
            let p = (a[i] as u64) * (b[j] as u64);
            acc[i + j] += p & 0xffff_ffff;
            acc[i + j + 1] += p >> 32;
        }
    }
    let mut out = vec![0u32; n + 1];
    let mut carry: u64 = 0;
    for i in 0..n {
        let s = acc[i] + carry;
        out[i] = (s & 0xffff_ffff) as u32;
        carry = s >> 32;
    }
    out[n] = carry as u32;
    out
}

fn mag_mul_small(m: &mut Vec<u32>, k: u32) {
    let mut carry: u64 = 0;
    for limb in m.iter_mut() {
        let p = (*limb as u64) * (k as u64) + carry;
        *limb = (p & 0xffff_ffff) as u32;
        carry = p >> 32;
    }
    if carry != 0 {
        m.push(carry as u32);
    }
}

fn mag_add_small(m: &mut Vec<u32>, k: u32) {
    let mut carry: u64 = k as u64;
    for limb in m.iter_mut() {
        let s = (*limb as u64) + carry;
        *limb = (s & 0xffff_ffff) as u32;
        carry = s >> 32;
        if carry == 0 {
            break;
        }
    }
    if carry != 0 {
        m.push(carry as u32);
    }
}

fn mag_div_small(m: &mut Vec<u32>, k: u32) -> u32 {
    let mut rem: u64 = 0;
    for i in (0..m.len()).rev() {
        let cur = (rem << 32) | (m[i] as u64);
        m[i] = (cur / (k as u64)) as u32;
        rem = cur % (k as u64);
    }
    mag_trim(m);
    rem as u32
}

fn mag_shl1(m: &[u32]) -> Vec<u32> {
    let mut out = vec![0u32; m.len() + 1];
    let mut carry: u32 = 0;
    for (i, &l) in m.iter().enumerate() {
        out[i] = (l << 1) | carry;
        carry = l >> 31;
    }
    out[m.len()] = carry;
    out
}

fn mag_bit_len(m: &[u32]) -> usize {
    for i in (0..m.len()).rev() {
        if m[i] != 0 {
            return i * 32 + (32 - m[i].leading_zeros() as usize);
        }
    }
    0
}

fn mag_bit(m: &[u32], i: usize) -> bool {
    let limb = i / 32;
    let bit = i % 32;
    m.get(limb).copied().unwrap_or(0) & (1u32 << bit) != 0
}

fn mag_any_bits_below(m: &[u32], bit_limit: usize) -> bool {
    let full_limbs = bit_limit / 32;
    if m.iter().take(full_limbs).any(|&limb| limb != 0) {
        return true;
    }
    let rem = bit_limit % 32;
    rem != 0
        && m.get(full_limbs)
            .copied()
            .is_some_and(|limb| limb & ((1u32 << rem) - 1) != 0)
}

fn mag_shr_to_u64(m: &[u32], shift: usize) -> u64 {
    let limb_shift = shift / 32;
    let bit_shift = shift % 32;
    let mut out = 0u64;
    for i in 0..2 {
        let limb = m.get(limb_shift + i).copied().unwrap_or(0) as u64;
        let part = if bit_shift == 0 {
            limb
        } else {
            (limb >> bit_shift)
                | ((m.get(limb_shift + i + 1).copied().unwrap_or(0) as u64) << (32 - bit_shift))
        };
        out |= (part & 0xffff_ffff) << (i * 32);
    }
    out
}

fn mag_divmod(a: &[u32], b: &[u32]) -> (Vec<u32>, Vec<u32>) {
    let bits = mag_bit_len(a);
    let mut q = vec![0u32; (bits + 31) / 32 + 1];
    let mut r = vec![0u32];
    for i in (0..bits).rev() {
        r = mag_shl1(&r);
        if mag_bit(a, i) {
            if r.is_empty() {
                r.push(0);
            }
            r[0] |= 1;
        }
        if mag_cmp(&r, b) != Ordering::Less {
            r = mag_sub(&r, b);
            q[i / 32] |= 1u32 << (i % 32);
        }
    }
    mag_trim(&mut q);
    mag_trim(&mut r);
    (q, r)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_format_roundtrip() {
        for s in &[
            "0",
            "1",
            "-1",
            "42",
            "-42",
            "1000000000",
            "18446744073709551615",
            "-18446744073709551616",
            "123456789012345678901234567890",
        ] {
            let b = JsBigInt::from_decimal(s).unwrap();
            assert_eq!(b.to_decimal(), *s);
        }
    }

    #[test]
    fn add_sub_signs() {
        let a = JsBigInt::from_decimal("100").unwrap();
        let b = JsBigInt::from_decimal("-30").unwrap();
        assert_eq!(a.add(&b).to_decimal(), "70");
        assert_eq!(a.sub(&b).to_decimal(), "130");
        assert_eq!(b.sub(&a).to_decimal(), "-130");
        assert_eq!(a.add(&a.neg()).to_decimal(), "0");
    }

    #[test]
    fn mul_signs() {
        let a = JsBigInt::from_decimal("12345678901234567890").unwrap();
        let b = JsBigInt::from_decimal("-2").unwrap();
        assert_eq!(a.mul(&b).to_decimal(), "-24691357802469135780");
    }

    #[test]
    fn divmod_trunc_toward_zero() {
        let a = JsBigInt::from_decimal("-7").unwrap();
        let b = JsBigInt::from_decimal("2").unwrap();
        let (q, r) = a.divmod(&b).unwrap();

        assert_eq!(q.to_decimal(), "-3");
        assert_eq!(r.to_decimal(), "-1");
    }

    #[test]
    fn pow_small() {
        let a = JsBigInt::from_decimal("2").unwrap();
        let e = JsBigInt::from_decimal("64").unwrap();
        assert_eq!(a.pow(&e).unwrap().to_decimal(), "18446744073709551616");
    }
}
