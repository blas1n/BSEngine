//! Fast render mode (used only by CI's headless E2E replays): shadow, bloom
//! and SSAO shading must be skipped, but every render target they'd
//! otherwise write must still be cleared to its correct neutral value.

mod common;

use common::{Draw, Harness, Light, Scene};
use glam::Vec3;

#[test]
fn fast_render_mode_skips_the_shadow_pass() {
    // Light straight down, so a cube directly above the origin casts its
    // shadow directly onto the origin -- avoids needing to hand-compute an
    // angled shadow offset. Camera sits off-axis so its view direction is
    // never anti-parallel with the harness's fixed Vec3::Y up vector.
    let scene_with = |plane: u64, cube: u64| Scene {
        draws: vec![
            Draw::new(plane, Vec3::ZERO).scaled(Vec3::new(20.0, 1.0, 20.0), Vec3::ZERO),
            Draw::new(cube, Vec3::new(0.0, 3.0, 0.0))
                .scaled(Vec3::splat(3.0), Vec3::new(0.0, 3.0, 0.0)),
        ],
        light: Light {
            direction: Vec3::new(0.0, -1.0, 0.0),
            ambient: Vec3::splat(0.05),
            ..Light::default()
        },
        camera_pos: Vec3::new(0.0, 5.0, 5.0),
        look_at: Vec3::ZERO,
        ..Scene::default()
    };

    let mut full = Harness::new();
    let full_plane = full.plane();
    let full_cube = full.cube();
    let shadowed = full.render(&scene_with(full_plane, full_cube));

    let mut fast = Harness::new_fast();
    let fast_plane = fast.plane();
    let fast_cube = fast.cube();
    let unshadowed = fast.render(&scene_with(fast_plane, fast_cube));

    assert!(
        unshadowed.centre_luma() > shadowed.centre_luma() + 5.0,
        "fast_render mode must skip the directional shadow pass (clearing the shadow map to \
         \"fully lit\" instead of drawing the occluding cube into it): the ground directly \
         under the cube should be brighter in fast mode ({:.1}) than in full mode, where the \
         cube casts a real shadow onto it ({:.1})",
        unshadowed.centre_luma(),
        shadowed.centre_luma()
    );
}
