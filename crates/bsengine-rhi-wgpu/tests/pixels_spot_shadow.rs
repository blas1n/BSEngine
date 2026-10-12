//! Spot light shadows, measured at the pixel.
//!
//! The fixture: a floor seen from straight above, lit by nothing but spot
//! lights -- no sun, no ambient, no bloom, no SSAO, tonemap `None` -- so a
//! dark pixel inside a cone can only be a shadow. Distances are on the floor
//! (y = 0); the camera at height 6 sees it at about 21.6 pixels per unit.

mod common;

use bsengine_core::{AmbientOcclusion, Bloom, ToneMap, ToneMappingMode};
use bsengine_rhi_wgpu::SpotLightEntry;
use common::{Draw, Harness, Pixels, Scene, HEIGHT, WIDTH};
use glam::{Quat, Vec3};

const CX: u32 = WIDTH / 2;
const CY: u32 = HEIGHT / 2;
/// Floor pixels per world unit at the camera's distance: 75 pixels over
/// 6 * tan(30 degrees).
const PX: f32 = 75.0 / (6.0 * 0.577_350_3);

/// The pixel `x` units along world +X from the centre (screen right).
fn at_x(x: f32) -> u32 {
    (CX as f32 + x * PX).round() as u32
}

fn floor(cube: u64) -> Draw {
    Draw::new(cube, Vec3::ZERO)
        .scaled(Vec3::new(40.0, 0.2, 40.0), Vec3::new(0.0, -0.1, 0.0))
        .colour(Vec3::ONE)
        .roughness(1.0)
}

/// A box of half-size `half` centred at `at`.
fn block(cube: u64, at: Vec3, half: Vec3) -> Draw {
    Draw::new(cube, Vec3::ZERO)
        .scaled(half * 2.0, at)
        .colour(Vec3::ONE)
}

/// A spot light at `position` pointing at `target`.
fn spot_at(position: Vec3, target: Vec3, outer_degrees: f32) -> SpotLightEntry {
    let direction = (target - position).normalize();
    let rotation = Quat::from_rotation_arc(Vec3::NEG_Z, direction);
    SpotLightEntry {
        position,
        direction,
        right: rotation * Vec3::X,
        color: Vec3::ONE,
        intensity: 6.0,
        range: 12.0,
        inner_angle: (outer_degrees - 1.0).to_radians(),
        outer_angle: outer_degrees.to_radians(),
        cookie: None,
    }
}

fn overhead(draws: Vec<Draw>, spots: Vec<SpotLightEntry>) -> Scene {
    Scene {
        draws,
        camera_pos: Vec3::new(0.0, 6.0, 0.01),
        look_at: Vec3::ZERO,
        light: common::Light {
            color: Vec3::ZERO,
            ambient: Vec3::ZERO,
            spots,
            ..common::Light::default()
        },
        bloom: Some(Bloom::default().disabled()),
        ssao: Some(AmbientOcclusion::default().disabled()),
        tone_map: Some(ToneMap::new(ToneMappingMode::None)),
        ..Scene::default()
    }
}

fn red(p: &Pixels, x: u32, y: u32) -> u8 {
    p.at(x, y)[0]
}

/// A block hanging between a downward spot light and the floor shades the
/// floor beneath it, where the same floor without the block is lit -- and
/// the floor beside the shadow stays as lit as it was. The light is 1 above
/// the block and 4 above the floor, so the block's half-width of 0.5 casts a
/// shadow 2 wide each side; 1.5 out is in it and still visible past the
/// block's own top, 2.8 out is beyond it and inside the cone.
#[test]
fn a_block_under_a_spot_light_shades_the_floor() {
    let mut h = Harness::new();
    let cube = h.cube();
    let light = || vec![spot_at(Vec3::new(0.0, 4.0, 0.0), Vec3::ZERO, 40.0)];
    let open = h.render(&overhead(vec![floor(cube)], light()));
    let blocked = h.render(&overhead(
        vec![
            floor(cube),
            block(cube, Vec3::new(0.0, 3.0, 0.0), Vec3::new(0.5, 0.1, 0.5)),
        ],
        light(),
    ));
    let (inside, outside) = (at_x(1.5), at_x(2.8));
    assert!(
        red(&open, inside, CY) > 80 && red(&open, outside, CY) > 40,
        "premise: without the block both are lit ({}, {})",
        red(&open, inside, CY),
        red(&open, outside, CY)
    );
    assert!(
        red(&blocked, inside, CY) < 5,
        "under the block, shadow: {}",
        red(&blocked, inside, CY)
    );
    assert!(
        red(&blocked, outside, CY).abs_diff(red(&open, outside, CY)) <= 3,
        "beyond it, lit as before: {} against {}",
        red(&blocked, outside, CY),
        red(&open, outside, CY)
    );
}

