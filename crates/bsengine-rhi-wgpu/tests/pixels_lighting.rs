//! Lighting and shadows.
//!
//! These are the tests that found the bug they now guard. The shadow
//! comparison sampler asked for `GreaterEqual` while the shadow pass writes
//! ordinary forward-Z depth, so `shadow_factor` returned 0 for every fragment
//! inside the shadow frustum -- and since the light matrix centres a 60-unit
//! box on the origin, that was every object in every game here. The sun
//! contributed nothing to any frame ever rendered, and no shadow was ever
//! drawn.
//!
//! The bug hid itself: with the sun contributing nothing, "everything is in
//! shadow" and "there are no shadows" look exactly alike. Two of the tests
//! below were written first in a form that also could not tell those apart --
//! they measured how much darker the frame got when the cube was added, which
//! the cube covering floor satisfies just as well as the cube shadowing it.
//! Both now exclude the pixels the cube itself occupies, computed rather than
//! assumed.

mod common;

use common::{Draw, Harness, Light, Pixels, PointLight, Scene};
use glam::Vec3;

/// A big flat floor at the origin.
fn floor(mesh: u64) -> Draw {
    Draw::new(mesh, Vec3::ZERO).scaled(Vec3::new(20.0, 1.0, 20.0), Vec3::ZERO)
}

/// A shallow camera, so a shadow lands beside its caster rather than being
/// hidden underneath it.
const CAMERA: Vec3 = Vec3::new(0.0, 4.0, 7.0);

fn sun_from_above() -> Light {
    Light {
        direction: Vec3::new(0.0, -1.0, 0.0),
        ambient: Vec3::splat(0.05),
        ..Light::default()
    }
}

/// Which pixels the cube itself covers, worked out by drawing the cube with no
/// floor and taking everything that is not the clear colour.
///
/// Without this a "the floor got darker" assertion is satisfied by the cube
/// merely standing in front of the floor, which is true whether or not shadows
/// work at all.
fn cube_silhouette(h: &mut Harness, cube: u64) -> Vec<bool> {
    let empty = h.render(&Scene {
        light: sun_from_above(),
        camera_pos: CAMERA,
        ..Scene::default()
    });
    let cube_only = h.render(&Scene {
        draws: vec![Draw::new(cube, Vec3::new(0.0, 3.0, 0.0))],
        light: sun_from_above(),
        camera_pos: CAMERA,
        ..Scene::default()
    });
    (0..(empty.width * empty.height))
        .map(|i| {
            let (x, y) = (i % empty.width, i / empty.width);
            empty.at(x, y) != cube_only.at(x, y)
        })
        .collect()
}

/// The strongest darkening between two frames, ignoring pixels the caster
/// covers. Returns (delta, x, y).
fn biggest_darkening_off_the_caster(
    open: &Pixels,
    occluded: &Pixels,
    silhouette: &[bool],
) -> (f32, u32, u32) {
    let mut worst = (0.0_f32, 0_u32, 0_u32);
    for y in 0..occluded.height {
        for x in 0..occluded.width {
            if silhouette[(y * occluded.width + x) as usize] {
                continue;
            }
            let delta = open.luma(x, y) - occluded.luma(x, y);
            if delta > worst.0 {
                worst = (delta, x, y);
            }
        }
    }
    worst
}

#[test]
fn the_sun_lights_a_surface_facing_it() {
    let mut h = Harness::new();
    let plane = h.plane();
    let camera_pos = Vec3::new(0.0, 6.0, 0.01);

    // Ambient off, so anything that is not black is the sun's doing.
    let facing = h.render(&Scene {
        draws: vec![floor(plane)],
        light: Light {
            direction: Vec3::new(0.0, -1.0, 0.0),
            ambient: Vec3::ZERO,
            ..Light::default()
        },
        camera_pos,
        ..Scene::default()
    });
    let facing_away = h.render(&Scene {
        draws: vec![floor(plane)],
        light: Light {
            direction: Vec3::new(0.0, 1.0, 0.0),
            ambient: Vec3::ZERO,
            ..Light::default()
        },
        camera_pos,
        ..Scene::default()
    });

    assert!(
        facing.luma(30, 30) > facing_away.luma(30, 30) + 40.0,
        "a floor under a downward sun should be much brighter than one lit from below: \
         {} facing, {} facing away",
        facing.luma(30, 30),
        facing_away.luma(30, 30)
    );
    assert!(
        facing.luma(30, 30) > 40.0,
        "with no ambient at all the sun alone has to light the floor; it read {}. \
         Zero here is the signature of the shadow-comparison bug: every fragment \
         inside the shadow frustum reporting as occluded.",
        facing.luma(30, 30)
    );
}

