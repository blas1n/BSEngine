//! Per-object motion vectors: the velocity the opaque pass writes, and what
//! TAA and motion blur do with it.
//!
//! The fixture is `pixels_motion_blur.rs`'s: a red backdrop 20 units from the
//! camera and, just in front of it, a green square about 40 pixels across in
//! the middle of the frame, everything emissive and nothing else lit. Here
//! the square moves on its own -- `Draw::moved_from` says where it was the
//! frame before -- while the camera holds still, which is exactly what a
//! camera-only reprojection cannot see.

mod common;

use bsengine_core::{AmbientOcclusion, Bloom, MotionBlur, ToneMap, ToneMappingMode};
use common::{Draw, Harness, Light, Pixels, Scene, HEIGHT, WIDTH};
use glam::{Mat4, Vec3};

const RED: Vec3 = Vec3::new(0.8, 0.0, 0.0);
const GREEN: Vec3 = Vec3::new(0.0, 0.8, 0.0);
const NONE: f32 = 65504.0;

/// The square, centred `x` units right of the middle.
fn square_at(cube: u64, x: f32) -> Draw {
    Draw::new(cube, Vec3::ZERO)
        .scaled(Vec3::new(6.0, 6.0, 0.2), Vec3::new(x, 0.0, -14.7))
        .colour(Vec3::ZERO)
        .emissive(GREEN)
}

fn backdrop(cube: u64) -> Draw {
    Draw::new(cube, Vec3::ZERO)
        .scaled(Vec3::new(80.0, 80.0, 0.2), Vec3::new(0.0, 0.0, -15.0))
        .colour(Vec3::ZERO)
        .emissive(RED)
}

fn scene(draws: Vec<Draw>) -> Scene {
    Scene {
        draws,
        light: Light {
            color: Vec3::ZERO,
            ambient: Vec3::ZERO,
            ..Light::default()
        },
        bloom: Some(Bloom::default().disabled()),
        ssao: Some(AmbientOcclusion::default().disabled()),
        tone_map: Some(ToneMap::new(ToneMappingMode::None)),
        ..Scene::default()
    }
}

/// The square at `x`, having been at `from` the frame before.
fn moved(cube: u64, from: f32, x: f32) -> Draw {
    let previous: Mat4 = square_at(cube, from).transform;
    square_at(cube, x).moved_from(previous)
}

fn at(v: &[[f32; 2]], x: u32, y: u32) -> [f32; 2] {
    v[(y * WIDTH + x) as usize]
}

/// The green square's columns on the middle row, first and last.
fn square_span(p: &Pixels) -> (u32, u32) {
    let y = HEIGHT / 2;
    let green: Vec<u32> = (0..WIDTH).filter(|&x| p.at(x, y)[1] > 100).collect();
    assert!(green.len() > 20, "premise: the square is in view");
    (green[0], green[green.len() - 1])
}

/// Nothing moved: every surface's velocity is zero, and where nothing was
/// drawn at all -- here, around the square, with no backdrop -- the buffer
/// says "no velocity", which hands those pixels to camera reprojection.
#[test]
fn a_still_frame_has_zero_velocity_and_none_where_nothing_drew() {
    let mut h = Harness::new();
    let cube = h.cube();
    let s = scene(vec![square_at(cube, 0.0)]);
    let p = h.render_moving(&s, &s);
    let v = h.velocity();
    let (left, right) = square_span(&p);
    let y = HEIGHT / 2;
    for x in left + 2..right - 1 {
        let [vx, vy] = at(&v, x, y);
        assert!(
            vx.abs() < 1e-4 && vy.abs() < 1e-4,
            "({x}, {y}) on the still square: {vx}, {vy}"
        );
    }
    for x in [2, left - 3, right + 3, WIDTH - 3] {
        assert_eq!(at(&v, x, y), [NONE, NONE], "({x}, {y}) has no surface");
    }
}

/// The square moves right while the camera holds still: its pixels carry
/// its own motion -- as far as it moved on screen, measured from the two
/// frames themselves -- and the still backdrop's carry none.
#[test]
fn a_moving_object_writes_its_own_motion() {
    let mut h = Harness::new();
    let cube = h.cube();
    let before = h.render(&scene(vec![backdrop(cube), square_at(cube, 0.0)]));
    let after = h.render_moving(
        &scene(vec![backdrop(cube), square_at(cube, 0.0)]),
        &scene(vec![backdrop(cube), moved(cube, 0.0, 0.6)]),
    );
    let v = h.velocity();
    let shift = square_span(&after).0 as f32 - square_span(&before).0 as f32;
    assert!(
        shift >= 3.0,
        "premise: the square moved on screen: {shift} px"
    );

    let (left, right) = square_span(&after);
    let y = HEIGHT / 2;
    let [vx, vy] = at(&v, (left + right) / 2, y);
    assert!(
        (vx * WIDTH as f32 - shift).abs() < 1.0 && (vy * HEIGHT as f32).abs() < 0.5,
        "the square's velocity is its move: {} px, {} px against {shift} px measured",
        vx * WIDTH as f32,
        vy * HEIGHT as f32
    );
    let [bx, by] = at(&v, 5, 5);
    assert!(
        bx.abs() < 1e-4 && by.abs() < 1e-4,
        "the still backdrop has none: {bx}, {by}"
    );
}

