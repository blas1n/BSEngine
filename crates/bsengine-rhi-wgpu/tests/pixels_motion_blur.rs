//! Camera motion blur, measured at the pixel.
//!
//! The fixture: a red backdrop 20 units from the camera and, just in front of
//! it, a green square about 40 pixels across in the middle of the frame.
//! Everything is emissive and nothing else is lit -- no sun, no ambient, no
//! bloom, no SSAO, tonemap `None` -- so each pixel is exactly one surface's
//! colour until the blur mixes them. The camera turns by shifting what it
//! looks at sideways between two frames: the scene slides horizontally across
//! the screen, so the square's left and right edges streak and its top and
//! bottom edges, running along the motion, do not.

mod common;

use bsengine_core::{AmbientOcclusion, Bloom, MotionBlur, ToneMap, ToneMappingMode};
use common::{Draw, Harness, Light, Pixels, Scene, HEIGHT, WIDTH};
use glam::Vec3;

const RED: Vec3 = Vec3::new(0.8, 0.0, 0.0);
const GREEN: Vec3 = Vec3::new(0.0, 0.8, 0.0);

/// Row through the middle of the square.
const MID_ROW: u32 = HEIGHT / 2;

/// How far sideways the camera's target moves between the two frames. From 5
/// units away that is a turn of about 5.7 degrees: roughly 13 pixels across
/// this frame.
const TURN: f32 = 0.5;

fn scene(cube: u64, look_x: f32, blur: Option<MotionBlur>) -> Scene {
    Scene {
        draws: vec![
            Draw::new(cube, Vec3::ZERO)
                .scaled(Vec3::new(80.0, 80.0, 0.2), Vec3::new(0.0, 0.0, -15.0))
                .colour(Vec3::ZERO)
                .emissive(RED),
            Draw::new(cube, Vec3::ZERO)
                .scaled(Vec3::new(6.0, 6.0, 0.2), Vec3::new(0.0, 0.0, -14.7))
                .colour(Vec3::ZERO)
                .emissive(GREEN),
        ],
        light: Light {
            color: Vec3::ZERO,
            ambient: Vec3::ZERO,
            ..Light::default()
        },
        look_at: Vec3::new(look_x, 0.0, 0.0),
        bloom: Some(Bloom::default().disabled()),
        ssao: Some(AmbientOcclusion::default().disabled()),
        tone_map: Some(ToneMap::new(ToneMappingMode::None)),
        motion_blur: blur,
        ..Scene::default()
    }
}

/// The whole frame's motion, unclamped: every streak is as long as the
/// distance the scene slid.
fn full() -> MotionBlur {
    MotionBlur {
        intensity: 1.0,
        max_blur: 1.0,
        ..MotionBlur::default()
    }
}

/// A pixel that is neither the backdrop's red nor the square's green but a
/// mix of both -- what a blur leaves where it crosses an edge.
fn mixed(p: [u8; 4]) -> bool {
    p[0] > 12 && p[1] > 12
}

/// Mixed pixels along row `y`.
fn mixed_in_row(p: &Pixels, y: u32) -> usize {
    (0..WIDTH).filter(|&x| mixed(p.at(x, y))).count()
}

/// The longest run of consecutive mixed pixels among `pixels` -- how wide
/// the widest blurred edge is. A streak mixes a run as long as itself; the
/// bilinear filter reading a fraction of a pixel off an edge mixes only the
/// pixel on either side of it.
fn longest_mixed_run(pixels: impl Iterator<Item = [u8; 4]>) -> usize {
    let (mut longest, mut run) = (0, 0);
    for p in pixels {
        run = if mixed(p) { run + 1 } else { 0 };
        longest = longest.max(run);
    }
    longest
}

/// The column through the middle of the square on `row`.
fn square_centre(p: &Pixels, row: u32) -> u32 {
    let green: Vec<u32> = (0..WIDTH).filter(|&x| p.at(x, row)[1] > 100).collect();
    assert!(
        green.len() > 20,
        "premise: the square is in view on row {row}"
    );
    (green[0] + green[green.len() - 1]) / 2
}

/// Absent, disabled and zero-intensity blur all skip the pass: with the
/// camera turning, all three are the frame without blur to the bit. And an
/// enabled blur on a camera that did not move changes nothing either -- the
/// side a blur that always smears would fail.
#[test]
fn no_blur_a_disabled_one_and_a_still_camera_change_nothing() {
    let mut h = Harness::new();
    let cube = h.cube();
    let none = h.render_moving(&scene(cube, 0.0, None), &scene(cube, TURN, None));
    for (what, blur) in [
        (
            "disabled",
            MotionBlur {
                enabled: false,
                ..full()
            },
        ),
        (
            "zero intensity",
            MotionBlur {
                intensity: 0.0,
                ..full()
            },
        ),
    ] {
        let p = h.render_moving(
            &scene(cube, 0.0, Some(blur)),
            &scene(cube, TURN, Some(blur)),
        );
        assert!(
            !p.differs_from(&none),
            "{what}: the frame must be untouched"
        );
    }

    let still_sharp = h.render(&scene(cube, TURN, None));
    let still = h.render_moving(
        &scene(cube, TURN, Some(full())),
        &scene(cube, TURN, Some(full())),
    );
    assert!(
        !still.differs_from(&still_sharp),
        "a camera that did not move is not blurred"
    );
}

