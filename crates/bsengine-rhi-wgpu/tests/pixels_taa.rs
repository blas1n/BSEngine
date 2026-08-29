//! TAA: does temporal antialiasing actually soften a diagonal edge, and does
//! it leave everything else alone?
//!
//! Both halves matter. "The edge looks smoother" is not an assertion, and a
//! test that only checked *that something changed* would pass just as happily
//! for an implementation that blurred the whole frame. So this file measures
//! the intended effect on the edge and, separately, the absence of any effect
//! where there is no edge.

mod common;

use common::{Draw, Harness, Light, Pixels, Scene};
use glam::Vec3;

/// How far the rotated square is turned out of pixel alignment.
///
/// The angle is the whole point of the fixture. An axis-aligned rectangle
/// lands on exact pixel columns and rows, produces no stair-steps, and so
/// gives antialiasing nothing to do -- a test built on one proves nothing
/// while appearing to pass. Thirty degrees is far from every multiple of
/// ninety and is not a slope with a short repeating period, so the edge
/// crosses the pixel grid at a fresh sub-pixel offset on essentially every
/// row.
const EDGE_ANGLE_DEGREES: f32 = 30.0;

/// Emissive value of the backdrop, and of the square in front of it.
///
/// The two are far apart because aliasing is only visible across contrast:
/// on a low-contrast edge the "in between" tones are within rounding of both
/// sides and nothing can be counted.
const BACKDROP_EMISSIVE: f32 = 0.05;
const SQUARE_EMISSIVE: f32 = 1.0;

/// A bright square, turned [`EDGE_ANGLE_DEGREES`] out of alignment, on a dark
/// backdrop.
///
/// Everything is lit purely by emission -- `colour(Vec3::ZERO)` with a black
/// light -- so both surfaces are exactly flat in colour. A shaded surface has
/// its own gradients, and every one of them would be counted as an
/// "intermediate tone" whether or not TAA ran.
///
/// The backdrop is a flattened cube rather than empty space on purpose: the
/// resolve pass has nothing to reproject where the depth buffer is still at
/// its far value, so an edge against the sky would exercise the early-out
/// instead of the accumulation this file is about.
///
/// Bloom, tone mapping and SSAO are each switched off explicitly, because the
/// surface applies `Default` for any of them a scene leaves absent and all
/// three default to enabled. Bloom spills light across the silhouette and
/// SSAO shades near it; either would manufacture intermediate tones with TAA
/// off and hide what TAA did.
fn rotated_square_scene(cube: u64, taa: Option<bsengine_core::Taa>) -> Scene {
    Scene {
        draws: vec![
            Draw::new(cube, Vec3::ZERO)
                .scaled(Vec3::new(40.0, 40.0, 0.2), Vec3::new(0.0, 0.0, -5.0))
                .colour(Vec3::ZERO)
                .emissive(Vec3::splat(BACKDROP_EMISSIVE)),
            Draw::new(cube, Vec3::ZERO)
                .scaled(Vec3::splat(1.5), Vec3::ZERO)
                .rotated_z(EDGE_ANGLE_DEGREES.to_radians())
                .colour(Vec3::ZERO)
                .emissive(Vec3::splat(SQUARE_EMISSIVE)),
        ],
        light: Light {
            color: Vec3::ZERO,
            ambient: Vec3::ZERO,
            ..Light::default()
        },
        bloom: Some(bsengine_core::Bloom {
            enabled: false,
            ..Default::default()
        }),
        tone_map: Some(bsengine_core::ToneMap {
            enabled: false,
            ..Default::default()
        }),
        ssao: Some(bsengine_core::AmbientOcclusion {
            enabled: false,
            ..Default::default()
        }),
        taa,
        ..Scene::default()
    }
}

/// How many frames a converged reading accumulates over.
///
/// Twice the Halton cycle length, so every sub-pixel position in the sequence
/// has contributed at least twice.
const CONVERGED_FRAMES: u32 = 16;

