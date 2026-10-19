//! The realtime light-probe volume: probes lit by GPU ray tracing, every
//! frame, against the static geometry -- DDGI's update (Majercik et al. 2019)
//! with the probe stored as L2 SH, which is what the scene shader already
//! reads.
//!
//! Per frame, for a slice of the probes (as many as `rays_per_frame` allows,
//! round robin): each traces `rays_per_probe` rays in a spherical Fibonacci
//! set turned by a fresh rotation every frame. A ray that misses sees the
//! sky; one that hits a back face sees nothing (the probe is inside
//! something); one that hits a front face sees
//! `emissive + albedo * (direct E / pi + last frame's probe GI there)` -- the
//! last term is what makes every bounce after the first, for free. The rays
//! are projected to SH and blended into the probe's history with
//! `hysteresis`.
//!
//! The tracing is `bsengine_bake::gpu_scene::TRACE_WGSL`, the lightmap bake's
//! own: the same tree, the same light formulas, the same sky. The geometry is
//! the static set's meshes, read back from their GPU buffers when the set
//! changes -- asynchronously, so no frame waits for it, and the browser can
//! do it too -- and built into a BVH on the CPU.
//!
//! Each ray's distance also goes into a 16 x 16 octahedral map of distance
//! moments per probe, which shading uses to drop the probes that cannot see
//! the point -- DDGI's answer to light leaking through walls.
//!
//! The grid's SH lives in two halves of the probe buffer. Each frame carries
//! the half last written into the other and updates the slice there, reading
//! the old half for the bounce term -- so no probe reads a neighbour that is
//! half updated, and the result is the same whatever order the GPU runs the
//! workgroups in. The header's `coeff_offset` then names the new half for
//! shading.

use crate::mesh::{GpuMeshRegistry, Vertex};
use crate::surface::ProbeVolumeParams;
use bsengine_bake::gpu_scene::{
    scene_uniform, sky_layout_entry, storage_layout_entry, uniform_layout_entry, GpuGeometry,
};
use bsengine_bake::{BakeLight, BakeTriangle, Sky};
use glam::{Mat3, Mat4, Quat, Vec2, Vec3};
use std::sync::{Arc, Mutex};
use wgpu::util::DeviceExt;