/// The turn streaks the square's left and right edges across the row -- a
/// dozen pixels of red/green mix where the sharp frame has at most a pixel
/// or two of edge -- while down the column through its middle, the top and
/// bottom edges run along the motion and stay sharp. Blur in every direction
/// would smear those as far as the others.
///
/// "Sharp" is measured as the widest blurred edge, not a count of touched
/// pixels: a yaw seen through a perspective lens also moves what is above
/// and below the horizon a fraction of a pixel vertically (about 0.1 here),
/// and that is enough for the bilinear read to take ~10% of the row beyond
/// the edge -- one pixel either side, which the reprojection is right to do.
#[test]
fn a_turning_camera_streaks_along_the_motion_only() {
    let mut h = Harness::new();
    let cube = h.cube();
    let sharp = h.render_moving(&scene(cube, 0.0, None), &scene(cube, TURN, None));
    let sharp_row = mixed_in_row(&sharp, MID_ROW);
    assert!(
        sharp_row <= 4,
        "premise: without blur the square's edges are sharp ({sharp_row} mixed)"
    );
    let centre = square_centre(&sharp, MID_ROW);

    let blurred = h.render_moving(
        &scene(cube, 0.0, Some(full())),
        &scene(cube, TURN, Some(full())),
    );
    let row = mixed_in_row(&blurred, MID_ROW);
    assert!(
        row >= sharp_row + 16,
        "the left and right edges streak: {row} mixed pixels on the row, {sharp_row} without blur"
    );
    let row_run = longest_mixed_run((0..WIDTH).map(|x| blurred.at(x, MID_ROW)));
    assert!(
        row_run >= 8,
        "each streaked edge is a long run of mix: the widest is {row_run} pixels"
    );
    let column_run = longest_mixed_run((0..HEIGHT).map(|y| blurred.at(centre, y)));
    assert!(
        column_run <= 2,
        "the top and bottom edges run along the motion and stay sharp: \
         the widest blurred edge down the column is {column_run} pixels"
    );
}

/// The first frame after a cut has no previous camera to measure against,
/// so it is sharp -- even though the frame drawn before the cut had the
/// camera somewhere else, the pair `render_moving` shows does streak.
#[test]
fn the_first_frame_after_a_cut_is_sharp() {
    let mut h = Harness::new();
    let cube = h.cube();
    let moving = h.render_moving(
        &scene(cube, 0.0, Some(full())),
        &scene(cube, TURN, Some(full())),
    );
    assert!(
        mixed_in_row(&moving, MID_ROW) > 10,
        "premise: this camera pair streaks when the second frame continues the first"
    );

    // The surface still holds the TURN camera as its previous one; a cut and
    // a frame from the other pose must not measure against it.
    let after_cut = h.render(&scene(cube, 0.0, Some(full())));
    let sharp = h.render(&scene(cube, 0.0, None));
    assert!(
        !after_cut.differs_from(&sharp),
        "the first frame after a cut is not blurred"
    );
}

/// `max_blur` caps a streak's length and `intensity` scales it. The turn
/// slides the scene about 13 pixels. Capped at 2% of the 200-pixel width,
/// each edge smears over at most about 4 pixels; half the intensity smears
/// over about half the full streak.
#[test]
fn max_blur_clamps_the_streak_and_intensity_scales_it() {
    let mut h = Harness::new();
    let cube = h.cube();
    let streak = |h: &mut Harness, blur: MotionBlur| {
        let p = h.render_moving(
            &scene(cube, 0.0, Some(blur)),
            &scene(cube, TURN, Some(blur)),
        );
        mixed_in_row(&p, MID_ROW)
    };
    let whole = streak(&mut h, full());
    let clamped = streak(
        &mut h,
        MotionBlur {
            max_blur: 0.02,
            ..full()
        },
    );
    let half = streak(
        &mut h,
        MotionBlur {
            intensity: 0.5,
            ..full()
        },
    );
    assert!(
        whole >= 20,
        "premise: unclamped, both edges streak a long way ({whole} mixed)"
    );
    assert!(
        (2..=10).contains(&clamped),
        "capped at 4 pixels a streak, both edges together mix at most ~10 pixels: {clamped} (unclamped {whole})"
    );
    assert!(
        half > clamped && half * 10 <= whole * 7,
        "half the intensity is a clearly shorter streak than the whole: {half} vs {whole}"
    );
}

/// Depth of field and motion blur on together: both passes run, each writing
/// the full-screen target the other did not, and what bloom and the
/// composite then read has to be the second one's output. The frame must
/// carry both effects -- it differs from either one alone. Reading the wrong
/// target would hand the composite the depth-of-field image without the
/// streak (or the fogged image with neither), identical to one of the two.
#[test]
fn depth_of_field_and_motion_blur_both_reach_the_frame() {
    use bsengine_core::DepthOfField;
    let mut h = Harness::new();
    let cube = h.cube();
    let dof = DepthOfField {
        far_enabled: true,
        near_enabled: false,
        ..DepthOfField::default()
    };
    let with = |blur: Option<MotionBlur>, dof: Option<DepthOfField>| {
        let mut from = scene(cube, 0.0, blur);
        from.depth_of_field = dof;
        let mut to = scene(cube, TURN, blur);
        to.depth_of_field = dof;
        (from, to)
    };
    let (f, t) = with(None, Some(dof));
    let dof_only = h.render_moving(&f, &t);
    let (f, t) = with(Some(full()), None);
    let blur_only = h.render_moving(&f, &t);
    let (f, t) = with(Some(full()), Some(dof));
    let both = h.render_moving(&f, &t);
    assert!(
        dof_only.differs_from(&blur_only),
        "premise: the two effects alone give different frames"
    );
    assert!(both.differs_from(&dof_only), "the streak is in the frame");
    assert!(both.differs_from(&blur_only), "the defocus is in the frame");
    assert!(
        longest_mixed_run((0..WIDTH).map(|x| both.at(x, MID_ROW))) >= 8,
        "and the edges still streak"
    );
}
