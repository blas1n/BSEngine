//! Screen-space reflections.
//!
//! What SSR can do and what it cannot are the same fact seen twice: it
//! reflects what is already on screen, and nothing else. Every test here is
//! either "the reflection arrived" or "the ray had no answer and said so" —
//! the second kind is what keeps the first honest, because a pass that simply
//! brightened reflective surfaces would satisfy the first alone.

mod common;

use bsengine_core::ScreenSpaceReflections;
use common::{Draw, Harness, Light, Scene};
use glam::Vec3;

/// A wide floor at the origin, its top face at y = 0.5.
fn floor(mesh: u64) -> Draw {
    Draw::new(mesh, Vec3::ZERO)
        .scaled(Vec3::new(20.0, 1.0, 20.0), Vec3::ZERO)
        // Smooth and dark: a mirror shows what is reflected in it rather than
        // its own colour, and a rough surface is faded out by design.
        .roughness(0.02)
        .colour(Vec3::splat(0.02))
}

/// Something bright standing on the floor, for it to reflect.
fn pillar(mesh: u64) -> Draw {
    Draw::new(mesh, Vec3::new(0.0, 1.5, 0.0))
        .scaled(Vec3::new(1.0, 2.0, 1.0), Vec3::new(0.0, 1.5, 0.0))
        .emissive(Vec3::new(4.0, 0.4, 0.4))
}

/// Looking across the floor at a shallow angle, which is where a reflection
/// off it lands in front of the camera rather than behind.
const CAMERA: Vec3 = Vec3::new(0.0, 1.2, 6.0);

fn scene(mesh: u64, ssr: Option<ScreenSpaceReflections>) -> Scene {
    Scene {
        draws: vec![floor(mesh), pillar(mesh)],
        camera_pos: CAMERA,
        look_at: Vec3::new(0.0, 0.6, 0.0),
        ssr,
        light: Light {
            ambient: Vec3::splat(0.02),
            ..Light::default()
        },
        ..Default::default()
    }
}

/// `scene` as its second frame: the first leaves the lit scene the
/// reflections read their hits from, which is the previous frame's -- the
/// first frame with reflections on has none to read, and reflects nothing.
/// Every frame with reflections on is measured as a second one, or the
/// "reflects nothing" tests below would pass on the first frame's emptiness.
fn settled(h: &mut Harness, scene: &Scene) -> common::Pixels {
    h.render(scene);
    h.render(scene)
}

/// The floor in front of the pillar, where its reflection should appear.
///
/// Computed from the frame rather than written down: the reflection lands
/// below the pillar's base, and where that is on screen depends on the camera
/// this file happens to use.
fn reflection_row(height: u32) -> u32 {
    (height as f32 * 0.72) as u32
}

#[test]
fn a_mirror_floor_reflects_what_stands_on_it() {
    let mut h = Harness::new();
    let cube = h.cube();

    let without = h.render(&scene(cube, None));
    let with = settled(
        &mut h,
        &scene(cube, Some(ScreenSpaceReflections::default())),
    );

    let y = reflection_row(without.height);
    let x = with.width / 2;
    let (before, after) = (without.at(x, y), with.at(x, y));
    assert!(
        after[0] > before[0] + 4,
        "the floor in front of a red pillar should pick up its reflection: \
         {before:?} -> {after:?} at ({x}, {y})"
    );
}

#[test]
fn switching_it_off_is_the_same_as_not_having_it() {
    // The property that keeps every scene authored before this renders as it
    // did, and the one a `enabled: false` that still traced would break.
    let mut h = Harness::new();
    let cube = h.cube();

    let absent = h.render(&scene(cube, None));
    let disabled = settled(
        &mut h,
        &scene(
            cube,
            Some(ScreenSpaceReflections {
                enabled: false,
                ..Default::default()
            }),
        ),
    );

    assert!(
        !disabled.differs_from(&absent),
        "a disabled component must be indistinguishable from no component: {}",
        disabled.describe()
    );
}

