//! Decals that change the surface normal, not just its colour.
//!
//! A colour decal is a sticker; a normal decal is a dent. The difference only
//! shows in the *shading*, so every test here compares two frames whose decals
//! differ in nothing but the normal map -- the albedo is identical, so anything
//! that moves is the lighting responding to a normal that changed.
//!
//! The decal carries its own tangent frame, taken from its box, which is why
//! this works on surfaces that have no tangents of their own. Terrain is the
//! case that matters: it is built from a heightmap and has never had them.

mod common;

use bsengine_rhi_wgpu::decals::DecalDraw;
use common::{Draw, Harness, Light, Pixels, Scene};
use glam::{Mat4, Vec3};

fn floor(mesh: u64) -> Draw {
    Draw::new(mesh, Vec3::ZERO).scaled(Vec3::new(20.0, 1.0, 20.0), Vec3::ZERO)
}

/// A decal covering the floor, with whatever normal map is handed in.
fn decal(colour: u64, normal: Option<u64>) -> DecalDraw {
    DecalDraw {
        model: Mat4::from_translation(Vec3::new(0.0, 0.5, 0.0))
            * Mat4::from_scale(Vec3::new(8.0, 2.0, 8.0)),
        opacity: 1.0,
        normal_fade: 0.0,
        texture: Some(colour),
        normal_texture: normal,
    }
}

/// A light coming in at an angle, so tilting a normal changes how much of it
/// the surface catches.
///
/// Straight overhead would be the worst possible fixture here: a normal tilted
/// in the plane the light sits in changes `n · l` barely at all, and the test
/// would be measuring rounding.
fn scene(mesh: u64, decals: Vec<DecalDraw>) -> Scene {
    Scene {
        draws: vec![floor(mesh)],
        decals,
        camera_pos: Vec3::new(0.0, 4.0, 7.0),
        look_at: Vec3::ZERO,
        light: Light {
            direction: Vec3::new(-0.8, -0.6, 0.0).normalize(),
            ..Light::default()
        },
        ..Default::default()
    }
}

/// A uniform normal-map texture encoding one tangent-space direction.
///
/// `(0.5, 0.5, 1.0)` is `(0, 0, 1)` -- straight out of the surface, the "no
/// dent" value every normal map is flat at.
fn normal_texture(h: &mut Harness, t: Vec3) -> u64 {
    let enc = |v: f32| ((v * 0.5 + 0.5) * 255.0).round().clamp(0.0, 255.0) as u8;
    let texel = [enc(t.x), enc(t.y), enc(t.z), 255];
    h.two_colour_texture(texel, texel)
}

#[test]
fn a_tilted_normal_map_changes_the_shading() {
    let mut h = Harness::new();
    let cube = h.cube();
    let white = h.two_colour_texture([255, 255, 255, 255], [255, 255, 255, 255]);
    let flat = normal_texture(&mut h, Vec3::new(0.0, 0.0, 1.0));
    // Tilted hard towards the decal's local -X, which is across the light.
    let tilted = normal_texture(&mut h, Vec3::new(-0.8, 0.0, 0.6).normalize());

    let with_flat = h.render(&scene(cube, vec![decal(white, Some(flat))]));
    let with_tilted = h.render(&scene(cube, vec![decal(white, Some(tilted))]));

    let flat_luma = with_flat.centre_luma();
    let tilted_luma = with_tilted.centre_luma();
    assert!(
        (flat_luma - tilted_luma).abs() > 0.05,
        "the same decal with a tilted normal map has to shade differently -- \
         the albedo is identical in both, so nothing else can move: \
         flat {flat_luma:.3}, tilted {tilted_luma:.3}"
    );
}

#[test]
fn a_flat_normal_map_leaves_the_surface_normal_alone() {
    // The property that keeps every colour-only decal rendering as it did:
    // a decal with no normal map is bound a flat one, and a flat one has to be
    // indistinguishable from not touching the normal at all.
    let mut h = Harness::new();
    let cube = h.cube();
    let white = h.two_colour_texture([255, 255, 255, 255], [255, 255, 255, 255]);
    let flat = normal_texture(&mut h, Vec3::new(0.0, 0.0, 1.0));

    let no_map = h.render(&scene(cube, vec![decal(white, None)]));
    let with_flat = h.render(&scene(cube, vec![decal(white, Some(flat))]));

    assert!(
        with_flat.max_channel_diff(&no_map) <= 1,
        "a flat normal map and no normal map must agree -- they mean the same \
         thing: {}",
        with_flat.describe()
    );
}

