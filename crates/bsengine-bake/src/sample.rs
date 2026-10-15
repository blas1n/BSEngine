//! Random numbers and directions, deterministically.
//!
//! Every texel seeds its own generator from its position, so a bake is the
//! same bytes on every run and on every thread count -- the property that
//! lets a test compare two bakes, and lets a re-bake of an unchanged scene
//! produce no diff in the atlas files.

use glam::Vec3;
use std::f32::consts::TAU;

/// SplitMix64: tiny, fast, and good enough for Monte Carlo directions.
#[derive(Debug, Clone)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed ^ 0x9E37_79B9_7F4A_7C15)
    }

    /// A generator for one texel: mixed from the layer and position so
    /// neighbouring texels are uncorrelated.
    pub fn for_texel(seed: u64, layer: u32, x: u32, y: u32) -> Self {
        let mut r = Self::new(seed);
        r.0 ^= (layer as u64) << 48 ^ (y as u64) << 24 ^ x as u64;
        r.next_u64();
        r
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform in `[0, 1)`.
    pub fn next(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }
}

/// Two unit vectors completing `n` to a right-handed orthonormal basis
/// (Duff et al. 2017, "Building an Orthonormal Basis, Revisited").
pub fn basis(n: Vec3) -> (Vec3, Vec3) {
    let sign = 1.0f32.copysign(n.z);
    let a = -1.0 / (sign + n.z);
    let b = n.x * n.y * a;
    (
        Vec3::new(1.0 + sign * n.x * n.x * a, sign * b, -sign * n.x),
        Vec3::new(b, sign + n.y * n.y * a, -n.y),
    )
}

/// A direction about `n` with probability density `cos(theta) / pi`.
///
/// Cosine weighting is what makes the irradiance estimate a plain average:
/// `E / pi = (1/N) * sum L(w_i)` with no per-sample weight, because the
/// density cancels the cosine in the integrand exactly.
pub fn cosine_hemisphere(n: Vec3, u1: f32, u2: f32) -> Vec3 {
    let r = u1.sqrt();
    let phi = TAU * u2;
    let (t, b) = basis(n);
    let z = (1.0 - u1).max(0.0).sqrt();
    (t * (r * phi.cos()) + b * (r * phi.sin()) + n * z).normalize()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_basis_is_orthonormal_for_every_direction() {
        let mut rng = Rng::new(1);
        for _ in 0..1000 {
            let n = (Vec3::new(rng.next(), rng.next(), rng.next()) * 2.0 - 1.0).normalize();
            let (t, b) = basis(n);
            for (x, y) in [(t, b), (t, n), (b, n)] {
                assert!(x.dot(y).abs() < 1e-5, "{x} . {y} for n = {n}");
            }
            assert!((t.length() - 1.0).abs() < 1e-5 && (b.length() - 1.0).abs() < 1e-5);
        }
    }

    /// Cosine-weighted directions have mean cos(theta) of 2/3 (the mean of
    /// `cos` under the density `cos/pi` over the hemisphere). A uniform
    /// sampler would give 1/2, so this tells the two apart.
    #[test]
    fn hemisphere_samples_are_cosine_weighted() {
        let mut rng = Rng::new(2);
        let n = Vec3::new(0.3, 0.9, -0.2).normalize();
        let count = 200_000;
        let mut sum = 0.0f64;
        for _ in 0..count {
            let d = cosine_hemisphere(n, rng.next(), rng.next());
            assert!(d.dot(n) >= -1e-6, "below the horizon: {d}");
            sum += d.dot(n) as f64;
        }
        let mean = sum / count as f64;
        assert!((mean - 2.0 / 3.0).abs() < 0.005, "mean cos = {mean}");
    }

    #[test]
    fn a_texel_generator_is_reproducible_and_texels_differ() {
        let a = Rng::for_texel(9, 0, 3, 4).next_u64();
        assert_eq!(a, Rng::for_texel(9, 0, 3, 4).next_u64());
        assert_ne!(a, Rng::for_texel(9, 0, 4, 4).next_u64());
        assert_ne!(a, Rng::for_texel(9, 1, 3, 4).next_u64());
    }
}
