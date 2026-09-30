//! FXAA, measured at the pixel: does it soften aliased edges, and leave
//! everything that is not an edge alone?
//!
//! The fixture is `pixels_taa.rs`'s: a bright square turned out of pixel
//! alignment on a dark backdrop, lit only by emission, with bloom, tone
//! mapping and SSAO off -- so without antialiasing every pixel is exactly one
//! of two tones, and antialiasing is precisely the appearance of tones in
//! between.

mod common;

use bsengine_core::Fxaa;
use common::{Draw, Harness, Light, Pixels, Scene};
use glam::Vec3;

const BACKDROP_EMISSIVE: f32 = 0.05;
const SQUARE_EMISSIVE: f32 = 1.0;

fn square_scene(cube: u64, degrees: f32, square: f32, fxaa: Option<Fxaa>) -> Scene {
    two_tone_scene(cube, degrees, BACKDROP_EMISSIVE, square, fxaa)
}

fn two_tone_scene(
    cube: u64,
    degrees: f32,
    backdrop: f32,
    square: f32,
    fxaa: Option<Fxaa>,
) -> Scene {
    Scene {
        draws: vec![
            Draw::new(cube, Vec3::ZERO)
                .scaled(Vec3::new(40.0, 40.0, 0.2), Vec3::new(0.0, 0.0, -5.0))
                .colour(Vec3::ZERO)
                .emissive(Vec3::splat(backdrop)),
            Draw::new(cube, Vec3::ZERO)
                .scaled(Vec3::splat(1.5), Vec3::ZERO)
                .rotated_z(degrees.to_radians())
                .colour(Vec3::ZERO)
                .emissive(Vec3::splat(square)),
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
        fxaa,
        ..Scene::default()
    }
}

/// The 30-degree square of `pixels_taa.rs`: every row of its edges crosses
/// the pixel grid at a fresh offset, so the stair-steps are short.
fn steep(cube: u64, fxaa: Option<Fxaa>) -> Scene {
    square_scene(cube, 30.0, SQUARE_EMISSIVE, fxaa)
}

/// Seven degrees: nearly axis-aligned, so each stair-step of the top and
/// bottom edges runs about eight pixels. A pixel in the middle of a step has
/// the same neighbours as one on a straight edge; only walking along the
/// edge to the step's end tells FXAA it is on a slope, and how far it is from
/// the step decides how much it blends.
fn shallow(cube: u64, fxaa: Option<Fxaa>) -> Scene {
    square_scene(cube, 7.0, SQUARE_EMISSIVE, fxaa)
}

/// Pixels neither clearly the square nor clearly the backdrop: a tenth of the
/// range in from either end (see `pixels_taa.rs`, which this mirrors).
fn intermediate_tone_count(p: &Pixels) -> u32 {
    let backdrop = p.luma(0, 0);
    let square = p.centre_luma();
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

/// Every pixel within `radius` of a tone change in `p` (an un-antialiased,
/// two-tone frame): the only place antialiasing may change anything.
fn silhouette_mask(p: &Pixels, radius: i32) -> Vec<bool> {
    let (w, h) = (p.width as i32, p.height as i32);
    let mut edge = vec![false; (w * h) as usize];
    for y in 0..h {
        for x in 0..w {
            let here = p.luma(x as u32, y as u32);
            let differs = [(-1, 0), (1, 0), (0, -1), (0, 1)].iter().any(|(dx, dy)| {
                let (nx, ny) = (x + dx, y + dy);
                nx >= 0 && ny >= 0 && nx < w && ny < h && p.luma(nx as u32, ny as u32) != here
            });
            edge[(y * w + x) as usize] = differs;
        }
    }
    let mut dilated = vec![false; edge.len()];
    for y in 0..h {
        for x in 0..w {
            if !edge[(y * w + x) as usize] {
                continue;
            }
            for dy in -radius..=radius {
                for dx in -radius..=radius {
                    let (nx, ny) = (x + dx, y + dy);
                    if nx >= 0 && ny >= 0 && nx < w && ny < h {
                        dilated[(ny * w + nx) as usize] = true;
                    }
                }
            }
        }
    }
    dilated
}

/// Absent and disabled both skip the pass: the same frame to the bit.
#[test]
fn fxaa_disabled_matches_no_fxaa_component_at_all() {
    let mut h = Harness::new();
    let cube = h.cube();
    let none = h.render(&steep(cube, None));
    let disabled = h.render(&steep(
        cube,
        Some(Fxaa {
            enabled: false,
            ..Fxaa::default()
        }),
    ));
    assert!(!disabled.differs_from(&none));
}

/// The steep square's aliased edges pick up in-between tones -- none without
/// FXAA, where every pixel is one side or the other -- and nothing away from
/// the silhouette changes. The second half is what the first means: a plain
/// blur of the whole frame would pass the first alone.
#[test]
fn fxaa_softens_a_diagonal_edge_and_nothing_else() {
    let mut h = Harness::new();
    let cube = h.cube();
    let off = h.render(&steep(cube, None));
    let on = h.render(&steep(cube, Some(Fxaa::default())));
    let (off_count, on_count) = (intermediate_tone_count(&off), intermediate_tone_count(&on));
    eprintln!("steep: intermediate tones off {off_count}, on {on_count}");
    assert_eq!(off_count, 0, "premise: without FXAA the edge is hard");
    assert!(
        on_count >= 100,
        "FXAA puts in-between tones along the edge: only {on_count}"
    );

    let mask = silhouette_mask(&off, 2);
    let strays: Vec<(u32, u32)> = (0..off.height)
        .flat_map(|y| (0..off.width).map(move |x| (x, y)))
        .filter(|&(x, y)| {
            let (a, b) = (off.at(x, y), on.at(x, y));
            (0..3).any(|c| a[c].abs_diff(b[c]) > 2) && !mask[(y * off.width + x) as usize]
        })
        .collect();
    assert!(
        strays.is_empty(),
        "{} pixels away from the silhouette changed, e.g. {:?}",
        strays.len(),
        &strays[..strays.len().min(5)]
    );
}

/// On the shallow edge, the edge walk is what does the work: a pixel in the
/// middle of an eight-pixel step looks, to its 3x3 neighbourhood, exactly
/// like a pixel on a straight edge, and only the walk to the step's end says
/// how far along the slope it is. So FXAA puts in-between tones along most
/// of each step, not only at its corners.
#[test]
fn fxaa_blends_along_the_length_of_a_shallow_steps() {
    let mut h = Harness::new();
    let cube = h.cube();
    let off = h.render(&shallow(cube, None));
    let on = h.render(&shallow(cube, Some(Fxaa::default())));
    let no_sub = h.render(&shallow(
        cube,
        Some(Fxaa {
            subpixel: 0.0,
            ..Fxaa::default()
        }),
    ));
    let (off_count, on_count, no_sub_count) = (
        intermediate_tone_count(&off),
        intermediate_tone_count(&on),
        intermediate_tone_count(&no_sub),
    );
    eprintln!(
        "shallow: intermediate tones off {off_count}, on {on_count}, subpixel 0 {no_sub_count}"
    );
    assert_eq!(off_count, 0, "premise: without FXAA the edge is hard");
    assert!(
        no_sub_count >= 100,
        "with the sub-pixel term off, the edge walk alone blends along the steps: {no_sub_count}"
    );
    assert!(
        on_count > no_sub_count + 20,
        "and the sub-pixel term adds its own softening at the steps' corners: \
         {on_count} with it, {no_sub_count} without"
    );
}

/// The thresholds: a faint edge -- the square barely brighter than the
/// backdrop -- is below FXAA's default contrast thresholds and left alone,
/// the "don't smooth noise" half of the contract; with both thresholds at
/// zero the same edge is smoothed, so the edge was there to find.
#[test]
fn a_faint_edge_is_left_alone_until_the_thresholds_allow_it() {
    let mut h = Harness::new();
    let cube = h.cube();
    let faint = |fxaa| square_scene(cube, 30.0, 0.058, fxaa);
    let off = h.render(&faint(None));
    let default = h.render(&faint(Some(Fxaa::default())));
    let permissive = h.render(&faint(Some(Fxaa {
        edge_threshold: 0.0,
        edge_threshold_min: 0.0,
        ..Fxaa::default()
    })));
    eprintln!(
        "faint: backdrop {} square {}",
        off.luma(0, 0),
        off.centre_luma()
    );
    assert!(
        off.centre_luma() > off.luma(0, 0),
        "premise: the square is brighter than the backdrop"
    );
    assert!(
        !default.differs_from(&off),
        "a faint edge is below the default thresholds"
    );
    assert!(
        permissive.differs_from(&off),
        "with no thresholds the faint edge is smoothed"
    );
}

/// With TAA on as well, the resolve accumulates FXAA's output rather than
/// the raw composite: the frame differs from TAA alone.
#[test]
fn taa_accumulates_the_fxaa_output() {
    let mut h = Harness::new();
    let cube = h.cube();
    let taa_only = {
        let mut s = steep(cube, None);
        s.taa = Some(bsengine_core::Taa::default());
        h.render_converged(&s, 16)
    };
    let both = {
        let mut s = steep(cube, Some(Fxaa::default()));
        s.taa = Some(bsengine_core::Taa::default());
        h.render_converged(&s, 16)
    };
    assert!(
        both.differs_from(&taa_only),
        "FXAA reaches the frame TAA resolves"
    );
}

/// Contrast is judged perceptually, as FXAA's thresholds are written for: a
/// dark edge -- black against 0.02 linear -- spans 0.02 of linear luma, under
/// the 0.0833 minimum, but about 0.14 of perceptual (gamma) luma, well over
/// it, and the eye sees it as an edge. The post targets are sRGB and sample
/// as linear light, so this is smoothed only if the shader converts first.
#[test]
fn a_dark_edge_is_found_by_its_perceptual_contrast() {
    let mut h = Harness::new();
    let cube = h.cube();
    let dark = |fxaa| two_tone_scene(cube, 30.0, 0.0, 0.02, fxaa);
    let off = h.render(&dark(None));
    let on = h.render(&dark(Some(Fxaa::default())));
    assert!(
        off.centre_luma() > off.luma(0, 0) + 20.0,
        "premise: in the 8-bit sRGB frame the dark square stands out: {} vs {}",
        off.centre_luma(),
        off.luma(0, 0)
    );
    assert!(on.differs_from(&off), "the dark edge is smoothed");
}
