//! SMAA 1x: subpixel morphological antialiasing on the finished LDR frame.
//!
//! A port of the reference implementation (Jimenez et al., `SMAA.hlsl`, MIT
//! -- see `smaa/LICENSE-SMAA.txt`) in three passes:
//!
//! 1. **Edges**: luma edge detection with local contrast adaptation, into an
//!    RG target (left edge, top edge).
//! 2. **Blend weights**: for every edge pixel, search along the edge for both
//!    ends (and, at High/Ultra, diagonals), look at the crossing edges there,
//!    and read the area the true edge covers from the precomputed `AreaTex`.
//! 3. **Neighbourhood blending**: mix each pixel with the neighbour across its
//!    strongest edge by that area.
//!
//! `AreaTex` and `SearchTex` are the reference's own tables, byte for byte
//! (`smaa/area_tex.bin`, `smaa/search_tex.bin`, extracted from `AreaTex.h`
//! and `SearchTex.h`). Regenerating them would mean porting the reference's
//! Python generator for no gain; Bevy and Godot ship the same bytes.
//!
//! The reference assumes Direct3D texture coordinates -- origin top-left, y
//! down -- which is what WebGPU uses too, so the tables are read unflipped.

/// `AreaTex`: 160x560 RG8, orthogonal areas in the left half and diagonal
/// ones in the right, seven subpixel-offset bands stacked vertically.
const AREA_TEX: &[u8] = include_bytes!("smaa/area_tex.bin");
const AREA_TEX_SIZE: (u32, u32) = (160, 560);
/// `SearchTex`: 64x16 R8, how far the last step of an edge search overshot.
const SEARCH_TEX: &[u8] = include_bytes!("smaa/search_tex.bin");
const SEARCH_TEX_SIZE: (u32, u32) = (64, 16);

/// The edges target: two channels, each 0 or 1. Filterable, which the
/// weights pass depends on -- it reads four edges at once by sampling
/// between them (the reference's "pseudo gather4").
const EDGES_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rg8Unorm;
/// The blend weights target: right/left areas of the horizontal edge in
/// `rg`, top/bottom of the vertical one in `ba`.
const WEIGHTS_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct SmaaGpu {
    /// `(1/width, 1/height, width, height)` -- the reference's
    /// `SMAA_RT_METRICS`.
    rt: [f32; 4],
    threshold: f32,
    max_search_steps: f32,
    /// 0 turns diagonal detection off (Low, Medium).
    max_search_steps_diag: f32,
    /// 1 when corner detection runs (High, Ultra).
    corners: u32,
    /// 1 when the colour target is sRGB: it samples as linear light, and
    /// luma edge detection is defined on gamma-encoded values.
    srgb_input: u32,
    _pad: [u32; 3],
}

const COMMON_WGSL: &str = r#"
struct FullscreenOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
}
struct Smaa {
    rt: vec4<f32>,
    threshold: f32,
    max_search_steps: f32,
    max_search_steps_diag: f32,
    corners: u32,
    srgb_input: u32,
    _pad0: u32, _pad1: u32, _pad2: u32,
}

@vertex
fn vs_fullscreen(@builtin(vertex_index) vi: u32) -> FullscreenOut {
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -3.0), vec2<f32>(-1.0, 1.0), vec2<f32>(3.0, 1.0),
    );
    let p = positions[vi];
    var out: FullscreenOut;
    out.pos = vec4<f32>(p.x, p.y, 0.0, 1.0);
    out.uv = vec2<f32>(p.x * 0.5 + 0.5, -p.y * 0.5 + 0.5);
    return out;
}
"#;

/// Pass 1: `SMAALumaEdgeDetectionPS`.
const EDGES_WGSL: &str = r#"
@group(0) @binding(0) var color_tex: texture_2d<f32>;
@group(0) @binding(1) var point_sampler: sampler;
@group(0) @binding(2) var<uniform> smaa: Smaa;

fn linear_to_srgb(c: vec3<f32>) -> vec3<f32> {
    let lo = c * 12.92;
    let hi = 1.055 * pow(max(c, vec3<f32>(0.0)), vec3<f32>(1.0 / 2.4)) - 0.055;
    return select(hi, lo, c <= vec3<f32>(0.0031308));
}