/// The probe grid's structure and its sampling, shared by the scene shader
/// (which declares `probes` read-only in group 2) and the update pass (which
/// declares it read-write): one copy of the lookup, so the update's bounce
/// term reads the grid exactly as shading does. Uses `PI` from its includer.
macro_rules! probe_grid_wgsl {
    () => {
        r#"
// The light-probe grid: a header, then nine L2 SH coefficients per probe, x
// fastest, then y, then z -- radiance SH; the cosine convolution happens in
// `eval_probe_sh_at`. Two halves of MAX_PROBES (4096) probes each: a baked
// volume uses the first; a realtime one alternates (`realtime_gi.rs`), and
// `coeff_offset` (in probes) names the half to shade with.
struct ProbeGrid {
    // World-space minimum corner of the volume.
    origin: vec3<f32>,
    enabled: u32,
    // World-space size of the volume (full extent, not half).
    extent: vec3<f32>,
    // Bit 0: the moments below are filled (a realtime volume), so shading
    // weighs each probe by whether it can see the point (see eval).
    flags: u32,
    // Probes per axis, sitting on the grid's lattice points.
    resolution: vec3<u32>,
    coeff_offset: u32,
    // vec4 rather than vec3: a vec3 array element is padded to 16 bytes, and
    // the CPU writes [f32; 4] per coefficient to match. Coefficient 0's `w`
    // is the realtime update's history flag; shading reads only rgb.
    //
    // After the two halves of SH (2 * MAX_PROBES * 9 entries) come two halves
    // of distance moments, MOMENT_VEC4S entries per probe: a 16 x 16
    // octahedral map around the probe, each texel the mean distance to what
    // the probe sees that way and its mean square, as two f16 in one u32 --
    // four texels to an entry, all four written by one thread. Two halves for the
    // reason the SH has two: the update's bounce term reads them too (else a
    // wall's dark side is lit through the wall, and the leak comes back by
    // way of the bounce), and must read last frame's. `coeff_offset` names
    // the half of both.
    coeffs: array<vec4<f32>>,
};

// 16 x 16, DDGI's depth resolution. 8 x 8 was tried and was too coarse: a
// texel spans ~22 degrees, so near a grazing wall the bilinear lookup mixed
// "the wall, 1.6 away" with "nothing for 5" into a mean beyond the point --
// and a probe behind the wall passed the test.
const MOMENT_RES: u32 = 16u;
const MOMENT_TEXELS: u32 = 256u;
// Entries of `coeffs` per probe's moments: four texels each.
const MOMENT_VEC4S: u32 = 64u;
// 2 * MAX_PROBES * 9: where the moments start in `coeffs`.
const MOMENTS_BASE: u32 = 73728u;

// The octahedral map (Cigolle et al.): a direction to [-1, 1]^2 and back.
fn oct_encode(d: vec3<f32>) -> vec2<f32> {
    let p = d.xy / (abs(d.x) + abs(d.y) + abs(d.z));
    if d.z < 0.0 {
        let s = select(vec2<f32>(-1.0), vec2<f32>(1.0), p >= vec2<f32>(0.0));
        return (vec2<f32>(1.0) - abs(p.yx)) * s;
    }
    return p;
}
fn oct_decode(e: vec2<f32>) -> vec3<f32> {
    var v = vec3<f32>(e, 1.0 - abs(e.x) - abs(e.y));
    if v.z < 0.0 {
        let s = select(vec2<f32>(-1.0), vec2<f32>(1.0), v.xy >= vec2<f32>(0.0));
        v = vec3<f32>((vec2<f32>(1.0) - abs(v.yx)) * s, v.z);
    }
    return normalize(v);
}
// The direction a moment texel stands for: its centre on the map.
fn moment_texel_direction(t: u32) -> vec3<f32> {
    let xy = vec2<f32>(f32(t % MOMENT_RES), f32(t / MOMENT_RES));
    return oct_decode((xy + vec2<f32>(0.5)) / f32(MOMENT_RES) * 2.0 - vec2<f32>(1.0));
}
// Texel `t` of probe `probe`'s moments in the half at probe `offset`.
fn moment_texel(probe: u32, t: u32, offset: u32) -> vec2<f32> {
    let entry = probes.coeffs[MOMENTS_BASE + (offset + probe) * MOMENT_VEC4S + t / 4u];
    return unpack2x16float(bitcast<u32>(entry[t % 4u]));
}
// Probe `probe`'s moments towards `d` in the half at probe `offset`,
// bilinear between texel centres, clamped at the map's edge.
fn probe_moments(probe: u32, d: vec3<f32>, offset: u32) -> vec2<f32> {
    let uv = (oct_encode(d) * 0.5 + vec2<f32>(0.5)) * f32(MOMENT_RES) - vec2<f32>(0.5);
    let base = vec2<i32>(floor(uv));
    let f = uv - floor(uv);
    var m = vec2<f32>(0.0);
    for (var k = 0u; k < 4u; k++) {
        let o = vec2<i32>(i32(k & 1u), i32(k >> 1u));
        let c = clamp(base + o, vec2<i32>(0), vec2<i32>(i32(MOMENT_RES) - 1));
        let w = select(1.0 - f.x, f.x, o.x == 1) * select(1.0 - f.y, f.y, o.y == 1);
        let t = u32(c.y) * MOMENT_RES + u32(c.x);
        m += moment_texel(probe, t, offset) * w;
    }
    return m;
}

// True when `world_pos` is inside the volume's box. Outside it there are
// no surrounding probes to interpolate between, so the IBL path stands.
fn inside_probe_volume(world_pos: vec3<f32>) -> bool {
    let local = world_pos - probes.origin;
    return all(local >= vec3<f32>(0.0, 0.0, 0.0)) && all(local <= probes.extent);
}
// `world_pos` in lattice units. Probes sit on the grid's *corners*, so a volume
// with `resolution` probes along an axis spans `resolution - 1` cells, and the
// last probe lands exactly on the far face rather than one cell short of it.
fn probe_grid_coord(world_pos: vec3<f32>) -> vec3<f32> {
    let cells = max(vec3<f32>(probes.resolution) - vec3<f32>(1.0, 1.0, 1.0), vec3<f32>(1.0, 1.0, 1.0));
    let extent = max(probes.extent, vec3<f32>(1e-6, 1e-6, 1e-6));
    let t = (world_pos - probes.origin) / extent;
    return clamp(t, vec3<f32>(0.0, 0.0, 0.0), vec3<f32>(1.0, 1.0, 1.0)) * cells;
}
// Index of probe (ix, iy, iz)'s first coefficient in the half starting at
// probe `offset`. x fastest, then y, then z -- the same order the CPU walks
// the lattice in when it uploads them.
fn probe_coeff_base(ix: u32, iy: u32, iz: u32, offset: u32) -> u32 {
    let idx = (iz * probes.resolution.y + iy) * probes.resolution.x + ix;
    // MAX_PROBES - 1: a half always holds that many. A resolution the CPU
    // side never produces would otherwise read past the end of it.
    return (offset + min(idx, 4095u)) * 9u;
}
// The probe grid's irradiance at `world_pos` for a surface facing `n`, in the
// same units the irradiance cube stores: E / PI, with the Lambert BRDF's 1/PI
// folded in, so callers write `* albedo` with no stray constant exactly as the
// IBL path does. Read from the half at probe `offset`.
//
// The eight surrounding probes' COEFFICIENTS are blended and the result
// evaluated once, rather than evaluating eight probes and blending the results.
// SH evaluation is linear in the coefficients, so the two are equivalent -- and
// this one does an eighth of the evaluation work.
//
// With `visibility` (a realtime grid, whose moments are filled), each corner's
// trilinear weight is also scaled by how much the probe faces the surface and
// by whether it can see it at all -- DDGI's weights: a wrap-shaded back-face
// term, and a Chebyshev test of the distance to the point against the
// distances the probe recorded that way. A probe on the far side of a wall
// recorded the wall, nearer than the point, and drops out; so does one buried
// inside geometry, whose rays all met back faces. The weights are then
// renormalised, so what is left is an interpolation between the probes that
// can see the point.
fn eval_probe_sh_at(world_pos: vec3<f32>, n: vec3<f32>, offset: u32, visibility: bool) -> vec3<f32> {
    let g = probe_grid_coord(world_pos);
    let cells = max(vec3<f32>(probes.resolution) - vec3<f32>(1.0), vec3<f32>(1.0));
    let spacing = probes.extent / cells;
    // Off the surface, so the surface itself is not what a probe saw first --
    // by a tenth of the probe spacing. More pushes a surface near a wall
    // through it (at 0.3, a quad 0.7 from a wall in a 2.4-spaced grid was
    // judged from the wall's far side, and the probes on its own side
    // dropped out).
    let biased = world_pos + n * (0.1 * min(min(spacing.x, spacing.y), spacing.z));
    var weight_sum = 0.0;
    let last = vec3<i32>(probes.resolution) - vec3<i32>(1, 1, 1);
    let i0 = clamp(vec3<i32>(floor(g)), vec3<i32>(0, 0, 0), last);
    let i1 = min(i0 + vec3<i32>(1, 1, 1), last);
    let frac = clamp(g - floor(g), vec3<f32>(0.0, 0.0, 0.0), vec3<f32>(1.0, 1.0, 1.0));

    var blended: array<vec4<f32>, 9>;
    for (var c: u32 = 0u; c < 9u; c++) {
        blended[c] = vec4<f32>(0.0, 0.0, 0.0, 0.0);
    }
    // Corner bit 0 is x, bit 1 is y, bit 2 is z -- the same ordering
    // `trilinear_weights` in sh.rs uses, so the two cannot disagree.
    for (var corner: u32 = 0u; corner < 8u; corner++) {
        let hi_x = (corner & 1u) != 0u;
        let hi_y = (corner & 2u) != 0u;
        let hi_z = (corner & 4u) != 0u;
        var w = select(1.0 - frac.x, frac.x, hi_x)
            * select(1.0 - frac.y, frac.y, hi_y)
            * select(1.0 - frac.z, frac.z, hi_z);
        let ci = vec3<u32>(
            u32(select(i0.x, i1.x, hi_x)),
            u32(select(i0.y, i1.y, hi_y)),
            u32(select(i0.z, i1.z, hi_z)),
        );
        if visibility {
            let probe_pos = probes.origin + spacing * vec3<f32>(ci);
            let to_probe = normalize(probe_pos - world_pos);
            let wrap = (dot(to_probe, n) + 1.0) * 0.5;
            w *= wrap * wrap + 0.2;
            let v = biased - probe_pos;
            let dist = length(v);
            let index = (ci.z * probes.resolution.y + ci.y) * probes.resolution.x + ci.x;
            let m = probe_moments(min(index, 4095u), v / max(dist, 1e-4), offset);
            if dist > m.x {
                let variance = abs(m.y - m.x * m.x);
                let d = dist - m.x;
                let chebyshev = variance / (variance + d * d);
                w *= chebyshev * chebyshev * chebyshev;
            }
            // Very small weights crushed further, as DDGI does, so a probe
            // that barely sees the point does not light it at all.
            if w < 0.2 {
                w *= w * w / 0.04;
            }
        }
        weight_sum += w;
        let base = probe_coeff_base(ci.x, ci.y, ci.z, offset);
        for (var c: u32 = 0u; c < 9u; c++) {
            blended[c] = blended[c] + probes.coeffs[base + c] * w;
        }
    }
    if visibility && weight_sum > 1e-12 {
        for (var c: u32 = 0u; c < 9u; c++) {
            blended[c] = blended[c] / weight_sum;
        }
    }

    // Cosine-lobe convolution constants: the per-band factors that turn a
    // radiance expansion into the irradiance a Lambertian surface receives.
    // Identical to `ShL2::eval_irradiance` in sh.rs, which asserts them.
    let a0 = 3.141593;
    let a1 = 2.094395;
    let a2 = 0.785398;
    var e = blended[0].rgb * (0.282095 * a0);
    e = e + blended[1].rgb * (0.488603 * n.y * a1);
    e = e + blended[2].rgb * (0.488603 * n.z * a1);
    e = e + blended[3].rgb * (0.488603 * n.x * a1);
    e = e + blended[4].rgb * (1.092548 * n.x * n.y * a2);
    e = e + blended[5].rgb * (1.092548 * n.y * n.z * a2);
    e = e + blended[6].rgb * (0.315392 * (3.0 * n.z * n.z - 1.0) * a2);
    e = e + blended[7].rgb * (1.092548 * n.x * n.z * a2);
    e = e + blended[8].rgb * (0.546274 * (n.x * n.x - n.y * n.y) * a2);
    // Ringing in a truncated expansion can drive irradiance below zero, which
    // would darken a surface rather than light it.
    return max(e, vec3<f32>(0.0, 0.0, 0.0)) / PI;
}
// The grid as shading reads it: the half the header names.
fn eval_probe_sh(world_pos: vec3<f32>, n: vec3<f32>) -> vec3<f32> {
    return eval_probe_sh_at(world_pos, n, probes.coeff_offset, (probes.flags & 1u) != 0u);
}
"#
    };
}
pub(crate) use probe_grid_wgsl;

