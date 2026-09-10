//! The shadow passes draw objects sharing a mesh with one instanced call.
//!
//! **Why the fixture looks like this.** Every draw here is the *same* cube
//! mesh at a *different* position. Both halves are load-bearing: if the
//! meshes differed, every batch would hold one object, instancing would be
//! a no-op, and these tests would pass while proving nothing. If the
//! transforms matched, a wrong instance-to-slot mapping would be
//! invisible.
mod common;

use common::{Draw, Harness, Light, PointLight, Scene};
use glam::Vec3;

/// Four cubes of one mesh, spread out so each is separately observable.
fn four_spread_cubes(cube: u64) -> Vec<Draw> {
    vec![
        Draw::new(cube, Vec3::new(-3.0, 0.0, 0.0)),
        Draw::new(cube, Vec3::new(-1.0, 0.0, 0.0)),
        Draw::new(cube, Vec3::new(1.0, 0.0, 0.0)),
        Draw::new(cube, Vec3::new(3.0, 0.0, 0.0)),
    ]
}

#[test]
fn this_adapter_supports_vertex_stage_storage_buffers() {
    // Not a claim about all hardware -- a guard for this suite. If this
    // fails, the other tests here are exercising the per-object fallback
    // and prove nothing about instancing.
    let h = Harness::new();
    assert!(
        h.instancing_supported(),
        "this adapter reports no VERTEX_STORAGE, so the instanced path is \
         inactive and the rest of this suite is testing the fallback"
    );
}

#[test]
fn four_cubes_of_one_mesh_cost_fewer_draw_calls_than_objects() {
    let mut h = Harness::new();
    let cube = h.cube();
    h.render(&Scene {
        draws: four_spread_cubes(cube),
        ..Scene::default()
    });

    let stats = h.frame_stats();
    assert!(
        stats.draw_calls < stats.objects_drawn,
        "the directional shadow pass should batch four same-mesh cubes into one \
         draw call, so draw_calls={} must be below objects_drawn={}",
        stats.draw_calls,
        stats.objects_drawn
    );
}

#[test]
fn batching_does_not_reduce_the_number_of_objects_drawn() {
    // The paired assertion. Fewer draw calls is the optimisation; fewer
    // *objects* would mean something stopped being drawn. Compare one cube
    // against four: objects must scale with the cubes even as draw calls
    // do not.
    let mut h = Harness::new();
    let cube = h.cube();

    h.render(&Scene {
        draws: vec![Draw::new(cube, Vec3::ZERO)],
        ..Scene::default()
    });
    let one = h.frame_stats();

    h.render(&Scene {
        draws: four_spread_cubes(cube),
        ..Scene::default()
    });
    let four = h.frame_stats();

    assert_eq!(
        four.objects_drawn - one.objects_drawn,
        6,
        "three extra cubes each add one opaque object and one directional-shadow \
         object, so objects_drawn must rise by 6 (one={}, four={})",
        one.objects_drawn,
        four.objects_drawn
    );
    // Only the opaque pass grows. This pins the scope of the change as
    // precisely as the batching itself: the three extra cubes still cost
    // three extra *opaque* draws, because the main pass is deliberately
    // not instanced (that would break MESH_WGSL's @group(1) contract,
    // which custom shaders copy). The shadow pass absorbs them for free.
    assert_eq!(
        four.draw_calls - one.draw_calls,
        3,
        "three extra same-mesh cubes must add exactly three draw calls -- one \
         opaque each, and nothing at all in the shadow pass, which batches them \
         (one={}, four={})",
        one.draw_calls,
        four.draw_calls
    );
}

#[test]
fn a_point_light_batches_its_cube_faces_too() {
    // The point-shadow pass renders six cube faces per light, each of which
    // previously issued one draw call per object. This is the pass that
    // dominates a lit frame, so it needs its own assertion rather than
    // riding on the directional one.
    // Measured as a delta against the identical scene with no point light,
    // so the assertion is about the point-shadow pass alone and not about
    // however many draws the sky and post-processing happen to cost.
    let mut h = Harness::new();
    let cube = h.cube();

    h.render(&Scene {
        draws: four_spread_cubes(cube),
        ..Scene::default()
    });
    let unlit = h.frame_stats();

    h.render(&Scene {
        draws: four_spread_cubes(cube),
        light: Light {
            points: vec![PointLight {
                position: Vec3::new(0.0, 4.0, 4.0),
                color: Vec3::ONE,
                intensity: 20.0,
                range: 40.0,
            }],
            ..Light::default()
        },
        ..Scene::default()
    });
    let lit = h.frame_stats();

    let objects_delta = lit.objects_drawn - unlit.objects_drawn;
    let draws_delta = lit.draw_calls - unlit.draw_calls;

    // Structural, and independent of batching: a point shadow is a
    // six-face cube render, so four in-range cubes are 24 drawn objects.
    assert_eq!(
        objects_delta, 24,
        "one point light must draw all four cubes into all six of its cube \
         faces: expected 24 extra objects, got {objects_delta} \
         (unlit={} lit={})",
        unlit.objects_drawn, lit.objects_drawn
    );
    // Batching: those 24 objects should cost six draws (one per face), not
    // 24. Per-object drawing would make this 24 * 3 < 24, which is false.
    assert!(
        draws_delta * 3 < objects_delta,
        "the point-shadow pass should batch four same-mesh cubes per face, so \
         its 24 objects cost about six draw calls; got {draws_delta} extra draw \
         calls for {objects_delta} extra objects"
    );
}