/// Counts pixels that are neither clearly the square nor clearly the backdrop.
///
/// On a hard aliased edge every pixel is one or the other: the rasterizer
/// either covered the pixel centre or it did not, and the emissive fixture
/// makes both outcomes exactly flat. Antialiasing is precisely the appearance
/// of values in between, so this count is the countable form of "the edge got
/// smoother".
///
/// The two reference tones are read from the frame itself -- a corner, which
/// only the backdrop reaches, and the centre, which is deep inside the square
/// -- rather than hardcoded, so the measure survives any change to the clear
/// colour or the transfer function.
fn intermediate_tone_count(p: &Pixels) -> u32 {
    let backdrop = p.luma(0, 0);
    let square = p.centre_luma();
    assert!(
        square - backdrop > 100.0,
        "the fixture must be high contrast for this measure to mean anything, \
         but the backdrop reads {backdrop} and the square {square}"
    );
    // A tenth of the way in from either end. Anything closer to an end than
    // that is "clearly" that side; the band still catches a single blend step,
    // which at the default history weight moves a pixel a tenth of the range.
    let margin = 0.1 * (square - backdrop);
    let (lo, hi) = (backdrop + margin, square - margin);

    let mut count = 0;
    for y in 0..p.height {
        for x in 0..p.width {
            let l = p.luma(x, y);
            if l > lo && l < hi {
                count += 1;
            }
        }
    }
    count
}

/// One 8-bit sRGB channel as the linear value the shader actually blended.
///
/// The post-process targets are sRGB, so `textureSample` hands the resolve
/// decoded light and the write re-encodes it. Reasoning about a blend weight
/// in 8-bit units without undoing that would be off by the transfer function.
fn srgb_to_linear(c: u8) -> f32 {
    let c = c as f32 / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// The inverse of [`srgb_to_linear`], in 8-bit units but unrounded so a
/// difference of two encoded values keeps its fractional part.
fn linear_to_srgb(c: f32) -> f32 {
    let s = if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    };
    s * 255.0
}

#[test]
fn taa_softens_a_diagonal_edge() {
    let mut h = Harness::new();
    let cube = h.cube();

    let off = h.render(&rotated_square_scene(cube, None));
    let on = h.render_converged(
        &rotated_square_scene(cube, Some(bsengine_core::Taa::default())),
        CONVERGED_FRAMES,
    );

    let off_count = intermediate_tone_count(&off);
    let on_count = intermediate_tone_count(&on);
    eprintln!("intermediate-tone pixels: TAA off = {off_count}, TAA on = {on_count}");

    // With no antialiasing anywhere in the pipeline and a flat-shaded fixture,
    // a pixel is one side of the edge or the other. Not "few": none.
    assert_eq!(
        off_count,
        0,
        "without TAA the edge must be hard -- {off_count} intermediate pixels \
         means something else in the pipeline is already softening it, and \
         this test would be measuring that instead. Frame: {}",
        off.describe()
    );

    // The square is ~100px tall with four diagonal sides, so a working resolve
    // has hundreds of edge pixels to work with. Requiring a hundred is well
    // clear of noise while not pinning the exact geometry.
    assert!(
        on_count >= 100,
        "converged TAA should put intermediate tones along the diagonal edge, \
         but only {on_count} pixels landed between the backdrop and the \
         square. Frame: {}",
        on.describe()
    );
}

