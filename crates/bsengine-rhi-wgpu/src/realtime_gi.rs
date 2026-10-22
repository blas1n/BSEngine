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
//! Everything else that moves is traced as instances, the two-level layout
//! hardware ray tracing gives RTXGI (a bottom-level tree per mesh, a top
//! level of transforms): each mesh a moving entity draws gets its own tree in
//! its own space, built once and appended after the static tree, and every
//! update uploads where each entity is now. A ray meets an instance through
//! its inverse transform, so moving one costs nothing but its 176 bytes.
//! The top level is a list rather than a tree -- every ray tests every
//! instance's box -- which is why there is a cap ([`MAX_GI_INSTANCES`]).
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
// MOMENTS_BASE + 2 * MAX_PROBES * MOMENT_VEC4S: where the probes' offsets
// start -- one entry per probe per half, `xyz` where the probe has been moved
// to from its lattice point (DDGI's relocation). Zero, the buffer's initial
// value, is "on its lattice point". (DDGI also classifies probes stuck inside
// geometry as inactive; visibility already gives such a probe no weight --
// it recorded walls all round, nearer than anything it could light -- and
// this update does not skip tracing them, so there is nothing for a class to
// do here.)
const OFFSETS_BASE: u32 = 598016u;

// Probe `probe`'s offset entry in the half at probe `offset`.
fn probe_offset(probe: u32, offset: u32) -> vec4<f32> {
    return probes.coeffs[OFFSETS_BASE + offset + probe];
}

// An adaptive volume (header flag bit 1, `bsengine_bake::probe_bricks`):
// bricks of 4 x 4 x 4 probes, each level three times finer, found through a
// tree. After the offsets, one copy written by the CPU: per probe its
// position and its spacing; the tree's nodes, four to an entry -- one per
// top-level brick (`resolution` of them, x fastest), each either a brick
// (`-(index + 1)`) or the first of a group's 27 children (its index); and per
// brick its minimum corner, its size and its 64 probe indices, x fastest.
//
// The indices are stored as the floats they are -- 928 as 928.0, exact below
// 2^24 -- not as u32 bits read back with bitcast: the bits of a small integer
// are a denormal float, and D3D12 flushes those to zero on the load, so every
// index read 0 there (and only there: Vulkan keeps them).
const POSITIONS_BASE: u32 = 606208u;
const NODES_BASE: u32 = 614400u;
const BRICKS_BASE: u32 = 614656u;
const BRICK_ENTRIES: u32 = 18u;
const MAX_BRICK: u32 = 511u;
const MAX_BRICK_LEVELS: u32 = 4u;
// No brick: a grid's cell.
const NO_BRICK: u32 = 4294967295u;

fn probes_adaptive() -> bool {
    return (probes.flags & 2u) != 0u;
}
fn brick_node(i: u32) -> f32 {
    // The tree holds under 2 * (MAX_BRICK + 1) nodes; a stray index stays
    // inside the table rather than reading the bricks as nodes.
    let j = min(i, 2u * MAX_BRICK + 1u);
    return probes.coeffs[NODES_BASE + j / 4u][j % 4u];
}

// The eight probes around a point: the cell's lower corner, how far into it
// the point is, the corners' spacing, and the brick it is in (NO_BRICK for a
// grid). A grid's corner is clamped to its last probe on each axis; a
// brick's never needs it (its local cell is 0..2, its probes 0..3).
struct ProbeCell {
    brick: u32,
    i0: vec3<u32>,
    last: vec3<u32>,
    frac: vec3<f32>,
    spacing: vec3<f32>,
};
fn probe_cell(world_pos: vec3<f32>) -> ProbeCell {
    if probes_adaptive() {
        // The walk `BrickLayout::locate` does: down from the top-level
        // brick, a third of the way at a time, to the brick that holds it.
        let top = vec3<f32>(probes.resolution);
        let t = clamp((world_pos - probes.origin) / max(probes.extent, vec3<f32>(1e-6)),
            vec3<f32>(0.0), vec3<f32>(1.0)) * top;
        let cell = clamp(floor(t), vec3<f32>(0.0), top - vec3<f32>(1.0));
        var f = t - cell;
        let c = vec3<u32>(cell);
        var node = brick_node((c.z * probes.resolution.y + c.y) * probes.resolution.x + c.x);
        for (var level = 0u; level < MAX_BRICK_LEVELS; level++) {
            if node < 0.0 {
                break;
            }
            let g = clamp(floor(f * 3.0), vec3<f32>(0.0), vec3<f32>(2.0));
            f = f * 3.0 - g;
            node = brick_node(u32(node) + u32(g.x) + 3u * (u32(g.y) + 3u * u32(g.z)));
        }
        let brick = min(u32(max(-node, 1.0)) - 1u, MAX_BRICK);
        let local = f * 3.0;
        let i0 = clamp(floor(local), vec3<f32>(0.0), vec3<f32>(2.0));
        let size = probes.coeffs[BRICKS_BASE + brick * BRICK_ENTRIES + 1u].xyz;
        return ProbeCell(
            brick,
            vec3<u32>(i0),
            vec3<u32>(3u),
            clamp(local - i0, vec3<f32>(0.0), vec3<f32>(1.0)),
            size / 3.0,
        );
    }
    let g = probe_grid_coord(world_pos);
    let cells = max(vec3<f32>(probes.resolution) - vec3<f32>(1.0), vec3<f32>(1.0));
    let last = vec3<i32>(probes.resolution) - vec3<i32>(1, 1, 1);
    return ProbeCell(
        NO_BRICK,
        vec3<u32>(clamp(vec3<i32>(floor(g)), vec3<i32>(0, 0, 0), last)),
        vec3<u32>(last),
        clamp(g - floor(g), vec3<f32>(0.0, 0.0, 0.0), vec3<f32>(1.0, 1.0, 1.0)),
        probes.extent / cells,
    );
}
// Corner `ci` of a cell (each axis i0 or i0 + 1, clamped) as a probe index:
// a grid's x fastest, then y, then z -- the same order the CPU walks the
// lattice in when it uploads them -- or the brick's own table.
fn probe_cell_index(cell: ProbeCell, ci: vec3<u32>) -> u32 {
    if cell.brick == NO_BRICK {
        return (ci.z * probes.resolution.y + ci.y) * probes.resolution.x + ci.x;
    }
    let k = ci.x + 4u * (ci.y + 4u * ci.z);
    let e = probes.coeffs[BRICKS_BASE + cell.brick * BRICK_ENTRIES + 2u + k / 4u];
    return u32(e[k % 4u]);
}
// An adaptive volume's probe `index`: where it is, and its spacing.
fn brick_probe_position(index: u32) -> vec3<f32> {
    return probes.coeffs[POSITIONS_BASE + 2u * min(index, 4095u)].xyz;
}
fn brick_probe_spacing(index: u32) -> vec3<f32> {
    return probes.coeffs[POSITIONS_BASE + 2u * min(index, 4095u) + 1u].xyz;
}

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
    // The cell around the point: a grid's, or the brick's it is in -- whose
    // spacing is its level's, so the bias and the weights below are the
    // same rule at every level.
    let cell = probe_cell(world_pos);
    let spacing = cell.spacing;
    // Off the surface, so the surface itself is not what a probe saw first:
    // 0.3 of the probe spacing along the normal. Measured in the gi-demo room
    // (games/gi-demo): at 0.1, and at DDGI's view-weighted bias
    // ((0.2 n + 0.8 v) * 0.75 * 0.3 spacing, which leaves 0.045 along the
    // normal), every surface showed one bright patch per probe -- a point
    // that close to the surface sits right at the distance the probe
    // recorded, and the test flickers in and out across the cells. At 0.3
    // the patches are gone (`a_closed_room_is_lit_evenly_through_the_volume`
    // holds it, and fails for both of the others). The price is the one DDGI
    // documents: geometry thinner than this, or nearer a surface, can be
    // judged from its far side.
    let biased = world_pos + n * (0.3 * min(min(spacing.x, spacing.y), spacing.z));
    var weight_sum = 0.0;
    let i0 = cell.i0;
    let i1 = min(i0 + vec3<u32>(1u, 1u, 1u), cell.last);
    let frac = cell.frac;

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
            select(i0.x, i1.x, hi_x),
            select(i0.y, i1.y, hi_y),
            select(i0.z, i1.z, hi_z),
        );
        let index = probe_cell_index(cell, ci);
        if visibility {
            // Where the probe actually is: its lattice point, moved out of
            // whatever geometry it landed in. The trilinear weight above stays
            // the lattice's -- only what the probe can see moved.
            let moved = probe_offset(min(index, 4095u), offset);
            var lattice = probes.origin + spacing * vec3<f32>(ci);
            if cell.brick != NO_BRICK {
                lattice = brick_probe_position(index);
            }
            let probe_pos = lattice + moved.xyz;
            let to_probe = normalize(probe_pos - world_pos);
            let wrap = (dot(to_probe, n) + 1.0) * 0.5;
            w *= wrap * wrap + 0.2;
            let v = biased - probe_pos;
            let dist = length(v);
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
        // MAX_PROBES - 1: a half always holds that many. An index the CPU
        // side never produces would otherwise read past the end of it.
        let base = (offset + min(index, 4095u)) * 9u;
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
    // Whether probes are relocated and classified. Always on; a test turns
    // it off to look at visibility alone.
    relocation: u32,
    // How many of `instances` this update traces.
    instance_count: u32,
};

