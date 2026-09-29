//! Colour grading, measured at the pixel.
//!
//! The fixture makes every pixel predictable to the 8-bit level: one emissive
//! cube filling the centre of the frame, no light, no ambient, no bloom, no
//! SSAO, and a tonemap of `None` (a clamp). So the composite's output is the
//! emissive colour itself, and the pixel is its sRGB encoding: the chosen
//! emissive displays as `[128, 64, 0]`, half-bright red and quarter-bright
//! green -- a colour that sits on either side of mid-grey, which is what lets
//! contrast show which side of 0.5 it pivots on.

mod common;

use bsengine_core::{AmbientOcclusion, Bloom, ColorGrading, ToneMap, ToneMappingMode};
use common::{Draw, Harness, Light, Scene};
use glam::Vec3;

/// Linear values whose sRGB encodings are 128/255, 64/255 and 0.
const EMISSIVE: Vec3 = Vec3::new(0.215_861, 0.051_269, 0.0);

fn scene(cube: u64, grade: Option<ColorGrading>) -> Scene {
    Scene {
        draws: vec![Draw::new(cube, Vec3::ZERO)
            .colour(Vec3::ZERO)
            .emissive(EMISSIVE)],
        light: Light {
            color: Vec3::ZERO,
            ambient: Vec3::ZERO,
            ..Light::default()
        },
        bloom: Some(Bloom::default().disabled()),
        ssao: Some(AmbientOcclusion::default().disabled()),
        tone_map: Some(ToneMap::new(ToneMappingMode::None)),
        color_grading: grade,
        ..Scene::default()
    }
}

fn grade(contrast: f32, saturation: f32, filter: Vec3) -> ColorGrading {
    ColorGrading {
        enabled: true,
        contrast,
        saturation,
        color_filter: filter.into(),
        ..ColorGrading::default()
    }
}

fn close(a: u8, b: u8, tolerance: u8) -> bool {
    a.abs_diff(b) <= tolerance
}

/// The premise every other test leans on: with no grade the centre pixel is
/// exactly the colour the fixture was built to show. And the identity grade
/// changes it by at most the rounding of one sRGB round trip, while a
/// *disabled* grade -- however extreme its settings -- changes nothing.
#[test]
fn no_grade_and_a_disabled_grade_leave_the_frame_alone() {
    let mut h = Harness::new();
    let cube = h.cube();
    let none = h.render(&scene(cube, None));
    let c = none.centre();
    assert!(
        close(c[0], 128, 1) && close(c[1], 64, 1) && c[2] <= 1,
        "premise: the ungraded centre pixel is the fixture's [128, 64, 0]: {c:?}"
    );

    let identity = h.render(&scene(cube, Some(ColorGrading::default())));
    assert!(
        identity.max_channel_diff(&none) <= 1,
        "the default grade is the identity, give or take one level of sRGB \
         round trip; max diff {}",
        identity.max_channel_diff(&none)
    );

    let off = h.render(&scene(
        cube,
        Some(ColorGrading {
            enabled: false,
            ..grade(3.0, 0.0, Vec3::new(0.0, 1.0, 0.0))
        }),
    ));
    assert!(
        !off.differs_from(&none),
        "a disabled grade must leave the frame bit-for-bit alone"
    );
}

/// Contrast pivots on *display* mid-grey. Red at display 0.502 barely moves
/// under contrast 2, while green at 0.251 is pushed to black. Pivoting on
/// linear 0.5 instead -- grading the value the pass outputs without encoding
/// it first -- would send red (linear 0.216) to black as well.
#[test]
fn contrast_pivots_on_display_mid_grey() {
    let mut h = Harness::new();
    let cube = h.cube();
    let p = h
        .render(&scene(cube, Some(grade(2.0, 1.0, Vec3::ONE))))
        .centre();
    assert!(
        close(p[0], 128, 2),
        "red sits on display mid-grey, so contrast leaves it: {p:?}"
    );
    assert!(
        p[1] <= 2,
        "green is below mid-grey and goes to black: {p:?}"
    );
}

/// Saturation 0 is greyscale at the colour's Rec. 709 luma in display space:
/// 0.2126 * 0.502 + 0.7152 * 0.251 = 0.286, which displays as 73.
#[test]
fn zero_saturation_is_grey_at_the_colours_luma() {
    let mut h = Harness::new();
    let cube = h.cube();
    let p = h
        .render(&scene(cube, Some(grade(1.0, 0.0, Vec3::ONE))))
        .centre();
    assert!(
        close(p[0], p[1], 1) && close(p[1], p[2], 1),
        "saturation 0 must be grey: {p:?}"
    );
    assert!(close(p[0], 73, 2), "at the colour's luma, 73: {p:?}");
}

/// The colour filter multiplies, and it comes *after* contrast. With filter
/// red 0.5 and contrast 2: contrast first leaves red at 0.504 and the filter
/// halves it to 64; filter first would halve red to 0.251 and contrast would
/// then push it to 0. A filter of zero green removes green outright.
#[test]
fn the_colour_filter_multiplies_after_contrast() {
    let mut h = Harness::new();
    let cube = h.cube();
    let p = h
        .render(&scene(
            cube,
            Some(grade(1.0, 1.0, Vec3::new(1.0, 0.0, 1.0))),
        ))
        .centre();
    assert!(
        close(p[0], 128, 1) && p[1] <= 1,
        "a filter with no green removes green and leaves red: {p:?}"
    );

    let p = h
        .render(&scene(
            cube,
            Some(grade(2.0, 1.0, Vec3::new(0.5, 1.0, 1.0))),
        ))
        .centre();
    assert!(
        close(p[0], 64, 3),
        "contrast, then the filter: red 0.504 halved is 64 (filter first would \
         give 0): {p:?}"
    );
}