fn luma(uv: vec2<f32>) -> f32 {
    var c = textureSampleLevel(color_tex, point_sampler, uv, 0.0).rgb;
    if smaa.srgb_input != 0u {
        c = linear_to_srgb(c);
    }
    return dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
}

@fragment
fn fs_edges(in: FullscreenOut) -> @location(0) vec4<f32> {
    let rt = smaa.rt;
    let uv = in.pos.xy * rt.xy;
    let off0 = rt.xyxy * vec4<f32>(-1.0, 0.0, 0.0, -1.0) + uv.xyxy;
    let off1 = rt.xyxy * vec4<f32>(1.0, 0.0, 0.0, 1.0) + uv.xyxy;
    let off2 = rt.xyxy * vec4<f32>(-2.0, 0.0, 0.0, -2.0) + uv.xyxy;

    let l = luma(uv);
    let l_left = luma(off0.xy);
    let l_top = luma(off0.zw);
    let delta = abs(vec2<f32>(l) - vec2<f32>(l_left, l_top));
    var edges = step(vec2<f32>(smaa.threshold), delta);
    if dot(edges, vec2<f32>(1.0)) == 0.0 {
        discard;
    }

    // Local contrast adaptation: an edge that is much weaker than another
    // one next to it is the soft side of a strong edge, not an edge.
    let l_right = luma(off1.xy);
    let l_bottom = luma(off1.zw);
    var max_delta = max(delta, abs(vec2<f32>(l) - vec2<f32>(l_right, l_bottom)));
    let l_leftleft = luma(off2.xy);
    let l_toptop = luma(off2.zw);
    max_delta = max(max_delta, abs(vec2<f32>(l_left, l_top) - vec2<f32>(l_leftleft, l_toptop)));
    let final_delta = max(max_delta.x, max_delta.y);
    edges = edges * step(vec2<f32>(final_delta), 2.0 * delta);
    return vec4<f32>(edges, 0.0, 0.0);
}
"#;

/// Pass 2: `SMAABlendingWeightCalculationPS` and everything it calls, for
/// SMAA 1x (every subsample index 0).
const WEIGHTS_WGSL: &str = r#"
@group(0) @binding(0) var edges_tex: texture_2d<f32>;
@group(0) @binding(1) var area_tex: texture_2d<f32>;
@group(0) @binding(2) var search_tex: texture_2d<f32>;
@group(0) @binding(3) var linear_sampler: sampler;
@group(0) @binding(4) var<uniform> smaa: Smaa;

const AREATEX_MAX_DISTANCE: f32 = 16.0;
const AREATEX_MAX_DISTANCE_DIAG: f32 = 20.0;
const AREATEX_PIXEL_SIZE: vec2<f32> = vec2<f32>(1.0 / 160.0, 1.0 / 560.0);
const AREATEX_SUBTEX_SIZE: f32 = 1.0 / 7.0;
const SEARCHTEX_SIZE: vec2<f32> = vec2<f32>(66.0, 33.0);
const SEARCHTEX_PACKED_SIZE: vec2<f32> = vec2<f32>(64.0, 16.0);
const CORNER_ROUNDING_NORM: f32 = 0.25;

fn edges_at(uv: vec2<f32>) -> vec2<f32> {
    return textureSampleLevel(edges_tex, linear_sampler, uv, 0.0).rg;
}
// `SMAASampleLevelZeroOffset`: a whole-texel offset, which is the same
// sample whether the offset is applied by the hardware or to the coordinate.
fn edges_off(uv: vec2<f32>, o: vec2<f32>) -> vec2<f32> {
    return edges_at(uv + o * smaa.rt.xy);
}

// ---- Diagonal search (High and Ultra) ----

fn decode_diag2(e_in: vec2<f32>) -> vec2<f32> {
    var e = e_in;
    e.x = e.x * abs(5.0 * e.x - 5.0 * 0.75);
    return round(e);
}
fn decode_diag4(e_in: vec4<f32>) -> vec4<f32> {
    var e = e_in;
    e.x = e.x * abs(5.0 * e.x - 5.0 * 0.75);
    e.z = e.z * abs(5.0 * e.z - 5.0 * 0.75);
    return round(e);
}

