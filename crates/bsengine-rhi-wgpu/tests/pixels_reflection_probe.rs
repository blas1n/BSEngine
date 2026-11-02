//! Reflection probes, measured at the pixel.
//!
//! Every scene here is lit by nothing but emissive panels: the sun and the
//! ambient term are black, and there is no skybox. So a mirror floor with no
//! probe reflects nothing and reads black, and every colour it shows with a
//! probe is light the probe captured. The measurements are per-channel
//! *biases* (one channel against the mean of the other two), as in
//! `pixels_light_probe.rs`: a probe that merely brightened the floor would
//! leave them where they were.
//!
//! The capture only happens when the probe list differs from the one last
//! captured -- by design, see `render_frame` -- so any test that reuses a
//! probe list across two different scenes first renders one with no probes,
//! which empties that cache.

mod common;

use bsengine_rhi_wgpu::ReflectionProbeParams;
use common::{Draw, Harness, Light, Pixels, Scene};
use glam::Vec3;

const RED: Vec3 = Vec3::new(1.0, 0.0, 0.0);
const GREEN: Vec3 = Vec3::new(0.0, 1.0, 0.0);
const WHITE: Vec3 = Vec3::ONE;

/// Radiance of the ceiling panels, in pre-tonemap units: bright enough that
/// a mirror's reflection of them is tens of 8-bit levels, not so bright the
/// reflected channel clips (a clipped channel cannot show a difference).
const PANEL_RADIANCE: f32 = 3.0;

/// Height of the panels' underside.
const CEILING_Y: f32 = 4.0;

/// Black sun, black ambient: the panels are the only light.
fn dark() -> Light {
    Light {
        color: Vec3::ZERO,
        ambient: Vec3::ZERO,
        ..Light::default()
    }
}

/// A mirror floor at y = 0 and the given ceiling panels, the camera looking
/// steeply down at floor point `target` so the centre pixel is floor, and
/// the reflection of that pixel travels up to the ceiling rather than off
/// past its edge.
fn mirror_room(
    plane: u64,
    cube: u64,
    panels: &[(f32, f32, Vec3)],
    target: Vec3,
    probes: Vec<ReflectionProbeParams>,
) -> Scene {
    let mut draws = vec![Draw::new(plane, Vec3::ZERO)
        .scaled(Vec3::new(40.0, 1.0, 40.0), Vec3::ZERO)
        .colour(WHITE)
        .metallic(1.0)
        // Not 0: the direct-light GGX term divides by the roughness, and a
        // black light times an infinite peak is NaN, not black.
        .roughness(0.05)];
    for &(x_min, x_max, colour) in panels {
        draws.push(
            Draw::new(cube, Vec3::ZERO)
                .scaled(
                    Vec3::new(x_max - x_min, 0.2, 40.0),
                    Vec3::new(0.5 * (x_min + x_max), CEILING_Y + 0.1, 0.0),
                )
                .colour(Vec3::ZERO)
                .emissive(colour * PANEL_RADIANCE),
        );
    }
    Scene {
        draws,
        light: dark(),
        // Under the ceiling, which the camera would otherwise see from
        // above; steep enough that the reflected ray reaches the ceiling
        // at z = -2.7, well inside the panels.
        camera_pos: target + Vec3::new(0.0, 3.0, 2.0),
        look_at: target,
        reflection_probes: probes,
        ..Scene::default()
    }
}

fn probe(center: Vec3, half_extents: Vec3) -> ReflectionProbeParams {
    ReflectionProbeParams {
        center,
        half_extents,
        box_projection: false,
        intensity: 1.0,
        realtime: false,
        // A hard edge: these tests are about which probe governs a
        // surface, the blend tests below about how they hand over.
        blend_distance: 0.0,
    }
}

/// How far channel `c` stands above the mean of the other two.
fn bias(p: [u8; 4], c: usize) -> f32 {
    let v = [p[0] as f32, p[1] as f32, p[2] as f32];
    v[c] - 0.5 * (v[(c + 1) % 3] + v[(c + 2) % 3])
}

fn luma(p: [u8; 4]) -> f32 {
    (p[0] as f32 + p[1] as f32 + p[2] as f32) / 3.0
}

const R: usize = 0;
const G: usize = 1;

fn floor(p: &Pixels) -> [u8; 4] {
    p.centre()
}