// A moving entity: its mesh's own tree (`root`, `tri_base`, in `nodes` and
// `tris` after the static tree), the transform into that tree's space, the
// one back out for normals, its world bounds, and its material.
struct GiInstance {
    world_to_local: mat4x4<f32>,
    normal_to_world: mat3x3<f32>,
    lo: vec3<f32>,
    root: u32,
    hi: vec3<f32>,
    tri_base: u32,
    albedo: vec4<f32>,
    emissive: vec4<f32>,
};

@group(0) @binding(0) var<storage, read> nodes: array<Node>;
@group(0) @binding(1) var<storage, read> tris: array<Tri>;
@group(0) @binding(2) var<storage, read_write> probes: ProbeGrid;
@group(0) @binding(3) var<uniform> scene: Scene;
@group(0) @binding(4) var sky: texture_2d<f32>;
@group(0) @binding(5) var<uniform> gi: GiParams;
@group(0) @binding(6) var<storage, read> instances: array<GiInstance>;

// `trace.wgsl`'s instance hooks, over the moving entities. The ray goes into
// the mesh's space through the inverse transform, its direction *not*
// renormalised: a hit at parameter t there is the hit at t in the world, so
// distances compare with the static tree's and with `best` unchanged, under
// any scale.
fn instance_count() -> u32 {
    return gi.instance_count;
}
fn instance_ray(i: u32, o: vec3<f32>, d: vec3<f32>, t_max: f32) -> InstanceRay {
    let inst = instances[i];
    let enters = box_hit(inst.lo, inst.hi, o, safe_inv(d), t_max);
    return InstanceRay(
        inst.root,
        inst.tri_base,
        (inst.world_to_local * vec4<f32>(o, 1.0)).xyz,
        (inst.world_to_local * vec4<f32>(d, 0.0)).xyz,
        enters,
    );
}

// What a hit found: the world-space face normal (turned as `trace.rs` turns
// it), and the surface's material -- the triangle's own for the static tree,
// the instance's for a moving mesh, whose triangles are shared by every
// entity that draws it.
struct HitSurface {
    n: vec3<f32>,
    albedo: vec3<f32>,
    emissive: vec3<f32>,
};
fn hit_surface(hit: Hit) -> HitSurface {
    let tri = tris[hit.tri];
    if hit.instance == NO_HIT {
        return HitSurface(tri.n.xyz, tri.albedo.xyz, tri.emissive.xyz);
    }
    let inst = instances[hit.instance];
    return HitSurface(
        normalize(inst.normal_to_world * tri.n.xyz),
        inst.albedo.xyz,
        inst.emissive.xyz,
    );
}

// Each thread's share of the nine coefficients, summed by the first nine.
var<workgroup> partial: array<array<vec3<f32>, 9>, 64>;
// Every ray's direction and distance, for the moments.
var<workgroup> ray_hits: array<vec4<f32>, 256>;
// The relocation rays: direction, and the distance to what they hit --
// negative for a back face, RELOCATE_MISS for nothing.
var<workgroup> fixed_hits: array<vec4<f32>, 32>;