#[test]
fn taa_leaves_the_interior_of_a_solid_region_alone() {
    let mut h = Harness::new();
    let cube = h.cube();

    let off = h.render(&rotated_square_scene(cube, None));
    let on = h.render_converged(
        &rotated_square_scene(cube, Some(bsengine_core::Taa::default())),
        CONVERGED_FRAMES,
    );

    // Well inside the square. Its half-height on screen is about 50 pixels, so
    // +-12 from the centre stays clear of every edge even as the sub-pixel
    // jitter moves the silhouette around.
    let (cx, cy) = (off.width / 2, off.height / 2);
    let interior = [
        (cx, cy),
        (cx - 12, cy),
        (cx + 12, cy),
        (cx, cy - 12),
        (cx, cy + 12),
    ];

    // This is the regression that makes `taa_softens_a_diagonal_edge`
    // meaningful. Without it, an implementation that smeared the entire frame
    // -- a plain blur, say -- would satisfy the edge test perfectly.
    for (x, y) in interior {
        let a = off.at(x, y);
        let b = on.at(x, y);
        for c in 0..3 {
            assert!(
                a[c].abs_diff(b[c]) <= 2,
                "TAA changed a pixel that is nowhere near an edge: ({x}, {y}) \
                 went from {a:?} to {b:?}. Antialiasing is supposed to touch \
                 the silhouette, not the flat interior"
            );
        }
    }

    // And the backdrop, for the same reason from the other side.
    let corner_off = off.at(2, 2);
    let corner_on = on.at(2, 2);
    for c in 0..3 {
        assert!(
            corner_off[c].abs_diff(corner_on[c]) <= 2,
            "TAA changed the empty backdrop: {corner_off:?} -> {corner_on:?}"
        );
    }

    // Five samples prove five pixels. The same claim over the whole frame:
    // every pixel TAA moved has to sit on the silhouette. An effect that
    // leaked outward -- a global exposure shift, an image displaced by the
    // jitter it was supposed to cancel, history sampled at the wrong offset --
    // would move pixels the aliased edge never reached.
    let on_silhouette = silhouette_mask(&off, 2);
    let mut strays = Vec::new();
    for y in 0..off.height {
        for x in 0..off.width {
            let (a, b) = (off.at(x, y), on.at(x, y));
            let moved = (0..3).any(|c| a[c].abs_diff(b[c]) > 2);
            if moved && !on_silhouette[(y * off.width + x) as usize] {
                strays.push((x, y, a, b));
            }
        }
    }
    assert!(
        strays.is_empty(),
        "{} pixels changed away from the silhouette, e.g. {:?} -- TAA must \
         work on the edge, not on the picture",
        strays.len(),
        &strays[..strays.len().min(5)]
    );
}

/// Marks every pixel within `radius` of a tone change in `p`.
///
/// Built from the TAA-off frame, where the fixture is strictly two-toned, so
/// the mask is exactly "the aliased edge and its immediate surroundings" --
/// the only place antialiasing has any business changing anything.
fn silhouette_mask(p: &Pixels, radius: i32) -> Vec<bool> {
    let mut edge = vec![false; (p.width * p.height) as usize];
    for y in 0..p.height {
        for x in 0..p.width {
            let here = p.luma(x, y);
            let differs = (x > 0 && p.luma(x - 1, y) != here)
                || (x + 1 < p.width && p.luma(x + 1, y) != here)
                || (y > 0 && p.luma(x, y - 1) != here)
                || (y + 1 < p.height && p.luma(x, y + 1) != here);
            if differs {
                edge[(y * p.width + x) as usize] = true;
            }
        }
    }

    let mut dilated = vec![false; edge.len()];
    for y in 0..p.height as i32 {
        for x in 0..p.width as i32 {
            if !edge[(y * p.width as i32 + x) as usize] {
                continue;
            }
            for dy in -radius..=radius {
                for dx in -radius..=radius {
                    let (nx, ny) = (x + dx, y + dy);
                    if nx >= 0 && ny >= 0 && nx < p.width as i32 && ny < p.height as i32 {
                        dilated[(ny * p.width as i32 + nx) as usize] = true;
                    }
                }
            }
        }
    }
    dilated
}

