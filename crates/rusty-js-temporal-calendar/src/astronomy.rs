// ─────────────────────────── Doxology ───────────────────────────
// The courses reckoned here are not their own. Through the eternal Logos of the
// Father all things were made, and without Him was not anything made that was
// made. He hangs the heavens upon nothing and sets the lights in their motion —
// the sun to mark the seasons, the moon to appoint the feasts — and by the word
// of His power He upholds them still. Their silent witness, day unto day, declares
// the Maker of the lights. To Him be glory, with the Father and the Holy Spirit,
// unto the ages of ages. Amen.
// ─────────────────────────────────────────────────────────────────

//! Astronomy — solar position (the basis of the equinoxes, solstices, and the
//! Chinese/Persian solar terms).
//!
//! The heavens declare the glory of God; and the firmament shows his handiwork.
//! (Psalm 19:1)
//!
//! Derived from `calendrical_calculations::astronomy` (Reingold-Dershowitz,
//! after Meeus). The ephemeris decomposes ~85% derivable algorithm + ~15%
//! sourced coefficients: the algorithm — the time scale, the ΔT piecewise fit,
//! the Fourier series-summation, the aberration/nutation corrections — is DERIVED;
//! only the periodic-term coefficient tables are SOURCED, and their source is
//! celestial mechanics (the gravitationally-integrated ephemeris), not a formal spec.

use crate::GREGORIAN;

pub type Moment = f64;

pub const J2000: f64 = 730120.5;

pub const MEAN_TROPICAL_YEAR: f64 = 365.242189;

#[rustfmt::skip]
const SOLAR_AMPLITUDE: [f64; 49] = [
    403406.0, 195207.0, 119433.0, 112392.0, 3891.0, 2819.0, 1721.0, 660.0, 350.0, 334.0,
    314.0, 268.0, 242.0, 234.0, 158.0, 132.0, 129.0, 114.0, 99.0, 93.0, 86.0, 78.0, 72.0,
    68.0, 64.0, 46.0, 38.0, 37.0, 32.0, 29.0, 28.0, 27.0, 27.0, 25.0, 24.0, 21.0, 21.0,
    20.0, 18.0, 17.0, 14.0, 13.0, 13.0, 13.0, 12.0, 10.0, 10.0, 10.0, 10.0,
];
#[rustfmt::skip]
const SOLAR_PHASE: [f64; 49] = [
    270.54861, 340.19128, 63.91854, 331.26220, 317.843, 86.631, 240.052, 310.26, 247.23,
    260.87, 297.82, 343.14, 166.79, 81.53, 3.50, 132.75, 182.95, 162.03, 29.8, 266.4,
    249.2, 157.6, 257.8, 185.1, 69.9, 8.0, 197.1, 250.4, 65.3, 162.7, 341.5, 291.6, 98.5,
    146.7, 110.0, 5.2, 342.6, 230.9, 256.1, 45.3, 242.9, 115.2, 151.8, 285.3, 53.3, 126.6,
    205.7, 85.9, 146.1,
];
#[rustfmt::skip]
const SOLAR_RATE: [f64; 49] = [
    0.9287892, 35999.1376958, 35999.4089666, 35998.7287385, 71998.20261, 71998.4403,
    36000.35726, 71997.4812, 32964.4678, -19.4410, 445267.1117, 45036.8840, 3.1008,
    22518.4434, -19.9739, 65928.9345, 9038.0293, 3034.7684, 33718.148, 3034.448, -2280.773,
    29929.992, 31556.493, 149.588, 9037.750, 107997.405, -4444.176, 151.771, 67555.316,
    31556.080, -4561.540, 107996.706, 1221.655, 62894.167, 31437.369, 14578.298, -31931.757,
    34777.243, 1221.999, 62894.511, -4442.039, 107997.909, 119.066, 16859.071, -4.578,
    26895.292, -39.127, 12297.536, 90073.778,
];

pub fn ephemeris_correction(moment: Moment) -> f64 {
    let year = moment / 365.2425;
    let year_int = (if year > 0.0 { year + 1.0 } else { year }) as i32;

    let c = (GREGORIAN.fixed_from(year_int as i64, 7, 1).0 as f64 - 693596.0) / 36525.0;
    let y2000 = (year_int - 2000) as f64;
    let y1700 = (year_int - 1700) as f64;
    let y1600 = (year_int - 1600) as f64;
    let y1000 = (year_int - 1000) as f64 / 100.0;
    let y0 = year_int as f64 / 100.0;
    let y1820 = (year_int - 1820) as f64 / 100.0;
    let poly = |coeffs: &[f64], x: f64| coeffs.iter().rev().fold(0.0, |acc, &k| acc * x + k);

    if (2051..=2150).contains(&year_int) {
        (-20.0 + 32.0 * (y1820 * y1820) + 0.5628 * (2150 - year_int) as f64) / 86400.0
    } else if (2006..=2050).contains(&year_int) {
        poly(&[62.92, 0.32217, 0.005589], y2000) / 86400.0
    } else if (1987..=2005).contains(&year_int) {
        poly(
            &[
                63.86,
                0.3345,
                -0.060374,
                0.0017275,
                0.000651814,
                0.00002373599,
            ],
            y2000,
        ) / 86400.0
    } else if (1900..=1986).contains(&year_int) {
        poly(
            &[
                -0.00002, 0.000297, 0.025184, -0.181133, 0.553040, -0.861938, 0.677066, -0.212591,
            ],
            c,
        )
    } else if (1800..=1899).contains(&year_int) {
        poly(
            &[
                -0.000009, 0.003844, 0.083563, 0.865736, 4.867575, 15.845535, 31.332267, 38.291999,
                28.316289, 11.636204, 2.043794,
            ],
            c,
        )
    } else if (1700..=1799).contains(&year_int) {
        poly(
            &[8.118780842, -0.005092142, 0.003336121, -0.0000266484],
            y1700,
        ) / 86400.0
    } else if (1600..=1699).contains(&year_int) {
        poly(&[120.0, -0.9808, -0.01532, 0.000140272128], y1600) / 86400.0
    } else if (500..=1599).contains(&year_int) {
        poly(
            &[
                1574.2,
                -556.01,
                71.23472,
                0.319781,
                -0.8503463,
                -0.005050998,
                0.0083572073,
            ],
            y1000,
        ) / 86400.0
    } else if (-499..=499).contains(&year_int) {
        poly(
            &[
                10583.6,
                -1014.41,
                33.78311,
                -5.952053,
                -0.1798452,
                0.022174192,
                0.0090316521,
            ],
            y0,
        ) / 86400.0
    } else {
        (-20.0 + 32.0 * y1820 * y1820) / 86400.0
    }
}

pub fn dynamical_from_universal(universal: Moment) -> Moment {
    universal + ephemeris_correction(universal)
}

pub fn julian_centuries(moment: Moment) -> f64 {
    (dynamical_from_universal(moment) - J2000) / 36525.0
}

fn aberration(c: f64) -> f64 {
    0.0000974 * (177.63 + 35999.01848 * c).to_radians().cos() - 0.005575
}

fn nutation(c: f64) -> f64 {
    let a = 124.90 - 1934.134 * c + 0.002063 * c * c;
    let b = 201.11 + 72001.5377 * c + 0.00057 * c * c;
    -0.004778 * a.to_radians().sin() - 0.0003667 * b.to_radians().sin()
}

