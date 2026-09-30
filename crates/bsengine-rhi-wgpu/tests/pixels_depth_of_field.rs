//! Depth of field, measured at the pixel.
//!
//! The fixture: a far backdrop 20 units from the camera, split down the
//! middle into red (left) and green (right), and a blue cube 4.5 units away
//! in front of its centre. Everything is emissive and nothing else is lit --
//! no sun, no ambient, no bloom, no SSAO, tonemap `None` -- so each pixel is
//! exactly one surface's colour until the blur mixes them. A sharp edge
//! stays one colour to the pixel; a blurred one mixes the colours either side.

mod common;

use bsengine_core::{AmbientOcclusion, Bloom, DepthOfField, ToneMap, ToneMappingMode};
use common::{Draw, Harness, Light, Pixels, Scene, WIDTH};
use glam::Vec3;

const RED: Vec3 = Vec3::new(0.8, 0.0, 0.0);
const GREEN: Vec3 = Vec3::new(0.0, 0.8, 0.0);
const BLUE: Vec3 = Vec3::new(0.0, 0.0, 0.8);

/// Row near the top of the frame: backdrop only, well above the cube.
const TOP_ROW: u32 = 20;
/// Row through the middle of the frame, across the cube.
const MID_ROW: u32 = 75;

fn scene(cube: u64, dof: Option<DepthOfField>) -> Scene {
    Scene {
        draws: vec![
            Draw::new(cube, Vec3::ZERO)
                .scaled(Vec3::new(40.0, 40.0, 0.2), Vec3::new(-20.0, 0.0, -15.0))
                .colour(Vec3::ZERO)
                .emissive(RED),
            Draw::new(cube, Vec3::ZERO)
                .scaled(Vec3::new(40.0, 40.0, 0.2), Vec3::new(20.0, 0.0, -15.0))
                .colour(Vec3::ZERO)
                .emissive(GREEN),
            Draw::new(cube, Vec3::ZERO)
                .colour(Vec3::ZERO)
                .emissive(BLUE),
        ],
        light: Light {
            color: Vec3::ZERO,
            ambient: Vec3::ZERO,
            ..Light::default()
        },
        bloom: Some(Bloom::default().disabled()),
        ssao: Some(AmbientOcclusion::default().disabled()),
        tone_map: Some(ToneMap::new(ToneMappingMode::None)),
        depth_of_field: dof,
        ..Scene::default()
    }
}

/// Far blur from 10 units, full by 15: the backdrop at 20 is fully blurred,
/// the cube at 4.5 not at all.
fn far_only() -> DepthOfField {
    DepthOfField {
        far_enabled: true,
        near_enabled: false,
        ..DepthOfField::default()
    }
}

/// The column where the backdrop turns from red to green on `row`.
fn edge_column(p: &Pixels, row: u32) -> u32 {
    (1..WIDTH)
        .find(|&x| p.at(x, row)[1] > p.at(x, row)[0])
        .expect("the backdrop has a red/green edge")
}

/// The first and last column of the blue cube on `row`.
fn cube_span(p: &Pixels, row: u32) -> (u32, u32) {
    let blue: Vec<u32> = (0..WIDTH).filter(|&x| p.at(x, row)[2] > 100).collect();
    (
        *blue.first().expect("the cube is in view"),
        *blue.last().unwrap(),
    )
}

/// Absent, disabled, and enabled with neither band on: all three skip the
/// pass, so all three are the same frame to the bit.
#[test]
fn no_dof_a_disabled_one_and_one_with_no_bands_change_nothing() {
    let mut h = Harness::new();
    let cube = h.cube();
    let none = h.render(&scene(cube, None));
    for (what, dof) in [
        (
            "disabled",
            DepthOfField {
                enabled: false,
                ..far_only()
            },
        ),
        (
            "no bands",
            DepthOfField {
                far_enabled: false,
                near_enabled: false,
                ..DepthOfField::default()
            },
        ),
    ] {
        let p = h.render(&scene(cube, Some(dof)));
        assert!(
            !p.differs_from(&none),
            "{what}: the frame must be untouched"
        );
    }
}