/// The update pass, compiled after `TRACE_WGSL` and [`probe_grid_wgsl`].
const UPDATE_WGSL: &str = r#"
struct GiParams {
    // The slice: probes first_probe .. first_probe + (workgroup count), mod
    // total_probes.
    first_probe: u32,
    total_probes: u32,
    rays: u32,
    frame: u32,
    // The halves, in probes: the bounce term and the history are read from
    // `read_offset`, the update is written to `write_offset`.
    read_offset: u32,
    write_offset: u32,
    hysteresis: f32,
    // Whether the bounce term weighs probes by visibility, as shading does.
    visibility: u32,
    // This frame's turn of the ray set, the same for every probe.
    rotation: mat3x3<f32>,
};

@group(0) @binding(0) var<storage, read> nodes: array<Node>;
@group(0) @binding(1) var<storage, read> tris: array<Tri>;
@group(0) @binding(2) var<storage, read_write> probes: ProbeGrid;
@group(0) @binding(3) var<uniform> scene: Scene;
@group(0) @binding(4) var sky: texture_2d<f32>;
@group(0) @binding(5) var<uniform> gi: GiParams;

// Each thread's share of the nine coefficients, summed by the first nine.
var<workgroup> partial: array<array<vec3<f32>, 9>, 64>;
// Every ray's direction and distance, for the moments.
var<workgroup> ray_hits: array<vec4<f32>, 256>;

fn probe_position(index: u32) -> vec3<f32> {
    let r = probes.resolution;
    let ix = index % r.x;
    let iy = (index / r.x) % r.y;
    let iz = index / (r.x * r.y);
    let cells = max(vec3<f32>(r) - vec3<f32>(1.0), vec3<f32>(1.0));
    return probes.origin + probes.extent * vec3<f32>(f32(ix), f32(iy), f32(iz)) / cells;
}

