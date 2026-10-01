//! SMAA, measured at the pixel: does it reconstruct aliased edges toward
//! their true coverage, and leave everything that is not an edge alone?
//!
//! The fixture is `pixels_fxaa.rs`'s: a bright square turned out of pixel
//! alignment on a dark backdrop, lit only by emission, with bloom, tone
//! mapping and SSAO off -- so without antialiasing every pixel is exactly one
//! of two tones. "True coverage" is TAA converged over 16 jittered frames,
//! which averages the rasteriser's own coverage at 16 sub-pixel positions.

mod common;

use bsengine_core::{Fxaa, Smaa, SmaaQuality};
use common::{Draw, Harness, Light, Pixels, Scene};
use glam::Vec3;

const BACKDROP_EMISSIVE: f32 = 0.05;
const SQUARE_EMISSIVE: f32 = 1.0;

fn two_tone_scene(cube: u64, degrees: f32, backdrop: f32, square: f32) -> Scene {
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
        ..Scene::default()
    }
}

fn square(cube: u64, degrees: f32, smaa: Option<Smaa>) -> Scene {
    Scene {
        smaa,
        ..two_tone_scene(cube, degrees, BACKDROP_EMISSIVE, SQUARE_EMISSIVE)
    }
}

fn quality(q: SmaaQuality) -> Option<Smaa> {
    Some(Smaa {
        enabled: true,
        quality: q,
    })
}

/// Pixels neither clearly the square nor clearly the backdrop: a tenth of the
/// range in from either end (as `pixels_fxaa.rs`).
fn intermediate_tone_count(p: &Pixels) -> u32 {
    let backdrop = p.luma(0, 0);
    let square = p.centre_luma();
    let margin = 0.1 * (square - backdrop);
    let (lo, hi) = (backdrop + margin, square - margin);
    (0..p.height)
        .flat_map(|y| (0..p.width).map(move |x| (x, y)))
        .filter(|&(x, y)| {
            let l = p.luma(x, y);
            l > lo && l < hi
        })
        .count() as u32
}