// DDGI's relocation and classification read their own rays: a fixed set,
// the same every frame, so that the decision to move a probe does not swing
// with the frame's rotation of the shading rays (RTXGI's "fixed rays").
const RELOCATE_RAYS: u32 = 32u;
const RELOCATE_MISS: f32 = 1.0e30;
// More than a quarter of them on back faces: the probe is inside something.
const INSIDE_BACKFACES: u32 = 8u;

// Probe `index`'s lattice point: a grid's, or the brick table's.
fn probe_lattice_position(index: u32) -> vec3<f32> {
    if probes_adaptive() {
        return brick_probe_position(index);
    }
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
    let surface = hit_surface(hit);
    let ng = surface.n;
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
    return RayResult(surface.emissive + surface.albedo * e, dist);
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

// Every coefficient, moment and offset of the grid carried from the read half
// to the write half, before `cs_update` overwrites its slice there. A pass of its own
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
    let moments = gi.total_probes * MOMENT_VEC4S;
    if j < moments {
        probes.coeffs[MOMENTS_BASE + gi.write_offset * MOMENT_VEC4S + j] =
            probes.coeffs[MOMENTS_BASE + gi.read_offset * MOMENT_VEC4S + j];
        return;
    }
    let k = j - moments;
    if k >= gi.total_probes {
        return;
    }
    probes.coeffs[OFFSETS_BASE + gi.write_offset + k] =
        probes.coeffs[OFFSETS_BASE + gi.read_offset + k];
}

// DDGI's relocation (Majercik et al. 2021, as RTXGI implements it): from the
// fixed rays, where to move the probe next, and whether it is inside geometry.
// A probe inside something -- more than a quarter of its rays met back faces
// -- moves through the nearest back face and a little beyond; one too near a
// surface moves away from it, toward the most open direction; one with room
// to spare drifts back toward its lattice point. Never further from that
// point than 0.45 of the spacing on any axis, so a probe stays in its own
// cell.
fn relocate(old: vec3<f32>, spacing: vec3<f32>) -> vec3<f32> {
    let near = 0.25 * min(min(spacing.x, spacing.y), spacing.z);
    var backfaces = 0u;
    var closest_back = RELOCATE_MISS;
    var closest_back_dir = vec3<f32>(0.0);
    var closest_front = RELOCATE_MISS;
    var closest_front_dir = vec3<f32>(0.0);
    var farthest_front = 0.0;
    var farthest_front_dir = vec3<f32>(0.0);
    for (var j = 0u; j < RELOCATE_RAYS; j++) {
        let h = fixed_hits[j];
        if h.w < 0.0 {
            backfaces += 1u;
            if -h.w < closest_back {
                closest_back = -h.w;
                closest_back_dir = h.xyz;
            }
        } else {
            if h.w < closest_front {
                closest_front = h.w;
                closest_front_dir = h.xyz;
            }
            if h.w > farthest_front {
                farthest_front = h.w;
                farthest_front_dir = h.xyz;
            }
        }
    }
    var full = old;
    if backfaces > INSIDE_BACKFACES {
        full = old + closest_back_dir * (closest_back + 0.5 * near);
    } else if closest_front < near {
        // Away, toward the most open direction -- unless that is back
        // toward the surface it is too near.
        if dot(closest_front_dir, farthest_front_dir) <= 0.0 {
            full = old + farthest_front_dir * min(farthest_front, near);
        }
    } else if closest_front > near {
        let back = min(closest_front - near, length(old));
        if back > 0.0 {
            full = old - normalize(old) * back;
        }
    }
    if all(abs(full) < 0.45 * spacing) {
        return full;
    }
    return old;
}

@compute @workgroup_size(64, 1, 1)
fn cs_update(
    @builtin(workgroup_id) wg: vec3<u32>,
    @builtin(local_invocation_index) lid: u32,
) {
    let probe = (gi.first_probe + wg.x) % gi.total_probes;
    // Traced from where the probe has been moved to (last frame's half).
    let old_offset = probe_offset(probe, gi.read_offset);
    let p = probe_lattice_position(probe) + old_offset.xyz;
    let cells = max(vec3<f32>(probes.resolution) - vec3<f32>(1.0), vec3<f32>(1.0));
    // A brick's probe moves within, and traces as far as, its own spacing.
    var spacing = probes.extent / cells;
    if probes_adaptive() {
        spacing = brick_probe_spacing(probe);
    }
    let max_dist = 1.5 * length(spacing);
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
    // The relocation rays: geometry only, no shading.
    if lid < RELOCATE_RAYS {
        let d = fibonacci(lid, RELOCATE_RAYS);
        let hit = traverse(p, d, 3.0e38, false);
        var dist = RELOCATE_MISS;
        if hit.tri != NO_HIT {
            dist = select(hit.t, -hit.t, dot(hit_surface(hit).n, d) >= 0.0);
        }
        fixed_hits[lid] = vec4<f32>(d, dist);
    }
    partial[lid] = acc;
    workgroupBarrier();
    if lid == 0u {
        var moved = vec3<f32>(0.0);
        if gi.relocation != 0u {
            moved = relocate(old_offset.xyz, spacing);
        }
        probes.coeffs[OFFSETS_BASE + gi.write_offset + probe] = vec4<f32>(moved, 0.0);
    }
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
    relocation: u32,
    instance_count: u32,
    /// The uniform struct rounds up to its 16-byte alignment.
    _pad: [u32; 2],
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

/// One moving entity, as the update traces it: its mesh's own tree, met
/// through this frame's transform.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GiDynamic {
    /// The mesh in the [`GpuMeshRegistry`], read back from its buffers once
    /// for every entity that draws it.
    pub mesh_id: u64,
    /// Local to world, this frame.
    pub model: Mat4,
    /// The base colour light bounces with.
    pub albedo: Vec3,
    /// Emitted radiance.
    pub emissive: Vec3,
}

/// Most moving instances one update traces. Every ray tests each one's box,
/// so the cost grows with the count (a linear list, not a tree over them);
/// past this the nearest to the volume are kept, and a warning says so.
pub const MAX_GI_INSTANCES: usize = 256;

/// An adaptive volume's spacings (`LightProbeVolume::min_probe_spacing` and
/// `max_probe_spacing`): its probes are `bsengine_bake::probe_bricks`' bricks
/// around the static set, not the grid's.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AdaptiveSpacing {
    /// Near geometry.
    pub min: f32,
    /// In open space.
    pub max: f32,
}

