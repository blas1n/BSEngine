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