/// The basic contract: a mirror inside a probe's box reflects what the probe
/// captured, and without the probe reflects nothing. The "nothing" is the
/// premise -- if the floor were already red without a probe, a red floor
/// with one would prove nothing about the probe.
#[test]
fn a_mirror_floor_reflects_the_room_the_probe_captured() {
    let mut h = Harness::new();
    let (plane, cube) = (h.plane(), h.cube());
    let panels = [(-20.0, 20.0, RED)];
    let room = |probes| mirror_room(plane, cube, &panels, Vec3::ZERO, probes);

    let off = floor(&h.render(&room(Vec::new())));
    let on = floor(&h.render(&room(vec![probe(
        Vec3::new(0.0, 2.0, 0.0),
        Vec3::new(8.0, 3.0, 8.0),
    )])));

    assert!(
        luma(off) < 8.0,
        "premise: with no probe, no sky and no light, the mirror floor reflects nothing: {off:?}"
    );
    assert!(
        bias(on, R) > 40.0,
        "inside the probe's box the mirror should show the red ceiling it captured: \
         {off:?} -> {on:?}"
    );
}

/// Box projection. The floor point is at x = -2, under the red half of the
/// ceiling; the probe was captured from x = +3, under the green half. Looking
/// straight up from the capture point, the probe sees green -- so a probe
/// that treats its capture as infinitely far away (box projection off) shows
/// the floor green, and one that follows the ray to the box's ceiling and
/// looks *there* (on) shows it red, which is what a real mirror at x = -2
/// would show. The box's top is the ceiling, as a probe fitted to a room is.
#[test]
fn box_projection_reflects_the_wall_above_the_surface_not_above_the_probe() {
    let mut h = Harness::new();
    let (plane, cube) = (h.plane(), h.cube());
    let panels = [(-20.0, 0.0, RED), (0.0, 20.0, GREEN)];
    let target = Vec3::new(-2.0, 0.0, 0.0);
    let room_probe = |box_projection| ReflectionProbeParams {
        box_projection,
        // Top face at y = 4.0, the ceiling; bottom just under the floor.
        ..probe(Vec3::new(3.0, 1.9, 0.0), Vec3::new(6.0, 2.1, 8.0))
    };
    let room = |probes| mirror_room(plane, cube, &panels, target, probes);

    let off = floor(&h.render(&room(vec![room_probe(false)])));
    let on = floor(&h.render(&room(vec![room_probe(true)])));

    assert!(
        bias(off, G) > 30.0,
        "premise: without box projection the probe's own view straight up is \
         green, and the floor shows it: {off:?}"
    );
    assert!(
        bias(on, R) > 30.0,
        "with box projection the floor at x = -2 should reflect the red ceiling \
         above it, not the green one above the probe: {on:?}"
    );
}

/// Where boxes nest, the smallest one containing the surface governs it --
/// in either order in the list. The inner probe has intensity 0, so a floor
/// governed by it is black while the outer probe alone shows it white; a
/// first-in-the-list rule gets one of the two orders wrong.
#[test]
fn the_smallest_box_containing_a_surface_governs_it() {
    let mut h = Harness::new();
    let (plane, cube) = (h.plane(), h.cube());
    let panels = [(-20.0, 20.0, WHITE)];
    let room = |probes| mirror_room(plane, cube, &panels, Vec3::ZERO, probes);
    let outer = probe(Vec3::new(0.0, 2.0, 0.0), Vec3::new(8.0, 3.0, 8.0));
    let inner = ReflectionProbeParams {
        intensity: 0.0,
        ..probe(Vec3::new(0.0, 1.0, 0.0), Vec3::new(1.5, 1.5, 1.5))
    };

    let outer_only = floor(&h.render(&room(vec![outer])));
    let inner_first = floor(&h.render(&room(vec![inner, outer])));
    let outer_first = floor(&h.render(&room(vec![outer, inner])));

    assert!(
        luma(outer_only) > 60.0,
        "premise: the outer probe alone lights the mirror: {outer_only:?}"
    );
    for (what, p) in [("inner first", inner_first), ("outer first", outer_first)] {
        assert!(
            luma(p) < 8.0,
            "{what}: the floor is inside the inner box, whose intensity is 0, so \
             it should be black: {p:?}"
        );
    }
}