/// What a frame asks of the realtime volume (`WgpuSurface::set_realtime_gi`).
#[derive(Clone)]
pub struct RealtimeGiInput {
    /// The volume's box and grid, through `LightProbeVolume::clamped_resolution`.
    /// An adaptive volume uses the box only.
    pub volume: ProbeVolumeParams,
    /// `Some` for an adaptive volume (`ProbePlacement::Adaptive`).
    pub adaptive: Option<AdaptiveSpacing>,
    /// Through `LightProbeVolume::clamped_rays_per_probe`.
    pub rays_per_probe: u32,
    /// Through `LightProbeVolume::clamped_hysteresis`.
    pub hysteresis: f32,
    /// The frame's ray budget: `[render] gi_rays_per_frame`.
    pub rays_per_frame: u32,
    /// The static set, in a stable order (a reorder alone rebuilds).
    pub statics: Vec<GiStatic>,
    /// Everything else that takes part, where it is this frame. Moving one
    /// rebuilds nothing; only a mesh no dynamic entity drew before does
    /// (its tree is built once, then shared by every entity drawing it).
    pub dynamics: Vec<GiDynamic>,
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

/// A hash of the static set and of the meshes the moving entities draw (in
/// ascending id order): what decides when the geometry is rebuilt. Where the
/// moving entities are is not in it -- that is uploaded every frame.
fn signature(statics: &[GiStatic], dynamic_meshes: &[u64]) -> u64 {
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
    for id in dynamic_meshes {
        eat(*id as u32);
        eat((*id >> 32) as u32);
    }
    eat(dynamic_meshes.len() as u32);
    h
}

/// The meshes `dynamics` draw that can be traced now: loaded, not skinned
/// (a skinned mesh's buffer is the skinning pass's output, which a tree
/// built once would not follow), each once, in ascending id order. One that
/// is not loaded yet joins the set -- and the signature -- the frame it is,
/// so its tree is built then rather than never.
fn traceable_meshes(registry: &GpuMeshRegistry, dynamics: &[GiDynamic]) -> Vec<u64> {
    let mut ids: Vec<u64> = dynamics
        .iter()
        .map(|d| d.mesh_id)
        .filter(|&id| registry.get(id).is_some() && !registry.is_skinned(id))
        .collect();
    ids.sort_unstable();
    ids.dedup();
    ids
}

/// One mesh's tree, where `land_geometry` laid it out: its root node and its
/// first triangle in the shared buffers, and its local bounds.
#[derive(Clone, Copy, Debug)]
struct MeshTree {
    root: u32,
    tri_base: u32,
    lo: Vec3,
    hi: Vec3,
}

/// `GiInstance` in `UPDATE_WGSL`: one moving entity, as a ray meets it.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct GpuInstance {
    world_to_local: [[f32; 4]; 4],
    /// Columns, each padded to a vec4 as a storage `mat3x3` lays them out.
    normal_to_world: [[f32; 4]; 3],
    lo: [f32; 3],
    root: u32,
    hi: [f32; 3],
    tri_base: u32,
    albedo: [f32; 4],
    emissive: [f32; 4],
}

const _: () = assert!(std::mem::size_of::<GpuInstance>() == 176);

/// `model`'s image of the box `lo..hi`: the box around its eight corners.
fn world_bounds(model: Mat4, lo: Vec3, hi: Vec3) -> (Vec3, Vec3) {
    let mut out = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
    for k in 0..8 {
        let c = Vec3::new(
            if k & 1 == 0 { lo.x } else { hi.x },
            if k & 2 == 0 { lo.y } else { hi.y },
            if k & 4 == 0 { lo.z } else { hi.z },
        );
        let w = model.transform_point3(c);
        out = (out.0.min(w), out.1.max(w));
    }
    out
}

/// This frame's instances, in the order `dynamics` gave them: each one whose
/// mesh has a tree, transformed. Past [`MAX_GI_INSTANCES`], the ones whose
/// bounds lie nearest the volume's centre.
fn instances(
    dynamics: &[GiDynamic],
    trees: &std::collections::HashMap<u64, MeshTree>,
    volume: &ProbeVolumeParams,
) -> (Vec<GpuInstance>, bool) {
    let centre = volume.origin + volume.extent * 0.5;
    let mut out: Vec<(f32, GpuInstance)> = dynamics
        .iter()
        .filter_map(|d| {
            let tree = trees.get(&d.mesh_id)?;
            // A degenerate transform (a zero scale) has no inverse, and
            // nothing to hit either.
            if d.model.determinant().abs() < 1e-12 {
                return None;
            }
            let (lo, hi) = world_bounds(d.model, tree.lo, tree.hi);
            let normal = Mat3::from_mat4(d.model).inverse().transpose();
            let nearest = centre.clamp(lo, hi);
            Some((
                nearest.distance_squared(centre),
                GpuInstance {
                    world_to_local: d.model.inverse().to_cols_array_2d(),
                    normal_to_world: [
                        normal.x_axis.extend(0.0).to_array(),
                        normal.y_axis.extend(0.0).to_array(),
                        normal.z_axis.extend(0.0).to_array(),
                    ],
                    lo: lo.to_array(),
                    root: tree.root,
                    hi: hi.to_array(),
                    tri_base: tree.tri_base,
                    albedo: d.albedo.extend(0.0).to_array(),
                    emissive: d.emissive.extend(0.0).to_array(),
                },
            ))
        })
        .collect();
    let over = out.len() > MAX_GI_INSTANCES;
    if over {
        // Stable, so equally near instances keep their order -- and the
        // update stays the same bytes from run to run.
        out.sort_by(|a, b| a.0.total_cmp(&b.0));
        out.truncate(MAX_GI_INSTANCES);
    }
    (out.into_iter().map(|(_, i)| i).collect(), over)
}

/// The static meshes' buffers, and the moving entities' meshes', being
/// copied back.
struct Readback {
    signature: u64,
    statics: Vec<GiStatic>,
    /// Per static: (vertex offset, vertex bytes, index offset, index bytes)
    /// in `staging`, or `None` for a mesh that was not there to copy.
    ranges: Vec<Option<(u64, u64, u64, u64)>>,
    /// The moving entities' meshes, and where each is in `staging`.
    meshes: Vec<(u64, (u64, u64, u64, u64))>,
    staging: wgpu::Buffer,
    done: Arc<Mutex<Option<bool>>>,
}

