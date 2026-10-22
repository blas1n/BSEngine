//! Offline lightmap baking.
//!
//! Given a scene's static triangles, its lights and its sky, a
//! [`LightmapBaker`] packs one rectangle per renderer into atlas layers and
//! fills every texel with the light it receives. The result is the input
//! `bsengine_rhi_wgpu::lightmap` reads: `E / PI` per texel, to be stored RGBM
//! ([`encode_rgbm`]), with each renderer's `scale_offset`.
//!
//! The trait exists because there is more than one way to compute the same
//! texels: [`CpuBaker`] here (deterministic, runs anywhere, including a
//! headless CI with no GPU, and the reference the others are checked against),
//! and a GPU compute backend later. Front ends pick one per platform.

pub mod bvh;
pub mod gpu;
pub mod gpu_scene;
pub mod pack;
pub mod probe_bake;
pub mod probe_bricks;
pub mod sample;
pub mod scene;
pub mod trace;

pub use gpu::{is_hardware, GpuBaker};
pub use pack::{Packing, Placement};
pub use scene::{BakeLight, BakeTriangle, Contribution, Sky};
pub use trace::{Texel, TraceSettings};

use bvh::Bvh;
use glam::{Vec2, Vec3};
#[cfg(feature = "native")]
use rayon::prelude::*;
use sample::Rng;

/// What one renderer asks of the bake, besides its triangles.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RendererInfo {
    /// Multiplies its texel density (`LightmapStatic::resolution_scale`).
    pub resolution_scale: f32,
}

/// Everything a bake reads.
pub struct BakeInput<'a> {
    /// Every static triangle; each names its renderer.
    pub triangles: &'a [BakeTriangle],
    /// One entry per renderer, indexed by [`BakeTriangle::renderer`].
    pub renderers: &'a [RendererInfo],
    /// Lights that take part in the bake (realtime-only ones left out).
    pub lights: &'a [BakeLight],
    pub sky: &'a Sky,
}

/// How a bake is run.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BakeSettings {
    pub trace: TraceSettings,
    /// Texels per world unit, before each renderer's `resolution_scale`.
    pub texels_per_unit: f32,
    /// Side of one atlas layer, in texels (the runtime's `LIGHTMAP_SIZE`).
    pub layer_size: u32,
    /// Most layers a bake may fill (the runtime's `MAX_LIGHTMAPS`).
    pub max_layers: u32,
    /// Whether `Baked` lights' direct light goes into the atlas. Only once
    /// the runtime skips those lights on lightmapped surfaces; before that it
    /// would be drawn twice.
    pub bake_direct: bool,
}

impl Default for BakeSettings {
    fn default() -> Self {
        Self {
            trace: TraceSettings::default(),
            texels_per_unit: 16.0,
            layer_size: 1024,
            max_layers: 4,
            bake_direct: false,
        }
    }
}

/// A finished bake.
#[derive(Debug, Clone, PartialEq)]
pub struct BakeOutput {
    pub layer_size: u32,
    /// One image per layer, row-major, `E / PI` linear.
    pub layers: Vec<Vec<Vec3>>,
    /// Per layer, where each texel's light came from: the luminance-weighted
    /// mean incoming direction, in world space, at most unit length (zero
    /// where none arrived) -- what a directional lightmap shades a bumped
    /// surface by ([`trace::World::texel_directional`]). Same layout as
    /// `layers`.
    pub directions: Vec<Vec<Vec3>>,
    /// Where each renderer went, by renderer index.
    pub placements: Vec<Placement>,
    /// The density the scene was packed at (lower than asked if it overflowed).
    pub texels_per_unit: f32,
}

/// A way of computing a bake.
pub trait LightmapBaker {
    /// For logs and the `--backend` switch.
    fn name(&self) -> &'static str;
    fn bake(&self, input: &BakeInput, settings: &BakeSettings) -> BakeOutput;
}

/// The path tracer, on every CPU core.
pub struct CpuBaker;