// Point i of n on the sphere, evenly spread (the spherical Fibonacci set).
fn fibonacci(i: u32, n: u32) -> vec3<f32> {
    let z = 1.0 - (2.0 * f32(i) + 1.0) / f32(n);
    let r = sqrt(max(1.0 - z * z, 0.0));
    let phi = f32(i) * 2.39996323;
    return vec3<f32>(r * cos(phi), r * sin(phi), z);
}

// What a ray from `o` along `d` brings back, and how far it went for the
// moments: the hit distance, capped at `max_dist` (a miss records the cap).
// A back face records its true distance too: a probe inside geometry then
// has walls all around it, nearer than anything it could light, and drops
// out of every lookup beyond them. (DDGI implementations also shorten
// back-face distances; a fifth was tried here and changed no result.)
struct RayResult {
    radiance: vec3<f32>,
    dist: f32,
};
fn trace_ray(o: vec3<f32>, d: vec3<f32>, max_dist: f32) -> RayResult {
    let hit = traverse(o, d, 3.0e38, false);
    if hit.tri == NO_HIT {
        return RayResult(sky_radiance(d), max_dist);
    }
    let tri = tris[hit.tri];
    let ng = tri.n.xyz;
    let dist = min(hit.t, max_dist);
    // A back face: the probe sees the inside of something.
    if dot(ng, d) >= 0.0 {
        return RayResult(vec3<f32>(0.0), dist);
    }
    let y = o + d * hit.t + ng * EPS;
    // The bounce reads last frame's half -- SH and moments -- with visibility,
    // as shading does: without it, the dark side of a wall would take the
    // light of the probes beyond it, and the probes on its own side would
    // see that.
    let e = all_lights(y, ng) / PI
        + eval_probe_sh_at(y, ng, gi.read_offset, gi.visibility != 0u);
    return RayResult(tri.emissive.xyz + tri.albedo.xyz * e, dist);
}

// The nine L2 basis functions of `d`, accumulated into `acc` times `l`.
// Written out rather than returned as an `array<f32, 9>`: FXC, which wgpu's
// D3D12 backend compiles HLSL with, rejects the array-returning function naga
// emits for that ("cannot convert from 'const float[9]' to 'float'").
fn accumulate_sh(acc: ptr<function, array<vec3<f32>, 9>>, d: vec3<f32>, l: vec3<f32>) {
    (*acc)[0] += l * 0.282095;
    (*acc)[1] += l * (0.488603 * d.y);
    (*acc)[2] += l * (0.488603 * d.z);
    (*acc)[3] += l * (0.488603 * d.x);
    (*acc)[4] += l * (1.092548 * d.x * d.y);
    (*acc)[5] += l * (1.092548 * d.y * d.z);
    (*acc)[6] += l * (0.315392 * (3.0 * d.z * d.z - 1.0));
    (*acc)[7] += l * (1.092548 * d.x * d.z);
    (*acc)[8] += l * (0.546274 * (d.x * d.x - d.y * d.y));
}

// Every coefficient and moment of the grid carried from the read half to the
// write half, before `cs_update` overwrites its slice there. A pass of its own
// because a buffer cannot be copied onto itself (WebGPU's
// copyBufferToBuffer refuses source == destination).
@compute @workgroup_size(64, 1, 1)
fn cs_carry(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    let sh = gi.total_probes * 9u;
    if i < sh {
        probes.coeffs[gi.write_offset * 9u + i] = probes.coeffs[gi.read_offset * 9u + i];
        return;
    }
    let j = i - sh;
    if j >= gi.total_probes * MOMENT_VEC4S {
        return;
    }
    probes.coeffs[MOMENTS_BASE + gi.write_offset * MOMENT_VEC4S + j] =
        probes.coeffs[MOMENTS_BASE + gi.read_offset * MOMENT_VEC4S + j];
}

@compute @workgroup_size(64, 1, 1)
fn cs_update(
    @builtin(workgroup_id) wg: vec3<u32>,
    @builtin(local_invocation_index) lid: u32,
) {
    let probe = (gi.first_probe + wg.x) % gi.total_probes;
    let p = probe_position(probe);
    let cells = max(vec3<f32>(probes.resolution) - vec3<f32>(1.0), vec3<f32>(1.0));
    let max_dist = 1.5 * length(probes.extent / cells);
    var acc: array<vec3<f32>, 9>;
    for (var c = 0u; c < 9u; c++) {
        acc[c] = vec3<f32>(0.0);
    }
    for (var r = lid; r < gi.rays; r += 64u) {
        // From the frame, the probe and the ray -- never from scheduling.
        rng_state = pcg(gi.frame ^ pcg(probe ^ pcg(r)));
        let d = normalize(gi.rotation * fibonacci(r, gi.rays));
        let result = trace_ray(p, d, max_dist);
        let l = result.radiance;
        ray_hits[r] = vec4<f32>(d, result.dist);
        accumulate_sh(&acc, d, l);
    }
    partial[lid] = acc;
    workgroupBarrier();
    let read = (gi.read_offset + probe) * 9u;
    let has_history = probes.coeffs[read].w > 0.5;
    // Four moment texels per thread, one entry of `coeffs`: every ray weighted
    // by how close it ran to the texel's direction (DDGI's cosine power of 50).
    {
        let at = MOMENTS_BASE + (gi.write_offset + probe) * MOMENT_VEC4S + lid;
        let before = probes.coeffs[MOMENTS_BASE + (gi.read_offset + probe) * MOMENT_VEC4S + lid];
        var packed = vec4<f32>(0.0);
        for (var k = 0u; k < 4u; k++) {
            let dir = moment_texel_direction(lid * 4u + k);
            var m = vec2<f32>(0.0);
            var wsum = 0.0;
            for (var r = 0u; r < gi.rays; r++) {
                let h = ray_hits[r];
                let w = pow(max(dot(dir, h.xyz), 0.0), 50.0);
                m += w * vec2<f32>(h.w, h.w * h.w);
                wsum += w;
            }
            var v = unpack2x16float(bitcast<u32>(before[k]));
            if wsum > 0.0 {
                let sample = m / wsum;
                v = select(sample, mix(sample, v, gi.hysteresis), has_history);
            }
            packed[k] = bitcast<f32>(pack2x16float(v));
        }
        probes.coeffs[at] = packed;
    }
    if lid < 9u {
        var sum = vec3<f32>(0.0);
        for (var t = 0u; t < 64u; t++) {
            sum += partial[t][lid];
        }
        // The Fibonacci set samples the sphere evenly: each ray stands for
        // 4 pi / n of solid angle.
        let sample = sum * (4.0 * PI / f32(gi.rays));
        var v = sample;
        if has_history {
            v = mix(sample, probes.coeffs[read + lid].xyz, gi.hysteresis);
        }
        probes.coeffs[(gi.write_offset + probe) * 9u + lid] =
            vec4<f32>(v, select(0.0, 1.0, lid == 0u));
    }
}
"#;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct GiParams {
    first_probe: u32,
    total_probes: u32,
    rays: u32,
    frame: u32,
    read_offset: u32,
    write_offset: u32,
    hysteresis: f32,
    visibility: u32,
    /// Columns, each padded to a vec4 as a uniform `mat3x3` lays them out.
    rotation: [[f32; 4]; 3],
}