#[test]
fn taa_converges_instead_of_shimmering() {
    let mut h = Harness::new();
    let cube = h.cube();
    let scene = || rotated_square_scene(cube, Some(bsengine_core::Taa::default()));

    // The camera never moves, so once the accumulation has settled one more
    // frame should barely move it. A resolve that never settled would swing
    // between jitter positions instead, and the two readings would differ by
    // the full contrast of the fixture wherever the silhouette moved.
    let a = h.render_converged(&scene(), CONVERGED_FRAMES);
    let b = h.render_converged(&scene(), CONVERGED_FRAMES + 1);

    let mut worst = (0u8, 0u32, 0u32);
    let mut moved = 0u32;
    for y in 0..a.height {
        for x in 0..a.width {
            let (pa, pb) = (a.at(x, y), b.at(x, y));
            let d = (0..3).map(|c| pa[c].abs_diff(pb[c])).max().unwrap_or(0);
            if d > worst.0 {
                worst = (d, x, y);
            }
            if d > 2 {
                moved += 1;
            }
        }
    }
    eprintln!(
        "frame {CONVERGED_FRAMES} vs {}: worst channel delta {} at ({}, {}), \
         {moved} pixels moved by more than 2",
        CONVERGED_FRAMES + 1,
        worst.0,
        worst.1,
        worst.2
    );

    // One more frame can only pull a pixel `1 - history_blend` of the way from
    // where it stands toward the incoming colour. That is what "converged"
    // means here, and the bound is read off the component rather than tuned to
    // the measurement.
    //
    // The step has to be measured in *linear* light: the resolve blends values
    // the sampler decoded from the sRGB target, while the readback is
    // re-encoded. A tenth of the range is a much larger 8-bit jump near black
    // than near white, so the worst case is a pixel sitting at the backdrop
    // tone -- which is exactly where a silhouette pixel that just changed
    // coverage sits.
    let backdrop = srgb_to_linear(a.at(0, 0)[1]);
    let square = srgb_to_linear(a.centre()[1]);
    let step = (1.0 - bsengine_core::Taa::default().history_blend) * (square - backdrop);
    let bound = (linear_to_srgb(backdrop + step) - linear_to_srgb(backdrop)).ceil() as u8 + 2;
    assert!(
        worst.0 <= bound,
        "an extra frame moved a pixel by {} levels at ({}, {}), more than the \
         {bound} a single blend step can account for -- the image is still \
         swinging between jitter positions rather than converging",
        worst.0,
        worst.1,
        worst.2
    );

    // And the movement must be confined to the silhouette, not spread over the
    // frame: the fixture's edge is a few hundred pixels out of 30000.
    assert!(
        moved < (a.width * a.height) / 10,
        "{moved} of {} pixels were still moving between consecutive converged \
         frames; convergence should leave only the silhouette in motion",
        a.width * a.height
    );
}

/// Turning the component off must return the exact rendering of not having it
/// at all -- an "off" switch that leaves a residue is not off.
///
/// Note what this does *not* isolate: both the jitter and the resolve are
/// gated on `enabled`, in the harness as in `bsengine-render`, so a static
/// scene cannot separate the two gates. That the `taa_enabled` uniform itself
/// reaches the shader is certified by `taa_softens_a_diagonal_edge`, which
/// cannot produce a single intermediate tone unless the flag arrives set.
#[test]
fn taa_disabled_matches_no_taa_component_at_all() {
    let mut h = Harness::new();
    let cube = h.cube();

    // `Taa::default()` is enabled, so the disabled case has to be spelled out.
    let disabled = || {
        Some(bsengine_core::Taa {
            enabled: false,
            ..Default::default()
        })
    };

    // One frame, and then the full accumulation. The single frame alone would
    // be a weak claim: with no history yet, the resolve degenerates to a
    // passthrough whatever the flag says, so it would agree even for an
    // implementation that ignored `enabled` entirely.
    let absent_once = h.render(&rotated_square_scene(cube, None));
    let disabled_once = h.render(&rotated_square_scene(cube, disabled()));
    assert!(
        !disabled_once.differs_from(&absent_once),
        "a disabled Taa component should render exactly as no component does; \
         absent gave {}, disabled gave {}",
        absent_once.describe(),
        disabled_once.describe()
    );

    let absent = h.render_converged(&rotated_square_scene(cube, None), CONVERGED_FRAMES);
    let off = h.render_converged(&rotated_square_scene(cube, disabled()), CONVERGED_FRAMES);
    assert!(
        !off.differs_from(&absent),
        "over {CONVERGED_FRAMES} frames a disabled Taa component should still \
         match no component at all; absent gave {}, disabled gave {}",
        absent.describe(),
        off.describe()
    );

    // And the comparison above must not be two ways of saying "nothing
    // happened": the same fixture with the component enabled has to differ.
    let on = h.render_converged(
        &rotated_square_scene(cube, Some(bsengine_core::Taa::default())),
        CONVERGED_FRAMES,
    );
    assert!(
        on.differs_from(&off),
        "enabling the component changed nothing, so the equality above proves \
         nothing either; both frames read {}",
        on.describe()
    );
}