/// The capture is taken when the probe list changes, not when the scene
/// does -- Godot's *Update Once*, Unity's baked probe. Repainting the ceiling
/// green under an unchanged probe keeps the red reflection; any change to
/// the probe itself re-captures and shows green.
#[test]
fn a_probe_captures_once_and_again_only_when_it_changes() {
    let mut h = Harness::new();
    let (plane, cube) = (h.plane(), h.cube());
    let p = probe(Vec3::new(0.0, 2.0, 0.0), Vec3::new(8.0, 3.0, 8.0));
    let room =
        |colour, probes| mirror_room(plane, cube, &[(-20.0, 20.0, colour)], Vec3::ZERO, probes);

    let red = floor(&h.render(&room(RED, vec![p])));
    let repainted = floor(&h.render(&room(GREEN, vec![p])));
    let moved = floor(&h.render(&room(
        GREEN,
        vec![ReflectionProbeParams {
            center: p.center + Vec3::new(0.0, 0.1, 0.0),
            ..p
        }],
    )));

    assert!(
        bias(red, R) > 40.0,
        "premise: the first capture is red: {red:?}"
    );
    assert!(
        bias(repainted, R) > 40.0,
        "an unchanged probe keeps its capture, so the floor stays red after the \
         ceiling is repainted: {repainted:?}"
    );
    assert!(
        bias(moved, G) > 40.0,
        "moving the probe re-captures, and the floor shows the green ceiling: {moved:?}"
    );
}

/// A rough floor reflects the capture blurred -- through the probe's lower
/// mips -- rather than not at all. Every mip of the slot has to have been
/// filled for this: with only mip 0 copied, roughness picks a level that is
/// still empty and the floor reads black.
#[test]
fn a_rough_floor_reflects_the_capture_through_its_blurred_mips() {
    let mut h = Harness::new();
    let (plane, cube) = (h.plane(), h.cube());
    let mut scene = mirror_room(
        plane,
        cube,
        &[(-20.0, 20.0, RED)],
        Vec3::ZERO,
        vec![probe(Vec3::new(0.0, 2.0, 0.0), Vec3::new(8.0, 3.0, 8.0))],
    );
    scene.draws[0].roughness = 0.8;
    let rough = floor(&h.render(&scene));
    assert!(
        bias(rough, R) > 20.0,
        "a rough floor under a red ceiling should still reflect red, blurred: {rough:?}"
    );
}

/// Each probe reflects its own capture. Two small probes far apart, one under
/// the red half of the ceiling and one under the green half, captured in the
/// same frame into two slots of the cube array: the floor in each box shows
/// its own probe's colour. Both captures written to one slot would show the
/// later one's colour in both boxes.
#[test]
fn each_probe_reflects_its_own_capture() {
    let mut h = Harness::new();
    let (plane, cube) = (h.plane(), h.cube());
    let panels = [(-20.0, 0.0, RED), (0.0, 20.0, GREEN)];
    let probes = vec![
        probe(Vec3::new(-10.0, 2.0, 0.0), Vec3::new(3.0, 3.0, 4.0)),
        probe(Vec3::new(10.0, 2.0, 0.0), Vec3::new(3.0, 3.0, 4.0)),
    ];
    let left = floor(&h.render(&mirror_room(
        plane,
        cube,
        &panels,
        Vec3::new(-10.0, 0.0, 0.0),
        probes.clone(),
    )));
    let right = floor(&h.render(&mirror_room(
        plane,
        cube,
        &panels,
        Vec3::new(10.0, 0.0, 0.0),
        probes,
    )));
    assert!(bias(left, R) > 30.0, "the left box reflects red: {left:?}");
    assert!(
        bias(right, G) > 30.0,
        "the right box reflects green: {right:?}"
    );
}

