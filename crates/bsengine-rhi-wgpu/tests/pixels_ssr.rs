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
    let with = h.render(&scene(cube, Some(ScreenSpaceReflections::default())));

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
    let disabled = h.render(&scene(
        cube,
        Some(ScreenSpaceReflections {
            enabled: false,
            ..Default::default()
        }),
    ));

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

    let with = h.render(&rough);
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
    let with = h.render(&empty(Some(ScreenSpaceReflections::default())));

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
    let normal = h.render(&scene(cube, Some(coarse)));
    let swallowed = h.render(&scene(
        cube,
        Some(ScreenSpaceReflections {
            // NDC depth spans 0..1, so this accepts a hit anywhere behind.
            thickness: 10.0,
            ..coarse
        }),
    ));

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
    let short = h.render(&scene(
        cube,
        Some(ScreenSpaceReflections {
            steps: 1,
            stride: 0.01,
            ..Default::default()
        }),
    ));

    assert!(
        short.max_channel_diff(&without) <= 2,
        "a ray reaching one centimetre cannot have found the pillar: {}",
        short.describe()
    );
}