#[test]
fn a_rough_floor_does_not_reflect() {
    // A single traced ray is the wrong answer for a rough surface, so all three
    // reference engines fade the effect out with roughness. Without this the
    // pass would put a mirror-sharp reflection on concrete.
    let mut h = Harness::new();
    let cube = h.cube();
    let rough = Scene {
        draws: vec![floor(cube).roughness(0.9), pillar(cube)],
        ..scene(cube, Some(ScreenSpaceReflections::default()))
    };
    let rough_off = Scene {
        ssr: None,
        ..Scene {
            draws: vec![floor(cube).roughness(0.9), pillar(cube)],
            ..scene(cube, None)
        }
    };

    let with = settled(&mut h, &rough);
    let without = h.render(&rough_off);
    assert!(
        with.max_channel_diff(&without) <= 2,
        "roughness 0.9 is well past the default cut-off, so nothing should be \
         added: {}",
        with.describe()
    );
}

#[test]
fn a_reflection_with_nothing_to_hit_adds_nothing() {
    // A floor alone. Every ray leaves the frame without finding a surface, and
    // the pass has no answer -- which has to mean "leave it alone", not "sample
    // whatever was at the edge".
    let mut h = Harness::new();
    let cube = h.cube();
    let empty = |ssr| Scene {
        draws: vec![floor(cube)],
        ..scene(cube, ssr)
    };

    let without = h.render(&empty(None));
    let with = settled(&mut h, &empty(Some(ScreenSpaceReflections::default())));

    assert!(
        with.max_channel_diff(&without) <= 2,
        "with nothing above the floor there is nothing to reflect: {}",
        with.describe()
    );
}

#[test]
fn thickness_decides_whether_passing_behind_something_counts_as_hitting_it() {
    // The depth buffer says where surfaces start and never where they end, so
    // a ray that has gone behind one has to decide: did it hit, or pass by?
    // `thickness` is that decision, and without an upper bound every ray that
    // goes behind anything reports a hit on it.
    //
    // Measured by widening the window until it swallows the scene. If the
    // default already behaved that way -- which is what deleting the bound
    // does -- these two frames would be identical.
    let mut h = Harness::new();
    let cube = h.cube();

    // A long stride on purpose: the ray steps *over* the pillar and lands well
    // behind it, which is the only situation where the two answers differ. With
    // short steps every sample that goes behind is already inside the window.
    let coarse = ScreenSpaceReflections {
        steps: 8,
        stride: 3.0,
        ..Default::default()
    };
    let normal = settled(&mut h, &scene(cube, Some(coarse)));
    let swallowed = settled(
        &mut h,
        &scene(
            cube,
            Some(ScreenSpaceReflections {
                // NDC depth spans 0..1, so this accepts a hit anywhere behind.
                thickness: 10.0,
                ..coarse
            }),
        ),
    );

    assert!(
        swallowed.differs_from(&normal),
        "a thickness that accepts any surface behind the ray has to find hits          the default rejects: {}",
        swallowed.describe()
    );
}

#[test]
fn a_ray_that_runs_out_of_steps_finds_nothing() {
    // Paired with the first test: the same scene, the same surface, but a reach
    // of one short step. This is what separates "the trace found the pillar"
    // from "the pass brightened a smooth surface".
    let mut h = Harness::new();
    let cube = h.cube();

    let without = h.render(&scene(cube, None));
    let short = settled(
        &mut h,
        &scene(
            cube,
            Some(ScreenSpaceReflections {
                steps: 1,
                stride: 0.01,
                ..Default::default()
            }),
        ),
    );

    assert!(
        short.max_channel_diff(&without) <= 2,
        "a ray reaching one centimetre cannot have found the pillar: {}",
        short.describe()
    );
}

// ---- composed with the environment ------------------------------------------

/// Measured in light, not in levels: no bloom (the glow of the bright mirror
/// and pillars around a pixel would spread onto it), no tone mapping (whose
/// shoulder flattens the very differences these tests read).
fn plain(scene: Scene) -> Scene {
    Scene {
        bloom: Some(bsengine_core::Bloom::default().disabled()),
        tone_map: Some(bsengine_core::ToneMap::new(
            bsengine_core::ToneMappingMode::None,
        )),
        ..scene
    }
}

