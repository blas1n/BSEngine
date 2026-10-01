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

/// The form factor from the wall's centre (normal +z) to a panel lying flat
/// 0.5 above it, facing down, spanning `z0..z1` in front of the wall's face
/// and 1 wide -- by brute-force quadrature, since the closed forms for
/// perpendicular rectangles are easy to get wrong.
fn perpendicular_form_factor(z0: f32, z1: f32) -> f32 {
    let (nx, nz) = (400, 200);
    let (dx, dz) = (1.0 / nx as f32, (z1 - z0) / nz as f32);
    let mut sum = 0.0;
    for i in 0..nx {
        for k in 0..nz {
            let r = Vec3::new(
                -0.5 + (i as f32 + 0.5) * dx,
                0.5,
                z0 + (k as f32 + 0.5) * dz,
            );
            let d2 = r.length_squared();
            let cos_wall = r.z / d2.sqrt();
            let cos_panel = r.y / d2.sqrt();
            sum += cos_wall * cos_panel / (PI * d2) * dx * dz;
        }
    }
    sum
}

/// Horizon clipping: a panel lying flat above the wall's centre, half of it
/// sunk behind the wall, lights the centre exactly as its front half alone
/// does -- the half behind is below the surface's horizon, and an
/// unclipped integral would count it. Both match the front half's form
/// factor, integrated numerically. A panel wholly behind the wall lights
/// nothing.
#[test]
fn the_part_of_a_panel_below_the_horizon_does_not_light() {
    let mut h = Harness::new();
    let cube = h.cube();
    // Local -z turned to face down, local y running into the wall.
    let down = Quat::from_rotation_x(-PI / 2.0);
    let flat = |z_centre: f32, depth: f32| RectLightEntry {
        position: Vec3::new(0.0, 0.5, z_centre),
        half_width: down * Vec3::X * 0.5,
        half_height: down * Vec3::Y * (depth * 0.5),
        color: Vec3::ONE,
        intensity: 1.0,
        range: 100.0,
    };
    // The frame's exact centre lies between four pixels (its size is even
    // both ways), and the light here changes fast with height -- the centre
    // row's pixel is half a pixel, 0.025 units, below it, which is 10% of
    // this panel's light. Their average is the centre's to second order.
    let mut centre = |rect: RectLightEntry| {
        let p = h.render(&scene(vec![wall(cube, 1.0, 0.0)], vec![rect]));
        let (x, y) = CENTRE;
        (linear(&p, x - 1, y - 1) + linear(&p, x, y - 1) + linear(&p, x - 1, y) + linear(&p, x, y))
            / 4.0
    };
    let straddling = centre(flat(WALL_Z, 1.0));
    let front_half = centre(flat(WALL_Z + 0.25, 0.5));
    let behind = centre(flat(WALL_Z - 0.3, 0.5));
    let expected = perpendicular_form_factor(0.0, 0.5);
    assert!(
        (front_half - expected).abs() < expected * 0.05,
        "the front half lights the centre by its form factor: {front_half} against {expected}"
    );
    assert!(
        (straddling - front_half).abs() < front_half * 0.02,
        "half sunk behind the wall, as much as its front half: {straddling} against {front_half}"
    );
    assert_eq!(behind, 0.0, "wholly behind the wall, nothing");
}

/// The form factor from the wall's centre to the part in front of the wall
/// of a 1 x 1 panel lying flat 0.5 above it, facing down, centred
/// `offset` in front of the wall's face and turned `turn` about the
/// vertical -- by quadrature over the whole panel, counting only the
/// points in front.
fn clipped_form_factor(offset: f32, turn: f32) -> f32 {
    let rot = Quat::from_rotation_y(turn);
    let (u_axis, v_axis) = (rot * Vec3::X, rot * Vec3::NEG_Z);
    let n = 400;
    let step = 1.0 / n as f32;
    let mut sum = 0.0;
    for i in 0..n {
        for k in 0..n {
            let (u, v) = (
                -0.5 + (i as f32 + 0.5) * step,
                -0.5 + (k as f32 + 0.5) * step,
            );
            let r = Vec3::new(0.0, 0.5, offset) + u * u_axis + v * v_axis;
            if r.z <= 0.0 {
                continue;
            }
            let d2 = r.length_squared();
            sum += (r.z / d2.sqrt()) * (r.y / d2.sqrt()) / (PI * d2) * step * step;
        }
    }
    sum
}

/// Every way a rectangle can cross the horizon: the same flat panel sunk
/// into the wall turned through eight directions -- along a side (two
/// corners in front, four ways round) and corner first, pushed out (three
/// corners in front) and pushed in (one). Each leaves a different clipped
/// polygon, from three corners to five, and each must light the centre by
/// the form factor of what is left in front.
#[test]
fn every_way_a_panel_crosses_the_horizon_is_clipped_right() {
    let mut h = Harness::new();
    let cube = h.cube();
    let mut cases = Vec::new();
    for k in 0..4 {
        cases.push((k as f32 * PI / 2.0, 0.0));
        cases.push((PI / 4.0 + k as f32 * PI / 2.0, 0.25));
        cases.push((PI / 4.0 + k as f32 * PI / 2.0, -0.25));
    }
    for (turn, offset) in cases {
        let rot = Quat::from_rotation_y(turn);
        let rect = RectLightEntry {
            position: Vec3::new(0.0, 0.5, WALL_Z + offset),
            half_width: rot * Vec3::X * 0.5,
            half_height: rot * Vec3::NEG_Z * 0.5,
            color: Vec3::ONE,
            intensity: 1.0,
            range: 100.0,
        };
        let p = h.render(&scene(vec![wall(cube, 1.0, 0.0)], vec![rect]));
        let (x, y) = CENTRE;
        let got = (linear(&p, x - 1, y - 1)
            + linear(&p, x, y - 1)
            + linear(&p, x - 1, y)
            + linear(&p, x, y))
            / 4.0;
        let expected = clipped_form_factor(offset, turn);
        assert!(
            (got - expected).abs() < expected * 0.05 + 0.002,
            "turned {:.0} deg, {offset} in front: {got} against {expected}",
            turn.to_degrees()
        );
    }
}

/// A metal has no diffuse: a rough white metal wall shows only its specular
/// lobe, a fraction of what the same white wall reflects diffusely.
#[test]
fn a_metal_has_no_diffuse_term() {
    let mut h = Harness::new();
    let cube = h.cube();
    let mut lit = |metallic: f32| {
        let p = h.render(&scene(
            vec![wall(cube, 1.0, metallic)],
            vec![panel(1.0, 1.0, 1.0, 1.0, Quat::IDENTITY)],
        ));
        linear(&p, CENTRE.0, CENTRE.1)
    };
    let (dielectric, metal) = (lit(0.0), lit(1.0));
    assert!(
        metal > 0.0 && metal < dielectric * 0.5,
        "metal {metal} against dielectric {dielectric}"
    );
}
