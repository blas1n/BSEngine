//! An adaptive light-probe volume's probes, baked: what the realtime update
//! (`bsengine_rhi_wgpu::realtime_gi`) computes over frames, computed once by
//! the lightmap bake's tracer and written beside the scene -- the way Unity
//! bakes its Adaptive Probe Volumes and Unreal its Volumetric Lightmap with
//! the lightmaps.
//!
//! Each probe traces a fixed spherical Fibonacci set of rays (the update's
//! set, unturned), projects what they bring back onto L2 SH -- radiance, the
//! scene shader convolves it -- and records each ray's distance into the same
//! 16 x 16 octahedral moments the update keeps, so shading weighs baked
//! probes by visibility exactly as it weighs realtime ones. The light at a
//! hit is the lightmap bake's: every light that takes part in the bake, the
//! sky, and `bounces` bounces.
//!
//! The file holds the layout too (`BrickLayout`): the bricks were cut around
//! the geometry as it was when baked, and the probes are only meaningful where
//! they were.

use crate::probe_bricks::{Brick, BrickLayout, Probe};
use crate::sample::Rng;
use crate::trace::{TraceSettings, World};
use crate::BakeInput;
use glam::Vec3;
#[cfg(feature = "native")]
use rayon::prelude::*;

/// Octahedral moment texels per axis, and per probe: the update's.
pub const MOMENT_RES: u32 = 16;
pub const MOMENT_TEXELS: usize = (MOMENT_RES * MOMENT_RES) as usize;

/// How a probe bake is run.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ProbeBakeSettings {
    /// Rays per probe; more is less noise.
    pub rays: u32,
    /// Bounces after the first hit, as the lightmap bake counts them.
    pub bounces: u32,
    pub seed: u64,
}

/// One baked probe: nine radiance SH coefficients, and per moment texel the
/// mean distance to what it saw that way and its mean square, as two f16
/// (`pack2x16float`'s layout, the mean in the low half).
#[derive(Debug, Clone, PartialEq)]
pub struct BakedProbe {
    pub sh: [Vec3; 9],
    pub moments: Vec<u32>,
}

/// A volume's layout and its baked probes, one per layout probe.
#[derive(Debug, Clone, PartialEq)]
pub struct BakedProbes {
    pub layout: BrickLayout,
    pub probes: Vec<BakedProbe>,
}

/// Bakes every probe of `layout` in the scene of `input`.
pub fn bake_probes(
    input: &BakeInput,
    layout: BrickLayout,
    settings: &ProbeBakeSettings,
) -> BakedProbes {
    let bvh = crate::bvh::Bvh::build(
        &input
            .triangles
            .iter()
            .map(|t| t.positions)
            .collect::<Vec<_>>(),
    );
    let world = World {
        bvh: &bvh,
        triangles: input.triangles,
        lights: input.lights,
        sky: input.sky,
        settings: TraceSettings {
            samples: 1,
            bounces: settings.bounces.max(1),
            seed: settings.seed,
        },
    };
    let rays = settings.rays.max(1);
    let directions: Vec<Vec3> = (0..rays).map(|i| fibonacci(i, rays)).collect();
    let texels: Vec<Vec3> = (0..MOMENT_TEXELS as u32)
        .map(moment_texel_direction)
        .collect();
    // Each probe seeds its own generator from its index, so rayon's order
    // changes nothing: the same bytes on any core count.
    #[cfg(feature = "native")]
    let probes = layout.probes.par_iter().enumerate();
    #[cfg(not(feature = "native"))]
    let probes = layout.probes.iter().enumerate();
    let baked = probes
        .map(|(i, p)| bake_one(&world, p, i as u64, settings.seed, &directions, &texels))
        .collect();
    BakedProbes {
        layout,
        probes: baked,
    }
}