// Returns (distance, last coord.w) and writes the last edge pair to `e`.
fn search_diag1(uv: vec2<f32>, dir: vec2<f32>, e: ptr<function, vec2<f32>>) -> vec2<f32> {
    var coord = vec4<f32>(uv, -1.0, 1.0);
    let t = vec3<f32>(smaa.rt.xy, 1.0);
    while coord.z < smaa.max_search_steps_diag - 1.0 && coord.w > 0.9 {
        let xyz = t * vec3<f32>(dir, 1.0) + coord.xyz;
        coord = vec4<f32>(xyz, coord.w);
        *e = edges_at(coord.xy);
        coord.w = dot(*e, vec2<f32>(0.5));
    }
    return coord.zw;
}

fn search_diag2(uv: vec2<f32>, dir: vec2<f32>, e: ptr<function, vec2<f32>>) -> vec2<f32> {
    var coord = vec4<f32>(uv, -1.0, 1.0);
    coord.x = coord.x + 0.25 * smaa.rt.x;
    let t = vec3<f32>(smaa.rt.xy, 1.0);
    while coord.z < smaa.max_search_steps_diag - 1.0 && coord.w > 0.9 {
        let xyz = t * vec3<f32>(dir, 1.0) + coord.xyz;
        coord = vec4<f32>(xyz, coord.w);
        *e = decode_diag2(edges_at(coord.xy));
        coord.w = dot(*e, vec2<f32>(0.5));
    }
    return coord.zw;
}

fn area_diag(dist: vec2<f32>, e: vec2<f32>, offset: f32) -> vec2<f32> {
    var tc = vec2<f32>(AREATEX_MAX_DISTANCE_DIAG) * e + dist;
    tc = AREATEX_PIXEL_SIZE * tc + 0.5 * AREATEX_PIXEL_SIZE;
    tc.x = tc.x + 0.5;
    tc.y = tc.y + AREATEX_SUBTEX_SIZE * offset;
    return textureSampleLevel(area_tex, linear_sampler, tc, 0.0).rg;
}

fn diag_weights(uv: vec2<f32>, e: vec2<f32>) -> vec2<f32> {
    var weights = vec2<f32>(0.0);
    var d = vec4<f32>(0.0);
    var end = vec2<f32>(0.0);

    if e.x > 0.0 {
        let r = search_diag1(uv, vec2<f32>(-1.0, 1.0), &end);
        d.x = r.x + f32(end.y > 0.9);
        d.z = r.y;
    }
    let r = search_diag1(uv, vec2<f32>(1.0, -1.0), &end);
    d.y = r.x;
    d.w = r.y;

    if d.x + d.y > 2.0 {
        let coords = vec4<f32>(-d.x + 0.25, d.x, d.y, -d.y - 0.25) * smaa.rt.xyxy + uv.xyxy;
        let raw = vec4<f32>(edges_off(coords.xy, vec2<f32>(-1.0, 0.0)), edges_off(coords.zw, vec2<f32>(1.0, 0.0)));
        let dec = decode_diag4(raw);
        // c.yxwz = decode(c.xyzw)
        let c = vec4<f32>(dec.y, dec.x, dec.w, dec.z);
        var cc = vec2<f32>(2.0) * c.xz + c.yw;
        // Drop the crossing edge where the search ran out before the end.
        cc = select(cc, vec2<f32>(0.0), step(vec2<f32>(0.9), d.zw) > vec2<f32>(0.5));
        weights = weights + area_diag(d.xy, cc, 0.0);
    }

    let r2 = search_diag2(uv, vec2<f32>(-1.0, -1.0), &end);
    d.x = r2.x;
    d.z = r2.y;
    if edges_off(uv, vec2<f32>(1.0, 0.0)).x > 0.0 {
        let r3 = search_diag2(uv, vec2<f32>(1.0, 1.0), &end);
        d.y = r3.x + f32(end.y > 0.9);
        d.w = r3.y;
    } else {
        d.y = 0.0;
        d.w = 0.0;
    }

    if d.x + d.y > 2.0 {
        let coords = vec4<f32>(-d.x, -d.x, d.y, d.y) * smaa.rt.xyxy + uv.xyxy;
        let cx = edges_off(coords.xy, vec2<f32>(-1.0, 0.0)).y;
        let cy = edges_off(coords.xy, vec2<f32>(0.0, -1.0)).x;
        let czw = edges_off(coords.zw, vec2<f32>(1.0, 0.0)).yx;
        var cc = vec2<f32>(2.0) * vec2<f32>(cx, czw.x) + vec2<f32>(cy, czw.y);
        cc = select(cc, vec2<f32>(0.0), step(vec2<f32>(0.9), d.zw) > vec2<f32>(0.5));
        weights = weights + area_diag(d.xy, cc, 0.0).yx;
    }
    return weights;
}

