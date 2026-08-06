//! Textures, custom shaders and the skybox: does the material side of the
//! pipeline reach the framebuffer?

mod common;

use common::{Draw, Harness, Light, Scene};
use glam::Vec3;

/// Straight down at a floor, which is the one mesh with white vertex colours --
/// `cube_vertices` paints each face a different colour, and those multiply into
/// the albedo, which would drown out anything a texture said.
const OVERHEAD: Vec3 = Vec3::new(0.0, 6.0, 0.01);

fn even_light() -> Light {
    Light {
        direction: Vec3::new(0.0, -1.0, 0.0),
        ambient: Vec3::splat(0.2),
        ..Light::default()
    }
}

#[test]
fn a_two_colour_texture_lands_with_its_halves_the_right_way_round() {
    let mut h = Harness::new();
    let plane = h.plane();
    let tex = h.two_colour_texture([255, 0, 0, 255], [0, 0, 255, 255]);

    let pixels = h.render(&Scene {
        draws: vec![Draw::new(plane, Vec3::ZERO)
            .scaled(Vec3::new(6.0, 1.0, 6.0), Vec3::ZERO)
            .textured(tex)],
        light: even_light(),
        camera_pos: OVERHEAD,
        ..Scene::default()
    });

    let left = pixels.at(pixels.width / 4, pixels.height / 2);
    let right = pixels.at(pixels.width * 3 / 4, pixels.height / 2);
    assert!(
        left[0] > left[2] + 20 && right[2] > right[0] + 20,
        "expected red on one side and blue on the other, got {left:?} and {right:?}"
    );
}

#[test]
fn a_custom_shader_replaces_the_standard_pipeline() {
    let mut h = Harness::new();
    let cube = h.cube();
    let shader = h.constant_colour_shader([0.0, 1.0, 0.0], "assets/shaders/test_constant.wgsl");

    // The cube's own front face is red, and the material is left white, so
    // anything the standard pipeline draws here comes out red. Green can only
    // be the custom shader's doing.
    let standard = h.render(&Scene {
        draws: vec![Draw::new(cube, Vec3::ZERO)],
        ..Scene::default()
    });
    let custom = h.render(&Scene {
        draws: vec![Draw::new(cube, Vec3::ZERO).shader(&shader)],
        ..Scene::default()
    });

    let [sr, sg, _, _] = standard.centre();
    assert!(
        sr > sg,
        "the standard pipeline should draw the cube's red front face, saw {}",
        standard.describe()
    );
    let [cr, cg, cb, _] = custom.centre();
    assert!(
        cg > cr + 40 && cg > cb + 40,
        "the custom shader's green should reach the framebuffer, saw {}",
        custom.describe()
    );
}

#[test]
fn a_skybox_fills_the_background_instead_of_the_clear_colour() {
    let mut h = Harness::new();
    let without = h.render(&Scene::default());

    h.set_test_skybox([255, 0, 255, 255]);
    let with_sky = h.render(&Scene {
        with_skybox: true,
        ..Scene::default()
    });

    assert!(
        with_sky.differs_from(&without),
        "background pixels should be the sky rather than the clear colour"
    );
    let [r, g, b, _] = with_sky.centre();
    assert!(
        r > g + 40 && b > g + 40,
        "expected the magenta test sky, saw {}",
        with_sky.describe()
    );
}