fn bake_one(
    world: &World,
    probe: &Probe,
    index: u64,
    seed: u64,
    directions: &[Vec3],
    texels: &[Vec3],
) -> BakedProbe {
    let mut rng = Rng::new(seed ^ index.wrapping_mul(0x9E37_79B9_7F4A_7C15));
    // The update's range: past it a ray records the range, as a miss does.
    let max_dist = 1.5 * probe.spacing.length();
    let mut sh = [Vec3::ZERO; 9];
    let mut hits = Vec::with_capacity(directions.len());
    for &d in directions {
        let (l, t) = world.probe_ray(probe.position, d, &mut rng);
        for (c, y) in sh.iter_mut().zip(sh_basis(d)) {
            *c += l * y;
        }
        hits.push((d, t.min(max_dist)));
    }
    // Each ray stands for 4 pi / n of the sphere.
    let weight = 4.0 * std::f32::consts::PI / directions.len() as f32;
    for c in &mut sh {
        *c *= weight;
    }
    let moments = texels
        .iter()
        .map(|&dir| {
            // DDGI's cosine power, as the update weighs its rays.
            let (mut m, mut wsum) = ([0.0f32; 2], 0.0f32);
            for &(d, t) in &hits {
                let c = dir.dot(d);
                if c <= 0.0 {
                    continue;
                }
                let w = c.powi(50);
                m[0] += w * t;
                m[1] += w * t * t;
                wsum += w;
            }
            let (mean, sq) = if wsum > 0.0 {
                (m[0] / wsum, m[1] / wsum)
            } else {
                (max_dist, max_dist * max_dist)
            };
            u32::from(f16_bits(mean)) | (u32::from(f16_bits(sq)) << 16)
        })
        .collect();
    BakedProbe { sh, moments }
}

/// Point `i` of `n` on the sphere, evenly spread: `realtime_gi`'s
/// `fibonacci`, term for term.
pub fn fibonacci(i: u32, n: u32) -> Vec3 {
    let z = 1.0 - (2.0 * i as f32 + 1.0) / n as f32;
    let r = (1.0 - z * z).max(0.0).sqrt();
    let phi = i as f32 * 2.399_963_2;
    Vec3::new(r * phi.cos(), r * phi.sin(), z)
}

/// The nine L2 basis functions at `d`, in the order and normalisation of
/// `realtime_gi`'s `accumulate_sh` and `sh.rs`.
fn sh_basis(d: Vec3) -> [f32; 9] {
    [
        0.282_095,
        0.488_603 * d.y,
        0.488_603 * d.z,
        0.488_603 * d.x,
        1.092_548 * d.x * d.y,
        1.092_548 * d.y * d.z,
        0.315_392 * (3.0 * d.z * d.z - 1.0),
        1.092_548 * d.x * d.z,
        0.546_274 * (d.x * d.x - d.y * d.y),
    ]
}

/// The direction moment texel `t` stands for: its centre on the octahedral
/// map, as `probe_grid_wgsl`'s `moment_texel_direction` decodes it.
pub fn moment_texel_direction(t: u32) -> Vec3 {
    let xy = glam::Vec2::new((t % MOMENT_RES) as f32, (t / MOMENT_RES) as f32);
    let e = (xy + glam::Vec2::splat(0.5)) / MOMENT_RES as f32 * 2.0 - glam::Vec2::ONE;
    let mut v = Vec3::new(e.x, e.y, 1.0 - e.x.abs() - e.y.abs());
    if v.z < 0.0 {
        let s = |c: f32| if c >= 0.0 { 1.0 } else { -1.0 };
        v = Vec3::new((1.0 - v.y.abs()) * s(v.x), (1.0 - v.x.abs()) * s(v.y), v.z);
    }
    v.normalize()
}