/// A realtime probe follows the scene: repaint the ceiling, and the mirror
/// floor shows the new colour a few frames later -- where a probe captured
/// once keeps the old one (the test above). It is refreshed one cube face a
/// frame and filtered into its slot only once all six are new, so it changes
/// whole, on a frame that finished a refresh, and not before the second frame
/// (all six faces at once would change it on the first). Until then every
/// frame shows the old capture complete, never a cube half of each.
#[test]
fn a_realtime_probe_follows_the_scene_a_face_a_frame() {
    let mut h = Harness::new();
    let (plane, cube) = (h.plane(), h.cube());
    let p = ReflectionProbeParams {
        realtime: true,
        ..probe(Vec3::new(0.0, 2.0, 0.0), Vec3::new(8.0, 3.0, 8.0))
    };
    let room =
        |colour, probes| mirror_room(plane, cube, &[(-20.0, 20.0, colour)], Vec3::ZERO, probes);

    let red = floor(&h.render(&room(RED, vec![p])));
    assert!(bias(red, R) > 40.0, "premise: captured whole, red: {red:?}");
    let mut turned = None;
    for frame in 1..=14 {
        let before = h.reflection_refreshes();
        let passes = h.cube_face_passes();
        let px = floor(&h.render(&room(GREEN, vec![p])));
        let finished = h.reflection_refreshes() > before;
        // The cost claim, which no pixel shows: a refresh that captured all
        // six faces and then filtered on the sixth frame would look the same.
        assert_eq!(
            h.cube_face_passes() - passes,
            1,
            "frame {frame}: a realtime probe is refreshed one face a frame"
        );
        if bias(px, G) > 40.0 {
            turned = Some((frame, finished));
            break;
        }
        assert!(
            bias(px, R) > 40.0,
            "frame {frame}: until a refresh finishes the floor shows the old capture, \
             whole: {px:?}"
        );
    }
    let (frame, finished) =
        turned.expect("within two refreshes the floor reflects the green ceiling");
    assert!(
        finished,
        "the reflection changed on frame {frame}, which finished no refresh: a cube \
         with only some faces new was shown"
    );
    assert!(
        frame >= 2,
        "one face a frame, not all six: the reflection changed on frame {frame}"
    );
}

/// Several realtime probes take turns: each is refreshed in full before the
/// next starts, and each gets its turn. Two probes over two floors, the
/// ceiling repainted green; both floors turn green within two rounds.
#[test]
fn realtime_probes_take_turns() {
    let mut h = Harness::new();
    let (plane, cube) = (h.plane(), h.cube());
    let probes = vec![
        ReflectionProbeParams {
            realtime: true,
            ..probe(Vec3::new(-10.0, 2.0, 0.0), Vec3::new(3.0, 3.0, 4.0))
        },
        ReflectionProbeParams {
            realtime: true,
            ..probe(Vec3::new(10.0, 2.0, 0.0), Vec3::new(3.0, 3.0, 4.0))
        },
    ];
    let (left, right) = (Vec3::new(-10.0, 0.0, 0.0), Vec3::new(10.0, 0.0, 0.0));
    let view = |h: &mut Harness, colour, target| {
        floor(&h.render(&mirror_room(
            plane,
            cube,
            &[(-20.0, 20.0, colour)],
            target,
            probes.clone(),
        )))
    };
    let before = (view(&mut h, RED, left), view(&mut h, RED, right));
    assert!(
        bias(before.0, R) > 30.0 && bias(before.1, R) > 30.0,
        "premise: both captured red: {before:?}"
    );
    let (mut left_green, mut right_green) = (false, false);
    for frame in 0..30 {
        let target = if frame % 2 == 0 { left } else { right };
        if bias(view(&mut h, GREEN, target), G) > 30.0 {
            if frame % 2 == 0 {
                left_green = true;
            } else {
                right_green = true;
            }
        }
        if left_green && right_green {
            break;
        }
    }
    assert!(
        left_green && right_green,
        "both realtime probes should have been refreshed: left {left_green}, right {right_green}"
    );
}

