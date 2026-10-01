//! MSAA, measured at the pixel: does it smooth geometry edges, leave
//! everything else alone, and keep the depth every post pass reads?
//!
//! The edge fixture is `pixels_taa.rs`'s: a bright square turned out of
//! pixel alignment on a dark backdrop, lit only by emission, bloom, tone
//! mapping and SSAO off -- without antialiasing every pixel is exactly one of
//! two tones.

mod common;

use bsengine_rhi_wgpu::particles::{ParticleBatch, ParticleInstance};
use common::{Draw, Harness, Light, Pixels, Scene};
use glam::Vec3;

const BACKDROP: f32 = 0.05;
const SQUARE: f32 = 1.0;

fn flat_scene() -> Scene {
    Scene {
        light: Light {
            color: Vec3::ZERO,
            ambient: Vec3::ZERO,
            ..Light::default()
        },
        bloom: Some(bsengine_core::Bloom {
            enabled: false,
            ..Default::default()
        }),
        tone_map: Some(bsengine_core::ToneMap {
            enabled: false,
            ..Default::default()
        }),
        ssao: Some(bsengine_core::AmbientOcclusion {
            enabled: false,
            ..Default::default()
        }),
        ..Scene::default()
    }
}

fn square(cube: u64, msaa: u32) -> Scene {
    Scene {
        draws: vec![
            Draw::new(cube, Vec3::ZERO)
                .scaled(Vec3::new(40.0, 40.0, 0.2), Vec3::new(0.0, 0.0, -5.0))
                .colour(Vec3::ZERO)
                .emissive(Vec3::splat(BACKDROP)),
            Draw::new(cube, Vec3::ZERO)
                .scaled(Vec3::splat(1.5), Vec3::ZERO)
                .rotated_z(30f32.to_radians())
                .colour(Vec3::ZERO)
                .emissive(Vec3::splat(SQUARE)),
        ],
        msaa,
        ..flat_scene()
    }
}

/// Pixels a tenth of the range in from both tones (see `pixels_taa.rs`).
fn intermediate_tones(p: &Pixels) -> u32 {
    let (lo, hi) = (p.luma(0, 0), p.centre_luma());
    let margin = 0.1 * (hi - lo);
    let mut n = 0;
    for y in 0..p.height {
        for x in 0..p.width {
            let l = p.luma(x, y);
            if l > lo + margin && l < hi - margin {
                n += 1;
            }
        }
    }
    n
}

/// Every pixel within `radius` of a tone change in a two-tone frame.
fn near_an_edge(p: &Pixels, radius: i32) -> Vec<bool> {
    let (w, h) = (p.width as i32, p.height as i32);
    let mut edge = vec![false; (w * h) as usize];
    for y in 0..h {
        for x in 0..w {
            let here = p.luma(x as u32, y as u32);
            let differs = [(-1, 0), (1, 0), (0, -1), (0, 1)].iter().any(|(dx, dy)| {
                let (nx, ny) = (x + dx, y + dy);
                nx >= 0 && ny >= 0 && nx < w && ny < h && p.luma(nx as u32, ny as u32) != here
            });
            if differs {
                for dy in -radius..=radius {
                    for dx in -radius..=radius {
                        let (nx, ny) = (x + dx, y + dy);
                        if nx >= 0 && ny >= 0 && nx < w && ny < h {
                            edge[(ny * w + nx) as usize] = true;
                        }
                    }
                }
            }
        }
    }
    edge
}

/// The square's aliased edges gain in-between tones -- none without MSAA --
/// and no pixel away from the silhouette changes: MSAA resolves coverage at
/// edges and is exactly the plain frame everywhere a pixel is covered by one
/// surface. A blur would pass the first half and fail the second.
#[test]
fn msaa_smooths_geometry_edges_and_nothing_else() {
    let mut h = Harness::new();
    let cube = h.cube();
    let off = h.render(&square(cube, 1));
    let on = h.render(&square(cube, 4));
    assert_eq!(h.msaa_samples(), 4, "premise: this adapter multisamples");
    let (off_n, on_n) = (intermediate_tones(&off), intermediate_tones(&on));
    eprintln!("intermediate tones: off {off_n}, MSAA {on_n}");
    assert_eq!(off_n, 0, "premise: without MSAA the edge is hard");
    assert!(
        on_n >= 100,
        "MSAA puts coverage tones along the edge: {on_n}"
    );

    let mask = near_an_edge(&off, 1);
    let strays: Vec<(u32, u32)> = (0..off.height)
        .flat_map(|y| (0..off.width).map(move |x| (x, y)))
        .filter(|&(x, y)| {
            let (a, b) = (off.at(x, y), on.at(x, y));
            (0..3).any(|c| a[c].abs_diff(b[c]) > 1) && !mask[(y * off.width + x) as usize]
        })
        .collect();
    assert!(
        strays.is_empty(),
        "{} pixels away from any edge changed, e.g. {:?}",
        strays.len(),
        &strays[..strays.len().min(5)]
    );
    // And off again is off: the shared surface does not keep sampling.
    let again = h.render(&square(cube, 1));
    assert_eq!(h.msaa_samples(), 1);
    assert!(!again.differs_from(&off));
}

