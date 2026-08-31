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

/// Pseudo-random offset in `[0, 1)` for the froxel at `coord` on frame
/// `frame`, used to move that froxel's lighting sample around inside its own
/// depth slice.
///
/// Shaft edges are high-contrast, so the grid's Z quantisation shows far more
/// than it does in uniform fog: every slice lights from a single depth, and the
/// step between a lit slice and a shadowed one draws a visible band across the
/// beam. Offsetting each froxel's sample point within its own slice trades
/// those bands for fine noise, which temporal accumulation then averages away.
///
/// A hash of the froxel coordinate *and* the frame index, not a random number
/// generator: headless replays have to stay reproducible, so the same froxel on
/// the same frame must produce the same offset on every run. The multipliers are
/// the usual spatial-hash primes; the products wrap on purpose (`wrapping_mul`
/// here, and u32 arithmetic wraps by definition in WGSL), which is what mixes
/// the high bits down into the low sixteen this actually reads.
///
/// Mirrored in WGSL by `froxel_jitter` in `post_process.rs`'s shared froxel
/// block -- the same arrangement [`henyey_greenstein`] uses -- and
/// `the_wgsl_froxel_jitter_matches_the_rust_one` asserts the two agree bit for
/// bit.
pub fn froxel_jitter(coord: [u32; 3], frame: u32) -> f32 {
    let h = coord[0].wrapping_mul(73856093)
        ^ coord[1].wrapping_mul(19349663)
        ^ coord[2].wrapping_mul(83492791)
        ^ frame.wrapping_mul(2654435761);
    (h & 0xFFFF) as f32 / 65536.0
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

    #[test]
    fn froxel_jitter_is_deterministic_for_a_given_frame() {
        // Headless replays must stay reproducible: the same froxel and frame
        // index have to produce the same offset every time, or two runs of the
        // same recording diverge and every pixel assertion downstream becomes
        // a coin toss.
        for frame in [0u32, 1, 7, 4096] {
            for coord in [[0u32, 0, 0], [3, 11, 41], [FROXEL_X - 1, FROXEL_Y - 1, 0]] {
                let first = froxel_jitter(coord, frame);
                for _ in 0..4 {
                    assert_eq!(
                        froxel_jitter(coord, frame),
                        first,
                        "froxel {coord:?} on frame {frame} must always jitter by \
                         the same amount"
                    );
                }
            }
        }
    }

    #[test]
    fn different_froxels_in_one_frame_jitter_differently() {
        // The other half of the determinism claim, and the half that says this
        // is dithering at all: an offset that were the same everywhere would be
        // a constant shift of the whole grid, which moves the bands rather than
        // breaking them up.
        let frame = 3u32;
        let mut seen = std::collections::HashSet::new();
        // A whole slab of the real grid, so this cannot pass on a lucky pair.
        for y in 0..8u32 {
            for x in 0..8u32 {
                seen.insert(froxel_jitter([x, y, 5], frame).to_bits());
            }
        }
        assert!(
            seen.len() > 48,
            "64 neighbouring froxels produced only {} distinct offsets on one \
             frame -- a jitter that barely varies across the grid shifts the \
             slice boundaries instead of dissolving them",
            seen.len()
        );
    }

    #[test]
    fn one_froxel_jitters_differently_across_frames() {
        // What makes the noise *temporal*: if a froxel took the same offset
        // every frame, averaging frames together would converge on that one
        // biased sample and the dither would never resolve into a gradient.
        let coord = [17u32, 23, 9];
        let mut seen = std::collections::HashSet::new();
        for frame in 0..16u32 {
            seen.insert(froxel_jitter(coord, frame).to_bits());
        }
        assert!(
            seen.len() > 12,
            "one froxel produced only {} distinct offsets over 16 frames; \
             temporal accumulation cannot average away a fixed offset",
            seen.len()
        );
    }

    #[test]
    fn the_offset_never_leaves_its_own_slice() {
        // `[0, 1)`, because the shader interpolates between this slice's near
        // and far edge with it. A value outside that range would sample a
        // neighbouring slice's depth, which is exactly the cross-slice leak the
        // dither exists to avoid.
        for frame in 0..4u32 {
            for z in 0..FROXEL_Z {
                for x in 0..16u32 {
                    let j = froxel_jitter([x, x * 7 + 1, z], frame);
                    assert!(
                        (0.0..1.0).contains(&j),
                        "froxel [{x}, {}, {z}] on frame {frame} jittered to {j}, \
                         outside its own slice",
                        x * 7 + 1
                    );
                }
            }
        }
    }
}