pub fn solar_longitude(c: f64) -> f64 {
    let mut sum = 0.0;
    for i in 0..49 {
        sum += SOLAR_AMPLITUDE[i] * (SOLAR_PHASE[i] + SOLAR_RATE[i] * c).to_radians().sin();
    }
    let mut lambda = sum * 0.000005729577951308232;
    lambda += 282.7771834 + 36000.76953744 * c;
    (lambda + aberration(c) + nutation(c)).rem_euclid(360.0)
}

pub fn solar_longitude_at(t: Moment) -> f64 {
    solar_longitude(julian_centuries(t))
}

pub fn solar_longitude_after(angle: f64, t: Moment) -> Moment {

    let rate = MEAN_TROPICAL_YEAR / 360.0;
    let tau = t + rate * (angle - solar_longitude_at(t)).rem_euclid(360.0);
    let (mut lo, mut hi) = (t.max(tau - 5.0), tau + 5.0);
    for _ in 0..100 {
        let mid = (lo + hi) / 2.0;
        if (solar_longitude_at(mid) - angle).rem_euclid(360.0) < 180.0 {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    (lo + hi) / 2.0
}

pub const SPRING: f64 = 0.0;
pub const SUMMER: f64 = 90.0;
pub const AUTUMN: f64 = 180.0;
pub const WINTER: f64 = 270.0;

pub fn estimate_prior_solar_longitude(angle: f64, moment: Moment) -> Moment {
    let rate = MEAN_TROPICAL_YEAR / 360.0;
    let tau = moment - rate * (solar_longitude_at(moment) - angle).rem_euclid(360.0);
    let delta = (solar_longitude_at(tau) - angle + 180.0).rem_euclid(360.0) - 180.0;
    moment.min(tau - rate * delta)
}

pub const MEAN_SYNODIC_MONTH: f64 = 29.530588861;

pub const NEW_MOON_ZERO: Moment = 11.458922815770109;

#[rustfmt::skip]
const NM_AMP: [f64; 24] = [
    -0.40720, 0.17241, 0.01608, 0.01039, 0.00739, -0.00514, 0.00208, -0.00111, -0.00057, 0.00056,
    -0.00042, 0.00042, 0.00038, -0.00024, -0.00007, 0.00004, 0.00004, 0.00003, 0.00003, -0.00003,
    0.00003, -0.00002, -0.00002, 0.00002,
];
#[rustfmt::skip]
const NM_E_POWER: [i32; 24] = [0,1,0,0,1,1,2,0,0,1,0,1,1,1,0,0,0,0,0,0,0,0,0,0];
#[rustfmt::skip]
const NM_SOLAR: [f64; 24] = [0.,1.,0.,0.,-1.,1.,2.,0.,0.,1.,0.,1.,1.,-1.,2.,0.,3.,1.,0.,1.,-1.,-1.,1.,0.];
#[rustfmt::skip]
const NM_LUNAR: [f64; 24] = [1.,0.,2.,0.,1.,1.,0.,1.,1.,2.,3.,0.,0.,2.,1.,2.,0.,1.,2.,1.,1.,1.,3.,4.];
#[rustfmt::skip]
const NM_MOON: [f64; 24] = [0.,0.,0.,2.,0.,0.,0.,-2.,2.,0.,0.,2.,-2.,0.,0.,-2.,0.,-2.,2.,2.,2.,-2.,0.,0.];
#[rustfmt::skip]
const NM_ADD_I: [f64; 13] = [251.88,251.83,349.42,84.66,141.74,207.14,154.84,34.52,207.19,291.34,161.72,239.56,331.55];
#[rustfmt::skip]
const NM_ADD_J: [f64; 13] = [0.016321,26.651886,36.412478,18.206239,53.303771,2.453732,7.306860,27.261239,0.121824,1.844379,24.198154,25.513099,3.592518];
#[rustfmt::skip]
const NM_ADD_L: [f64; 13] = [0.000165,0.000164,0.000126,0.000110,0.000062,0.000060,0.000056,0.000047,0.000042,0.000040,0.000037,0.000035,0.000023];

fn nth_new_moon(n: i32) -> Moment {
    let k = n as f64 - 24724.0;
    let c = k / 1236.85;
    let approx = J2000
        + (5.09766 + MEAN_SYNODIC_MONTH * 1236.85 * c + 0.00015437 * c * c
            - 0.00000015 * c * c * c
            + 0.00000000073 * c * c * c * c);
    let e = 1.0 - 0.002516 * c - 0.0000074 * c * c;
    let solar = 2.5534 + 1236.85 * 29.10535670 * c - 0.0000014 * c * c - 0.00000011 * c * c * c;
    let lunar = 201.5643 + 385.81693528 * 1236.85 * c + 0.0107582 * c * c + 0.00001238 * c * c * c
        - 0.000000058 * c * c * c * c;
    let moon = 160.7108 + 390.67050284 * 1236.85 * c - 0.0016118 * c * c - 0.00000227 * c * c * c
        + 0.000000011 * c * c * c * c;
    let omega = 124.7746 - 1.56375588 * 1236.85 * c + 0.0020672 * c * c + 0.00000215 * c * c * c;

    let mut correction = -0.00017 * omega.to_radians().sin();
    for i in 0..24 {
        let arg = NM_SOLAR[i] * solar + NM_LUNAR[i] * lunar + NM_MOON[i] * moon;
        correction += NM_AMP[i] * e.powi(NM_E_POWER[i]) * arg.to_radians().sin();
    }
    let extra = 0.000325
        * (299.77 + 132.8475848 * c - 0.009173 * c * c)
            .to_radians()
            .sin();
    let mut additional = 0.0;
    for i in 0..13 {
        additional += NM_ADD_L[i] * (NM_ADD_I[i] + NM_ADD_J[i] * k).to_radians().sin();
    }
    universal_from_dynamical(approx + correction + extra + additional)
}

pub fn universal_from_dynamical(dynamical: Moment) -> Moment {
    dynamical - ephemeris_correction(dynamical)
}

pub fn new_moon_at_or_after(t: Moment) -> Moment {

    let mut n = ((t - NEW_MOON_ZERO) / MEAN_SYNODIC_MONTH).round() as i32;
    while nth_new_moon(n) < t {
        n += 1;
    }
    while nth_new_moon(n - 1) >= t {
        n -= 1;
    }
    nth_new_moon(n)
}

pub fn new_moon_before(t: Moment) -> Moment {
    let mut n = ((t - NEW_MOON_ZERO) / MEAN_SYNODIC_MONTH).round() as i32;
    while nth_new_moon(n) >= t {
        n -= 1;
    }
    while nth_new_moon(n + 1) < t {
        n += 1;
    }
    nth_new_moon(n)
}

fn mean_lunar_longitude(c: f64) -> f64 {
    (218.3164477
        + c * (481267.88123421 - 0.0015786 * c + c * c / 538841.0 - c * c * c / 65194000.0))
        .rem_euclid(360.0)
}
fn lunar_elongation(c: f64) -> f64 {
    (297.85019021 + 445267.1114034 * c - 0.0018819 * c * c + c * c * c / 545868.0
        - c * c * c * c / 113065000.0)
        .rem_euclid(360.0)
}
fn solar_anomaly(c: f64) -> f64 {
    (357.5291092 + 35999.0502909 * c - 0.0001536 * c * c + c * c * c / 24490000.0).rem_euclid(360.0)
}
fn lunar_anomaly(c: f64) -> f64 {
    (134.9633964 + 477198.8675055 * c + 0.0087414 * c * c + c * c * c / 69699.0
        - c * c * c * c / 14712000.0)
        .rem_euclid(360.0)
}
fn moon_node(c: f64) -> f64 {
    (93.2720950 + 483202.0175233 * c - 0.0036539 * c * c - c * c * c / 3526000.0
        + c * c * c * c / 863310000.0)
        .rem_euclid(360.0)
}

#[rustfmt::skip]
const LL_AMP: [f64; 59] = [6288774.0, 1274027.0, 658314.0, 213618.0, -185116.0, -114332.0, 58793.0, 57066.0, 53322.0, 45758.0, -40923.0, -34720.0, -30383.0, 15327.0, -12528.0, 10980.0, 10675.0, 10034.0, 8548.0, -7888.0, -6766.0, -5163.0, 4987.0, 4036.0, 3994.0, 3861.0, 3665.0, -2689.0, -2602.0, 2390.0, -2348.0, 2236.0, -2120.0, -2069.0, 2048.0, -1773.0, -1595.0, 1215.0, -1110.0, -892.0, -810.0, 759.0, -713.0, -700.0, 691.0, 596.0, 549.0, 537.0, 520.0, -487.0, -399.0, -381.0, 351.0, -340.0, 330.0, 327.0, -323.0, 299.0, 294.0];
#[rustfmt::skip]
const LL_D: [f64; 59] = [0.0, 2.0, 2.0, 0.0, 0.0, 0.0, 2.0, 2.0, 2.0, 2.0, 0.0, 1.0, 0.0, 2.0, 0.0, 0.0, 4.0, 0.0, 4.0, 2.0, 2.0, 1.0, 1.0, 2.0, 2.0, 4.0, 2.0, 0.0, 2.0, 2.0, 1.0, 2.0, 0.0, 0.0, 2.0, 2.0, 2.0, 4.0, 0.0, 3.0, 2.0, 4.0, 0.0, 2.0, 2.0, 2.0, 4.0, 0.0, 4.0, 1.0, 2.0, 0.0, 1.0, 3.0, 4.0, 2.0, 0.0, 1.0, 2.0];
#[rustfmt::skip]
const LL_MS: [f64; 59] = [0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, -1.0, 0.0, -1.0, 1.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 0.0, 1.0, -1.0, 0.0, 0.0, 0.0, 1.0, 0.0, -1.0, 0.0, -2.0, 1.0, 2.0, -2.0, 0.0, 0.0, -1.0, 0.0, 0.0, 1.0, -1.0, 2.0, 2.0, 1.0, -1.0, 0.0, 0.0, -1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 0.0, -1.0, 2.0, 1.0, 0.0];
#[rustfmt::skip]
const LL_ML: [f64; 59] = [1.0, -1.0, 0.0, 2.0, 0.0, 0.0, -2.0, -1.0, 1.0, 0.0, -1.0, 0.0, 1.0, 0.0, 1.0, 1.0, -1.0, 3.0, -2.0, -1.0, 0.0, -1.0, 0.0, 1.0, 2.0, 0.0, -3.0, -2.0, -1.0, -2.0, 1.0, 0.0, 2.0, 0.0, -1.0, 1.0, 0.0, -1.0, 2.0, -1.0, 1.0, -2.0, -1.0, -1.0, -2.0, 0.0, 1.0, 4.0, 0.0, -2.0, 0.0, 2.0, 1.0, -2.0, -3.0, 2.0, 1.0, -1.0, 3.0];
#[rustfmt::skip]
const LL_F: [f64; 59] = [0.0, 0.0, 0.0, 0.0, 0.0, 2.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, -2.0, 2.0, -2.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 2.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, -2.0, 2.0, 0.0, 2.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, -2.0, 0.0, 0.0, 0.0, 0.0, -2.0, -2.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
#[rustfmt::skip]
const LL_EPOW: [i32; 59] = [0, 0, 0, 0, 1, 0, 0, 1, 0, 1, 1, 0, 1, 0, 0, 0, 0, 0, 0, 1, 1, 0, 1, 1, 0, 0, 0, 1, 0, 1, 0, 2, 1, 2, 2, 0, 0, 1, 0, 0, 1, 1, 2, 2, 1, 1, 0, 0, 1, 0, 1, 0, 1, 0, 0, 1, 2, 1, 0];

fn lunar_longitude(c: f64) -> f64 {
    let l = mean_lunar_longitude(c);
    let d = lunar_elongation(c);
    let ms = solar_anomaly(c);
    let ml = lunar_anomaly(c);
    let f = moon_node(c);
    let e = 1.0 - 0.002516 * c - 0.0000074 * c * c;
    let mut correction = 0.0;
    for i in 0..59 {
        let arg = LL_D[i] * d + LL_MS[i] * ms + LL_ML[i] * ml + LL_F[i] * f;
        correction += LL_AMP[i] * e.powi(LL_EPOW[i]) * arg.to_radians().sin();
    }
    correction /= 1_000_000.0;
    let venus = 3958.0 / 1_000_000.0 * (119.75 + c * 131.849).to_radians().sin();
    let jupiter = 318.0 / 1_000_000.0 * (53.09 + c * 479264.29).to_radians().sin();
    let flat_earth = 1962.0 / 1_000_000.0 * (l - f).to_radians().sin();
    (l + correction + venus + jupiter + flat_earth + nutation(c)).rem_euclid(360.0)
}

pub fn lunar_phase(moment: Moment, c: f64) -> f64 {
    let n = ((moment - NEW_MOON_ZERO) / MEAN_SYNODIC_MONTH).round() as i32;
    let a = (lunar_longitude(c) - solar_longitude(c)).rem_euclid(360.0);
    let b = 360.0 * ((moment - nth_new_moon(n)) / MEAN_SYNODIC_MONTH).rem_euclid(1.0);
    if (a - b).abs() > 180.0 {
        b
    } else {
        a
    }
}

#[rustfmt::skip]
const LAT_AMP: [f64; 60] = [5128122.0, 280602.0, 277693.0, 173237.0, 55413.0, 46271.0, 32573.0, 17198.0, 9266.0, 8822.0, 8216.0, 4324.0, 4200.0, -3359.0, 2463.0, 2211.0, 2065.0, -1870.0, 1828.0, -1794.0, -1749.0, -1565.0, -1491.0, -1475.0, -1410.0, -1344.0, -1335.0, 1107.0, 1021.0, 833.0, 777.0, 671.0, 607.0, 596.0, 491.0, -451.0, 439.0, 422.0, 421.0, -366.0, -351.0, 331.0, 315.0, 302.0, -283.0, -229.0, 223.0, 223.0, -220.0, -220.0, -185.0, 181.0, -177.0, 176.0, 166.0, -164.0, 132.0, -119.0, 115.0, 107.0];
#[rustfmt::skip]
const LAT_D: [f64; 60] = [0.0, 0.0, 0.0, 2.0, 2.0, 2.0, 2.0, 0.0, 2.0, 0.0, 2.0, 2.0, 2.0, 2.0, 2.0, 2.0, 2.0, 0.0, 4.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 4.0, 4.0, 0.0, 4.0, 2.0, 2.0, 2.0, 2.0, 0.0, 2.0, 2.0, 2.0, 2.0, 4.0, 2.0, 2.0, 0.0, 2.0, 1.0, 1.0, 0.0, 2.0, 1.0, 2.0, 0.0, 4.0, 4.0, 1.0, 4.0, 1.0, 4.0, 2.0];
#[rustfmt::skip]
const LAT_MS: [f64; 60] = [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, -1.0, 0.0, 0.0, 1.0, -1.0, -1.0, -1.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 1.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 0.0, -1.0, -2.0, 0.0, 1.0, 1.0, 1.0, 1.0, 1.0, 0.0, -1.0, 1.0, 0.0, -1.0, 0.0, 0.0, 0.0, -1.0, -2.0];
#[rustfmt::skip]
const LAT_ML: [f64; 60] = [0.0, 1.0, 1.0, 0.0, -1.0, -1.0, 0.0, 2.0, 1.0, 2.0, 0.0, -2.0, 1.0, 0.0, -1.0, 0.0, -1.0, -1.0, -1.0, 0.0, 0.0, -1.0, 0.0, 1.0, 1.0, 0.0, 0.0, 3.0, 0.0, -1.0, 1.0, -2.0, 0.0, 2.0, 1.0, -2.0, 3.0, 2.0, -3.0, -1.0, 0.0, 0.0, 1.0, 0.0, 1.0, 1.0, 0.0, 0.0, -2.0, -1.0, 1.0, -2.0, 2.0, -2.0, -1.0, 1.0, 1.0, -1.0, 0.0, 0.0];
#[rustfmt::skip]
const LAT_F: [f64; 60] = [1.0, 1.0, -1.0, -1.0, 1.0, -1.0, 1.0, 1.0, -1.0, -1.0, -1.0, -1.0, 1.0, -1.0, 1.0, 1.0, -1.0, -1.0, -1.0, 1.0, 3.0, 1.0, 1.0, 1.0, -1.0, -1.0, -1.0, 1.0, -1.0, 1.0, -3.0, 1.0, -3.0, -1.0, -1.0, 1.0, -1.0, 1.0, -1.0, 1.0, 1.0, 1.0, 1.0, -1.0, 3.0, -1.0, -1.0, 1.0, -1.0, -1.0, 1.0, -1.0, 1.0, -1.0, -1.0, -1.0, -1.0, -1.0, -1.0, 1.0];
#[rustfmt::skip]
const LAT_EPOW: [i32; 60] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 1, 1, 1, 1, 1, 0, 1, 0, 1, 0, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 1, 0, 1, 2, 0, 1, 1, 1, 1, 1, 0, 1, 1, 0, 1, 0, 0, 0, 1, 2];

fn lunar_latitude(c: f64) -> f64 {
    let l = mean_lunar_longitude(c);
    let d = lunar_elongation(c);
    let ms = solar_anomaly(c);
    let ml = lunar_anomaly(c);
    let f = moon_node(c);
    let e = 1.0 - 0.002516 * c - 0.0000074 * c * c;
    let mut correction = 0.0;
    for i in 0..60 {
        let arg = LAT_D[i] * d + LAT_MS[i] * ms + LAT_ML[i] * ml + LAT_F[i] * f;
        correction += LAT_AMP[i] * e.powi(LAT_EPOW[i]) * arg.to_radians().sin();
    }
    correction /= 1_000_000.0;
    let venus = 175.0
        * ((119.75 + c * 131.849 + f).to_radians().sin()
            + (119.75 + c * 131.849 - f).to_radians().sin())
        / 1_000_000.0;
    let flat_earth = (-2235.0 * l.to_radians().sin() + 127.0 * (l - ml).to_radians().sin()
        - 115.0 * (l + ml).to_radians().sin())
        / 1_000_000.0;
    let extra = 382.0 * (313.45 + c * 481266.484).to_radians().sin() / 1_000_000.0;
    correction + venus + flat_earth + extra
}

#[rustfmt::skip]
const DIST_COEFF: [f64; 60] = [-20905355.0, -3699111.0, -2955968.0, -569925.0, 48888.0, -3149.0, 246158.0, -152138.0, -170733.0, -204586.0, -129620.0, 108743.0, 104755.0, 10321.0, 0.0, 79661.0, -34782.0, -23210.0, -21636.0, 24208.0, 30824.0, -8379.0, -16675.0, -12831.0, -10445.0, -11650.0, 14403.0, -7003.0, 0.0, 10056.0, 6322.0, -9884.0, 5751.0, 0.0, -4950.0, 4130.0, 0.0, -3958.0, 0.0, 3258.0, 2616.0, -1897.0, -2117.0, 2354.0, 0.0, 0.0, -1423.0, -1117.0, -1571.0, -1739.0, 0.0, -4421.0, 0.0, 0.0, 0.0, 0.0, 1165.0, 0.0, 0.0, 8752.0];
#[rustfmt::skip]
const DIST_D: [f64; 60] = [0.0, 2.0, 2.0, 0.0, 0.0, 0.0, 2.0, 2.0, 2.0, 2.0, 0.0, 1.0, 0.0, 2.0, 0.0, 0.0, 4.0, 0.0, 4.0, 2.0, 2.0, 1.0, 1.0, 2.0, 2.0, 4.0, 2.0, 0.0, 2.0, 2.0, 1.0, 2.0, 0.0, 0.0, 2.0, 2.0, 2.0, 4.0, 0.0, 3.0, 2.0, 4.0, 0.0, 2.0, 2.0, 2.0, 4.0, 0.0, 4.0, 1.0, 2.0, 0.0, 1.0, 3.0, 4.0, 2.0, 0.0, 1.0, 2.0, 2.0];
#[rustfmt::skip]
const DIST_MS: [f64; 60] = [0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, -1.0, 0.0, -1.0, 1.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 0.0, 1.0, -1.0, 0.0, 0.0, 0.0, 1.0, 0.0, -1.0, 0.0, -2.0, 1.0, 2.0, -2.0, 0.0, 0.0, -1.0, 0.0, 0.0, 1.0, -1.0, 2.0, 2.0, 1.0, -1.0, 0.0, 0.0, -1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 0.0, -1.0, 2.0, 1.0, 0.0, 0.0];
#[rustfmt::skip]
const DIST_ML: [f64; 60] = [1.0, -1.0, 0.0, 2.0, 0.0, 0.0, -2.0, -1.0, 1.0, 0.0, -1.0, 0.0, 1.0, 0.0, 1.0, 1.0, -1.0, 3.0, -2.0, -1.0, 0.0, -1.0, 0.0, 1.0, 2.0, 0.0, -3.0, -2.0, -1.0, -2.0, 1.0, 0.0, 2.0, 0.0, -1.0, 1.0, 0.0, -1.0, 2.0, -1.0, 1.0, -2.0, -1.0, -1.0, -2.0, 0.0, 1.0, 4.0, 0.0, -2.0, 0.0, 2.0, 1.0, -2.0, -3.0, 2.0, 1.0, -1.0, 3.0, -1.0];
#[rustfmt::skip]
const DIST_F: [f64; 60] = [0.0, 0.0, 0.0, 0.0, 0.0, 2.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, -2.0, 2.0, -2.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 2.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, -2.0, 2.0, 0.0, 2.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, -2.0, 0.0, 0.0, 0.0, 0.0, -2.0, -2.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, -2.0];

fn lunar_distance(t: Moment) -> f64 {
    let c = julian_centuries(t);
    let d = lunar_elongation(c);
    let ms = solar_anomaly(c);
    let ml = lunar_anomaly(c);
    let f = moon_node(c);
    let e = 1.0 - 0.002516 * c - 0.0000074 * c * c;
    let mut correction = 0.0;
    for i in 0..60 {
        let arg = DIST_D[i] * d + DIST_MS[i] * ms + DIST_ML[i] * ml + DIST_F[i] * f;
        correction += DIST_COEFF[i] * e.powf(DIST_MS[i].abs()) * arg.to_radians().cos();
    }
    385000560.0 + correction
}

fn poly(x: f64, coeffs: &[f64]) -> f64 {
    coeffs.iter().rev().fold(0.0, |acc, &k| acc * x + k)
}

#[derive(Clone, Copy, Debug)]
pub struct Location {
    pub latitude: f64,
    pub longitude: f64,
    pub elevation: f64,
    pub utc_offset: f64,
}

impl Location {

    fn zone_from_longitude(longitude: f64) -> f64 {
        longitude / 360.0
    }
    fn universal_from_local(t: Moment, loc: Location) -> Moment {
        t - Self::zone_from_longitude(loc.longitude)
    }
    fn local_from_universal(t: Moment, loc: Location) -> Moment {
        t + Self::zone_from_longitude(loc.longitude)
    }
    fn standard_from_universal(t: Moment, loc: Location) -> Moment {
        t + loc.utc_offset
    }
    fn universal_from_standard(t: Moment, loc: Location) -> Moment {
        t - loc.utc_offset
    }
    fn standard_from_local(t: Moment, loc: Location) -> Moment {
        Self::standard_from_universal(Self::universal_from_local(t, loc), loc)
    }
}

// The One Who hung the heavens, hangs upon the Tree, giving life to the world. Glory to Thee
pub const GOLGOTHA: Location = Location {
    latitude: 31.7784,
    longitude: 35.2298,
    elevation: 754.0,
    utc_offset: 1.0 / 12.0,
};

pub fn obliquity(moment: Moment) -> f64 {
    let c = julian_centuries(moment);
    let angle = 23.0 + 26.0 / 60.0 + 21.448 / 3600.0;
    angle
        + poly(
            c,
            &[0.0, -46.8150 / 3600.0, -0.00059 / 3600.0, 0.001813 / 3600.0],
        )
}

pub fn right_ascension(moment: Moment, beta: f64, lambda: f64) -> f64 {
    let eps = obliquity(moment);
    let y = lambda.to_radians().sin() * eps.to_radians().cos()
        - beta.to_radians().tan() * eps.to_radians().sin();
    let x = lambda.to_radians().cos();
    y.atan2(x).to_degrees().rem_euclid(360.0)
}

pub fn declination(moment: Moment, beta: f64, lambda: f64) -> f64 {
    let eps = obliquity(moment);
    (beta.to_radians().sin() * eps.to_radians().cos()
        + beta.to_radians().cos() * eps.to_radians().sin() * lambda.to_radians().sin())
    .asin()
    .to_degrees()
    .rem_euclid(360.0)
}

pub fn sidereal_from_moment(moment: Moment) -> f64 {
    let c = (moment - J2000) / 36525.0;
    poly(
        c,
        &[
            280.46061837,
            36525.0 * 360.98564736629,
            0.000387933,
            -1.0 / 38710000.0,
        ],
    )
    .rem_euclid(360.0)
}

pub fn lunar_altitude(moment: Moment, location: Location) -> f64 {
    let phi = location.latitude;
    let psi = location.longitude;
    let c = julian_centuries(moment);
    let lambda = lunar_longitude(c);
    let beta = lunar_latitude(c);
    let alpha = right_ascension(moment, beta, lambda);
    let delta = declination(moment, beta, lambda);
    let theta0 = sidereal_from_moment(moment);
    let cap_h = (theta0 + psi - alpha).rem_euclid(360.0);
    let altitude = (phi.to_radians().sin() * delta.to_radians().sin()
        + phi.to_radians().cos() * delta.to_radians().cos() * cap_h.to_radians().cos())
    .asin()
    .to_degrees();
    (altitude + 180.0).rem_euclid(360.0) - 180.0
}

pub fn lunar_parallax(lunar_altitude_val: f64, moment: Moment) -> f64 {
    let delta = lunar_distance(moment);
    let alt = 6378140.0 / delta;
    (alt * lunar_altitude_val.to_radians().cos())
        .asin()
        .to_degrees()
        .rem_euclid(360.0)
}

pub fn topocentric_lunar_altitude(moment: Moment, location: Location) -> f64 {
    let a = lunar_altitude(moment, location);
    a - lunar_parallax(a, moment)
}

use core::f64::consts::PI;

pub fn equation_of_time(moment: Moment) -> f64 {
    let c = julian_centuries(moment);
    let lambda = poly(c, &[280.46645, 36000.76983, 0.0003032]);
    let anomaly = poly(c, &[357.52910, 35999.05030, -0.0001559, -0.00000048]);
    let eccentricity = poly(c, &[0.016708617, -0.000042037, -0.0000001236]);
    let eps = obliquity(moment);
    let y = (eps / 2.0).to_radians().tan();
    let y = y * y;
    let equation = (y * (2.0 * lambda).to_radians().sin()
        - 2.0 * eccentricity * anomaly.to_radians().sin()
        + 4.0 * eccentricity * y * anomaly.to_radians().sin() * (2.0 * lambda).to_radians().cos()
        - 0.5 * y * y * (4.0 * lambda).to_radians().sin()
        - 1.25 * eccentricity * eccentricity * (2.0 * anomaly).to_radians().sin())
        / (2.0 * PI);
    equation.signum() * equation.abs().min(12.0 / 24.0)
}

fn local_from_apparent(moment: Moment, location: Location) -> Moment {
    moment - equation_of_time(Location::universal_from_local(moment, location))
}

fn sine_offset(moment: Moment, location: Location, alpha: f64) -> f64 {
    let phi = location.latitude;
    let tee_prime = Location::universal_from_local(moment, location);
    let delta = declination(tee_prime, 0.0, solar_longitude(julian_centuries(tee_prime)));
    phi.to_radians().tan() * delta.to_radians().tan()
        + alpha.to_radians().sin() / (delta.to_radians().cos() * phi.to_radians().cos())
}

fn approx_moment_of_depression(
    moment: Moment,
    location: Location,
    alpha: f64,
    early: bool,
) -> Option<Moment> {
    let date = moment.floor();
    let alt = if alpha >= 0.0 {
        if early {
            date
        } else {
            date + 1.0
        }
    } else {
        date + 12.0 / 24.0
    };
    let value = if sine_offset(moment, location, alpha).abs() > 1.0 {
        sine_offset(alt, location, alpha)
    } else {
        sine_offset(moment, location, alpha)
    };
    if value.abs() <= 1.0 {
        let offset =
            (value.asin().to_degrees().rem_euclid(360.0) / 360.0 + 0.5).rem_euclid(1.0) - 0.5;
        let m = date
            + if early {
                6.0 / 24.0 - offset
            } else {
                18.0 / 24.0 + offset
            };
        Some(local_from_apparent(m, location))
    } else {
        None
    }
}

fn moment_of_depression(
    approx: Moment,
    location: Location,
    alpha: f64,
    early: bool,
) -> Option<Moment> {
    let moment = approx_moment_of_depression(approx, location, alpha, early)?;
    if (approx - moment).abs() < 30.0 {
        Some(moment)
    } else {
        moment_of_depression(moment, location, alpha, early)
    }
}

pub fn refraction(location: Location) -> f64 {
    let h = location.elevation.max(0.0);
    let earth_r = 6.372e6;
    let dip = (earth_r / (earth_r + h)).acos().to_degrees();
    34.0 / 60.0 + dip + (19.0 / 3600.0) * h.sqrt()
}

pub fn dusk(date: f64, location: Location, alpha: f64) -> Option<Moment> {
    let m = moment_of_depression(date + 18.0 / 24.0, location, alpha, false)?;
    Some(Location::standard_from_local(m, location))
}

pub fn sunset(date: Moment, location: Location) -> Option<Moment> {
    let alpha = refraction(location) + 16.0 / 60.0;
    dusk(date, location, alpha)
}

pub fn arc_of_light(moment: Moment) -> f64 {
    let c = julian_centuries(moment);
    (lunar_latitude(c).to_radians().cos() * lunar_phase(moment, c).to_radians().cos())
        .acos()
        .to_degrees()
}

pub fn simple_best_view(date: f64, location: Location) -> Moment {
    let dark = dusk(date, location, 4.5);
    let best = dark.unwrap_or(date + 1.0);
    Location::universal_from_standard(best, location)
}

pub fn visible_crescent(date: Moment, location: Location) -> bool {
    let tee = simple_best_view((date - 1.0).floor(), location);
    let phase = lunar_phase(tee, julian_centuries(tee));
    let h = lunar_altitude(tee, location);
    let arcl = arc_of_light(tee);
    phase > 0.0 && phase < 90.0 && (10.6..=90.0).contains(&arcl) && h > 4.1
}

fn binary_search(mut l: f64, mut h: f64, test: impl Fn(f64) -> bool, epsilon: f64) -> f64 {
    loop {
        let mid = l + (h - l) / 2.0;
        if test(mid) {
            h = mid;
        } else {
            l = mid;
        }
        if (h - l) < epsilon {
            return mid;
        }
    }
}

pub fn observed_lunar_altitude(moment: Moment, location: Location) -> f64 {
    topocentric_lunar_altitude(moment, location) + refraction(location) + 16.0 / 60.0
}

pub fn moonset(date: Moment, location: Location) -> Option<Moment> {
    let moment = Location::universal_from_standard(date, location);
    let waxing = lunar_phase(date, julian_centuries(date)) < 180.0;
    let alt = observed_lunar_altitude(moment, location);
    let lat = location.latitude;
    let offset = alt / (4.0 * (90.0 - lat.abs()));
    let approx = if waxing {
        if offset > 0.0 {
            moment + offset
        } else {
            moment + 1.0 + offset
        }
    } else {
        moment - offset + 0.5
    };
    let set = binary_search(
        approx - 6.0 / 24.0,
        approx + 6.0 / 24.0,
        |x| observed_lunar_altitude(x, location) < 0.0,
        1.0 / 24.0 / 60.0,
    );
    if set < moment + 1.0 {
        let std = Location::standard_from_universal(set, location).max(date);
        if std < date {
            None
        } else {
            Some(std)
        }
    } else {
        None
    }
}

pub fn moonlag(date: Moment, location: Location) -> Option<f64> {
    let sun = sunset(date, location)?;
    match moonset(date, location) {
        Some(moon) => Some(moon - sun),
        None => Some(1.0),
    }
}

pub fn saudi_criterion(date: Moment, location: Location) -> Option<bool> {
    let sunset_m = sunset(date - 1.0, location)?;
    let tee = Location::universal_from_standard(sunset_m, location);
    let phase = lunar_phase(tee, julian_centuries(tee));
    let lag = moonlag(date - 1.0, location)?;
    Some(phase > 0.0 && phase < 90.0 && lag > 0.0)
}

pub fn lunar_phase_at_or_before(angle: f64, moment: Moment) -> Moment {
    let c = julian_centuries(moment);
    let tau = moment - (MEAN_SYNODIC_MONTH / 360.0) * ((lunar_phase(moment, c) - angle) % 360.0);
    let a = tau - 2.0;
    let b = moment.min(tau + 2.0);
    binary_search(
        a,
        b,
        |x| (lunar_phase(x, julian_centuries(x)) - angle).rem_euclid(360.0) < 180.0,
        1e-5,
    )
}

pub const ISLAMIC_EPOCH_FRIDAY: f64 = 227015.0;

pub fn saudi_new_month_on_or_before(date: Moment, location: Location) -> Moment {
    let last_new_moon = lunar_phase_at_or_before(0.0, date).floor();
    let age = date - last_new_moon;
    let mut tau = if age <= 3.0 && !saudi_criterion(date, location).unwrap_or(false) {
        last_new_moon - 30.0
    } else {
        last_new_moon
    };
    while !saudi_criterion(tau, location).unwrap_or(false) {
        tau += 1.0;
    }
    tau
}

pub fn observational_islamic_from_fixed(date: Moment, location: Location) -> (i32, u8, u8) {
    let crescent = saudi_new_month_on_or_before(date, location);
    let elapsed_months = ((crescent - ISLAMIC_EPOCH_FRIDAY) / MEAN_SYNODIC_MONTH).round() as i64;
    let year = (elapsed_months.div_euclid(12) + 1) as i32;
    let month = (elapsed_months.rem_euclid(12) + 1) as u8;
    let day = ((date - crescent) + 1.0) as u8;
    (year, month, day)
}

pub fn fixed_from_observational_islamic(
    year: i32,
    month: u8,
    day: u8,
    location: Location,
) -> Moment {
    let midmonth = ISLAMIC_EPOCH_FRIDAY
        + (((year as f64 - 1.0) * 12.0 + month as f64 - 0.5) * MEAN_SYNODIC_MONTH).floor();
    saudi_new_month_on_or_before(midmonth, location) + day as f64 - 1.0
}

pub const TEHRAN: Location = Location {
    latitude: 35.68,
    longitude: 52.5,
    elevation: 1100.0,
    utc_offset: 3.5 / 24.0,
};

pub const PERSIAN_EPOCH: i64 = 226896;

fn midday_in_tehran(date: f64) -> Moment {

    Location::universal_from_standard(date + 0.5, TEHRAN)
}

pub fn persian_new_year_on_or_before(date: f64) -> i64 {
    let approx = estimate_prior_solar_longitude(SPRING, midday_in_tehran(date));
    let mut day = approx.floor() as i64 - 1;
    while solar_longitude_at(midday_in_tehran(day as f64)).rem_euclid(360.0) > SPRING + 2.0 {
        day += 1;
    }
    day
}

pub fn fixed_from_persian(year: i32, month: u8, day: u8) -> i64 {

    let y = year as f64 - 1.0;
    let new_year =
        persian_new_year_on_or_before(PERSIAN_EPOCH as f64 + 180.0 + MEAN_TROPICAL_YEAR * y);
    let m = month as i64;
    let month_days = if m <= 7 {
        31 * (m - 1)
    } else {
        30 * (m - 1) + 6
    };
    new_year - 1 + month_days + day as i64
}

pub fn persian_from_fixed(date: i64) -> (i32, u8, u8) {
    let new_year = persian_new_year_on_or_before(date as f64);

    let year = (((new_year - PERSIAN_EPOCH) as f64 / MEAN_TROPICAL_YEAR).round() as i64 + 1) as i32;
    let day_of_year = date - fixed_from_persian(year, 1, 1) + 1;
    let month = if day_of_year <= 186 {
        (day_of_year as f64 / 31.0).ceil() as u8
    } else {
        ((day_of_year - 6) as f64 / 30.0).ceil() as u8
    };
    let day = (date - fixed_from_persian(year, month, 1) + 1) as u8;
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RataDie;

    const RD_VALS: [i64; 33] = [
        -214193, -61387, 25469, 49217, 171307, 210155, 253427, 369740, 400085, 434355, 452605,
        470160, 473837, 507850, 524156, 544676, 567118, 569477, 601716, 613424, 626596, 645554,
        664224, 671401, 694799, 704424, 708842, 709409, 709580, 727274, 728714, 744313, 764652,
    ];
    const EXPECTED: [f64; 33] = [
        119.47343190503307,
        254.2489611345809,
        181.43599673954304,
        188.66392267483752,
        289.0915666249348,
        59.11974154849304,
        228.31455470912624,
        34.46076992887538,
        63.18799596698955,
        2.4575913259759545,
        350.475934906397,
        13.498220866371412,
        37.403920329437824,
        81.02813003520714,
        313.86049865107634,
        19.95443016415811,
        176.05943166351062,
        344.92295174632454,
        79.96492181924987,
        99.30231774304411,
        121.53530416596914,
        88.56742889029556,
        129.289884101192,
        6.146910693067184,
        28.25199345351575,
        151.7806330331332,
        185.94586701843946,
        28.55560762159439,
        193.3478921554779,
        357.15125499424175,
        336.1706924761211,
        228.18487947607719,
        116.43935225951282,
    ];

    #[test]
    fn solar_longitude_matches_oracle_vectors() {
        for (rd, exp) in RD_VALS.iter().zip(EXPECTED.iter()) {
            let got = solar_longitude(julian_centuries(*rd as f64 + 0.5));
            assert!(
                (got - exp).abs() < 1e-9,
                "rd {rd}: got {got}, expected {exp}"
            );
        }
    }

    #[test]
    fn new_moon_matches_oracle_vectors() {

        const EXPECTED_NM: [f64; 33] = [
            -214174.60582868298,
            -61382.99532831192,
            25495.80977675628,
            49238.50244808781,
            171318.43531326813,
            210180.69184966758,
            253442.85936730343,
            369763.74641362444,
            400091.5783431683,
            434376.5781067696,
            452627.1919724953,
            470167.57836052414,
            473858.8532764285,
            507878.6668429224,
            524179.2470620894,
            544702.7538732041,
            567146.5131819838,
            569479.2032589674,
            601727.0335578924,
            613449.7621296605,
            626620.3698017383,
            645579.0767485882,
            664242.8867184789,
            671418.970538101,
            694807.5633711396,
            704433.4911827276,
            708863.5970001582,
            709424.4049294397,
            709602.0826867367,
            727291.2094001573,
            728737.4476913146,
            744329.5739998783,
            764676.1912733881,
        ];
        for (rd, exp) in RD_VALS.iter().zip(EXPECTED_NM.iter()) {
            let got = new_moon_at_or_after(*rd as f64);
            assert!(
                (got - exp).abs() < 1e-7,
                "rd {rd}: got {got}, expected {exp}"
            );
        }
    }

    #[test]
    fn lunar_longitude_matches_oracle_vectors() {
        const EXPECTED_LL: [f64; 33] = [
            244.85390528515035,
            208.85673853696503,
            213.74684265158967,
            292.04624333935743,
            156.81901407583166,
            108.0556329349528,
            39.35609790324581,
            98.56585102192106,
            332.95829627335894,
            92.25965175091615,
            78.13202909213766,
            274.9469953879383,
            128.3628442664409,
            89.51845094326185,
            24.607322526832988,
            53.4859568448797,
            187.89852001941696,
            320.1723620959754,
            314.0425667275923,
            145.47406514043587,
            185.03050779751646,
            142.18913274552065,
            253.74337531953228,
            151.64868501335397,
            287.9877436469169,
            25.626707154435444,
            290.28830064619893,
            189.91314245171338,
            284.93173002623826,
            152.3390442635215,
            51.66226507971774,
            26.68206023138705,
            175.5008226195208,
        ];
        for (rd, exp) in RD_VALS.iter().zip(EXPECTED_LL.iter()) {
            let got = lunar_longitude(julian_centuries(*rd as f64));
            assert!(
                (got - exp).abs() < 1e-6,
                "rd {rd}: got {got}, expected {exp}"
            );
        }
    }

    #[test]
    fn lunar_latitude_matches_oracle_vectors() {
        const EXPECTED_LAT: [f64; 33] = [
            2.4527590208461576,
            -4.90223034654341,
            -2.9394693592610484,
            5.001904508580623,
            -3.208909826304433,
            0.894361559890105,
            -3.8633355687979827,
            -2.5224444701068927,
            1.0320696124422062,
            3.005689926794408,
            1.613842956502888,
            4.766740664556875,
            4.899202930916035,
            4.838473946607273,
            2.301475724501815,
            -0.8905637199828537,
            4.7657836433468495,
            -2.737358003826797,
            -4.035652608005429,
            -3.157214517184652,
            -1.8796147336498752,
            -3.379519408995276,
            -4.398341468078228,
            2.099198567294447,
            5.268746128633113,
            -1.6722994521634027,
            4.6820126551666865,
            3.705518210116447,
            2.493964063649065,
            -4.167774638752936,
            -2.873757531859998,
            -4.667251128743298,
            5.138562328560728,
        ];
        for (rd, exp) in RD_VALS.iter().zip(EXPECTED_LAT.iter()) {
            let got = lunar_latitude(julian_centuries(*rd as f64));
            assert!(
                (got - exp).abs() < 1e-6,
                "rd {rd}: got {got}, expected {exp}"
            );
        }
    }

    #[test]
    fn lunar_distance_matches_oracle_vectors() {
        const EXPECTED_DIST: [f64; 33] = [
            387624532.22874624,
            393677431.9167689,
            402232943.80299366,
            392558548.8426357,
            366799795.8707107,
            365107305.3822873,
            401995197.0122423,
            404025417.6150537,
            377671971.8515077,
            403160628.6150732,
            375160036.9057225,
            369934038.34809774,
            402543074.28064245,
            374847147.6967837,
            403469151.42100906,
            386211365.4436033,
            385336015.6086019,
            400371744.7464432,
            395970218.00750065,
            383858113.5538787,
            389634540.7722341,
            390868707.6609328,
            368015493.693663,
            399800095.77937233,
            404273360.3039046,
            382777325.7053601,
            378047375.3350678,
            385774023.9948239,
            371763698.0990588,
            362461692.8996066,
            394214466.3812425,
            405787977.04490376,
            404202826.42484397,
        ];
        for (rd, exp) in RD_VALS.iter().zip(EXPECTED_DIST.iter()) {
            let got = lunar_distance(*rd as f64);
            assert!(
                (got - exp).abs() < 1.0,
                "rd {rd}: got {got}, expected {exp}"
            );
        }
    }

    #[test]
    fn lunar_altitude_and_parallax_match_oracle_vectors() {
        const EXPECTED_ALT: [f64; 33] = [
            -13.163184128188277,
            -7.281425833096932,
            -77.1499009115812,
            -30.401178593900795,
            71.84857827681589,
            -43.79857984753659,
            40.65320421851649,
            -40.2787255279427,
            29.611156512065406,
            -19.973178784428228,
            -23.740743779700097,
            30.956688013173505,
            -18.88869091014726,
            -32.16116202243495,
            -45.68091943596022,
            -50.292110029959986,
            -54.3453056090807,
            -34.56600009726776,
            44.13198955291821,
            -57.539862986917285,
            -62.08243959461623,
            -54.07209109276471,
            -16.120452006695814,
            23.864594681196934,
            32.95014668614863,
            72.69165128891194,
            -29.849481790038908,
            31.610644151367637,
            -42.21968940776054,
            28.6478092363985,
            -38.95055354031621,
            27.601977078963245,
            -54.85468160086816,
        ];
        const EXPECTED_PLX: [f64; 33] = [
            0.9180377088277034,
            0.9208275970231943,
            0.20205836298974478,
            0.8029475944705559,
            0.3103764190238057,
            0.7224552232666479,
            0.6896953754669151,
            0.6900664438899986,
            0.8412721901635796,
            0.8519504336914271,
            0.8916972264563727,
            0.8471706468502866,
            0.8589744596828851,
            0.8253387743371953,
            0.6328154405175959,
            0.60452566100182,
            0.5528114670829496,
            0.7516491660573382,
            0.6624140811593374,
            0.5109678575066725,
            0.4391324179474404,
            0.5486027633624313,
            0.9540023420545446,
            0.835939538308717,
            0.7585615249134946,
            0.284040095327141,
            0.8384425157447107,
            0.8067682261382678,
            0.7279971552035109,
            0.8848306274359499,
            0.720943806048675,
            0.7980998225232075,
            0.5204553405568378,
        ];

        let oracle_ref = Location {
            latitude: 6427.0 / 300.0,
            longitude: 11947.0 / 300.0,
            elevation: 298.0,
            utc_offset: 1.0 / 8.0,
        };
        for ((rd, ea), ep) in RD_VALS
            .iter()
            .zip(EXPECTED_ALT.iter())
            .zip(EXPECTED_PLX.iter())
        {
            let alt = lunar_altitude(*rd as f64, oracle_ref);
            assert!((alt - ea).abs() < 1e-6, "alt rd {rd}: {alt} vs {ea}");
            let plx = lunar_parallax(alt, *rd as f64);
            assert!((plx - ep).abs() < 1e-6, "parallax rd {rd}: {plx} vs {ep}");
        }

        assert!((obliquity(J2000) - 23.4392911).abs() < 1e-4);

        for rd in RD_VALS.iter() {
            let a = topocentric_lunar_altitude(*rd as f64, GOLGOTHA);
            assert!(
                (-90.0..=90.0).contains(&a),
                "Golgotha altitude out of range: {a}"
            );
        }
    }

    #[test]
    fn sunset_matches_oracle_vectors_at_jerusalem() {

        let jerusalem = Location {
            latitude: 31.78,
            longitude: 35.24,
            elevation: 740.0,
            utc_offset: 1.0 / 12.0,
        };
        const EXPECTED: [f64; 33] = [
            -214192.2194436165,
            -61386.30267524347,
            25469.734889564967,
            49217.72851448112,
            171307.70878832813,
            210155.77420199668,
            253427.70087725233,
            369740.7627365203,
            400085.77677703864,
            434355.74808897293,
            452605.7425360138,
            470160.75310216413,
            473837.76440251875,
            507850.7840412511,
            524156.7225351998,
            544676.7561346035,
            567118.7396585084,
            569477.7396636717,
            601716.784057734,
            613424.7870863203,
            626596.781969136,
            645554.7863087669,
            664224.778132625,
            671401.7496876866,
            694799.7602310368,
            704424.7619096127,
            708842.730647343,
            709409.7603906896,
            709580.7240122546,
            727274.745361792,
            728714.734750938,
            744313.699821144,
            764652.7844809336,
        ];
        for (rd, exp) in RD_VALS.iter().zip(EXPECTED.iter()) {
            let s = sunset(*rd as f64, jerusalem).expect("sunset occurs");
            assert!((s - exp).abs() < 1e-4, "sunset rd {rd}: {s} vs {exp}");
        }

        for rd in RD_VALS.iter() {
            assert!(sunset(*rd as f64, GOLGOTHA).is_some());
        }
    }

    #[test]
    fn crescent_criterion_confirmed_by_the_oracle() {

        let oracle_ref = Location {
            latitude: 6427.0 / 300.0,
            longitude: 11947.0 / 300.0,
            elevation: 298.0,
            utc_offset: 1.0 / 8.0,
        };
        const EXPECTED: [bool; 33] = [
            false, false, true, false, false, true, false, true, false, false, true, false, false,
            true, true, true, true, false, false, true, true, true, false, false, false, false,
            false, false, true, false, true, false, true,
        ];
        for (rd, exp) in RD_VALS.iter().zip(EXPECTED.iter()) {
            let got = saudi_criterion(*rd as f64, oracle_ref).expect("criterion defined");
            assert_eq!(
                got, *exp,
                "crescent criterion rd {rd}: got {got}, oracle says {exp}"
            );
        }

        let visible = (730000..730059)
            .filter(|&rd| saudi_criterion(rd as f64, GOLGOTHA) == Some(true))
            .count();
        assert!(
            visible > 0 && visible < 59,
            "criterion must discriminate at Golgotha: {visible}/59"
        );
    }

    #[test]
    fn observational_islamic_month_confirmed_by_oracle() {

        let oracle_ref = Location {
            latitude: 6427.0 / 300.0,
            longitude: 11947.0 / 300.0,
            elevation: 298.0,
            utc_offset: 1.0 / 8.0,
        };
        const FIXED: [i64; 31] = [
            -214193, -61387, 25469, 49217, 171307, 210155, 253427, 369740, 400085, 434355, 452605,
            470160, 473837, 507850, 524156, 544676, 567118, 569477, 613424, 626596, 645554, 664224,
            671401, 694799, 704424, 708842, 709409, 709580, 728714, 744313, 764652,
        ];
        const EXPECTED: [f64; 31] = [
            -214203.0, -61412.0, 25467.0, 49210.0, 171290.0, 210152.0, 253414.0, 369735.0,
            400063.0, 434348.0, 452598.0, 470139.0, 473830.0, 507850.0, 524150.0, 544674.0,
            567118.0, 569450.0, 613421.0, 626592.0, 645551.0, 664214.0, 671391.0, 694779.0,
            704405.0, 708835.0, 709396.0, 709573.0, 728709.0, 744301.0, 764647.0,
        ];
        for (rd, exp) in FIXED.iter().zip(EXPECTED.iter()) {
            let got = saudi_new_month_on_or_before(*rd as f64, oracle_ref);
            assert_eq!(
                got, *exp,
                "umm-al-qura month rd {rd}: got {got}, oracle {exp}"
            );
        }

        for rd in [731000.0, 738000.0, 745000.0] {
            let (y, m, _d) = observational_islamic_from_fixed(rd, GOLGOTHA);
            let month_start = fixed_from_observational_islamic(y, m, 1, GOLGOTHA);
            assert!(
                (rd - month_start) >= 0.0 && (rd - month_start) < 31.0,
                "rd {rd} in month {y}-{m}"
            );
        }
    }

    #[test]
    fn persian_astronomical_nowruz_and_epoch() {
        use crate::{GREGORIAN, JULIAN};

        assert_eq!(JULIAN.fixed_from(622, 3, 19).0, PERSIAN_EPOCH);

        let nowruz = |ap: i32| GREGORIAN.ymd_from_fixed(RataDie(fixed_from_persian(ap, 1, 1)));
        assert_eq!(nowruz(1393), (2014, 3, 21));
        assert_eq!(nowruz(1399), (2020, 3, 20));
        assert_eq!(nowruz(1400), (2021, 3, 21));
        assert_eq!(nowruz(1403), (2024, 3, 20));
        assert_eq!(nowruz(1404), (2025, 3, 21));

        for rd in (fixed_from_persian(1380, 1, 1)..fixed_from_persian(1420, 1, 1)).step_by(23) {
            let (y, m, d) = persian_from_fixed(rd);
            assert_eq!(fixed_from_persian(y, m, d), rd, "rd {rd} -> ({y},{m},{d})");
            assert!((1..=12).contains(&m) && (1..=31).contains(&d));
        }
    }

    #[test]
    fn equinox_finder_2024() {

        let around = GREGORIAN.fixed_from(2024, 1, 1).0 as f64;
        let eq = solar_longitude_after(0.0, around);
        let (y, m, d) = GREGORIAN.ymd_from_fixed(RataDie(eq.floor() as i64));
        assert_eq!((y, m), (2024, 3));
        assert_eq!(d, 20, "vernal equinox 2024 = March 20");

        let lon = solar_longitude_at(eq).rem_euclid(360.0);
        assert!(
            lon < 0.01 || lon > 359.99,
            "longitude at equinox ~0, got {lon}"
        );
    }
}