/// The far band blurs the backdrop: a pixel a few columns left of the
/// red/green edge, pure red without depth of field, picks up green. A pixel
/// far from any edge stays what it was -- blurring a flat area changes
/// nothing, which is the check that the pass blurs rather than tints.
#[test]
fn the_far_band_blurs_the_backdrop() {
    let mut h = Harness::new();
    let cube = h.cube();
    let sharp = h.render(&scene(cube, None));
    let edge = edge_column(&sharp, TOP_ROW);
    let near_edge = sharp.at(edge - 3, TOP_ROW);
    assert!(
        near_edge[1] <= 1,
        "premise: without depth of field the backdrop edge is sharp: {near_edge:?}"
    );

    let blurred = h.render(&scene(cube, Some(far_only())));
    let p = blurred.at(edge - 3, TOP_ROW);
    assert!(
        p[1] > 30,
        "3 columns from the edge, the blurred backdrop mixes in green: {p:?}"
    );
    let flat = blurred.at(10, TOP_ROW);
    assert!(
        flat[0].abs_diff(sharp.at(10, TOP_ROW)[0]) <= 1 && flat[1] <= 1,
        "far from any edge the backdrop is unchanged: {flat:?}"
    );
}

/// The cube is in focus, so it stays sharp -- the pixel just inside its
/// edge is its own blue -- and, being sharp, it spreads nowhere: the blurred
/// backdrop right beside it carries no blue. A gather that let every nearer
/// sample count would paint a blue halo there.
#[test]
fn a_sharp_subject_stays_sharp_and_leaves_no_halo() {
    let mut h = Harness::new();
    let cube = h.cube();
    let sharp = h.render(&scene(cube, None));
    let (first, last) = cube_span(&sharp, MID_ROW);
    assert!(last - first > 10, "premise: the cube spans several columns");

    let blurred = h.render(&scene(cube, Some(far_only())));
    let inside = blurred.at(first + 1, MID_ROW);
    assert!(
        inside[2].abs_diff(sharp.at(first + 1, MID_ROW)[2]) <= 1 && inside[0] <= 1,
        "just inside its edge the in-focus cube is unchanged: {inside:?}"
    );
    let beside = blurred.at(first - 3, MID_ROW);
    assert!(
        beside[2] <= 3,
        "the blurred backdrop beside a sharp cube takes no blue from it: {beside:?}"
    );
}

/// The near band alone: the cube, nearer than the band's end, blurs -- its
/// edge now mixes with the backdrop -- while the backdrop, beyond the band,
/// keeps its sharp red/green edge.
#[test]
fn the_near_band_blurs_what_is_near_and_nothing_far() {
    let mut h = Harness::new();
    let cube = h.cube();
    let sharp = h.render(&scene(cube, None));
    let (first, _) = cube_span(&sharp, MID_ROW);
    let edge = edge_column(&sharp, TOP_ROW);

    let near = DepthOfField {
        far_enabled: false,
        near_enabled: true,
        near_distance: 6.0,
        near_transition: 2.0,
        ..DepthOfField::default()
    };
    let blurred = h.render(&scene(cube, Some(near)));
    let outside = blurred.at(first - 2, MID_ROW);
    assert!(
        outside[2] > 20,
        "the blurred near cube spreads blue past its edge: {outside:?}"
    );
    let backdrop = blurred.at(edge - 3, TOP_ROW);
    assert!(
        backdrop[1] <= 1,
        "the far backdrop is outside the near band and stays sharp: {backdrop:?}"
    );
}

/// Both bands on: the cube, between them, is in focus but -- because the
/// near band is on -- still gathers, in case a blurred foreground is
/// spreading over it. The blurred backdrop *behind* it must not come along:
/// a neighbour behind a pixel reaches it only as far as that pixel's own
/// blur, which for the sharp cube is nothing. So the pixel just inside the
/// cube's edge is still its own blue, with no red or green bled in.
#[test]
fn a_blurred_background_does_not_spill_over_a_sharp_subject() {
    let mut h = Harness::new();
    let cube = h.cube();
    let sharp = h.render(&scene(cube, None));
    let (first, _) = cube_span(&sharp, MID_ROW);

    let both = DepthOfField {
        far_enabled: true,
        near_enabled: true,
        near_distance: 3.0,
        near_transition: 2.0,
        ..DepthOfField::default()
    };
    let p = h.render(&scene(cube, Some(both))).at(first + 1, MID_ROW);
    let before = sharp.at(first + 1, MID_ROW);
    assert!(
        p[2].abs_diff(before[2]) <= 1 && p[0] <= 1 && p[1] <= 1,
        "the in-focus cube's edge keeps its own colour ({before:?}): {p:?}"
    );
}
