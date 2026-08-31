//! Spherical harmonics (L2, 9 coefficients) for light probes.
//!
//! Deliberately GPU-free: projection, evaluation and interpolation are all
//! pure functions over `glam` types, so the normalisation constants -- the
//! part of this that is easy to get wrong and hard to notice -- can be
//! asserted exactly rather than eyeballed in a render.

use glam::Vec3;

/// Coefficient count for an L2 (order-3) spherical harmonic expansion.
pub const SH_COEFF_COUNT: usize = 9;

/// One probe's radiance, as L2 spherical-harmonic coefficients (RGB each).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShL2 {
    /// The nine coefficients, in the standard l-major order:
    /// index 0 is l=0; 1..=3 are l=1; 4..=8 are l=2.
    pub coeffs: [Vec3; SH_COEFF_COUNT],
}

impl Default for ShL2 {
    fn default() -> Self {
        Self {
            coeffs: [Vec3::ZERO; SH_COEFF_COUNT],
        }
    }
}

/// The nine real SH basis functions evaluated in direction `d`
/// (`d` must be normalized).
pub fn sh_basis(d: Vec3) -> [f32; SH_COEFF_COUNT] {
    [
        0.282_095,       // l=0
        0.488_603 * d.y, // l=1
        0.488_603 * d.z,
        0.488_603 * d.x,
        1.092_548 * d.x * d.y, // l=2
        1.092_548 * d.y * d.z,
        0.315_392 * (3.0 * d.z * d.z - 1.0),
        1.092_548 * d.x * d.z,
        0.546_274 * (d.x * d.x - d.y * d.y),
    ]
}

impl ShL2 {
    /// Accumulates one sample of `radiance` arriving from direction `dir`,
    /// covering `solid_angle` steradians.
    pub fn accumulate(&mut self, dir: Vec3, radiance: Vec3, solid_angle: f32) {
        let basis = sh_basis(dir);
        for (coeff, b) in self.coeffs.iter_mut().zip(basis) {
            *coeff += radiance * b * solid_angle;
        }
    }

    /// Evaluates the **irradiance** (not radiance) arriving at a surface
    /// with normal `n`, using the standard cosine-lobe convolution
    /// constants. These per-band factors are what turn a radiance
    /// expansion into the diffuse irradiance a Lambertian surface sees;
    /// omitting them is the classic bug that leaves probe lighting looking
    /// plausible but far too directional.
    pub fn eval_irradiance(&self, n: Vec3) -> Vec3 {
        // The cosine-lobe convolution factors are exactly pi, 2pi/3 and
        // pi/4; written in closed form both because it is more accurate
        // than the decimal expansion and because clippy's `approx_constant`
        // rejects a literal pi.
        const A0: f32 = std::f32::consts::PI;
        const A1: f32 = 2.0 * std::f32::consts::PI / 3.0;
        const A2: f32 = std::f32::consts::FRAC_PI_4;
        let basis = sh_basis(n);
        let band = [A0, A1, A1, A1, A2, A2, A2, A2, A2];
        let mut out = Vec3::ZERO;
        for i in 0..SH_COEFF_COUNT {
            out += self.coeffs[i] * basis[i] * band[i];
        }
        // Irradiance is never negative; ringing from a truncated expansion
        // can push it below zero, which would darken rather than light.
        out.max(Vec3::ZERO)
    }

    /// Component-wise linear blend, used by trilinear interpolation.
    /// Blending coefficients and evaluating once is both cheaper than and
    /// equivalent to evaluating each probe and blending the results,
    /// because SH evaluation is linear in the coefficients.
    pub fn lerp(a: &ShL2, b: &ShL2, t: f32) -> ShL2 {
        let mut out = ShL2::default();
        for i in 0..SH_COEFF_COUNT {
            out.coeffs[i] = a.coeffs[i].lerp(b.coeffs[i], t);
        }
        out
    }
}