/// One static entity, as the update traces it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GiStatic {
    /// The mesh in the [`GpuMeshRegistry`], read back from its buffers.
    pub mesh_id: u64,
    /// Local to world.
    pub model: Mat4,
    /// The base colour light bounces with.
    pub albedo: Vec3,
    /// Emitted radiance.
    pub emissive: Vec3,
}

/// What a frame asks of the realtime volume (`WgpuSurface::set_realtime_gi`).
#[derive(Clone)]
pub struct RealtimeGiInput {
    /// The volume's box and grid, through `LightProbeVolume::clamped_resolution`.
    pub volume: ProbeVolumeParams,
    /// Through `LightProbeVolume::clamped_rays_per_probe`.
    pub rays_per_probe: u32,
    /// Through `LightProbeVolume::clamped_hysteresis`.
    pub hysteresis: f32,
    /// The frame's ray budget: `[render] gi_rays_per_frame`.
    pub rays_per_frame: u32,
    /// The static set, in a stable order (a reorder alone rebuilds).
    pub statics: Vec<GiStatic>,
    /// Every light, as realtime GI takes them (`Contribution::Indirect`).
    pub lights: Vec<BakeLight>,
    /// Shared so a frame need not copy a panorama; re-uploaded only when the
    /// `Arc` changes.
    pub sky: Arc<Sky>,
}

/// `row`'s mesh in world space, appended to `out` as `renderer`'s triangles.
/// The one conversion from a mesh to GI triangles: the update's, and the
/// lightmap bake's (through `bsengine_render::gi_scene`).
pub fn world_triangles(
    vertices: &[Vertex],
    indices: &[u32],
    model: Mat4,
    albedo: Vec3,
    emissive: Vec3,
    renderer: u32,
    out: &mut Vec<BakeTriangle>,
) {
    let normal_matrix = Mat3::from_mat4(model).inverse().transpose();
    for tri in indices.chunks_exact(3) {
        let v = |k: usize| &vertices[tri[k] as usize];
        out.push(BakeTriangle {
            positions: [0, 1, 2].map(|k| model.transform_point3(Vec3::from(v(k).position))),
            normals: [0, 1, 2]
                .map(|k| (normal_matrix * Vec3::from(v(k).normal)).normalize_or_zero()),
            uv2: [0, 1, 2].map(|k| Vec2::from(v(k).uv2)),
            albedo,
            emissive,
            renderer,
        });
    }
}

/// A hash of the static set: what decides when the geometry is rebuilt.
fn signature(statics: &[GiStatic]) -> u64 {
    // FNV-1a over the bits: stable across runs, unlike `DefaultHasher`, which
    // a test comparing two surfaces would otherwise trip over.
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut eat = |bits: u32| {
        for b in bits.to_le_bytes() {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    };
    for s in statics {
        eat(s.mesh_id as u32);
        eat((s.mesh_id >> 32) as u32);
        for f in s.model.to_cols_array() {
            eat(f.to_bits());
        }
        for f in s.albedo.to_array().into_iter().chain(s.emissive.to_array()) {
            eat(f.to_bits());
        }
    }
    eat(statics.len() as u32);
    h
}

/// The static meshes' buffers, being copied back.
struct Readback {
    signature: u64,
    statics: Vec<GiStatic>,
    /// Per static: (vertex offset, vertex bytes, index offset, index bytes)
    /// in `staging`, or `None` for a mesh that was not there to copy.
    ranges: Vec<Option<(u64, u64, u64, u64)>>,
    staging: wgpu::Buffer,
    done: Arc<Mutex<Option<bool>>>,
}

struct Geometry {
    signature: u64,
    nodes: wgpu::Buffer,
    tris: wgpu::Buffer,
    node_count: u32,
}

/// The update's two pipelines.
struct Pipelines {
    update: wgpu::ComputePipeline,
    carry: wgpu::ComputePipeline,
}

/// Where the pipelines are. Built on a thread of their own, never in a frame:
/// on D3D12 wgpu compiles HLSL with FXC, which takes seconds over this shader
/// (4.6 s on a desktop, over a minute on CI's software adapter) -- the BVH
/// traversal's indexed stack is what FXC is slow at, and the bake's shader is
/// no faster. A frame that waited would hitch; instead the volume starts
/// updating the frame the pipelines are ready, and until then the grid keeps
/// what it had. (Engines compile pipelines asynchronously for the same
/// reason.) The browser has no threads to spare, and compiles in its GPU
/// process anyway, so there it is built in place.
enum PipelineState {
    #[cfg(not(target_arch = "wasm32"))]
    Compiling(std::thread::JoinHandle<Pipelines>),
    Ready(Pipelines),
    /// The build panicked (a validation error): logged once, and the volume
    /// never updates. (Only a thread's build can end here.)
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    Failed,
}

fn build_pipelines(device: &wgpu::Device, layout: &wgpu::BindGroupLayout) -> Pipelines {
    let source = format!(
        "{}{}{}",
        bsengine_bake::gpu_scene::TRACE_WGSL,
        probe_grid_wgsl!(),
        UPDATE_WGSL
    );
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("realtime gi update"),
        source: wgpu::ShaderSource::Wgsl(source.into()),
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("realtime gi layout"),
        bind_group_layouts: &[layout],
        push_constant_ranges: &[],
    });
    let entry = |entry_point| {
        device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some(entry_point),
            layout: Some(&pipeline_layout),
            module: &module,
            entry_point,
            compilation_options: Default::default(),
            cache: None,
        })
    };
    Pipelines {
        update: entry("cs_update"),
        carry: entry("cs_carry"),
    }
}

