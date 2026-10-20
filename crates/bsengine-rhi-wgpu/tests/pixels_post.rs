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

/// SSAO leaves alone what nothing occludes: an open floor, and the floor
/// around a cube floating well clear of it. The test above is the easy side --
/// "something got darker" -- and the effect passed it while darkening every
/// flat surface in view (its normal pointed away from the camera, so each
/// surface hid half its own samples) and printing offset copies of every
/// object onto the wall behind it (eight fixed sample directions, the same at
/// every pixel, and no blur): the translucent discs around the spheres in
/// games/gi-demo, and the bands along its walls.
///
/// Looking down at the floor, with a cube two metres above it -- four times
/// the default radius, so no sample from the floor can reach it. Every floor
/// pixel the cube does not cover or shade must read the same with SSAO on as
/// off.
#[test]
fn ssao_leaves_open_and_unoccluded_surfaces_alone() {
    let mut h = Harness::new();
    let cube = h.cube();
    let plane = h.plane();
    let floor = || Draw::new(plane, Vec3::ZERO).scaled(Vec3::new(20.0, 1.0, 20.0), Vec3::ZERO);
    let scene = |draws: Vec<Draw>, enabled: bool| Scene {
        draws,
        // Four times the default strength, so that a little wrongly
        // occluded floor shows as more than a level or two.
        ssao: Some(bsengine_core::AmbientOcclusion {
            enabled,
            intensity: 4.0,
            ..Default::default()
        }),
        camera_pos: Vec3::new(0.0, 6.0, 0.5),
        look_at: Vec3::ZERO,
        ..Scene::default()
    };
    let with_cube = || vec![floor(), Draw::new(cube, Vec3::new(0.0, 2.0, 0.0))];

    let bare = h.render(&scene(vec![floor()], false));
    let off = h.render(&scene(with_cube(), false));
    let on = h.render(&scene(with_cube(), true));

    // The floor pixels: the same with and without the cube (so neither the
    // cube nor its shadow is on them).
    let floor_px = |x: u32, y: u32| bare.at(x, y) == off.at(x, y);
    let cube_px = |x: u32, y: u32| !floor_px(x, y);
    // Premise: the floor pixels include ones right beside the cube's
    // silhouette -- where the copies landed -- not only the open floor.
    let beside = (0..on.height)
        .flat_map(|y| (0..on.width).map(move |x| (x, y)))
        .filter(|&(x, y)| {
            floor_px(x, y)
                && (x.saturating_sub(3)..(x + 4).min(on.width)).any(|nx| {
                    (y.saturating_sub(3)..(y + 4).min(on.height)).any(|ny| cube_px(nx, ny))
                })
        })
        .count();
    assert!(
        beside > 50,
        "premise: floor pixels beside the cube, found {beside}"
    );

    let mut worst = (0.0_f32, 0, 0);
    for y in 0..on.height {
        for x in 0..on.width {
            if !floor_px(x, y) {
                continue;
            }
            let d = (on.luma(x, y) - off.luma(x, y)).abs();
            if d > worst.0 {
                worst = (d, x, y);
            }
        }
    }
    assert!(
        worst.0 <= 2.0,
        "SSAO changed an unoccluded floor pixel by {} at ({}, {})",
        worst.0,
        worst.1,
        worst.2
    );
}

/// The blur that smooths the occlusion stays on its own surface. A low slab on
/// the floor puts a ring of occlusion on the floor round its base; a red cube
/// floating two metres up is placed so that, seen from above, the edge of its
/// top face lies right against that ring. Nothing occludes that top face, so
/// it must read the same with SSAO on as off -- a blur that averaged across
/// the depth jump would pull the ring's darkness onto the face's edge. Where
/// the ring is comes from frames without the red cube, so the premise does not
/// depend on the blur under test.
#[test]
fn ssao_blur_does_not_bleed_across_a_depth_jump() {
    let mut h = Harness::new();
    let cube = h.cube();
    let plane = h.plane();
    let draws = |floating: bool| {
        let mut d = vec![
            Draw::new(plane, Vec3::ZERO).scaled(Vec3::new(20.0, 1.0, 20.0), Vec3::ZERO),
            // A slab lower than the radius, so its edge occludes the floor
            // even seen from straight above, where the depth buffer shows
            // only its top.
            Draw::new(cube, Vec3::ZERO).scaled(Vec3::new(1.0, 0.3, 1.0), Vec3::new(0.0, 0.15, 0.0)),
        ];
        if floating {
            d.push(Draw::new(cube, FLOAT_AT).colour(Vec3::new(1.0, 0.0, 0.0)));
        }
        d
    };
    let scene = |enabled: bool, floating: bool| Scene {
        draws: draws(floating),
        // Four times the default strength: the ring is a few levels deep at
        // its outer edge, where the face meets it, and a bleed of a fraction of
        // that would hide under the tolerance.
        ssao: Some(bsengine_core::AmbientOcclusion {
            enabled,
            intensity: 4.0,
            ..Default::default()
        }),
        camera_pos: Vec3::new(0.0, 7.0, 0.5),
        look_at: Vec3::ZERO,
        ..Scene::default()
    };
    let off = h.render(&scene(false, true));
    let on = h.render(&scene(true, true));
    // Where the ring is, found without the floating cube -- so the premise
    // does not depend on the very blur under test.
    let ring_off = h.render(&scene(false, false));
    let ring_on = h.render(&scene(true, false));

    let red = |x: u32, y: u32| {
        let p = off.at(x, y);
        p[0] as i32 > p[1] as i32 + 40
    };
    let ring = |x: u32, y: u32| ring_on.luma(x, y) < ring_off.luma(x, y) - 3.0;
    let near = |x: u32, y: u32, f: &dyn Fn(u32, u32) -> bool| {
        (x.saturating_sub(2)..(x + 3).min(on.width))
            .any(|nx| (y.saturating_sub(2)..(y + 3).min(on.height)).any(|ny| f(nx, ny)))
    };
    // Premise: occluded floor right against the red face, within the blur's
    // reach of it.
    let touching = (0..on.height)
        .flat_map(|y| (0..on.width).map(move |x| (x, y)))
        .filter(|&(x, y)| red(x, y) && near(x, y, &|nx, ny| !red(nx, ny) && ring(nx, ny)))
        .count();
    assert!(
        touching > 5,
        "premise: the red face's edge against occluded floor, {touching} pixels"
    );

    let mut worst = (0.0_f32, 0, 0);
    for y in 0..on.height {
        for x in 0..on.width {
            if red(x, y) {
                // The red channel: on a red face luma weighs it by a fifth.
                let d = off.at(x, y)[0] as f32 - on.at(x, y)[0] as f32;
                if d > worst.0 {
                    worst = (d, x, y);
                }
            }
        }
    }
    assert!(
        worst.0 <= 2.0,
        "the floating face was darkened by {} at ({}, {})",
        worst.0,
        worst.1,
        worst.2
    );
}

