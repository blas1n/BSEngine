//! Soft shadows: a directional shadow's edge is a gradient, not the shadow
//! map's texel grid.

mod common;

use common::{Draw, Harness, Light, Pixels, Scene};
use glam::Vec3;

/// A floor seen from straight above, with a caster above it but outside the
/// view, so every pixel on screen is floor and the only thing on it is the
/// caster's shadow -- edge included. The sun is tilted to throw the shadow
/// sideways into the middle of the frame.
fn scene(cube: u64, soft: bool, with_caster: bool) -> Scene {
    let floor = Draw::new(cube, Vec3::ZERO).scaled(Vec3::new(40.0, 1.0, 40.0), Vec3::ZERO);
    let caster_at = Vec3::new(-4.0, 3.0, 0.0);
    let caster = Draw::new(cube, caster_at).scaled(Vec3::splat(1.5), caster_at);
    let mut draws = vec![floor];
    if with_caster {
        draws.push(caster);
    }
    Scene {
        draws,
        camera_pos: Vec3::new(0.0, 5.0, 0.01),
        look_at: Vec3::ZERO,
        light: Light {
            direction: Vec3::new(1.0, -1.0, 0.0).normalize(),
            // Low, so the shadow is far darker than the lit floor.
            ambient: Vec3::splat(0.02),
            ..Light::default()
        },
        soft_shadows: soft,
        ..Default::default()
    }
}

/// Each pixel's brightness as a fraction of the same pixel with no caster:
/// 1 where the floor is lit, the shadow's depth where it is shadowed. A ratio
/// rather than raw brightness, because the floor itself is not evenly lit --
/// its own falloff spreads raw brightness over a range the shadow edge would
/// be lost in.
fn shadow_ratios(frame: &Pixels, uncast: &Pixels) -> Vec<f32> {
    (0..frame.height)
        .flat_map(|y| (0..frame.width).map(move |x| (x, y)))
        .map(|(x, y)| frame.luma(x, y) / uncast.luma(x, y).max(1.0))
        .collect()
}

/// How many pixels are part-way between shadowed and lit: the edge.
fn edge_pixels(ratios: &[f32]) -> (usize, f32) {
    let mut sorted = ratios.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let shadow = sorted[sorted.len() / 20];
    let depth = 1.0 - shadow;
    let between = ratios
        .iter()
        .filter(|&&r| r > shadow + depth * 0.15 && r < 1.0 - depth * 0.15)
        .count();
    (between, shadow)
}

/// With soft shadows the edge is a gradient several pixels wide; with them
/// off it is a step, with almost nothing in between. The same scene both ways,
/// so the only difference is the filter.
#[test]
fn a_soft_shadow_edge_is_a_gradient_and_a_hard_one_a_step() {
    let mut h = Harness::new();
    let cube = h.cube();

    let uncast = h.render(&scene(cube, true, false));
    let hard = h.render(&scene(cube, false, true));
    let soft = h.render(&scene(cube, true, true));

    // The caster only ever darkens: a pixel it made brighter would be the
    // caster itself on screen, and its faces would count as "edge".
    let brightened = shadow_ratios(&hard, &uncast)
        .iter()
        .filter(|&&r| r > 1.05)
        .count();
    assert_eq!(
        brightened, 0,
        "premise: the caster is out of view; only its shadow is on screen"
    );

    let hard_ratios = shadow_ratios(&hard, &uncast);
    let (hard_edge, hard_shadow) = edge_pixels(&hard_ratios);
    let lit = hard_ratios.iter().filter(|&&r| r > 0.95).count();
    assert!(
        hard_shadow < 0.5 && lit > hard_ratios.len() / 10,
        "premise: the frame holds both deep shadow ({hard_shadow} of lit) and \
         lit floor ({lit} pixels)"
    );

    let (soft_edge, soft_shadow) = edge_pixels(&shadow_ratios(&soft, &uncast));
    assert!(
        soft_shadow < 0.5,
        "premise: the soft frame has its shadow too ({soft_shadow} of lit)"
    );
    assert!(
        soft_edge > hard_edge * 2 + 20,
        "the soft edge is a gradient: {soft_edge} pixels between lit and shadow, \
         against {hard_edge} for the hard one"
    );
}