/// The realtime volume's GPU side. Made the first frame a realtime volume is
/// asked for, kept while the surface lives.
pub(crate) struct RealtimeGi {
    /// Shared with every other `RealtimeGi` on the same device ([`shared`]).
    pipelines: Arc<Mutex<PipelineState>>,
    layout: Arc<wgpu::BindGroupLayout>,
    params: wgpu::Buffer,
    scene: wgpu::Buffer,
    sky: Option<(Arc<Sky>, wgpu::Texture)>,
    geometry: Option<Geometry>,
    readback: Option<Readback>,
    /// The volume the history belongs to: a different one starts over.
    volume: Option<ProbeVolumeParams>,
    /// Updates run so far, the seed of each one's rotation and rays.
    frame: u32,
    /// Where the next update's slice starts.
    cursor: u32,
    /// The half (0 or 1) last written: next frame's read half.
    current: u32,
    /// Readbacks started, for tests that assert a rebuild happens once.
    pub(crate) readbacks_started: u32,
    /// Updates dispatched, for tests that wait for the geometry to land.
    pub(crate) updates: u32,
    /// Whether shading and the bounce term weigh probes by visibility. Always
    /// on; a test turns it off to measure the leak it prevents.
    pub(crate) visibility: bool,
}

/// One device's layout and pipelines.
struct SharedPipelines {
    device: Arc<wgpu::Device>,
    layout: Arc<wgpu::BindGroupLayout>,
    state: Arc<Mutex<PipelineState>>,
}

/// Every device's pipelines, built the first time a realtime volume appears on
/// it and kept for the process: building them is the expensive part (FXC, see
/// [`PipelineState`]), and a second surface on the same device -- the editor's
/// play view, every test sharing `WgpuSurface::offscreen_for_testing`'s device
/// -- has no reason to pay it again. Holding the device here keeps it alive,
/// which is what a process with one renderer does anyway.
static SHARED: Mutex<Vec<SharedPipelines>> = Mutex::new(Vec::new());

/// `device`'s layout and pipelines, starting their build if this is the first
/// ask.
fn shared(device: &Arc<wgpu::Device>) -> (Arc<wgpu::BindGroupLayout>, Arc<Mutex<PipelineState>>) {
    let mut all = SHARED.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(s) = all.iter().find(|s| Arc::ptr_eq(&s.device, device)) {
        return (Arc::clone(&s.layout), Arc::clone(&s.state));
    }
    let compute = wgpu::ShaderStages::COMPUTE;
    let layout = Arc::new(
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("realtime gi bgl"),
            entries: &[
                storage_layout_entry(0, compute, true),
                storage_layout_entry(1, compute, true),
                storage_layout_entry(2, compute, false),
                uniform_layout_entry(3, compute),
                sky_layout_entry(4, compute),
                uniform_layout_entry(5, compute),
            ],
        }),
    );
    #[cfg(not(target_arch = "wasm32"))]
    let state = {
        let (device, layout) = (Arc::clone(device), Arc::clone(&layout));
        PipelineState::Compiling(
            std::thread::Builder::new()
                .name("realtime gi pipelines".into())
                .spawn(move || build_pipelines(&device, &layout))
                .expect("a thread for the realtime GI pipelines"),
        )
    };
    #[cfg(target_arch = "wasm32")]
    let state = PipelineState::Ready(build_pipelines(device, &layout));
    let state = Arc::new(Mutex::new(state));
    all.push(SharedPipelines {
        device: Arc::clone(device),
        layout: Arc::clone(&layout),
        state: Arc::clone(&state),
    });
    (layout, state)
}

impl RealtimeGi {
    pub(crate) fn new(device: &Arc<wgpu::Device>) -> Self {
        let (layout, pipelines) = shared(device);
        let params = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("realtime gi params"),
            size: std::mem::size_of::<GiParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let scene = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("realtime gi scene"),
            size: std::mem::size_of::<bsengine_bake::gpu_scene::GpuSceneUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self {
            pipelines,
            layout,
            params,
            scene,
            sky: None,
            geometry: None,
            readback: None,
            volume: None,
            frame: 0,
            cursor: 0,
            current: 0,
            readbacks_started: 0,
            updates: 0,
            visibility: true,
        }
    }

    /// Whether the pipelines are built, collecting them from their thread the
    /// first time they are. Never waits.
    fn pipelines_ready(&mut self) -> bool {
        let mut state = self.pipelines.lock().unwrap_or_else(|e| e.into_inner());
        #[cfg(not(target_arch = "wasm32"))]
        if matches!(&*state, PipelineState::Compiling(h) if h.is_finished()) {
            let PipelineState::Compiling(handle) =
                std::mem::replace(&mut *state, PipelineState::Failed)
            else {
                unreachable!("matched above");
            };
            match handle.join() {
                Ok(p) => *state = PipelineState::Ready(p),
                Err(_) => tracing::error!(
                    "realtime GI: building the update pipelines failed; the volume will not update"
                ),
            }
        }
        matches!(&*state, PipelineState::Ready(_))
    }