#[test]
fn the_normal_only_changes_where_the_decal_is() {
    // Paired with the first test. Without it, a normal buffer that leaked
    // outside the decal -- or one the mesh shader read with the wrong sign --
    // would relight the whole floor and still "shade differently".
    let mut h = Harness::new();
    let cube = h.cube();
    let white = h.two_colour_texture([255, 255, 255, 255], [255, 255, 255, 255]);
    let tilted = normal_texture(&mut h, Vec3::new(-0.8, 0.0, 0.6).normalize());

    let plain = h.render(&scene(cube, vec![]));
    let dented = h.render(&scene(
        cube,
        vec![DecalDraw {
            // A small box in the middle, so most of the floor is outside it.
            model: Mat4::from_translation(Vec3::new(0.0, 0.5, 0.0))
                * Mat4::from_scale(Vec3::new(2.0, 2.0, 2.0)),
            ..decal(white, Some(tilted))
        }],
    ));

    let changed = dented.differing_pixels(&plain);
    let total = (plain.width * plain.height) as usize;
    assert!(
        changed > 0,
        "the dent has to show at all: {}",
        dented.describe()
    );
    assert!(
        changed < total / 3,
        "a 2-unit box on a 20-unit floor must not relight most of the frame: \
         {changed} of {total} pixels changed"
    );
}

#[test]
fn a_normal_decal_lands_on_terrain_too() {
    // Terrain has no tangents -- it is generated from a heightmap -- so a
    // material normal map could not work there at all. A decal's frame comes
    // from its own box, which is exactly what makes this possible.
    let mut h = Harness::new();
    let plane = h.cube();
    let white = h.two_colour_texture([255, 255, 255, 255], [255, 255, 255, 255]);
    let flat = normal_texture(&mut h, Vec3::new(0.0, 0.0, 1.0));
    let tilted = normal_texture(&mut h, Vec3::new(-0.8, 0.0, 0.6).normalize());
    let layers = [
        h.two_colour_texture([200, 200, 200, 255], [200, 200, 200, 255]),
        h.two_colour_texture([200, 200, 0, 255], [200, 200, 0, 255]),
        h.two_colour_texture([0, 0, 200, 255], [0, 0, 200, 255]),
        h.two_colour_texture([200, 0, 200, 255], [200, 0, 200, 255]),
    ];
    let weight = h.two_colour_texture([255, 0, 0, 255], [255, 0, 0, 255]);

    let terrain_scene = |decals: Vec<DecalDraw>| Scene {
        terrain: vec![(
            plane,
            Mat4::from_scale(Vec3::new(20.0, 1.0, 20.0)),
            layers,
            weight,
        )],
        draws: Vec::new(),
        ..scene(plane, decals)
    };

    let with_flat = h.render(&terrain_scene(vec![DecalDraw {
        model: Mat4::from_scale(Vec3::new(8.0, 2.0, 8.0)),
        ..decal(white, Some(flat))
    }]));
    let with_tilted = h.render(&terrain_scene(vec![DecalDraw {
        model: Mat4::from_scale(Vec3::new(8.0, 2.0, 8.0)),
        ..decal(white, Some(tilted))
    }]));

    let (a, b) = (with_flat.centre_luma(), with_tilted.centre_luma());
    assert!(
        (a - b).abs() > 0.05,
        "terrain has to respond to a decal normal the same way a mesh does: \
         flat {a:.3}, tilted {b:.3}"
    );
}

#[test]
fn a_normal_decal_whose_box_misses_the_floor_changes_no_shading() {
    // The normal half of `a_decal_is_bounded_by_its_box`. A decal writes its
    // normal into a screen-space buffer, so without the coverage weight it
    // reaches every pixel its *silhouette* covers -- including surfaces behind
    // it that its box never touches. That relights them, and nothing about the
    // colour would show it.
    //
    // The box has to be big enough to rasterise; a thinner one produces no
    // fragments from this camera and the test would pass for the wrong reason.
    let mut h = Harness::new();
    let cube = h.cube();
    let white = h.two_colour_texture([255, 255, 255, 255], [255, 255, 255, 255]);
    let tilted = normal_texture(&mut h, Vec3::new(-0.8, 0.0, 0.6).normalize());

    let plain = h.render(&scene(cube, vec![]));
    let floating = h.render(&scene(
        cube,
        vec![DecalDraw {
            model: Mat4::from_translation(Vec3::new(0.0, 3.0, 0.0))
                * Mat4::from_scale(Vec3::new(4.0, 2.0, 4.0)),
            ..decal(white, Some(tilted))
        }],
    ));

    assert!(
        !floating.differs_from(&plain),
        "a decal floating above the floor must not relight it: {}",
        floating.describe()
    );
}

/// Kept honest: the fixture's light must actually make a tilt visible.
///
/// If the light were straight overhead, tilting the normal in the plane it
/// sits in would barely change `n · l`, and every test above would be
/// measuring rounding rather than shading.
#[test]
fn the_fixture_lights_the_floor_well_enough_to_see_a_tilt() {
    let mut h = Harness::new();
    let cube = h.cube();
    let plain: Pixels = h.render(&scene(cube, vec![]));
    let luma = plain.centre_luma();
    assert!(
        luma > 0.1,
        "an unlit floor cannot show a normal change: centre luma {luma:.3}"
    );
}