// ---- Horizontal and vertical search ----

fn search_length(e: vec2<f32>, offset: f32) -> f32 {
    var scale = SEARCHTEX_SIZE * vec2<f32>(0.5, -1.0);
    var bias = SEARCHTEX_SIZE * vec2<f32>(offset, 1.0);
    scale = scale + vec2<f32>(-1.0, 1.0);
    bias = bias + vec2<f32>(0.5, -0.5);
    scale = scale / SEARCHTEX_PACKED_SIZE;
    bias = bias / SEARCHTEX_PACKED_SIZE;
    return textureSampleLevel(search_tex, linear_sampler, scale * e + bias, 0.0).r;
}

fn search_x_left(uv_in: vec2<f32>, end: f32) -> f32 {
    var uv = uv_in;
    var e = vec2<f32>(0.0, 1.0);
    while uv.x > end && e.y > 0.8281 && e.x == 0.0 {
        e = edges_at(uv);
        uv = uv - vec2<f32>(2.0, 0.0) * smaa.rt.xy;
    }
    let offset = -(255.0 / 127.0) * search_length(e, 0.0) + 3.25;
    return smaa.rt.x * offset + uv.x;
}

fn search_x_right(uv_in: vec2<f32>, end: f32) -> f32 {
    var uv = uv_in;
    var e = vec2<f32>(0.0, 1.0);
    while uv.x < end && e.y > 0.8281 && e.x == 0.0 {
        e = edges_at(uv);
        uv = uv + vec2<f32>(2.0, 0.0) * smaa.rt.xy;
    }
    let offset = -(255.0 / 127.0) * search_length(e, 0.5) + 3.25;
    return -smaa.rt.x * offset + uv.x;
}

fn search_y_up(uv_in: vec2<f32>, end: f32) -> f32 {
    var uv = uv_in;
    var e = vec2<f32>(1.0, 0.0);
    while uv.y > end && e.x > 0.8281 && e.y == 0.0 {
        e = edges_at(uv);
        uv = uv - vec2<f32>(0.0, 2.0) * smaa.rt.xy;
    }
    let offset = -(255.0 / 127.0) * search_length(e.yx, 0.0) + 3.25;
    return smaa.rt.y * offset + uv.y;
}

fn search_y_down(uv_in: vec2<f32>, end: f32) -> f32 {
    var uv = uv_in;
    var e = vec2<f32>(1.0, 0.0);
    while uv.y < end && e.x > 0.8281 && e.y == 0.0 {
        e = edges_at(uv);
        uv = uv + vec2<f32>(0.0, 2.0) * smaa.rt.xy;
    }
    let offset = -(255.0 / 127.0) * search_length(e.yx, 0.5) + 3.25;
    return -smaa.rt.y * offset + uv.y;
}

fn area(dist: vec2<f32>, e1: f32, e2: f32, offset: f32) -> vec2<f32> {
    // Rounding prevents precision errors of bilinear filtering.
    var tc = vec2<f32>(AREATEX_MAX_DISTANCE) * round(4.0 * vec2<f32>(e1, e2)) + dist;
    tc = AREATEX_PIXEL_SIZE * tc + 0.5 * AREATEX_PIXEL_SIZE;
    tc.y = AREATEX_SUBTEX_SIZE * offset + tc.y;
    return textureSampleLevel(area_tex, linear_sampler, tc, 0.0).rg;
}

// ---- Corners (High and Ultra) ----

