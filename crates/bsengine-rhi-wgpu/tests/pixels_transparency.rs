//! Transparency, in pixels.
//!
//! The whole point of the feature is that what is behind a surface still shows
//! through it, which is exactly the kind of claim that cannot be checked
//! without reading the frame back.

mod common;

use common::{Draw, Harness, Light, Scene};
use glam::Vec3;

/// Flat white ambient and no sun, so these tests are about blending rather
/// than about shading.
fn flat() -> Light {
    Light {
        color: Vec3::ZERO,
        ambient: Vec3::ONE,
        ..Light::default()
    }
}

/// A wide red wall behind the origin.
fn backdrop(plane: u64) -> Draw {
    Draw::new(plane, Vec3::ZERO)
        .scaled(Vec3::new(30.0, 1.0, 30.0), Vec3::new(0.0, -1.5, 0.0))
        .colour(Vec3::new(1.0, 0.0, 0.0))
}

#[test]
fn a_half_transparent_surface_mixes_with_what_is_behind_it() {
    let mut h = Harness::new();
    let plane = h.plane();

    // A blue pane lying above a red floor, seen from overhead. Both are planes,
    // whose vertex colours are white, so the material colour is what shows.
    let pane = |opacity: f32| {
        Draw::new(plane, Vec3::ZERO)
            .scaled(Vec3::new(4.0, 1.0, 4.0), Vec3::new(0.0, 1.0, 0.0))
            .colour(Vec3::new(0.0, 0.0, 1.0))
            .opacity(opacity)
    };
    let camera_pos = Vec3::new(0.0, 8.0, 0.01);

    let solid = h.render(&Scene {
        draws: vec![backdrop(plane), pane(1.0)],
        light: flat(),
        camera_pos,
        ..Scene::default()
    });
    let glass = h.render(&Scene {
        draws: vec![backdrop(plane), pane(0.5)],
        light: flat(),
        camera_pos,
        ..Scene::default()
    });

    let [sr, _, _, _] = solid.centre();
    let [gr, _, gb, _] = glass.centre();
    assert!(
        gr > sr + 20,
        "red from the floor below should come through a half-transparent pane: \
         solid {:?}, glass {:?}",
        solid.centre(),
        glass.centre()
    );
    assert!(
        gb > 20,
        "and the pane's own blue should still be there, saw {:?}",
        glass.centre()
    );
}

#[test]
fn opacity_one_is_indistinguishable_from_an_opaque_draw() {
    let mut h = Harness::new();
    let cube = h.cube();

    // The regression that matters for every scene authored before this feature:
    // nothing that did not ask for transparency may change appearance.
    let implicit = h.render(&Scene {
        draws: vec![Draw::new(cube, Vec3::ZERO)],
        ..Scene::default()
    });
    let explicit = h.render(&Scene {
        draws: vec![Draw::new(cube, Vec3::ZERO).opacity(1.0)],
        ..Scene::default()
    });

    assert!(
        !implicit.differs_from(&explicit),
        "opacity 1.0 must take the opaque path and produce a byte-identical frame"
    );
}

#[test]
fn a_transparent_surface_is_hidden_by_an_opaque_one_in_front() {
    let mut h = Harness::new();
    let plane = h.plane();

    // Glass low down, an opaque green lid above it, camera overhead. With the
    // depth test off in the transparent pass, the glass would blend over the
    // lid that is in front of it.
    let glass = Draw::new(plane, Vec3::ZERO)
        .scaled(Vec3::new(4.0, 1.0, 4.0), Vec3::new(0.0, 0.0, 0.0))
        .colour(Vec3::new(0.0, 0.0, 1.0))
        .opacity(0.5);
    let lid = Draw::new(plane, Vec3::ZERO)
        .scaled(Vec3::new(4.0, 1.0, 4.0), Vec3::new(0.0, 2.0, 0.0))
        .colour(Vec3::new(0.0, 1.0, 0.0));

    let pixels = h.render(&Scene {
        draws: vec![glass, lid],
        light: flat(),
        camera_pos: Vec3::new(0.0, 8.0, 0.01),
        ..Scene::default()
    });

    let [r, g, b, _] = pixels.centre();
    assert!(
        g > b + 40 && g > r + 40,
        "the opaque lid in front should hide the glass under it, saw {}",
        pixels.describe()
    );
}

#[test]
fn two_transparent_panes_blend_in_depth_order() {
    let mut h = Harness::new();
    let plane = h.plane();

    // Two half-transparent panes over a red floor, one blue and one green. The
    // nearer one contributes more, so which colour dominates says whether the
    // sort ran back to front. Submitting them in the opposite order must give
    // the same picture -- that is the part a wrong sort fails.
    let blue_low = || {
        Draw::new(plane, Vec3::ZERO)
            .scaled(Vec3::new(4.0, 1.0, 4.0), Vec3::new(0.0, 0.5, 0.0))
            .colour(Vec3::new(0.0, 0.0, 1.0))
            .opacity(0.5)
    };
    let green_high = || {
        Draw::new(plane, Vec3::ZERO)
            .scaled(Vec3::new(4.0, 1.0, 4.0), Vec3::new(0.0, 2.0, 0.0))
            .colour(Vec3::new(0.0, 1.0, 0.0))
            .opacity(0.5)
    };
    let camera_pos = Vec3::new(0.0, 8.0, 0.01);

    let near_last = h.render(&Scene {
        draws: vec![backdrop(plane), blue_low(), green_high()],
        light: flat(),
        camera_pos,
        ..Scene::default()
    });
    let near_first = h.render(&Scene {
        draws: vec![backdrop(plane), green_high(), blue_low()],
        light: flat(),
        camera_pos,
        ..Scene::default()
    });

    assert!(
        !near_last.differs_from(&near_first),
        "submission order must not matter once the pass sorts by depth: \
         {} vs {}",
        near_last.describe(),
        near_first.describe()
    );

    // And the nearer (green) pane has to dominate, which is what "back to
    // front" buys. If the sort ran the other way the blue would win.
    let [_, g, b, _] = near_last.centre();
    assert!(
        g > b,
        "the nearer green pane should dominate the farther blue one, saw {}",
        near_last.describe()
    );
}