/// Upward motion is upward velocity: uv's y runs down the screen, so a
/// square rising by some rows has a velocity of minus that many rows.
#[test]
fn a_rising_object_writes_upward_motion() {
    let mut h = Harness::new();
    let cube = h.cube();
    let rise = |from: f32, y: f32| {
        let previous = Mat4::from_scale_rotation_translation(
            Vec3::new(6.0, 6.0, 0.2),
            glam::Quat::IDENTITY,
            Vec3::new(0.0, from, -14.7),
        );
        Draw::new(cube, Vec3::ZERO)
            .scaled(Vec3::new(6.0, 6.0, 0.2), Vec3::new(0.0, y, -14.7))
            .colour(Vec3::ZERO)
            .emissive(GREEN)
            .moved_from(previous)
    };
    let top = |p: &Pixels| {
        (0..HEIGHT)
            .find(|&yy| p.at(WIDTH / 2, yy)[1] > 100)
            .expect("premise: the square is in view") as f32
    };
    let before = h.render(&scene(vec![backdrop(cube), square_at(cube, 0.0)]));
    let after = h.render_moving(
        &scene(vec![backdrop(cube), square_at(cube, 0.0)]),
        &scene(vec![backdrop(cube), rise(0.0, 0.6)]),
    );
    let v = h.velocity();
    let shift = top(&after) - top(&before);
    assert!(shift <= -3.0, "premise: the square rose: {shift} rows");
    let [vx, vy] = at(&v, WIDTH / 2, HEIGHT / 2);
    assert!(
        (vy * HEIGHT as f32 - shift).abs() < 1.0 && (vx * WIDTH as f32).abs() < 0.5,
        "the square's velocity is its rise: {} rows against {shift} measured",
        vy * HEIGHT as f32
    );
}

/// An object that was behind the camera last frame had no screen position
/// then, so it has no velocity now -- not a huge one from dividing by a
/// negative depth -- and TAA and motion blur treat it as camera reprojection
/// does, which gives up on it too.
#[test]
fn an_object_from_behind_the_camera_has_no_velocity() {
    let mut h = Harness::new();
    let cube = h.cube();
    // The camera stands at z = 5 looking down -z; z = 10 is behind it.
    let behind = Mat4::from_scale_rotation_translation(
        Vec3::new(6.0, 6.0, 0.2),
        glam::Quat::IDENTITY,
        Vec3::new(0.0, 0.0, 10.0),
    );
    let p = h.render_moving(
        &scene(vec![backdrop(cube)]),
        &scene(vec![
            backdrop(cube),
            square_at(cube, 0.0).moved_from(behind),
        ]),
    );
    let (left, right) = square_span(&p);
    assert_eq!(
        at(&h.velocity(), (left + right) / 2, HEIGHT / 2),
        [NONE, NONE]
    );
}

/// A translucent pane writes no velocity, as in Unity and Unreal: through a
/// still pane, the pixel keeps the motion of the opaque square moving behind
/// it, which is what TAA sees there.
#[test]
fn a_translucent_pane_keeps_the_motion_behind_it() {
    let mut h = Harness::new();
    let cube = h.cube();
    let pane = || {
        Draw::new(cube, Vec3::ZERO)
            .scaled(Vec3::new(30.0, 30.0, 0.2), Vec3::new(0.0, 0.0, -10.0))
            .colour(Vec3::ZERO)
            .emissive(Vec3::splat(0.8))
            .opacity(0.5)
    };
    let p = h.render_moving(
        &scene(vec![backdrop(cube), square_at(cube, 0.0), pane()]),
        &scene(vec![backdrop(cube), moved(cube, 0.0, 0.6), pane()]),
    );
    let [vx, _] = at(&h.velocity(), WIDTH / 2, HEIGHT / 2);
    assert!(
        p.at(WIDTH / 2, HEIGHT / 2)[0] > 40,
        "premise: the pane is in front of the square"
    );
    assert!(
        vx > 1e-3 && vx < 1000.0,
        "the square's motion shows through the pane: {vx}"
    );
}