/// A lit floor with nothing over it is lit smoothly -- no shadow acne, the
/// speckle a too-small offset leaves where a surface shadows itself. Acne
/// lives on surfaces the light meets at an angle, where the map's depth
/// changes across each texel, so the light here comes in obliquely; the
/// floor's lighting then varies across the frame, but only gradually, and
/// no pixel differs from its neighbour by more than a few levels -- where
/// acne makes neighbours swing between lit and black.
///
/// Also the soft filter's hard case: with the filter off, acne is not
/// blurred away either.
#[test]
fn an_open_floor_has_no_shadow_acne() {
    let mut h = Harness::new();
    let cube = h.cube();
    for soft in [true, false] {
        let p = h.render(&Scene {
            soft_shadows: soft,
            ..overhead(
                vec![floor(cube)],
                vec![spot_at(
                    Vec3::new(3.0, 2.0, 0.0),
                    Vec3::new(-0.5, 0.0, 0.0),
                    50.0,
                )],
            )
        });
        let (mut lit, mut worst) = (0, 0u8);
        for y in CY - 25..CY + 25 {
            for x in at_x(-1.5)..at_x(1.0) {
                let (a, b) = (red(&p, x, y), red(&p, x + 1, y));
                if a > 20 {
                    lit += 1;
                }
                worst = worst.max(a.abs_diff(b));
            }
        }
        assert!(
            lit > 2000,
            "premise: the light covers the region (soft {soft}): {lit} lit"
        );
        assert!(
            worst <= 6,
            "smooth, no speckle (soft {soft}): neighbours differ by {worst}"
        );
    }
}

/// The soft filter softens spot shadows too: with `ShadowSettings::soft` the
/// shadow's edge is a gradient several pixels wide, without it a step. The
/// count of in-between pixels across the edge is what tells them apart.
#[test]
fn soft_shadows_soften_a_spot_shadows_edge() {
    let mut h = Harness::new();
    let cube = h.cube();
    let mut penumbra = |soft: bool| {
        let p = h.render(&Scene {
            soft_shadows: soft,
            // Close over the edge: the 3x3 filter spans about 0.04 units on
            // the floor here, a pixel from the usual overhead view and five
            // from one unit up. The edge is where the block's top, the widest
            // outline the light sees, projects: 0.5 * 4 / 0.9 = 2.22.
            camera_pos: Vec3::new(2.22, 1.0, 0.01),
            look_at: Vec3::new(2.22, 0.0, 0.0),
            ..overhead(
                vec![
                    floor(cube),
                    block(cube, Vec3::new(0.0, 3.0, 0.0), Vec3::new(0.5, 0.1, 0.5)),
                ],
                vec![spot_at(Vec3::new(0.0, 4.0, 0.0), Vec3::ZERO, 40.0)],
            )
        });
        // Across the frame, and so across the edge at its centre.
        let row: Vec<u8> = (0..WIDTH).map(|x| red(&p, x, CY)).collect();
        let lit = *row.iter().max().unwrap();
        assert!(
            lit > 60 && *row.iter().min().unwrap() < 5,
            "premise: the row crosses the edge"
        );
        row.iter()
            .filter(|&&v| v as f32 > lit as f32 * 0.15 && (v as f32) < lit as f32 * 0.85)
            .count()
    };
    let (soft, hard) = (penumbra(true), penumbra(false));
    assert!(
        soft >= hard + 2,
        "soft {soft} in-between pixels against hard {hard}"
    );
}