    /// Waits for the pipelines, for a test that is not measuring the wait.
    fn wait_for_pipelines(&mut self) {
        let mut state = self.pipelines.lock().unwrap_or_else(|e| e.into_inner());
        #[cfg(not(target_arch = "wasm32"))]
        if let PipelineState::Compiling(_) = &*state {
            let PipelineState::Compiling(handle) =
                std::mem::replace(&mut *state, PipelineState::Failed)
            else {
                unreachable!("matched above");
            };
            if let Ok(p) = handle.join() {
                *state = PipelineState::Ready(p);
            }
        }
    }

    /// Forgets the history (the next update of every probe starts fresh) and
    /// where the round robin was. The geometry is kept.
    pub(crate) fn reset(&mut self) {
        self.volume = None;
        self.cursor = 0;
        self.current = 0;
        self.frame = 0;
    }

    /// Starts copying the static set's meshes back, unless that set is
    /// already built or already on its way.
    fn request_geometry(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        registry: &GpuMeshRegistry,
        statics: &[GiStatic],
    ) {
        let sig = signature(statics);
        if self.geometry.as_ref().is_some_and(|g| g.signature == sig)
            || self.readback.as_ref().is_some_and(|r| r.signature == sig)
        {
            return;
        }
        let mut ranges = Vec::with_capacity(statics.len());
        let mut size = 0u64;
        for s in statics {
            match registry.get(s.mesh_id) {
                Some(mesh) if !registry.is_skinned(s.mesh_id) => {
                    let (vb, ib) = (mesh.vertex_buffer.size(), mesh.index_buffer.size());
                    ranges.push(Some((size, vb, size + vb, ib)));
                    size += vb + ib;
                }
                // Not loaded yet, or skinned (its buffer is the skinning
                // pass's output, not a static shape): left out.
                _ => ranges.push(None),
            }
        }
        let staging = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("realtime gi mesh readback"),
            size: size.max(4),
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("realtime gi mesh readback"),
        });
        for (s, range) in statics.iter().zip(&ranges) {
            if let (Some(mesh), Some((vo, vb, io, ib))) = (registry.get(s.mesh_id), range) {
                encoder.copy_buffer_to_buffer(&mesh.vertex_buffer, 0, &staging, *vo, *vb);
                encoder.copy_buffer_to_buffer(&mesh.index_buffer, 0, &staging, *io, *ib);
            }
        }
        queue.submit(Some(encoder.finish()));
        let done = Arc::new(Mutex::new(None));
        let flag = Arc::clone(&done);
        staging.slice(..).map_async(wgpu::MapMode::Read, move |r| {
            *flag.lock().unwrap() = Some(r.is_ok());
        });
        self.readbacks_started += 1;
        self.readback = Some(Readback {
            signature: sig,
            statics: statics.to_vec(),
            ranges,
            staging,
            done,
        });
    }

    /// Builds the geometry from a readback that has landed. Never waits:
    /// one still in flight is left for a later frame.
    fn land_geometry(&mut self, device: &wgpu::Device) {
        let state = self.readback.as_ref().and_then(|r| *r.done.lock().unwrap());
        let Some(ok) = state else {
            return;
        };
        let readback = self.readback.take().expect("checked above");
        if !ok {
            tracing::warn!("realtime GI: reading the static meshes back failed; retrying");
            return;
        }
        let mut triangles = Vec::new();
        {
            let bytes = readback.staging.slice(..).get_mapped_range();
            for (s, range) in readback.statics.iter().zip(&readback.ranges) {
                let Some((vo, vb, io, ib)) = *range else {
                    continue;
                };
                let vertices: &[Vertex] =
                    bytemuck::cast_slice(&bytes[vo as usize..(vo + vb) as usize]);
                let indices: &[u32] = bytemuck::cast_slice(&bytes[io as usize..(io + ib) as usize]);
                world_triangles(
                    vertices,
                    indices,
                    s.model,
                    s.albedo,
                    s.emissive,
                    0,
                    &mut triangles,
                );
            }
        }
        readback.staging.unmap();
        let geometry = GpuGeometry::build(&triangles);
        let init = |label, contents: &[u8]| {
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(label),
                contents,
                usage: wgpu::BufferUsages::STORAGE,
            })
        };
        self.geometry = Some(Geometry {
            signature: readback.signature,
            nodes: init("realtime gi nodes", bytemuck::cast_slice(&geometry.nodes)),
            tris: init(
                "realtime gi triangles",
                bytemuck::cast_slice(&geometry.tris),
            ),
            node_count: geometry.node_count,
        });
    }

    /// The sky texture for `sky`, re-uploaded only when the `Arc` changes.
    fn sky_view(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        sky: &Arc<Sky>,
    ) -> wgpu::TextureView {
        if !self.sky.as_ref().is_some_and(|(s, _)| Arc::ptr_eq(s, sky)) {
            let (_, image) = scene_uniform(0, &[], sky);
            let texture = device.create_texture_with_data(
                queue,
                &wgpu::TextureDescriptor {
                    label: Some("realtime gi sky"),
                    size: wgpu::Extent3d {
                        width: image.width,
                        height: image.height,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Rgba32Float,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                },
                wgpu::util::TextureDataOrder::LayerMajor,
                bytemuck::cast_slice(&image.texels),
            );
            self.sky = Some((Arc::clone(sky), texture));
        }
        self.sky
            .as_ref()
            .expect("set above")
            .1
            .create_view(&Default::default())
    }

    /// One frame's update. Returns whether probes were updated: `false`
    /// while the geometry is still being read back.
    pub(crate) fn update(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        registry: &GpuMeshRegistry,
        probe_buffer: &wgpu::Buffer,
        input: &RealtimeGiInput,
    ) -> bool {
        self.request_geometry(device, queue, registry, &input.statics);
        // Lands a readback the GPU has finished; never blocks.
        device.poll(wgpu::Maintain::Poll);
        self.land_geometry(device);
        let Some(node_count) = self.geometry.as_ref().map(|g| g.node_count) else {
            return false;
        };
        if !self.pipelines_ready() {
            return false;
        }

        let volume = input.volume;
        let [rx, ry, rz] = volume.resolution;
        let total = (rx * ry * rz).min(bsengine_core::MAX_PROBES as u32).max(1);
        if self.volume != Some(volume) {
            // A new volume, or a changed one: its probes owe nothing to the
            // old ones. Both halves zeroed, so every history flag is clear.
            self.reset();
            self.volume = Some(volume);
            queue.write_buffer(
                probe_buffer,
                crate::surface::PROBE_GRID_HEADER_SIZE,
                &vec![
                    0u8;
                    (crate::surface::PROBE_GRID_SIZE - crate::surface::PROBE_GRID_HEADER_SIZE)
                        as usize
                ],
            );
        }

        let (scene, _) = scene_uniform(node_count, &input.lights, &input.sky);
        queue.write_buffer(&self.scene, 0, bytemuck::bytes_of(&scene));
        let sky_view = self.sky_view(device, queue, &input.sky);

        let rays = input.rays_per_probe.clamp(64, 256).div_ceil(64) * 64;
        let per_frame = (input.rays_per_frame / rays).clamp(1, total);
        let first = self.cursor % total;
        self.cursor = (first + per_frame) % total;
        let read = self.current;
        let write = 1 - read;
        let rotation = random_rotation(self.frame);
        let params = GiParams {
            first_probe: first,
            total_probes: total,
            rays,
            frame: self.frame,
            read_offset: read * bsengine_core::MAX_PROBES as u32,
            write_offset: write * bsengine_core::MAX_PROBES as u32,
            hysteresis: input.hysteresis,
            visibility: u32::from(self.visibility),
            rotation: [
                rotation.x_axis.extend(0.0).to_array(),
                rotation.y_axis.extend(0.0).to_array(),
                rotation.z_axis.extend(0.0).to_array(),
            ],
        };
        queue.write_buffer(&self.params, 0, bytemuck::bytes_of(&params));
        // The header: shade with the half this update writes.
        let header = crate::surface::ProbeGridHeader {
            origin: volume.origin.to_array(),
            enabled: 1,
            extent: volume.extent.to_array(),
            flags: u32::from(self.visibility),
            resolution: volume.resolution,
            coeff_offset: write * bsengine_core::MAX_PROBES as u32,
        };
        queue.write_buffer(probe_buffer, 0, bytemuck::bytes_of(&header));

        let geometry = self.geometry.as_ref().expect("checked above");
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("realtime gi bg"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: geometry.nodes.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: geometry.tris.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: probe_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: self.scene.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(&sky_view),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: self.params.as_entire_binding(),
                },
            ],
        });
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("realtime gi update"),
        });
        {
            let state = self.pipelines.lock().unwrap_or_else(|e| e.into_inner());
            let PipelineState::Ready(pipelines) = &*state else {
                unreachable!("checked by pipelines_ready above");
            };
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("realtime gi update"),
                timestamp_writes: None,
            });
            pass.set_bind_group(0, &bind, &[]);
            // Every probe carried over, then the slice overwritten: two
            // dispatches, which WebGPU orders (each is its own usage scope).
            pass.set_pipeline(&pipelines.carry);
            // Nine SH entries and 64 moment entries a probe.
            pass.dispatch_workgroups((total * (9 + 64)).div_ceil(64), 1, 1);
            pass.set_pipeline(&pipelines.update);
            pass.set_bind_group(0, &bind, &[]);
            pass.dispatch_workgroups(per_frame, 1, 1);
        }
        queue.submit(Some(encoder.finish()));
        self.current = write;
        self.frame = self.frame.wrapping_add(1);
        self.updates += 1;
        true
    }
}

