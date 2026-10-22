//! Screen-space GI: the trace through the depth buffer, and the composite
//! that replaces each pixel's indirect diffuse light where the trace found
//! something.
//!
//! The fixture: a white floor running away from the camera, a wall standing
//! on it to the right, and the flat ambient as the light everything had
//! before -- seen from above and behind, so the floor beside the wall and
//! the wall's face are both in view.

mod common;

use bsengine_core::{
    AmbientOcclusion, Bloom, ScreenSpaceGlobalIllumination, ToneMap, ToneMappingMode,
};
use common::{Draw, Harness, Light, Pixels, Scene, HEIGHT, WIDTH};
use glam::Vec3;

const AMBIENT: Vec3 = Vec3::splat(0.3);

/// Columns of floor beside the wall's foot (the wall stands at about 13/24
/// to 19/24 of the frame, from this camera), and of floor far from it.
const NEAR: (u32, u32) = (WIDTH * 9 / 24, WIDTH * 12 / 24);
const FAR: (u32, u32) = (WIDTH / 24, WIDTH * 4 / 24);

fn vertex(p: [f32; 3], n: [f32; 3]) -> bsengine_rhi_wgpu::Vertex {
    bsengine_rhi_wgpu::Vertex {
        position: p,
        color: [1.0, 1.0, 1.0],
        normal: n,
        uv: [0.0, 0.0],
        uv2: [0.0, 0.0],
        tangent: [0.0; 4],
    }
}

/// The floor, y = 0 from x = -3 to 1 and z = -2 to -16, facing up.
fn floor(h: &mut Harness) -> u64 {
    let n = [0.0, 1.0, 0.0];
    h.mesh(
        &[
            vertex([-3.0, 0.0, -2.0], n),
            vertex([1.0, 0.0, -2.0], n),
            vertex([1.0, 0.0, -16.0], n),
            vertex([-3.0, 0.0, -16.0], n),
        ],
        &[0, 1, 2, 0, 2, 3],
    )
}

/// The wall, x = 1 from y = 0 to 3 along the floor, facing the floor (-x).
fn wall(h: &mut Harness) -> u64 {
    let n = [-1.0, 0.0, 0.0];
    h.mesh(
        &[
            vertex([1.0, 0.0, -2.0], n),
            vertex([1.0, 0.0, -16.0], n),
            vertex([1.0, 3.0, -16.0], n),
            vertex([1.0, 3.0, -2.0], n),
        ],
        // Wound to face -x, toward the floor and the camera.
        &[0, 2, 1, 0, 3, 2],
    )
}

fn scene(draws: Vec<Draw>, ssgi: Option<ScreenSpaceGlobalIllumination>) -> Scene {
    Scene {
        draws,
        light: Light {
            color: Vec3::ZERO,
            ambient: AMBIENT,
            ..Light::default()
        },
        camera_pos: Vec3::new(-1.5, 3.0, 2.0),
        look_at: Vec3::new(0.0, 0.0, -8.0),
        bloom: Some(Bloom::default().disabled()),
        ssao: Some(AmbientOcclusion::default().disabled()),
        tone_map: Some(ToneMap::new(ToneMappingMode::None)),
        ssgi,
        ..Scene::default()
    }
}

fn on() -> Option<ScreenSpaceGlobalIllumination> {
    Some(ScreenSpaceGlobalIllumination {
        rays: 16,
        steps: 48,
        stride: 0.1,
        thickness: 0.002,
        ..Default::default()
    })
}