struct Geometry {
    signature: u64,
    /// The static tree at 0, then every moving mesh's own tree.
    nodes: wgpu::Buffer,
    tris: wgpu::Buffer,
    /// The static tree's node count (0 for no statics).
    node_count: u32,
    /// Each moving mesh's tree in `nodes` and `tris`.
    trees: std::collections::HashMap<u64, MeshTree>,
    /// The static set's triangles, world space: what an adaptive volume's
    /// bricks are refined around, kept so a changed volume needs no readback.
    static_triangles: Vec<[Vec3; 3]>,
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
    /// The volume the history belongs to: a different one starts over. For
    /// an adaptive volume, also its spacings and the static set its bricks
    /// were cut around.
    volume: Option<(ProbeVolumeParams, Option<(AdaptiveSpacing, u64)>)>,
    /// An adaptive volume's layout, uploaded with it.
    pub(crate) bricks: Option<bsengine_bake::probe_bricks::BrickLayout>,
    /// Whether a layout cut short by the budget has been warned about.
    warned_layout: bool,
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
    /// Whether the update relocates and classifies probes. Always on; a test
    /// turns it off to see visibility on its own, or what relocation adds.
    pub(crate) relocation: bool,
    /// The moving instances, rewritten every update: [`MAX_GI_INSTANCES`]
    /// of [`GpuInstance`].
    instances: wgpu::Buffer,
    /// How many instances the last update traced, for tests.
    pub(crate) instances_traced: u32,
    /// Whether the cap has been warned about (once a process, not a frame).
    warned_over_cap: bool,
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
                storage_layout_entry(6, compute, true),
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
        let instances = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("realtime gi instances"),
            size: (MAX_GI_INSTANCES * std::mem::size_of::<GpuInstance>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self {
            pipelines,
            layout,
            params,
            scene,
            instances,
            instances_traced: 0,
            warned_over_cap: false,
            sky: None,
            geometry: None,
            readback: None,
            volume: None,
            bricks: None,
            warned_layout: false,
            frame: 0,
            cursor: 0,
            current: 0,
            readbacks_started: 0,
            updates: 0,
            visibility: true,
            relocation: true,
        }
    }