/// One texel to compute: where it is in the atlas and on the surface.
pub(crate) struct Job {
    pub(crate) layer: u32,
    pub(crate) x: u32,
    pub(crate) y: u32,
    pub(crate) position: Vec3,
    pub(crate) normal: Vec3,
}

impl LightmapBaker for CpuBaker {
    fn name(&self) -> &'static str {
        "cpu"
    }

    fn bake(&self, input: &BakeInput, settings: &BakeSettings) -> BakeOutput {
        let prepared = prepare(input, settings);
        let world = trace::World {
            bvh: &prepared.bvh,
            triangles: input.triangles,
            lights: input.lights,
            sky: input.sky,
            settings: settings.trace,
        };
        // Each texel seeds its own generator, so the order rayon runs them in
        // changes nothing: the bake is the same bytes on any core count --
        // and on one, which is how it runs without the `native` feature (the
        // browser has no threads for rayon to spawn).
        #[cfg(feature = "native")]
        let jobs = prepared.jobs.par_iter();
        #[cfg(not(feature = "native"))]
        let jobs = prepared.jobs.iter();
        let values: Vec<trace::Texel> = jobs
            .map(|j| {
                let mut rng = Rng::for_texel(settings.trace.seed, j.layer, j.x, j.y);
                world.texel_directional(j.position, j.normal, settings.bake_direct, &mut rng)
            })
            .collect();
        finish(prepared, values, settings)
    }
}

/// Everything about a bake that does not depend on how its texels are
/// computed: where each renderer goes, which texels it covers and where on
/// the surface each one is, and the tree the rays are traced through. Both
/// backends start here, so they compute the same texels at the same places --
/// and differ only in how each texel's light is found.
pub(crate) struct Prepared {
    pub(crate) packing: Packing,
    pub(crate) covered: Vec<Vec<bool>>,
    pub(crate) jobs: Vec<Job>,
    pub(crate) bvh: Bvh,
}

pub(crate) fn prepare(input: &BakeInput, settings: &BakeSettings) -> Prepared {
    let size = settings.layer_size;
    let mut areas = vec![0.0f32; input.renderers.len()];
    for t in input.triangles {
        let [a, b, c] = t.positions;
        areas[t.renderer as usize] += 0.5 * (b - a).cross(c - a).length();
    }
    let scales: Vec<f32> = input.renderers.iter().map(|r| r.resolution_scale).collect();
    let packing = pack::pack(
        &areas,
        &scales,
        settings.texels_per_unit,
        size,
        settings.max_layers,
    );
    let mut covered = vec![vec![false; (size * size) as usize]; packing.layers as usize];
    let jobs = rasterise(input.triangles, &packing.placements, size, &mut covered);
    let bvh = Bvh::build(
        &input
            .triangles
            .iter()
            .map(|t| t.positions)
            .collect::<Vec<_>>(),
    );
    Prepared {
        packing,
        covered,
        jobs,
        bvh,
    }
}

/// Places each texel's value, `values[i]` for `prepared.jobs[i]`, and dilates
/// -- the directions as the irradiance, so a chart's rim carries its edge's.
pub(crate) fn finish(
    prepared: Prepared,
    values: Vec<trace::Texel>,
    settings: &BakeSettings,
) -> BakeOutput {
    let size = settings.layer_size;
    let Prepared {
        packing,
        covered,
        jobs,
        ..
    } = prepared;
    let mut layers = vec![vec![Vec3::ZERO; (size * size) as usize]; packing.layers as usize];
    let mut directions = layers.clone();
    for (j, v) in jobs.iter().zip(values) {
        let at = (j.y * size + j.x) as usize;
        layers[j.layer as usize][at] = v.irradiance;
        directions[j.layer as usize][at] = v.direction;
    }
    for ((image, dirs), mask) in layers.iter_mut().zip(&mut directions).zip(&covered) {
        dilate(image, mask, size, pack::PADDING + 1);
        dilate(dirs, mask, size, pack::PADDING + 1);
    }
    BakeOutput {
        layer_size: size,
        layers,
        directions,
        placements: packing.placements,
        texels_per_unit: packing.texels_per_unit,
    }
}

