//! Rect lights, measured at the pixel.
//!
//! The fixture: a white wall 5 units in front of the camera, lit by nothing
//! but rect lights -- no sun, no ambient, no bloom, no SSAO, tonemap `None`
//! -- so each pixel's linear value is exactly what the rect lights put there,
//! and can be held against the analytic form factor of a rectangle.

mod common;

use bsengine_core::{AmbientOcclusion, Bloom, ToneMap, ToneMappingMode};
use bsengine_rhi_wgpu::area_light::RectLightEntry;
use common::{Draw, Harness, Light, Pixels, Scene, HEIGHT, WIDTH};
use glam::{Quat, Vec3};
use std::f32::consts::PI;

/// The wall's front face.
const WALL_Z: f32 = -4.9;

fn wall(cube: u64, roughness: f32, metallic: f32) -> Draw {
    Draw::new(cube, Vec3::ZERO)
        .scaled(Vec3::new(40.0, 40.0, 0.2), Vec3::new(0.0, 0.0, -5.0))
        .colour(Vec3::ONE)
        .roughness(roughness)
        .metallic(metallic)
}

/// A `width` x `height` panel `distance` in front of the wall, facing it
/// (its local -z is the world's -z), turned by `rotation` about its centre.
fn panel(width: f32, height: f32, distance: f32, intensity: f32, rotation: Quat) -> RectLightEntry {
    RectLightEntry {
        position: Vec3::new(0.0, 0.0, WALL_Z + distance),
        half_width: rotation * Vec3::X * (width * 0.5),
        half_height: rotation * Vec3::Y * (height * 0.5),
        color: Vec3::ONE,
        intensity,
        range: 100.0,
    }
}

fn scene(draws: Vec<Draw>, rects: Vec<RectLightEntry>) -> Scene {
    Scene {
        draws,
        light: Light {
            color: Vec3::ZERO,
            ambient: Vec3::ZERO,
            rects,
            ..Light::default()
        },
        bloom: Some(Bloom::default().disabled()),
        ssao: Some(AmbientOcclusion::default().disabled()),
        tone_map: Some(ToneMap::new(ToneMappingMode::None)),
        ..Scene::default()
    }
}