/// A rotation drawn from `frame` alone: the same on every machine, and a new
/// one every frame so the ray set covers the sphere over time.
fn random_rotation(frame: u32) -> Mat3 {
    // SplitMix-style mixing of the frame into three uniforms.
    let mut x = u64::from(frame).wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut next = || {
        x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = x;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^= z >> 31;
        (z >> 40) as f32 / (1u64 << 24) as f32
    };
    // Shoemake's uniform random unit quaternion.
    let (u1, u2, u3) = (next(), next(), next());
    let (a, b) = ((1.0 - u1).sqrt(), u1.sqrt());
    let tau = std::f32::consts::TAU;
    let q = Quat::from_xyzw(
        a * (tau * u2).sin(),
        a * (tau * u2).cos(),
        b * (tau * u3).sin(),
        b * (tau * u3).cos(),
    );
    Mat3::from_quat(q.normalize())
}

/// For `tests/realtime_gi.rs`: makes a `RealtimeGi` on `device` and waits for
/// its pipelines, returning how long `new` took and how long until they were
/// built -- what shows the build is off the frame -- and whether they were.
#[doc(hidden)]
pub fn build_pipelines_for_testing(
    device: &Arc<wgpu::Device>,
) -> (std::time::Duration, std::time::Duration, bool) {
    let started = std::time::Instant::now();
    let mut gi = RealtimeGi::new(device);
    let returned = started.elapsed();
    gi.wait_for_pipelines();
    let built = started.elapsed();
    (returned, built, gi.pipelines_ready())
}