/// The camera turns and nothing moves: still surfaces now move on screen,
/// and their velocity says by how much -- the camera's motion is in it too,
/// as it has to be for TAA to use it in place of camera reprojection.
#[test]
fn a_turning_camera_moves_still_surfaces() {
    let mut h = Harness::new();
    let cube = h.cube();
    let look = |x: f32| Scene {
        look_at: Vec3::new(x, 0.0, 0.0),
        ..scene(vec![backdrop(cube), square_at(cube, 0.0)])
    };
    let before = h.render(&look(0.0));
    let after = h.render_moving(&look(0.0), &look(0.5));
    let v = h.velocity();
    let shift = square_span(&after).0 as f32 - square_span(&before).0 as f32;
    assert!(shift <= -3.0, "premise: the square slid left: {shift} px");
    let (left, right) = square_span(&after);
    let [vx, _] = at(&v, (left + right) / 2, HEIGHT / 2);
    assert!(
        (vx * WIDTH as f32 - shift).abs() < 1.0,
        "the still square's velocity is the camera's turn: {} px against {shift} px",
        vx * WIDTH as f32
    );
}

/// A custom shader is the author's own WGSL and has no velocity to write:
/// its pixels say so, and fall back to camera reprojection.
#[test]
fn a_custom_shader_writes_no_velocity() {
    let mut h = Harness::new();
    let cube = h.cube();
    let shader = h.constant_colour_shader([0.0, 0.8, 0.0], "mv_custom");
    let s = scene(vec![backdrop(cube), square_at(cube, 0.0).shader(&shader)]);
    let p = h.render_moving(&s, &s);
    let v = h.velocity();
    let (left, right) = square_span(&p);
    assert_eq!(at(&v, (left + right) / 2, HEIGHT / 2), [NONE, NONE]);
    let [bx, _] = at(&v, 5, 5);
    assert!(bx.abs() < 1e-4, "premise: the backdrop beside it wrote one");
}

/// Terrain writes no velocity either, and it does not inherit one from what
/// it hides: a square moving *behind* a terrain wall leaves the wall's
/// pixels with "no velocity", not the square's motion -- which TAA would
/// otherwise use to drag the wall's history after the hidden square.
#[test]
fn terrain_keeps_no_velocity_over_a_hidden_moving_object() {
    let mut h = Harness::new();
    let cube = h.cube();
    let grey = h.two_colour_texture([120, 120, 120, 255], [120, 120, 120, 255]);
    let weight = h.two_colour_texture([255, 0, 0, 255], [255, 0, 0, 255]);
    let wall = (
        cube,
        Mat4::from_scale_rotation_translation(
            Vec3::new(80.0, 80.0, 0.2),
            glam::Quat::IDENTITY,
            Vec3::new(0.0, 0.0, -14.0),
        ),
        [grey; 4],
        weight,
    );
    let s = |square: Draw| Scene {
        terrain: vec![wall],
        ..scene(vec![square])
    };
    let seen = h.render_moving(
        &scene(vec![square_at(cube, 0.0)]),
        &scene(vec![moved(cube, 0.0, 0.6)]),
    );
    let (left, right) = square_span(&seen);
    let (cx, cy) = ((left + right) / 2, HEIGHT / 2);
    let [vx, _] = h.velocity()[(cy * WIDTH + cx) as usize];
    assert!(
        vx.abs() > 1e-3 && vx < 1000.0,
        "premise: uncovered, the square writes its motion there: {vx}"
    );

    let hidden = h.render_moving(&s(square_at(cube, 0.0)), &s(moved(cube, 0.0, 0.6)));
    assert!(
        (0..WIDTH).all(|x| hidden.at(x, cy)[1] < 100),
        "premise: the wall hides the square"
    );
    assert_eq!(
        h.velocity()[(cy * WIDTH + cx) as usize],
        [NONE, NONE],
        "the wall in front of it has no velocity"
    );
}

/// Under MSAA the velocity is resolved like the colour: still the square's
/// move inside it.
#[test]
fn the_velocity_resolves_under_msaa() {
    let mut h = Harness::new();
    let cube = h.cube();
    let s = |draws| Scene {
        msaa: 4,
        ..scene(draws)
    };
    let before = h.render(&s(vec![backdrop(cube), square_at(cube, 0.0)]));
    let after = h.render_moving(
        &s(vec![backdrop(cube), square_at(cube, 0.0)]),
        &s(vec![backdrop(cube), moved(cube, 0.0, 0.6)]),
    );
    assert_eq!(h.msaa_samples(), 4, "premise: drawn multisampled");
    let v = h.velocity();
    let shift = square_span(&after).0 as f32 - square_span(&before).0 as f32;
    let (left, right) = square_span(&after);
    let [vx, _] = at(&v, (left + right) / 2, HEIGHT / 2);
    assert!(
        (vx * WIDTH as f32 - shift).abs() < 1.0,
        "{} px against {shift} px",
        vx * WIDTH as f32
    );
}