/// A pixel's red channel back in linear light.
fn linear(p: &Pixels, x: u32, y: u32) -> f32 {
    let c = p.at(x, y)[0] as f32 / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// The form factor from a point to a parallel `a` x `b` rectangle at
/// distance `d`, the point under one corner (Howell's catalogue, B-3).
fn corner_form_factor(a: f32, b: f32, d: f32) -> f32 {
    let (x, y) = (a / d, b / d);
    let (sx, sy) = ((1.0 + x * x).sqrt(), (1.0 + y * y).sqrt());
    (x / sx * (y / sx).atan() + y / sy * (x / sy).atan()) / (2.0 * PI)
}

/// Under the centre of a `w` x `h` panel: four corner rectangles.
fn centre_form_factor(w: f32, h: f32, d: f32) -> f32 {
    4.0 * corner_form_factor(w / 2.0, h / 2.0, d)
}

const CENTRE: (u32, u32) = (WIDTH / 2, HEIGHT / 2);

/// The wall straight across from the panel's centre is lit by exactly the
/// panel's radiance times its form factor -- the closed form for a parallel
/// rectangle -- whatever its size, shape and distance. A rough wall, so the
/// little specular there is stays inside the 3% this allows. The case a
/// point light cannot pass: halving the distance does not quadruple the
/// light from a panel this close.
#[test]
fn the_wall_receives_the_rectangles_form_factor() {
    let mut h = Harness::new();
    let cube = h.cube();
    for (w, ht, d, intensity) in [
        (1.0, 1.0, 1.0, 1.0),
        (0.5, 0.5, 1.0, 1.0),
        (1.0, 1.0, 2.0, 2.0),
        (2.0, 0.5, 1.0, 1.0),
    ] {
        let p = h.render(&scene(
            vec![wall(cube, 1.0, 0.0)],
            vec![panel(w, ht, d, intensity, Quat::IDENTITY)],
        ));
        let got = linear(&p, CENTRE.0, CENTRE.1);
        let expected = intensity * centre_form_factor(w, ht, d);
        assert!(
            (got - expected).abs() < expected * 0.03,
            "{w} x {ht} panel {d} away at intensity {intensity}: {got} against {expected}"
        );
    }
}

/// One-sided: turned to face away, the panel lights nothing in front of it.
#[test]
fn a_panel_lights_nothing_behind_it() {
    let mut h = Harness::new();
    let cube = h.cube();
    let lit = h.render(&scene(
        vec![wall(cube, 1.0, 0.0)],
        vec![panel(1.0, 1.0, 1.0, 1.0, Quat::IDENTITY)],
    ));
    assert!(
        linear(&lit, CENTRE.0, CENTRE.1) > 0.1,
        "premise: facing the wall, it lights it"
    );
    let turned = h.render(&scene(
        vec![wall(cube, 1.0, 0.0)],
        vec![panel(1.0, 1.0, 1.0, 1.0, Quat::from_rotation_y(PI))],
    ));
    assert_eq!(turned.at(CENTRE.0, CENTRE.1)[0], 0, "turned away, nothing");
}

/// A glossy wall reflects the panel's shape: a long thin panel makes a long
/// thin highlight, along its width -- and turned a quarter, across. A point
/// light's highlight is round whichever way it is turned.
#[test]
fn a_glossy_surface_reflects_the_panels_shape() {
    let mut h = Harness::new();
    let cube = h.cube();
    let mut extent = |rotation: Quat| {
        let p = h.render(&scene(
            vec![wall(cube, 0.25, 1.0)],
            vec![panel(2.0, 0.2, 1.5, 1.0, rotation)],
        ));
        let wide = (0..WIDTH).filter(|&x| p.at(x, CENTRE.1)[0] > 128).count();
        let tall = (0..HEIGHT).filter(|&y| p.at(CENTRE.0, y)[0] > 128).count();
        (wide, tall)
    };
    let (wide, tall) = extent(Quat::IDENTITY);
    assert!(
        wide >= 3 * tall && tall > 0,
        "a wide panel, a wide highlight: {wide} x {tall}"
    );
    let (wide, tall) = extent(Quat::from_rotation_z(PI / 2.0));
    assert!(
        tall >= 3 * wide && wide > 0,
        "turned a quarter, a tall one: {wide} x {tall}"
    );
}

/// The light fades out by its range: the wall 1 unit away is lit with a
/// range of 10, less with a range of 1.2 (the window is already closing),
/// and not at all with a range of 0.9.
#[test]
fn the_light_fades_out_at_its_range() {
    let mut h = Harness::new();
    let cube = h.cube();
    let mut at_range = |range: f32| {
        let p = h.render(&scene(
            vec![wall(cube, 1.0, 0.0)],
            vec![RectLightEntry {
                range,
                ..panel(1.0, 1.0, 1.0, 1.0, Quat::IDENTITY)
            }],
        ));
        linear(&p, CENTRE.0, CENTRE.1)
    };
    let (far, near, short) = (at_range(10.0), at_range(1.2), at_range(0.9));
    assert!(far > 0.1, "premise: in range, lit: {far}");
    assert!(
        near < far * 0.8 && near > 0.0,
        "fading near the range: {near} of {far}"
    );
    assert_eq!(short, 0.0, "out of range, nothing");
}

/// Four rect lights are shaded, and a fifth is not: five identical panels
/// light the wall exactly four times as much as one.
#[test]
fn four_rect_lights_are_shaded_and_a_fifth_is_not() {
    let mut h = Harness::new();
    let cube = h.cube();
    let one = panel(0.5, 0.5, 1.0, 1.0, Quat::IDENTITY);
    let mut lit = |n: usize| {
        let p = h.render(&scene(vec![wall(cube, 1.0, 0.0)], vec![one; n]));
        linear(&p, CENTRE.0, CENTRE.1)
    };
    let (single, four, five) = (lit(1), lit(4), lit(5));
    assert!(
        (four - 4.0 * single).abs() < single * 0.1,
        "four panels, four times the light: {four} against {single}"
    );
    assert_eq!(five, four, "the fifth is past the cap");
}

/// Terrain is lit by rect lights as meshes are.
#[test]
fn terrain_is_lit_by_rect_lights() {
    let mut h = Harness::new();
    let cube = h.cube();
    let white = h.two_colour_texture([255, 255, 255, 255], [255, 255, 255, 255]);
    let weight = h.two_colour_texture([255, 0, 0, 255], [255, 0, 0, 255]);
    let terrain_wall = (
        cube,
        glam::Mat4::from_scale_rotation_translation(
            Vec3::new(40.0, 40.0, 0.2),
            Quat::IDENTITY,
            Vec3::new(0.0, 0.0, -5.0),
        ),
        [white; 4],
        weight,
    );
    let with = |rects| Scene {
        terrain: vec![terrain_wall],
        ..scene(Vec::new(), rects)
    };
    let dark = h.render(&with(Vec::new()));
    let lit = h.render(&with(vec![panel(1.0, 1.0, 1.0, 1.0, Quat::IDENTITY)]));
    assert_eq!(dark.at(CENTRE.0, CENTRE.1)[0], 0, "premise: unlit, black");
    assert!(
        linear(&lit, CENTRE.0, CENTRE.1) > 0.1,
        "the panel lights the terrain: {:?}",
        lit.at(CENTRE.0, CENTRE.1)
    );
}
