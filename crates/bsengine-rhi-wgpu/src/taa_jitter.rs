//! Sub-pixel jitter offsets for temporal antialiasing.
//!
//! Each frame the projection matrix is nudged by a fraction of a pixel so
//! successive frames sample different points inside the same pixel;
//! accumulating those samples over time is what antialiases the edge. The
//! offsets come from a Halton sequence rather than random values because it
//! is low-discrepancy -- it spreads samples evenly instead of clumping the
//! way uniform random does over a short window, which matters when only a
//! handful of frames contribute before the history is clamped away.
//!
//! Pure math, deliberately GPU-free, so the sequence is unit-testable
//! without an adapter.

/// Length of the jitter cycle. After this many frames the offsets repeat,
/// which bounds the pattern and keeps it deterministic for tests.
pub const JITTER_SEQUENCE_LENGTH: u32 = 8;

/// The `index`-th value of the radical-inverse (van der Corput) sequence in
/// `base`, in `[0, 1)`. Halton is just this evaluated in two coprime bases.
fn radical_inverse(mut index: u32, base: u32) -> f32 {
    let mut result = 0.0f32;
    let mut fraction = 1.0f32 / base as f32;
    while index > 0 {
        result += (index % base) as f32 * fraction;
        index /= base;
        fraction /= base as f32;
    }
    result
}

/// Sub-pixel offset for `frame_index`, in **pixels**, each component in
/// `[-0.5, 0.5)`. Halton(2, 3), the standard choice for TAA.
///
/// The offset is centered on zero so the jittered image stays aligned with
/// the unjittered one on average; an uncentered `[0, 1)` offset would shift
/// the whole picture half a pixel.
pub fn jitter_offset_pixels(frame_index: u32) -> (f32, f32) {
    let i = frame_index % JITTER_SEQUENCE_LENGTH + 1; // +1: index 0 gives (0,0), a wasted frame
    (radical_inverse(i, 2) - 0.5, radical_inverse(i, 3) - 0.5)
}

/// Converts a pixel-space jitter offset into the clip-space translation to
/// apply to a projection matrix. Clip space spans 2 units across `width`
/// pixels, hence `2 / width`; the Y term is negated because clip-space Y
/// points up while pixel-space Y points down.
pub fn jitter_clip_offset(frame_index: u32, width: u32, height: u32) -> (f32, f32) {
    if width == 0 || height == 0 {
        return (0.0, 0.0);
    }
    let (jx, jy) = jitter_offset_pixels(frame_index);
    (2.0 * jx / width as f32, -2.0 * jy / height as f32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn radical_inverse_matches_known_values() {
        // Base 2: 1 -> 0.5, 2 -> 0.25, 3 -> 0.75. These are the textbook
        // van der Corput values; getting them right is what makes the
        // sequence low-discrepancy rather than merely varied.
        assert!((radical_inverse(1, 2) - 0.5).abs() < 1e-6);
        assert!((radical_inverse(2, 2) - 0.25).abs() < 1e-6);
        assert!((radical_inverse(3, 2) - 0.75).abs() < 1e-6);
        // Base 3: 1 -> 1/3, 2 -> 2/3.
        assert!((radical_inverse(1, 3) - 1.0 / 3.0).abs() < 1e-6);
        assert!((radical_inverse(2, 3) - 2.0 / 3.0).abs() < 1e-6);
    }

    #[test]
    fn every_offset_stays_inside_one_pixel() {
        for frame in 0..64 {
            let (x, y) = jitter_offset_pixels(frame);
            assert!(
                (-0.5..0.5).contains(&x) && (-0.5..0.5).contains(&y),
                "frame {frame} jittered outside its own pixel: ({x}, {y}) -- an \
                 offset past half a pixel samples the neighbouring pixel and \
                 blurs instead of antialiasing"
            );
        }
    }

    #[test]
    fn no_offset_repeats_within_one_cycle() {
        let mut seen: Vec<(f32, f32)> = Vec::new();
        for frame in 0..JITTER_SEQUENCE_LENGTH {
            let o = jitter_offset_pixels(frame);
            for prev in &seen {
                assert!(
                    (prev.0 - o.0).abs() > 1e-6 || (prev.1 - o.1).abs() > 1e-6,
                    "frame {frame} repeats offset {o:?} within the cycle -- a \
                     repeated sample contributes no new information"
                );
            }
            seen.push(o);
        }
    }

    #[test]
    fn the_sequence_repeats_after_a_full_cycle() {
        assert_eq!(
            jitter_offset_pixels(0),
            jitter_offset_pixels(JITTER_SEQUENCE_LENGTH),
            "the cycle must be exactly JITTER_SEQUENCE_LENGTH long"
        );
    }

    #[test]
    fn a_zero_sized_target_gets_no_jitter_instead_of_a_division_by_zero() {
        assert_eq!(jitter_clip_offset(3, 0, 0), (0.0, 0.0));
    }

    #[test]
    fn clip_offset_shrinks_as_the_target_grows() {
        // The same sub-pixel nudge is a smaller slice of clip space on a
        // bigger target -- if this did not hold, jitter would be resolution
        // dependent and over-blur small windows.
        let (small, _) = jitter_clip_offset(1, 100, 100);
        let (large, _) = jitter_clip_offset(1, 1000, 1000);
        assert!(small.abs() > large.abs());
    }
}
