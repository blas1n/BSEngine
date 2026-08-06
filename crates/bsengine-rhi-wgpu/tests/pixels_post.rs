//! Bloom, SSAO and tone mapping: does the composite stage change pixels?

mod common;

use common::{Draw, Harness, Light, Scene};
use glam::Vec3;

#[test]
fn tone_mapping_changes_a_bright_surface() {
    let mut h = Harness::new();
    let cube = h.cube();
    // Bright enough to need mapping down. On a surface that never exceeds 1.0
    // an HDR curve has nothing to do, and the test would compare two identical
    // frames and call that a pass.
    let bright = || Draw::new(cube, Vec3::ZERO).emissive(Vec3::splat(4.0));

    let off = h.render(&Scene {
        draws: vec![bright()],
        tone_map: Some(bsengine_core::ToneMap {
            enabled: false,
            ..Default::default()
        }),
        ..Scene::default()
    });
    let on = h.render(&Scene {
        draws: vec![bright()],
        tone_map: Some(bsengine_core::ToneMap {
            enabled: true,
            ..Default::default()
        }),
        ..Scene::default()
    });

    assert!(
        on.centre() != off.centre(),
        "tone mapping should change a surface bright enough to need it; both read {:?}",
        on.centre()
    );
}

#[test]
fn bloom_brightens_the_pixels_around_a_bright_object() {
    let mut h = Harness::new();
    let cube = h.cube();

    // Small and very bright. The measurement has to be taken *outside* the
    // object: sampling on it would only confirm that something already bright
    // is still bright, which is true with bloom disabled too.
    let spark = || {
        Draw::new(cube, Vec3::ZERO)
            .scaled(Vec3::splat(0.35), Vec3::ZERO)
            .emissive(Vec3::splat(8.0))
    };
    let scene = |bloom: bsengine_core::Bloom| Scene {
        draws: vec![spark()],
        bloom: Some(bloom),
        light: Light {
            ambient: Vec3::ZERO,
            ..Light::default()
        },
        ..Scene::default()
    };

    let off = h.render(&scene(bsengine_core::Bloom {
        enabled: false,
        ..Default::default()
    }));
    let on = h.render(&scene(bsengine_core::Bloom {
        intensity: 1.0,
        enabled: true,
        ..Default::default()
    }));

    // Where the spill is strongest anywhere the object is not, rather than
    // guessing a distance that depends on the object's screen size.
    //
    // The mask is "differs from an empty frame", not "is bright". A brightness
    // threshold picked the whole frame the first time this was written -- the
    // background clear colour reads about 85 -- which left no pixels to search
    // and made the test report no spill anywhere.
    let empty = h.render(&Scene {
        light: Light {
            ambient: Vec3::ZERO,
            ..Light::default()
        },
        ..Scene::default()
    });
    let object = |x: u32, y: u32| empty.at(x, y) != off.at(x, y);

    let mut best = (0.0_f32, 0_u32, 0_u32);
    for y in 0..on.height {
        for x in 0..on.width {
            if object(x, y) {
                continue;
            }
            let delta = on.luma(x, y) - off.luma(x, y);
            if delta > best.0 {
                best = (delta, x, y);
            }
        }
    }

    assert!(
        best.0 > 2.0,
        "bloom should spill light onto pixels the object does not cover; the \
         strongest brightening off the object was {} at ({}, {})",
        best.0,
        best.1,
        best.2
    );
}

#[test]
fn ssao_darkens_where_geometry_meets_geometry() {
    let mut h = Harness::new();
    let cube = h.cube();
    let plane = h.plane();

    // A cube sitting *on* the floor, so there is an interior corner to occlude.
    // A floating cube has no contact edge and SSAO would have nothing to do.
    let draws = || {
        vec![
            Draw::new(plane, Vec3::ZERO).scaled(Vec3::new(20.0, 1.0, 20.0), Vec3::ZERO),
            Draw::new(cube, Vec3::new(0.0, 0.5, 0.0)),
        ]
    };
    let camera_pos = Vec3::new(3.0, 2.0, 3.0);

    let off = h.render(&Scene {
        draws: draws(),
        ssao: Some(bsengine_core::AmbientOcclusion {
            enabled: false,
            ..Default::default()
        }),
        camera_pos,
        ..Scene::default()
    });
    let on = h.render(&Scene {
        draws: draws(),
        ssao: Some(bsengine_core::AmbientOcclusion {
            enabled: true,
            ..Default::default()
        }),
        camera_pos,
        ..Scene::default()
    });

    assert!(
        on.differs_from(&off),
        "SSAO should change the frame where the cube meets the floor"
    );

    // And it must darken rather than brighten: an effect that changed pixels in
    // the wrong direction would satisfy the assertion above.
    let darkened = (0..on.height)
        .flat_map(|y| (0..on.width).map(move |x| (x, y)))
        .filter(|&(x, y)| on.luma(x, y) < off.luma(x, y) - 1.0)
        .count();
    let brightened = (0..on.height)
        .flat_map(|y| (0..on.width).map(move |x| (x, y)))
        .filter(|&(x, y)| on.luma(x, y) > off.luma(x, y) + 1.0)
        .count();
    assert!(
        darkened > brightened,
        "ambient occlusion should darken: {darkened} pixels darker, {brightened} brighter"
    );
}