/// An 8-bit sRGB level back to linear light.
fn linear(c: u8) -> f32 {
    let c = c as f32 / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// The floor as a metal mirror of `colour`: what it reflects is all it shows,
/// through its own `F0`. The plane, at the cube floor's height: the cube's
/// faces carry a tint each, which would be the metal's colour as well.
fn metal_floor(plane: u64, colour: Vec3) -> Draw {
    Draw::new(plane, Vec3::ZERO)
        .scaled(Vec3::new(20.0, 1.0, 20.0), Vec3::new(0.0, 0.5, 0.0))
        .roughness(0.02)
        .colour(colour)
        .metallic(1.0)
}

/// A pillar glowing `glow`, at `x`.
fn pillar_at(mesh: u64, x: f32, glow: Vec3) -> Draw {
    Draw::new(mesh, Vec3::new(x, 1.5, 0.0))
        .scaled(Vec3::new(1.0, 2.0, 1.0), Vec3::new(x, 1.5, 0.0))
        .colour(Vec3::ZERO)
        .emissive(glow)
}

/// The reflection's pixel, below the pillar as [`reflection_row`] has it, in
/// linear light.
fn reflection_at(p: &common::Pixels) -> Vec3 {
    let c = p.at(p.width / 2, reflection_row(p.height));
    Vec3::new(linear(c[0]), linear(c[1]), linear(c[2]))
}

/// Where the screen finds what a surface reflects, that replaces the
/// environment's reflection there -- it does not add to it. A white metal
/// mirror under a blue sky reflects the sky; in front of the red pillar,
/// with reflections on, the share `w` the screen found is the pillar's, and
/// the sky keeps only `1 - w` of its blue. `w` is at least the red's share
/// of the pillar's own: the red comes through the mirror's `F * brdf`, which
/// is under 1 (0.38 at this grazing angle, measured), and the sky's blue
/// went through the same. Added on top (as the reflection pass once was),
/// the blue would stay whole whatever the red.
#[test]
fn a_reflection_replaces_the_environment_it_found_something_in_front_of() {
    let mut h = Harness::new();
    let (cube, plane) = (h.cube(), h.plane());
    h.set_test_skybox([0, 0, 160, 255]);
    let glow = Vec3::new(0.8, 0.0, 0.0);
    let room = |ssr| {
        plain(Scene {
            draws: vec![metal_floor(plane, Vec3::ONE), pillar_at(cube, 0.0, glow)],
            with_skybox: true,
            ..scene(cube, ssr)
        })
    };
    let without = reflection_at(&h.render(&room(None)));
    let with = reflection_at(&settled(
        &mut h,
        &room(Some(ScreenSpaceReflections::default())),
    ));
    let red_share = with.x / glow.x;
    assert!(
        without.z > 0.1 && red_share > 0.1,
        "premise: the sky is reflected there, and then the pillar is: {without} -> {with}"
    );
    let kept = with.z / without.z;
    assert!(
        kept < 0.6 && kept <= 1.0 - red_share + 0.03,
        "the sky keeps at most {} of its blue there, and it kept {kept}: {without} -> {with}",
        1.0 - red_share
    );
}

/// The reflection goes through the surface's own reflectance, as the
/// environment's does: what reflections add to a gold mirror in front of the
/// white pillar is gold -- red over green over blue, blue well under red. At
/// this grazing angle Fresnel takes every metal some way toward white
/// (`F0` (1, 0.8, 0.3) reads about (1, 0.86, 0.5) here), so not in the
/// proportion of `F0` itself; added unweighted (as the pass once was), the
/// three would be equal.
#[test]
fn a_metal_reflects_what_it_finds_in_its_own_colour() {
    let mut h = Harness::new();
    let (cube, plane) = (h.cube(), h.plane());
    let gold = Vec3::new(1.0, 0.8, 0.3);
    let room = |ssr| {
        plain(Scene {
            draws: vec![
                metal_floor(plane, gold),
                pillar_at(cube, 0.0, Vec3::splat(0.8)),
            ],
            ..scene(cube, ssr)
        })
    };
    let without = reflection_at(&h.render(&room(None)));
    let with = reflection_at(&settled(
        &mut h,
        &room(Some(ScreenSpaceReflections::default())),
    ));
    let added = with - without;
    assert!(
        added.x > 0.05,
        "premise: the pillar is reflected: {without} -> {with}"
    );
    assert!(
        added.x > added.y && added.y > added.z && added.z < 0.85 * added.x,
        "gold, not white: reflections added {added}"
    );
}

/// A hit's light is last frame's: the pillar turns from red to green, and
/// the frame it turns still reflects it red -- the reflection's colour comes
/// from the scene of the frame before -- and the next one green.
#[test]
fn a_reflection_shows_its_hit_as_it_was_last_frame() {
    let mut h = Harness::new();
    let (cube, plane) = (h.cube(), h.plane());
    let room = |glow| {
        plain(Scene {
            draws: vec![metal_floor(plane, Vec3::ONE), pillar_at(cube, 0.0, glow)],
            ..scene(cube, Some(ScreenSpaceReflections::default()))
        })
    };
    let (red, green) = (Vec3::new(0.8, 0.0, 0.0), Vec3::new(0.0, 0.8, 0.0));
    let before = reflection_at(&settled(&mut h, &room(red)));
    let turning = reflection_at(&h.render(&room(green)));
    let after = reflection_at(&h.render(&room(green)));
    assert!(
        before.x > 0.1 && before.y < 0.25 * before.x,
        "premise: red reflected: {before}"
    );
    assert!(
        turning.x > 0.1 && turning.y < 0.25 * turning.x,
        "the frame it turns green, still red: {turning}"
    );
    assert!(
        after.y > 0.1 && after.x < 0.25 * after.y,
        "and green the frame after: {after}"
    );
}

/// A hit is found in last frame's scene where last frame's camera saw it,
/// not at the pixel it is at now: the camera slides sideways between two
/// frames, and the reflection of the red and the green pillar side by side
/// turns from red to green where two frames from where it stopped have it.
/// Read from last frame's scene at this frame's pixels, the edge would sit
/// where the camera left it, some ten pixels over.
#[test]
fn a_hit_is_found_where_the_last_frame_saw_it() {
    let mut h = Harness::new();
    let (cube, plane) = (h.cube(), h.plane());
    let room = |dx: f32| {
        plain(Scene {
            draws: vec![
                metal_floor(plane, Vec3::ONE),
                pillar_at(cube, -0.5, Vec3::new(0.8, 0.0, 0.0)),
                pillar_at(cube, 0.5, Vec3::new(0.0, 0.8, 0.0)),
            ],
            camera_pos: CAMERA + Vec3::new(dx, 0.0, 0.0),
            look_at: Vec3::new(dx, 0.6, 0.0),
            ..scene(cube, Some(ScreenSpaceReflections::default()))
        })
    };
    // Slid: one frame from 0.4 to the left, then the frame at 0.
    h.render(&room(-0.4));
    let slid = h.render(&room(0.0));
    let still = settled(&mut h, &room(0.0));
    let y = reflection_row(still.height);
    // Where the reflection turns from red to green along the row.
    let edge = |p: &common::Pixels| {
        (p.width / 4..p.width * 3 / 4).find(|&x| {
            let c = p.at(x, y);
            linear(c[1]) > linear(c[0]) && linear(c[1]) > 0.02
        })
    };
    let (Some(moved), Some(stood)) = (edge(&slid), edge(&still)) else {
        panic!("premise: both pillars are reflected along the row");
    };
    assert!(
        moved.abs_diff(stood) <= 2,
        "after the slide the reflection's red-to-green edge is where standing \
         still puts it: column {moved} against {stood}"
    );
}

/// The first frame with reflections has no last frame to find its hits in,
/// and keeps the environment's reflection whole: the mirror under the blue
/// sky in front of the pillar is what it is with reflections off. Taken from
/// an empty history, the hit would be black -- and replace the sky's blue
/// with nothing for a frame.
#[test]
fn the_first_frame_keeps_the_environment() {
    let mut h = Harness::new();
    let (cube, plane) = (h.cube(), h.plane());
    h.set_test_skybox([0, 0, 160, 255]);
    let room = |ssr| {
        plain(Scene {
            draws: vec![
                metal_floor(plane, Vec3::ONE),
                pillar_at(cube, 0.0, Vec3::new(0.8, 0.0, 0.0)),
            ],
            with_skybox: true,
            ..scene(cube, ssr)
        })
    };
    let without = reflection_at(&h.render(&room(None)));
    let first = reflection_at(&h.render(&room(Some(ScreenSpaceReflections::default()))));
    let second = reflection_at(&h.render(&room(Some(ScreenSpaceReflections::default()))));
    assert!(
        second.z < 0.6 * without.z,
        "premise: from the second frame the pillar replaces the sky there: {without} -> {second}"
    );
    assert!(
        (first - without).abs().max_element() < 0.01,
        "the first frame, the sky's reflection as it was: {without} -> {first}"
    );
}

/// A transparent surface takes no reflections from the screen: the trace is
/// of the opaque surface behind it, at its pixel, and is not what the pane
/// in front reflects. A half-transparent metal pane low in front of the
/// camera -- over the floor where the red pillar is reflected, clear of the
/// pillar itself -- shows that reflection through it at half, and a pane
/// taking the floor's reflection for its own would add the red again.
///
/// ⚠️ Clear of the pillar: a pane over the pillar dims the pillar in the
/// scene the reflections read their hits from, and halves the reflection
/// with no pane over it at all -- the first version of this test measured
/// that, and passed with the guard deleted.
#[test]
fn a_transparent_surface_takes_no_reflection_from_the_screen() {
    let mut h = Harness::new();
    let (cube, plane) = (h.cube(), h.plane());
    let pane = || {
        let mut d = Draw::new(plane, Vec3::ZERO)
            .colour(Vec3::ONE)
            .metallic(1.0)
            .roughness(0.02)
            .opacity(0.5);
        // Upright, facing the camera, just in front of it and below its eye.
        d.transform = glam::Mat4::from_translation(Vec3::new(0.0, PANE_Y, PANE_Z))
            * glam::Mat4::from_rotation_x(std::f32::consts::FRAC_PI_2)
            * glam::Mat4::from_scale(Vec3::new(2.0, 1.0, PANE_H));
        d
    };
    let room = |with_pane: bool| {
        let mut draws = vec![
            metal_floor(plane, Vec3::ONE),
            pillar_at(cube, 0.0, Vec3::new(0.8, 0.0, 0.0)),
        ];
        if with_pane {
            draws.push(pane());
        }
        plain(Scene {
            draws,
            ..scene(cube, Some(ScreenSpaceReflections::default()))
        })
    };
    let bare = settled(&mut h, &room(false));
    let behind = settled(&mut h, &room(true));
    // The pillar itself, straight ahead at the frame's centre.
    let pillar = |p: &common::Pixels| linear(p.at(p.width / 2, p.height / 2)[0]);
    assert!(
        (pillar(&bare) - pillar(&behind)).abs() < 0.01,
        "premise: the pane is clear of the pillar: {} -> {}",
        pillar(&bare),
        pillar(&behind)
    );
    let (bare, behind) = (reflection_at(&bare), reflection_at(&behind));
    assert!(
        bare.x > 0.1 && behind.x < 0.9 * bare.x,
        "premise: the floor reflects the pillar there, and the pane covers it: {bare} -> {behind}"
    );
    assert!(
        behind.x < 0.75 * bare.x,
        "through the half-transparent pane, about half of it -- not the pane          reflecting it again: {bare} -> {behind}"
    );
}

/// Where the pane of the transparency test stands: low, and near the camera.
const PANE_Y: f32 = 0.85;
const PANE_Z: f32 = 5.0;
const PANE_H: f32 = 0.3;

/// A hit the last frame's camera did not see has no light to bring back, and
/// leaves the environment's reflection whole: the camera looked at a green
/// wall far off last frame, so this frame's hits on the pillar fall outside
/// what it saw, and the mirror under the blue sky shows the sky as it does
/// with reflections off. Read at the nearest edge of last frame's view, they
/// would bring back that wall's green.
#[test]
fn a_hit_the_last_frame_did_not_see_brings_back_nothing() {
    let mut h = Harness::new();
    let (cube, plane) = (h.cube(), h.plane());
    h.set_test_skybox([0, 0, 160, 255]);
    let wall = || {
        Draw::new(cube, Vec3::new(-40.0, 1.0, 0.0))
            // Big enough to fill last frame's view to its corners: what an
            // unseen hit would be read at.
            .scaled(Vec3::new(1.0, 300.0, 300.0), Vec3::new(-40.0, 1.0, 0.0))
            .colour(Vec3::ZERO)
            .emissive(Vec3::new(0.0, 0.8, 0.0))
    };
    let room = |look_at: Vec3, ssr| {
        plain(Scene {
            draws: vec![
                metal_floor(plane, Vec3::ONE),
                pillar_at(cube, 0.0, Vec3::new(0.8, 0.0, 0.0)),
                wall(),
            ],
            with_skybox: true,
            look_at,
            ..scene(cube, ssr)
        })
    };
    let ahead = Vec3::new(0.0, 0.6, 0.0);
    let without = reflection_at(&h.render(&room(ahead, None)));
    let on = Some(ScreenSpaceReflections::default());
    // Last frame at the wall, reflections on (so it is the history); this
    // frame ahead.
    let away = h.render(&room(Vec3::new(-40.0, 1.0, 6.0), on));
    let corner = away.at(0, 0);
    assert!(
        linear(corner[1]) > 0.3 && linear(corner[2]) < 0.05,
        "premise: last frame saw the green wall to its corners: {corner:?}"
    );
    let back = reflection_at(&h.render(&room(ahead, on)));
    assert!(
        (back - without).abs().max_element() < 0.02,
        "no light from a hit last frame did not see: {without} -> {back}"
    );
}

/// A hit is traced against this frame's geometry: the pillar jumps aside,
/// and the frame it jumps the floor where its reflection was shows none --
/// the rays there pass where it no longer stands. (Its new reflection comes
/// a frame later: last frame's scene has nothing there to read.) Traced
/// against last frame's depth, the rays would still find it where it was.
#[test]
fn a_hit_is_traced_against_this_frames_geometry() {
    let mut h = Harness::new();
    let (cube, plane) = (h.cube(), h.plane());
    let room = |x: f32| {
        plain(Scene {
            draws: vec![
                metal_floor(plane, Vec3::ONE),
                pillar_at(cube, x, Vec3::new(0.8, 0.0, 0.0)),
            ],
            ..scene(cube, Some(ScreenSpaceReflections::default()))
        })
    };
    let there = reflection_at(&settled(&mut h, &room(0.0)));
    let gone = reflection_at(&h.render(&room(4.0)));
    assert!(there.x > 0.1, "premise: reflected where it stood: {there}");
    assert!(
        gone.x < 0.25 * there.x,
        "the frame it moves, no reflection where it was: {there} -> {gone}"
    );
}

/// What screen-space reflections cost at 1920 x 1080, at their default
/// settings, now that they trace before the opaque pass: the prepass of the
/// opaque meshes' depth and normals, the trace, the copy of the lit scene
/// for the next frame. The mirror floor and pillar with and without them, a
/// minute's worth of frames after a warm-up. `cargo test --release -p
/// bsengine-rhi-wgpu --test pixels_ssr -- --ignored --nocapture bench_ssr`
/// -- release, because a debug build's validation layers dominate any GPU
/// timing (CLAUDE.md).
#[test]
#[ignore]
fn bench_ssr() {
    let frames = 60u32;
    let mut times = Vec::new();
    for (label, ssr) in [
        ("off", None),
        ("on", Some(ScreenSpaceReflections::default())),
    ] {
        let mut h = Harness::new();
        h.resize(1920, 1080);
        let (cube, plane) = (h.cube(), h.plane());
        let s = plain(Scene {
            draws: vec![
                metal_floor(plane, Vec3::ONE),
                pillar_at(cube, 0.0, Vec3::new(0.8, 0.0, 0.0)),
            ],
            ..scene(cube, ssr)
        });
        for _ in 0..10 {
            h.render(&s);
        }
        let t = std::time::Instant::now();
        for _ in 0..frames {
            h.render(&s);
        }
        let ms = t.elapsed().as_secs_f64() * 1000.0 / f64::from(frames);
        eprintln!("ssr {label}: {ms:.2} ms/frame");
        times.push(ms);
    }
    eprintln!("ssr costs {:.2} ms/frame at 1080p", times[1] - times[0]);
}

/// The columns of `row` where red outweighs green and blue by a margin: where
/// the pillar, or its reflection, is.
fn red_columns(p: &common::Pixels, row: u32) -> Vec<u32> {
    (0..p.width)
        .filter(|&x| {
            let [r, g, b, _] = p.at(x, row);
            r as i32 > g as i32 + 20 && r as i32 > b as i32 + 20
        })
        .collect()
}

/// The pillar's glow in the moving-reflection test.
const RED: Vec3 = Vec3::new(4.0, 0.4, 0.4);

/// A moving object is reflected where it is. The hit is found on this
/// frame's depth, but its light is read from last frame's scene, so the trace
/// has to know where the hit was then: for a surface that moved, its own
/// velocity says so (the prepass writes it, as the opaque pass would). Found
/// by last frame's camera alone -- correct only for what stands still -- the
/// pillar's reflection read the floor the pillar had not reached yet, and a
/// moving object's reflection lagged a frame behind it.
#[test]
fn a_moving_object_is_reflected_where_it_is() {
    let mut h = Harness::new();
    let cube = h.cube();
    let ssr = Some(ScreenSpaceReflections::default());
    let (from, to) = (-1.6, 1.6);
    let was = Scene {
        draws: vec![floor(cube), pillar_at(cube, from, RED)],
        ..scene(cube, ssr)
    };
    let moved = Draw {
        previous: Some(pillar_at(cube, from, RED).transform),
        ..pillar_at(cube, to, RED)
    };
    let is = Scene {
        draws: vec![floor(cube), moved],
        ..scene(cube, ssr)
    };

    // Where the pillar now stands on screen, from a frame without
    // reflections: its body's red columns at the frame's middle height.
    let still = h.render(&Scene {
        ssr: None,
        draws: vec![floor(cube), pillar_at(cube, to, RED)],
        ..scene(cube, None)
    });
    let body = red_columns(&still, still.height / 2);
    assert!(!body.is_empty(), "premise: the pillar is on screen");
    let x = body[body.len() / 2];
    let y = reflection_row(still.height);
    let dark = still.at(x, y);

    // Last frame: the pillar at `from`, nowhere near `x`.
    let before = h.render(&was);
    assert!(
        !red_columns(&before, before.height / 2).contains(&x),
        "premise: last frame the pillar was elsewhere, so the history at {x} is floor"
    );
    let now = h.render_moving(&was, &is);
    let lit = now.at(x, y);
    // And as much as a pillar standing there all along reflects.
    let standing = settled(
        &mut h,
        &Scene {
            draws: vec![floor(cube), pillar_at(cube, to, RED)],
            ..scene(cube, ssr)
        },
    )
    .at(x, y);
    assert!(
        standing[0] > dark[0] + 8,
        "premise: a pillar standing at {x} is reflected there: {dark:?} -> {standing:?}"
    );
    assert!(
        lit[0] as f32 >= dark[0] as f32 + 0.7 * (standing[0] as f32 - dark[0] as f32),
        "the moving pillar should be reflected where it is now, as brightly as a \
         standing one: floor {dark:?}, moving {lit:?}, standing {standing:?} at ({x}, {y})"
    );
}
