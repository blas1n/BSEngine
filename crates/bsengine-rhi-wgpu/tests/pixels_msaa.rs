//! MSAA, measured at the pixel: does it smooth geometry edges, leave
//! everything else alone, and keep the depth every post pass reads?
//!
//! The edge fixture is `pixels_taa.rs`'s: a bright square turned out of
//! pixel alignment on a dark backdrop, lit only by emission, bloom, tone
//! mapping and SSAO off -- without antialiasing every pixel is exactly one of
//! two tones.

mod common;

use bsengine_rhi_wgpu::particles::{ParticleBatch, ParticleInstance};
use common::{Draw, Harness, Light, Pixels, Scene, HEIGHT, WIDTH};
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
    // Sampled just inside the cube's right and top edges, not at its centre:
    // with the depth gone (the empty frame left it at the far plane) DOF
    // blurs the whole frame alike, and the middle of a large blue cube is
    // still blue when blurred. Next to an edge, blur pulls in the backdrop.
    let right = (cx..reference.width)
        .find(|&x| reference.at(x, cy)[2] < centre[2] / 2)
        .expect("premise: the cube's right edge is on screen");
    let top = (0..cy)
        .rev()
        .find(|&y| reference.at(cx, y)[2] < centre[2] / 2)
        .expect("premise: the cube's top edge is on screen");
    let inside = [(cx, cy), (right - 3, cy), (cx, top + 3)];
    for &(x, y) in &inside[1..] {
        let p = reference.at(x, y);
        assert!(
            p[2] > 150 && p[0] < 5 && p[1] < 5,
            "premise: in focus, the cube stays sharp blue up to its edge: {p:?} at ({x}, {y})"
        );
    }
    for (x, y) in inside {
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
///
/// Every geometry pass resolves, so a pass that forgot to would be hidden
/// by any later pass's resolve of the same samples. Each pass is therefore
/// the *last* one drawn in one of the four scenes below: opaque only, then
/// adding the sky, the transparent pane, and the particle. A pass whose
/// resolve went missing leaves the previous pass's resolve -- or nothing --
/// in the frame, and its own content shows up as an interior change.
#[test]
fn every_geometry_pass_draws_under_msaa() {
    let mut h = Harness::new();
    let cube = h.cube();
    h.set_test_skybox([40, 60, 90, 255]);
    let custom = h.constant_colour_shader([0.2, 0.9, 0.3], "msaa_custom");
    let scene = |msaa, last: usize| {
        let mut draws = vec![Draw::new(cube, Vec3::ZERO)
            .scaled(Vec3::splat(0.8), Vec3::new(-1.2, 0.0, 0.0))
            .shader(&custom)];
        if last >= 2 {
            draws.push(
                Draw::new(cube, Vec3::ZERO)
                    .scaled(Vec3::splat(0.8), Vec3::new(1.2, 0.0, 0.0))
                    .colour(Vec3::ONE)
                    .opacity(0.5),
            );
        }
        let particles = if last >= 3 {
            vec![ParticleBatch {
                texture_id: None,
                instances: vec![ParticleInstance {
                    position: [0.0, 1.2, 0.0],
                    size: 0.5,
                    color: [1.0, 0.9, 0.2, 1.0],
                }],
            }]
        } else {
            Vec::new()
        };
        Scene {
            draws,
            particles,
            with_skybox: last >= 1,
            msaa,
            ..flat_scene()
        }
    };
    let names = ["opaque", "sky", "transparent", "particle"];
    for (last, name) in names.iter().enumerate() {
        let off = h.render(&scene(1, last));
        let on = h.render(&scene(4, last));
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
            "with the {name} pass last, the frame looks the same away from edges"
        );
        assert!(
            off.differs_from(&on),
            "premise: with the {name} pass last, the edges did change"
        );
    }
}

/// The normals resolve too: SSR reads them, and under MSAA the geometry
/// passes write them into a multisampled target first. The fixture is
/// `pixels_ssr.rs`'s mirror floor in front of a red pillar.
///
/// An empty frame is drawn just before the multisampled one, so the
/// single-sample normal target holds the clear value rather than the floor's
/// normals from the reference frame: a resolve that went missing leaves SSR
/// with no surface to reflect off, and the reflection disappears.
#[test]
fn ssr_reads_the_resolved_normals() {
    let mut h = Harness::new();
    let cube = h.cube();
    let scene = |ssr, msaa| Scene {
        draws: vec![
            Draw::new(cube, Vec3::ZERO)
                .scaled(Vec3::new(20.0, 1.0, 20.0), Vec3::ZERO)
                .roughness(0.02)
                .colour(Vec3::splat(0.02)),
            Draw::new(cube, Vec3::new(0.0, 1.5, 0.0))
                .scaled(Vec3::new(1.0, 2.0, 1.0), Vec3::new(0.0, 1.5, 0.0))
                .emissive(Vec3::new(4.0, 0.4, 0.4)),
        ],
        camera_pos: Vec3::new(0.0, 1.2, 6.0),
        look_at: Vec3::new(0.0, 0.6, 0.0),
        ssr,
        msaa,
        light: Light {
            ambient: Vec3::splat(0.02),
            ..Light::default()
        },
        ..Scene::default()
    };
    let ssr = Some(bsengine_core::ScreenSpaceReflections::default());
    let (x, y) = (WIDTH / 2, (HEIGHT as f32 * 0.72) as u32);

    let plain = h.render(&scene(None, 4)).at(x, y);
    let reference = h.render(&scene(ssr, 1)).at(x, y);
    h.render(&Scene {
        msaa: 1,
        ..Scene::default()
    });
    let on = h.render(&scene(ssr, 4)).at(x, y);
    assert_eq!(h.msaa_samples(), 4, "premise: drawn multisampled");
    assert!(
        reference[0] > plain[0] + 4,
        "premise: without MSAA the floor reflects the pillar: {plain:?} -> {reference:?}"
    );
    assert!(
        (0..3).all(|c| on[c].abs_diff(reference[c]) <= 2),
        "under MSAA the reflection is the same: {reference:?} vs {on:?}"
    );
}