fn corner_horizontal(weights: vec2<f32>, tc: vec4<f32>, d: vec2<f32>) -> vec2<f32> {
    let left_right = step(d.xy, d.yx);
    var rounding = (1.0 - CORNER_ROUNDING_NORM) * left_right;
    rounding = rounding / (left_right.x + left_right.y);
    var factor = vec2<f32>(1.0);
    factor.x = factor.x - rounding.x * edges_off(tc.xy, vec2<f32>(0.0, 1.0)).x;
    factor.x = factor.x - rounding.y * edges_off(tc.zw, vec2<f32>(1.0, 1.0)).x;
    factor.y = factor.y - rounding.x * edges_off(tc.xy, vec2<f32>(0.0, -2.0)).x;
    factor.y = factor.y - rounding.y * edges_off(tc.zw, vec2<f32>(1.0, -2.0)).x;
    return weights * clamp(factor, vec2<f32>(0.0), vec2<f32>(1.0));
}

fn corner_vertical(weights: vec2<f32>, tc: vec4<f32>, d: vec2<f32>) -> vec2<f32> {
    let left_right = step(d.xy, d.yx);
    var rounding = (1.0 - CORNER_ROUNDING_NORM) * left_right;
    rounding = rounding / (left_right.x + left_right.y);
    var factor = vec2<f32>(1.0);
    factor.x = factor.x - rounding.x * edges_off(tc.xy, vec2<f32>(1.0, 0.0)).y;
    factor.x = factor.x - rounding.y * edges_off(tc.zw, vec2<f32>(1.0, 1.0)).y;
    factor.y = factor.y - rounding.x * edges_off(tc.xy, vec2<f32>(-2.0, 0.0)).y;
    factor.y = factor.y - rounding.y * edges_off(tc.zw, vec2<f32>(-2.0, 1.0)).y;
    return weights * clamp(factor, vec2<f32>(0.0), vec2<f32>(1.0));
}

@fragment
fn fs_weights(in: FullscreenOut) -> @location(0) vec4<f32> {
    let rt = smaa.rt;
    let pix = in.pos.xy;
    let uv = pix * rt.xy;
    let off0 = rt.xyxy * vec4<f32>(-0.25, -0.125, 1.25, -0.125) + uv.xyxy;
    let off1 = rt.xyxy * vec4<f32>(-0.125, -0.25, -0.125, 1.25) + uv.xyxy;
    let off2 = rt.xxyy * vec4<f32>(-2.0, 2.0, -2.0, 2.0) * smaa.max_search_steps
        + vec4<f32>(off0.xz, off1.yw);

    var weights = vec4<f32>(0.0);
    var e = edges_at(uv);

    if e.y > 0.0 {
        // Edge at north. Diagonals have both a north and a west edge, so
        // searching from one of them is enough; a diagonal found wins.
        var found_diag = false;
        if smaa.max_search_steps_diag > 0.0 {
            let dw = diag_weights(uv, e);
            weights.x = dw.x;
            weights.y = dw.y;
            found_diag = dw.x != -dw.y;
        }
        if !found_diag {
            var coords = vec3<f32>(search_x_left(off0.xy, off2.x), off1.y, 0.0);
            var d = vec2<f32>(coords.x, 0.0);
            // Crossing edges, two at a time: sampling at -0.25 tells them apart.
            let e1 = edges_at(coords.xy).x;
            coords.z = search_x_right(off0.zw, off2.y);
            d.y = coords.z;
            d = abs(round(rt.zz * d - pix.xx));
            let e2 = edges_off(coords.zy, vec2<f32>(1.0, 0.0)).x;
            var w = area(sqrt(d), e1, e2, 0.0);
            if smaa.corners != 0u {
                w = corner_horizontal(w, vec4<f32>(coords.x, uv.y, coords.z, uv.y), d);
            }
            weights.x = w.x;
            weights.y = w.y;
        } else {
            // Skip vertical processing.
            e.x = 0.0;
        }
    }

    if e.x > 0.0 {
        // Edge at west.
        var coords = vec3<f32>(off0.x, search_y_up(off1.xy, off2.z), 0.0);
        var d = vec2<f32>(coords.y, 0.0);
        let e1 = edges_at(coords.xy).y;
        coords.z = search_y_down(off1.zw, off2.w);
        d.y = coords.z;
        d = abs(round(rt.ww * d - pix.yy));
        let e2 = edges_off(coords.xz, vec2<f32>(0.0, 1.0)).y;
        var w = area(sqrt(d), e1, e2, 0.0);
        if smaa.corners != 0u {
            w = corner_vertical(w, vec4<f32>(uv.x, coords.y, uv.x, coords.z), d);
        }
        weights.z = w.x;
        weights.w = w.y;
    }
    return weights;
}
"#;