/// The eight trilinear weights for `frac` (each component in `[0, 1]`),
/// ordered so index bit 0 is x, bit 1 is y, bit 2 is z.
pub fn trilinear_weights(frac: Vec3) -> [f32; 8] {
    let mut w = [0.0f32; 8];
    for (i, slot) in w.iter_mut().enumerate() {
        let wx = if i & 1 == 0 { 1.0 - frac.x } else { frac.x };
        let wy = if i & 2 == 0 { 1.0 - frac.y } else { frac.y };
        let wz = if i & 4 == 0 { 1.0 - frac.z } else { frac.z };
        *slot = wx * wy * wz;
    }
    w
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Integrating a constant environment must leave only the l=0
    /// coefficient non-zero: every higher band's basis function integrates
    /// to zero over the sphere. This is exact and is the assertion that
    /// catches a wrong normalisation constant -- a bug that otherwise just
    /// reads as "the scene looks a bit bright".
    #[test]
    fn a_uniform_environment_projects_onto_l0_only() {
        let mut sh = ShL2::default();
        // Fibonacci sphere: even coverage without pole clustering.
        let n = 4096;
        let solid_angle = 4.0 * std::f32::consts::PI / n as f32;
        for i in 0..n {
            let y = 1.0 - 2.0 * (i as f32 + 0.5) / n as f32;
            let r = (1.0 - y * y).max(0.0).sqrt();
            let theta = std::f32::consts::PI * (1.0 + 5.0f32.sqrt()) * i as f32;
            let dir = Vec3::new(theta.cos() * r, y, theta.sin() * r).normalize();
            sh.accumulate(dir, Vec3::ONE, solid_angle);
        }
        assert!(
            (sh.coeffs[0].x - 0.282_095 * 4.0 * std::f32::consts::PI).abs() < 0.05,
            "l=0 must equal the basis constant times the full solid angle, got {}",
            sh.coeffs[0].x
        );
        for (i, c) in sh.coeffs.iter().enumerate().skip(1) {
            assert!(
                c.length() < 0.05,
                "coefficient {i} should vanish for a uniform environment, got {c:?}"
            );
        }
    }

    /// Reconstruction of a uniform environment must return that same
    /// constant in every direction -- the round trip is the real check.
    #[test]
    fn a_uniform_environment_reconstructs_to_its_own_colour() {
        let mut sh = ShL2::default();
        let n = 4096;
        let solid_angle = 4.0 * std::f32::consts::PI / n as f32;
        for i in 0..n {
            let y = 1.0 - 2.0 * (i as f32 + 0.5) / n as f32;
            let r = (1.0 - y * y).max(0.0).sqrt();
            let theta = std::f32::consts::PI * (1.0 + 5.0f32.sqrt()) * i as f32;
            let dir = Vec3::new(theta.cos() * r, y, theta.sin() * r).normalize();
            sh.accumulate(dir, Vec3::splat(0.5), solid_angle);
        }
        for probe_dir in [Vec3::X, Vec3::Y, Vec3::Z, Vec3::NEG_X, -Vec3::Y] {
            let e = sh.eval_irradiance(probe_dir.normalize());
            assert!(
                (e.x - 0.5 * std::f32::consts::PI).abs() < 0.1,
                "uniform 0.5 environment should give pi*0.5 irradiance in \
                 every direction, got {e:?} for {probe_dir:?}"
            );
        }
    }

    #[test]
    fn a_bright_direction_dominates_the_reconstruction() {
        // A single bright lobe must read brighter from that side than the
        // opposite side -- otherwise the l=1 terms carry no direction and
        // the probe is just an ambient constant.
        let mut sh = ShL2::default();
        sh.accumulate(Vec3::X, Vec3::new(10.0, 0.0, 0.0), 1.0);
        let front = sh.eval_irradiance(Vec3::X);
        let back = sh.eval_irradiance(Vec3::NEG_X);
        assert!(
            front.x > back.x,
            "the lit side must be brighter: front {front:?} back {back:?}"
        );
    }

    #[test]
    fn trilinear_weights_always_sum_to_one() {
        for &f in &[
            Vec3::ZERO,
            Vec3::ONE,
            Vec3::splat(0.5),
            Vec3::new(0.1, 0.9, 0.3),
            Vec3::new(0.75, 0.25, 1.0),
        ] {
            let sum: f32 = trilinear_weights(f).iter().sum();
            assert!(
                (sum - 1.0).abs() < 1e-5,
                "weights for {f:?} summed to {sum}, not 1.0 -- a partition of \
                 unity is what keeps interpolation from brightening or \
                 darkening the result"
            );
        }
    }

    #[test]
    fn a_point_on_a_lattice_corner_takes_that_corner_alone() {
        let w = trilinear_weights(Vec3::ZERO);
        assert!((w[0] - 1.0).abs() < 1e-6);
        for x in &w[1..] {
            assert!(x.abs() < 1e-6);
        }
        let w = trilinear_weights(Vec3::ONE);
        assert!((w[7] - 1.0).abs() < 1e-6);
        for x in &w[..7] {
            assert!(x.abs() < 1e-6);
        }
    }

    #[test]
    fn lerp_at_the_ends_returns_the_endpoints() {
        let mut a = ShL2::default();
        a.coeffs[0] = Vec3::X;
        let mut b = ShL2::default();
        b.coeffs[0] = Vec3::Y;
        assert_eq!(ShL2::lerp(&a, &b, 0.0).coeffs[0], Vec3::X);
        assert_eq!(ShL2::lerp(&a, &b, 1.0).coeffs[0], Vec3::Y);
    }
}