// --- The colour LUT ---------------------------------------------------------

/// Displays as `[128, 64, 100]`: blue between slices, so a lookup that took
/// only the nearer slice, or read blue off the wrong axis, lands elsewhere.
const EMISSIVE_WITH_BLUE: Vec3 = Vec3::new(0.215_861, 0.051_269, 0.127_438);

/// A LUT strip of `n` slices, each texel the display colour `f` returns for
/// the display colour it stands for, in the layout the component documents:
/// red across a slice, green down it, blue from slice to slice.
fn lut_strip(n: u32, f: impl Fn(Vec3) -> Vec3) -> Vec<u8> {
    let mut rgba = Vec::with_capacity((n * n * n * 4) as usize);
    let step = |i: u32| i as f32 / (n - 1) as f32;
    for g in 0..n {
        for b in 0..n {
            for r in 0..n {
                let out = f(Vec3::new(step(r), step(g), step(b)));
                for c in [out.x, out.y, out.z] {
                    rgba.push((c.clamp(0.0, 1.0) * 255.0).round() as u8);
                }
                rgba.push(255);
            }
        }
    }
    rgba
}

fn with_blue(cube: u64, grade: Option<ColorGrading>) -> Scene {
    let mut s = scene(cube, grade);
    s.draws[0].emissive = EMISSIVE_WITH_BLUE;
    s
}

/// A rotating LUT -- each colour's red, green and blue become its green,
/// blue and red -- turns `[128, 64, 100]` into `[64, 100, 128]`. Every axis
/// has to be read the documented way round for that: a green axis running
/// up instead of down, or blue taken from the nearer slice only, lands on
/// another colour. The identity LUT, alongside, must change nothing: the
/// hard side of "a LUT did something" is that the right LUT did nothing.
#[test]
fn a_lut_maps_each_colour_to_the_colour_its_texel_holds() {
    let mut h = Harness::new();
    let cube = h.cube();
    let ungraded = h.render(&with_blue(cube, None));
    let u = ungraded.centre();
    assert!(
        close(u[0], 128, 1) && close(u[1], 64, 1) && close(u[2], 100, 1),
        "premise: the fixture displays [128, 64, 100]: {u:?}"
    );

    let n = 16;
    h.set_color_lut(n * n, n, &lut_strip(n, |c| c)).unwrap();
    let identity = h.render(&with_blue(cube, Some(ColorGrading::default())));
    assert!(
        identity.max_channel_diff(&ungraded) <= 1,
        "the identity LUT changes nothing: max diff {}",
        identity.max_channel_diff(&ungraded)
    );

    h.set_color_lut(n * n, n, &lut_strip(n, |c| Vec3::new(c.y, c.z, c.x)))
        .unwrap();
    let p = h
        .render(&with_blue(cube, Some(ColorGrading::default())))
        .centre();
    assert!(
        close(p[0], 64, 2) && close(p[1], 100, 2) && close(p[2], 128, 2),
        "the rotating LUT sends [128, 64, 100] to [64, 100, 128]: {p:?}"
    );
}

/// Contribution blends the LUT's colour with the input: at 0.5 the rotated
/// colour and the original meet halfway, `[96, 82, 114]`.
#[test]
fn lut_contribution_blends_toward_the_luts_colour() {
    let mut h = Harness::new();
    let cube = h.cube();
    let n = 16;
    h.set_color_lut(n * n, n, &lut_strip(n, |c| Vec3::new(c.y, c.z, c.x)))
        .unwrap();
    let p = h
        .render(&with_blue(
            cube,
            Some(ColorGrading {
                lut_contribution: 0.5,
                ..ColorGrading::default()
            }),
        ))
        .centre();
    assert!(
        close(p[0], 96, 2) && close(p[1], 82, 2) && close(p[2], 114, 2),
        "halfway between [128, 64, 100] and [64, 100, 128]: {p:?}"
    );
}

/// An image that is not a strip is refused, and nothing is applied: the
/// frame is the one with no LUT at all.
#[test]
fn an_image_that_is_not_a_strip_is_refused() {
    let mut h = Harness::new();
    let cube = h.cube();
    let without = h.render(&with_blue(cube, Some(ColorGrading::default())));
    let err = h
        .set_color_lut(16, 16, &vec![0u8; 16 * 16 * 4])
        .expect_err("16 x 16 is not a strip");
    assert!(err.contains("strip"), "{err}");
    let after = h.render(&with_blue(cube, Some(ColorGrading::default())));
    assert!(
        !after.differs_from(&without),
        "a refused LUT is not applied"
    );
}

/// The LUT comes after the adjustments, as Unity's Color Lookup does. With
/// saturation 0 and the rotating LUT: grey first, then rotated, is still the
/// same grey -- the fixture's luma, 73 on every channel. The other order
/// would rotate first and then take the luma of `[64, 100, 128]`, which is
/// 94.
#[test]
fn the_lut_applies_after_the_adjustments() {
    let mut h = Harness::new();
    let cube = h.cube();
    let n = 16;
    h.set_color_lut(n * n, n, &lut_strip(n, |c| Vec3::new(c.y, c.z, c.x)))
        .unwrap();
    let p = h
        .render(&scene(
            cube,
            Some(ColorGrading {
                saturation: 0.0,
                ..ColorGrading::default()
            }),
        ))
        .centre();
    assert!(
        close(p[0], 73, 2) && close(p[1], 73, 2) && close(p[2], 73, 2),
        "saturation first makes it grey at 73, which the LUT leaves grey: {p:?}"
    );
}