/// Every texel whose centre a renderer's triangle covers, with the world
/// position and normal there. The first triangle to cover a texel keeps it:
/// two triangles of one chart share an edge, and a texel centre exactly on it
/// would otherwise be computed twice.
fn rasterise(
    triangles: &[BakeTriangle],
    placements: &[Placement],
    size: u32,
    covered: &mut [Vec<bool>],
) -> Vec<Job> {
    let mut jobs = Vec::new();
    for t in triangles {
        let pl = placements[t.renderer as usize];
        let origin = Vec2::new(pl.x as f32, pl.y as f32);
        let q = t.uv2.map(|uv| origin + uv * pl.side as f32);
        let area = (q[1] - q[0]).perp_dot(q[2] - q[0]);
        if area.abs() < 1e-12 {
            continue;
        }
        let lo = q[0].min(q[1]).min(q[2]).floor().max(Vec2::ZERO);
        let hi = q[0]
            .max(q[1])
            .max(q[2])
            .ceil()
            .min(Vec2::splat(size as f32));
        for y in lo.y as u32..hi.y as u32 {
            for x in lo.x as u32..hi.x as u32 {
                let c = Vec2::new(x as f32 + 0.5, y as f32 + 0.5);
                let w0 = (q[1] - c).perp_dot(q[2] - c) / area;
                let w1 = (q[2] - c).perp_dot(q[0] - c) / area;
                let w2 = 1.0 - w0 - w1;
                if w0 < -1e-5 || w1 < -1e-5 || w2 < -1e-5 {
                    continue;
                }
                let cell = &mut covered[pl.layer as usize][(y * size + x) as usize];
                if *cell {
                    continue;
                }
                *cell = true;
                let position = t.positions[0] * w0 + t.positions[1] * w1 + t.positions[2] * w2;
                let normal =
                    (t.normals[0] * w0 + t.normals[1] * w1 + t.normals[2] * w2).normalize_or_zero();
                if normal == Vec3::ZERO {
                    continue;
                }
                jobs.push(Job {
                    layer: pl.layer,
                    x,
                    y,
                    position,
                    normal,
                });
            }
        }
    }
    jobs
}

/// Spreads each covered texel's value into the empty texels beside it, one
/// ring per pass. Bilinear sampling at a chart's edge reads the texel outside
/// it; left black, that texel darkens the edge into a visible seam.
fn dilate(image: &mut [Vec3], covered: &[bool], size: u32, passes: u32) {
    let mut mask = covered.to_vec();
    let s = size as i32;
    for _ in 0..passes {
        let mut next = mask.clone();
        let mut out = image.to_vec();
        for y in 0..s {
            for x in 0..s {
                let i = (y * s + x) as usize;
                if mask[i] {
                    continue;
                }
                let (mut sum, mut n) = (Vec3::ZERO, 0);
                for dy in -1..=1 {
                    for dx in -1..=1 {
                        let (nx, ny) = (x + dx, y + dy);
                        if nx < 0 || ny < 0 || nx >= s || ny >= s {
                            continue;
                        }
                        let j = (ny * s + nx) as usize;
                        if mask[j] {
                            sum += image[j];
                            n += 1;
                        }
                    }
                }
                if n > 0 {
                    out[i] = sum / n as f32;
                    next[i] = true;
                }
            }
        }
        image.copy_from_slice(&out);
        mask = next;
    }
}

/// `E / PI` texels as RGBM bytes: `rgb / (m * range)` and `m = max / range`,
/// `m` rounded *up* to the next byte so no channel overflows. The runtime
/// decodes `rgb * a * range`; `range` is `bsengine_core::LIGHTMAP_RGBM_RANGE`.
pub fn encode_rgbm(texels: &[Vec3], range: f32) -> Vec<u8> {
    let mut out = Vec::with_capacity(texels.len() * 4);
    for v in texels {
        let v = v.max(Vec3::ZERO);
        let m = ((v.max_element() / range).clamp(1.0 / 255.0, 1.0) * 255.0).ceil() / 255.0;
        let channel = |c: f32| ((c / (m * range)).clamp(0.0, 1.0) * 255.0).round() as u8;
        out.extend_from_slice(&[
            channel(v.x),
            channel(v.y),
            channel(v.z),
            (m * 255.0).round() as u8,
        ]);
    }
    out
}

