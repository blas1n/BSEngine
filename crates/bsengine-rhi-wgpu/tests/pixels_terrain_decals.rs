//! Decals landing on terrain.
//!
//! Terrain was not a decal receiver when decals shipped: its model-buffer slots
//! were numbered inside the main pass's own loop, so the depth prepass -- which
//! is what a decal projects onto -- had nothing to draw them from. A decal on
//! the ground of a terrain level therefore projected onto the far plane and
//! vanished, which is most of what decals are for.
//!
//! These are also the first pixel tests terrain has ever had. `terrain_draw`
//! calls `render_frame` directly and only checks that nothing panics, so
//! anything terrain actually *renders* was unobservable.

mod common;

use bsengine_rhi_wgpu::decals::DecalDraw;
use common::{Harness, Scene};
use glam::{Mat4, Vec3};

/// A big flat terrain chunk at the origin, lying in the XZ plane at y = 0.
fn chunk(mesh: u64, layers: [u64; 4], weight: u64) -> (u64, Mat4, [u64; 4], u64) {
    (
        mesh,
        Mat4::from_scale(Vec3::new(20.0, 1.0, 20.0)),
        layers,
        weight,
    )
}

/// A decal box straddling the terrain surface.
fn decal(texture: u64) -> DecalDraw {
    DecalDraw {
        model: Mat4::from_translation(Vec3::new(0.0, 0.0, 0.0))
            * Mat4::from_scale(Vec3::new(8.0, 2.0, 8.0)),
        opacity: 1.0,
        normal_fade: 0.0,
        texture: Some(texture),
    }
}

fn terrain_scene(mesh: u64, layers: [u64; 4], weight: u64, decals: Vec<DecalDraw>) -> Scene {
    Scene {
        terrain: vec![chunk(mesh, layers, weight)],
        decals,
        camera_pos: Vec3::new(0.0, 4.0, 7.0),
        look_at: Vec3::ZERO,
        ..Default::default()
    }
}

/// Four layer textures and a weight that selects the first one.
///
/// Distinct colours per layer deliberately: with all four the same, a splat
/// that read the wrong layer would look correct.
fn terrain_textures(h: &mut Harness) -> ([u64; 4], u64) {
    let layers = [
        h.two_colour_texture([0, 200, 0, 255], [0, 200, 0, 255]),
        h.two_colour_texture([200, 200, 0, 255], [200, 200, 0, 255]),
        h.two_colour_texture([0, 0, 200, 255], [0, 0, 200, 255]),
        h.two_colour_texture([200, 0, 200, 255], [200, 0, 200, 255]),
    ];
    // Weight (1, 0, 0, 0): all of layer 0.
    let weight = h.two_colour_texture([255, 0, 0, 255], [255, 0, 0, 255]);
    (layers, weight)
}

#[test]
fn terrain_renders_at_all() {
    // The precondition for everything below, and the first assertion anywhere
    // that terrain puts pixels on screen rather than merely not panicking.
    let mut h = Harness::new();
    let plane = h.cube();
    let (layers, weight) = terrain_textures(&mut h);

    let empty = h.render(&Scene {
        camera_pos: Vec3::new(0.0, 4.0, 7.0),
        look_at: Vec3::ZERO,
        ..Default::default()
    });
    let with_terrain = h.render(&terrain_scene(plane, layers, weight, vec![]));

    assert!(
        with_terrain.differs_from(&empty),
        "a terrain chunk has to reach the screen: {}",
        with_terrain.describe()
    );
}

#[test]
fn a_decal_lands_on_terrain() {
    let mut h = Harness::new();
    let plane = h.cube();
    let (layers, weight) = terrain_textures(&mut h);
    let red = h.two_colour_texture([255, 0, 0, 255], [255, 0, 0, 255]);

    let plain = h.render(&terrain_scene(plane, layers, weight, vec![]));
    let painted = h.render(&terrain_scene(plane, layers, weight, vec![decal(red)]));

    let before = plain.centre();
    let after = painted.centre();
    assert!(
        after[0] > before[0] && after[1] < before[1],
        "a red decal should make the green terrain redder and less green: \
         {before:?} -> {after:?}"
    );
}

#[test]
fn a_decal_that_misses_the_terrain_leaves_it_alone() {
    // Paired with the test above. Without it, a decal pass that painted every
    // pixel it covered on screen -- ignoring where the terrain actually is --
    // would pass that one perfectly.
    let mut h = Harness::new();
    let plane = h.cube();
    let (layers, weight) = terrain_textures(&mut h);
    let red = h.two_colour_texture([255, 0, 0, 255], [255, 0, 0, 255]);

    let plain = h.render(&terrain_scene(plane, layers, weight, vec![]));
    let floating = h.render(&terrain_scene(
        plane,
        layers,
        weight,
        vec![DecalDraw {
            // Well above the ground, and big enough to rasterise -- a thinner
            // box produces no fragments at all from this camera, which would
            // make this test pass for the wrong reason.
            model: Mat4::from_translation(Vec3::new(0.0, 3.0, 0.0))
                * Mat4::from_scale(Vec3::new(4.0, 2.0, 4.0)),
            ..decal(red)
        }],
    ));

    assert!(
        !floating.differs_from(&plain),
        "a decal whose box does not reach the terrain must leave it alone: {}",
        floating.describe()
    );
}

#[test]
fn a_decal_reaches_terrain_and_meshes_in_the_same_frame() {
    // The slot numbering is what this is really about: terrain chunks take
    // model-buffer slots after the mesh draw calls, and the prepass and the
    // main pass have to agree on which slot each one got. With a mesh in the
    // frame the terrain's slots start somewhere other than zero, so a prepass
    // that numbered them differently would draw the terrain at the mesh's
    // matrix -- and the decal would land somewhere neither of them is.
    let mut h = Harness::new();
    let plane = h.cube();
    let cube = h.cube();
    let (layers, weight) = terrain_textures(&mut h);
    let red = h.two_colour_texture([255, 0, 0, 255], [255, 0, 0, 255]);

    let with_mesh = |decals: Vec<DecalDraw>| Scene {
        draws: vec![common::Draw::new(cube, Vec3::new(3.0, 0.5, 0.0))],
        ..terrain_scene(plane, layers, weight, decals)
    };

    let plain = h.render(&with_mesh(vec![]));
    let painted = h.render(&with_mesh(vec![decal(red)]));

    let before = plain.centre();
    let after = painted.centre();
    assert!(
        after[0] > before[0] && after[1] < before[1],
        "the decal still has to land on the terrain when a mesh shares the \
         frame and pushes the terrain's model slots along: {before:?} -> {after:?}"
    );
}