/// A whole capture -- the probe list changed -- restarts the realtime
/// refresh. Both share one capture cube: the whole capture leaves the last
/// probe's six faces in it, so a refresh that carried on mid-probe would file
/// the rest of the other probe's faces under the one it was refreshing. Two
/// realtime probes over a red and a green ceiling, the list changed after a
/// few frames of refresh: the red side must never show green.
#[test]
fn a_whole_capture_restarts_the_realtime_refresh() {
    let mut h = Harness::new();
    let (plane, cube) = (h.plane(), h.cube());
    let panels = [(-20.0, 0.0, RED), (0.0, 20.0, GREEN)];
    let left = ReflectionProbeParams {
        realtime: true,
        ..probe(Vec3::new(-10.0, 2.0, 0.0), Vec3::new(3.0, 3.0, 4.0))
    };
    let right = |dx: f32| ReflectionProbeParams {
        realtime: true,
        ..probe(Vec3::new(10.0 + dx, 2.0, 0.0), Vec3::new(3.0, 3.0, 4.0))
    };
    let view = |h: &mut Harness, probes: Vec<ReflectionProbeParams>| {
        floor(&h.render(&mirror_room(
            plane,
            cube,
            &panels,
            Vec3::new(-10.0, 0.0, 0.0),
            probes,
        )))
    };
    // Part-way into refreshing the left probe.
    for _ in 0..3 {
        view(&mut h, vec![left, right(0.0)]);
    }
    // The right probe moves: the list changed, both are captured whole.
    for frame in 0..14 {
        let px = view(&mut h, vec![left, right(0.1)]);
        assert!(
            bias(px, R) > 30.0,
            "frame {frame}: the left floor must keep reflecting its red ceiling: {px:?}"
        );
    }
}

// ---- blending ---------------------------------------------------------------

