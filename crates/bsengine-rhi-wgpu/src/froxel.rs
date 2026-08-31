//! Froxel grid geometry and the scattering phase function.
//!
//! Deliberately GPU-free so the two things that fail silently -- the phase
//! function's normalisation and the depth slicing -- can be asserted
//! exactly instead of eyeballed in a render.

/// Froxel grid dimensions. X/Y are a coarse screen tiling; Z is the depth
/// axis. 160x90 keeps a 16:9 aspect at 1/8 resolution.
pub const FROXEL_X: u32 = 160;
/// See [`FROXEL_X`].
pub const FROXEL_Y: u32 = 90;
/// Depth slices. See [`froxel_slice_depth`] for why they are not linear.
pub const FROXEL_Z: u32 = 64;

/// World-space view depth at the far edge of slice `slice` (0-based) of a
/// grid spanning `near..far`.
///
/// **Exponential, not linear.** Perspective means a near froxel covers far
/// less world space than a distant one; slicing linearly wastes most of the
/// volume on distance nobody looks at while under-sampling the near field
/// where fog gradients are actually visible. Linear slicing produces fog
/// that looks almost right and bands close to the camera -- the classic
/// froxel bug, and one that no end-to-end "is it foggy" test catches.
pub fn froxel_slice_depth(slice: u32, near: f32, far: f32) -> f32 {
    let t = (slice + 1) as f32 / FROXEL_Z as f32;
    near * (far / near).powf(t)
}

/// Henyey-Greenstein phase function: the fraction of light scattered from
/// direction `cos_theta` (the cosine between the incoming light direction
/// and the view direction) for anisotropy `g`.
///
/// `g` in (-1, 1): 0 is isotropic, positive scatters forward (the bright
/// halo around a light seen through haze), negative scatters backward.
///
/// The `1/(4*pi)` factor is what makes this integrate to 1 over the
/// sphere. Dropping it -- easy, since many references quote the
/// unnormalised form -- makes the fog roughly 12x too bright, which reads
/// as "the density constant needs tuning" rather than as a bug.
pub fn henyey_greenstein(cos_theta: f32, g: f32) -> f32 {
    let g = g.clamp(-0.99, 0.99);
    let g2 = g * g;
    let denom = 1.0 + g2 - 2.0 * g * cos_theta;
    (1.0 - g2) / (4.0 * std::f32::consts::PI * denom.max(1e-4).powf(1.5))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_phase_function_integrates_to_one_over_the_sphere() {
        // The normalisation check. Integrate over theta with the sin(theta)
        // Jacobian and the 2*pi azimuthal factor.
        for &g in &[0.0f32, 0.3, -0.3, 0.7] {
            let steps = 2000;
            let mut total = 0.0f64;
            for i in 0..steps {
                let theta = std::f32::consts::PI * (i as f32 + 0.5) / steps as f32;
                let d_theta = std::f32::consts::PI / steps as f32;
                total += (henyey_greenstein(theta.cos(), g)
                    * theta.sin()
                    * 2.0
                    * std::f32::consts::PI
                    * d_theta) as f64;
            }
            assert!(
                (total - 1.0).abs() < 0.02,
                "phase function with g={g} integrated to {total}, not 1.0 -- \
                 a missing 1/(4pi) makes fog ~12x too bright and merely looks \
                 like a mistuned density"
            );
        }
    }

    #[test]
    fn positive_g_scatters_forward() {
        let fwd = henyey_greenstein(1.0, 0.6);
        let back = henyey_greenstein(-1.0, 0.6);
        assert!(
            fwd > back,
            "positive g must scatter forward: {fwd} vs {back}"
        );
    }

    #[test]
    fn zero_g_is_isotropic() {
        let a = henyey_greenstein(1.0, 0.0);
        let b = henyey_greenstein(-1.0, 0.0);
        let c = henyey_greenstein(0.0, 0.0);
        assert!((a - b).abs() < 1e-6 && (a - c).abs() < 1e-6);
        assert!(
            (a - 1.0 / (4.0 * std::f32::consts::PI)).abs() < 1e-6,
            "isotropic scattering must equal 1/(4pi), got {a}"
        );
    }

    #[test]
    fn slice_depths_are_monotonic_and_end_on_the_far_plane() {
        let (near, far) = (0.1f32, 100.0f32);
        let mut prev = near;
        for s in 0..FROXEL_Z {
            let d = froxel_slice_depth(s, near, far);
            assert!(d > prev, "slice {s} depth {d} did not increase past {prev}");
            prev = d;
        }
        assert!(
            (prev - far).abs() < 0.01,
            "the last slice must land on the far plane, got {prev}"
        );
    }

    #[test]
    fn slices_are_denser_near_the_camera_than_far_away() {
        // The whole point of exponential slicing. If this fails the
        // distribution is linear and fog will band up close.
        let (near, far) = (0.1f32, 100.0f32);
        let first = froxel_slice_depth(0, near, far) - near;
        let last = froxel_slice_depth(FROXEL_Z - 1, near, far)
            - froxel_slice_depth(FROXEL_Z - 2, near, far);
        assert!(
            last > first * 10.0,
            "far slices must be much thicker than near ones: near {first}, far {last}"
        );
    }
}
