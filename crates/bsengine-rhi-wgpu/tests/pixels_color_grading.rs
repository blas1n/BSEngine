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
