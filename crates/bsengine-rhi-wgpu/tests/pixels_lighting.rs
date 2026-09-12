//! Lighting and shadows.
//!
//! These are the tests that found the bug they now guard. The shadow
//! comparison sampler asked for `GreaterEqual` while the shadow pass writes
//! ordinary forward-Z depth, so `shadow_factor` returned 0 for every fragment
//! inside the shadow frustum -- and since the light matrix at the time centred
//! a 60-unit box on the world origin, that was every object in every game
//! here. (That box is gone: the frustum now follows the camera. The bug it
//! helped hide is what these tests still guard.) The sun
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
    floor_at(mesh, Vec3::ZERO)
}

/// The same floor, centred anywhere.
fn floor_at(mesh: u64, origin: Vec3) -> Draw {
    Draw::new(mesh, origin).scaled(Vec3::new(20.0, 1.0, 20.0), origin)
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

/// The strongest darkening a cube casts onto floor it does not cover, with
/// the whole fixture — floor, cube and camera — placed at `origin`.
///
/// Parameterised by position because that is the property at stake. The
/// directional shadow frustum used to be a fixed box on the world origin, so
/// this measurement was only ever taken where it happened to work. Taking the
/// identical measurement somewhere else is what tells a shadow that follows
/// the camera from one that does not.
fn darkening_beside_a_caster(origin: Vec3) -> (f32, u32, u32, Pixels, Pixels) {
    let cam_offset = CAMERA;
    let mut h = Harness::new();
    let plane = h.plane();
    let cube = h.cube();
    let caster = || Draw::new(cube, origin + Vec3::new(0.0, 3.0, 0.0));
    let scene = |draws: Vec<Draw>| Scene {
        draws,
        light: sun_from_above(),
        camera_pos: origin + cam_offset,
        look_at: origin,
        ..Scene::default()
    };

    // Which pixels the cube itself covers, so "the floor got darker" cannot be
    // satisfied by the cube merely standing in front of it.
    let empty = h.render(&scene(vec![]));
    let cube_only = h.render(&scene(vec![caster()]));
    let silhouette: Vec<bool> = (0..(empty.width * empty.height))
        .map(|i| {
            let (x, y) = (i % empty.width, i / empty.width);
            empty.at(x, y) != cube_only.at(x, y)
        })
        .collect();

    let open = h.render(&scene(vec![floor_at(plane, origin)]));
    let occluded = h.render(&scene(vec![floor_at(plane, origin), caster()]));
    let (delta, x, y) = biggest_darkening_off_the_caster(&open, &occluded, &silhouette);
    (delta, x, y, open, occluded)
}

#[test]
fn an_occluder_darkens_floor_it_does_not_cover() {
    let (delta, x, y, open, occluded) = darkening_beside_a_caster(Vec3::ZERO);
    assert!(
        delta > 60.0,
        "the cube should darken floor it is not standing in front of; the strongest \
         darkening away from its silhouette was {delta} at ({x}, {y}), open {:?} \
         occluded {:?}",
        open.at(x, y),
        occluded.at(x, y)
    );
}

/// A caster far enough away to fall in a later cascade must still shadow.
///
/// This is the test that makes cascade *selection* observable. At the default
/// 200-unit shadow distance, clamped to this harness's 100-unit camera, the
/// boundaries sit at 6.25, 25, 56.25 and 100 units — so a caster ~80 units
/// down the view axis belongs to the last cascade. A shader that ignored the
/// split and always sampled cascade 0 would find this fragment far outside
/// that cascade's map and report it lit, with no shadow at all.
///
/// Written because exactly that mutation passed all seven of the other tests
/// in this file: every one of them places its camera 7 units from its caster,
/// where cascade 0 happens to be the right answer.
///
/// The fixture is deliberately not the shared one. Its first version reused
/// `darkening_beside_a_caster` with a distant camera and measured a darkening
/// of 1 — not because cascades were broken (it failed with a single cascade
/// too) but because a unit cube 80 units away covers a handful of pixels and
/// its straight-down shadow falls entirely inside the silhouette the
/// measurement masks out. A big caster and an angled sun put the shadow
/// somewhere a test can see it.
#[test]
fn a_distant_caster_is_shadowed_by_a_later_cascade() {
    let mut h = Harness::new();
    let plane = h.plane();
    let cube = h.cube();

    // Angled so the shadow lands beside the caster rather than under it, and
    // large so both cover real pixels at this range.
    let sun = || Light {
        direction: Vec3::new(-0.45, -1.0, 0.0).normalize(),
        ambient: Vec3::splat(0.05),
        ..Light::default()
    };
    let camera_pos = Vec3::new(0.0, 40.0, 70.0);
    let at = Vec3::new(0.0, 8.0, 0.0);
    let caster = || Draw::new(cube, at).scaled(Vec3::splat(6.0), at);
    let scene = |draws: Vec<Draw>| Scene {
        draws,
        light: sun(),
        camera_pos,
        look_at: Vec3::ZERO,
        ..Scene::default()
    };

    let empty = h.render(&scene(vec![]));
    let cube_only = h.render(&scene(vec![caster()]));
    let silhouette: Vec<bool> = (0..(empty.width * empty.height))
        .map(|i| {
            let (x, y) = (i % empty.width, i / empty.width);
            empty.at(x, y) != cube_only.at(x, y)
        })
        .collect();

    let open = h.render(&scene(vec![floor(plane)]));
    let occluded = h.render(&scene(vec![floor(plane), caster()]));
    let (delta, x, y) = biggest_darkening_off_the_caster(&open, &occluded, &silhouette);
    assert!(
        delta > 30.0,
        "a caster ~80 units from the camera falls in the last cascade and must \
         still shadow the floor beside it; the strongest darkening away from its \
         silhouette was only {delta} at ({x}, {y}), open {:?} occluded {:?}. \
         Near-zero here means the shader sampled a cascade that does not cover \
         this depth.",
        open.at(x, y),
        occluded.at(x, y)
    );
}

/// The cascade cross-fade must change what is drawn.
///
/// Neighbouring cascades have different texel densities, so an unfaded
/// boundary reads as a step in shadow sharpness at a fixed distance from the
/// player. The fade is the engine's default (following Unreal, where Unity and
/// Godot both default it off), so something has to observe that it runs at all
/// — otherwise the setting, the uniform field and the shader's `mix` are all
/// dead code that no test would notice.
///
/// A large caster spanning a boundary, rendered with the fade off and on: the
/// two frames must differ somewhere.
#[test]
fn the_cascade_cross_fade_changes_the_image() {
    let mut h = Harness::new();
    let plane = h.plane();
    let cube = h.cube();
    let sun = || Light {
        direction: Vec3::new(-0.45, -1.0, 0.0).normalize(),
        ambient: Vec3::splat(0.05),
        ..Light::default()
    };
    // Long and low, so the caster's shadow stretches across a cascade
    // boundary rather than sitting inside one cascade.
    let camera_pos = Vec3::new(0.0, 12.0, 60.0);
    let at = Vec3::new(0.0, 6.0, 0.0);
    let scene = |blend: f32| Scene {
        draws: vec![
            floor(plane),
            Draw::new(cube, at).scaled(Vec3::new(4.0, 4.0, 30.0), at),
        ],
        light: sun(),
        camera_pos,
        look_at: Vec3::ZERO,
        shadow_blend: blend,
        ..Scene::default()
    };

    let hard = h.render(&scene(0.0));
    let faded = h.render(&scene(0.5));
    let differing = (0..(hard.width * hard.height))
        .filter(|i| {
            let (x, y) = (i % hard.width, i / hard.width);
            (hard.luma(x, y) - faded.luma(x, y)).abs() > 2.0
        })
        .count();
    assert!(
        differing > 0,
        "turning the cascade cross-fade from 0 to 0.5 must change some pixel; \
         not one of {} differed by more than 2 luma, which means the blend \
         branch never runs",
        hard.width * hard.height
    );
}

/// The defect that prompted the camera-following fit, at the level of pixels.
///
/// The shadow frustum was a 60-unit box centred on `Vec3::ZERO`, so a caster
/// standing outside it cast nothing at all. `games/scale-level` is authored
/// across x = 0..152 and most of it fell outside — the shipped demo had no
/// directional shadows over four fifths of its length, at any resolution.
#[test]
fn a_caster_far_from_the_world_origin_still_shadows() {
    // Beyond the old box by a wide margin, and inside the range the level
    // actually uses.
    let origin = Vec3::new(140.0, 0.0, -60.0);
    let (delta, x, y, open, occluded) = darkening_beside_a_caster(origin);
    assert!(
        delta > 60.0,
        "a cube at {origin:?} should shadow the floor exactly as one at the world \
         origin does; the strongest darkening away from its silhouette was only \
         {delta} at ({x}, {y}), open {:?} occluded {:?}. Near-zero here means the \
         shadow frustum did not follow the camera.",
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

/// Which pixels either cube covers, for the two-caster test below.
///
/// Same idea as [`cube_silhouette`], but for both positions at once, so a
/// "the floor got darker" reading can never be satisfied by a cube merely
/// standing in front of the floor.
fn two_cube_silhouette(h: &mut Harness, cube: u64, a: Vec3, b: Vec3) -> Vec<bool> {
    let empty = h.render(&Scene {
        light: sun_from_above(),
        camera_pos: CAMERA,
        ..Scene::default()
    });
    let cubes_only = h.render(&Scene {
        draws: vec![Draw::new(cube, a), Draw::new(cube, b)],
        light: sun_from_above(),
        camera_pos: CAMERA,
        ..Scene::default()
    });
    (0..(empty.width * empty.height))
        .map(|i| {
            let (x, y) = (i % empty.width, i / empty.width);
            empty.at(x, y) != cubes_only.at(x, y)
        })
        .collect()
}

/// Two objects of the *same mesh* each cast a shadow in their own place.
///
/// **This is the only test that can see a wrong instance-to-slot mapping.**
/// The shadow passes batch objects sharing a mesh into one instanced draw,
/// and every instance looks up its own transform through a slot array. If
/// that lookup is wrong -- if every instance reads the batch's first slot --
/// then all the cubes cast one shadow stacked in one place.
///
/// Nothing else in the suite notices. Draw-call counts are identical either
/// way, so the instancing statistics tests are blind to it by construction,
/// and every other shadow test here uses a single caster, where "the batch's
/// first slot" and "my slot" are the same thing. Running that mutation
/// against the whole crate left all 19 suites green before this test existed.
///
/// The two frames it compares are chosen so the mutation cannot hide:
/// `only_b` has one cube, so its batch has one slot and even a broken lookup
/// finds the right transform; `both` has two, where a broken lookup drops B's
/// shadow onto A. So B's shadow is measured in a frame that is correct
/// regardless, then required to still be there in the frame that is not.
#[test]
fn each_instance_of_a_shared_mesh_casts_its_own_shadow() {
    let mut h = Harness::new();
    let plane = h.plane();
    let cube = h.cube();

    let a = Vec3::new(-2.5, 3.0, 0.0);
    let b = Vec3::new(2.5, 3.0, 0.0);
    let scene = |draws: Vec<Draw>| Scene {
        draws,
        light: sun_from_above(),
        camera_pos: CAMERA,
        ..Scene::default()
    };

    let mask = two_cube_silhouette(&mut h, cube, a, b);
    let open = h.render(&scene(vec![floor(plane)]));
    let only_b = h.render(&scene(vec![floor(plane), Draw::new(cube, b)]));
    let both = h.render(&scene(vec![
        floor(plane),
        Draw::new(cube, a),
        Draw::new(cube, b),
    ]));

    // Where B's shadow falls, established in the one-cube frame.
    let (delta_b, bx, by) = biggest_darkening_off_the_caster(&open, &only_b, &mask);
    assert!(
        delta_b > 40.0,
        "sanity: one cube on its own must cast a visible shadow before this test \
         can say anything about two. Strongest darkening was {delta_b} at ({bx}, {by})"
    );

    // ...and it must still be there when A is added beside it.
    let delta_both = open.luma(bx, by) - both.luma(bx, by);
    assert!(
        delta_both > delta_b * 0.5,
        "B's shadow vanished when A was added: the floor at ({bx}, {by}) darkened by \
         {delta_b} with B alone but only {delta_both} with both cubes. Both cubes share \
         a mesh, so they batch into one instanced shadow draw -- this is what it looks \
         like when every instance reads the same slot and they all cast A's shadow"
    );

    // And the paired direction: A's shadow must be there too, so a bug that
    // dropped B's onto A cannot pass by symmetry.
    let only_a = h.render(&scene(vec![floor(plane), Draw::new(cube, a)]));
    let (delta_a, ax, ay) = biggest_darkening_off_the_caster(&open, &only_a, &mask);
    assert!(
        delta_a > 40.0,
        "sanity: cube A alone must cast a visible shadow; got {delta_a} at ({ax}, {ay})"
    );
    let delta_both_a = open.luma(ax, ay) - both.luma(ax, ay);
    assert!(
        delta_both_a > delta_a * 0.5,
        "A's shadow vanished when B was added: floor at ({ax}, {ay}) darkened by \
         {delta_a} with A alone but only {delta_both_a} with both"
    );
}

/// The point-shadow half of [`each_instance_of_a_shared_mesh_casts_its_own_shadow`].
///
/// Found by enumerating this feature's surface and asking which items had a
/// consumer-side assertion. The `slots[0u]` mutation breaks *both* shadow
/// shaders, but only the directional test above caught it -- so breaking
/// only the point-shadow fetch would have gone unnoticed. The two passes
/// batch independently (the point pass batches a per-light culled subset,
/// the directional pass batches everything), so one test cannot stand in
/// for the other.
#[test]
fn each_instance_casts_its_own_point_light_shadow() {
    let mut h = Harness::new();
    let plane = h.plane();
    let cube = h.cube();
    let camera_pos = Vec3::new(0.0, 6.0, 7.0);

    let a = Vec3::new(-2.0, 2.0, 0.0);
    let b = Vec3::new(2.0, 2.0, 0.0);

    // Directional light off entirely, so only the point light can brighten
    // anything and only its cube shadow map can darken it.
    let light = || Light {
        color: Vec3::ZERO,
        ambient: Vec3::splat(0.02),
        points: vec![PointLight {
            position: Vec3::new(0.0, 6.0, 0.0),
            color: Vec3::ONE,
            intensity: 90.0,
            range: 40.0,
        }],
        ..Light::default()
    };
    let scene = |draws: Vec<Draw>| Scene {
        draws,
        light: light(),
        camera_pos,
        ..Scene::default()
    };

    let empty = h.render(&scene(vec![]));
    let cubes_only = h.render(&scene(vec![Draw::new(cube, a), Draw::new(cube, b)]));
    let silhouette: Vec<bool> = (0..(empty.width * empty.height))
        .map(|i| {
            let (x, y) = (i % empty.width, i / empty.width);
            empty.at(x, y) != cubes_only.at(x, y)
        })
        .collect();

    let open = h.render(&scene(vec![floor(plane)]));
    let only_b = h.render(&scene(vec![floor(plane), Draw::new(cube, b)]));
    let both = h.render(&scene(vec![
        floor(plane),
        Draw::new(cube, a),
        Draw::new(cube, b),
    ]));

    let (delta_b, bx, by) = biggest_darkening_off_the_caster(&open, &only_b, &silhouette);
    assert!(
        delta_b > 20.0,
        "sanity: one cube under a point light must cast a visible shadow before this \
         test can say anything about two; strongest darkening was {delta_b} at ({bx}, {by})"
    );

    let delta_both = open.luma(bx, by) - both.luma(bx, by);
    assert!(
        delta_both > delta_b * 0.5,
        "B's point-light shadow vanished when A was added: floor at ({bx}, {by}) darkened \
         by {delta_b} with B alone but only {delta_both} with both cubes. The point-shadow \
         pass batches same-mesh casters into one instanced draw per cube face -- this is \
         what it looks like when every instance reads the same slot"
    );
}