/// The depth every post pass reads is resolved from the multisampled one.
/// Depth of field blurs by distance: the near cube stays sharp, the far
/// backdrop blurs -- under MSAA exactly as without it. Between the two, an
/// empty frame is drawn without MSAA, so the single-sample depth holds
/// nothing but the far plane: a resolve that did not run would leave that
/// there, and the cube, read as far away, would blur.
#[test]
fn post_passes_read_the_resolved_depth() {
    let mut h = Harness::new();
    let cube = h.cube();
    let dof = |msaa| Scene {
        draws: vec![
            Draw::new(cube, Vec3::ZERO)
                .scaled(Vec3::new(40.0, 40.0, 0.2), Vec3::new(-20.0, 0.0, -15.0))
                .colour(Vec3::ZERO)
                .emissive(Vec3::new(0.8, 0.0, 0.0)),
            Draw::new(cube, Vec3::ZERO)
                .scaled(Vec3::new(40.0, 40.0, 0.2), Vec3::new(20.0, 0.0, -15.0))
                .colour(Vec3::ZERO)
                .emissive(Vec3::new(0.0, 0.8, 0.0)),
            Draw::new(cube, Vec3::ZERO)
                .colour(Vec3::ZERO)
                .emissive(Vec3::new(0.0, 0.0, 0.8)),
        ],
        depth_of_field: Some(bsengine_core::DepthOfField::default()),
        msaa,
        ..flat_scene()
    };
    let reference = h.render(&dof(1));
    h.render(&flat_scene());
    let msaa = h.render(&dof(4));
    let (cx, cy) = (reference.width / 2, reference.height / 2);
    let centre = reference.at(cx, cy);
    assert!(
        centre[2] > 150 && centre[0] < 5,
        "premise: the in-focus cube is sharp blue at the centre: {centre:?}"
    );
    for (x, y) in [(cx, cy), (cx + 5, cy), (cx, cy - 5)] {
        let (a, b) = (reference.at(x, y), msaa.at(x, y));
        assert!(
            (0..3).all(|c| a[c].abs_diff(b[c]) <= 2),
            "({x}, {y}) on the cube: {b:?} under MSAA, {a:?} without -- \
             depth of field read the right depth"
        );
    }
}

/// Every pipeline a geometry pass draws with exists multisampled: the sky,
/// a transparent pane, particles and a custom shader all draw under MSAA
/// (a missing variant is a validation error, which fails the test), and
/// away from edges each looks as it does without.
#[test]
fn every_geometry_pass_draws_under_msaa() {
    let mut h = Harness::new();
    let cube = h.cube();
    h.set_test_skybox([40, 60, 90, 255]);
    let custom = h.constant_colour_shader([0.2, 0.9, 0.3], "msaa_custom");
    let scene = |msaa| Scene {
        draws: vec![
            Draw::new(cube, Vec3::ZERO)
                .scaled(Vec3::splat(0.8), Vec3::new(-1.2, 0.0, 0.0))
                .shader(&custom),
            Draw::new(cube, Vec3::ZERO)
                .scaled(Vec3::splat(0.8), Vec3::new(1.2, 0.0, 0.0))
                .colour(Vec3::ONE)
                .opacity(0.5),
        ],
        particles: vec![ParticleBatch {
            texture_id: None,
            instances: vec![ParticleInstance {
                position: [0.0, 1.2, 0.0],
                size: 0.5,
                color: [1.0, 0.9, 0.2, 1.0],
            }],
        }],
        with_skybox: true,
        msaa,
        ..flat_scene()
    };
    let off = h.render(&scene(1));
    let on = h.render(&scene(4));
    assert_eq!(h.msaa_samples(), 4, "premise: drawn multisampled");
    let mask = near_an_edge(&off, 1);
    let interior_changes = (0..off.height)
        .flat_map(|y| (0..off.width).map(move |x| (x, y)))
        .filter(|&(x, y)| {
            let (a, b) = (off.at(x, y), on.at(x, y));
            (0..3).any(|c| a[c].abs_diff(b[c]) > 2) && !mask[(y * off.width + x) as usize]
        })
        .count();
    assert_eq!(
        interior_changes, 0,
        "sky, custom shader, transparent pane and particle look the same away from edges"
    );
    assert!(off.differs_from(&on), "premise: and the edges did change");
}