/// `f` as IEEE half, rounded to nearest even; what WGSL's `pack2x16float`
/// stores. Out-of-range values saturate to infinity, as there.
pub fn f16_bits(f: f32) -> u16 {
    let x = f.to_bits();
    let sign = ((x >> 16) & 0x8000) as u16;
    let exp = ((x >> 23) & 0xff) as i32;
    let mant = x & 0x7f_ffff;
    if exp == 0xff {
        return sign | 0x7c00 | if mant != 0 { 0x200 } else { 0 };
    }
    let e = exp - 127 + 15;
    if e >= 0x1f {
        return sign | 0x7c00;
    }
    if e <= 0 {
        if e < -10 {
            return sign;
        }
        // Subnormal half: the implicit bit made explicit, shifted down.
        let m = mant | 0x80_0000;
        let shift = (14 - e) as u32;
        let half = m >> shift;
        let rem = m & ((1 << shift) - 1);
        let mid = 1 << (shift - 1);
        let round = rem > mid || (rem == mid && half & 1 == 1);
        return sign | (half + u32::from(round)) as u16;
    }
    let half = ((e as u32) << 10) | (mant >> 13);
    let rem = mant & 0x1fff;
    let round = rem > 0x1000 || (rem == 0x1000 && half & 1 == 1);
    sign | (half + u32::from(round)) as u16
}

/// The inverse of [`f16_bits`], for tests and readers of the file.
pub fn f16_to_f32(h: u16) -> f32 {
    let sign = if h & 0x8000 != 0 { -1.0 } else { 1.0 };
    let exp = (h >> 10) & 0x1f;
    let mant = f32::from(h & 0x3ff);
    sign * match exp {
        0 => mant * 2f32.powi(-24),
        0x1f => f32::INFINITY,
        _ => (1.0 + mant / 1024.0) * 2f32.powi(i32::from(exp) - 15),
    }
}

/// The file's first bytes.
const MAGIC: &[u8; 8] = b"BSPROBE1";

/// `probes` as the bytes `--bake` writes: little-endian throughout.
pub fn encode(probes: &BakedProbes) -> Vec<u8> {
    let l = &probes.layout;
    let mut out = Vec::new();
    out.extend_from_slice(MAGIC);
    let f = |v: f32, out: &mut Vec<u8>| out.extend_from_slice(&v.to_le_bytes());
    let u = |v: u32, out: &mut Vec<u8>| out.extend_from_slice(&v.to_le_bytes());
    for v in l.origin.to_array().into_iter().chain(l.extent.to_array()) {
        f(v, &mut out);
    }
    for v in l.top {
        u(v, &mut out);
    }
    u(l.levels, &mut out);
    u(
        u32::from(l.truncated) | u32::from(l.coarsened) << 1,
        &mut out,
    );
    u(l.nodes.len() as u32, &mut out);
    u(l.bricks.len() as u32, &mut out);
    u(l.probes.len() as u32, &mut out);
    for &n in &l.nodes {
        u(n, &mut out);
    }
    for b in &l.bricks {
        for v in b.lo.to_array().into_iter().chain(b.size.to_array()) {
            f(v, &mut out);
        }
        u(b.level, &mut out);
        for &p in &b.probes {
            u(p, &mut out);
        }
    }
    for p in &l.probes {
        for v in p
            .position
            .to_array()
            .into_iter()
            .chain(p.spacing.to_array())
        {
            f(v, &mut out);
        }
    }
    for p in &probes.probes {
        for c in p.sh {
            for v in c.to_array() {
                f(v, &mut out);
            }
        }
        for &m in &p.moments {
            u(m, &mut out);
        }
    }
    out
}