/// Where the floating cube of the test above sits: seen from above, the edge
/// of its top face lies against the slab's ring of occlusion.
const FLOAT_AT: Vec3 = Vec3::new(0.85, 2.5, 0.0);

/// Occlusion is the same on both sides of a scene that is. A slab on the floor
/// seen from above, the camera on its plane of symmetry: the darkening round
/// the slab must match its own mirror image, row by row. That is what the
/// per-pixel kernel rotations, averaged by the blur, buy: the sixteen
/// rotations are a mirror-symmetric set and every pixel ends up with all of
/// them. One fixed set of directions is not symmetric -- the ring came out
/// heavier on one side, in steps -- and the rotations without the blur leave
/// each pixel its own one rotation, a dither pattern that does not mirror.
/// Compared as on/off ratios, so the sun's own asymmetry cancels.
#[test]
fn ssao_is_as_symmetric_as_the_scene() {
    let mut h = Harness::new();
    let cube = h.cube();
    let plane = h.plane();
    let scene = |enabled: bool| Scene {
        draws: vec![
            Draw::new(plane, Vec3::ZERO).scaled(Vec3::new(20.0, 1.0, 20.0), Vec3::ZERO),
            Draw::new(cube, Vec3::ZERO).scaled(Vec3::new(1.0, 0.3, 1.0), Vec3::new(0.0, 0.15, 0.0)),
        ],
        ssao: Some(bsengine_core::AmbientOcclusion {
            enabled,
            intensity: 4.0,
            ..Default::default()
        }),
        camera_pos: Vec3::new(0.0, 7.0, 0.5),
        look_at: Vec3::ZERO,
        ..Scene::default()
    };
    let off = h.render(&scene(false));
    let on = h.render(&scene(true));
    let w = on.width;
    let ratio = |x: u32, y: u32| on.luma(x, y) / off.luma(x, y);
    // Pixels the frame without SSAO already shows symmetric -- not the slab's
    // shadow, which falls on one side, and through the tonemap's curve turns
    // the same occlusion into a different ratio -- and lit (a ratio of two
    // near-zero lumas is noise).
    let usable = |x: u32, y: u32| {
        off.luma(x, y) > 20.0 && (off.luma(x, y) - off.luma(w - 1 - x, y)).abs() <= 1.0
    };

    // Per row: the occlusion summed over the left half against the right.
    // A sum, not pixel against pixel: the blur's 4x4 window is half a pixel
    // off centre, so a sharp edge of the ring lands on different pixels on
    // the two sides.
    let mut occluded = 0;
    let mut worst_row = (0.0_f32, 0);
    // And within the ring, how much each pixel differs from its right-hand
    // neighbour, on average: a smooth darkening, or a pattern.
    let (mut step_sum, mut step_n) = (0.0_f32, 0);
    for y in 0..on.height {
        let (mut left, mut right) = (0.0_f32, 0.0_f32);
        for x in 0..w / 2 {
            if !usable(x, y) {
                continue;
            }
            let (l, r) = (1.0 - ratio(x, y), 1.0 - ratio(w - 1 - x, y));
            left += l;
            right += r;
            if l > 0.1 {
                occluded += 1;
                if x + 1 < w / 2 && usable(x + 1, y) {
                    step_sum += (ratio(x, y) - ratio(x + 1, y)).abs();
                    step_n += 1;
                }
            }
        }
        let d = (left - right).abs();
        if d > worst_row.0 {
            worst_row = (d, y);
        }
    }
    let step = step_sum / step_n.max(1) as f32;
    // Premise: the ring is there to compare, on this half.
    assert!(
        occluded > 100,
        "premise: occluded pixels on the left half, {occluded}"
    );
    // Measured: 0.57 here; 1.99 with one fixed set of directions, 2.29 with
    // no blur.
    assert!(
        worst_row.0 <= 1.2,
        "row {} is occluded {} more on one side than the other",
        worst_row.1,
        worst_row.0
    );
    // Measured: 0.056 here; 0.23 with no blur, each pixel showing its own
    // rotation.
    assert!(
        step <= 0.12,
        "the occlusion steps by {step} from pixel to pixel: a pattern, not a darkening"
    );
}