    /// Whether the pipelines are built, collecting them from their thread the
    /// first time they are. Never waits.
    fn pipelines_ready(&mut self) -> bool {
        // Only a thread's build is collected here; in a browser they were
        // built in place and `state` is only read.
        #[cfg_attr(target_arch = "wasm32", allow(unused_mut))]
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

    /// Waits for the pipelines, for a test that is not measuring the wait. (A
    /// browser builds them in place, so there is nothing to wait for there.)
    fn wait_for_pipelines(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let mut state = self.pipelines.lock().unwrap_or_else(|e| e.into_inner());
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
    }

    /// Forgets the history (the next update of every probe starts fresh) and
    /// where the round robin was. The geometry is kept.
    pub(crate) fn reset(&mut self) {
        self.volume = None;
        self.cursor = 0;
        self.current = 0;
        self.frame = 0;
    }

    /// Starts copying the static set's meshes back, and the meshes the
    /// moving entities draw, unless that geometry is already built or
    /// already on its way.
    fn request_geometry(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        registry: &GpuMeshRegistry,
        statics: &[GiStatic],
        dynamics: &[GiDynamic],
    ) {
        let dynamic_meshes = traceable_meshes(registry, dynamics);
        let sig = signature(statics, &dynamic_meshes);
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
        let mut meshes = Vec::with_capacity(dynamic_meshes.len());
        for id in dynamic_meshes {
            let mesh = registry.get(id).expect("kept only if loaded");
            let (vb, ib) = (mesh.vertex_buffer.size(), mesh.index_buffer.size());
            meshes.push((id, (size, vb, size + vb, ib)));
            size += vb + ib;
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
        for (id, (vo, vb, io, ib)) in &meshes {
            let mesh = registry.get(*id).expect("kept only if loaded");
            encoder.copy_buffer_to_buffer(&mesh.vertex_buffer, 0, &staging, *vo, *vb);
            encoder.copy_buffer_to_buffer(&mesh.index_buffer, 0, &staging, *io, *ib);
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
            meshes,
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
        // Each moving mesh's triangles in its own space -- the albedo and
        // emissive here are placeholders, the instance's own are read at a
        // hit -- and its bounds there.
        let mut mesh_triangles: Vec<(u64, Vec<BakeTriangle>, Vec3, Vec3)> = Vec::new();
        {
            let bytes = readback.staging.slice(..).get_mapped_range();
            for (id, (vo, vb, io, ib)) in &readback.meshes {
                let vertices: &[Vertex] =
                    bytemuck::cast_slice(&bytes[*vo as usize..(vo + vb) as usize]);
                let indices: &[u32] =
                    bytemuck::cast_slice(&bytes[*io as usize..(io + ib) as usize]);
                let mut local = Vec::new();
                world_triangles(
                    vertices,
                    indices,
                    Mat4::IDENTITY,
                    Vec3::ONE,
                    Vec3::ZERO,
                    0,
                    &mut local,
                );
                if local.is_empty() {
                    continue;
                }
                let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
                for p in local.iter().flat_map(|t| t.positions) {
                    (lo, hi) = (lo.min(p), hi.max(p));
                }
                mesh_triangles.push((*id, local, lo, hi));
            }
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
        let (mut nodes, mut tris) = (geometry.nodes, geometry.tris);
        // Every moving mesh's tree after the static one, each at its own
        // offset: a tree's indices are its own (`trace.wgsl`'s
        // `traverse_tree`), so it is laid out unchanged. Built once per mesh,
        // not per entity, and not again when an entity moves.
        let mut trees = std::collections::HashMap::new();
        for (id, local, lo, hi) in mesh_triangles {
            let tree = GpuGeometry::build(&local);
            trees.insert(
                id,
                MeshTree {
                    root: nodes.len() as u32,
                    tri_base: tris.len() as u32,
                    lo,
                    hi,
                },
            );
            nodes.extend(tree.nodes);
            tris.extend(tree.tris);
        }
        let init = |label, contents: &[u8]| {
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(label),
                contents,
                usage: wgpu::BufferUsages::STORAGE,
            })
        };
        self.geometry = Some(Geometry {
            signature: readback.signature,
            nodes: init("realtime gi nodes", bytemuck::cast_slice(&nodes)),
            tris: init("realtime gi triangles", bytemuck::cast_slice(&tris)),
            node_count: geometry.node_count,
            trees,
            static_triangles: triangles.iter().map(|t| t.positions).collect(),
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
        self.request_geometry(device, queue, registry, &input.statics, &input.dynamics);
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
        let signature = self.geometry.as_ref().expect("checked above").signature;
        let key = (volume, input.adaptive.map(|a| (a, signature)));
        if self.volume != Some(key) {
            // A new volume, or a changed one: its probes owe nothing to the
            // old ones. Both halves zeroed, so every history flag is clear.
            self.reset();
            self.volume = Some(key);
            queue.write_buffer(
                probe_buffer,
                crate::surface::PROBE_GRID_HEADER_SIZE,
                &vec![
                    0u8;
                    (crate::surface::PROBE_GRID_SIZE - crate::surface::PROBE_GRID_HEADER_SIZE)
                        as usize
                ],
            );
            // An adaptive volume's bricks, cut around the static set as it
            // is now, written after the zeroing (the queue keeps the order).
            self.bricks = input.adaptive.map(|a| {
                let layout = bsengine_bake::probe_bricks::BrickLayout::build(
                    &bsengine_bake::probe_bricks::BrickSettings {
                        origin: volume.origin,
                        extent: volume.extent,
                        min_spacing: a.min,
                        max_spacing: a.max,
                        max_probes: bsengine_core::MAX_PROBES,
                        max_bricks: bsengine_core::MAX_PROBE_BRICKS,
                    },
                    &self
                        .geometry
                        .as_ref()
                        .expect("checked above")
                        .static_triangles,
                );
                upload_bricks(queue, probe_buffer, &layout);
                layout
            });
            if let Some(l) = &self.bricks {
                if (l.truncated || l.coarsened) && !self.warned_layout {
                    self.warned_layout = true;
                    tracing::warn!(
                        "realtime GI: the adaptive volume needs more than {} probes or {} bricks \
                         at its spacings; it is {} (fewer levels near geometry, or a coarser \
                         top level) -- raise min_probe_spacing or max_probe_spacing, or shrink \
                         the box",
                        bsengine_core::MAX_PROBES,
                        bsengine_core::MAX_PROBE_BRICKS,
                        if l.coarsened {
                            "coarser everywhere"
                        } else {
                            "coarser near geometry"
                        }
                    );
                }
            }
        }
        let total = match &self.bricks {
            Some(l) => l.probes.len() as u32,
            None => {
                let [rx, ry, rz] = volume.resolution;
                rx * ry * rz
            }
        }
        .min(bsengine_core::MAX_PROBES as u32)
        .max(1);

        let (scene, _) = scene_uniform(node_count, &input.lights, &input.sky);
        queue.write_buffer(&self.scene, 0, bytemuck::bytes_of(&scene));
        let sky_view = self.sky_view(device, queue, &input.sky);

        let rays = input.rays_per_probe.clamp(64, 256).div_ceil(64) * 64;
        // Each probe also traces the 32 fixed relocation rays.
        let per_frame = (input.rays_per_frame / (rays + 32)).clamp(1, total);
        let first = self.cursor % total;
        self.cursor = (first + per_frame) % total;
        // Where the moving entities are now: uploaded every update, so a
        // moved one is traced where it is, with no rebuild.
        let (frame_instances, over_cap) = instances(
            &input.dynamics,
            &self.geometry.as_ref().expect("checked above").trees,
            &volume,
        );
        if over_cap && !self.warned_over_cap {
            self.warned_over_cap = true;
            tracing::warn!(
                "realtime GI: {} moving renderers take part, more than {MAX_GI_INSTANCES}; \
                 the nearest {MAX_GI_INSTANCES} to the volume are traced (ExcludeFromGi \
                 leaves one out)",
                input.dynamics.len()
            );
        }
        if !frame_instances.is_empty() {
            queue.write_buffer(&self.instances, 0, bytemuck::cast_slice(&frame_instances));
        }
        self.instances_traced = frame_instances.len() as u32;
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
            relocation: u32::from(self.relocation),
            instance_count: self.instances_traced,
            _pad: [0; 2],
        };
        queue.write_buffer(&self.params, 0, bytemuck::bytes_of(&params));
        // The header: shade with the half this update writes.
        let header = crate::surface::ProbeGridHeader {
            origin: volume.origin.to_array(),
            enabled: 1,
            extent: volume.extent.to_array(),
            flags: u32::from(self.visibility) | (u32::from(self.bricks.is_some()) << 1),
            // An adaptive volume's top level, which its tree starts from.
            resolution: self.bricks.as_ref().map_or(volume.resolution, |l| l.top),
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
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: self.instances.as_entire_binding(),
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
            // Nine SH entries, 64 moment entries and an offset a probe.
            pass.dispatch_workgroups((total * (9 + 64 + 1)).div_ceil(64), 1, 1);
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

/// The lookup test's shader: `probe_grid_wgsl` and an entry point that runs
/// its `probe_cell` and `probe_cell_index` at each point.
const CELLS_WGSL: &str = r#"
@group(0) @binding(0) var<storage, read> probes: ProbeGrid;
@group(0) @binding(1) var<storage, read> points: array<vec4<f32>>;
@group(0) @binding(2) var<storage, read_write> cells: array<vec4<u32>>;
@compute @workgroup_size(64, 1, 1)
fn cs_cells(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if i >= arrayLength(&points) {
        return;
    }
    let cell = probe_cell(points[i].xyz);
    let i1 = min(cell.i0 + vec3<u32>(1u), cell.last);
    var lo = vec4<u32>(0u);
    var hi = vec4<u32>(0u);
    for (var corner = 0u; corner < 8u; corner++) {
        let ci = vec3<u32>(
            select(cell.i0.x, i1.x, (corner & 1u) != 0u),
            select(cell.i0.y, i1.y, (corner & 2u) != 0u),
            select(cell.i0.z, i1.z, (corner & 4u) != 0u),
        );
        let index = probe_cell_index(cell, ci);
        if corner < 4u {
            lo[corner] = index;
        } else {
            hi[corner - 4u] = index;
        }
    }
    cells[3u * i] = lo;
    cells[3u * i + 1u] = hi;
    cells[3u * i + 2u] = vec4<u32>(bitcast<vec3<u32>>(cell.frac), cell.brick);
}
"#;

/// Runs [`CELLS_WGSL`] over `points` against `probe_buffer`; for
/// `WgpuSurface::probe_cells_for_testing`. Blocks.
pub(crate) fn probe_cells(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    probe_buffer: &wgpu::Buffer,
    points: &[Vec3],
) -> Vec<([u32; 8], Vec3)> {
    let source = format!(
        "const PI: f32 = 3.14159265;\n{}{}",
        probe_grid_wgsl!(),
        CELLS_WGSL
    );
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("probe cells"),
        source: wgpu::ShaderSource::Wgsl(source.into()),
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("probe cells"),
        layout: None,
        module: &module,
        entry_point: "cs_cells",
        compilation_options: Default::default(),
        cache: None,
    });
    let input: Vec<[f32; 4]> = points.iter().map(|p| p.extend(0.0).to_array()).collect();
    let input = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("probe cells points"),
        contents: bytemuck::cast_slice(&input),
        usage: wgpu::BufferUsages::STORAGE,
    });
    let size = (points.len() * 48).max(48) as u64;
    let output = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("probe cells out"),
        size,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    let staging = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("probe cells readback"),
        size,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("probe cells"),
        layout: &pipeline.get_bind_group_layout(0),
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: probe_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: input.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: output.as_entire_binding(),
            },
        ],
    });
    let mut encoder = device.create_command_encoder(&Default::default());
    {
        let mut pass = encoder.begin_compute_pass(&Default::default());
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &bind, &[]);
        pass.dispatch_workgroups((points.len() as u32).div_ceil(64), 1, 1);
    }
    encoder.copy_buffer_to_buffer(&output, 0, &staging, 0, size);
    queue.submit(Some(encoder.finish()));
    let (tx, rx) = std::sync::mpsc::channel();
    staging.slice(..).map_async(wgpu::MapMode::Read, move |r| {
        let _ = tx.send(r);
    });
    crate::surface::wait_for_gpu(device, queue);
    rx.recv()
        .expect("map_async reported")
        .expect("mapping the probe cells failed");
    let words: Vec<u32> = bytemuck::cast_slice(&staging.slice(..).get_mapped_range()).to_vec();
    staging.unmap();
    (0..points.len())
        .map(|i| {
            let w = &words[i * 12..i * 12 + 12];
            let mut corners = [0u32; 8];
            corners.copy_from_slice(&w[..8]);
            (
                corners,
                Vec3::new(
                    f32::from_bits(w[8]),
                    f32::from_bits(w[9]),
                    f32::from_bits(w[10]),
                ),
            )
        })
        .collect()
}