/// Directions as RGB bytes, `v * 0.5 + 0.5` -- linear, alpha 255. A
/// direction is at most unit length, so no range is needed; 8 bits give it
/// steps of 1/127, finer than a normal map's.
pub fn encode_direction(directions: &[Vec3]) -> Vec<u8> {
    let channel = |c: f32| ((c.clamp(-1.0, 1.0) * 0.5 + 0.5) * 255.0).round() as u8;
    directions
        .iter()
        .flat_map(|v| [channel(v.x), channel(v.y), channel(v.z), 255])
        .collect()
}

/// The inverse of [`encode_direction`], as the shader computes it.
pub fn decode_direction(rgba: [u8; 4]) -> Vec3 {
    let [r, g, b, _] = rgba.map(|c| c as f32 / 255.0 * 2.0 - 1.0);
    Vec3::new(r, g, b)
}

/// The inverse of [`encode_rgbm`], as the shader computes it.
pub fn decode_rgbm(rgba: [u8; 4], range: f32) -> Vec3 {
    let [r, g, b, a] = rgba.map(|c| c as f32 / 255.0);
    Vec3::new(r, g, b) * a * range
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A unit floor quad facing +Y, `uv2` spanning `[0,1]²`.
    fn floor(renderer: u32, at: Vec3, albedo: Vec3) -> [BakeTriangle; 2] {
        let p = [
            at,
            at + Vec3::X,
            at + Vec3::X + Vec3::NEG_Z,
            at + Vec3::NEG_Z,
        ];
        let uv = [
            Vec2::new(0.0, 0.0),
            Vec2::new(1.0, 0.0),
            Vec2::new(1.0, 1.0),
            Vec2::new(0.0, 1.0),
        ];
        let tri = |a: usize, b: usize, c: usize| BakeTriangle {
            positions: [p[a], p[b], p[c]],
            normals: [Vec3::Y; 3],
            uv2: [uv[a], uv[b], uv[c]],
            albedo,
            emissive: Vec3::ZERO,
            renderer,
        };
        [tri(0, 1, 2), tri(0, 2, 3)]
    }

    fn settings() -> BakeSettings {
        BakeSettings {
            trace: TraceSettings {
                samples: 8,
                bounces: 2,
                seed: 11,
            },
            texels_per_unit: 16.0,
            layer_size: 64,
            max_layers: 4,
            bake_direct: false,
        }
    }

    fn at(out: &BakeOutput, layer: u32, x: u32, y: u32) -> Vec3 {
        out.layers[layer as usize][(y * out.layer_size + x) as usize]
    }

    /// End to end on the closed form: an open floor under a uniform sky
    /// stores the sky's radiance in every texel of its rectangle -- and the
    /// rectangle the placement names is where those texels are.
    #[test]
    fn an_open_floor_bakes_the_sky_into_its_rectangle() {
        let tris = floor(0, Vec3::ZERO, Vec3::splat(0.5));
        let sky = Sky::Uniform(Vec3::new(0.3, 0.6, 0.9));
        let out = CpuBaker.bake(
            &BakeInput {
                triangles: &tris,
                renderers: &[RendererInfo {
                    resolution_scale: 1.0,
                }],
                lights: &[],
                sky: &sky,
            },
            &settings(),
        );
        let pl = out.placements[0];
        assert_eq!(pl.side, 16, "1 m² at 16 texels per unit");
        let mut checked = 0;
        for y in pl.y..pl.y + pl.side {
            for x in pl.x..pl.x + pl.side {
                let v = at(&out, pl.layer, x, y);
                assert!(
                    (v - Vec3::new(0.3, 0.6, 0.9)).length() < 1e-4,
                    "({x}, {y}): {v}"
                );
                checked += 1;
            }
        }
        assert_eq!(checked, 256);
        // Far from any rectangle the layer is black: the bake wrote only
        // where a renderer is (plus the dilated rim).
        assert_eq!(at(&out, 0, 63, 63), Vec3::ZERO);
    }

    /// The rim around a rectangle carries the edge's value, so bilinear
    /// sampling at the edge does not pull in black.
    #[test]
    fn dilation_fills_the_rim_around_a_rectangle() {
        let tris = floor(0, Vec3::ZERO, Vec3::splat(0.5));
        let sky = Sky::Uniform(Vec3::ONE);
        let out = CpuBaker.bake(
            &BakeInput {
                triangles: &tris,
                renderers: &[RendererInfo {
                    resolution_scale: 1.0,
                }],
                lights: &[],
                sky: &sky,
            },
            &settings(),
        );
        let pl = out.placements[0];
        assert!(
            pl.x >= 1,
            "premise: there is a column left of the rectangle"
        );
        let rim = at(&out, pl.layer, pl.x - 1, pl.y + 3);
        assert!(
            (rim - Vec3::ONE).length() < 1e-4,
            "the rim is the edge's value: {rim}"
        );
        // And the directions' rim an edge direction -- a zero there would
        // shade a bumped edge as lit from nowhere (`v = 0`: the plain
        // lightmap), a seam along every chart. Not the one texel beside it
        // exactly: the rim averages the covered texels around it, which for
        // the uniform irradiance above is the same value and for the noisy
        // directions is not.
        let size = out.layer_size;
        let dir = |x: u32, y: u32| out.directions[pl.layer as usize][(y * size + x) as usize];
        let rim = dir(pl.x - 1, pl.y + 3);
        assert!(
            rim.y > 0.3 && rim.length() <= 1.0,
            "the rim's light comes from above, as the floor's does: {rim}"
        );
    }

    /// Two floors side by side, one shadowing nothing: each gets its own
    /// rectangle, and the bake is the same bytes twice.
    #[test]
    fn two_bakes_of_one_scene_are_identical() {
        let mut tris = floor(0, Vec3::ZERO, Vec3::splat(0.8)).to_vec();
        tris.extend(floor(1, Vec3::new(0.0, 0.5, 0.0), Vec3::splat(0.8)));
        let sky = Sky::Uniform(Vec3::ONE);
        let input = BakeInput {
            triangles: &tris,
            renderers: &[
                RendererInfo {
                    resolution_scale: 1.0,
                },
                RendererInfo {
                    resolution_scale: 1.0,
                },
            ],
            lights: &[],
            sky: &sky,
        };
        let a = CpuBaker.bake(&input, &settings());
        let b = CpuBaker.bake(&input, &settings());
        assert_ne!(
            a.placements[0], a.placements[1],
            "each renderer has its own place"
        );
        let bits = |o: &BakeOutput| -> Vec<u32> {
            o.layers
                .iter()
                .flatten()
                .flat_map(|v| v.to_array().map(f32::to_bits))
                .collect()
        };
        assert_eq!(bits(&a), bits(&b));
        // And the lower floor really is shadowed by the upper one: a premise
        // that the scene is not trivially uniform.
        let pl = a.placements[0];
        let under = at(&a, pl.layer, pl.x + pl.side / 2, pl.y + pl.side / 2);
        assert!(
            under.x < 0.9,
            "the lower floor is shaded by the upper: {under}"
        );
    }

    #[test]
    fn rgbm_round_trips_within_a_percent_up_to_the_range() {
        for v in [0.01f32, 0.1, 0.5, 1.0, 2.5, 7.9] {
            let c = Vec3::new(v, v * 0.5, v * 0.25);
            let rgba = encode_rgbm(&[c], 8.0);
            let back = decode_rgbm([rgba[0], rgba[1], rgba[2], rgba[3]], 8.0);
            assert!(
                (back - c).length() <= c.length() * 0.01 + 1e-3,
                "{c} -> {rgba:?} -> {back}"
            );
        }
        assert_eq!(encode_rgbm(&[Vec3::ZERO], 8.0)[..3], [0, 0, 0]);
    }
}