/// A probe bake on the very first frame -- before any spot shadow pass has
/// drawn into the maps -- sees the spot light unshadowed, as later bakes do.
/// A fresh depth texture is zeros, an occluder at the light itself, so maps
/// not cleared at creation would bake the light as entirely shadowed.
#[test]
fn a_first_frame_probe_bake_is_not_shadowed_by_empty_maps() {
    let mut h = Harness::new();
    let (plane, cube) = (h.plane(), h.cube());
    let probes = bsengine_core::LightProbeVolume {
        half_extents: Vec3::new(2.5, 1.5, 2.5).into(),
        resolution: [4, 2, 4],
    };
    let room = |intensity: f32, volume| Scene {
        camera_pos: Vec3::new(0.0, 2.0, 6.0),
        look_at: Vec3::new(0.0, -0.5, 0.0),
        light_probes: volume,
        ..overhead(
            vec![
                Draw::new(plane, Vec3::ZERO)
                    .scaled(Vec3::new(16.0, 1.0, 16.0), Vec3::new(0.0, -0.5, 0.0))
                    .roughness(0.9),
                Draw::new(cube, Vec3::ZERO)
                    .scaled(Vec3::new(0.3, 4.0, 8.0), Vec3::new(3.0, 1.5, 0.0))
                    .colour(Vec3::ONE)
                    .roughness(1.0),
            ],
            vec![SpotLightEntry {
                intensity,
                ..spot_at(Vec3::new(1.5, 1.5, 0.0), Vec3::new(3.0, 1.5, 0.0), 60.0)
            }],
        )
    };
    // The very first frame this surface draws bakes the probes.
    let first = h.render(&room(60.0, Some(probes))).centre()[1] as i32;
    // Warm: a frame without probes (the spot pass draws the map), then a
    // bake; and the same with the light off, for the bake's own baseline.
    h.render(&room(60.0, None));
    let warm = h.render(&room(60.0, Some(probes))).centre()[1] as i32;
    h.render(&room(0.0, None));
    let dark = h.render(&room(0.0, Some(probes))).centre()[1] as i32;
    assert!(
        warm - dark > 10,
        "premise: the warm bake carries the spot's bounce ({warm} vs {dark})"
    );
    assert!(
        (first - warm).abs() <= 3,
        "the first frame's bake matches it: {first} against {warm} (dark {dark})"
    );
}

/// The shadow starts where the caster stands: a box resting on the floor,
/// lit from the side, shades the floor right at its foot. A bias large
/// enough to lift shadows off their casters (peter-panning) leaves a lit gap
/// there. The light at (3, 4, 0) over a box 1 high casts the shadow from its
/// far face at x = -0.5 out to x = -1.67.
#[test]
fn a_shadow_starts_at_the_foot_of_its_caster() {
    let mut h = Harness::new();
    let cube = h.cube();
    let scene = |draws| {
        overhead(
            draws,
            vec![spot_at(
                Vec3::new(3.0, 4.0, 0.0),
                Vec3::new(-0.5, 0.0, 0.0),
                50.0,
            )],
        )
    };
    let open = h.render(&scene(vec![floor(cube)]));
    let boxed = h.render(&scene(vec![
        floor(cube),
        block(cube, Vec3::new(0.0, 0.5, 0.0), Vec3::splat(0.5)),
    ]));
    // The box's top hides the floor out to 0.6 seen from overhead.
    for x in [-0.75, -1.4] {
        assert!(
            red(&open, at_x(x), CY) > 40,
            "premise: lit at {x} without the box"
        );
        assert!(
            red(&boxed, at_x(x), CY) < 5,
            "shadowed at {x}, by its foot: {}",
            red(&boxed, at_x(x), CY)
        );
    }
    let beyond = at_x(-2.2);
    assert!(
        red(&boxed, beyond, CY).abs_diff(red(&open, beyond, CY)) <= 3,
        "and lit past the shadow's tip"
    );
}

/// Two spot lights have a map each: where their cones overlap, a block that
/// shades one light's floor leaves the other's light on it. With two lights
/// over the same floor, the point 0.8 from the left light's axis is in the
/// left block's shadow and well inside the right light's cone: lit by the
/// right light alone, exactly as much as when the right light is the only
/// one. Every light reading the first light's map would shade it from both.
///
/// The cones must overlap for this. Side by side, a light reading the wrong
/// map also reads it through the wrong matrix -- the layer picks both -- and
/// its own floor lands outside that frustum, where everything reads lit.
#[test]
fn each_spot_light_has_its_own_shadow_map() {
    let mut h = Harness::new();
    let cube = h.cube();
    let left = spot_at(Vec3::new(-1.0, 4.0, 0.0), Vec3::new(-1.0, 0.0, 0.0), 30.0);
    let right = spot_at(Vec3::new(1.0, 4.0, 0.0), Vec3::new(1.0, 0.0, 0.0), 30.0);
    // Over the left light: its shadow spans x = -1 +- 1.2 on the floor, and
    // the block's own top hides x = -1 +- 0.61 from the camera.
    let draws = || {
        vec![
            floor(cube),
            block(cube, Vec3::new(-1.0, 3.0, 0.0), Vec3::new(0.3, 0.05, 0.3)),
        ]
    };
    let x = at_x(-0.2);
    let both = h.render(&overhead(draws(), vec![left.clone(), right.clone()]));
    let left_only = h.render(&overhead(draws(), vec![left]));
    let right_only = h.render(&overhead(draws(), vec![right]));
    assert!(
        red(&left_only, x, CY) < 5,
        "premise: the left light's floor is shadowed there"
    );
    assert!(
        red(&right_only, x, CY) > 40,
        "premise: the right light reaches it"
    );
    assert!(
        red(&both, x, CY).abs_diff(red(&right_only, x, CY)) <= 3,
        "together, the right light's share is untouched: {} against {}",
        red(&both, x, CY),
        red(&right_only, x, CY)
    );
}

