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
/// through its own `F0`.
fn metal_floor(mesh: u64, colour: Vec3) -> Draw {
    floor(mesh).colour(colour).metallic(1.0)
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
/// with reflections on, the share `w` the screen found (its red, over the
/// pillar's) is the pillar's, and the sky keeps only `1 - w` of the blue.
/// Added on top (as the reflection pass once was), the blue would stay
/// whole whatever the red.
#[test]
fn a_reflection_replaces_the_environment_it_found_something_in_front_of() {
    let mut h = Harness::new();
    let cube = h.cube();
    h.set_test_skybox([0, 0, 160, 255]);
    let glow = Vec3::new(0.8, 0.0, 0.0);
    let room = |ssr| {
        plain(Scene {
            draws: vec![metal_floor(cube, Vec3::ONE), pillar_at(cube, 0.0, glow)],
            with_skybox: true,
            ..scene(cube, ssr)
        })
    };
    let without = reflection_at(&h.render(&room(None)));
    let with = reflection_at(&settled(
        &mut h,
        &room(Some(ScreenSpaceReflections::default())),
    ));
    let w = with.x / glow.x;
    assert!(
        without.z > 0.1 && w > 0.2 && w < 1.01,
        "premise: the sky is reflected there, and then the pillar is: {without} -> {with} (w {w})"
    );
    assert!(
        (with.z / without.z - (1.0 - w)).abs() < 0.08,
        "the sky keeps {} of its blue there, and it kept {}: {without} -> {with}",
        1.0 - w,
        with.z / without.z
    );
}

/// The reflection goes through the surface's own reflectance, as the
/// environment's does: a gold mirror shows the white pillar gold -- its
/// channels in the proportion of its `F0`, (1, 0.8, 0.3). Unweighted (as the
/// pass once added it), white.
#[test]
fn a_metal_reflects_what_it_finds_in_its_own_colour() {
    let mut h = Harness::new();
    let cube = h.cube();
    let gold = Vec3::new(1.0, 0.8, 0.3);
    let room = plain(Scene {
        draws: vec![
            metal_floor(cube, gold),
            pillar_at(cube, 0.0, Vec3::splat(0.8)),
        ],
        ..scene(cube, Some(ScreenSpaceReflections::default()))
    });
    let p = reflection_at(&settled(&mut h, &room));
    assert!(p.x > 0.15, "premise: the pillar is reflected: {p}");
    assert!(
        (p.y / p.x - 0.8).abs() < 0.1 && (p.z / p.x - 0.3).abs() < 0.1,
        "gold: green and blue at 0.8 and 0.3 of red, not white: {p}"
    );
}

/// A hit's light is last frame's: the pillar turns from red to green, and
/// the frame it turns still reflects it red -- the reflection's colour comes
/// from the scene of the frame before -- and the next one green.
#[test]
fn a_reflection_shows_its_hit_as_it_was_last_frame() {
    let mut h = Harness::new();
    let cube = h.cube();
    let room = |glow| {
        plain(Scene {
            draws: vec![metal_floor(cube, Vec3::ONE), pillar_at(cube, 0.0, glow)],
            ..scene(cube, Some(ScreenSpaceReflections::default()))
        })
    };
    let (red, green) = (Vec3::new(0.8, 0.0, 0.0), Vec3::new(0.0, 0.8, 0.0));
    let before = reflection_at(&settled(&mut h, &room(red)));
    let turning = reflection_at(&h.render(&room(green)));
    let after = reflection_at(&h.render(&room(green)));
    assert!(
        before.x > 0.15 && before.y < 0.02,
        "premise: red reflected: {before}"
    );
    assert!(
        turning.x > 0.15 && turning.y < 0.02,
        "the frame it turns green, still red: {turning}"
    );
    assert!(
        after.y > 0.15 && after.x < 0.02,
        "and green the frame after: {after}"
    );
}

/// A hit is found in last frame's scene where last frame's camera saw it,
/// not at the pixel it is at now: the camera slides sideways between two
/// frames, and the reflection of the red and the green pillar side by side
/// is what two frames from where it stopped show -- the boundary between
/// them where it is. Read from last frame's scene at this frame's pixels, it
/// would sit where the camera left it.
#[test]
fn a_hit_is_found_where_the_last_frame_saw_it() {
    let mut h = Harness::new();
    let cube = h.cube();
    let room = |dx: f32| {
        plain(Scene {
            draws: vec![
                metal_floor(cube, Vec3::ONE),
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
    let mut worst = 0.0f32;
    let mut colour = 0.0f32;
    for x in still.width / 4..still.width * 3 / 4 {
        let (a, b) = (slid.at(x, y), still.at(x, y));
        colour = colour.max(linear(b[0]).max(linear(b[1])));
        for c in 0..3 {
            worst = worst.max((linear(a[c]) - linear(b[c])).abs());
        }
    }
    assert!(
        colour > 0.15,
        "premise: the pillars are reflected: {colour}"
    );
    assert!(
        worst < 0.03,
        "after the slide the reflection is where it is from standing still: off by {worst}"
    );
}