/// Reads what [`encode`] wrote; an error names what was wrong rather than
/// loading a half-read volume.
pub fn decode(bytes: &[u8]) -> Result<BakedProbes, String> {
    struct Reader<'a>(&'a [u8]);
    impl Reader<'_> {
        fn u(&mut self) -> Result<u32, String> {
            let (head, rest) = self
                .0
                .split_first_chunk::<4>()
                .ok_or("the probe file ends early")?;
            self.0 = rest;
            Ok(u32::from_le_bytes(*head))
        }
        fn f(&mut self) -> Result<f32, String> {
            self.u().map(f32::from_bits)
        }
        fn v3(&mut self) -> Result<Vec3, String> {
            Ok(Vec3::new(self.f()?, self.f()?, self.f()?))
        }
    }
    let rest = bytes
        .strip_prefix(MAGIC.as_slice())
        .ok_or("not a baked probe file (no BSPROBE1 header)")?;
    let mut r = Reader(rest);
    let origin = r.v3()?;
    let extent = r.v3()?;
    let top = [r.u()?, r.u()?, r.u()?];
    let levels = r.u()?;
    let flags = r.u()?;
    let (nodes, bricks, count) = (r.u()? as usize, r.u()? as usize, r.u()? as usize);
    // The counts bound every allocation below by what the file can hold.
    if (nodes + bricks * 71 + count * (6 + 27 + MOMENT_TEXELS)) * 4 > bytes.len() {
        return Err("the probe file is shorter than its counts say".into());
    }
    let nodes = (0..nodes).map(|_| r.u()).collect::<Result<Vec<_>, _>>()?;
    let mut brick_list = Vec::with_capacity(bricks);
    for _ in 0..bricks {
        let lo = r.v3()?;
        let size = r.v3()?;
        let level = r.u()?;
        let mut probes = [0u32; 64];
        for p in &mut probes {
            *p = r.u()?;
            if *p as usize >= count {
                return Err(format!("a brick names probe {p}, of {count}"));
            }
        }
        brick_list.push(Brick {
            lo,
            size,
            level,
            probes,
        });
    }
    let mut layout_probes = Vec::with_capacity(count);
    for _ in 0..count {
        layout_probes.push(Probe {
            position: r.v3()?,
            spacing: r.v3()?,
        });
    }
    let mut baked = Vec::with_capacity(count);
    for _ in 0..count {
        let mut sh = [Vec3::ZERO; 9];
        for c in &mut sh {
            *c = r.v3()?;
        }
        let moments = (0..MOMENT_TEXELS)
            .map(|_| r.u())
            .collect::<Result<Vec<_>, _>>()?;
        baked.push(BakedProbe { sh, moments });
    }
    if !r.0.is_empty() {
        return Err(format!("{} bytes after the last probe", r.0.len()));
    }
    Ok(BakedProbes {
        layout: BrickLayout {
            origin,
            extent,
            top,
            levels,
            nodes,
            bricks: brick_list,
            probes: layout_probes,
            truncated: flags & 1 != 0,
            coarsened: flags & 2 != 0,
        },
        probes: baked,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::probe_bricks::BrickSettings;
    use crate::{BakeTriangle, RendererInfo, Sky};

    /// A closed unit box, every face facing in, emitting `e` with albedo
    /// `rho`.
    fn furnace(e: f32, rho: f32) -> Vec<BakeTriangle> {
        let mut out = Vec::new();
        for (min, u, v) in [
            (Vec3::new(0.0, 0.0, 1.0), Vec3::X, Vec3::NEG_Z),
            (Vec3::new(0.0, 1.0, 0.0), Vec3::X, Vec3::Z),
            (Vec3::new(0.0, 0.0, 0.0), Vec3::Y, Vec3::Z),
            (Vec3::new(1.0, 0.0, 1.0), Vec3::Y, Vec3::NEG_Z),
            (Vec3::new(0.0, 0.0, 0.0), Vec3::X, Vec3::Y),
            (Vec3::new(1.0, 0.0, 1.0), Vec3::NEG_X, Vec3::Y),
        ] {
            let n = u.cross(v).normalize();
            let p = [min, min + u, min + u + v, min + v];
            for [a, b, c] in [[0, 1, 2], [0, 2, 3]] {
                out.push(BakeTriangle {
                    positions: [p[a], p[b], p[c]],
                    normals: [n; 3],
                    uv2: [glam::Vec2::ZERO; 3],
                    albedo: Vec3::splat(rho),
                    emissive: Vec3::splat(e),
                    renderer: 0,
                });
            }
        }
        out
    }

    /// The irradiance a surface facing `n` gets from radiance SH `c`: the
    /// cosine lobe's band factors, as the scene shader applies them.
    fn irradiance(c: &[Vec3; 9], n: Vec3) -> Vec3 {
        use std::f32::consts::{FRAC_PI_4, PI};
        let a = [
            PI,
            2.0 * PI / 3.0,
            2.0 * PI / 3.0,
            2.0 * PI / 3.0,
            FRAC_PI_4,
            FRAC_PI_4,
            FRAC_PI_4,
            FRAC_PI_4,
            FRAC_PI_4,
        ];
        let y = sh_basis(n);
        (0..9).map(|i| c[i] * y[i] * a[i]).sum()
    }

    fn layout(tris: &[BakeTriangle]) -> BrickLayout {
        BrickLayout::build(
            &BrickSettings {
                origin: Vec3::splat(0.05),
                extent: Vec3::splat(0.9),
                min_spacing: 0.1,
                max_spacing: 0.3,
                max_probes: 4096,
                max_bricks: 512,
            },
            &tris.iter().map(|t| t.positions).collect::<Vec<_>>(),
        )
    }

    fn bake(tris: &[BakeTriangle], sky: &Sky, bounces: u32) -> BakedProbes {
        bake_probes(
            &BakeInput {
                triangles: tris,
                renderers: &[RendererInfo {
                    resolution_scale: 1.0,
                }],
                lights: &[],
                sky,
            },
            layout(tris),
            &ProbeBakeSettings {
                rays: 256,
                bounces,
                seed: 1,
            },
        )
    }

    /// Closed form, the furnace: inside a closed box every ray brings back
    /// `e (1 + rho + ... + rho^(b-1))` with `b` bounces, so every probe's
    /// E / PI facing anywhere is that -- whatever its brick. The sky (100)
    /// would show a ray that escaped.
    #[test]
    fn baked_probes_in_a_furnace_hold_the_geometric_series() {
        let (e, rho) = (0.4f32, 0.5f32);
        let tris = furnace(e, rho);
        let baked = bake(&tris, &Sky::Uniform(Vec3::splat(100.0)), 3);
        assert_eq!(baked.probes.len(), 1000, "premise: 27 bricks of 0.1");
        let want = e * (1.0 + rho + rho * rho);
        for (i, p) in baked.probes.iter().enumerate() {
            for n in [Vec3::X, Vec3::NEG_Y, Vec3::new(1.0, 1.0, 1.0).normalize()] {
                let got = irradiance(&p.sh, n).x / std::f32::consts::PI;
                assert!(
                    (got - want).abs() < want * 0.01,
                    "probe {i}, {n}: {got} vs {want}"
                );
            }
        }
    }

    /// The moments are distances to what the probe saw, up to the update's
    /// range (1.5 x the spacing's length, 0.26 here): from the probe at 0.15,
    /// the texel facing -x reads the 0.15 m to that wall and its square the
    /// square, and the one facing +x -- the far wall 0.85 away -- the range;
    /// from the corner probe, the wall 0.05 behind it.
    #[test]
    fn a_baked_probes_moments_are_the_distances_it_saw() {
        let tris = furnace(0.4, 0.5);
        let baked = bake(&tris, &Sky::Uniform(Vec3::ZERO), 1);
        let texel_toward = |d: Vec3| {
            (0..MOMENT_TEXELS as u32)
                .max_by(|a, b| {
                    moment_texel_direction(*a)
                        .dot(d)
                        .total_cmp(&moment_texel_direction(*b).dot(d))
                })
                .unwrap() as usize
        };
        let unpack = |m: u32| (f16_to_f32(m as u16), f16_to_f32((m >> 16) as u16));
        let at = |p: Vec3| {
            baked
                .layout
                .probes
                .iter()
                .position(|q| (q.position - p).length() < 1e-4)
                .expect("a probe there")
        };
        let probe = &baked.probes[at(Vec3::splat(0.15))];
        let (mean, sq) = unpack(probe.moments[texel_toward(Vec3::NEG_X)]);
        assert!(
            (mean - 0.15).abs() < 0.01,
            "the wall at -x, 0.15 away: {mean}"
        );
        assert!(
            (sq - mean * mean).abs() < 0.003,
            "a wall, not a spread: {sq} vs {mean}^2"
        );
        let range = 1.5 * (Vec3::splat(0.1)).length();
        let (far, _) = unpack(probe.moments[texel_toward(Vec3::X)]);
        assert!(
            (far - range).abs() < 0.005,
            "the far wall is past the range, which is recorded: {far} vs {range}"
        );
        let corner = &baked.probes[at(Vec3::splat(0.05))];
        let (near, _) = unpack(corner.moments[texel_toward(Vec3::NEG_X)]);
        assert!(
            (near - 0.05).abs() < 0.01,
            "the wall 0.05 behind the corner probe: {near}"
        );
    }

    /// Outside the closed box the walls show their backs: inside something,
    /// as far as a probe there can tell, so nothing comes from them however
    /// they glow -- under a black sky the probe is black -- and the distance
    /// to them is still recorded, the true one, for visibility to drop the
    /// probe from lookups beyond.
    #[test]
    fn a_back_face_brings_nothing_but_its_distance() {
        let tris = furnace(0.4, 0.5);
        let layout = BrickLayout::build(
            &BrickSettings {
                origin: Vec3::new(1.2, 0.0, 0.0),
                extent: Vec3::splat(0.9),
                min_spacing: 0.3,
                max_spacing: 0.3,
                max_probes: 4096,
                max_bricks: 512,
            },
            &[],
        );
        let baked = bake_probes(
            &BakeInput {
                triangles: &tris,
                renderers: &[RendererInfo {
                    resolution_scale: 1.0,
                }],
                lights: &[],
                sky: &Sky::Uniform(Vec3::ZERO),
            },
            layout,
            &ProbeBakeSettings {
                rays: 256,
                bounces: 2,
                seed: 1,
            },
        );
        let at = baked
            .layout
            .probes
            .iter()
            .position(|p| (p.position - Vec3::new(1.2, 0.3, 0.3)).length() < 1e-4)
            .expect("a probe 0.2 outside the box's +x wall");
        let p = &baked.probes[at];
        for n in [Vec3::NEG_X, Vec3::X, Vec3::Y] {
            assert!(
                irradiance(&p.sh, n).length() < 1e-4,
                "{n}: {}",
                irradiance(&p.sh, n)
            );
        }
        let toward = (0..MOMENT_TEXELS as u32)
            .max_by(|a, b| {
                moment_texel_direction(*a)
                    .dot(Vec3::NEG_X)
                    .total_cmp(&moment_texel_direction(*b).dot(Vec3::NEG_X))
            })
            .unwrap() as usize;
        let mean = f16_to_f32(p.moments[toward] as u16);
        assert!(
            (mean - 0.2).abs() < 0.02,
            "the wall's back, 0.2 away: {mean}"
        );
    }

    #[test]
    fn the_file_reads_back_what_was_written_and_refuses_a_bad_one() {
        let tris = furnace(0.4, 0.5);
        let baked = bake(&tris, &Sky::Uniform(Vec3::ZERO), 1);
        let bytes = encode(&baked);
        assert_eq!(decode(&bytes).unwrap(), baked);
        assert!(decode(&bytes[..bytes.len() - 4]).is_err(), "short");
        assert!(decode(b"BSPROBE0").is_err(), "wrong magic");
        let mut extra = bytes.clone();
        extra.push(0);
        assert!(decode(&extra).is_err(), "trailing bytes");
    }

    /// The half conversion is WGSL's `pack2x16float`: exact for what f16
    /// holds, nearest-even between, and it reads back.
    #[test]
    fn f16_round_trips_and_rounds_to_nearest_even() {
        for v in [0.0f32, 1.0, -2.5, 0.000_061_035_156, 65504.0, 0.333_251_95] {
            assert_eq!(f16_to_f32(f16_bits(v)), v, "{v}");
        }
        assert_eq!(f16_bits(1.0), 0x3c00);
        // 1 + 2^-11 is halfway between 1 and the next half: to even, 1.
        assert_eq!(f16_bits(1.0 + 2f32.powi(-11)), 0x3c00);
        assert_eq!(f16_bits(1.0 + 3.0 * 2f32.powi(-11)), 0x3c02);
        assert_eq!(f16_bits(1.0e6), 0x7c00, "saturates");
        // A subnormal half.
        assert!((f16_to_f32(f16_bits(1.0e-6)) - 1.0e-6).abs() < 6.0e-8);
    }
}