/// Terrain has its own spot light loop, and takes the shadow in it too.
#[test]
fn terrain_takes_spot_shadows() {
    let mut h = Harness::new();
    let cube = h.cube();
    let white = h.two_colour_texture([255; 4], [255; 4]);
    let weight = h.two_colour_texture([255, 0, 0, 255], [255, 0, 0, 255]);
    let mut render = |draws: Vec<Draw>| {
        h.render(&Scene {
            // The unit cube scaled flat: its top at y = 0.5.
            terrain: vec![(
                cube,
                glam::Mat4::from_scale(Vec3::new(20.0, 1.0, 20.0)),
                [white; 4],
                weight,
            )],
            camera_pos: Vec3::new(0.0, 6.5, 0.01),
            look_at: Vec3::new(0.0, 0.5, 0.0),
            ..overhead(
                draws,
                vec![spot_at(
                    Vec3::new(0.0, 4.5, 0.0),
                    Vec3::new(0.0, 0.5, 0.0),
                    40.0,
                )],
            )
        })
    };
    let open = render(vec![]);
    let blocked = render(vec![block(
        cube,
        Vec3::new(0.0, 3.5, 0.0),
        Vec3::new(0.5, 0.1, 0.5),
    )]);
    assert!(
        red(&open, at_x(1.5), CY) > 80,
        "premise: the terrain is lit"
    );
    assert!(
        red(&blocked, at_x(1.5), CY) < 5,
        "and shaded under the block: {}",
        red(&blocked, at_x(1.5), CY)
    );
}

/// Light probes are baked with spot shadows: a white wall lit by a spot
/// light bounces light onto the floor through the probes, and a block
/// covering the light's whole cone takes that bounce away. A bake that
/// ignored the shadow would still see the wall lit.
#[test]
fn probes_are_baked_with_spot_shadows() {
    let mut h = Harness::new();
    let (plane, cube) = (h.plane(), h.cube());
    let probes = bsengine_core::LightProbeVolume {
        half_extents: Vec3::new(2.5, 1.5, 2.5).into(),
        resolution: [4, 2, 4],
    };
    let room = |intensity: f32, blocked: bool, volume| {
        let mut draws = vec![
            Draw::new(plane, Vec3::ZERO)
                .scaled(Vec3::new(16.0, 1.0, 16.0), Vec3::new(0.0, -0.5, 0.0))
                .roughness(0.9),
            Draw::new(cube, Vec3::ZERO)
                .scaled(Vec3::new(0.3, 4.0, 8.0), Vec3::new(3.0, 1.5, 0.0))
                .colour(Vec3::ONE)
                .roughness(1.0),
        ];
        if blocked {
            // Just in front of the light, across its whole cone. Black: its
            // own lit face looks back into the room, and a white one would
            // bounce light onto the floor itself.
            draws.push(
                block(cube, Vec3::new(1.9, 1.5, 0.0), Vec3::new(0.05, 1.0, 1.0)).colour(Vec3::ZERO),
            );
        }
        Scene {
            camera_pos: Vec3::new(0.0, 2.0, 6.0),
            look_at: Vec3::new(0.0, -0.5, 0.0),
            light_probes: volume,
            ..overhead(
                draws,
                vec![SpotLightEntry {
                    intensity,
                    ..spot_at(Vec3::new(1.5, 1.5, 0.0), Vec3::new(3.0, 1.5, 0.0), 60.0)
                }],
            )
        }
    };
    // What the probes add to the floor with the light on, less what they add
    // with it off: the bake lights the floor somewhat with no light at all
    // (72 levels here), and only the rest is the spot light's bounce.
    let mut probe_light = |intensity: f32, blocked: bool| {
        let off = h.render(&room(intensity, blocked, None));
        let on = h.render(&room(intensity, blocked, Some(probes)));
        on.centre()[1] as i32 - off.centre()[1] as i32
    };
    let dark = probe_light(0.0, false);
    let mut bounce = |blocked: bool| probe_light(60.0, blocked) - dark;
    let open = bounce(false);
    assert!(
        open > 10,
        "premise: the lit wall bounces light onto the floor ({open})"
    );
    let blocked = bounce(true);
    assert!(
        blocked < open / 4,
        "the shadowed wall bounces almost none: {blocked} against {open}"
    );
}