fn linear(c: u8) -> f32 {
    let c = f32::from(c) / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// Mean linear rgb over the floor pixels in a column band `x0..x1` of the
/// middle rows -- the floor's pixels are the grey ones, the wall's red or
/// black, so those are left out by colour.
fn floor_band(p: &Pixels, x0: u32, x1: u32) -> Vec3 {
    let mut sum = Vec3::ZERO;
    let mut n = 0.0;
    for y in HEIGHT / 2..HEIGHT * 3 / 4 {
        for x in x0..x1 {
            let [r, g, b, _] = p.at(x, y);
            // The floor is grey-ish: its blue is near its green.
            if (i32::from(g) - i32::from(b)).abs() < 12 && g > 20 {
                sum += Vec3::new(linear(r), linear(g), linear(b));
                n += 1.0;
            }
        }
    }
    assert!(n > 20.0, "premise: floor pixels in {x0}..{x1}: {n}");
    sum / n
}

/// The red wall's light lands on the floor beside it: the floor near the wall
/// reddens -- red up, green and blue barely -- and more than the floor far
/// from it does.
#[test]
fn a_red_wall_bleeds_onto_the_floor_beside_it() {
    let mut h = Harness::new();
    let (f, w) = (floor(&mut h), wall(&mut h));
    let draws = || {
        vec![
            Draw::new(f, Vec3::ZERO).colour(Vec3::ONE).roughness(1.0),
            Draw::new(w, Vec3::ZERO)
                .colour(Vec3::ZERO)
                .emissive(Vec3::new(3.0, 0.0, 0.0)),
        ]
    };
    let off = h.render(&scene(draws(), None));
    let lit = h.render(&scene(draws(), on()));
    let (near, far) = (NEAR, FAR);
    let (near_off, near_on) = (
        floor_band(&off, near.0, near.1),
        floor_band(&lit, near.0, near.1),
    );
    let (far_off, far_on) = (
        floor_band(&off, far.0, far.1),
        floor_band(&lit, far.0, far.1),
    );
    let gain = near_on - near_off;
    assert!(
        gain.x > 0.05,
        "the floor by the wall reddens: {near_off} -> {near_on}"
    );
    assert!(
        gain.y.abs() < gain.x * 0.3 && gain.z.abs() < gain.x * 0.3,
        "red, not white: {gain}"
    );
    assert!(
        gain.x > 2.0 * (far_on - far_off).x,
        "more by the wall than away from it: {gain} vs {}",
        far_on - far_off
    );
}

/// What the trace finds replaces the light the surface had, rather than
/// adding to it: beside a black wall, the floor's rays that meet the wall
/// bring nothing back where the flat ambient had given them light, and the
/// floor there darkens. A trace that only added could never do that.
#[test]
fn a_black_wall_darkens_the_floor_it_shades() {
    let mut h = Harness::new();
    let (f, w) = (floor(&mut h), wall(&mut h));
    let draws = || {
        vec![
            Draw::new(f, Vec3::ZERO).colour(Vec3::ONE).roughness(1.0),
            Draw::new(w, Vec3::ZERO).colour(Vec3::ZERO),
        ]
    };
    let off = h.render(&scene(draws(), None));
    let lit = h.render(&scene(draws(), on()));
    let band = NEAR;
    let (before, after) = (
        floor_band(&off, band.0, band.1),
        floor_band(&lit, band.0, band.1),
    );
    assert!(
        after.y < before.y * 0.9,
        "the black wall takes its share of the sky: {before} -> {after}"
    );
}

/// Where no ray can find anything -- a floor with nothing above it -- the
/// floor is lit as without screen-space GI, byte for byte: every ray misses
/// and the composite adds exactly zero. And a disabled component is no
/// component, the whole frame identical.
#[test]
fn with_nothing_to_find_the_frame_is_unchanged() {
    let mut h = Harness::new();
    let f = floor(&mut h);
    let draws = || vec![Draw::new(f, Vec3::ZERO).colour(Vec3::ONE).roughness(1.0)];
    let off = h.render(&scene(draws(), None));
    let lit = h.render(&scene(draws(), on()));
    // The floor's own interior, byte for byte. At its outer edge, where the
    // depth jumps to the background, a grazing ray can pass behind the edge
    // within the thickness and count as a hit -- a handful of pixels (3 here),
    // the depth buffer's own ambiguity rather than light from anything.
    let mut interior = 0;
    for y in HEIGHT / 4..HEIGHT * 3 / 4 {
        for x in WIDTH / 4..WIDTH * 3 / 4 {
            assert_eq!(lit.at(x, y), off.at(x, y), "({x}, {y})");
            interior += 1;
        }
    }
    assert!(interior > 0);
    assert!(
        lit.differing_pixels(&off) <= 8,
        "only the edge: {} pixels",
        lit.differing_pixels(&off)
    );
    let disabled = h.render(&scene(
        draws(),
        Some(ScreenSpaceGlobalIllumination {
            enabled: false,
            ..Default::default()
        }),
    ));
    assert!(!disabled.differs_from(&off));
}