/// Writes an adaptive volume's baked probes over the whole grid: zeroed, then
/// the tables, every probe's SH and moments into the first half, and a header
/// that shades from it with visibility -- what a realtime update leaves after
/// it, without the update. No offsets: a baked probe stays on its lattice
/// point.
pub(crate) fn upload_baked(
    queue: &wgpu::Queue,
    buffer: &wgpu::Buffer,
    baked: &bsengine_bake::probe_bake::BakedProbes,
) {
    use crate::surface::{PROBE_GRID_HEADER_SIZE, PROBE_GRID_SIZE, PROBE_MOMENTS_BASE};
    queue.write_buffer(
        buffer,
        PROBE_GRID_HEADER_SIZE,
        &vec![0u8; (PROBE_GRID_SIZE - PROBE_GRID_HEADER_SIZE) as usize],
    );
    upload_bricks(queue, buffer, &baked.layout);
    let probes = &baked.probes[..baked.probes.len().min(bsengine_core::MAX_PROBES)];
    let sh: Vec<[f32; 4]> = probes
        .iter()
        .flat_map(|p| p.sh.map(|c| c.extend(0.0).to_array()))
        .collect();
    if !sh.is_empty() {
        queue.write_buffer(buffer, PROBE_GRID_HEADER_SIZE, bytemuck::cast_slice(&sh));
    }
    let moments: Vec<u32> = probes
        .iter()
        .flat_map(|p| {
            let mut m = p.moments.clone();
            m.resize(bsengine_bake::probe_bake::MOMENT_TEXELS, 0);
            m
        })
        .collect();
    if !moments.is_empty() {
        queue.write_buffer(
            buffer,
            PROBE_GRID_HEADER_SIZE + PROBE_MOMENTS_BASE * 16,
            bytemuck::cast_slice(&moments),
        );
    }
    let header = crate::surface::ProbeGridHeader {
        origin: baked.layout.origin.to_array(),
        enabled: 1,
        extent: baked.layout.extent.to_array(),
        // Moments filled, and bricks.
        flags: 1 | 2,
        resolution: baked.layout.top,
        coeff_offset: 0,
    };
    queue.write_buffer(buffer, 0, bytemuck::bytes_of(&header));
}