fn mixed(p: [u8; 4]) -> bool {
    p[0] > 12 && p[1] > 12
}

fn longest_mixed_run(pixels: impl Iterator<Item = [u8; 4]>) -> usize {
    let (mut longest, mut run) = (0, 0);
    for p in pixels {
        run = if mixed(p) { run + 1 } else { 0 };
        longest = longest.max(run);
    }
    longest
}

/// With the camera still, a moving square streaks along its motion: its
/// left and right edges blur over about the distance it moved, its top and
/// bottom edges -- running along the motion -- stay hard. The same move
/// with no previous position given, which is all a camera-only blur could
/// see, leaves the frame sharp.
#[test]
fn motion_blur_streaks_a_moving_object_under_a_still_camera() {
    let mut h = Harness::new();
    let cube = h.cube();
    let blur = MotionBlur {
        intensity: 1.0,
        max_blur: 1.0,
        ..MotionBlur::default()
    };
    let with = |draws| Scene {
        motion_blur: Some(blur),
        ..scene(draws)
    };
    let first = with(vec![backdrop(cube), square_at(cube, 0.0)]);
    let unseen = h.render_moving(&first, &with(vec![backdrop(cube), square_at(cube, 2.0)]));
    let streaked = h.render_moving(&first, &with(vec![backdrop(cube), moved(cube, 0.0, 2.0)]));

    let y = HEIGHT / 2;
    let row = |p: &Pixels| longest_mixed_run((0..WIDTH).map(|x| p.at(x, y)));
    assert!(
        row(&unseen) <= 1,
        "premise: without a previous position nothing streaks: {}",
        row(&unseen)
    );
    assert!(
        row(&streaked) >= 3,
        "the moving square's left and right edges streak: {}",
        row(&streaked)
    );
    let (left, right) = square_span(&unseen);
    let column = |p: &Pixels| longest_mixed_run((0..HEIGHT).map(|yy| p.at((left + right) / 2, yy)));
    assert!(
        column(&streaked) <= 1,
        "its top and bottom edges run along the motion and stay hard: {}",
        column(&streaked)
    );
    for (x, yy) in [(5, 5), (WIDTH - 5, HEIGHT - 5)] {
        assert_eq!(
            streaked.at(x, yy),
            unseen.at(x, yy),
            "the still backdrop far from it is untouched"
        );
    }
}

/// TAA follows a moving object to its own history. The square slides right
/// over sixteen frames; the converged frame is compared with the square
/// standing still where it ended, converged as long. Reprojected by the
/// camera alone, the history at its leading edge is backdrop and at its
/// trailing edge square, and the clamp only partly hides that.
#[test]
fn taa_follows_a_moving_object_to_its_own_history() {
    let mut h = Harness::new();
    let cube = h.cube();
    let taa = Some(bsengine_core::Taa::default());
    let step = 0.05;
    let frames: Vec<Scene> = (0..16)
        .map(|i| {
            let x = i as f32 * step;
            let d = if i == 0 {
                square_at(cube, x)
            } else {
                moved(cube, x - step, x)
            };
            Scene {
                taa,
                ..scene(vec![backdrop(cube), d])
            }
        })
        .collect();
    let unseen: Vec<Scene> = (0..16)
        .map(|i| Scene {
            taa,
            ..scene(vec![backdrop(cube), square_at(cube, i as f32 * step)])
        })
        .collect();
    let end = 15.0 * step;
    let still = h.render_converged(
        &Scene {
            taa,
            ..scene(vec![backdrop(cube), square_at(cube, end)])
        },
        16,
    );
    let moving = h.render_sequence(&frames);
    let camera_only = h.render_sequence(&unseen);
    let err = |p: &Pixels| -> f32 {
        (0..HEIGHT)
            .flat_map(|y| (0..WIDTH).map(move |x| (x, y)))
            .map(|(x, y)| (p.luma(x, y) - still.luma(x, y)).abs())
            .sum()
    };
    let (e_moving, e_camera) = (err(&moving), err(&camera_only));
    eprintln!("taa error against the still square: velocity {e_moving}, camera only {e_camera}");
    assert!(
        e_camera > 1000.0,
        "premise: reprojected by the camera alone, the moving square visibly \
         trails: {e_camera}"
    );
    assert!(
        e_moving < e_camera * 0.6,
        "with its velocity, the moving square converges much as a still one \
         does: error {e_moving}, camera-only {e_camera}"
    );
}