/// Pass 3: `SMAANeighborhoodBlendingPS`.
const BLEND_WGSL: &str = r#"
@group(0) @binding(0) var color_tex: texture_2d<f32>;
@group(0) @binding(1) var weights_tex: texture_2d<f32>;
@group(0) @binding(2) var linear_sampler: sampler;
@group(0) @binding(3) var<uniform> smaa: Smaa;

@fragment
fn fs_blend(in: FullscreenOut) -> @location(0) vec4<f32> {
    let rt = smaa.rt;
    let uv = in.pos.xy * rt.xy;
    let off = rt.xyxy * vec4<f32>(1.0, 0.0, 0.0, 1.0) + uv.xyxy;

    // This pixel's weights: right (from the right neighbour's left), top,
    // and its own bottom and left.
    var a: vec4<f32>;
    a.x = textureSampleLevel(weights_tex, linear_sampler, off.xy, 0.0).a;
    a.y = textureSampleLevel(weights_tex, linear_sampler, off.zw, 0.0).g;
    let own = textureSampleLevel(weights_tex, linear_sampler, uv, 0.0);
    a.w = own.x;
    a.z = own.z;

    if dot(a, vec4<f32>(1.0)) < 1e-5 {
        return textureSampleLevel(color_tex, linear_sampler, uv, 0.0);
    }
    let h = max(a.x, a.z) > max(a.y, a.w);
    var blending_offset = vec4<f32>(0.0, a.y, 0.0, a.w);
    var blending_weight = a.yw;
    if h {
        blending_offset = vec4<f32>(a.x, 0.0, a.z, 0.0);
        blending_weight = a.xz;
    }
    blending_weight = blending_weight / dot(blending_weight, vec2<f32>(1.0));
    // Bilinear filtering mixes this pixel with the chosen neighbour.
    let bc = blending_offset * vec4<f32>(rt.xy, -rt.xy) + uv.xyxy;
    return blending_weight.x * textureSampleLevel(color_tex, linear_sampler, bc.xy, 0.0)
        + blending_weight.y * textureSampleLevel(color_tex, linear_sampler, bc.zw, 0.0);
}
"#;

struct Targets {
    size: (u32, u32),
    _edges: crate::profiler::TrackedTexture,
    edges_view: wgpu::TextureView,
    _weights: crate::profiler::TrackedTexture,
    weights_view: wgpu::TextureView,
    _out: crate::profiler::TrackedTexture,
    out_view: wgpu::TextureView,
    /// The output in the post chain's `tex2d` layout, for the TAA resolve.
    out_bg: wgpu::BindGroup,
    weights_bg: wgpu::BindGroup,
}

/// This frame's bind groups over the colour input. Made every frame the pass
/// runs rather than kept, so a resize that replaces the colour target can
/// never leave SMAA reading the old one.
struct FrameBindGroups {
    edges_bg: wgpu::BindGroup,
    blend_bg: wgpu::BindGroup,
}

/// The SMAA passes, their tables and their targets. Everything sized or
/// uploaded is made the first frame it is needed: a project that never turns
/// SMAA on never pays for its 180 KB of tables or three targets.
pub(crate) struct SmaaPass {
    edges_pipeline: wgpu::RenderPipeline,
    weights_pipeline: wgpu::RenderPipeline,
    blend_pipeline: wgpu::RenderPipeline,
    edges_bgl: wgpu::BindGroupLayout,
    weights_bgl: wgpu::BindGroupLayout,
    blend_bgl: wgpu::BindGroupLayout,
    point_sampler: wgpu::Sampler,
    linear_sampler: wgpu::Sampler,
    uniform: wgpu::Buffer,
    tables: Option<(
        crate::profiler::TrackedTexture,
        crate::profiler::TrackedTexture,
    )>,
    targets: Option<Targets>,
    frame: Option<FrameBindGroups>,
}