#[test]
fn an_occluder_darkens_floor_it_does_not_cover() {
    let mut h = Harness::new();
    let plane = h.plane();
    let cube = h.cube();
    let silhouette = cube_silhouette(&mut h, cube);

    let open = h.render(&Scene {
        draws: vec![floor(plane)],
        light: sun_from_above(),
        camera_pos: CAMERA,
        ..Scene::default()
    });
    let occluded = h.render(&Scene {
        draws: vec![floor(plane), Draw::new(cube, Vec3::new(0.0, 3.0, 0.0))],
        light: sun_from_above(),
        camera_pos: CAMERA,
        ..Scene::default()
    });

    let (delta, x, y) = biggest_darkening_off_the_caster(&open, &occluded, &silhouette);
    assert!(
        delta > 60.0,
        "the cube should darken floor it is not standing in front of; the strongest \
         darkening away from its silhouette was {delta} at ({x}, {y}), open {:?} \
         occluded {:?}",
        open.at(x, y),
        occluded.at(x, y)
    );
}

#[test]
fn the_shadow_does_not_cover_the_whole_floor() {
    let mut h = Harness::new();
    let plane = h.plane();
    let cube = h.cube();
    let silhouette = cube_silhouette(&mut h, cube);

    let open = h.render(&Scene {
        draws: vec![floor(plane)],
        light: sun_from_above(),
        camera_pos: CAMERA,
        ..Scene::default()
    });
    let occluded = h.render(&Scene {
        draws: vec![floor(plane), Draw::new(cube, Vec3::new(0.0, 3.0, 0.0))],
        light: sun_from_above(),
        camera_pos: CAMERA,
        ..Scene::default()
    });

    // A shadow that covers everything is not a shadow -- it is the bug this
    // file was written to catch. Floor at the frame's edge is far from the cube
    // and has to keep both its brightness and its lighting.
    let (edge_x, edge_y) = (5, occluded.height - 5);
    assert!(
        !silhouette[(edge_y * occluded.width + edge_x) as usize],
        "the sample point for far floor must not be somewhere the cube covers"
    );
    let far = occluded.luma(edge_x, edge_y);
    assert!(
        (open.luma(edge_x, edge_y) - far).abs() < 5.0,
        "floor far from the occluder should be unchanged by it: {} open, {far} with the cube",
        open.luma(edge_x, edge_y)
    );

    // And the shadowed floor must be markedly darker than that far floor, in
    // the same frame. Comparing within one frame is what distinguishes a real
    // shadow from a renderer that darkened everything equally.
    let (_, sx, sy) = biggest_darkening_off_the_caster(&open, &occluded, &silhouette);
    assert!(
        occluded.luma(sx, sy) < far - 40.0,
        "shadowed floor at ({sx}, {sy}) read {} while floor far from the cube read {far}; \
         a shadow has to be darker than the floor around it",
        occluded.luma(sx, sy)
    );
}

#[test]
fn a_point_light_is_blocked_by_an_occluder() {
    let mut h = Harness::new();
    let plane = h.plane();
    let cube = h.cube();
    let camera_pos = Vec3::new(0.0, 5.0, 6.0);

    // Directional light off entirely, so only the point light can brighten
    // anything and only its cube shadow map can darken it.
    let light = || Light {
        color: Vec3::ZERO,
        ambient: Vec3::splat(0.02),
        points: vec![PointLight {
            position: Vec3::new(0.0, 4.0, 0.0),
            color: Vec3::ONE,
            intensity: 60.0,
            range: 40.0,
        }],
        ..Light::default()
    };

    let open = h.render(&Scene {
        draws: vec![floor(plane)],
        light: light(),
        camera_pos,
        ..Scene::default()
    });
    let occluded = h.render(&Scene {
        draws: vec![floor(plane), Draw::new(cube, Vec3::new(0.0, 2.0, 0.0))],
        light: light(),
        camera_pos,
        ..Scene::default()
    });

    // The same silhouette exclusion as the directional case: a cube standing in
    // front of floor darkens those pixels whether or not it casts a shadow.
    let empty = h.render(&Scene {
        light: light(),
        camera_pos,
        ..Scene::default()
    });
    let cube_only = h.render(&Scene {
        draws: vec![Draw::new(cube, Vec3::new(0.0, 2.0, 0.0))],
        light: light(),
        camera_pos,
        ..Scene::default()
    });
    let silhouette: Vec<bool> = (0..(empty.width * empty.height))
        .map(|i| {
            let (x, y) = (i % empty.width, i / empty.width);
            empty.at(x, y) != cube_only.at(x, y)
        })
        .collect();

    let (delta, x, y) = biggest_darkening_off_the_caster(&open, &occluded, &silhouette);
    assert!(
        delta > 30.0,
        "the cube should cast a point-light shadow on floor it does not cover; the \
         strongest darkening away from its silhouette was {delta} at ({x}, {y}), \
         open {:?} occluded {:?}",
        open.at(x, y),
        occluded.at(x, y)
    );
}