/// Writes an adaptive volume's tables into the probe buffer (the layout at
/// `surface::PROBE_POSITIONS_BASE` on, which `probe_grid_wgsl` reads): per
/// probe its position and spacing, the tree's nodes, and per brick its corner,
/// size and probe indices.
fn upload_bricks(
    queue: &wgpu::Queue,
    buffer: &wgpu::Buffer,
    layout: &bsengine_bake::probe_bricks::BrickLayout,
) {
    use crate::surface::{
        MAX_PROBE_NODES, PROBE_BRICKS_BASE, PROBE_GRID_HEADER_SIZE, PROBE_NODES_BASE,
        PROBE_POSITIONS_BASE,
    };
    let at = |entry: u64| PROBE_GRID_HEADER_SIZE + entry * 16;
    // The layout was cut to these budgets; past them is a bug, and writing
    // on would overrun into the next table.
    let probes = &layout.probes[..layout.probes.len().min(bsengine_core::MAX_PROBES)];
    let bricks = &layout.bricks[..layout.bricks.len().min(bsengine_core::MAX_PROBE_BRICKS)];
    let nodes = &layout.nodes[..layout.nodes.len().min(MAX_PROBE_NODES as usize)];
    debug_assert_eq!(probes.len(), layout.probes.len());
    debug_assert_eq!(bricks.len(), layout.bricks.len());
    debug_assert_eq!(nodes.len(), layout.nodes.len());
    let positions: Vec<[f32; 4]> = probes
        .iter()
        .flat_map(|p| {
            [
                p.position.extend(0.0).to_array(),
                p.spacing.extend(0.0).to_array(),
            ]
        })
        .collect();
    if !positions.is_empty() {
        queue.write_buffer(
            buffer,
            at(PROBE_POSITIONS_BASE),
            bytemuck::cast_slice(&positions),
        );
    }
    // As floats, not bits (see `probe_grid_wgsl`): a brick `-(index + 1)`,
    // a group its first child's index.
    let mut packed: Vec<f32> = nodes
        .iter()
        .map(|&n| {
            if n & bsengine_bake::probe_bricks::LEAF != 0 {
                -(((n & !bsengine_bake::probe_bricks::LEAF) + 1) as f32)
            } else {
                n as f32
            }
        })
        .collect();
    packed.resize(nodes.len().div_ceil(4) * 4, 0.0);
    if !packed.is_empty() {
        queue.write_buffer(buffer, at(PROBE_NODES_BASE), bytemuck::cast_slice(&packed));
    }
    let mut table: Vec<f32> = Vec::with_capacity(bricks.len() * 72);
    for b in bricks {
        table.extend(b.lo.extend(0.0).to_array());
        table.extend(b.size.extend(0.0).to_array());
        table.extend(b.probes.map(|p| p as f32));
    }
    if !table.is_empty() {
        queue.write_buffer(buffer, at(PROBE_BRICKS_BASE), bytemuck::cast_slice(&table));
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

#[cfg(test)]
mod tests {
    /// The shader indexes `coeffs` by literal offsets -- a WGSL `const`
    /// cannot be computed from the Rust side -- so a change to the buffer's
    /// layout on either side alone would have the shader reading moments as
    /// offsets, or SH as moments, with nothing failing to compile.
    #[test]
    fn probe_grid_layout_matches_the_shader() {
        let wgsl = crate::realtime_gi::probe_grid_wgsl!();
        for (name, value) in [
            ("MOMENTS_BASE", crate::surface::PROBE_MOMENTS_BASE),
            ("OFFSETS_BASE", crate::surface::PROBE_OFFSETS_BASE),
            ("POSITIONS_BASE", crate::surface::PROBE_POSITIONS_BASE),
            ("NODES_BASE", crate::surface::PROBE_NODES_BASE),
            ("BRICKS_BASE", crate::surface::PROBE_BRICKS_BASE),
            ("BRICK_ENTRIES", crate::surface::PROBE_BRICK_ENTRIES),
            ("MAX_BRICK", bsengine_core::MAX_PROBE_BRICKS as u64 - 1),
            (
                "MAX_BRICK_LEVELS",
                u64::from(bsengine_bake::probe_bricks::MAX_LEVELS),
            ),
        ] {
            let line = format!("const {name}: u32 = {value}u;");
            assert!(wgsl.contains(&line), "the shader should declare `{line}`");
        }
    }

    use super::{instances, GiDynamic, MeshTree, MAX_GI_INSTANCES};
    use crate::surface::ProbeVolumeParams;
    use glam::{Mat4, Vec3};

    fn one_tree() -> std::collections::HashMap<u64, MeshTree> {
        std::collections::HashMap::from([(
            7,
            MeshTree {
                root: 3,
                tri_base: 5,
                lo: Vec3::splat(-0.5),
                hi: Vec3::splat(0.5),
            },
        )])
    }

    fn at(x: f32) -> GiDynamic {
        GiDynamic {
            mesh_id: 7,
            model: Mat4::from_translation(Vec3::new(x, 0.0, 0.0)),
            albedo: Vec3::ONE,
            emissive: Vec3::ZERO,
        }
    }

    /// Past the cap, the instances nearest the volume are the ones kept:
    /// 300 unit cubes in a row from the volume outwards, of which the first
    /// 256 survive -- in the order given -- and the warning is asked for.
    /// The ones beyond are the ones far from every probe.
    #[test]
    fn past_the_cap_the_instances_nearest_the_volume_are_kept() {
        let volume = ProbeVolumeParams {
            origin: Vec3::splat(-1.0),
            extent: Vec3::splat(2.0),
            resolution: [2, 2, 2],
        };
        // Given farthest first, so keeping the first 256 given would keep
        // the wrong ones.
        let dynamics: Vec<GiDynamic> = (0..300).rev().map(|i| at(i as f32 * 2.0)).collect();
        let (kept, over) = instances(&dynamics, &one_tree(), &volume);
        assert!(over, "300 is past the cap");
        assert_eq!(kept.len(), MAX_GI_INSTANCES);
        let farthest = kept.iter().map(|i| i.lo[0]).fold(f32::MIN, f32::max);
        assert!(
            farthest < 256.0 * 2.0,
            "only the 256 nearest are kept; the farthest kept starts at x = {farthest}"
        );
        let (all, over) = instances(&dynamics[..200], &one_tree(), &volume);
        assert!(
            !over && all.len() == 200,
            "under the cap, every one is kept"
        );
    }

    /// An instance's record: the tree it names, its box in the world, and
    /// the inverse of its transform. One with no tree (its mesh not read
    /// back yet) or no inverse (scaled to nothing) is not traced.
    #[test]
    fn an_instance_carries_its_tree_box_and_inverse() {
        let volume = ProbeVolumeParams {
            origin: Vec3::ZERO,
            extent: Vec3::ONE,
            resolution: [2, 2, 2],
        };
        let mut moved = at(10.0);
        moved.model *= Mat4::from_scale(Vec3::new(2.0, 1.0, 1.0));
        let unloaded = GiDynamic {
            mesh_id: 8,
            ..at(0.0)
        };
        let flat = GiDynamic {
            model: Mat4::from_scale(Vec3::new(1.0, 0.0, 1.0)),
            ..at(0.0)
        };
        let (kept, _) = instances(&[moved, unloaded, flat], &one_tree(), &volume);
        assert_eq!(kept.len(), 1, "only the loaded, invertible one");
        let i = kept[0];
        assert_eq!((i.root, i.tri_base), (3, 5));
        assert_eq!(i.lo, [9.0, -0.5, -0.5]);
        assert_eq!(i.hi, [11.0, 0.5, 0.5]);
        let back = Mat4::from_cols_array_2d(&i.world_to_local) * moved.model;
        assert!(back.abs_diff_eq(Mat4::IDENTITY, 1e-5), "{back}");
    }
}