fn tex_entry(binding: u32, filterable: bool) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    }
}

fn sampler_entry(binding: u32, kind: wgpu::SamplerBindingType) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Sampler(kind),
        count: None,
    }
}

fn uniform_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

fn pipeline(
    device: &wgpu::Device,
    name: &str,
    body: &str,
    entry: &str,
    bgl: &wgpu::BindGroupLayout,
    format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(name),
        source: wgpu::ShaderSource::Wgsl(format!("{COMMON_WGSL}{body}").into()),
    });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some(name),
        bind_group_layouts: &[bgl],
        push_constant_ranges: &[],
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(name),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: "vs_fullscreen",
            buffers: &[],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: entry,
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(wgpu::BlendState::REPLACE),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    })
}

impl SmaaPass {
    /// The pipelines and layouts; nothing sized, nothing uploaded.
    pub(crate) fn new(device: &wgpu::Device, output_format: wgpu::TextureFormat) -> Self {
        use wgpu::SamplerBindingType::{Filtering, NonFiltering};
        let edges_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("smaa edges bgl"),
            entries: &[
                tex_entry(0, false),
                sampler_entry(1, NonFiltering),
                uniform_entry(2),
            ],
        });
        let weights_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("smaa weights bgl"),
            entries: &[
                tex_entry(0, true),
                tex_entry(1, true),
                tex_entry(2, true),
                sampler_entry(3, Filtering),
                uniform_entry(4),
            ],
        });
        let blend_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("smaa blend bgl"),
            entries: &[
                tex_entry(0, true),
                tex_entry(1, true),
                sampler_entry(2, Filtering),
                uniform_entry(3),
            ],
        });
        let sampler = |label: &str, filter: wgpu::FilterMode| {
            device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some(label),
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                mag_filter: filter,
                min_filter: filter,
                ..Default::default()
            })
        };
        Self {
            edges_pipeline: pipeline(
                device,
                "smaa edges",
                EDGES_WGSL,
                "fs_edges",
                &edges_bgl,
                EDGES_FORMAT,
            ),
            weights_pipeline: pipeline(
                device,
                "smaa weights",
                WEIGHTS_WGSL,
                "fs_weights",
                &weights_bgl,
                WEIGHTS_FORMAT,
            ),
            blend_pipeline: pipeline(
                device,
                "smaa blend",
                BLEND_WGSL,
                "fs_blend",
                &blend_bgl,
                output_format,
            ),
            edges_bgl,
            weights_bgl,
            blend_bgl,
            point_sampler: sampler("smaa point sampler", wgpu::FilterMode::Nearest),
            linear_sampler: sampler("smaa linear sampler", wgpu::FilterMode::Linear),
            uniform: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("smaa uniform"),
                size: std::mem::size_of::<SmaaGpu>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            tables: None,
            targets: None,
            frame: None,
        }
    }

    /// Readies this frame's passes over `color` (the composite's output), or
    /// with `None` marks them as not running. `tex2d_bgl` and `post_sampler`
    /// are the post chain's, so the output binds where FXAA's does.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        smaa: Option<bsengine_core::Smaa>,
        color: &wgpu::TextureView,
        size: (u32, u32),
        output_format: wgpu::TextureFormat,
        tex2d_bgl: &wgpu::BindGroupLayout,
        post_sampler: &wgpu::Sampler,
    ) {
        let Some(smaa) = smaa.filter(|s| s.enabled) else {
            self.frame = None;
            return;
        };
        let (threshold, steps, steps_diag, corners) = smaa.quality.parameters();
        let data = SmaaGpu {
            rt: [
                1.0 / size.0 as f32,
                1.0 / size.1 as f32,
                size.0 as f32,
                size.1 as f32,
            ],
            threshold,
            max_search_steps: steps as f32,
            max_search_steps_diag: steps_diag as f32,
            corners: corners as u32,
            srgb_input: output_format.is_srgb() as u32,
            _pad: [0; 3],
        };
        queue.write_buffer(&self.uniform, 0, bytemuck::bytes_of(&data));

        let tables = self.tables.get_or_insert_with(|| {
            let table = |label: &str, (w, h): (u32, u32), format, data: &[u8]| {
                crate::profiler::create_tracked_texture_with_data(
                    device,
                    queue,
                    &wgpu::TextureDescriptor {
                        label: Some(label),
                        size: wgpu::Extent3d {
                            width: w,
                            height: h,
                            depth_or_array_layers: 1,
                        },
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format,
                        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                        view_formats: &[],
                    },
                    wgpu::util::TextureDataOrder::LayerMajor,
                    data,
                )
            };
            (
                table(
                    "smaa area tex",
                    AREA_TEX_SIZE,
                    wgpu::TextureFormat::Rg8Unorm,
                    AREA_TEX,
                ),
                table(
                    "smaa search tex",
                    SEARCH_TEX_SIZE,
                    wgpu::TextureFormat::R8Unorm,
                    SEARCH_TEX,
                ),
            )
        });

        if self.targets.as_ref().map(|t| t.size) != Some(size) {
            let target = |label: &str, format| {
                crate::profiler::create_tracked_texture(
                    device,
                    &wgpu::TextureDescriptor {
                        label: Some(label),
                        size: wgpu::Extent3d {
                            width: size.0,
                            height: size.1,
                            depth_or_array_layers: 1,
                        },
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format,
                        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                            | wgpu::TextureUsages::TEXTURE_BINDING,
                        view_formats: &[],
                    },
                )
            };
            let edges = target("smaa edges", EDGES_FORMAT);
            let edges_view = edges.create_view(&Default::default());
            let weights = target("smaa weights", WEIGHTS_FORMAT);
            let weights_view = weights.create_view(&Default::default());
            let out = target("smaa out", output_format);
            let out_view = out.create_view(&Default::default());
            let area_view = tables.0.create_view(&Default::default());
            let search_view = tables.1.create_view(&Default::default());
            let weights_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("smaa weights bg"),
                layout: &self.weights_bgl,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&edges_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&area_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(&search_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::Sampler(&self.linear_sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: self.uniform.as_entire_binding(),
                    },
                ],
            });
            let out_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("smaa out bg"),
                layout: tex2d_bgl,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&out_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(post_sampler),
                    },
                ],
            });
            self.targets = Some(Targets {
                size,
                _edges: edges,
                edges_view,
                _weights: weights,
                weights_view,
                _out: out,
                out_view,
                out_bg,
                weights_bg,
            });
        }
        let targets = self.targets.as_ref().expect("made above");

        let edges_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("smaa edges bg"),
            layout: &self.edges_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(color),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.point_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.uniform.as_entire_binding(),
                },
            ],
        });
        let blend_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("smaa blend bg"),
            layout: &self.blend_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(color),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&targets.weights_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.linear_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: self.uniform.as_entire_binding(),
                },
            ],
        });
        self.frame = Some(FrameBindGroups { edges_bg, blend_bg });
    }

    /// Whether [`Self::prepare`] readied the passes this frame.
    pub(crate) fn active(&self) -> bool {
        self.frame.is_some()
    }

    /// Records the three passes, and returns the output's bind group (in the
    /// post chain's `tex2d` layout) for the next pass to read -- or `None`
    /// when SMAA is not running this frame.
    pub(crate) fn encode(&self, encoder: &mut wgpu::CommandEncoder) -> Option<&wgpu::BindGroup> {
        let frame = self.frame.as_ref()?;
        let targets = self.targets.as_ref()?;
        let passes: [(
            &str,
            &wgpu::TextureView,
            &wgpu::RenderPipeline,
            &wgpu::BindGroup,
        ); 3] = [
            // Cleared, because the edge pass discards every pixel that has no
            // edge: what it leaves behind has to read as "no edge".
            (
                "smaa edges pass",
                &targets.edges_view,
                &self.edges_pipeline,
                &frame.edges_bg,
            ),
            (
                "smaa weights pass",
                &targets.weights_view,
                &self.weights_pipeline,
                &targets.weights_bg,
            ),
            (
                "smaa blend pass",
                &targets.out_view,
                &self.blend_pipeline,
                &frame.blend_bg,
            ),
        ];
        for (label, view, pipeline, bg) in passes {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some(label),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                ..Default::default()
            });
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, bg, &[]);
            pass.draw(0..3, 0..1);
        }
        Some(&targets.out_bg)
    }
}