/// An 8-bit sRGB level back to linear light.
fn linear(c: u8) -> f32 {
    let c = c as f32 / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// The floor's centre pixel, in linear light, of `scene` rendered without
/// tone mapping -- so a probe's share of the reflection is that share of the
/// light, not of a curve -- and without bloom, whose glow from the bright
/// floor around a pixel falls away as the floor beside it darkens: through a
/// blend it took a tenth off the probe's share at the pixel.
fn floor_light(h: &mut Harness, mut scene: Scene) -> Vec3 {
    scene.tone_map = Some(bsengine_core::ToneMap::new(
        bsengine_core::ToneMappingMode::None,
    ));
    scene.bloom = Some(bsengine_core::Bloom::default().disabled());
    scene.ssao = Some(bsengine_core::AmbientOcclusion::default().disabled());
    let p = floor(&h.render(&scene));
    Vec3::new(linear(p[0]), linear(p[1]), linear(p[2]))
}

/// A probe at a quarter of its brightness: the white ceiling's reflection,
/// three times over the panels' radiance at full, would clip unmapped.
fn dim(p: ReflectionProbeParams) -> ReflectionProbeParams {
    ReflectionProbeParams {
        intensity: 0.25,
        ..p
    }
}

/// Inside a smaller box, within its blend distance of a face, the surface
/// hands over to the larger box around it in proportion: the inner probe
/// (intensity 0, so its share is black) is all of the reflection deeper in
/// than its blend distance of 1, and none at its face -- a floor point 0.75
/// in from the face shows three quarters of it, so a quarter of the outer
/// probe's light. In either order in the list. The hard edge this replaced
/// showed the inner probe whole right up to the face: black at every one of
/// these points.
#[test]
fn a_smaller_probe_hands_over_to_the_larger_across_its_blend_distance() {
    let mut h = Harness::new();
    let (plane, cube) = (h.plane(), h.cube());
    let panels = [(-20.0, 20.0, WHITE)];
    // The floor point at half the inner box's height, so only its x faces
    // are near; deep inside the outer box everywhere here.
    let outer = ReflectionProbeParams {
        blend_distance: 1.0,
        ..dim(probe(Vec3::new(0.0, 1.5, 0.0), Vec3::new(8.0, 3.0, 8.0)))
    };
    let inner = ReflectionProbeParams {
        intensity: 0.0,
        blend_distance: 1.0,
        ..probe(Vec3::ZERO, Vec3::splat(1.5))
    };
    let at = |h: &mut Harness, x: f32, probes: Vec<ReflectionProbeParams>| {
        // An empty list first, so each list is captured afresh (see the top).
        h.render(&mirror_room(plane, cube, &panels, Vec3::ZERO, Vec::new()));
        floor_light(
            h,
            mirror_room(plane, cube, &panels, Vec3::new(x, 0.0, 0.0), probes),
        )
        .x
    };
    let alone = at(&mut h, 0.0, vec![outer]);
    assert!(
        alone > 0.3 && alone < 0.95,
        "premise: the outer probe alone lights the floor, unclipped: {alone}"
    );
    // (x, the inner probe's share there): 1.5 - |x| in from its face.
    for (x, inner_share) in [
        (0.0, 1.0),
        (0.75, 0.75),
        (1.0, 0.5),
        (1.25, 0.25),
        (1.6, 0.0),
    ] {
        for (what, probes) in [
            ("inner first", vec![inner, outer]),
            ("outer first", vec![outer, inner]),
        ] {
            let seen = at(&mut h, x, probes) / alone;
            assert!(
                (seen - (1.0 - inner_share)).abs() < 0.06,
                "{what}, x = {x}: the outer probe's share should be {}, it is {seen}",
                1.0 - inner_share
            );
        }
    }
}

/// At its own faces a probe hands over to the sky, as it would to a larger
/// box: halfway through its blend distance the floor reflects half the
/// probe's capture (the white ceiling, in every channel) and half the sky's
/// specular (blue, in blue alone). Faded to nothing instead, the blue would
/// be half the probe's and none of the sky's.
#[test]
fn a_probe_hands_over_to_the_sky_at_its_own_faces() {
    let mut h = Harness::new();
    let (plane, cube) = (h.plane(), h.cube());
    h.set_test_skybox([0, 0, 128, 255]);
    let panels = [(-20.0, 20.0, WHITE)];
    let p = ReflectionProbeParams {
        blend_distance: 1.0,
        ..dim(probe(Vec3::new(0.0, 1.5, 0.0), Vec3::new(2.0, 3.0, 8.0)))
    };
    let mut at = |x: f32| {
        h.render(&mirror_room(plane, cube, &panels, Vec3::ZERO, Vec::new()));
        floor_light(
            &mut h,
            mirror_room(plane, cube, &panels, Vec3::new(x, 0.0, 0.0), vec![p]),
        )
    };
    let (inside, halfway, outside) = (at(0.0), at(1.5), at(2.5));
    assert!(
        inside.x > 0.3 && outside.z > 0.05 && outside.x < 0.02,
        "premise: inside, the white capture; outside, the blue sky alone: {inside} / {outside}"
    );
    assert!(
        (halfway.x / inside.x - 0.5).abs() < 0.06,
        "halfway, half the capture: {halfway} against {inside}"
    );
    let want = 0.5 * inside.z + 0.5 * outside.z;
    assert!(
        (halfway.z - want).abs() < 0.06 * want,
        "and half the sky: blue {} where half of each is {want}",
        halfway.z
    );
}

/// A blend distance of 0 is the hard edge: the inner probe (black) whole
/// right up to its face -- 0.1 in, the floor still black.
#[test]
fn a_blend_distance_of_zero_is_a_hard_edge() {
    let mut h = Harness::new();
    let (plane, cube) = (h.plane(), h.cube());
    let panels = [(-20.0, 20.0, WHITE)];
    let outer = dim(probe(Vec3::new(0.0, 1.5, 0.0), Vec3::new(8.0, 3.0, 8.0)));
    let inner = ReflectionProbeParams {
        intensity: 0.0,
        ..probe(Vec3::ZERO, Vec3::splat(1.5))
    };
    let near_face = floor_light(
        &mut h,
        mirror_room(
            plane,
            cube,
            &panels,
            Vec3::new(1.4, 0.0, 0.0),
            vec![outer, inner],
        ),
    );
    assert!(
        near_face.max_element() < 0.01,
        "0.1 inside a hard-edged black probe, black: {near_face}"
    );
}

/// The floor light of the scene `make` builds, with its probes captured
/// afresh: it is rendered once without them first, which empties the
/// capture cache, so a probe list equal to the previous scene's still
/// captures this scene and not that one.
fn captured(h: &mut Harness, make: impl Fn() -> Scene) -> Vec3 {
    floor_light(
        h,
        Scene {
            reflection_probes: Vec::new(),
            ..make()
        },
    );
    floor_light(h, make())
}

/// One texel, uploaded linear as the material maps are.
fn data_texel(h: &mut Harness, rgba: [u8; 4]) -> u64 {
    h.texture_with(
        bsengine_core::TextureImportSettings {
            srgb: false,
            mipmaps: false,
            ..Default::default()
        },
        1,
        1,
        &rgba,
    )
}

/// The probe every capture-material test uses: the room's, at a quarter
/// brightness so a white ceiling's reflection does not clip.
fn room_probe() -> Vec<ReflectionProbeParams> {
    vec![dim(probe(
        Vec3::new(0.0, 2.0, 0.0),
        Vec3::new(8.0, 3.0, 8.0),
    ))]
}

/// A probe captures a surface with its material's emissive map, as the
/// camera draws it: a red panel whose emissive map is black emits nothing
/// on screen, and the mirror under it reflects nothing either. A white map
/// is no map (premise: the same panel, so the red is the panel's).
#[test]
fn a_probe_captures_the_emissive_map() {
    let mut h = Harness::new();
    let (plane, cube) = (h.plane(), h.cube());
    let (black, white) = (
        data_texel(&mut h, [0, 0, 0, 255]),
        data_texel(&mut h, [255, 255, 255, 255]),
    );
    let room = |map: u64| {
        move || {
            let mut scene =
                mirror_room(plane, cube, &[(-20.0, 20.0, RED)], Vec3::ZERO, room_probe());
            let panel = scene.draws.pop().unwrap();
            scene.draws.push(panel.emissive_mapped(map));
            scene
        }
    };
    let lit = captured(&mut h, room(white));
    let dark = captured(&mut h, room(black));
    assert!(
        lit.x > 0.1 && lit.x > 5.0 * lit.y,
        "premise: under a white emissive map the mirror shows the red panel: {lit}"
    );
    assert!(
        dark.max_element() < 0.01 * lit.x,
        "a black emissive map leaves nothing for the probe to capture: {dark} against {lit}"
    );
}

/// The room of a capture-material test: the mirror floor under a white
/// diffuse slab where the panels were, lit by `light` instead of glowing.
fn lit_room(plane: u64, cube: u64, ceiling: Draw, light: Light) -> Scene {
    let mut scene = mirror_room(plane, cube, &[], Vec3::ZERO, room_probe());
    scene.draws.push(ceiling);
    scene.light = light;
    scene
}

fn ceiling(cube: u64) -> Draw {
    Draw::new(cube, Vec3::ZERO)
        .scaled(
            Vec3::new(40.0, 0.2, 40.0),
            Vec3::new(0.0, CEILING_Y + 0.1, 0.0),
        )
        .colour(WHITE)
        .roughness(1.0)
}

/// The ORM map's occlusion darkens what a probe captures of the ambient
/// light, as it darkens it on screen: a ceiling lit by ambient alone under
/// an occlusion of 0 at full strength is reflected as a black ceiling is;
/// at strength 0 the same map changes nothing (premise: the ceiling's
/// ambient is in the reflection at all). Against a black ceiling and not
/// against black: the mirror floor takes the ambient too, so its own share
/// is under every reading here.
#[test]
fn a_probe_captures_the_orm_maps_occlusion() {
    let mut h = Harness::new();
    let (plane, cube) = (h.plane(), h.cube());
    let occluded = data_texel(&mut h, [0, 255, 255, 255]);
    let ambient_only = || Light {
        color: Vec3::ZERO,
        ambient: Vec3::splat(0.5),
        ..Light::default()
    };
    let room = |ceiling: Draw| lit_room(plane, cube, ceiling, ambient_only());
    let black = captured(&mut h, || room(ceiling(cube).colour(Vec3::ZERO)));
    let open = captured(&mut h, || room(ceiling(cube).orm_mapped(occluded, 0.0)));
    let shut = captured(&mut h, || room(ceiling(cube).orm_mapped(occluded, 1.0)));
    assert!(
        open.x - black.x > 0.05,
        "premise: at strength 0 the ambient-lit ceiling is reflected: {open} over a black \
         ceiling's {black}"
    );
    assert!(
        (shut - black).abs().max_element() < 0.05 * (open.x - black.x),
        "occlusion 0 at full strength captures no ambient -- the ceiling is reflected as a \
         black one: {shut} against {black} (unoccluded {open})"
    );
}

/// A point light below the ceiling and off to one side, so the slab is lit
/// at a slant from a direction with both an x and a z part -- a normal map
/// tilted along either tangent axis turns the slab toward it or away,
/// whichever way the cube's face lays its tangent.
fn side_light() -> Light {
    Light {
        color: Vec3::ZERO,
        ambient: Vec3::ZERO,
        points: vec![common::PointLight {
            position: Vec3::new(2.5, 2.5, 1.0),
            color: Vec3::ONE,
            intensity: 10.0,
            range: 8.0,
        }],
        ..Light::default()
    }
}

/// A probe captures a surface with its normal map: the ceiling under a map
/// tilted 60 degrees one way and the other, along u and along v, is
/// captured brighter than flat turned toward the light and darker turned
/// away -- where a capture that ignored the map shows the flat ceiling
/// four times.
#[test]
fn a_probe_captures_the_normal_map() {
    let mut h = Harness::new();
    let (plane, cube) = (h.plane(), h.cube());
    let enc = |c: f32| ((c * 0.5 + 0.5) * 255.0).round() as u8;
    let (s, c) = 60f32.to_radians().sin_cos();
    let flat = captured(&mut h, || {
        lit_room(plane, cube, ceiling(cube), side_light())
    });
    assert!(
        flat.x > 0.05,
        "premise: the flat ceiling is lit and reflected: {flat}"
    );
    let mut seen = Vec::new();
    for n in [
        Vec3::new(s, 0.0, c),
        Vec3::new(-s, 0.0, c),
        Vec3::new(0.0, s, c),
        Vec3::new(0.0, -s, c),
    ] {
        let map = data_texel(&mut h, [enc(n.x), enc(n.y), enc(n.z), 255]);
        seen.push(
            captured(&mut h, || {
                lit_room(
                    plane,
                    cube,
                    ceiling(cube).normal_mapped(map, 1.0),
                    side_light(),
                )
            })
            .x,
        );
    }
    let (lo, hi) = seen
        .iter()
        .fold((f32::MAX, f32::MIN), |(lo, hi), &x| (lo.min(x), hi.max(x)));
    assert!(
        hi > 1.3 * flat.x && lo < 0.7 * flat.x,
        "tilted toward the light the ceiling is captured brighter than flat, tilted \
         away darker: {seen:?} against flat {}",
        flat.x
    );
}

/// A probe captures a surface with its ORM map's metallic: a metal ceiling
/// (factor 1) whose map says 0 is captured as the dielectric it is drawn as
/// -- its diffuse lit -- not as the metal the factor alone makes it.
#[test]
fn a_probe_captures_the_orm_maps_metallic() {
    let mut h = Harness::new();
    let (plane, cube) = (h.plane(), h.cube());
    let not_metal = data_texel(&mut h, [255, 255, 0, 255]);
    let dielectric = captured(&mut h, || {
        lit_room(plane, cube, ceiling(cube), side_light())
    });
    let metal = captured(&mut h, || {
        lit_room(plane, cube, ceiling(cube).metallic(1.0), side_light())
    });
    let mapped = captured(&mut h, || {
        lit_room(
            plane,
            cube,
            ceiling(cube).metallic(1.0).orm_mapped(not_metal, 0.0),
            side_light(),
        )
    });
    assert!(
        metal.x < 0.5 * dielectric.x,
        "premise: the metal ceiling is captured darker than the dielectric one: \
         {metal} against {dielectric}"
    );
    assert!(
        (mapped - dielectric).abs().max_element() <= 0.02 * dielectric.x,
        "metallic 1 times a map's 0 is captured as metallic 0: {mapped} against {dielectric}"
    );
}

/// And its roughness: a metal ceiling -- specular only, so the roughness
/// is all that shapes what it sends the probe -- at roughness 1 under a map
/// whose G is 0.2 (51 is exactly a fifth of 255) is captured as the same
/// ceiling at roughness 0.2.
#[test]
fn a_probe_captures_the_orm_maps_roughness() {
    let mut h = Harness::new();
    let (plane, cube) = (h.plane(), h.cube());
    let fifth = data_texel(&mut h, [255, 51, 255, 255]);
    let metal = |roughness: f32| ceiling(cube).metallic(1.0).roughness(roughness);
    let rough = captured(&mut h, || lit_room(plane, cube, metal(1.0), side_light()));
    let smooth = captured(&mut h, || lit_room(plane, cube, metal(0.2), side_light()));
    let mapped = captured(&mut h, || {
        lit_room(plane, cube, metal(1.0).orm_mapped(fifth, 0.0), side_light())
    });
    assert!(
        (smooth - rough).abs().max_element() > 0.2 * rough.x.max(smooth.x),
        "premise: roughness 0.2 and 1 are captured differently: {smooth} against {rough}"
    );
    assert!(
        (mapped - smooth).abs().max_element() <= 0.02 * smooth.max_element(),
        "roughness 1 times a map's 0.2 is captured as roughness 0.2: {mapped} against {smooth}"
    );
}