/// Every pixel within `radius` of a tone change in `p` (an un-antialiased,
/// two-tone frame): the only place antialiasing may change anything.
fn silhouette_mask(p: &Pixels, radius: i32) -> Vec<bool> {
    let (w, h) = (p.width as i32, p.height as i32);
    let edge: Vec<bool> = (0..h)
        .flat_map(|y| (0..w).map(move |x| (x, y)))
        .map(|(x, y)| {
            let here = p.luma(x as u32, y as u32);
            [(-1, 0), (1, 0), (0, -1), (0, 1)].iter().any(|(dx, dy)| {
                let (nx, ny) = (x + dx, y + dy);
                nx >= 0 && ny >= 0 && nx < w && ny < h && p.luma(nx as u32, ny as u32) != here
            })
        })
        .collect();
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

/// How far `p`'s silhouette is from the converged-TAA coverage reference,
/// summed over the pixels near the aliased frame's edges.
fn coverage_error(off: &Pixels, reference: &Pixels, p: &Pixels) -> f32 {
    let mask = silhouette_mask(off, 2);
    (0..off.height)
        .flat_map(|y| (0..off.width).map(move |x| (x, y)))
        .filter(|&(x, y)| mask[(y * off.width + x) as usize])
        .map(|(x, y)| (p.luma(x, y) - reference.luma(x, y)).abs())
        .sum()
}

fn coverage_reference(h: &mut Harness, cube: u64, degrees: f32) -> Pixels {
    let mut s = square(cube, degrees, None);
    s.taa = Some(bsengine_core::Taa::default());
    h.render_converged(&s, 16)
}

/// Absent and disabled both skip the passes: the same frame to the bit.
#[test]
fn smaa_disabled_matches_no_smaa_component_at_all() {
    let mut h = Harness::new();
    let cube = h.cube();
    let none = h.render(&square(cube, 30.0, None));
    let disabled = h.render(&square(
        cube,
        30.0,
        Some(Smaa {
            enabled: false,
            ..Smaa::default()
        }),
    ));
    assert!(!disabled.differs_from(&none));
}

/// The steep square's aliased edges pick up in-between tones -- none without
/// SMAA -- and nothing away from the silhouette changes. The second half is
/// what the first means: a plain blur of the whole frame passes the first.
#[test]
fn smaa_softens_a_diagonal_edge_and_nothing_else() {
    let mut h = Harness::new();
    let cube = h.cube();
    let off = h.render(&square(cube, 30.0, None));
    let on = h.render(&square(cube, 30.0, Some(Smaa::default())));
    let (off_count, on_count) = (intermediate_tone_count(&off), intermediate_tone_count(&on));
    eprintln!("steep: intermediate tones off {off_count}, on {on_count}");
    assert_eq!(off_count, 0, "premise: without SMAA the edge is hard");
    assert!(
        on_count >= 100,
        "SMAA puts in-between tones along the edge: only {on_count}"
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

/// Coverage error at `degrees` for the aliased frame and for each preset in
/// `presets`, in that order.
fn coverage_errors(degrees: f32, presets: &[SmaaQuality]) -> (f32, Vec<f32>) {
    let mut h = Harness::new();
    let cube = h.cube();
    let off = h.render(&square(cube, degrees, None));
    let reference = coverage_reference(&mut h, cube, degrees);
    let errors = presets
        .iter()
        .map(|&q| {
            let on = h.render(&square(cube, degrees, quality(q)));
            let e = coverage_error(&off, &reference, &on);
            eprintln!("{degrees} deg {q:?}: error {e}");
            e
        })
        .collect();
    let e_off = coverage_error(&off, &reference, &off);
    eprintln!("{degrees} deg aliased: error {e_off}");
    (e_off, errors)
}

/// Reconstructed *correctly*: on a range of edge angles, each preset brings
/// the silhouette closer to its true coverage than the aliased frame is. A
/// wrong area lookup -- a flipped table, a misread search length -- still
/// makes in-between tones, but in the wrong places.
///
/// 45 degrees exactly is left out on purpose: that edge runs through pixel
/// corners, where the 16-sample reference itself is nearly a hard step, and
/// the aliased staircase is already almost the reference.
#[test]
fn every_preset_moves_edges_toward_their_true_coverage() {
    let presets = [
        SmaaQuality::Low,
        SmaaQuality::Medium,
        SmaaQuality::High,
        SmaaQuality::Ultra,
    ];
    for degrees in [2.0, 7.0, 30.0, 42.0, 60.0] {
        let (e_off, errors) = coverage_errors(degrees, &presets);
        for (q, e_on) in presets.iter().zip(errors) {
            assert!(
                e_on < e_off * 0.8,
                "{degrees} deg {q:?}: SMAA brings the edge closer to its true coverage: \
                 error {e_on}, aliased {e_off}"
            );
        }
    }
}

/// How far each search runs is what tells a pixel where on a long step it
/// is. At 2 degrees each stair-step is about 29 pixels long: Low's 4 search
/// steps reach 8 pixels each way, Medium's 8 reach 16, and only the second
/// sees enough of the step. The two presets differ in nothing else that
/// this high-contrast edge can feel (both thresholds are far below it, and
/// neither looks for diagonals or corners).
#[test]
fn a_longer_search_reconstructs_long_steps_better() {
    let (_, e) = coverage_errors(2.0, &[SmaaQuality::Low, SmaaQuality::Medium]);
    assert!(
        e[1] < e[0] * 0.9,
        "Medium's longer search is closer to the true coverage: {} vs Low's {}",
        e[1],
        e[0]
    );
}

/// Diagonal detection, which High has and Medium does not, is what handles
/// edges near 45 degrees: there each stair-step is one pixel, and only the
/// diagonal search sees them as one line.
#[test]
fn diagonal_detection_reconstructs_near_diagonal_edges() {
    for degrees in [42.0, 48.0] {
        let (_, e) = coverage_errors(degrees, &[SmaaQuality::Medium, SmaaQuality::High]);
        assert!(
            e[1] < e[0] * 0.9,
            "{degrees} deg: High's diagonal search is closer to the true coverage: \
             {} vs Medium's {}",
            e[1],
            e[0]
        );
    }
}

/// Corner detection, which High has and Medium does not, holds back the
/// blend where an edge turns a corner, so the square's corners stay square.
/// At 7 degrees the steps are short enough that both presets' searches find
/// both ends, so the corners are the difference.
#[test]
fn corner_detection_keeps_corners_sharp() {
    let (_, e) = coverage_errors(7.0, &[SmaaQuality::Medium, SmaaQuality::High]);
    assert!(
        e[1] < e[0] * 0.95,
        "High's corner handling is closer to the true coverage: {} vs Medium's {}",
        e[1],
        e[0]
    );
}

/// Renders a two-tone 30-degree edge with no SMAA and with each preset.
fn preset_frames(backdrop: f32, square: f32) -> (Pixels, [Pixels; 4]) {
    let mut h = Harness::new();
    let cube = h.cube();
    let scene = |smaa| Scene {
        smaa,
        ..two_tone_scene(cube, 30.0, backdrop, square)
    };
    let off = h.render(&scene(None));
    assert!(
        off.centre_luma() > off.luma(0, 0) + 5.0,
        "premise: the square visibly differs from the backdrop: {} vs {}",
        off.centre_luma(),
        off.luma(0, 0)
    );
    let presets = [
        SmaaQuality::Low,
        SmaaQuality::Medium,
        SmaaQuality::High,
        SmaaQuality::Ultra,
    ]
    .map(|q| h.render(&scene(quality(q))));
    (off, presets)
}

/// The edge threshold, per preset: 0.8 against 1.0 linear is 0.906 against
/// 1.0 of perceptual luma -- a step of 0.094. Ultra's 0.05 finds it; Low's
/// 0.15 and Medium's and High's 0.1 do not.
#[test]
fn the_threshold_decides_which_faint_edges_are_edges() {
    let (off, [low, medium, high, ultra]) = preset_frames(0.8, 1.0);
    assert!(!low.differs_from(&off), "Low: below 0.15, untouched");
    assert!(!medium.differs_from(&off), "Medium: below 0.1, untouched");
    assert!(!high.differs_from(&off), "High: below 0.1, untouched");
    assert!(ultra.differs_from(&off), "Ultra: above 0.05, smoothed");
}

/// Edges are found by perceptual contrast, as SMAA's thresholds assume: black
/// against 0.02 linear is 0.02 of linear luma (under every preset's
/// threshold) but about 0.155 of perceptual luma (over all of them). The post
/// targets are sRGB and sample as linear light, so this edge is smoothed
/// only if the shader encodes first.
#[test]
fn a_dark_edge_is_found_by_its_perceptual_contrast() {
    let (off, [low, _, high, _]) = preset_frames(0.0, 0.02);
    assert!(low.differs_from(&off), "Low smooths the dark edge");
    assert!(high.differs_from(&off), "High smooths the dark edge");
}

/// FXAA and SMAA are one choice in both reference engines that offer SMAA,
/// so with both components SMAA runs and FXAA does not: the frame is SMAA's
/// alone.
#[test]
fn smaa_replaces_fxaa_when_both_are_on() {
    let mut h = Harness::new();
    let cube = h.cube();
    let smaa_only = h.render(&square(cube, 30.0, Some(Smaa::default())));
    let fxaa_only = h.render(&Scene {
        fxaa: Some(Fxaa::default()),
        ..square(cube, 30.0, None)
    });
    assert!(
        fxaa_only.differs_from(&smaa_only),
        "premise: the two give different frames"
    );
    let both = h.render(&Scene {
        fxaa: Some(Fxaa::default()),
        ..square(cube, 30.0, Some(Smaa::default()))
    });
    assert!(!both.differs_from(&smaa_only), "both on is SMAA alone");
}

/// With TAA on as well, the resolve accumulates SMAA's output rather than
/// the raw composite: the frame differs from TAA alone.
#[test]
fn taa_accumulates_the_smaa_output() {
    let mut h = Harness::new();
    let cube = h.cube();
    let taa_only = {
        let mut s = square(cube, 30.0, None);
        s.taa = Some(bsengine_core::Taa::default());
        h.render_converged(&s, 16)
    };
    let both = {
        let mut s = square(cube, 30.0, Some(Smaa::default()));
        s.taa = Some(bsengine_core::Taa::default());
        h.render_converged(&s, 16)
    };
    assert!(
        both.differs_from(&taa_only),
        "SMAA reaches the frame TAA resolves"
    );
}

/// After the screen-sized targets are made anew, SMAA reads the new frame:
/// it binds the composite's output afresh every frame, rather than keeping a
/// view into a target a resize has replaced. Resized to the same size, so
/// nothing else about the frame changes.
#[test]
fn smaa_reads_the_new_targets_after_a_resize() {
    let mut h = Harness::new();
    let cube = h.cube();
    let before = h.render(&square(cube, 30.0, Some(Smaa::default())));
    let reference = h.render(&square(cube, 45.0, Some(Smaa::default())));
    assert!(
        reference.differs_from(&before),
        "premise: the two frames differ"
    );
    h.render(&square(cube, 30.0, Some(Smaa::default())));
    h.recreate_targets();
    let after = h.render(&square(cube, 45.0, Some(Smaa::default())));
    assert!(
        !after.differs_from(&reference),
        "after the resize, the frame drawn is the frame shown: {}",
        after.describe()
    );
}
