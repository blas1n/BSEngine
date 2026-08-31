use crate::mesh::GpuMeshRegistry;
use bsengine_ecs::Resource;
use glam::{Mat4, Vec3};
use std::sync::Arc;

const MAX_POINT_LIGHTS: usize = 8;
const MAX_SPOT_LIGHTS: usize = 8;

/// How many named render passes `render_frame` can time in a single frame.
/// The frame today has at most 6 (directional shadow, the point-light shadow
/// loop collapsed to one aggregate timing, main, sky, transparent, egui) --
/// 16 leaves comfortable headroom without the query set growing unbounded.
const MAX_TIMED_PASSES: u32 = 16;
/// Two timestamp queries (begin + end) per timed pass.
const TIMESTAMP_QUERY_COUNT: u32 = MAX_TIMED_PASSES * 2;

/// `pub(crate)` only so that
/// `post_process::tests::the_injection_pass_ports_point_shadow_factor_verbatim`
/// can compare this text against the froxel injection shader's copy of
/// `point_shadow_factor`. Nothing outside a test reads it.
pub(crate) const MESH_WGSL: &str = r#"
const MAX_POINT_LIGHTS: u32 = 8u;
const MAX_SPOT_LIGHTS: u32 = 8u;
const PI: f32 = 3.14159265358979323846;
struct CameraUniform {
    view_proj: mat4x4<f32>,
    light_view_proj: mat4x4<f32>,
    cam_pos: vec3<f32>,
    time: f32,
};
struct ModelUniform {
    model: mat4x4<f32>,
    metallic: f32,
    roughness: f32,
    _pad0: f32,
    _pad1: f32,
    emissive: vec3<f32>,
    _pad2: f32,
    base_color: vec3<f32>,
    opacity: f32,
};
struct PointLightEntry {
    position: vec3<f32>,
    _pad0: f32,
    color: vec3<f32>,
    intensity: f32,
    range: f32,
    _pad1: f32,
    _pad2: f32,
    _pad3: f32,
};
struct SpotLightEntry {
    position: vec3<f32>,
    _pad0: f32,
    direction: vec3<f32>,
    inner_cos: f32,
    color: vec3<f32>,
    outer_cos: f32,
    intensity: f32,
    range: f32,
    _pad1: f32,
    _pad2: f32,
};
struct LightUniform {
    direction: vec3<f32>,
    _pad0: f32,
    color: vec3<f32>,
    _pad1: f32,
    ambient: vec3<f32>,
    num_point_lights: u32,
    point_lights: array<PointLightEntry, 8>,
    num_spot_lights: u32,
    // Two of the three spare floats that used to sit here. Both shaders read
    // the *same* uniform buffer, so both must declare this struct identically
    // -- even the terrain shader, which does not sample IBL.
    ibl_enabled: u32,
    ibl_max_mip: f32,
    _pad4: f32,
    spot_lights: array<SpotLightEntry, 8>,
};
@group(0) @binding(0) var<uniform> camera: CameraUniform;
@group(1) @binding(0) var<uniform> model_data: ModelUniform;
@group(2) @binding(0) var<uniform> light: LightUniform;
@group(3) @binding(0) var t_diffuse: texture_2d<f32>;
@group(3) @binding(1) var s_diffuse: sampler;
@group(2) @binding(1) var shadow_sampler: sampler_comparison;
@group(2) @binding(2) var shadow_map: texture_depth_2d;
@group(2) @binding(4) var point_shadow_map: texture_2d_array<f32>;
// Image-based lighting. These are always bound -- with 1x1 dummies when no
// skybox is loaded -- because a bind group layout cannot vary per frame;
// `light.ibl_enabled` is what actually decides whether they are read.
@group(2) @binding(5) var irradiance_cube: texture_cube<f32>;
@group(2) @binding(6) var prefilter_cube: texture_cube<f32>;
@group(2) @binding(7) var brdf_lut: texture_2d<f32>;
// Binding 8, not 3: 3 is already taken in the bind group layout by the point
// shadow sampler, which no shader declares (the cube lookup uses textureLoad)
// but which is bound all the same, as a non-filtering sampler.
@group(2) @binding(8) var ibl_sampler: sampler;
// Baked light probes. A uniform buffer with a fixed maximum, not a storage
// buffer: this engine uses no storage buffers anywhere, and fixed-size uniform
// arrays are its established pattern (see MAX_POINT_LIGHTS above). 32 probes x
// 9 coefficients x vec4 is 4608 bytes, well inside the 64 KiB uniform limit.
//
// Bound in every frame, exactly like the IBL maps: a bind group layout is fixed
// at creation, so a scene with no volume uploads `enabled: 0` and zeroed
// coefficients rather than skipping the binding.
struct ProbeUniform {
    // MAX_PROBES(32) * 9 coefficients. vec4 rather than vec3 because std140
    // pads a vec3 array element out to 16 bytes anyway.
    coeffs: array<vec4<f32>, 288>,
    // World-space minimum corner of the volume.
    origin: vec3<f32>,
    enabled: u32,
    // World-space size of the volume (full extent, not half).
    extent: vec3<f32>,
    _pad0: f32,
    // Probes per axis, sitting on the grid's lattice points.
    resolution: vec3<u32>,
    _pad1: u32,
};
@group(2) @binding(9) var<uniform> probes: ProbeUniform;

struct VertIn {
    @location(0) pos: vec3<f32>,
    @location(1) col: vec3<f32>,
    @location(2) normal: vec3<f32>,
    @location(3) uv: vec2<f32>,
}
struct VertOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) col: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) world_pos: vec3<f32>,
    @location(4) light_space_pos: vec4<f32>,
}
@vertex
fn vs_main(in: VertIn) -> VertOut {
    var out: VertOut;
    let world_pos4 = model_data.model * vec4<f32>(in.pos, 1.0);
    out.clip_pos = camera.view_proj * world_pos4;
    out.world_pos = world_pos4.xyz;
    out.col = in.col;
    let normal_matrix = mat3x3<f32>(
        model_data.model[0].xyz,
        model_data.model[1].xyz,
        model_data.model[2].xyz,
    );
    out.world_normal = normalize(normal_matrix * in.normal);
    out.uv = in.uv;
    out.light_space_pos = camera.light_view_proj * world_pos4;
    return out;
}
fn shadow_factor(lsp: vec4<f32>) -> f32 {
    let proj = lsp.xyz / lsp.w;
    let uv = proj.xy * vec2<f32>(0.5, -0.5) + vec2<f32>(0.5, 0.5);
    let depth = proj.z;
    if (uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0 || depth < 0.0 || depth > 1.0) {
        return 1.0;
    }
    return textureSampleCompare(shadow_map, shadow_sampler, uv, depth - 0.003);
}
// Linear-distance cube shadow lookup: `to_frag` is the direction from the
// light to the fragment (world-space, unnormalized). Selects the cube face
// whose axis has the largest magnitude, derives that face's UV analytically
// (matching point_light_face_view_projs's Rust-side look_at_rh/perspective
// construction exactly -- verified by hand against glam's look_at_rh
// convention), then compares against the stored linear distance for that
// face/light layer instead of using a depth-compare sampler (R32Float isn't
// natively filterable without an unrequested device feature, so this reads
// a raw texel via textureLoad rather than textureSample).
fn point_shadow_factor(light_index: u32, to_frag: vec3<f32>) -> f32 {
    let ax = abs(to_frag.x);
    let ay = abs(to_frag.y);
    let az = abs(to_frag.z);
    var face: u32;
    var u: f32;
    var v: f32;
    var ma: f32;
    if (ax >= ay && ax >= az) {
        ma = ax;
        if (to_frag.x > 0.0) {
            face = 0u;
            u = -to_frag.z;
            v = -to_frag.y;
        } else {
            face = 1u;
            u = to_frag.z;
            v = -to_frag.y;
        }
    } else if (ay >= ax && ay >= az) {
        ma = ay;
        if (to_frag.y > 0.0) {
            face = 2u;
            u = to_frag.x;
            v = to_frag.z;
        } else {
            face = 3u;
            u = to_frag.x;
            v = -to_frag.z;
        }
    } else {
        ma = az;
        if (to_frag.z > 0.0) {
            face = 4u;
            u = to_frag.x;
            v = -to_frag.y;
        } else {
            face = 5u;
            u = -to_frag.x;
            v = -to_frag.y;
        }
    }
    let ndc = vec2<f32>(u, v) / max(ma, 0.0001);
    let uv = ndc * vec2<f32>(0.5, -0.5) + vec2<f32>(0.5, 0.5);
    // Must match POINT_SHADOW_MAP_SIZE in bsengine-rhi-wgpu/src/surface.rs.
    let size = 512.0;
    let px = vec2<i32>(clamp(uv, vec2<f32>(0.0), vec2<f32>(0.999999)) * size);
    let layer = i32(light_index * 6u + face);
    let stored = textureLoad(point_shadow_map, px, layer, 0).r;
    let dist = length(to_frag);
    if (dist - 0.1 > stored) {
        return 0.0;
    }
    return 1.0;
}
fn distribution_ggx(n_dot_h: f32, roughness: f32) -> f32 {
    let a = roughness * roughness;
    let a2 = a * a;
    let denom = n_dot_h * n_dot_h * (a2 - 1.0) + 1.0;
    return a2 / (PI * denom * denom);
}
fn geometry_schlick_ggx(ndotx: f32, roughness: f32) -> f32 {
    let r = roughness + 1.0;
    let k = r * r / 8.0;
    return ndotx / (ndotx * (1.0 - k) + k);
}
fn geometry_smith(n_dot_v: f32, n_dot_l: f32, roughness: f32) -> f32 {
    return geometry_schlick_ggx(n_dot_v, roughness) * geometry_schlick_ggx(n_dot_l, roughness);
}
fn fresnel_schlick(cos_theta: f32, f0: vec3<f32>) -> vec3<f32> {
    return f0 + (vec3<f32>(1.0, 1.0, 1.0) - f0) * pow(clamp(1.0 - cos_theta, 0.0, 1.0), 5.0);
}
// Schlick with a roughness term. The plain Fresnel is derived for a perfect
// mirror and over-brightens rough metals at grazing angles.
fn fresnel_schlick_roughness(cos_theta: f32, f0: vec3<f32>, roughness: f32) -> vec3<f32> {
    let inv_rough = vec3<f32>(1.0 - roughness);
    return f0 + (max(inv_rough, f0) - f0) * pow(clamp(1.0 - cos_theta, 0.0, 1.0), 5.0);
}
// True when `world_pos` is inside the baked volume's box. Outside it there are
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
// Index of probe (ix, iy, iz)'s first coefficient. x fastest, then y, then z --
// the same order the CPU walks the lattice in when it uploads them.
fn probe_coeff_base(ix: u32, iy: u32, iz: u32) -> u32 {
    let idx = (iz * probes.resolution.y + iy) * probes.resolution.x + ix;
    // MAX_PROBES - 1. A resolution the CPU side never produces would otherwise
    // read past the end of the array.
    return min(idx, 31u) * 9u;
}
// The probe grid's irradiance at `world_pos` for a surface facing `n`, in the
// same units the irradiance cube stores: E / PI, with the Lambert BRDF's 1/PI
// folded in, so callers write `* albedo` with no stray constant exactly as the
// IBL path does.
//
// The eight surrounding probes' COEFFICIENTS are blended and the result
// evaluated once, rather than evaluating eight probes and blending the results.
// SH evaluation is linear in the coefficients, so the two are equivalent -- and
// this one does an eighth of the evaluation work.
fn eval_probe_sh(world_pos: vec3<f32>, n: vec3<f32>) -> vec3<f32> {
    let g = probe_grid_coord(world_pos);
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
        let w = select(1.0 - frac.x, frac.x, hi_x)
            * select(1.0 - frac.y, frac.y, hi_y)
            * select(1.0 - frac.z, frac.z, hi_z);
        let base = probe_coeff_base(
            u32(select(i0.x, i1.x, hi_x)),
            u32(select(i0.y, i1.y, hi_y)),
            u32(select(i0.z, i1.z, hi_z)),
        );
        for (var c: u32 = 0u; c < 9u; c++) {
            blended[c] = blended[c] + probes.coeffs[base + c] * w;
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
@fragment
fn fs_main(in: VertOut) -> @location(0) vec4<f32> {
    let n = normalize(in.world_normal);
    let v = normalize(camera.cam_pos - in.world_pos);
    let albedo = textureSample(t_diffuse, s_diffuse, in.uv).rgb * in.col * model_data.base_color;
    let metallic = model_data.metallic;
    let roughness = max(model_data.roughness, 0.04);
    let f0 = mix(vec3<f32>(0.04, 0.04, 0.04), albedo, metallic);
    let n_dot_v = max(dot(n, v), 0.0001);
    let lit = shadow_factor(in.light_space_pos);

    var lo = vec3<f32>(0.0, 0.0, 0.0);
    {
        let l = normalize(-light.direction);
        let h = normalize(v + l);
        let n_dot_l = max(dot(n, l), 0.0);
        let n_dot_h = max(dot(n, h), 0.0);
        let h_dot_v = max(dot(h, v), 0.0);
        let ndf = distribution_ggx(n_dot_h, roughness);
        let g = geometry_smith(n_dot_v, n_dot_l, roughness);
        let f = fresnel_schlick(h_dot_v, f0);
        let kd = (vec3<f32>(1.0, 1.0, 1.0) - f) * (1.0 - metallic);
        let specular = (ndf * g * f) / (4.0 * n_dot_v * n_dot_l + 0.0001);
        lo += (kd * albedo / PI + specular) * light.color * n_dot_l * lit;
    }
    for (var i: u32 = 0u; i < light.num_point_lights; i++) {
        let pl = light.point_lights[i];
        let to_light = pl.position - in.world_pos;
        let dist = length(to_light);
        if dist < pl.range {
            let l = normalize(to_light);
            let h = normalize(v + l);
            let n_dot_l = max(dot(n, l), 0.0);
            let n_dot_h = max(dot(n, h), 0.0);
            let h_dot_v = max(dot(h, v), 0.0);
            let t = 1.0 - dist / pl.range;
            let ndf = distribution_ggx(n_dot_h, roughness);
            let g = geometry_smith(n_dot_v, n_dot_l, roughness);
            let f = fresnel_schlick(h_dot_v, f0);
            let kd = (vec3<f32>(1.0, 1.0, 1.0) - f) * (1.0 - metallic);
            let specular = (ndf * g * f) / (4.0 * n_dot_v * n_dot_l + 0.0001);
            let pt_lit = point_shadow_factor(i, -to_light);
            lo += (kd * albedo / PI + specular) * pl.color * (pl.intensity * t * t) * n_dot_l * pt_lit;
        }
    }
    for (var j: u32 = 0u; j < light.num_spot_lights; j++) {
        let sl = light.spot_lights[j];
        let to_light = sl.position - in.world_pos;
        let dist = length(to_light);
        if dist < sl.range {
            let light_dir = normalize(to_light);
            let cos_angle = dot(-light_dir, sl.direction);
            let spot_factor = smoothstep(sl.outer_cos, sl.inner_cos, cos_angle);
            if spot_factor > 0.0 {
                let l = light_dir;
                let h = normalize(v + l);
                let n_dot_l = max(dot(n, l), 0.0);
                let n_dot_h = max(dot(n, h), 0.0);
                let h_dot_v = max(dot(h, v), 0.0);
                let t = 1.0 - dist / sl.range;
                let ndf = distribution_ggx(n_dot_h, roughness);
                let g = geometry_smith(n_dot_v, n_dot_l, roughness);
                let f = fresnel_schlick(h_dot_v, f0);
                let kd = (vec3<f32>(1.0, 1.0, 1.0) - f) * (1.0 - metallic);
                let specular = (ndf * g * f) / (4.0 * n_dot_v * n_dot_l + 0.0001);
                lo += (kd * albedo / PI + specular) * sl.color * (sl.intensity * t * t * spot_factor) * n_dot_l;
            }
        }
    }
    // With no skybox this is exactly the old flat-ambient term, evaluated by
    // exactly the old expression -- the branch is on a uniform, so every
    // no-skybox frame stays bit-for-bit what it was before IBL existed.
    //
    // No ambient-occlusion factor multiplies the IBL term: SSAO here is a
    // post-process pass over the depth buffer, so no AO value exists at this
    // point in the frame, and SSAO keeps attenuating the composited result
    // exactly as it does today.
    //
    // The diffuse/specular energy split is hoisted out of the IBL branch
    // because the probe path below needs `kd_ibl` too, and a probe volume in a
    // scene with no skybox still has to light something. It depends only on the
    // material and the view angle, so computing it unconditionally changes no
    // pixel: `ambient_term` is still exactly `light.ambient * albedo` whenever
    // both branches are skipped.
    let f_ibl = fresnel_schlick_roughness(n_dot_v, f0, roughness);
    let kd_ibl = (vec3<f32>(1.0) - f_ibl) * (1.0 - metallic);
    var specular_ibl = vec3<f32>(0.0, 0.0, 0.0);
    var ambient_term = light.ambient * albedo;
    if (light.ibl_enabled != 0u) {
        let irradiance = textureSample(irradiance_cube, ibl_sampler, n).rgb;
        let diffuse_ibl = irradiance * albedo * kd_ibl;

        let r = reflect(-v, n);
        let prefiltered = textureSampleLevel(
            prefilter_cube, ibl_sampler, r, roughness * light.ibl_max_mip
        ).rgb;
        let brdf = textureSample(brdf_lut, ibl_sampler, vec2<f32>(n_dot_v, roughness)).rg;
        specular_ibl = prefiltered * (f_ibl * brdf.x + brdf.y);

        ambient_term = diffuse_ibl + specular_ibl;
    }
    if (probes.enabled != 0u && inside_probe_volume(in.world_pos)) {
        // REPLACES the ambient/IBL irradiance, never adds to it. The probes
        // captured the real scene *including the skybox background and the flat
        // ambient term*, so their SH already carries the sky's contribution;
        // adding would double-count sky light and wash the scene out. The
        // specular half is kept as-is -- probes are a diffuse-only, L2
        // representation and carry no reflection to replace it with.
        let probe_irradiance = eval_probe_sh(in.world_pos, n);
        ambient_term = probe_irradiance * albedo * kd_ibl + specular_ibl;
    }
    let color = ambient_term + lo + model_data.emissive;
    return vec4<f32>(color, model_data.opacity);
}
"#;

const TERRAIN_WGSL: &str = r#"
const MAX_POINT_LIGHTS: u32 = 8u;
const MAX_SPOT_LIGHTS: u32 = 8u;
const PI: f32 = 3.14159265358979323846;
const TILE_SIZE: f32 = 2.0;
struct CameraUniform {
    view_proj: mat4x4<f32>,
    light_view_proj: mat4x4<f32>,
    cam_pos: vec3<f32>,
    time: f32,
};
struct ModelUniform {
    model: mat4x4<f32>,
    metallic: f32,
    roughness: f32,
    _pad0: f32,
    _pad1: f32,
    emissive: vec3<f32>,
    _pad2: f32,
    base_color: vec3<f32>,
    opacity: f32,
};
struct PointLightEntry {
    position: vec3<f32>,
    _pad0: f32,
    color: vec3<f32>,
    intensity: f32,
    range: f32,
    _pad1: f32,
    _pad2: f32,
    _pad3: f32,
};
struct SpotLightEntry {
    position: vec3<f32>,
    _pad0: f32,
    direction: vec3<f32>,
    inner_cos: f32,
    color: vec3<f32>,
    outer_cos: f32,
    intensity: f32,
    range: f32,
    _pad1: f32,
    _pad2: f32,
};
struct LightUniform {
    direction: vec3<f32>,
    _pad0: f32,
    color: vec3<f32>,
    _pad1: f32,
    ambient: vec3<f32>,
    num_point_lights: u32,
    point_lights: array<PointLightEntry, 8>,
    num_spot_lights: u32,
    // Two of the three spare floats that used to sit here. Both shaders read
    // the *same* uniform buffer, so both must declare this struct identically
    // -- even the terrain shader, which does not sample IBL.
    ibl_enabled: u32,
    ibl_max_mip: f32,
    _pad4: f32,
    spot_lights: array<SpotLightEntry, 8>,
};
@group(0) @binding(0) var<uniform> camera: CameraUniform;
@group(1) @binding(0) var<uniform> model_data: ModelUniform;
@group(2) @binding(0) var<uniform> light: LightUniform;
@group(3) @binding(0) var t_layer0: texture_2d<f32>;
@group(3) @binding(1) var t_layer1: texture_2d<f32>;
@group(3) @binding(2) var t_layer2: texture_2d<f32>;
@group(3) @binding(3) var t_layer3: texture_2d<f32>;
@group(3) @binding(4) var t_weight: texture_2d<f32>;
@group(3) @binding(5) var s_terrain: sampler;
@group(2) @binding(1) var shadow_sampler: sampler_comparison;
@group(2) @binding(2) var shadow_map: texture_depth_2d;
@group(2) @binding(4) var point_shadow_map: texture_2d_array<f32>;

struct VertIn {
    @location(0) pos: vec3<f32>,
    @location(1) col: vec3<f32>,
    @location(2) normal: vec3<f32>,
    @location(3) uv: vec2<f32>,
}
struct VertOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) col: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) world_pos: vec3<f32>,
    @location(4) light_space_pos: vec4<f32>,
}
@vertex
fn vs_main(in: VertIn) -> VertOut {
    var out: VertOut;
    let world_pos4 = model_data.model * vec4<f32>(in.pos, 1.0);
    out.clip_pos = camera.view_proj * world_pos4;
    out.world_pos = world_pos4.xyz;
    out.col = in.col;
    let normal_matrix = mat3x3<f32>(
        model_data.model[0].xyz,
        model_data.model[1].xyz,
        model_data.model[2].xyz,
    );
    out.world_normal = normalize(normal_matrix * in.normal);
    out.uv = in.uv;
    out.light_space_pos = camera.light_view_proj * world_pos4;
    return out;
}
fn shadow_factor(lsp: vec4<f32>) -> f32 {
    let proj = lsp.xyz / lsp.w;
    let uv = proj.xy * vec2<f32>(0.5, -0.5) + vec2<f32>(0.5, 0.5);
    let depth = proj.z;
    if (uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0 || depth < 0.0 || depth > 1.0) {
        return 1.0;
    }
    return textureSampleCompare(shadow_map, shadow_sampler, uv, depth - 0.003);
}
fn point_shadow_factor(light_index: u32, to_frag: vec3<f32>) -> f32 {
    let ax = abs(to_frag.x);
    let ay = abs(to_frag.y);
    let az = abs(to_frag.z);
    var face: u32;
    var u: f32;
    var v: f32;
    var ma: f32;
    if (ax >= ay && ax >= az) {
        ma = ax;
        if (to_frag.x > 0.0) {
            face = 0u;
            u = -to_frag.z;
            v = -to_frag.y;
        } else {
            face = 1u;
            u = to_frag.z;
            v = -to_frag.y;
        }
    } else if (ay >= ax && ay >= az) {
        ma = ay;
        if (to_frag.y > 0.0) {
            face = 2u;
            u = to_frag.x;
            v = to_frag.z;
        } else {
            face = 3u;
            u = to_frag.x;
            v = -to_frag.z;
        }
    } else {
        ma = az;
        if (to_frag.z > 0.0) {
            face = 4u;
            u = to_frag.x;
            v = -to_frag.y;
        } else {
            face = 5u;
            u = -to_frag.x;
            v = -to_frag.y;
        }
    }
    let ndc = vec2<f32>(u, v) / max(ma, 0.0001);
    let uv = ndc * vec2<f32>(0.5, -0.5) + vec2<f32>(0.5, 0.5);
    let size = 512.0;
    let px = vec2<i32>(clamp(uv, vec2<f32>(0.0), vec2<f32>(0.999999)) * size);
    let layer = i32(light_index * 6u + face);
    let stored = textureLoad(point_shadow_map, px, layer, 0).r;
    let dist = length(to_frag);
    if (dist - 0.1 > stored) {
        return 0.0;
    }
    return 1.0;
}
fn distribution_ggx(n_dot_h: f32, roughness: f32) -> f32 {
    let a = roughness * roughness;
    let a2 = a * a;
    let denom = n_dot_h * n_dot_h * (a2 - 1.0) + 1.0;
    return a2 / (PI * denom * denom);
}
fn geometry_schlick_ggx(ndotx: f32, roughness: f32) -> f32 {
    let r = roughness + 1.0;
    let k = r * r / 8.0;
    return ndotx / (ndotx * (1.0 - k) + k);
}
fn geometry_smith(n_dot_v: f32, n_dot_l: f32, roughness: f32) -> f32 {
    return geometry_schlick_ggx(n_dot_v, roughness) * geometry_schlick_ggx(n_dot_l, roughness);
}
fn fresnel_schlick(cos_theta: f32, f0: vec3<f32>) -> vec3<f32> {
    return f0 + (vec3<f32>(1.0, 1.0, 1.0) - f0) * pow(clamp(1.0 - cos_theta, 0.0, 1.0), 5.0);
}
@fragment
fn fs_main(in: VertOut) -> @location(0) vec4<f32> {
    let n = normalize(in.world_normal);
    let v = normalize(camera.cam_pos - in.world_pos);

    var w = textureSample(t_weight, s_terrain, in.uv);
    let w_sum = max(w.r + w.g + w.b + w.a, 0.0001);
    w = w / w_sum;

    let tiled_uv = in.world_pos.xz / TILE_SIZE;
    let c0 = textureSample(t_layer0, s_terrain, tiled_uv).rgb;
    let c1 = textureSample(t_layer1, s_terrain, tiled_uv).rgb;
    let c2 = textureSample(t_layer2, s_terrain, tiled_uv).rgb;
    let c3 = textureSample(t_layer3, s_terrain, tiled_uv).rgb;
    let albedo = (c0 * w.r + c1 * w.g + c2 * w.b + c3 * w.a) * in.col * model_data.base_color;

    let metallic = model_data.metallic;
    let roughness = max(model_data.roughness, 0.04);
    let f0 = mix(vec3<f32>(0.04, 0.04, 0.04), albedo, metallic);
    let n_dot_v = max(dot(n, v), 0.0001);
    let lit = shadow_factor(in.light_space_pos);

    var lo = vec3<f32>(0.0, 0.0, 0.0);
    {
        let l = normalize(-light.direction);
        let h = normalize(v + l);
        let n_dot_l = max(dot(n, l), 0.0);
        let n_dot_h = max(dot(n, h), 0.0);
        let h_dot_v = max(dot(h, v), 0.0);
        let ndf = distribution_ggx(n_dot_h, roughness);
        let g = geometry_smith(n_dot_v, n_dot_l, roughness);
        let f = fresnel_schlick(h_dot_v, f0);
        let kd = (vec3<f32>(1.0, 1.0, 1.0) - f) * (1.0 - metallic);
        let specular = (ndf * g * f) / (4.0 * n_dot_v * n_dot_l + 0.0001);
        lo += (kd * albedo / PI + specular) * light.color * n_dot_l * lit;
    }
    for (var i: u32 = 0u; i < light.num_point_lights; i++) {
        let pl = light.point_lights[i];
        let to_light = pl.position - in.world_pos;
        let dist = length(to_light);
        if dist < pl.range {
            let l = normalize(to_light);
            let h = normalize(v + l);
            let n_dot_l = max(dot(n, l), 0.0);
            let n_dot_h = max(dot(n, h), 0.0);
            let h_dot_v = max(dot(h, v), 0.0);
            let t = 1.0 - dist / pl.range;
            let ndf = distribution_ggx(n_dot_h, roughness);
            let g = geometry_smith(n_dot_v, n_dot_l, roughness);
            let f = fresnel_schlick(h_dot_v, f0);
            let kd = (vec3<f32>(1.0, 1.0, 1.0) - f) * (1.0 - metallic);
            let specular = (ndf * g * f) / (4.0 * n_dot_v * n_dot_l + 0.0001);
            let pt_lit = point_shadow_factor(i, -to_light);
            lo += (kd * albedo / PI + specular) * pl.color * (pl.intensity * t * t) * n_dot_l * pt_lit;
        }
    }
    for (var j: u32 = 0u; j < light.num_spot_lights; j++) {
        let sl = light.spot_lights[j];
        let to_light = sl.position - in.world_pos;
        let dist = length(to_light);
        if dist < sl.range {
            let light_dir = normalize(to_light);
            let cos_angle = dot(-light_dir, sl.direction);
            let spot_factor = smoothstep(sl.outer_cos, sl.inner_cos, cos_angle);
            if spot_factor > 0.0 {
                let l = light_dir;
                let h = normalize(v + l);
                let n_dot_l = max(dot(n, l), 0.0);
                let n_dot_h = max(dot(n, h), 0.0);
                let h_dot_v = max(dot(h, v), 0.0);
                let t = 1.0 - dist / sl.range;
                let ndf = distribution_ggx(n_dot_h, roughness);
                let g = geometry_smith(n_dot_v, n_dot_l, roughness);
                let f = fresnel_schlick(h_dot_v, f0);
                let kd = (vec3<f32>(1.0, 1.0, 1.0) - f) * (1.0 - metallic);
                let specular = (ndf * g * f) / (4.0 * n_dot_v * n_dot_l + 0.0001);
                lo += (kd * albedo / PI + specular) * sl.color * (sl.intensity * t * t * spot_factor) * n_dot_l;
            }
        }
    }
    let color = light.ambient * albedo + lo + model_data.emissive;
    return vec4<f32>(color, model_data.opacity);
}
"#;

const SHADOW_WGSL: &str = r#"
struct CameraUniform {
    view_proj: mat4x4<f32>,
    light_view_proj: mat4x4<f32>,
};
@group(0) @binding(0) var<uniform> camera: CameraUniform;

struct ModelUniform {
    model: mat4x4<f32>,
};
@group(1) @binding(0) var<uniform> model_data: ModelUniform;

struct VertIn {
    @location(0) pos: vec3<f32>,
    @location(1) col: vec3<f32>,
    @location(2) normal: vec3<f32>,
    @location(3) uv: vec2<f32>,
}

@vertex
fn vs_shadow(in: VertIn) -> @builtin(position) vec4<f32> {
    let world = model_data.model * vec4<f32>(in.pos, 1.0);
    return camera.light_view_proj * world;
}
"#;

const POINT_SHADOW_WGSL: &str = r#"
struct ShadowUniform {
    view_proj: mat4x4<f32>,
    light_pos: vec3<f32>,
};
@group(0) @binding(0) var<uniform> shadow_uniform: ShadowUniform;

struct ModelUniform {
    model: mat4x4<f32>,
};
@group(1) @binding(0) var<uniform> model_data: ModelUniform;

struct VertIn {
    @location(0) pos: vec3<f32>,
    @location(1) col: vec3<f32>,
    @location(2) normal: vec3<f32>,
    @location(3) uv: vec2<f32>,
}
struct VertOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) world_pos: vec3<f32>,
}

@vertex
fn vs_point_shadow(in: VertIn) -> VertOut {
    var out: VertOut;
    let world = model_data.model * vec4<f32>(in.pos, 1.0);
    out.clip_pos = shadow_uniform.view_proj * world;
    out.world_pos = world.xyz;
    return out;
}

@fragment
fn fs_point_shadow(in: VertOut) -> @location(0) vec4<f32> {
    let dist = length(in.world_pos - shadow_uniform.light_pos);
    return vec4<f32>(dist, 0.0, 0.0, 1.0);
}
"#;

const SKYBOX_WGSL: &str = r#"
const PI: f32 = 3.14159265358979323846;
struct SkyUniform {
    inv_vp: mat4x4<f32>,
};
@group(0) @binding(0) var<uniform> sky: SkyUniform;
@group(1) @binding(0) var t_sky: texture_2d<f32>;
@group(1) @binding(1) var s_sky: sampler;
struct SkyOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) ndc: vec2<f32>,
};
@vertex
fn vs_sky(@builtin(vertex_index) vi: u32) -> SkyOut {
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>( 3.0, -1.0),
        vec2<f32>(-1.0,  3.0),
    );
    let p = positions[vi];
    var out: SkyOut;
    // z = w so NDC depth = 1.0; LessEqual test passes only where no geometry wrote depth
    out.clip_pos = vec4<f32>(p.x, p.y, 1.0, 1.0);
    out.ndc = p;
    return out;
}
@fragment
fn fs_sky(in: SkyOut) -> @location(0) vec4<f32> {
    let world = sky.inv_vp * vec4<f32>(in.ndc.x, in.ndc.y, 1.0, 1.0);
    let dir = normalize(world.xyz);
    let phi = atan2(dir.z, dir.x);
    let theta = asin(clamp(dir.y, -1.0, 1.0));
    let u = phi / (2.0 * PI) + 0.5;
    let v = 0.5 - theta / PI;
    return textureSample(t_sky, s_sky, vec2<f32>(u, v));
}
"#;

/// The scene shader used to capture light probes.
///
/// A near-copy of [`MESH_WGSL`] with exactly two deliberate differences:
///
///  1. The camera comes from a per-face dynamic-offset uniform instead of
///     `camera`, because one bake renders `probes * 6` views inside a single
///     command encoder and `queue.write_buffer` cannot vary between passes of
///     one submission -- the same reason the point-shadow pass has its own
///     per-face uniform.
///  2. **It shades direct light only**: the directional light, the point
///     lights, the spot lights, the flat ambient term and emissive. It never
///     samples the IBL cubes and never samples probes.
///
/// Point 2 is not an optimisation, it is what makes the bake terminate.
/// Probe irradiance is *derived from* what this shader outputs; if this
/// shader also read probe or IBL irradiance, the bake would be feeding its
/// own output back in -- probes lighting probes, and (with IBL) sky light
/// counted twice, since the skybox is already captured as this pass's
/// background. The result is deliberately **one bounce**.
///
/// `LightUniform` must stay byte-identical to the copies in [`MESH_WGSL`] and
/// [`TERRAIN_WGSL`]: all three read the same `light_buffer`.
const PROBE_CAPTURE_WGSL: &str = r#"
const MAX_POINT_LIGHTS: u32 = 8u;
const MAX_SPOT_LIGHTS: u32 = 8u;
const PI: f32 = 3.14159265358979323846;
struct CaptureUniform {
    view_proj: mat4x4<f32>,
    light_view_proj: mat4x4<f32>,
    inv_view_proj: mat4x4<f32>,
    probe_pos: vec3<f32>,
    _pad: f32,
};
struct ModelUniform {
    model: mat4x4<f32>,
    metallic: f32,
    roughness: f32,
    _pad0: f32,
    _pad1: f32,
    emissive: vec3<f32>,
    _pad2: f32,
    base_color: vec3<f32>,
    opacity: f32,
};
struct PointLightEntry {
    position: vec3<f32>,
    _pad0: f32,
    color: vec3<f32>,
    intensity: f32,
    range: f32,
    _pad1: f32,
    _pad2: f32,
    _pad3: f32,
};
struct SpotLightEntry {
    position: vec3<f32>,
    _pad0: f32,
    direction: vec3<f32>,
    inner_cos: f32,
    color: vec3<f32>,
    outer_cos: f32,
    intensity: f32,
    range: f32,
    _pad1: f32,
    _pad2: f32,
};
struct LightUniform {
    direction: vec3<f32>,
    _pad0: f32,
    color: vec3<f32>,
    _pad1: f32,
    ambient: vec3<f32>,
    num_point_lights: u32,
    point_lights: array<PointLightEntry, 8>,
    num_spot_lights: u32,
    ibl_enabled: u32,
    ibl_max_mip: f32,
    _pad4: f32,
    spot_lights: array<SpotLightEntry, 8>,
};
@group(0) @binding(0) var<uniform> capture: CaptureUniform;
@group(1) @binding(0) var<uniform> model_data: ModelUniform;
@group(2) @binding(0) var<uniform> light: LightUniform;
@group(2) @binding(1) var shadow_sampler: sampler_comparison;
@group(2) @binding(2) var shadow_map: texture_depth_2d;
@group(2) @binding(4) var point_shadow_map: texture_2d_array<f32>;
// Bindings 3 and 5..8 of group 2 exist in the layout (the point-shadow
// sampler and the three IBL resources) and are deliberately NOT declared
// here: an unused binding is legal, and declaring the IBL cubes is the first
// step toward accidentally sampling them.
@group(3) @binding(0) var t_diffuse: texture_2d<f32>;
@group(3) @binding(1) var s_diffuse: sampler;

struct VertIn {
    @location(0) pos: vec3<f32>,
    @location(1) col: vec3<f32>,
    @location(2) normal: vec3<f32>,
    @location(3) uv: vec2<f32>,
}
struct VertOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) col: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) world_pos: vec3<f32>,
    @location(4) light_space_pos: vec4<f32>,
}
@vertex
fn vs_capture(in: VertIn) -> VertOut {
    var out: VertOut;
    let world_pos4 = model_data.model * vec4<f32>(in.pos, 1.0);
    out.clip_pos = capture.view_proj * world_pos4;
    out.world_pos = world_pos4.xyz;
    out.col = in.col;
    let normal_matrix = mat3x3<f32>(
        model_data.model[0].xyz,
        model_data.model[1].xyz,
        model_data.model[2].xyz,
    );
    out.world_normal = normalize(normal_matrix * in.normal);
    out.uv = in.uv;
    out.light_space_pos = capture.light_view_proj * world_pos4;
    return out;
}
fn shadow_factor(lsp: vec4<f32>) -> f32 {
    let proj = lsp.xyz / lsp.w;
    let uv = proj.xy * vec2<f32>(0.5, -0.5) + vec2<f32>(0.5, 0.5);
    let depth = proj.z;
    if (uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0 || depth < 0.0 || depth > 1.0) {
        return 1.0;
    }
    return textureSampleCompare(shadow_map, shadow_sampler, uv, depth - 0.003);
}
fn point_shadow_factor(light_index: u32, to_frag: vec3<f32>) -> f32 {
    let ax = abs(to_frag.x);
    let ay = abs(to_frag.y);
    let az = abs(to_frag.z);
    var face: u32;
    var u: f32;
    var v: f32;
    var ma: f32;
    if (ax >= ay && ax >= az) {
        ma = ax;
        if (to_frag.x > 0.0) {
            face = 0u;
            u = -to_frag.z;
            v = -to_frag.y;
        } else {
            face = 1u;
            u = to_frag.z;
            v = -to_frag.y;
        }
    } else if (ay >= ax && ay >= az) {
        ma = ay;
        if (to_frag.y > 0.0) {
            face = 2u;
            u = to_frag.x;
            v = to_frag.z;
        } else {
            face = 3u;
            u = to_frag.x;
            v = -to_frag.z;
        }
    } else {
        ma = az;
        if (to_frag.z > 0.0) {
            face = 4u;
            u = to_frag.x;
            v = -to_frag.y;
        } else {
            face = 5u;
            u = -to_frag.x;
            v = -to_frag.y;
        }
    }
    let ndc = vec2<f32>(u, v) / max(ma, 0.0001);
    let uv = ndc * vec2<f32>(0.5, -0.5) + vec2<f32>(0.5, 0.5);
    let size = 512.0;
    let px = vec2<i32>(clamp(uv, vec2<f32>(0.0), vec2<f32>(0.999999)) * size);
    let layer = i32(light_index * 6u + face);
    let stored = textureLoad(point_shadow_map, px, layer, 0).r;
    let dist = length(to_frag);
    if (dist - 0.1 > stored) {
        return 0.0;
    }
    return 1.0;
}
fn distribution_ggx(n_dot_h: f32, roughness: f32) -> f32 {
    let a = roughness * roughness;
    let a2 = a * a;
    let denom = n_dot_h * n_dot_h * (a2 - 1.0) + 1.0;
    return a2 / (PI * denom * denom);
}
fn geometry_schlick_ggx(ndotx: f32, roughness: f32) -> f32 {
    let r = roughness + 1.0;
    let k = r * r / 8.0;
    return ndotx / (ndotx * (1.0 - k) + k);
}
fn geometry_smith(n_dot_v: f32, n_dot_l: f32, roughness: f32) -> f32 {
    return geometry_schlick_ggx(n_dot_v, roughness) * geometry_schlick_ggx(n_dot_l, roughness);
}
fn fresnel_schlick(cos_theta: f32, f0: vec3<f32>) -> vec3<f32> {
    return f0 + (vec3<f32>(1.0, 1.0, 1.0) - f0) * pow(clamp(1.0 - cos_theta, 0.0, 1.0), 5.0);
}
@fragment
fn fs_capture(in: VertOut) -> @location(0) vec4<f32> {
    let n = normalize(in.world_normal);
    // The "eye" for this capture is the probe itself: what the probe records
    // is the radiance leaving each surface *toward the probe*.
    let v = normalize(capture.probe_pos - in.world_pos);
    let albedo = textureSample(t_diffuse, s_diffuse, in.uv).rgb * in.col * model_data.base_color;
    let metallic = model_data.metallic;
    let roughness = max(model_data.roughness, 0.04);
    let f0 = mix(vec3<f32>(0.04, 0.04, 0.04), albedo, metallic);
    let n_dot_v = max(dot(n, v), 0.0001);
    let lit = shadow_factor(in.light_space_pos);

    var lo = vec3<f32>(0.0, 0.0, 0.0);
    {
        let l = normalize(-light.direction);
        let h = normalize(v + l);
        let n_dot_l = max(dot(n, l), 0.0);
        let n_dot_h = max(dot(n, h), 0.0);
        let h_dot_v = max(dot(h, v), 0.0);
        let ndf = distribution_ggx(n_dot_h, roughness);
        let g = geometry_smith(n_dot_v, n_dot_l, roughness);
        let f = fresnel_schlick(h_dot_v, f0);
        let kd = (vec3<f32>(1.0, 1.0, 1.0) - f) * (1.0 - metallic);
        let specular = (ndf * g * f) / (4.0 * n_dot_v * n_dot_l + 0.0001);
        lo += (kd * albedo / PI + specular) * light.color * n_dot_l * lit;
    }
    for (var i: u32 = 0u; i < light.num_point_lights; i++) {
        let pl = light.point_lights[i];
        let to_light = pl.position - in.world_pos;
        let dist = length(to_light);
        if dist < pl.range {
            let l = normalize(to_light);
            let h = normalize(v + l);
            let n_dot_l = max(dot(n, l), 0.0);
            let n_dot_h = max(dot(n, h), 0.0);
            let h_dot_v = max(dot(h, v), 0.0);
            let t = 1.0 - dist / pl.range;
            let ndf = distribution_ggx(n_dot_h, roughness);
            let g = geometry_smith(n_dot_v, n_dot_l, roughness);
            let f = fresnel_schlick(h_dot_v, f0);
            let kd = (vec3<f32>(1.0, 1.0, 1.0) - f) * (1.0 - metallic);
            let specular = (ndf * g * f) / (4.0 * n_dot_v * n_dot_l + 0.0001);
            let pt_lit = point_shadow_factor(i, -to_light);
            lo += (kd * albedo / PI + specular) * pl.color * (pl.intensity * t * t) * n_dot_l * pt_lit;
        }
    }
    for (var j: u32 = 0u; j < light.num_spot_lights; j++) {
        let sl = light.spot_lights[j];
        let to_light = sl.position - in.world_pos;
        let dist = length(to_light);
        if dist < sl.range {
            let light_dir = normalize(to_light);
            let cos_angle = dot(-light_dir, sl.direction);
            let spot_factor = smoothstep(sl.outer_cos, sl.inner_cos, cos_angle);
            if spot_factor > 0.0 {
                let l = light_dir;
                let h = normalize(v + l);
                let n_dot_l = max(dot(n, l), 0.0);
                let n_dot_h = max(dot(n, h), 0.0);
                let h_dot_v = max(dot(h, v), 0.0);
                let t = 1.0 - dist / sl.range;
                let ndf = distribution_ggx(n_dot_h, roughness);
                let g = geometry_smith(n_dot_v, n_dot_l, roughness);
                let f = fresnel_schlick(h_dot_v, f0);
                let kd = (vec3<f32>(1.0, 1.0, 1.0) - f) * (1.0 - metallic);
                let specular = (ndf * g * f) / (4.0 * n_dot_v * n_dot_l + 0.0001);
                lo += (kd * albedo / PI + specular) * sl.color * (sl.intensity * t * t * spot_factor) * n_dot_l;
            }
        }
    }
    // The flat ambient term stays -- it is a *direct* constant light in this
    // engine's model, not a bounce, so it cannot make the bake recursive.
    // What is deliberately missing is the image-based-lighting branch
    // MESH_WGSL has at this exact point: see this shader's doc comment.
    let ambient_term = light.ambient * albedo;
    let color = ambient_term + lo + model_data.emissive;
    return vec4<f32>(color, 1.0);
}
"#;

/// The skybox, drawn as the background of every probe capture face.
///
/// A separate module rather than another entry point in
/// [`PROBE_CAPTURE_WGSL`]: the sky texture would have to occupy a
/// `@group @binding` slot that shader already gives a different type, and two
/// conflicting declarations of one binding is a WGSL validation error whether
/// or not both entry points use them.
///
/// The probe must see the sky, because sky light reaching a wall and bouncing
/// onto the floor is exactly the effect probes exist to capture. Sampling the
/// sky here (as *background radiance*) is not the same as sampling IBL: this
/// is the environment itself, at one texel, not an irradiance convolution fed
/// back into the shading.
const PROBE_CAPTURE_SKY_WGSL: &str = r#"
const PI: f32 = 3.14159265358979323846;
struct CaptureUniform {
    view_proj: mat4x4<f32>,
    light_view_proj: mat4x4<f32>,
    inv_view_proj: mat4x4<f32>,
    probe_pos: vec3<f32>,
    _pad: f32,
};
@group(0) @binding(0) var<uniform> capture: CaptureUniform;
@group(1) @binding(0) var t_sky: texture_2d<f32>;
@group(1) @binding(1) var s_sky: sampler;
struct SkyOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) ndc: vec2<f32>,
};
@vertex
fn vs_capture_sky(@builtin(vertex_index) vi: u32) -> SkyOut {
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>( 3.0, -1.0),
        vec2<f32>(-1.0,  3.0),
    );
    let p = positions[vi];
    var out: SkyOut;
    // z = w so NDC depth = 1.0; LessEqual passes only where no geometry drew.
    out.clip_pos = vec4<f32>(p.x, p.y, 1.0, 1.0);
    out.ndc = p;
    return out;
}
@fragment
fn fs_capture_sky(in: SkyOut) -> @location(0) vec4<f32> {
    // Same equirectangular lookup SKYBOX_WGSL does, through this face's own
    // inverse view-projection instead of the camera's.
    let world = capture.inv_view_proj * vec4<f32>(in.ndc.x, in.ndc.y, 1.0, 1.0);
    let dir = normalize(world.xyz);
    let phi = atan2(dir.z, dir.x);
    let theta = asin(clamp(dir.y, -1.0, 1.0));
    let u = phi / (2.0 * PI) + 0.5;
    let v = 0.5 - theta / PI;
    return textureSample(t_sky, s_sky, vec2<f32>(u, v));
}
"#;

pub(crate) const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
const MAX_OBJECTS: usize = 1024;
const MODEL_STRIDE: u64 = 256;
// view_proj(64) + light_view_proj(64) + cam_pos(12) + pad(4) = 144
const CAMERA_UNIFORM_SIZE: u64 = 144;
// inv_vp mat4x4<f32> = 64 bytes
const SKY_UNIFORM_SIZE: u64 = 64;
// direction(16) + color(16) + ambient+count(16) + 8×PointLightGpu(48=384) +
// num_spot+pad(16) + 8×SpotLightGpu(64=512) = 960
// `pub(crate)`: the froxel injection pass binds this same buffer, and its own
// layout has to declare the identical `min_binding_size`.
pub(crate) const LIGHT_UNIFORM_SIZE: u64 = 960;
// The IBL flags took two of the three spare floats in that `num_spot+pad(16)`
// block rather than growing the struct, so this must still hold. It is also
// the layout's `min_binding_size`, so a mismatch would be a bind group
// validation failure at runtime rather than here.
const _: () = assert!(
    std::mem::size_of::<LightUniformData>() as u64 == LIGHT_UNIFORM_SIZE,
    "LightUniformData must stay exactly LIGHT_UNIFORM_SIZE bytes"
);
// Vertex stride: position(12) + color(12) + normal(12) + uv(8) = 44 bytes
const VERTEX_STRIDE: u64 = 44;
const SHADOW_MAP_SIZE: u32 = 2048;
/// Deliberately much smaller than `SHADOW_MAP_SIZE` — at this size, 48 layers
/// (`MAX_POINT_LIGHTS` * 6 faces) of `R32Float` is ~48 MiB; at 2048 it would be
/// ~768 MiB, unreasonable for a secondary shadow feature.
const POINT_SHADOW_MAP_SIZE: u32 = 512;
/// Per-face dynamic-offset stride for `point_shadow_uniform_buffer`, mirroring
/// `MODEL_STRIDE`'s 256-byte convention (`PointShadowUniformData` is 80 bytes;
/// 256 satisfies wgpu's minimum uniform buffer offset alignment on every backend).
const POINT_SHADOW_STRIDE: u64 = 256;

/// Edge length, in texels, of one cube face of a probe capture.
///
/// Deliberately tiny. The capture's entire purpose is to be reduced to nine
/// SH coefficients immediately afterward, and an L2 expansion cannot represent
/// anything finer than a very broad lobe -- every texel above this resolution
/// is work whose result is thrown away by the projection. 16x16x6 is already
/// 1536 samples per probe, far more than nine coefficients need.
const PROBE_FACE_SIZE: u32 = 16;
/// Per-face dynamic-offset stride for `probe_capture_uniform_buffer`, same
/// 256-byte convention as `MODEL_STRIDE` and `POINT_SHADOW_STRIDE`
/// (`ProbeCaptureUniformData` is 208 bytes).
const PROBE_CAPTURE_STRIDE: u64 = 256;
/// Far plane of each probe capture frustum. A probe integrates the whole
/// sphere around it, so this is "how far away scene geometry still counts",
/// not a light range -- generous, because a wall that falls outside it simply
/// vanishes from the probe and its colour never bleeds.
const PROBE_CAPTURE_RANGE: f32 = 200.0;
/// Colour format of the probe capture target. HDR, like the main scene buffer:
/// probe radiance is pre-tonemap and routinely exceeds 1.0, and clipping it to
/// 8-bit here would quietly cap how much light a probe can carry.
const PROBE_CAPTURE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
/// Size of [`ProbeUniformData`], and the group-2 binding-9 `min_binding_size`.
///
/// `MAX_PROBES(32) * 9 coefficients * vec4(16)` = 4608 bytes of coefficients,
/// plus 48 bytes describing the volume's box and grid. Far inside the 64 KiB
/// uniform binding limit, which is what makes a fixed-size uniform array a
/// workable substitute for the storage buffer this engine does not use.
const PROBE_UNIFORM_SIZE: u64 = 4656;

/// GPU-uniform layout for the baked probe grid. Mirrors `ProbeUniform` in
/// [`MESH_WGSL`].
///
/// A uniform buffer with a fixed maximum rather than a storage buffer: this
/// engine uses no storage buffers anywhere, and fixed-size uniform arrays are
/// its established pattern (`MAX_POINT_LIGHTS`).
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct ProbeUniformData {
    /// Nine coefficients per probe, `vec4` each because std140 pads a `vec3`
    /// array element out to 16 bytes regardless. Nested rather than a flat
    /// `[[f32; 4]; 288]` purely so the probe index is visible at the use site;
    /// the bytes are identical, and the shader reads them flat.
    coeffs: [[[f32; 4]; crate::sh::SH_COEFF_COUNT]; bsengine_core::MAX_PROBES],
    /// World-space minimum corner of the volume.
    origin: [f32; 3],
    /// Nonzero when probe GI should be sampled at all. Zero -- with zeroed
    /// coefficients -- is what a scene with no volume uploads: a bind group
    /// layout is fixed at creation, so the binding cannot simply be skipped.
    enabled: u32,
    /// World-space size of the volume (full extent, not half).
    extent: [f32; 3],
    _pad0: f32,
    /// Probes per axis, on the grid's lattice points.
    resolution: [u32; 3],
    _pad1: u32,
}

// The layout's `min_binding_size`, so a mismatch here would surface as a bind
// group validation failure at runtime rather than at compile time. It also has
// to match `ProbeUniform` in MESH_WGSL byte for byte: a shorter Rust struct
// would leave the shader reading the volume's bounds out of coefficient data.
const _: () = assert!(
    std::mem::size_of::<ProbeUniformData>() as u64 == PROBE_UNIFORM_SIZE,
    "ProbeUniformData must stay exactly PROBE_UNIFORM_SIZE bytes"
);

/// The world-space box and grid one probe bake was performed for.
///
/// [`WgpuSurface::render_frame`] takes this per frame and re-bakes only when it
/// differs from the last bake, which is what turns "bake once after load" into
/// a rule the render system can enforce without a second ECS system or any
/// `Added`/`Changed` bookkeeping.
///
/// It describes the **volume**, not the scene inside it. Moving a wall after
/// load does not re-bake, and the floor keeps the colour the wall used to
/// bleed onto it -- the accepted cost of baking once rather than every frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProbeVolumeParams {
    /// World-space minimum corner of the box.
    pub origin: Vec3,
    /// World-space size of the box (full extent, not half).
    pub extent: Vec3,
    /// Probes per axis, already through
    /// [`bsengine_core::LightProbeVolume::clamped_resolution`].
    pub resolution: [u32; 3],
}

/// World positions of one volume's probes, in the order the scene shader
/// indexes them: x fastest, then y, then z.
///
/// The first probe on each axis sits exactly on the box's minimum face and the
/// last exactly on its maximum face. Trilinear interpolation is defined by the
/// eight *corners* of a cell, so lattice-point placement is what makes the
/// interpolation well-formed; probes at cell centres would leave the outer
/// half-cell of the volume extrapolating instead.
fn probe_positions(params: &ProbeVolumeParams) -> Vec<Vec3> {
    let [rx, ry, rz] = params.resolution;
    let mut out = Vec::with_capacity((rx * ry * rz) as usize);
    // `clamped_resolution` guarantees at least 2 per axis; a single probe on an
    // axis has no cell to span, so it sits on the minimum face.
    let t = |i: u32, r: u32| {
        if r <= 1 {
            0.0
        } else {
            i as f32 / (r - 1) as f32
        }
    };
    for z in 0..rz {
        for y in 0..ry {
            for x in 0..rx {
                out.push(params.origin + params.extent * Vec3::new(t(x, rx), t(y, ry), t(z, rz)));
            }
        }
    }
    out
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct CameraUniformData {
    view_proj: [[f32; 4]; 4],
    light_view_proj: [[f32; 4]; 4],
    cam_pos: [f32; 3],
    /// Seconds elapsed since app startup — was an unused padding field
    /// (`cam_pos: vec3<f32>` needs 16-byte alignment; this fills the
    /// remaining 4 bytes), now exposed to shaders as `camera.time`.
    time: f32,
}

/// Material parameters uploaded per draw call.
pub struct MaterialParams {
    /// 0 = fully dielectric, 1 = fully metallic, PBR-style.
    pub metallic: f32,
    /// Surface microfacet roughness, 0 = mirror-smooth, 1 = fully rough.
    pub roughness: f32,
    /// Self-illumination color added regardless of lighting.
    pub emissive: Vec3,
    /// Base albedo color multiplied with the surface texture (if any).
    pub base_color: Vec3,
    /// 1.0 = opaque. Below that the draw leaves the opaque pass for the sorted
    /// transparent one.
    pub opacity: f32,
}

impl Default for MaterialParams {
    fn default() -> Self {
        Self {
            metallic: 0.0,
            roughness: 0.5,
            emissive: Vec3::ZERO,
            base_color: Vec3::ONE,
            opacity: 1.0,
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct ModelUniformData {
    model: [[f32; 4]; 4],
    metallic: f32,
    roughness: f32,
    _pad0: f32,
    _pad1: f32,
    emissive: [f32; 3],
    _pad2: f32,
    base_color: [f32; 3],
    // Was `_pad3`. The slot the padding occupied is exactly where opacity
    // belongs, so carrying it costs nothing in size or alignment.
    opacity: f32,
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct PointShadowUniformData {
    view_proj: [[f32; 4]; 4],
    light_pos: [f32; 3],
    _pad: f32,
}

/// One probe capture face's camera, addressed by dynamic offset. Mirrors
/// `CaptureUniform` in [`PROBE_CAPTURE_WGSL`] and [`PROBE_CAPTURE_SKY_WGSL`].
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct ProbeCaptureUniformData {
    view_proj: [[f32; 4]; 4],
    light_view_proj: [[f32; 4]; 4],
    /// Rotation-only inverse of `view_proj`, for the sky background.
    inv_view_proj: [[f32; 4]; 4],
    /// The probe's world position, which is this capture's eye point.
    probe_pos: [f32; 3],
    _pad: f32,
}

// The dynamic offset of face `n` is `n * PROBE_CAPTURE_STRIDE`, so a struct
// larger than the stride would have consecutive faces overwriting each other's
// tails -- silently, and only for the faces after the first.
const _: () = assert!(
    std::mem::size_of::<ProbeCaptureUniformData>() as u64 <= PROBE_CAPTURE_STRIDE,
    "ProbeCaptureUniformData must fit within PROBE_CAPTURE_STRIDE"
);

/// A single point light entry for the GPU buffer.
pub struct PointLightEntry {
    /// World-space position of the light.
    pub position: Vec3,
    /// Light color (linear RGB, unclamped so intensity can exceed 1.0 per channel).
    pub color: Vec3,
    /// Brightness multiplier applied to `color`.
    pub intensity: f32,
    /// Distance at which the light's contribution falls off to zero.
    pub range: f32,
}

/// A single spot light entry for the GPU buffer.
pub struct SpotLightEntry {
    /// World-space position of the light.
    pub position: Vec3,
    /// World-space direction the cone points toward.
    pub direction: Vec3,
    /// Light color (linear RGB, unclamped so intensity can exceed 1.0 per channel).
    pub color: Vec3,
    /// Brightness multiplier applied to `color`.
    pub intensity: f32,
    /// Distance at which the light's contribution falls off to zero.
    pub range: f32,
    /// Half-angle (radians) of the cone's fully-lit inner core.
    pub inner_angle: f32,
    /// Half-angle (radians) of the cone's outer falloff edge.
    pub outer_angle: f32,
}

/// Light parameters passed per frame.
pub struct LightData {
    /// Directional (sun) light's world-space direction.
    pub direction: Vec3,
    /// Directional (sun) light's color.
    pub color: Vec3,
    /// Flat ambient light added to every surface regardless of direction.
    pub ambient: Vec3,
    /// Active point lights this frame (uploaded up to a fixed GPU-side cap).
    pub point_lights: Vec<PointLightEntry>,
    /// Active spot lights this frame (uploaded up to a fixed GPU-side cap).
    pub spot_lights: Vec<SpotLightEntry>,
}

impl Default for LightData {
    fn default() -> Self {
        Self {
            direction: Vec3::new(-0.4, -0.8, -0.4).normalize(),
            color: Vec3::ONE,
            ambient: Vec3::splat(0.15),
            point_lights: Vec::new(),
            spot_lights: Vec::new(),
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct PointLightGpu {
    position: [f32; 3],
    _pad0: f32,
    color: [f32; 3],
    intensity: f32,
    range: f32,
    _pad1: f32,
    _pad2: f32,
    _pad3: f32,
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct SpotLightGpu {
    position: [f32; 3],
    _pad0: f32,
    direction: [f32; 3],
    inner_cos: f32,
    color: [f32; 3],
    outer_cos: f32,
    intensity: f32,
    range: f32,
    _pad1: f32,
    _pad2: f32,
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct LightUniformData {
    direction: [f32; 3],
    _pad0: f32,
    color: [f32; 3],
    _pad1: f32,
    ambient: [f32; 3],
    num_point_lights: u32,
    point_lights: [PointLightGpu; 8],
    num_spot_lights: u32,
    /// Non-zero when `WgpuSurface::ibl` holds maps for the current skybox.
    /// Zero makes the shader fall back to the flat `ambient * albedo` term,
    /// which is why the IBL bindings can be dummies without changing a pixel.
    ///
    /// Took `_pad2`'s slot, and a `u32` is the same 4 bytes that `f32` was, so
    /// `LIGHT_UNIFORM_SIZE` is still 960.
    ibl_enabled: u32,
    /// Highest mip index of the prefiltered specular cube, i.e. the mip the
    /// shader samples at `roughness == 1`. Took `_pad3`'s slot.
    ibl_max_mip: f32,
    _pad4: f32,
    spot_lights: [SpotLightGpu; 8],
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct SkyUniformData {
    inv_vp: [[f32; 4]; 4],
}

struct SkyboxState {
    pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    uniform_bg: wgpu::BindGroup,
    texture_bg: wgpu::BindGroup,
    _texture: crate::profiler::TrackedTexture,
    _sampler: wgpu::Sampler,
}

/// Maps our windowing-layer key codes to egui's key enum, for the subset
/// egui's TextEdit/DragValue widgets actually need (arrows, editing keys,
/// alphanumerics for shortcuts). Modifier keys (Ctrl/Shift/Alt) are not
/// mapped here — they're carried on `Modifiers` instead, matching egui's
/// convention of not treating them as standalone `Event::Key` presses.
fn map_keycode_to_egui(code: bsengine_input::KeyCode) -> Option<egui::Key> {
    use bsengine_input::KeyCode as K;
    Some(match code {
        K::A => egui::Key::A,
        K::B => egui::Key::B,
        K::C => egui::Key::C,
        K::D => egui::Key::D,
        K::E => egui::Key::E,
        K::F => egui::Key::F,
        K::G => egui::Key::G,
        K::H => egui::Key::H,
        K::I => egui::Key::I,
        K::J => egui::Key::J,
        K::K => egui::Key::K,
        K::L => egui::Key::L,
        K::M => egui::Key::M,
        K::N => egui::Key::N,
        K::O => egui::Key::O,
        K::P => egui::Key::P,
        K::Q => egui::Key::Q,
        K::R => egui::Key::R,
        K::S => egui::Key::S,
        K::T => egui::Key::T,
        K::U => egui::Key::U,
        K::V => egui::Key::V,
        K::W => egui::Key::W,
        K::X => egui::Key::X,
        K::Y => egui::Key::Y,
        K::Z => egui::Key::Z,
        K::Space => egui::Key::Space,
        K::Enter => egui::Key::Enter,
        K::Escape => egui::Key::Escape,
        K::Backspace => egui::Key::Backspace,
        K::Tab => egui::Key::Tab,
        K::Left => egui::Key::ArrowLeft,
        K::Right => egui::Key::ArrowRight,
        K::Up => egui::Key::ArrowUp,
        K::Down => egui::Key::ArrowDown,
        K::Key0 => egui::Key::Num0,
        K::Key1 => egui::Key::Num1,
        K::Key2 => egui::Key::Num2,
        K::Key3 => egui::Key::Num3,
        K::Key4 => egui::Key::Num4,
        K::Key5 => egui::Key::Num5,
        K::Key6 => egui::Key::Num6,
        K::Key7 => egui::Key::Num7,
        K::Key8 => egui::Key::Num8,
        K::Key9 => egui::Key::Num9,
        K::Delete => egui::Key::Delete,
        K::Minus => egui::Key::Minus,
        K::Period => egui::Key::Period,
        K::Comma => egui::Key::Comma,
        K::Home => egui::Key::Home,
        K::End => egui::Key::End,
        K::Equals
        | K::ControlLeft
        | K::ControlRight
        | K::ShiftLeft
        | K::ShiftRight
        | K::AltLeft
        | K::AltRight
        | K::Unknown => return None,
    })
}

/// Applies a TAA sub-pixel jitter to the view-projection used for
/// rasterization, leaving the caller's unjittered matrix alone.
///
/// The offset goes on the **projection's third column** (glam is
/// column-major, so `z_axis.x`/`z_axis.y` are rows 0 and 1 of that column).
/// That is the column the perspective divide scales by `w`, so the resulting
/// NDC shift is the same sub-pixel amount at every depth. Adding the offset to
/// the *combined* view-projection instead would scale it by the world-space z
/// coordinate — a shear, not a sub-pixel nudge.
///
/// `cam_proj` is the projection factor of `view_proj` (every caller builds the
/// latter as `cam_proj * view`), so `cam_proj.inverse() * view_proj` recovers
/// the view matrix to re-compose the jittered projection against.
///
/// A degenerate `cam_proj` returns `view_proj` unchanged rather than a matrix
/// full of NaNs: the editor override hands over an all-zero `editor_proj`
/// until its orbit camera has run once.
fn jittered_view_proj(view_proj: Mat4, cam_proj: Mat4, jitter_clip: (f32, f32)) -> Mat4 {
    if jitter_clip == (0.0, 0.0) || cam_proj.determinant().abs() < f32::EPSILON {
        return view_proj;
    }
    let mut jittered_proj = cam_proj;
    jittered_proj.z_axis.x += jitter_clip.0;
    jittered_proj.z_axis.y += jitter_clip.1;
    jittered_proj * (cam_proj.inverse() * view_proj)
}

/// The camera's near and far clip distances, recovered from its projection.
///
/// The froxel grid slices the range between them, and the apply pass inverts
/// that slicing, so both need the two numbers as scalars. Reading them back out
/// of the same matrix the frame rasterises with -- rather than plumbing
/// `Camera::near`/`far` down as two more parameters -- is what keeps them from
/// ever disagreeing with the depth buffer the apply pass unprojects, including
/// on the editor path where the projection comes from the orbit camera and no
/// `Camera` component is involved at all.
///
/// Unprojecting NDC z of 0 and 1 down the view axis gives them directly: glam's
/// `perspective_rh` writes wgpu's 0..1 depth range, so those are the two clip
/// planes. A degenerate or reversed projection falls back to
/// `Camera::default()`'s planes rather than returning zeros -- the slicing
/// divides by `log(far/near)`, and a zero near would put NaN in every froxel.
fn camera_near_far(cam_proj: Mat4) -> (f32, f32) {
    let fallback = (0.1, 1000.0);
    if cam_proj.determinant().abs() < f32::EPSILON {
        return fallback;
    }
    let inv = cam_proj.inverse();
    let view_depth_at = |ndc_z: f32| {
        let p = inv * glam::Vec4::new(0.0, 0.0, ndc_z, 1.0);
        // View space looks down -Z, so the positive depth is the negated z.
        -(p.z / p.w)
    };
    let near = view_depth_at(0.0);
    let far = view_depth_at(1.0);
    if !near.is_finite() || !far.is_finite() || near <= 0.0 || far <= near {
        return fallback;
    }
    (near, far)
}

/// The `(forward, up)` pair of every cube face, in face order.
///
/// The standard cubemap face orientation convention (+Y/-Y up-vectors on the
/// X/Z faces to avoid a degenerate look-at when looking straight up/down).
///
/// **This is the single authority for the cube-face convention in this file.**
/// [`point_light_face_view_projs`] builds its `look_at_rh` matrices from it,
/// and [`probe_face_texel_direction`] recovers a per-texel direction from it,
/// so the direction a probe attributes a captured texel to cannot drift from
/// the matrix that texel was actually rendered with. Writing a second
/// convention next to this one is how probes end up lit from the wrong side --
/// a bug whose output still looks like plausible lighting.
const CUBE_FACE_DIRS: [(Vec3, Vec3); 6] = [
    (Vec3::X, Vec3::NEG_Y),
    (Vec3::NEG_X, Vec3::NEG_Y),
    (Vec3::Y, Vec3::Z),
    (Vec3::NEG_Y, Vec3::NEG_Z),
    (Vec3::Z, Vec3::NEG_Y),
    (Vec3::NEG_Z, Vec3::NEG_Y),
];

/// The 90-degree square projection every cube face is rendered through.
fn cube_face_projection(range: f32) -> Mat4 {
    Mat4::perspective_rh(std::f32::consts::FRAC_PI_2, 1.0, 0.05, range.max(0.5))
}

/// Computes the 6 face view-projection matrices for a point light's cube shadow
/// map, one per axis-aligned direction, using [`CUBE_FACE_DIRS`].
fn point_light_face_view_projs(position: Vec3, range: f32) -> [Mat4; 6] {
    let proj = cube_face_projection(range);
    CUBE_FACE_DIRS.map(|(dir, up)| proj * Mat4::look_at_rh(position, position + dir, up))
}

/// The rotation-only inverse view-projection of each cube face, which is what
/// an equirectangular skybox lookup needs: dropping the translation leaves a
/// matrix that maps an NDC corner to a *direction*, so the sky stays infinitely
/// far away instead of sliding past the probe.
///
/// Built the same way `bsengine-render`'s `sky_vp_inv` is (strip the
/// translation column, then invert the product), so a probe's captured sky is
/// the same image the camera would see looking that way.
fn probe_face_sky_vp_invs(range: f32) -> [Mat4; 6] {
    let proj = cube_face_projection(range);
    CUBE_FACE_DIRS.map(|(dir, up)| {
        let view = Mat4::look_at_rh(Vec3::ZERO, dir, up);
        let view_rot = Mat4::from_cols(view.x_axis, view.y_axis, view.z_axis, glam::Vec4::W);
        (proj * view_rot).inverse()
    })
}

/// The world-space direction, as seen from the probe, of texel `(x, y)` on
/// cube face `face` of a probe capture.
///
/// Derived from [`CUBE_FACE_DIRS`] and glam's `look_at_rh`, not from a
/// hand-written cube convention. `look_at_rh` builds the basis
/// `f = forward, s = normalize(f x up), u = s x f` and maps a world offset `p`
/// to view space as `(dot(s, p), dot(u, p), dot(-f, p))`. A 90-degree square
/// perspective projects view-space `(vx, vy, vz)` to
/// `ndc = (vx / -vz, vy / -vz)`, so the ray through `ndc` is view-space
/// `(ndc.x, ndc.y, -1)`, which is world-space `f + ndc.x * s + ndc.y * u`.
///
/// `probe_face_texel_direction_matches_the_capture_matrix_ndc` closes that
/// derivation against the real matrices rather than trusting the algebra.
fn probe_face_texel_direction(face: usize, x: u32, y: u32) -> Vec3 {
    let (ndc_x, ndc_y) = probe_face_texel_ndc(x, y);
    let (forward, up) = CUBE_FACE_DIRS[face];
    let s = forward.cross(up).normalize();
    let u = s.cross(forward);
    (forward + s * ndc_x + u * ndc_y).normalize()
}

/// The NDC coordinates of the centre of texel `(x, y)` on a probe capture
/// face. `y` counts down the framebuffer while NDC `y` counts up, hence the
/// flip.
fn probe_face_texel_ndc(x: u32, y: u32) -> (f32, f32) {
    let n = PROBE_FACE_SIZE as f32;
    (
        ((x as f32 + 0.5) / n) * 2.0 - 1.0,
        1.0 - ((y as f32 + 0.5) / n) * 2.0,
    )
}

/// Decodes one IEEE-754 binary16 value, the element type of the `Rgba16Float`
/// probe capture target.
///
/// Hand-rolled because this workspace has no `half` dependency and pulling one
/// in for a dozen lines of bit twiddling is not worth the supply-chain
/// surface. `f16_round_trips_exact_values` and
/// `f16_decodes_subnormals_and_specials` pin the three branches.
fn f16_bits_to_f32(bits: u16) -> f32 {
    let sign = if bits & 0x8000 != 0 { -1.0f32 } else { 1.0 };
    let exp = ((bits >> 10) & 0x1f) as i32;
    let frac = (bits & 0x03ff) as f32;
    if exp == 0 {
        // Subnormal (and zero): the value is frac * 2^-24, with no implicit
        // leading 1.
        sign * frac * 2.0f32.powi(-24)
    } else if exp == 0x1f {
        if frac == 0.0 {
            sign * f32::INFINITY
        } else {
            f32::NAN
        }
    } else {
        sign * (1.0 + frac / 1024.0) * 2.0f32.powi(exp - 15)
    }
}

/// The solid angle, in steradians, that texel `(x, y)` of a probe capture face
/// subtends at the probe.
///
/// A cube face is a *flat* square held at unit distance, so its texels do not
/// all cover the same slice of the sphere: a corner texel is both further away
/// and seen edge-on. The projected solid angle of the texel at `(u, v)` in
/// `[-1, 1]` is therefore `dA * (1 + u^2 + v^2)^(-3/2)` with `dA = 4 / n^2`.
///
/// The tempting shortcut is a flat `4*PI / (6*n*n)` for every texel. It is
/// wrong in a way that survives eyeballing: it over-weights the face corners
/// (by up to `3^(3/2)`, about 5.2x, at the very corner), which biases every
/// probe's directional terms toward the cube's diagonals.
/// `probe_face_texel_solid_angles_sum_to_the_full_sphere` and
/// `a_corner_texel_covers_less_sky_than_a_centre_texel` pin both properties.
fn probe_face_texel_solid_angle(x: u32, y: u32) -> f32 {
    let n = PROBE_FACE_SIZE as f32;
    let (u, v) = probe_face_texel_ndc(x, y);
    (4.0 / (n * n)) * (1.0 + u * u + v * v).powf(-1.5)
}

/// Everything group 2 ("light") binds, gathered into one struct.
///
/// The group is built twice -- once in the constructor and again every time
/// the skybox changes, since a bind group is immutable and the IBL views it
/// holds are not -- and only the two cube views differ between those calls.
/// Passing nine loose references to a helper twice is exactly how the two
/// call sites would drift apart, so they pass this instead.
struct LightBindings<'a> {
    light_buffer: &'a wgpu::Buffer,
    shadow_sampler: &'a wgpu::Sampler,
    shadow_map_view: &'a wgpu::TextureView,
    point_shadow_sampler: &'a wgpu::Sampler,
    point_shadow_view: &'a wgpu::TextureView,
    ibl_sampler: &'a wgpu::Sampler,
    /// Real irradiance cube, or the 1x1x6 dummy when no skybox is loaded.
    irradiance_view: &'a wgpu::TextureView,
    /// Real prefiltered cube, or that same dummy.
    prefilter_view: &'a wgpu::TextureView,
    brdf_lut_view: &'a wgpu::TextureView,
    /// The baked probe grid. Always present; `enabled` is 0 when no volume
    /// has been baked.
    probe_buffer: &'a wgpu::Buffer,
}

/// Builds the light bind group. The single place the group-2 binding numbers
/// appear on the Rust side, so a skybox load cannot bind them differently from
/// how construction did.
fn create_light_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    bindings: &LightBindings,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("light bg"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: bindings.light_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(bindings.shadow_sampler),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::TextureView(bindings.shadow_map_view),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: wgpu::BindingResource::Sampler(bindings.point_shadow_sampler),
            },
            wgpu::BindGroupEntry {
                binding: 4,
                resource: wgpu::BindingResource::TextureView(bindings.point_shadow_view),
            },
            wgpu::BindGroupEntry {
                binding: 5,
                resource: wgpu::BindingResource::TextureView(bindings.irradiance_view),
            },
            wgpu::BindGroupEntry {
                binding: 6,
                resource: wgpu::BindingResource::TextureView(bindings.prefilter_view),
            },
            wgpu::BindGroupEntry {
                binding: 7,
                resource: wgpu::BindingResource::TextureView(bindings.brdf_lut_view),
            },
            wgpu::BindGroupEntry {
                binding: 8,
                resource: wgpu::BindingResource::Sampler(bindings.ibl_sampler),
            },
            wgpu::BindGroupEntry {
                binding: 9,
                resource: bindings.probe_buffer.as_entire_binding(),
            },
        ],
    })
}

/// Owns the render output target (a window's swapchain or an offscreen
/// texture), all GPU pipelines/buffers for the main scene and shadow passes,
/// the egui renderer, and per-frame render state.
pub struct WgpuSurface {
    output: crate::output::Output,
    pub(crate) device: Arc<wgpu::Device>,
    pub(crate) queue: Arc<wgpu::Queue>,
    pipeline: wgpu::RenderPipeline,
    transparent_pipeline: wgpu::RenderPipeline,
    particles: crate::particles::ParticleRenderer,
    depth_texture: crate::profiler::TrackedTexture,
    depth_view: wgpu::TextureView,
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    model_buffer: wgpu::Buffer,
    model_bind_group: wgpu::BindGroup,
    light_buffer: wgpu::Buffer,
    light_bind_group: wgpu::BindGroup,
    /// Kept because `light_bind_group` is rebuilt whenever the skybox changes:
    /// a bind group is immutable, so swapping the dummy IBL maps for real ones
    /// (or back) means building a new one against this same layout.
    light_bgl: wgpu::BindGroupLayout,
    _white_texture: crate::profiler::TrackedTexture,
    _sampler: wgpu::Sampler,
    default_texture_bind_group: wgpu::BindGroup,
    shadow_pipeline: wgpu::RenderPipeline,
    _shadow_map_texture: crate::profiler::TrackedTexture,
    shadow_map_view: wgpu::TextureView,
    // No longer underscore-prefixed: the two shadow samplers and the point
    // shadow array view are read back whenever `light_bind_group` is rebuilt.
    shadow_comparison_sampler: wgpu::Sampler,
    point_shadow_pipeline: wgpu::RenderPipeline,
    _point_shadow_color_texture: crate::profiler::TrackedTexture,
    _point_shadow_depth_texture: crate::profiler::TrackedTexture,
    point_shadow_color_full_view: wgpu::TextureView,
    point_shadow_depth_view: wgpu::TextureView,
    point_shadow_sampler: wgpu::Sampler,
    point_shadow_uniform_buffer: wgpu::Buffer,
    point_shadow_bind_group: wgpu::BindGroup,
    /// Probe capture target: one cube face per `(probe, face)` as an array
    /// layer, laid out exactly like `_point_shadow_color_texture`
    /// (`probe * 6 + face`). Underscore-prefixed only in the sense that
    /// nothing samples it in a shader -- `project_captures_to_sh` copies out
    /// of it, which is why it is `COPY_SRC`.
    probe_capture_texture: crate::profiler::TrackedTexture,
    /// One shared depth buffer for the capture, cleared per face, exactly as
    /// the point-shadow pass reuses `point_shadow_depth_view`.
    _probe_capture_depth_texture: crate::profiler::TrackedTexture,
    probe_capture_depth_view: wgpu::TextureView,
    /// Draws scene geometry into a capture face with **direct light only** --
    /// see [`PROBE_CAPTURE_WGSL`] for why that restriction is load-bearing.
    probe_capture_pipeline: wgpu::RenderPipeline,
    /// Fills the rest of a capture face with the skybox, when one is loaded.
    probe_capture_sky_pipeline: wgpu::RenderPipeline,
    /// Per-face camera for the capture, one `PROBE_CAPTURE_STRIDE` slot per
    /// `(probe, face)`. A capture renders every face inside one command
    /// encoder, and `queue.write_buffer` cannot vary between passes of a
    /// single submission -- the same constraint that gave the point-shadow
    /// pass its own per-face uniform buffer.
    probe_capture_uniform_buffer: wgpu::Buffer,
    probe_capture_bind_group: wgpu::BindGroup,
    /// The baked probe grid the scene shader samples, at group 2 binding 9.
    ///
    /// Bound in every frame, not only when a volume exists: a bind group layout
    /// is fixed at creation, so the no-probe case is expressed as `enabled: 0`
    /// with zeroed coefficients rather than as a missing binding -- the same
    /// arrangement [`Self::dummy_ibl_cube_view`] gives the IBL maps.
    probe_buffer: wgpu::Buffer,
    /// The volume [`Self::probe_buffer`]'s contents were baked for, or `None`
    /// when nothing has been baked and the buffer is still the zeroed,
    /// `enabled: 0` one construction wrote.
    ///
    /// Comparing this against each frame's volume is the whole of the
    /// bake-once policy: a bake happens when a volume appears or its own
    /// parameters change, and never otherwise.
    baked_probe_volume: Option<ProbeVolumeParams>,
    /// Layout of the skybox's texture+sampler group. Held here rather than
    /// built inside `set_skybox_from_rgba` because `probe_capture_sky_pipeline`
    /// is built once at construction and has to bind `SkyboxState::texture_bg`
    /// against the very same layout object.
    sky_tex_bgl: wgpu::BindGroupLayout,
    egui_ctx: egui::Context,
    egui_renderer: egui_wgpu::Renderer,
    skybox: Option<SkyboxState>,
    loaded_skybox_path: Option<String>,
    /// The irradiance and prefiltered specular maps convolved from the current
    /// skybox, or `None` when no skybox is loaded.
    ///
    /// Rebuilt by [`Self::set_skybox_from_rgba`] and dropped by
    /// [`Self::clear_skybox`], because both maps are convolutions of one
    /// specific environment and mean nothing once that environment is gone.
    ibl: Option<crate::ibl::IblMaps>,
    /// The BRDF integration LUT, sampled by `(n_dot_v, roughness)` for the
    /// split-sum approximation's second term.
    ///
    /// Unlike [`Self::ibl`] this is **not** optional and is never rebuilt: it is
    /// a pure function of the BRDF, so one table generated at construction
    /// serves every skybox, and every frame without one. That is also why it
    /// needs no dummy counterpart -- it is bound at group 2 binding 7 in every
    /// frame, skybox or not.
    brdf_lut_view: wgpu::TextureView,
    /// Held only to keep the texture [`Self::brdf_lut_view`] looks at alive --
    /// and counted in the profiler -- for the lifetime of the surface.
    _brdf_lut_texture: crate::profiler::TrackedTexture,
    /// Trilinear clamped sampler for all three IBL bindings.
    ibl_sampler: wgpu::Sampler,
    /// A 1x1x6 black cubemap bound at bindings 5 and 6 whenever [`Self::ibl`]
    /// is `None`.
    ///
    /// A bind group layout is fixed at creation, so the IBL bindings exist in
    /// every scene -- including the many that never load a skybox. Rather than
    /// branch the layout (two layouts means two pipelines and two code paths
    /// that can disagree), those scenes bind this and set `ibl_enabled` to 0,
    /// which stops the shader reading it at all.
    dummy_ibl_cube_view: wgpu::TextureView,
    /// Held only to keep [`Self::dummy_ibl_cube_view`]'s texture alive.
    _dummy_ibl_cube_texture: crate::profiler::TrackedTexture,
    pipeline_layout: wgpu::PipelineLayout,
    terrain_pipeline: wgpu::RenderPipeline,
    terrain_bgl: wgpu::BindGroupLayout,
    custom_pipelines: std::collections::HashMap<String, wgpu::RenderPipeline>,
    post_process: crate::post_process::PostProcessState,
    start_time: std::time::Instant,
    dock_state: Option<egui_dock::DockState<String>>,
    last_saved_layout_json: Option<String>,
    /// When true, `render_frame` and `PostProcessState::apply` still clear
    /// the shadow map / bloom / SSAO render targets to their neutral values
    /// every frame, but skip the expensive shading work that writes anything
    /// else into them — CI's headless E2E replays are the one caller that
    /// needs this; see the design doc for why it must be clear-only, not a
    /// bare skip.
    fast_render: bool,
    /// Whether the adapter this device came from supports
    /// `wgpu::Features::TIMESTAMP_QUERY`. Set once at construction; every
    /// other `timestamp_*` field is `Some` iff this is `true`.
    timestamp_supported: bool,
    /// GPU timestamp query set `render_frame` writes begin/end pass
    /// boundaries into, when `timestamp_supported`.
    timestamp_query_set: Option<wgpu::QuerySet>,
    /// Resolve target for `timestamp_query_set` -- a query set's raw results
    /// can only be resolved into a buffer with `QUERY_RESOLVE` usage, which
    /// cannot be combined with `MAP_READ` on this adapter (no
    /// `MAPPABLE_PRIMARY_BUFFERS`), so resolving and CPU-reading are two
    /// buffers, not one.
    timestamp_resolve_buffer: Option<wgpu::Buffer>,
    /// `MAP_READ`-capable copy of `timestamp_resolve_buffer`, read back on
    /// the CPU each frame to build `FrameStats::gpu_pass_times_ms`.
    timestamp_readback_buffer: Option<wgpu::Buffer>,
    /// Rolling history of completed frames' stats, shared with
    /// `ProfilerPanel` and headless/MCP queries via
    /// [`Self::frame_stats_history`].
    frame_stats_history:
        std::sync::Arc<std::sync::Mutex<std::collections::VecDeque<crate::profiler::FrameStats>>>,
    /// The previous frame's **unjittered** view-projection, which the TAA
    /// resolve reprojects through to find where this frame's pixel sat in the
    /// image it is blending against.
    ///
    /// Deliberately unjittered: the jitter exists only to move the
    /// rasterization sample point inside a pixel, so reprojecting with it
    /// would chase the jitter instead of the camera and the accumulation
    /// would never converge.
    ///
    /// Updated at the end of every `render_frame`, whether or not TAA is
    /// enabled, so enabling it mid-run reprojects against the frame that
    /// actually preceded it rather than an arbitrarily old one.
    prev_unjittered_view_proj: Mat4,
}

impl WgpuSurface {
    /// Initializes the wgpu adapter/device/swapchain for `window` and builds
    /// every pipeline, buffer, and bind group the main render loop needs.
    pub async fn new(window: Arc<winit::window::Window>) -> Result<Self, String> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let surface = instance
            .create_surface(window.clone())
            .map_err(|e| e.to_string())?;

        let (adapter, device, queue, timestamp_supported) =
            Self::request_device(&instance, Some(&surface)).await?;

        let size = window.inner_size();
        let caps = surface.get_capabilities(&adapter);
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        Self::build(
            device,
            queue,
            crate::output::Output::Window {
                surface,
                config,
                _window: window,
            },
            false,
            timestamp_supported,
        )
    }

    /// Creates a renderer with no window at all.
    ///
    /// The frame goes to a texture this renderer owns and can be read back with
    /// [`Self::read_pixels`]. Pipelines come from the same [`Self::build`] the
    /// windowed path uses, so pixels observed here are the output of the
    /// pipelines that draw to a window.
    pub async fn new_offscreen(width: u32, height: u32, fast_render: bool) -> Result<Self, String> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });
        let (_adapter, device, queue, timestamp_supported) =
            Self::request_device(&instance, None).await?;
        let texture = crate::output::create_offscreen_texture(&device, width, height);
        Self::build(
            device,
            queue,
            crate::output::Output::Offscreen {
                texture,
                width,
                height,
            },
            fast_render,
            timestamp_supported,
        )
    }

    /// The pixels of the frame most recently rendered, tightly packed RGBA8,
    /// top row first.
    ///
    /// `Err` in windowed mode: a swapchain texture is not created with
    /// `COPY_SRC` and cannot be copied out of.
    ///
    /// The values are **sRGB-encoded**. Comparing them for brightness is fine;
    /// comparing them against linear colour values is not.
    pub fn read_pixels(&self) -> Result<Vec<u8>, String> {
        match &self.output {
            crate::output::Output::Offscreen {
                texture,
                width,
                height,
            } => Ok(crate::output::read_pixels(
                &self.device,
                &self.queue,
                texture,
                *width,
                *height,
            )),
            crate::output::Output::Window { .. } => Err(
                "read_pixels needs an offscreen renderer; a swapchain texture is not COPY_SRC"
                    .to_string(),
            ),
        }
    }

    /// The width of the render target, in pixels.
    pub fn width(&self) -> u32 {
        self.output.width()
    }

    /// The height of the render target, in pixels.
    pub fn height(&self) -> u32 {
        self.output.height()
    }

    /// Whether this renderer skips the shadow/bloom/SSAO shading work in
    /// favour of clearing those targets to their neutral values. Only ever
    /// true for `bsengine-runtime`'s CI replay path.
    pub fn is_fast_render(&self) -> bool {
        self.fast_render
    }

    /// This renderer's GPU device, for callers that must put resources on the
    /// same device -- a `GpuMeshRegistry`, for one.
    pub fn device_arc(&self) -> Arc<wgpu::Device> {
        self.device.clone()
    }

    /// This renderer's GPU queue, needed alongside [`Self::device_arc`] to build
    /// a `GpuTextureRegistry`.
    pub fn queue_arc(&self) -> Arc<wgpu::Queue> {
        self.queue.clone()
    }

    /// Requests an adapter and a device. The windowed path wants an adapter the
    /// surface can present on; offscreen takes whatever is available.
    ///
    /// Also decides, once, whether this adapter supports GPU timestamp
    /// queries (`wgpu::Features::TIMESTAMP_QUERY`) and requests the feature
    /// only when it does -- some adapters (notably CI's software/WARP
    /// adapters) do not support it, and requesting an unsupported feature
    /// fails the whole device request rather than being ignored.
    async fn request_device(
        instance: &wgpu::Instance,
        compatible_surface: Option<&wgpu::Surface<'static>>,
    ) -> Result<(wgpu::Adapter, Arc<wgpu::Device>, Arc<wgpu::Queue>, bool), String> {
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::None,
                compatible_surface,
                force_fallback_adapter: false,
            })
            .await
            .ok_or("No adapter found")?;

        let timestamp_supported = adapter.features().contains(wgpu::Features::TIMESTAMP_QUERY);
        let required_features = if timestamp_supported {
            wgpu::Features::TIMESTAMP_QUERY
        } else {
            wgpu::Features::empty()
        };

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("BSEngine surface device"),
                    required_features,
                    required_limits: wgpu::Limits::downlevel_defaults(),
                    memory_hints: wgpu::MemoryHints::default(),
                },
                None,
            )
            .await
            .map_err(|e| format!("Device request failed: {e}"))?;

        Ok((
            adapter,
            Arc::new(device),
            Arc::new(queue),
            timestamp_supported,
        ))
    }

    /// A bare device/queue pair with no surface, texture, or pipeline setup
    /// at all -- for unit tests elsewhere in this crate (`mesh.rs`,
    /// `texture.rs`, and this file's own tests) that need a real device to
    /// construct a registry against, without paying for `new_offscreen`'s
    /// full pipeline build.
    #[cfg(test)]
    pub(crate) async fn headless_device_for_testing() -> (Arc<wgpu::Device>, Arc<wgpu::Queue>) {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });
        let (_adapter, device, queue, _timestamp_supported) = Self::request_device(&instance, None)
            .await
            .expect("headless device for test");
        (device, queue)
    }

    /// Everything after the output target is settled: pipelines, buffers, bind
    /// groups, shadow maps, post-processing.
    ///
    /// **Both constructors go through here.** What makes a pixel test worth
    /// anything is that it exercises the pipelines a real frame uses; a second
    /// construction path would quietly take that away.
    fn build(
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
        output: crate::output::Output,
        fast_render: bool,
        timestamp_supported: bool,
    ) -> Result<Self, String> {
        let format = output.format();
        let width = output.width();
        let height = output.height();

        // GPU timestamp queries: only allocated when the adapter actually
        // supports them. `timestamp_resolve_buffer` is the resolve target
        // (`QUERY_RESOLVE | COPY_SRC`); `timestamp_readback_buffer` is a
        // second, `MAP_READ`-capable buffer the resolve result is copied
        // into for CPU reads -- the two usages cannot live on one buffer
        // without `Features::MAPPABLE_PRIMARY_BUFFERS`, which is not
        // requested here.
        let (timestamp_query_set, timestamp_resolve_buffer, timestamp_readback_buffer) =
            if timestamp_supported {
                let query_set = device.create_query_set(&wgpu::QuerySetDescriptor {
                    label: Some("frame profiler timestamps"),
                    ty: wgpu::QueryType::Timestamp,
                    count: TIMESTAMP_QUERY_COUNT,
                });
                let buffer_size = TIMESTAMP_QUERY_COUNT as u64 * wgpu::QUERY_SIZE as u64;
                let resolve_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("frame profiler timestamp resolve"),
                    size: buffer_size,
                    usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
                    mapped_at_creation: false,
                });
                let readback_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("frame profiler timestamp readback"),
                    size: buffer_size,
                    usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
                (Some(query_set), Some(resolve_buffer), Some(readback_buffer))
            } else {
                (None, None, None)
            };

        let (depth_texture, depth_view) = Self::create_depth_texture(&device, width, height);

        let camera_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("camera uniform"),
            size: CAMERA_UNIFORM_SIZE,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let camera_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("camera bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: wgpu::BufferSize::new(CAMERA_UNIFORM_SIZE),
                },
                count: None,
            }],
        });
        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("camera bg"),
            layout: &camera_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
        });

        let model_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("model uniform"),
            size: MODEL_STRIDE * MAX_OBJECTS as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let model_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("model bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: true,
                    min_binding_size: wgpu::BufferSize::new(
                        std::mem::size_of::<ModelUniformData>() as u64,
                    ),
                },
                count: None,
            }],
        });
        let model_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("model bg"),
            layout: &model_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &model_buffer,
                    offset: 0,
                    size: wgpu::BufferSize::new(std::mem::size_of::<ModelUniformData>() as u64),
                }),
            }],
        });

        let light_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("light uniform"),
            size: LIGHT_UNIFORM_SIZE,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let light_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("light bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(LIGHT_UNIFORM_SIZE),
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2Array,
                        multisampled: false,
                    },
                    count: None,
                },
                // 5/6/7/8 are the IBL maps and their sampler. A layout is
                // fixed at creation, so these entries exist even in scenes
                // that never load a skybox; the dummies bound in that case
                // are never sampled, because `ibl_enabled` is 0.
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::Cube,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 6,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::Cube,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 7,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 8,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                // 9 is the baked probe grid. Like the IBL entries above it is
                // present in every scene, including the many with no probe
                // volume; those upload `enabled: 0` and zeroed coefficients.
                // Binding 3 is the only other free slot and is taken by the
                // point shadow sampler, which no shader declares.
                wgpu::BindGroupLayoutEntry {
                    binding: 9,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(PROBE_UNIFORM_SIZE),
                    },
                    count: None,
                },
            ],
        });

        // Bound at 5 and 6 whenever no skybox is loaded. 1x1x6 and cleared to
        // black: the shader never samples it (`ibl_enabled` is 0 in that case),
        // so its only job is to satisfy the layout, and the smallest possible
        // texture is the one least likely to be mistaken for real lighting if
        // that flag ever went wrong.
        //
        // The BRDF LUT needs no equivalent: it is a pure function of the BRDF,
        // exists unconditionally on the surface, and so is bound at 7 in both
        // cases.
        let dummy_ibl_cube_texture = crate::ibl::create_cubemap(
            &device,
            "ibl dummy cube",
            1,
            1,
            crate::ibl::ENV_CUBE_FORMAT,
            wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        );
        // Explicitly zeroed rather than left to wgpu's lazy zero-init, so the
        // contents are a property of this code and not of a backend detail.
        // 8 bytes per Rgba16Float texel, one texel per face, six faces.
        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &dummy_ibl_cube_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &[0u8; 48],
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(8),
                rows_per_image: Some(1),
            },
            wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 6,
            },
        );
        let dummy_ibl_cube_view = crate::ibl::cube_view(&dummy_ibl_cube_texture);

        // Trilinear and clamped: the prefiltered cube's roughness lookup lands
        // between mips, and a nearest sampler would turn that continuous
        // roughness response into five visible steps.
        let ibl_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("ibl sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let (white_texture, sampler, texture_bgl, default_texture_bind_group) =
            Self::create_default_texture(&device, &queue);

        // --- shadow map ---
        let shadow_map_texture = crate::profiler::create_tracked_texture(
            &device,
            &wgpu::TextureDescriptor {
                label: Some("shadow map"),
                size: wgpu::Extent3d {
                    width: SHADOW_MAP_SIZE,
                    height: SHADOW_MAP_SIZE,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: DEPTH_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
        );
        let shadow_map_view =
            shadow_map_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let shadow_comparison_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("shadow comparison sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            compare: Some(wgpu::CompareFunction::LessEqual),
            ..Default::default()
        });

        // --- point light shadow maps (linear-distance cube arrays) ---
        let point_shadow_color_texture = crate::profiler::create_tracked_texture(
            &device,
            &wgpu::TextureDescriptor {
                label: Some("point shadow color array"),
                size: wgpu::Extent3d {
                    width: POINT_SHADOW_MAP_SIZE,
                    height: POINT_SHADOW_MAP_SIZE,
                    depth_or_array_layers: (MAX_POINT_LIGHTS * 6) as u32,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::R32Float,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
        );
        let point_shadow_color_full_view =
            point_shadow_color_texture.create_view(&wgpu::TextureViewDescriptor {
                label: Some("point shadow color array view (full)"),
                dimension: Some(wgpu::TextureViewDimension::D2Array),
                ..Default::default()
            });

        let point_shadow_depth_texture = crate::profiler::create_tracked_texture(
            &device,
            &wgpu::TextureDescriptor {
                label: Some("point shadow depth"),
                size: wgpu::Extent3d {
                    width: POINT_SHADOW_MAP_SIZE,
                    height: POINT_SHADOW_MAP_SIZE,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: DEPTH_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            },
        );
        let point_shadow_depth_view =
            point_shadow_depth_texture.create_view(&wgpu::TextureViewDescriptor::default());

        // R32Float is not natively filterable without the FLOAT32_FILTERABLE
        // device feature (not requested — required_features is empty()), so
        // this must be a non-filtering sampler with Nearest filter modes,
        // matching the directional shadow's own shadow_comparison_sampler
        // (also Nearest) for consistency.
        let point_shadow_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("point shadow sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let point_shadow_uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("point shadow uniform"),
            size: POINT_SHADOW_STRIDE * (MAX_POINT_LIGHTS * 6) as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let point_shadow_uniform_bgl =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("point shadow uniform bgl"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: true,
                        min_binding_size: wgpu::BufferSize::new(std::mem::size_of::<
                            PointShadowUniformData,
                        >() as u64),
                    },
                    count: None,
                }],
            });
        let point_shadow_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("point shadow uniform bg"),
            layout: &point_shadow_uniform_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &point_shadow_uniform_buffer,
                    offset: 0,
                    size: wgpu::BufferSize::new(
                        std::mem::size_of::<PointShadowUniformData>() as u64
                    ),
                }),
            }],
        });

        // Generated here, with the other one-time GPU resources, because the
        // split-sum BRDF table depends on nothing but the BRDF: no environment,
        // no scene, no camera. One integration at construction serves every
        // skybox this surface ever loads -- and every frame it loads none.
        // Both constructors reach this point, so an offscreen surface has the
        // LUT on exactly the same terms a windowed one does.
        let (brdf_lut_texture, brdf_lut_view) = crate::ibl::generate_brdf_lut(&device, &queue);

        let probe_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("probe uniform"),
            // COPY_SRC so a test can read back what the bake actually uploaded.
            // A bake's result is otherwise entirely invisible to the CPU -- it
            // goes straight from a texture readback into a uniform only the
            // shader ever sees -- and "the probes were baked" would then only
            // be assertable indirectly, through pixels.
            usage: wgpu::BufferUsages::UNIFORM
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            size: PROBE_UNIFORM_SIZE,
            mapped_at_creation: false,
        });
        // Explicitly zeroed rather than left to wgpu's lazy zero-init, for the
        // same reason the dummy IBL cube is: `enabled: 0` is exactly what keeps
        // every scene without a probe volume rendering as it did before probes
        // existed, and that should be a property of this code rather than of a
        // backend detail.
        queue.write_buffer(
            &probe_buffer,
            0,
            bytemuck::bytes_of(&<ProbeUniformData as bytemuck::Zeroable>::zeroed()),
        );

        // No skybox at construction, so the cube bindings get the dummy. Every
        // later rebuild goes through `rebuild_light_bind_group`, which binds
        // the same way from the same struct.
        let light_bind_group = create_light_bind_group(
            &device,
            &light_bgl,
            &LightBindings {
                light_buffer: &light_buffer,
                shadow_sampler: &shadow_comparison_sampler,
                shadow_map_view: &shadow_map_view,
                point_shadow_sampler: &point_shadow_sampler,
                point_shadow_view: &point_shadow_color_full_view,
                ibl_sampler: &ibl_sampler,
                irradiance_view: &dummy_ibl_cube_view,
                prefilter_view: &dummy_ibl_cube_view,
                brdf_lut_view: &brdf_lut_view,
                probe_buffer: &probe_buffer,
            },
        );

        let vertex_buffer_layout = wgpu::VertexBufferLayout {
            array_stride: VERTEX_STRIDE,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x3,
                    offset: 0,
                    shader_location: 0,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x3,
                    offset: 12,
                    shader_location: 1,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x3,
                    offset: 24,
                    shader_location: 2,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: 36,
                    shader_location: 3,
                },
            ],
        };

        let shadow_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("shadow shader"),
            source: wgpu::ShaderSource::Wgsl(SHADOW_WGSL.into()),
        });
        let shadow_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("shadow pipeline layout"),
                bind_group_layouts: &[&camera_bgl, &model_bgl],
                push_constant_ranges: &[],
            });
        let shadow_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("shadow pipeline"),
            layout: Some(&shadow_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shadow_shader,
                entry_point: "vs_shadow",
                buffers: std::slice::from_ref(&vertex_buffer_layout),
                compilation_options: Default::default(),
            },
            fragment: None,
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: Some(wgpu::Face::Back),
                front_face: wgpu::FrontFace::Ccw,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState {
                    constant: 2,
                    slope_scale: 2.0,
                    clamp: 0.0,
                },
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let point_shadow_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("point shadow shader"),
            source: wgpu::ShaderSource::Wgsl(POINT_SHADOW_WGSL.into()),
        });
        let point_shadow_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("point shadow pipeline layout"),
                bind_group_layouts: &[&point_shadow_uniform_bgl, &model_bgl],
                push_constant_ranges: &[],
            });
        let point_shadow_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("point shadow pipeline"),
                layout: Some(&point_shadow_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &point_shadow_shader,
                    entry_point: "vs_point_shadow",
                    buffers: std::slice::from_ref(&vertex_buffer_layout),
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &point_shadow_shader,
                    entry_point: "fs_point_shadow",
                    targets: &[Some(wgpu::ColorTargetState {
                        format: wgpu::TextureFormat::R32Float,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    cull_mode: Some(wgpu::Face::Back),
                    front_face: wgpu::FrontFace::Ccw,
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: DEPTH_FORMAT,
                    depth_write_enabled: true,
                    depth_compare: wgpu::CompareFunction::Less,
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                }),
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
                cache: None,
            });

        // --- light probe capture (one cube per probe, MAX_PROBES of them) ---
        let probe_capture_texture = crate::profiler::create_tracked_texture(
            &device,
            &wgpu::TextureDescriptor {
                label: Some("probe capture array"),
                size: wgpu::Extent3d {
                    width: PROBE_FACE_SIZE,
                    height: PROBE_FACE_SIZE,
                    depth_or_array_layers: (bsengine_core::MAX_PROBES * 6) as u32,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: PROBE_CAPTURE_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            },
        );
        let probe_capture_depth_texture = crate::profiler::create_tracked_texture(
            &device,
            &wgpu::TextureDescriptor {
                label: Some("probe capture depth"),
                size: wgpu::Extent3d {
                    width: PROBE_FACE_SIZE,
                    height: PROBE_FACE_SIZE,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: DEPTH_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            },
        );
        let probe_capture_depth_view =
            probe_capture_depth_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let probe_capture_uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("probe capture uniform"),
            size: PROBE_CAPTURE_STRIDE * (bsengine_core::MAX_PROBES * 6) as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let probe_capture_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("probe capture bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: true,
                    min_binding_size: wgpu::BufferSize::new(std::mem::size_of::<
                        ProbeCaptureUniformData,
                    >() as u64),
                },
                count: None,
            }],
        });
        let probe_capture_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("probe capture bg"),
            layout: &probe_capture_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &probe_capture_uniform_buffer,
                    offset: 0,
                    size: wgpu::BufferSize::new(
                        std::mem::size_of::<ProbeCaptureUniformData>() as u64
                    ),
                }),
            }],
        });

        let probe_capture_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("probe capture shader"),
            source: wgpu::ShaderSource::Wgsl(PROBE_CAPTURE_WGSL.into()),
        });
        // Groups 1..3 are the scene's own model / light / texture groups, so a
        // capture draw is the main pass's draw with a different camera. Group 2
        // carries the IBL bindings the capture shader deliberately never
        // declares; a bind group layout may expose more than an entry point
        // uses.
        let probe_capture_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("probe capture pipeline layout"),
                bind_group_layouts: &[&probe_capture_bgl, &model_bgl, &light_bgl, &texture_bgl],
                push_constant_ranges: &[],
            });
        let probe_capture_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("probe capture pipeline"),
                layout: Some(&probe_capture_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &probe_capture_shader,
                    entry_point: "vs_capture",
                    buffers: std::slice::from_ref(&vertex_buffer_layout),
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &probe_capture_shader,
                    entry_point: "fs_capture",
                    targets: &[Some(wgpu::ColorTargetState {
                        format: PROBE_CAPTURE_FORMAT,
                        blend: Some(wgpu::BlendState::REPLACE),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                // Same culling and depth rules as the opaque mesh pipeline: a
                // probe should see the scene the camera sees.
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    cull_mode: Some(wgpu::Face::Back),
                    front_face: wgpu::FrontFace::Ccw,
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: DEPTH_FORMAT,
                    depth_write_enabled: true,
                    depth_compare: wgpu::CompareFunction::Less,
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                }),
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
                cache: None,
            });

        let sky_tex_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("sky tex bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let probe_capture_sky_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("probe capture sky shader"),
            source: wgpu::ShaderSource::Wgsl(PROBE_CAPTURE_SKY_WGSL.into()),
        });
        let probe_capture_sky_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("probe capture sky pipeline layout"),
                bind_group_layouts: &[&probe_capture_bgl, &sky_tex_bgl],
                push_constant_ranges: &[],
            });
        let probe_capture_sky_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("probe capture sky pipeline"),
                layout: Some(&probe_capture_sky_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &probe_capture_sky_shader,
                    entry_point: "vs_capture_sky",
                    buffers: &[],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &probe_capture_sky_shader,
                    entry_point: "fs_capture_sky",
                    targets: &[Some(wgpu::ColorTargetState {
                        format: PROBE_CAPTURE_FORMAT,
                        blend: Some(wgpu::BlendState::REPLACE),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    cull_mode: None,
                    ..Default::default()
                },
                // Depth-test only, at NDC depth 1.0: the sky fills exactly the
                // texels no geometry claimed, and never overwrites one that did.
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: DEPTH_FORMAT,
                    depth_write_enabled: false,
                    depth_compare: wgpu::CompareFunction::LessEqual,
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                }),
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
                cache: None,
            });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("mesh shader"),
            source: wgpu::ShaderSource::Wgsl(MESH_WGSL.into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("mesh pipeline layout"),
            bind_group_layouts: &[&camera_bgl, &model_bgl, &light_bgl, &texture_bgl],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("mesh pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: std::slice::from_ref(&vertex_buffer_layout),
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: crate::post_process::HDR_FORMAT,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: Some(wgpu::Face::Back),
                front_face: wgpu::FrontFace::Ccw,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // Same shader, same layout, same vertex format as the opaque pipeline.
        // Exactly two things differ, and both are what drawing transparent
        // geometry means:
        //
        //   - alpha blending, so what is already in the buffer shows through
        //   - depth writes off, so one transparent surface does not stop the
        //     one behind it from being drawn
        //
        // The depth *test* stays on: transparent geometry behind an opaque
        // wall is still hidden by it.
        let transparent_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("transparent mesh pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: std::slice::from_ref(&vertex_buffer_layout),
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: crate::post_process::HDR_FORMAT,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: Some(wgpu::Face::Back),
                front_face: wgpu::FrontFace::Ccw,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let terrain_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("terrain texture bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let terrain_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("terrain shader"),
            source: wgpu::ShaderSource::Wgsl(TERRAIN_WGSL.into()),
        });
        let terrain_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("terrain pipeline layout"),
                bind_group_layouts: &[&camera_bgl, &model_bgl, &light_bgl, &terrain_bgl],
                push_constant_ranges: &[],
            });
        let terrain_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("terrain pipeline"),
            layout: Some(&terrain_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &terrain_shader,
                entry_point: "vs_main",
                buffers: std::slice::from_ref(&vertex_buffer_layout),
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &terrain_shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: crate::post_process::HDR_FORMAT,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: Some(wgpu::Face::Back),
                front_face: wgpu::FrontFace::Ccw,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let particles = crate::particles::ParticleRenderer::new(&device, &camera_bgl);

        let egui_ctx = egui::Context::default();
        crate::theme::apply(&egui_ctx);
        let egui_renderer = egui_wgpu::Renderer::new(&device, format, None, 1, false);

        // The froxel injection pass shares the scene's shadow maps and light
        // uniform rather than owning copies -- see `FroxelShadowBindings`. All
        // four outlive the post-process state and none is recreated, so the
        // bind group it builds from them holds for the surface's whole life.
        let post_process = crate::post_process::PostProcessState::new(
            &device,
            width,
            height,
            &depth_view,
            format,
            &crate::post_process::FroxelShadowBindings {
                shadow_map_view: &shadow_map_view,
                shadow_sampler: &shadow_comparison_sampler,
                point_shadow_view: &point_shadow_color_full_view,
                light_buffer: &light_buffer,
            },
        );

        Ok(Self {
            output,
            device,
            queue,
            pipeline,
            transparent_pipeline,
            particles,
            depth_texture,
            depth_view,
            camera_buffer,
            camera_bind_group,
            model_buffer,
            model_bind_group,
            light_buffer,
            light_bind_group,
            light_bgl,
            _white_texture: white_texture,
            _sampler: sampler,
            default_texture_bind_group,
            shadow_pipeline,
            _shadow_map_texture: shadow_map_texture,
            shadow_map_view,
            shadow_comparison_sampler,
            point_shadow_pipeline,
            _point_shadow_color_texture: point_shadow_color_texture,
            _point_shadow_depth_texture: point_shadow_depth_texture,
            point_shadow_color_full_view,
            point_shadow_depth_view,
            point_shadow_sampler,
            point_shadow_uniform_buffer,
            point_shadow_bind_group,
            probe_capture_texture,
            _probe_capture_depth_texture: probe_capture_depth_texture,
            probe_capture_depth_view,
            probe_capture_pipeline,
            probe_capture_sky_pipeline,
            probe_capture_uniform_buffer,
            probe_capture_bind_group,
            probe_buffer,
            // Nothing baked yet, and `probe_buffer` was just zeroed to match.
            baked_probe_volume: None,
            sky_tex_bgl,
            egui_ctx,
            egui_renderer,
            skybox: None,
            loaded_skybox_path: None,
            // No skybox yet, so there is no environment to have convolved.
            ibl: None,
            brdf_lut_view,
            _brdf_lut_texture: brdf_lut_texture,
            ibl_sampler,
            dummy_ibl_cube_view,
            _dummy_ibl_cube_texture: dummy_ibl_cube_texture,
            pipeline_layout,
            terrain_pipeline,
            terrain_bgl,
            custom_pipelines: std::collections::HashMap::new(),
            post_process,
            // No frame has been rendered yet, so there is no previous
            // view-projection. It is never read before the first frame
            // stores one: `PostProcessState` starts with `history_valid`
            // false, and the resolve does not reproject without history.
            prev_unjittered_view_proj: Mat4::IDENTITY,
            start_time: std::time::Instant::now(),
            dock_state: None,
            last_saved_layout_json: None,
            fast_render,
            timestamp_supported,
            timestamp_query_set,
            timestamp_resolve_buffer,
            timestamp_readback_buffer,
            frame_stats_history: std::sync::Arc::new(std::sync::Mutex::new(
                std::collections::VecDeque::with_capacity(
                    crate::profiler::FRAME_STATS_HISTORY_CAPACITY,
                ),
            )),
        })
    }

    fn create_depth_texture(
        device: &wgpu::Device,
        width: u32,
        height: u32,
    ) -> (crate::profiler::TrackedTexture, wgpu::TextureView) {
        let texture = crate::profiler::create_tracked_texture(
            device,
            &wgpu::TextureDescriptor {
                label: Some("depth texture"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: DEPTH_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
        );
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        (texture, view)
    }

    fn create_default_texture(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> (
        crate::profiler::TrackedTexture,
        wgpu::Sampler,
        wgpu::BindGroupLayout,
        wgpu::BindGroup,
    ) {
        let texture = crate::profiler::create_tracked_texture(
            device,
            &wgpu::TextureDescriptor {
                label: Some("white texture"),
                size: wgpu::Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            },
        );
        queue.write_texture(
            texture.as_image_copy(),
            &[255u8, 255, 255, 255],
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(4),
                rows_per_image: None,
            },
            wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
        );
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("default sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("texture bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("default texture bg"),
            layout: &bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });
        (texture, sampler, bgl, bind_group)
    }

    /// Uploads already-decoded RGBA8 pixel data as the active skybox
    /// texture, rebuilding the sampler/bind groups/pipeline around it.
    pub fn set_skybox_from_rgba(&mut self, width: u32, height: u32, rgba: &[u8]) {
        let texture = crate::profiler::create_tracked_texture(
            &self.device,
            &wgpu::TextureDescriptor {
                label: Some("skybox texture"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            },
        );
        self.queue.write_texture(
            texture.as_image_copy(),
            rgba,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(4 * width),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        let tex_view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = self.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("skybox sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let uniform_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("skybox uniform"),
            size: SKY_UNIFORM_SIZE,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let sky_uniform_bgl =
            self.device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("sky uniform bgl"),
                    entries: &[wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: wgpu::BufferSize::new(SKY_UNIFORM_SIZE),
                        },
                        count: None,
                    }],
                });
        let uniform_bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("sky uniform bg"),
            layout: &sky_uniform_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        // `self.sky_tex_bgl`, not a fresh layout: `probe_capture_sky_pipeline`
        // was built against that object at construction and binds the very
        // `texture_bg` created here.
        let sky_tex_bgl = &self.sky_tex_bgl;
        let texture_bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("sky tex bg"),
            layout: sky_tex_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&tex_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        let shader = self
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("skybox shader"),
                source: wgpu::ShaderSource::Wgsl(SKYBOX_WGSL.into()),
            });
        let pipeline_layout = self
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("skybox pipeline layout"),
                bind_group_layouts: &[&sky_uniform_bgl, sky_tex_bgl],
                push_constant_ranges: &[],
            });
        let pipeline = self
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("skybox pipeline"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: "vs_sky",
                    buffers: &[],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: "fs_sky",
                    targets: &[Some(wgpu::ColorTargetState {
                        // The skybox pass draws into the HDR buffer, not the
                        // final output. Building this pipeline for the output
                        // format made set_pipeline fail validation the moment
                        // any project actually set a skybox.
                        format: crate::post_process::HDR_FORMAT,
                        blend: Some(wgpu::BlendState::REPLACE),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    cull_mode: None,
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: DEPTH_FORMAT,
                    depth_write_enabled: false,
                    depth_compare: wgpu::CompareFunction::LessEqual,
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                }),
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
                cache: None,
            });

        // The environment changed, so everything convolved from the old one is
        // stale: rebuild the irradiance and prefiltered maps from the texture
        // and sampler this very skybox is about to be drawn with, so what
        // materials reflect and what the camera sees are the same image.
        //
        // Here rather than lazily at first use because it is a heavy one-off
        // (42 render passes) that belongs to the load, not to a frame.
        self.ibl = Some(crate::ibl::IblMaps::generate(
            &self.device,
            &self.queue,
            &tex_view,
            &sampler,
        ));
        // The group-2 bind group still points at the dummy cube (or the
        // previous skybox's maps); a bind group cannot be edited in place.
        self.rebuild_light_bind_group();

        self.skybox = Some(SkyboxState {
            pipeline,
            uniform_buffer,
            uniform_bg,
            texture_bg,
            _texture: texture,
            _sampler: sampler,
        });
    }

    /// Unloads the current skybox, if any, reverting to no skybox rendering.
    pub fn clear_skybox(&mut self) {
        self.skybox = None;
        self.loaded_skybox_path = None;
        // The IBL maps are convolutions of the skybox that just went away;
        // keeping them would light the scene with an environment no longer
        // being rendered.
        self.ibl = None;
        // Back to the dummy cube. Dropping the maps without this would leave
        // the bind group holding views into freed textures.
        self.rebuild_light_bind_group();
    }

    /// Rebuilds the group-2 bind group so bindings 5 and 6 point at whatever
    /// [`Self::ibl`] currently is: the real maps, or the dummy cube when there
    /// is no skybox.
    ///
    /// Must be called after *every* assignment to `self.ibl`. The uniform's
    /// `ibl_enabled` flag is written from `self.ibl` each frame, so a stale
    /// bind group here would mean the shader sampling one skybox's maps while
    /// the flag describes another's.
    fn rebuild_light_bind_group(&mut self) {
        let (irradiance_view, prefilter_view) = match &self.ibl {
            Some(maps) => (&maps.irradiance_view, &maps.prefilter_view),
            None => (&self.dummy_ibl_cube_view, &self.dummy_ibl_cube_view),
        };
        let bind_group = create_light_bind_group(
            &self.device,
            &self.light_bgl,
            &LightBindings {
                light_buffer: &self.light_buffer,
                shadow_sampler: &self.shadow_comparison_sampler,
                shadow_map_view: &self.shadow_map_view,
                point_shadow_sampler: &self.point_shadow_sampler,
                point_shadow_view: &self.point_shadow_color_full_view,
                ibl_sampler: &self.ibl_sampler,
                irradiance_view,
                prefilter_view,
                brdf_lut_view: &self.brdf_lut_view,
                probe_buffer: &self.probe_buffer,
            },
        );
        self.light_bind_group = bind_group;
    }

    /// Whether a skybox is currently loaded and will be rendered.
    pub fn has_skybox(&self) -> bool {
        self.skybox.is_some()
    }

    /// Whether image-based lighting maps are currently available -- true
    /// exactly when a skybox is loaded, since they are generated from it.
    pub fn has_ibl(&self) -> bool {
        self.ibl.is_some()
    }

    /// Path of the currently loaded skybox texture, if any.
    pub fn loaded_skybox_path(&self) -> Option<&str> {
        self.loaded_skybox_path.as_deref()
    }

    /// Records which path the currently-uploaded skybox came from, for callers
    /// that upload via [`WgpuSurface::set_skybox_from_rgba`] and do their own
    /// file loading.
    pub fn set_loaded_skybox_path(&mut self, path: &str) {
        self.loaded_skybox_path = Some(path.to_string());
    }

    /// Shared handle to the rolling frame-stats history, for `ProfilerPanel`
    /// and headless/MCP queries.
    pub fn frame_stats_history(
        &self,
    ) -> std::sync::Arc<std::sync::Mutex<std::collections::VecDeque<crate::profiler::FrameStats>>>
    {
        self.frame_stats_history.clone()
    }

    /// The most recently completed frame's stats, if any frame has rendered yet.
    pub fn latest_frame_stats(&self) -> Option<crate::profiler::FrameStats> {
        self.frame_stats_history.lock().unwrap().back().cloned()
    }

    /// Whether this device requests GPU timestamp queries for pass timing --
    /// i.e. whether the adapter reported `wgpu::Features::TIMESTAMP_QUERY`
    /// support. Exposed mainly so tests can branch on the machine's actual
    /// capability rather than assume one outcome.
    pub fn gpu_timestamps_supported(&self) -> bool {
        self.timestamp_supported
    }

    /// Builds the `timestamp_writes` for a single self-contained named pass
    /// (one `begin_render_pass` call whose full duration is what's being
    /// measured), and records its index/name for [`Self::read_gpu_pass_times`]
    /// to consume after the frame is submitted. Returns `None` -- meaning
    /// "don't time this pass" -- whenever GPU timestamps aren't supported, or
    /// whenever a frame has already used every slot in the query set (an
    /// extremely defensive bound; today's frame uses at most 6 of 16).
    fn next_timed_pass(
        &self,
        name: &'static str,
        pass_index: &mut u32,
        pass_names: &mut Vec<&'static str>,
    ) -> Option<wgpu::RenderPassTimestampWrites<'_>> {
        if !self.timestamp_supported || *pass_index >= MAX_TIMED_PASSES {
            return None;
        }
        let idx = *pass_index;
        *pass_index += 1;
        pass_names.push(name);
        Some(wgpu::RenderPassTimestampWrites {
            query_set: self.timestamp_query_set.as_ref().unwrap(),
            beginning_of_pass_write_index: Some(idx * 2),
            end_of_pass_write_index: Some(idx * 2 + 1),
        })
    }

    /// Same idea as [`Self::next_timed_pass`], but for the point-light shadow
    /// loop, which issues a *variable* number of `begin_render_pass` calls
    /// per frame (one per cube face per active point light -- up to 48 with
    /// 8 lights). Rather than spend a query-set slot per face, this brackets
    /// the whole loop as a single `"point_shadow"` pass: the caller reserves
    /// one `pass_index` up front (see the call site in `render_frame`) and
    /// passes it back in on the loop's first and last iteration so the
    /// begin/end timestamps land on the actual first/last GPU work.
    fn point_shadow_timestamp_writes(
        &self,
        pass_index: Option<u32>,
        is_first: bool,
        is_last: bool,
    ) -> Option<wgpu::RenderPassTimestampWrites<'_>> {
        let idx = pass_index?;
        if !is_first && !is_last {
            return None;
        }
        Some(wgpu::RenderPassTimestampWrites {
            query_set: self.timestamp_query_set.as_ref().unwrap(),
            beginning_of_pass_write_index: is_first.then_some(idx * 2),
            end_of_pass_write_index: is_last.then_some(idx * 2 + 1),
        })
    }

    /// Resolves and reads back this frame's GPU timestamps, converting raw
    /// tick pairs into `PassTiming`s. Must be called after the frame's
    /// `resolve_query_set` + `copy_buffer_to_buffer` have been submitted to
    /// the queue -- this blocks (via `map_async` + `device.poll(Wait)`,
    /// the same pattern `output::read_pixels` uses) until that GPU work
    /// completes, so it is only ever called when `pass_count > 0`.
    ///
    /// Degrades to an empty `Vec` rather than panicking if the mapping ever
    /// fails -- this is a profiling-only path and must never be the reason a
    /// frame errors out.
    fn read_gpu_pass_times(
        &self,
        pass_count: u32,
        pass_names: &[&'static str],
    ) -> Vec<crate::profiler::PassTiming> {
        let Some(readback_buffer) = &self.timestamp_readback_buffer else {
            return Vec::new();
        };
        let ticks_len = pass_count as usize * 2;
        let bytes_len = (ticks_len * wgpu::QUERY_SIZE as usize) as u64;

        let slice = readback_buffer.slice(0..bytes_len);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        self.device.poll(wgpu::Maintain::Wait);
        match rx.recv() {
            Ok(Ok(())) => {}
            _ => return Vec::new(),
        }

        let timings = {
            let mapped = slice.get_mapped_range();
            let ticks: &[u64] = bytemuck::cast_slice(&mapped);
            let period = self.queue.get_timestamp_period();
            pass_names
                .iter()
                .enumerate()
                .map(|(i, name)| {
                    let begin = ticks[i * 2];
                    let end = ticks[i * 2 + 1];
                    let duration_ms = end.saturating_sub(begin) as f32 * period / 1_000_000.0;
                    crate::profiler::PassTiming {
                        name: (*name).to_string(),
                        duration_ms,
                    }
                })
                .collect()
        };
        readback_buffer.unmap();
        timings
    }

    /// Renders the scene once per probe per cube face into
    /// `probe_capture_texture`, ready for [`Self::project_captures_to_sh`].
    ///
    /// Structurally the point-light shadow loop with a different target: one
    /// single-layer `TextureView` per `(probe, face)` at layer
    /// `probe * 6 + face`, a render pass into it, the capture pipeline, the
    /// per-face uniform by dynamic offset, then the same `draw_calls` walk with
    /// the model bind group's own dynamic offset.
    ///
    /// **The caller must already have uploaded this frame's `light_buffer` and
    /// `model_buffer`** -- `render_frame` writes both before it reaches any
    /// pass, and a capture reuses them rather than duplicating that work.
    ///
    /// Terrain chunks are not captured. They take a different group-3 layout
    /// than the capture pipeline is built for, and terrain is ground: it
    /// receives bounced light far more than it contributes it.
    ///
    /// Probes past [`bsengine_core::MAX_PROBES`] are ignored, matching the
    /// uniform array's fixed size.
    fn capture_probes(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        light_view_proj: Mat4,
        draw_calls: &[(u64, Mat4, Option<u64>, MaterialParams, Option<String>)],
        registry: &GpuMeshRegistry,
        tex_registry: Option<&crate::texture::GpuTextureRegistry>,
        positions: &[Vec3],
    ) {
        let probe_count = positions.len().min(bsengine_core::MAX_PROBES);
        if probe_count == 0 {
            return;
        }

        // Every face's uniform is written up front, before a single pass is
        // encoded, because `queue.write_buffer` is ordered against *submits*,
        // not against passes: rewriting one slot between passes of this
        // encoder would give every face the last value written. The
        // point-shadow loop stages its faces the same way and for the same
        // reason.
        let sky_vp_invs = probe_face_sky_vp_invs(PROBE_CAPTURE_RANGE);
        for (probe_idx, position) in positions.iter().take(probe_count).enumerate() {
            let view_projs = point_light_face_view_projs(*position, PROBE_CAPTURE_RANGE);
            for (face, vp) in view_projs.iter().enumerate() {
                let slot = probe_idx * 6 + face;
                let data = ProbeCaptureUniformData {
                    view_proj: vp.to_cols_array_2d(),
                    light_view_proj: light_view_proj.to_cols_array_2d(),
                    inv_view_proj: sky_vp_invs[face].to_cols_array_2d(),
                    probe_pos: position.to_array(),
                    _pad: 0.0,
                };
                self.queue.write_buffer(
                    &self.probe_capture_uniform_buffer,
                    slot as u64 * PROBE_CAPTURE_STRIDE,
                    bytemuck::cast_slice(&[data]),
                );
            }
        }

        for probe_idx in 0..probe_count {
            for face in 0..6usize {
                let slot = probe_idx * 6 + face;
                let layer_view =
                    self.probe_capture_texture
                        .create_view(&wgpu::TextureViewDescriptor {
                            label: Some("probe capture layer view"),
                            dimension: Some(wgpu::TextureViewDimension::D2),
                            base_array_layer: slot as u32,
                            array_layer_count: Some(1),
                            ..Default::default()
                        });
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("probe capture pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &layer_view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            // The same background the main pass clears to, so a
                            // probe in an empty scene records the scene's own
                            // backdrop rather than black. Overwritten by the sky
                            // pipeline below wherever a skybox is loaded.
                            load: wgpu::LoadOp::Clear(wgpu::Color {
                                r: 0.08,
                                g: 0.08,
                                b: 0.08,
                                a: 1.0,
                            }),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: &self.probe_capture_depth_view,
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Clear(1.0),
                            store: wgpu::StoreOp::Store,
                        }),
                        stencil_ops: None,
                    }),
                    // A bake is `probes * 6` passes -- up to 192 -- which no
                    // query set can hold a slot pair for, and it happens once
                    // rather than every frame, so it is deliberately untimed.
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });
                let uniform_offset = (slot as u64 * PROBE_CAPTURE_STRIDE) as u32;
                pass.set_pipeline(&self.probe_capture_pipeline);
                pass.set_bind_group(0, &self.probe_capture_bind_group, &[uniform_offset]);
                pass.set_bind_group(2, &self.light_bind_group, &[]);
                for (i, (mesh_id, _, tex_id, _, _)) in draw_calls.iter().enumerate() {
                    if i >= MAX_OBJECTS {
                        break;
                    }
                    let Some(mesh) = registry.get(*mesh_id) else {
                        continue;
                    };
                    // Custom-shader draws are captured with the standard
                    // capture shader: a custom pipeline binds group 0 as the
                    // camera uniform, which this pass does not have, and a
                    // custom shader's output is not decomposable into direct
                    // light anyway.
                    let tex_bg = tex_id
                        .and_then(|id| tex_registry.and_then(|r| r.get_bind_group(id)))
                        .unwrap_or(&self.default_texture_bind_group);
                    let offset = (i as u64 * MODEL_STRIDE) as u32;
                    pass.set_bind_group(1, &self.model_bind_group, &[offset]);
                    pass.set_bind_group(3, tex_bg, &[]);
                    pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                    pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                    pass.draw_indexed(0..mesh.index_count, 0, 0..1);
                }
                if let Some(sky) = &self.skybox {
                    pass.set_pipeline(&self.probe_capture_sky_pipeline);
                    pass.set_bind_group(0, &self.probe_capture_bind_group, &[uniform_offset]);
                    pass.set_bind_group(1, &sky.texture_bg, &[]);
                    pass.draw(0..3, 0..1);
                }
            }
        }
    }

    /// Reads `probe_capture_texture` back and projects each probe's six faces
    /// onto L2 spherical harmonics.
    ///
    /// Must run after the encoder [`Self::capture_probes`] wrote into has been
    /// submitted: this copies the texture on its own encoder and blocks on the
    /// map, so anything still sitting in an unsubmitted encoder would be
    /// missed.
    ///
    /// Every texel contributes `radiance * basis(dir) * solid_angle`, with
    /// `dir` from [`probe_face_texel_direction`] (the same cube convention the
    /// capture matrices were built from) and `solid_angle` from
    /// [`probe_face_texel_solid_angle`] (the real projected solid angle, not a
    /// flat share of the sphere).
    fn project_captures_to_sh(&self, probe_count: usize) -> Vec<crate::sh::ShL2> {
        let probes = probe_count.min(bsengine_core::MAX_PROBES);
        if probes == 0 {
            return Vec::new();
        }
        // Rgba16Float.
        const BYTES_PER_TEXEL: u32 = 8;
        let layers = (probes * 6) as u32;
        let unpadded_bytes_per_row = PROBE_FACE_SIZE * BYTES_PER_TEXEL;
        // `copy_texture_to_buffer` requires every row to start on a 256-byte
        // boundary. A 16-texel row is 128 bytes, so each row really is padded
        // here -- reading the buffer as if it were tightly packed would
        // interleave two rows per face and silently scramble every direction.
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let padded_bytes_per_row = unpadded_bytes_per_row.div_ceil(align) * align;
        let layer_stride = (padded_bytes_per_row * PROBE_FACE_SIZE) as u64;

        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("probe capture readback"),
            size: layer_stride * layers as u64,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("probe capture readback encoder"),
            });
        encoder.copy_texture_to_buffer(
            wgpu::ImageCopyTexture {
                texture: &self.probe_capture_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::ImageCopyBuffer {
                buffer: &buffer,
                layout: wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_bytes_per_row),
                    rows_per_image: Some(PROBE_FACE_SIZE),
                },
            },
            wgpu::Extent3d {
                width: PROBE_FACE_SIZE,
                height: PROBE_FACE_SIZE,
                depth_or_array_layers: layers,
            },
        );
        self.queue.submit(std::iter::once(encoder.finish()));

        let slice = buffer.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        self.device.poll(wgpu::Maintain::Wait);
        rx.recv()
            .expect("map_async never reported a result")
            .expect("mapping the probe capture readback buffer failed");

        let mapped = slice.get_mapped_range();
        let mut out = vec![crate::sh::ShL2::default(); probes];
        for (probe_idx, sh) in out.iter_mut().enumerate() {
            for face in 0..6usize {
                let layer_start = (probe_idx * 6 + face) as u64 * layer_stride;
                for y in 0..PROBE_FACE_SIZE {
                    let row_start = (layer_start + (y * padded_bytes_per_row) as u64) as usize;
                    for x in 0..PROBE_FACE_SIZE {
                        let t = &mapped[row_start + (x * BYTES_PER_TEXEL) as usize..];
                        let radiance = Vec3::new(
                            f16_bits_to_f32(u16::from_le_bytes([t[0], t[1]])),
                            f16_bits_to_f32(u16::from_le_bytes([t[2], t[3]])),
                            f16_bits_to_f32(u16::from_le_bytes([t[4], t[5]])),
                        );
                        sh.accumulate(
                            probe_face_texel_direction(face, x, y),
                            radiance,
                            probe_face_texel_solid_angle(x, y),
                        );
                    }
                }
            }
        }
        drop(mapped);
        buffer.unmap();
        out
    }

    /// Bakes `volume`'s probe grid into [`Self::probe_buffer`], or uploads a
    /// disabled grid when `volume` is `None`.
    ///
    /// Runs on its own command encoder, submitted before returning, because
    /// [`Self::project_captures_to_sh`] blocks on a buffer map and so cannot
    /// see anything still sitting in the frame's unsubmitted encoder.
    ///
    /// Expensive by design -- up to `MAX_PROBES * 6` render passes plus a
    /// synchronous readback -- which is why the caller runs it once rather than
    /// per frame.
    fn bake_probes(
        &self,
        light_view_proj: Mat4,
        draw_calls: &[(u64, Mat4, Option<u64>, MaterialParams, Option<String>)],
        registry: &GpuMeshRegistry,
        tex_registry: Option<&crate::texture::GpuTextureRegistry>,
        volume: Option<ProbeVolumeParams>,
    ) {
        let mut data = <ProbeUniformData as bytemuck::Zeroable>::zeroed();
        if let Some(params) = volume {
            let positions = probe_positions(&params);
            let mut encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("probe bake encoder"),
                });
            self.capture_probes(
                &mut encoder,
                light_view_proj,
                draw_calls,
                registry,
                tex_registry,
                &positions,
            );
            self.queue.submit(std::iter::once(encoder.finish()));

            for (probe, sh) in self
                .project_captures_to_sh(positions.len())
                .iter()
                .enumerate()
            {
                for (slot, coeff) in sh.coeffs.iter().enumerate() {
                    data.coeffs[probe][slot] = [coeff.x, coeff.y, coeff.z, 0.0];
                }
            }
            data.origin = params.origin.to_array();
            data.enabled = 1;
            data.extent = params.extent.to_array();
            data.resolution = params.resolution;
        }
        // Written even in the `None` case: the binding exists in every frame,
        // so "no probes" has to be expressed as `enabled: 0` with zeroed
        // coefficients rather than as a skipped upload -- otherwise removing a
        // volume would leave the previous bake lighting the scene forever.
        self.queue
            .write_buffer(&self.probe_buffer, 0, bytemuck::bytes_of(&data));
    }

    /// Reads [`Self::probe_buffer`] back, so a test can assert on what the
    /// shader is actually about to sample rather than on a CPU-side mirror of
    /// it. Blocks on a buffer map; not for use in a frame.
    #[cfg(test)]
    fn read_probe_uniform(&self) -> ProbeUniformData {
        let staging = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("probe uniform readback"),
            size: PROBE_UNIFORM_SIZE,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("probe uniform readback encoder"),
            });
        encoder.copy_buffer_to_buffer(&self.probe_buffer, 0, &staging, 0, PROBE_UNIFORM_SIZE);
        self.queue.submit(std::iter::once(encoder.finish()));

        let slice = staging.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        self.device.poll(wgpu::Maintain::Wait);
        rx.recv()
            .expect("map_async never reported a result")
            .expect("mapping the probe uniform readback buffer failed");
        let data = *bytemuck::from_bytes::<ProbeUniformData>(&slice.get_mapped_range());
        staging.unmap();
        data
    }

    /// Renders one full frame: shadow map, main scene pass, post-processing,
    /// skybox, and the game/editor UI overlay, then presents the swapchain.
    /// Returns the ids of any `UiWidget::Button`s clicked this frame.
    #[allow(clippy::too_many_arguments)] // one frame's worth of render inputs; splitting into a struct is a larger refactor
    pub fn render_frame(
        &mut self,
        view_proj: Mat4,
        cam_pos: Vec3,
        light_view_proj: Mat4,
        sky_vp_inv: Option<Mat4>,
        draw_calls: &[(u64, Mat4, Option<u64>, MaterialParams, Option<String>)],
        terrain_draw_calls: &[(u64, Mat4, [u64; 4], u64)],
        occluded_count: u32,
        registry: &GpuMeshRegistry,
        light: LightData,
        tex_registry: Option<&crate::texture::GpuTextureRegistry>,
        hud_texts: &std::collections::HashMap<String, String>,
        ui_state: &bsengine_core::UiState,
        cursor_x: f32,
        cursor_y: f32,
        left_just_pressed: bool,
        left_just_released: bool,
        cam_proj: Mat4,
        bloom: Option<bsengine_core::Bloom>,
        tone_map: Option<bsengine_core::ToneMap>,
        ambient_occlusion: Option<bsengine_core::AmbientOcclusion>,
        mut inspector: Option<&mut bsengine_core::InspectorState>,
        key_events: &[bsengine_input::KeyInput],
        ctrl_held: bool,
        shift_held: bool,
        alt_held: bool,
        editor_panels: Option<&bsengine_core::EditorPanelRegistry>,
        type_registry: Option<&bevy_ecs::reflect::AppTypeRegistry>,
        elapsed_seconds: f32,
        particles: &[crate::particles::ParticleBatch],
        taa: Option<bsengine_core::Taa>,
        jitter_clip: (f32, f32),
        // The caller's frame counter -- the same one `jitter_clip` above was
        // derived from. Passed alongside that offset rather than instead of it
        // because the fog needs it on the frames the offset is `(0.0, 0.0)`:
        // the froxel depth dither runs whether or not TAA does, and a frozen
        // index would make every frame pick the same offset, which is a fixed
        // pattern rather than a dither.
        frame_index: u32,
        unjittered_view_proj: Mat4,
        light_probes: Option<ProbeVolumeParams>,
        fog: Option<bsengine_core::VolumetricFog>,
    ) -> Result<std::collections::HashSet<String>, String> {
        // Wall-clock CPU time for this call, for `FrameStats::cpu_frame_time_ms`.
        let frame_start = std::time::Instant::now();

        // The sub-pixel TAA jitter belongs to *rasterization only*, so it is
        // applied here rather than by the caller, and only to the matrix the
        // vertex shader uses. `unjittered_view_proj` stays untouched and is
        // what the reprojection matrices uploaded below are built from --
        // reprojecting through a jittered matrix would chase the jitter
        // instead of the camera and never converge. See `jittered_view_proj`
        // for why the offset goes where it does.
        let raster_view_proj = jittered_view_proj(view_proj, cam_proj, jitter_clip);

        let camera_data = CameraUniformData {
            view_proj: raster_view_proj.to_cols_array_2d(),
            light_view_proj: light_view_proj.to_cols_array_2d(),
            cam_pos: cam_pos.to_array(),
            time: elapsed_seconds,
        };
        self.queue
            .write_buffer(&self.camera_buffer, 0, bytemuck::cast_slice(&[camera_data]));

        let mut point_lights_gpu = [PointLightGpu {
            position: [0.0; 3],
            _pad0: 0.0,
            color: [0.0; 3],
            intensity: 0.0,
            range: 0.0,
            _pad1: 0.0,
            _pad2: 0.0,
            _pad3: 0.0,
        }; 8];
        let num_point_lights = light.point_lights.len().min(MAX_POINT_LIGHTS) as u32;
        for (i, pl) in light.point_lights.iter().enumerate().take(MAX_POINT_LIGHTS) {
            point_lights_gpu[i] = PointLightGpu {
                position: pl.position.to_array(),
                _pad0: 0.0,
                color: pl.color.to_array(),
                intensity: pl.intensity,
                range: pl.range,
                _pad1: 0.0,
                _pad2: 0.0,
                _pad3: 0.0,
            };
        }
        let mut spot_lights_gpu = [SpotLightGpu {
            position: [0.0; 3],
            _pad0: 0.0,
            direction: [0.0, -1.0, 0.0],
            inner_cos: 0.0,
            color: [0.0; 3],
            outer_cos: 0.0,
            intensity: 0.0,
            range: 0.0,
            _pad1: 0.0,
            _pad2: 0.0,
        }; 8];
        let num_spot_lights = light.spot_lights.len().min(MAX_SPOT_LIGHTS) as u32;
        for (i, sl) in light.spot_lights.iter().enumerate().take(MAX_SPOT_LIGHTS) {
            spot_lights_gpu[i] = SpotLightGpu {
                position: sl.position.to_array(),
                _pad0: 0.0,
                direction: sl.direction.normalize().to_array(),
                inner_cos: sl.inner_angle.cos(),
                color: sl.color.to_array(),
                outer_cos: sl.outer_angle.cos(),
                intensity: sl.intensity,
                range: sl.range,
                _pad1: 0.0,
                _pad2: 0.0,
            };
        }
        let light_data = LightUniformData {
            direction: light.direction.normalize().to_array(),
            _pad0: 0.0,
            color: light.color.to_array(),
            _pad1: 0.0,
            ambient: light.ambient.to_array(),
            num_point_lights,
            point_lights: point_lights_gpu,
            num_spot_lights,
            // The maps and the flag come from the same place, so the shader
            // can never sample dummy cubemaps as if they were an environment.
            ibl_enabled: u32::from(self.ibl.is_some()),
            ibl_max_mip: (crate::ibl::PREFILTER_MIP_LEVELS - 1) as f32,
            _pad4: 0.0,
            spot_lights: spot_lights_gpu,
        };
        self.queue
            .write_buffer(&self.light_buffer, 0, bytemuck::cast_slice(&[light_data]));

        for (i, (_, model, _, mat, _)) in draw_calls.iter().enumerate() {
            if i >= MAX_OBJECTS {
                break;
            }
            let data = ModelUniformData {
                model: model.to_cols_array_2d(),
                metallic: mat.metallic,
                roughness: mat.roughness,
                _pad0: 0.0,
                _pad1: 0.0,
                emissive: mat.emissive.to_array(),
                _pad2: 0.0,
                base_color: mat.base_color.to_array(),
                opacity: mat.opacity,
            };
            self.queue.write_buffer(
                &self.model_buffer,
                i as u64 * MODEL_STRIDE,
                bytemuck::cast_slice(&[data]),
            );
        }

        // --- light probe bake ---
        // Here, after the light and model uniforms are written and before the
        // frame's own encoder exists: `capture_probes` reuses both of those
        // buffers, and `bake_probes` submits its own encoder and blocks on a
        // readback, which is cleaner to do outside the frame's encoder than
        // interleaved with it.
        //
        // `fast_render` never bakes. 192 render passes plus a synchronous map
        // is precisely the expensive work that mode exists to skip, and the
        // neutral result -- an `enabled: 0` grid -- is what the shadow pass's
        // "clear to nothing occludes anything" is for shadows. Folding it into
        // the comparison rather than guarding the call keeps a fast_render
        // surface at zero bakes rather than one per frame.
        let volume_to_bake = if self.fast_render { None } else { light_probes };
        // A bake happens only when the volume appears or its own parameters
        // change. Note this tracks the *volume*, not the scene inside it:
        // moving a wall after load does not re-bake, and the floor keeps the
        // colour that wall used to bleed onto it. That is the accepted cost of
        // baking once, which is what item 48 asked for.
        if self.baked_probe_volume != volume_to_bake {
            self.bake_probes(
                light_view_proj,
                draw_calls,
                registry,
                tex_registry,
                volume_to_bake,
            );
            self.baked_probe_volume = volume_to_bake;
        }

        // Per-frame draw-call/triangle counters for the profiler. Local to
        // this call (not a shared atomic like texture memory), since
        // `mesh_thumbnail.rs`'s `render_thumbnail` can run mid-frame and
        // must never inflate this frame's `FrameStats`.
        let mut frame_draw_calls: u32 = 0;
        let mut frame_triangles: u64 = 0;

        // GPU pass-timing bookkeeping for the profiler (see `next_timed_pass`
        // / `point_shadow_timestamp_writes`). `gpu_pass_index` is a running
        // count of timed passes issued so far this frame -- also the number
        // of query-set slot *pairs* used, since each pass writes a begin and
        // an end timestamp. `gpu_pass_names` is parallel: `gpu_pass_names[i]`
        // names the pass whose timestamps live at slots `2*i`/`2*i+1`. Both
        // stay empty when `!self.timestamp_supported`, since `next_timed_pass`
        // and `point_shadow_timestamp_writes` never advance them in that case.
        let mut gpu_pass_index: u32 = 0;
        let mut gpu_pass_names: Vec<&'static str> = Vec::new();

        // `presentable` is the swapchain frame to hand back at the end of the
        // function, and is `None` when rendering offscreen.
        let (view, presentable) = self.output.acquire()?;
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("render encoder"),
            });

        // --- post-process config upload ---
        {
            let b = bloom.unwrap_or_default();
            let tm = tone_map.unwrap_or_default();
            let ao = ambient_occlusion.unwrap_or_default();
            let tonemap_mode = match tm.mode {
                bsengine_core::ToneMappingMode::None => 0u32,
                bsengine_core::ToneMappingMode::Reinhard => 1,
                bsengine_core::ToneMappingMode::ReinhardLuminance => 2,
                bsengine_core::ToneMappingMode::Aces => 3,
                bsengine_core::ToneMappingMode::Filmic => 4,
            };
            let pp_config = crate::post_process::PostProcessConfigGpu {
                bloom_threshold: b.threshold,
                bloom_softness: b.softness,
                bloom_intensity: b.intensity,
                bloom_radius: b.radius,
                bloom_enabled: b.enabled as u32,
                tonemap_mode,
                tonemap_exposure: tm.exposure,
                tonemap_enabled: tm.enabled as u32,
                ssao_radius: ao.radius,
                ssao_bias: ao.bias,
                ssao_intensity: ao.intensity,
                ssao_sample_count: ao.sample_count,
                ssao_enabled: ao.enabled as u32,
                // Not `taa.unwrap_or_default()` like the three effects above:
                // `Taa::default()` is enabled, and for TAA an absent component
                // has to mean *off* -- that is what leaves every pre-existing
                // pixel test rendering exactly as it did before.
                taa_enabled: taa.map(|t| t.enabled).unwrap_or(false) as u32,
                taa_history_blend: taa.map(|t| t.history_blend).unwrap_or(0.0),
                taa_clamp_strength: taa.map(|t| t.clamp_strength).unwrap_or(0.0),
            };
            self.post_process.update_config(&self.queue, pp_config);
            let inv_proj = cam_proj.inverse();
            self.post_process.update_ssao_camera(
                &self.queue,
                crate::post_process::SsaoCameraGpu {
                    proj: cam_proj.to_cols_array_2d(),
                    inv_proj: inv_proj.to_cols_array_2d(),
                },
            );
            // Both matrices are the unjittered ones on purpose -- see
            // `TaaCameraGpu` and the jitter comment at the top of this
            // function.
            self.post_process.update_taa_camera(
                &self.queue,
                crate::post_process::TaaCameraGpu {
                    inv_view_proj: unjittered_view_proj.inverse().to_cols_array_2d(),
                    prev_view_proj: self.prev_unjittered_view_proj.to_cols_array_2d(),
                },
            );

            // Volumetric fog. An absent component -- and a present but
            // disabled one -- uploads `enabled: 0`, which makes the apply
            // pass an exact passthrough. That is what leaves every scene
            // that never asks for fog rendering as it did before the froxel
            // volumes existed.
            let active_fog = fog.filter(|f| f.enabled);
            let (fog_near, fog_far) = camera_near_far(cam_proj);
            self.post_process.update_fog(
                &self.queue,
                crate::post_process::FogUniform {
                    // The *jittered* matrix, matching what actually
                    // rasterised the depth buffer the apply pass unprojects.
                    // With TAA off the two are identical; with it on, the
                    // unjittered one would read depth a fraction of a pixel
                    // off its own reconstruction.
                    inv_view_proj: raster_view_proj.inverse().to_cols_array_2d(),
                    // The same matrix the shadow pass below rasterises with and
                    // the scene shader tests surfaces against. Sharing it is
                    // what lets a froxel and a surface at one world position
                    // agree about whether they are in shadow.
                    light_view_proj: light_view_proj.to_cols_array_2d(),
                    camera_pos: cam_pos.to_array(),
                    near: fog_near,
                    // Toward the light, which is the convention the scene
                    // shader's `let l = normalize(-light.direction)` uses.
                    // Handing over the travel direction instead would flip
                    // the phase function and darken exactly the view that
                    // should be brightest.
                    light_dir: (-light.direction.normalize_or_zero()).to_array(),
                    far: fog_far,
                    light_color: light.color.to_array(),
                    density: active_fog.map(|f| f.density).unwrap_or(0.0),
                    fog_color: active_fog.map(|f| *f.color).unwrap_or(Vec3::ONE).to_array(),
                    anisotropy: active_fog.map(|f| f.anisotropy).unwrap_or(0.0),
                    enabled: u32::from(active_fog.is_some()),
                    // Unconditional, unlike `jitter_clip`: the depth dither is
                    // not part of TAA and runs on every foggy frame.
                    frame_index,
                    _pad1: 0.0,
                    _pad2: 0.0,
                },
            );
        }

        // --- shadow pass ---
        // In fast_render mode this still clears the shadow map to depth=1.0
        // (max distance -- "nothing occludes anything"), which reads as
        // fully lit, but skips redrawing every object into it. See the
        // design doc for why clearing (not skipping the pass outright) is
        // required for correctness.
        {
            let mut shadow_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("shadow pass"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.shadow_map_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: self.next_timed_pass(
                    "directional_shadow",
                    &mut gpu_pass_index,
                    &mut gpu_pass_names,
                ),
                occlusion_query_set: None,
            });
            if !self.fast_render {
                shadow_pass.set_pipeline(&self.shadow_pipeline);
                shadow_pass.set_bind_group(0, &self.camera_bind_group, &[]);
                for (i, (mesh_id, _, _, _, _)) in draw_calls.iter().enumerate() {
                    if i >= MAX_OBJECTS {
                        break;
                    }
                    let Some(mesh) = registry.get(*mesh_id) else {
                        continue;
                    };
                    let offset = (i as u64 * MODEL_STRIDE) as u32;
                    shadow_pass.set_bind_group(1, &self.model_bind_group, &[offset]);
                    shadow_pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                    shadow_pass
                        .set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                    shadow_pass.draw_indexed(0..mesh.index_count, 0, 0..1);
                    frame_draw_calls += 1;
                    frame_triangles += (mesh.index_count / 3) as u64;
                }
            }
        }

        // --- point light shadow passes (linear-distance cube arrays) ---
        {
            let active_lights: Vec<_> = light.point_lights.iter().take(MAX_POINT_LIGHTS).collect();
            for (light_idx, pl) in active_lights.iter().enumerate() {
                let view_projs = point_light_face_view_projs(pl.position, pl.range);
                for (face, vp) in view_projs.iter().enumerate() {
                    let slot = light_idx * 6 + face;
                    let data = PointShadowUniformData {
                        view_proj: vp.to_cols_array_2d(),
                        light_pos: pl.position.to_array(),
                        _pad: 0.0,
                    };
                    self.queue.write_buffer(
                        &self.point_shadow_uniform_buffer,
                        slot as u64 * POINT_SHADOW_STRIDE,
                        bytemuck::cast_slice(&[data]),
                    );
                }
            }
            // A frame with N active point lights issues N*6 of these passes
            // (one per cube face). That's up to 48 with MAX_POINT_LIGHTS --
            // too many to give each its own query-set slot pair without the
            // set growing unboundedly. Instead this reserves a single
            // "point_shadow" slot up front and brackets the whole loop:
            // the very first face's pass writes the begin timestamp, the
            // very last face's pass writes the end timestamp, and everything
            // in between is untimed. The result is one aggregate GPU
            // duration covering every point-light shadow face this frame.
            let point_shadow_pass_index = if active_lights.is_empty()
                || !self.timestamp_supported
                || gpu_pass_index >= MAX_TIMED_PASSES
            {
                None
            } else {
                let idx = gpu_pass_index;
                gpu_pass_index += 1;
                gpu_pass_names.push("point_shadow");
                Some(idx)
            };
            let total_point_shadow_passes = active_lights.len() * 6;
            let mut point_shadow_pass_counter = 0usize;
            for (light_idx, _pl) in active_lights.iter().enumerate() {
                for face in 0..6usize {
                    let is_first_point_shadow_pass = point_shadow_pass_counter == 0;
                    let is_last_point_shadow_pass =
                        point_shadow_pass_counter + 1 == total_point_shadow_passes;
                    let slot = light_idx * 6 + face;
                    let layer_view = self._point_shadow_color_texture.create_view(
                        &wgpu::TextureViewDescriptor {
                            label: Some("point shadow layer view"),
                            dimension: Some(wgpu::TextureViewDimension::D2),
                            base_array_layer: slot as u32,
                            array_layer_count: Some(1),
                            ..Default::default()
                        },
                    );
                    let mut point_shadow_pass =
                        encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                            label: Some("point shadow pass"),
                            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                                view: &layer_view,
                                resolve_target: None,
                                ops: wgpu::Operations {
                                    load: wgpu::LoadOp::Clear(wgpu::Color {
                                        r: 1.0e6,
                                        g: 0.0,
                                        b: 0.0,
                                        a: 1.0,
                                    }),
                                    store: wgpu::StoreOp::Store,
                                },
                            })],
                            depth_stencil_attachment: Some(
                                wgpu::RenderPassDepthStencilAttachment {
                                    view: &self.point_shadow_depth_view,
                                    depth_ops: Some(wgpu::Operations {
                                        load: wgpu::LoadOp::Clear(1.0),
                                        store: wgpu::StoreOp::Store,
                                    }),
                                    stencil_ops: None,
                                },
                            ),
                            timestamp_writes: self.point_shadow_timestamp_writes(
                                point_shadow_pass_index,
                                is_first_point_shadow_pass,
                                is_last_point_shadow_pass,
                            ),
                            occlusion_query_set: None,
                        });
                    point_shadow_pass.set_pipeline(&self.point_shadow_pipeline);
                    let uniform_offset = (slot as u64 * POINT_SHADOW_STRIDE) as u32;
                    point_shadow_pass.set_bind_group(
                        0,
                        &self.point_shadow_bind_group,
                        &[uniform_offset],
                    );
                    for (i, (mesh_id, _, _, _, _)) in draw_calls.iter().enumerate() {
                        if i >= MAX_OBJECTS {
                            break;
                        }
                        let Some(mesh) = registry.get(*mesh_id) else {
                            continue;
                        };
                        let offset = (i as u64 * MODEL_STRIDE) as u32;
                        point_shadow_pass.set_bind_group(1, &self.model_bind_group, &[offset]);
                        point_shadow_pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                        point_shadow_pass.set_index_buffer(
                            mesh.index_buffer.slice(..),
                            wgpu::IndexFormat::Uint32,
                        );
                        point_shadow_pass.draw_indexed(0..mesh.index_count, 0, 0..1);
                        frame_draw_calls += 1;
                        frame_triangles += (mesh.index_count / 3) as u64;
                    }
                    point_shadow_pass_counter += 1;
                }
            }
        }

        // Opaque and transparent draws are separated here, and each keeps its
        // ORIGINAL index. That index is the model uniform's dynamic offset --
        // using a position within a filtered list would hand every object
        // someone else's transform, colour and opacity.
        let mut opaque: Vec<usize> = Vec::new();
        let mut transparent: Vec<usize> = Vec::new();
        for (i, (_, _, _, params, _)) in draw_calls.iter().enumerate().take(MAX_OBJECTS) {
            if params.opacity < 1.0 {
                transparent.push(i);
            } else {
                opaque.push(i);
            }
        }
        // Back to front, so nearer surfaces blend over farther ones. Sorting by
        // the distance to each model's origin is the usual approximation: it is
        // wrong for interpenetrating or very large transparent meshes, which is
        // a limitation to know rather than a bug to chase here.
        transparent.sort_by(|&a, &b| {
            let d = |i: usize| (draw_calls[i].1.w_axis.truncate() - cam_pos).length_squared();
            d(b).partial_cmp(&d(a)).unwrap_or(std::cmp::Ordering::Equal)
        });

        // --- main pass (into HDR buffer) ---
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("render pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.post_process.hdr_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.08,
                            g: 0.08,
                            b: 0.08,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: self.next_timed_pass(
                    "main",
                    &mut gpu_pass_index,
                    &mut gpu_pass_names,
                ),
                occlusion_query_set: None,
            });

            pass.set_bind_group(0, &self.camera_bind_group, &[]);
            pass.set_bind_group(2, &self.light_bind_group, &[]);

            for &i in &opaque {
                let (mesh_id, _, tex_id, _, custom_path) = &draw_calls[i];
                let Some(mesh) = registry.get(*mesh_id) else {
                    continue;
                };
                let pipeline = custom_path
                    .as_deref()
                    .and_then(|p| self.custom_pipelines.get(p))
                    .unwrap_or(&self.pipeline);
                pass.set_pipeline(pipeline);
                let tex_bg = tex_id
                    .and_then(|id| tex_registry.and_then(|r| r.get_bind_group(id)))
                    .unwrap_or(&self.default_texture_bind_group);
                let offset = (i as u64 * MODEL_STRIDE) as u32;
                pass.set_bind_group(1, &self.model_bind_group, &[offset]);
                pass.set_bind_group(3, tex_bg, &[]);
                pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..mesh.index_count, 0, 0..1);
                frame_draw_calls += 1;
                frame_triangles += (mesh.index_count / 3) as u64;
            }

            // Terrain chunks get their own model-buffer slots, starting right
            // after the ones `draw_calls` used above (`opaque`/`transparent`
            // only ever index `0..draw_calls.len().min(MAX_OBJECTS)`), so a
            // dedicated running counter can't collide with them. `terrain_slot`
            // is capped the same way `draw_calls` is: once it reaches
            // `MAX_OBJECTS` further terrain chunks are skipped rather than
            // overrunning `model_buffer`.
            let mut terrain_slot = draw_calls.len().min(MAX_OBJECTS);
            for (mesh_id, model, layer_ids, weight_id) in terrain_draw_calls {
                if terrain_slot >= MAX_OBJECTS {
                    break;
                }
                let Some(mesh) = registry.get(*mesh_id) else {
                    continue;
                };
                let Some(tex_reg) = tex_registry else {
                    continue;
                };
                let (Some(v0), Some(v1), Some(v2), Some(v3), Some(vw)) = (
                    tex_reg.get_view(layer_ids[0]),
                    tex_reg.get_view(layer_ids[1]),
                    tex_reg.get_view(layer_ids[2]),
                    tex_reg.get_view(layer_ids[3]),
                    tex_reg.get_view(*weight_id),
                ) else {
                    continue;
                };
                let terrain_bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("terrain bg"),
                    layout: &self.terrain_bgl,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(v0),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::TextureView(v1),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: wgpu::BindingResource::TextureView(v2),
                        },
                        wgpu::BindGroupEntry {
                            binding: 3,
                            resource: wgpu::BindingResource::TextureView(v3),
                        },
                        wgpu::BindGroupEntry {
                            binding: 4,
                            resource: wgpu::BindingResource::TextureView(vw),
                        },
                        wgpu::BindGroupEntry {
                            binding: 5,
                            resource: wgpu::BindingResource::Sampler(&self._sampler),
                        },
                    ],
                });
                let model_data = ModelUniformData {
                    model: model.to_cols_array_2d(),
                    metallic: 0.0,
                    roughness: 0.9,
                    _pad0: 0.0,
                    _pad1: 0.0,
                    emissive: [0.0; 3],
                    _pad2: 0.0,
                    base_color: [1.0, 1.0, 1.0],
                    opacity: 1.0,
                };
                self.queue.write_buffer(
                    &self.model_buffer,
                    terrain_slot as u64 * MODEL_STRIDE,
                    bytemuck::cast_slice(&[model_data]),
                );
                pass.set_pipeline(&self.terrain_pipeline);
                pass.set_bind_group(
                    1,
                    &self.model_bind_group,
                    &[(terrain_slot as u64 * MODEL_STRIDE) as u32],
                );
                pass.set_bind_group(3, &terrain_bg, &[]);
                pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..mesh.index_count, 0, 0..1);
                frame_draw_calls += 1;
                frame_triangles += (mesh.index_count / 3) as u64;
                terrain_slot += 1;
            }
        }

        // --- skybox pass (after geometry so depth=1.0 pixels get the sky) ---
        if let (Some(sky), Some(inv)) = (&self.skybox, sky_vp_inv) {
            let sky_data = SkyUniformData {
                inv_vp: inv.to_cols_array_2d(),
            };
            self.queue
                .write_buffer(&sky.uniform_buffer, 0, bytemuck::cast_slice(&[sky_data]));
            let mut sky_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("skybox pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.post_process.hdr_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: self.next_timed_pass(
                    "sky",
                    &mut gpu_pass_index,
                    &mut gpu_pass_names,
                ),
                occlusion_query_set: None,
            });
            sky_pass.set_pipeline(&sky.pipeline);
            sky_pass.set_bind_group(0, &sky.uniform_bg, &[]);
            sky_pass.set_bind_group(1, &sky.texture_bg, &[]);
            sky_pass.draw(0..3, 0..1);
            frame_draw_calls += 1;
            frame_triangles += 1;
        }

        // --- transparent pass (after the skybox, so glass shows sky through it) ---
        if !transparent.is_empty() {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("transparent pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.post_process.hdr_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        // Load, not Clear, on both attachments. Clearing colour
                        // would wipe the scene this pass is meant to blend
                        // into; clearing depth would let glass float in front
                        // of walls it is behind.
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: self.next_timed_pass(
                    "transparent",
                    &mut gpu_pass_index,
                    &mut gpu_pass_names,
                ),
                occlusion_query_set: None,
            });
            pass.set_bind_group(0, &self.camera_bind_group, &[]);
            pass.set_bind_group(2, &self.light_bind_group, &[]);
            for &i in &transparent {
                let (mesh_id, _, tex_id, _, custom_path) = &draw_calls[i];
                let Some(mesh) = registry.get(*mesh_id) else {
                    continue;
                };
                // A custom shader keeps its own pipeline, which is opaque. That
                // is a documented limitation rather than an oversight: a custom
                // shader decides its own output, alpha included.
                let pipeline = custom_path
                    .as_deref()
                    .and_then(|p| self.custom_pipelines.get(p))
                    .unwrap_or(&self.transparent_pipeline);
                pass.set_pipeline(pipeline);
                let tex_bg = tex_id
                    .and_then(|id| tex_registry.and_then(|r| r.get_bind_group(id)))
                    .unwrap_or(&self.default_texture_bind_group);
                let offset = (i as u64 * MODEL_STRIDE) as u32;
                pass.set_bind_group(1, &self.model_bind_group, &[offset]);
                pass.set_bind_group(3, tex_bg, &[]);
                pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..mesh.index_count, 0, 0..1);
                frame_draw_calls += 1;
                frame_triangles += (mesh.index_count / 3) as u64;
            }
        }

        // --- particle pass (after transparency, so sparks read over glass) ---
        if !particles.is_empty() {
            let (particle_draw_calls, particle_triangles) = self.particles.draw(
                &mut encoder,
                &self.queue,
                &self.post_process.hdr_view,
                &self.depth_view,
                &self.camera_bind_group,
                particles,
                tex_registry,
                &self.default_texture_bind_group,
            );
            frame_draw_calls += particle_draw_calls;
            frame_triangles += particle_triangles;
        }

        // --- post-process passes: bloom → SSAO → composite → swapchain ---
        let is_editor = inspector.as_ref().map(|i| i.editor_mode).unwrap_or(false);
        // In editor mode the rendered game view is a sub-panel within the
        // dock layout, not the whole window — anchor the HUD to that
        // panel's top-left instead of the window's, or "Fell! Retry" etc.
        // paint under the toolbar/other panels instead of over the game.
        let hud_offset = inspector
            .as_ref()
            .filter(|i| i.editor_mode)
            .map(|i| (i.viewport_pos[0] + 8.0, i.viewport_pos[1] + 8.0))
            .unwrap_or((8.0, 8.0));
        let (pp_draw_calls, pp_triangles) =
            self.post_process
                .apply(&mut encoder, &view, self.fast_render);
        frame_draw_calls += pp_draw_calls;
        frame_triangles += pp_triangles;

        // UI + HUD overlay via egui (always on in editor mode)
        let has_ui = is_editor
            || !hud_texts.is_empty()
            || !ui_state.widgets.is_empty()
            || inspector.is_some();
        let mut clicked = std::collections::HashSet::<String>::new();
        if has_ui {
            let screen_rect = egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(self.output.width() as f32, self.output.height() as f32),
            );
            let modifiers = egui::Modifiers {
                alt: alt_held,
                ctrl: ctrl_held,
                shift: shift_held,
                mac_cmd: false,
                command: ctrl_held,
            };
            let cursor_pos = egui::Pos2::new(cursor_x, cursor_y);
            let mut egui_events = vec![egui::Event::PointerMoved(cursor_pos)];
            if left_just_pressed {
                egui_events.push(egui::Event::PointerButton {
                    pos: cursor_pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers,
                });
            }
            if left_just_released {
                egui_events.push(egui::Event::PointerButton {
                    pos: cursor_pos,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers,
                });
            }
            for ev in key_events {
                let pressed = ev.state == bsengine_input::ElementState::Pressed;
                if pressed {
                    if let Some(text) = &ev.text {
                        if !text.is_empty() && !text.chars().any(|c| c.is_control()) {
                            egui_events.push(egui::Event::Text(text.clone()));
                        }
                    }
                }
                if let Some(key) = map_keycode_to_egui(ev.key_code) {
                    egui_events.push(egui::Event::Key {
                        key,
                        physical_key: Some(key),
                        pressed,
                        repeat: false,
                        modifiers,
                    });
                }
            }
            let raw_input = egui::RawInput {
                screen_rect: Some(screen_rect),
                events: egui_events,
                modifiers,
                time: Some(self.start_time.elapsed().as_secs_f64()),
                ..Default::default()
            };

            let mut new_text_values = ui_state.text_values.clone();
            let full_output = self.egui_ctx.run(raw_input, |ctx| {
                // HUD texts — text-only overlay anchored to the game
                // viewport's top-left (the whole window's top-left in
                // non-editor mode, since hud_offset falls back to (8, 8)
                // there — see hud_offset's computation above).
                if !hud_texts.is_empty() {
                    let painter = ctx.layer_painter(egui::LayerId::new(
                        egui::Order::Foreground,
                        egui::Id::new("hud"),
                    ));
                    let mut sorted_keys: Vec<&String> = hud_texts.keys().collect();
                    sorted_keys.sort();
                    for (i, key) in sorted_keys.iter().enumerate() {
                        let text = &hud_texts[*key];
                        painter.text(
                            egui::pos2(hud_offset.0, hud_offset.1 + i as f32 * 24.0),
                            egui::Align2::LEFT_TOP,
                            text,
                            egui::FontId::proportional(20.0),
                            egui::Color32::WHITE,
                        );
                    }
                }

                // Interactive UI widgets
                for widget in &ui_state.widgets {
                    use bsengine_core::UiWidget;
                    match widget {
                        UiWidget::Label {
                            id,
                            text,
                            x,
                            y,
                            font_size,
                        } => {
                            egui::Area::new(egui::Id::new(id.as_str()))
                                .fixed_pos(egui::pos2(*x, *y))
                                .show(ctx, |ui| {
                                    ui.label(egui::RichText::new(text.as_str()).size(*font_size));
                                });
                        }
                        UiWidget::Button {
                            id,
                            label,
                            x,
                            y,
                            width,
                            height,
                        } => {
                            egui::Area::new(egui::Id::new(id.as_str()))
                                .fixed_pos(egui::pos2(*x, *y))
                                .show(ctx, |ui| {
                                    if ui
                                        .add_sized(
                                            egui::vec2(*width, *height),
                                            egui::Button::new(label.as_str()),
                                        )
                                        .clicked()
                                    {
                                        clicked.insert(id.clone());
                                    }
                                });
                        }
                        UiWidget::Panel {
                            id,
                            title,
                            x,
                            y,
                            width,
                            height,
                        } => {
                            egui::Window::new(title.as_str())
                                .id(egui::Id::new(id.as_str()))
                                .fixed_pos(egui::pos2(*x, *y))
                                .fixed_size(egui::vec2(*width, *height))
                                .collapsible(false)
                                .resizable(false)
                                .show(ctx, |_ui| {});
                        }
                        UiWidget::TextInput {
                            id,
                            hint,
                            x,
                            y,
                            width,
                        } => {
                            let text_val = new_text_values.entry(id.clone()).or_default();
                            egui::Area::new(egui::Id::new(id.as_str()))
                                .fixed_pos(egui::pos2(*x, *y))
                                .show(ctx, |ui| {
                                    ui.add(
                                        egui::TextEdit::singleline(text_val)
                                            .hint_text(hint.as_str())
                                            .desired_width(*width),
                                    );
                                });
                        }
                        UiWidget::Image {
                            id,
                            x,
                            y,
                            width,
                            height,
                            ..
                        } => {
                            egui::Area::new(egui::Id::new(id.as_str()))
                                .fixed_pos(egui::pos2(*x, *y))
                                .show(ctx, |ui| {
                                    ui.allocate_exact_size(
                                        egui::vec2(*width, *height),
                                        egui::Sense::hover(),
                                    );
                                });
                        }
                        UiWidget::ProgressBar {
                            id,
                            x,
                            y,
                            width,
                            height,
                            fraction,
                        } => {
                            egui::Area::new(egui::Id::new(id.as_str()))
                                .fixed_pos(egui::pos2(*x, *y))
                                .show(ctx, |ui| {
                                    ui.add_sized(
                                        egui::vec2(*width, *height),
                                        egui::ProgressBar::new(*fraction),
                                    );
                                });
                        }
                    }
                }

                // Runtime inspector panels / full editor layout
                if let Some(insp) = inspector.as_deref_mut() {
                    if insp.editor_mode {
                        // Keyboard shortcuts. Ignored while an egui widget (e.g. a
                        // DragValue or text field) has focus, so Ctrl+Z etc. don't
                        // get stolen from in-progress text editing.
                        let mut despawn_entity = false;
                        let mut duplicate_selected = false;
                        if ctx.memory(|m| m.focused()).is_none() {
                            let (del, dup, undo, redo, save, move_mode, rotate_mode, scale_mode) =
                                ctx.input(|i| {
                                    (
                                        i.key_pressed(egui::Key::Delete),
                                        i.modifiers.ctrl && i.key_pressed(egui::Key::D),
                                        i.modifiers.ctrl
                                            && !i.modifiers.shift
                                            && i.key_pressed(egui::Key::Z),
                                        (i.modifiers.ctrl
                                            && i.modifiers.shift
                                            && i.key_pressed(egui::Key::Z))
                                            || (i.modifiers.ctrl && i.key_pressed(egui::Key::Y)),
                                        i.modifiers.ctrl && i.key_pressed(egui::Key::S),
                                        i.key_pressed(egui::Key::W),
                                        i.key_pressed(egui::Key::E),
                                        i.key_pressed(egui::Key::R),
                                    )
                                });
                            if del {
                                despawn_entity = true;
                            }
                            if dup {
                                duplicate_selected = true;
                            }
                            if undo {
                                insp.request_undo = true;
                            }
                            if redo {
                                insp.request_redo = true;
                            }
                            if save {
                                insp.cmd_queue.push(bsengine_core::InspectorCmd::SaveScene);
                            }
                            if move_mode {
                                insp.gizmo_mode = bsengine_core::GizmoMode::Translate;
                            }
                            if rotate_mode {
                                insp.gizmo_mode = bsengine_core::GizmoMode::Rotate;
                            }
                            if scale_mode {
                                insp.gizmo_mode = bsengine_core::GizmoMode::Scale;
                            }
                        }
                        if despawn_entity {
                            if let Some(id) = insp.selected_id {
                                insp.cmd_queue
                                    .push(bsengine_core::InspectorCmd::Despawn { id });
                                insp.selected_id = None;
                            }
                        }
                        if duplicate_selected {
                            if let Some(id) = insp.selected_id {
                                insp.cmd_queue
                                    .push(bsengine_core::InspectorCmd::Duplicate { id });
                            }
                        }

                        if let Some(registry) = editor_panels {
                            crate::panels::ensure_builtin_panels(
                                registry,
                                self.device.clone(),
                                self.queue.clone(),
                                self.frame_stats_history.clone(),
                            );
                            let mut dock_state = self.dock_state.take().unwrap_or_else(|| {
                                crate::panels::load_dock_state(&crate::panels::layout_path())
                                    .unwrap_or_else(crate::panels::default_dock_state)
                            });

                            egui::TopBottomPanel::top("bse_editor_toolbar").show(ctx, |ui| {
                                ui.horizontal(|ui| {
                                    ui.horizontal(|ui| {
                                        if ui
                                            .button(format!(
                                                "{} Play",
                                                egui_phosphor::regular::PLAY
                                            ))
                                            .on_hover_text("Play (toggles editor/play mode)")
                                            .clicked()
                                        {
                                            let starting_play = insp.play_state
                                                != bsengine_core::EditorPlayState::Playing;
                                            insp.play_state = if starting_play {
                                                bsengine_core::EditorPlayState::Playing
                                            } else {
                                                bsengine_core::EditorPlayState::Stopped
                                            };
                                            if starting_play {
                                                // Unity/Unreal-style "Play resets the
                                                // scene": every Play press respawns
                                                // from the scene file's authored
                                                // state, discarding whatever the
                                                // previous session's physics/scripts
                                                // did to it (e.g. a fallen ball).
                                                insp.cmd_queue
                                                    .push(bsengine_core::InspectorCmd::ReloadScene);
                                            }
                                        }
                                        let mode_label = if insp.play_state
                                            == bsengine_core::EditorPlayState::Playing
                                        {
                                            "● Playing"
                                        } else {
                                            "◆ Editor"
                                        };
                                        ui.label(mode_label);
                                    });
                                    ui.separator();
                                    ui.horizontal(|ui| {
                                        if ui
                                            .button(egui_phosphor::regular::ARROW_COUNTER_CLOCKWISE)
                                            .on_hover_text("Undo (Ctrl+Z)")
                                            .clicked()
                                        {
                                            insp.request_undo = true;
                                        }
                                        if ui
                                            .button(egui_phosphor::regular::ARROW_CLOCKWISE)
                                            .on_hover_text("Redo (Ctrl+Y)")
                                            .clicked()
                                        {
                                            insp.request_redo = true;
                                        }
                                        let save_enabled = insp.current_scene_path.is_some();
                                        if ui
                                            .add_enabled(
                                                save_enabled,
                                                egui::Button::new(
                                                    egui_phosphor::regular::FLOPPY_DISK,
                                                ),
                                            )
                                            .on_hover_text("Save Scene (Ctrl+S)")
                                            .on_disabled_hover_text("No scene file loaded")
                                            .clicked()
                                        {
                                            insp.cmd_queue
                                                .push(bsengine_core::InspectorCmd::SaveScene);
                                        }
                                    });
                                    ui.separator();
                                    ui.horizontal(|ui| {
                                        if ui
                                            .selectable_label(
                                                insp.gizmo_mode
                                                    == bsengine_core::GizmoMode::Translate,
                                                format!(
                                                    "{} Move",
                                                    egui_phosphor::regular::ARROWS_OUT_CARDINAL
                                                ),
                                            )
                                            .on_hover_text("Move (W)")
                                            .clicked()
                                        {
                                            insp.gizmo_mode = bsengine_core::GizmoMode::Translate;
                                        }
                                        if ui
                                            .selectable_label(
                                                insp.gizmo_mode == bsengine_core::GizmoMode::Rotate,
                                                format!(
                                                    "{} Rotate",
                                                    egui_phosphor::regular::ARROWS_CLOCKWISE
                                                ),
                                            )
                                            .on_hover_text("Rotate (E)")
                                            .clicked()
                                        {
                                            insp.gizmo_mode = bsengine_core::GizmoMode::Rotate;
                                        }
                                        if ui
                                            .selectable_label(
                                                insp.gizmo_mode == bsengine_core::GizmoMode::Scale,
                                                format!(
                                                    "{} Scale",
                                                    egui_phosphor::regular::CORNERS_OUT
                                                ),
                                            )
                                            .on_hover_text("Scale (R)")
                                            .clicked()
                                        {
                                            insp.gizmo_mode = bsengine_core::GizmoMode::Scale;
                                        }
                                    });
                                    ui.separator();
                                    crate::panels::window_menu_ui(ui, &mut dock_state, registry);
                                });
                            });

                            let entities_snapshot = insp.entities.clone();
                            let mut panels_guard = registry.0.lock().unwrap();
                            let type_registry_guard = type_registry.map(|r| r.read());
                            let mut tab_viewer = crate::panels::BseTabViewer {
                                insp,
                                entities_snapshot: &entities_snapshot,
                                cursor_pos: (cursor_x, cursor_y),
                                panels: &mut panels_guard,
                                type_registry: type_registry_guard.as_deref(),
                            };
                            let mut dock_style = egui_dock::Style::from_egui(ctx.style().as_ref());
                            // `Style::from_egui` derives the tab-bar's "focused"/"hovered"/
                            // "*_with_kb_focus" text colors from `Visuals::strong_text_color()`,
                            // which resolves to `widgets.active.fg_stroke` — this theme
                            // deliberately makes that near-black (dark text on the bright
                            // accent-colored Play/active-button background), which egui_dock's
                            // reuse of the same field turns into near-invisible dark-on-dark
                            // text for whichever tab is currently focused or hovered. Override
                            // just those tab-text colors back to the theme's normal bright text.
                            dock_style.tab.focused.text_color = crate::theme::TEXT;
                            dock_style.tab.focused_with_kb_focus.text_color = crate::theme::TEXT;
                            dock_style.tab.hovered.text_color = crate::theme::TEXT;
                            dock_style.tab.active_with_kb_focus.text_color = crate::theme::TEXT;
                            dock_style.tab.inactive_with_kb_focus.text_color = crate::theme::TEXT;
                            egui_dock::DockArea::new(&mut dock_state)
                                .style(dock_style)
                                .show(ctx, &mut tab_viewer);
                            drop(panels_guard);

                            let layout_json =
                                serde_json::to_string(&dock_state).unwrap_or_default();
                            if self.last_saved_layout_json.as_deref() != Some(layout_json.as_str())
                            {
                                crate::panels::save_dock_state(
                                    &crate::panels::layout_path(),
                                    &dock_state,
                                );
                                self.last_saved_layout_json = Some(layout_json);
                            }
                            self.dock_state = Some(dock_state);
                        }
                    } else {
                        // Overlay mode: side panels rendered over the running game
                        let entities_snapshot = insp.entities.clone();
                        let current_sel = insp.selected_id;
                        let mut new_sel = insp.selected_id;

                        egui::SidePanel::left("bse_insp_entities")
                            .default_width(200.0)
                            .show(ctx, |ui| {
                                ui.heading("Entities");
                                ui.separator();
                                egui::ScrollArea::vertical().show(ui, |ui| {
                                    for info in &entities_snapshot {
                                        let label = info.name.as_deref().unwrap_or("(unnamed)");
                                        let text = format!("[{}] {}", info.id, label);
                                        if ui
                                            .selectable_label(current_sel == Some(info.id), text)
                                            .clicked()
                                        {
                                            new_sel = Some(info.id);
                                        }
                                    }
                                });
                            });

                        if new_sel != insp.selected_id {
                            insp.selected_id = new_sel;
                            insp.sync_selection();
                        }

                        if let Some(sel_id) = insp.selected_id {
                            let entity_name = insp
                                .entities
                                .iter()
                                .find(|e| e.id == sel_id)
                                .and_then(|e| e.name.as_deref().map(String::from))
                                .unwrap_or_else(|| format!("Entity {sel_id}"));

                            let type_registry_guard = type_registry.map(|r| r.read());
                            let reflect_ctx = crate::panels::reflect_ui::ReflectUiCtx {
                                entities: &entities_snapshot,
                                type_registry: type_registry_guard.as_deref(),
                            };
                            let mut to_apply: Vec<(String, Box<dyn bevy_reflect::Reflect>)> =
                                Vec::new();

                            egui::SidePanel::right("bse_insp_props")
                                .default_width(220.0)
                                .show(ctx, |ui| {
                                    ui.heading(&entity_name);
                                    ui.separator();
                                    for (type_path, value) in
                                        insp.reflected_components.iter_mut().filter(|(p, _)| {
                                            !crate::panels::reflect_ui::is_hidden_reflected_type(p)
                                        })
                                    {
                                        ui.colored_label(crate::theme::TEXT, type_path.as_str());
                                        if crate::panels::reflect_ui::draw_reflect_ui(
                                            ui,
                                            value.as_mut(),
                                            &reflect_ctx,
                                        ) {
                                            crate::panels::reflect_ui::validate_after_edit(
                                                type_path,
                                                value.as_mut(),
                                                type_registry_guard.as_deref(),
                                            );
                                            to_apply.push((type_path.clone(), value.clone_value()));
                                        }
                                        ui.separator();
                                    }
                                });

                            for (type_path, value) in to_apply {
                                insp.cmd_queue.push(
                                    bsengine_core::InspectorCmd::ApplyReflectedComponent {
                                        id: sel_id,
                                        type_path,
                                        value,
                                    },
                                );
                            }
                        }
                    }
                }
            });

            let clipped_primitives = self
                .egui_ctx
                .tessellate(full_output.shapes, full_output.pixels_per_point);
            let screen_descriptor = egui_wgpu::ScreenDescriptor {
                size_in_pixels: [self.output.width(), self.output.height()],
                pixels_per_point: full_output.pixels_per_point,
            };

            for (id, image_delta) in &full_output.textures_delta.set {
                self.egui_renderer
                    .update_texture(&self.device, &self.queue, *id, image_delta);
            }
            self.egui_renderer.update_buffers(
                &self.device,
                &self.queue,
                &mut encoder,
                &clipped_primitives,
                &screen_descriptor,
            );
            {
                let mut egui_pass = encoder
                    .begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("egui ui pass"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &view,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Load,
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: None,
                        timestamp_writes: self.next_timed_pass(
                            "egui",
                            &mut gpu_pass_index,
                            &mut gpu_pass_names,
                        ),
                        ..Default::default()
                    })
                    .forget_lifetime();
                self.egui_renderer
                    .render(&mut egui_pass, &clipped_primitives, &screen_descriptor);
            }
            for id in &full_output.textures_delta.free {
                self.egui_renderer.free_texture(id);
            }
            let _ = new_text_values;
        }

        // Resolve this frame's GPU timestamps into a buffer the CPU can map.
        // `gpu_pass_index` is the number of timed passes issued above (0
        // when `!self.timestamp_supported`, since `next_timed_pass` and
        // `point_shadow_timestamp_writes` never advance it in that case).
        if gpu_pass_index > 0 {
            if let (Some(query_set), Some(resolve_buffer), Some(readback_buffer)) = (
                &self.timestamp_query_set,
                &self.timestamp_resolve_buffer,
                &self.timestamp_readback_buffer,
            ) {
                let ticks = gpu_pass_index * 2;
                encoder.resolve_query_set(query_set, 0..ticks, resolve_buffer, 0);
                encoder.copy_buffer_to_buffer(
                    resolve_buffer,
                    0,
                    readback_buffer,
                    0,
                    ticks as u64 * wgpu::QUERY_SIZE as u64,
                );
            }
        }

        self.queue.submit(std::iter::once(encoder.finish()));

        let gpu_pass_times_ms = if gpu_pass_index > 0 {
            self.read_gpu_pass_times(gpu_pass_index, &gpu_pass_names)
        } else {
            Vec::new()
        };

        if let Some(frame) = presentable {
            frame.present();
        } else {
            // Offscreen mode (headless E2E replays, MCP test sessions) has no
            // swapchain to present to, so nothing else provides the
            // backpressure `present()` gives the windowed path above --
            // every `submit()` here would otherwise queue GPU work with zero
            // synchronization between frames. Submitting faster than the GPU
            // retires work can wrap wgpu-hal's D3D12 command-allocator ring
            // before the GPU has finished an allocator's previous use,
            // producing a hard D3D12 validation error (COMMAND_ALLOCATOR_SYNC
            // / OBJECT_DELETED_WHILE_STILL_IN_USE) -- reproduced by every one
            // of this workspace's E2E replays on Windows CI once offscreen
            // rendering was turned on for the headless test runtime.
            //
            // Has to be inline here, not in `WgpuSurface`'s `Drop` impl: both
            // teardown paths that hit this bug skip Rust destructors
            // entirely -- an E2E replay's process exits via
            // `std::process::exit` (`bsengine-runtime/src/main.rs`), and an
            // MCP session's child is torn down via `Child::kill`
            // (`bsengine-mcp/src/session.rs`) -- so a `Drop`-based wait alone
            // (tried first; still present below, for the paths that *do*
            // drop normally, e.g. this crate's own tests) never runs on
            // either failing path.
            self.device.poll(wgpu::Maintain::Wait);
        }

        let frame_stats = crate::profiler::FrameStats {
            cpu_frame_time_ms: frame_start.elapsed().as_secs_f32() * 1000.0,
            gpu_pass_times_ms,
            gpu_timestamps_supported: self.timestamp_supported,
            draw_calls: frame_draw_calls,
            triangles: frame_triangles,
            occluded_count,
            texture_memory_bytes: crate::profiler::texture_memory_bytes(),
            texture_count: crate::profiler::texture_count(),
        };
        {
            let mut history = self.frame_stats_history.lock().unwrap();
            history.push_back(frame_stats);
            while history.len() > crate::profiler::FRAME_STATS_HISTORY_CAPACITY {
                history.pop_front();
            }
        }

        // This frame's camera becomes next frame's reprojection source. Stored
        // last, after the resolve above has consumed the previous value.
        self.prev_unjittered_view_proj = unjittered_view_proj;

        Ok(clicked)
    }

    /// Reconfigures the swapchain and depth/post-process targets for a new
    /// window size; a no-op if either dimension is zero (e.g. minimized).
    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.output.resize(&self.device, width, height);
        let (depth_texture, depth_view) = Self::create_depth_texture(&self.device, width, height);
        self.depth_texture = depth_texture;
        self.depth_view = depth_view;
        self.post_process
            .resize_targets(&self.device, &self.depth_view, width, height);
    }

    /// Throws away the accumulated TAA history, so the next
    /// [`Self::render_frame`] starts a fresh temporal accumulation instead of
    /// blending against whatever was rendered before.
    ///
    /// A surface is long-lived and its history survives across frames by
    /// design -- that accumulation *is* the effect. Anything that wants a
    /// frame to depend only on the scene it was handed, rather than on the
    /// scene rendered before it, has to say so explicitly; the pixel-test
    /// harness calls this before every one-shot `render` for exactly that
    /// reason.
    pub fn invalidate_taa_history(&mut self) {
        self.post_process.invalidate_history();
    }

    /// Checks that `wgsl` is a shader `wgpu` will accept, without touching the
    /// GPU. `path` only labels the message.
    ///
    /// Both halves of `wgpu`'s own front end are run, in its order: naga's WGSL
    /// parser (syntax and name resolution), then `naga::valid::Validator`
    /// (everything the parser lets through). The second half is not optional --
    /// `parse_str` alone accepts a vertex entry point with no
    /// `@builtin(position)` output, a `return` whose type does not match the
    /// declared one, and two globals sharing a `@binding`; all three are
    /// rejected by the validator, which is exactly what
    /// `wgpu::Device::create_shader_module` runs internally. Pre-checking only
    /// the parse would leave those three cases reaching the device.
    ///
    /// Capabilities are `naga::valid::Capabilities::all` rather than the ones
    /// this device actually reports, so this pass can never reject a shader the
    /// device would have accepted. Capability-gated features (`f16`, push
    /// constants, ...) are left to the device, whose verdict
    /// [`Self::compile_and_store_shader`] captures with an error scope rather
    /// than letting it panic.
    pub fn validate_wgsl(path: &str, wgsl: &str) -> Result<(), String> {
        let module = wgpu::naga::front::wgsl::parse_str(wgsl)
            .map_err(|e| e.emit_to_string_with_path(wgsl, path))?;
        wgpu::naga::valid::Validator::new(
            wgpu::naga::valid::ValidationFlags::all(),
            wgpu::naga::valid::Capabilities::all(),
        )
        .validate(&module)
        .map(|_| ())
        .map_err(|e| e.emit_to_string_with_path(wgsl, path))
    }

    /// Compiles a WGSL custom shader and stores it as a render pipeline keyed
    /// by `path`, replacing any previously compiled pipeline for that path.
    ///
    /// Returns `Err` with the reason if the source does not compile, having
    /// left `custom_pipelines` untouched -- so a shader edited into a broken
    /// intermediate state keeps drawing with its last working pipeline instead
    /// of vanishing. Callers use that to avoid re-attempting a compile that
    /// cannot succeed until the file changes again.
    ///
    /// Nothing here may panic on bad *content*: hot reload recompiles at
    /// runtime from whatever the file currently says, and a half-typed shader
    /// must not take the running game down with it. `wgpu`'s default uncaptured
    /// error handler panics, so the two device calls are wrapped in a
    /// validation error scope. The naga pre-pass above is what produces a
    /// readable message (line, column, offending token); the error scope is the
    /// backstop for what a device-independent pre-pass cannot know -- capability
    /// gaps, a renamed `vs_main`/`fs_main`, bindings that do not match
    /// `pipeline_layout`.
    pub fn compile_and_store_shader(&mut self, path: &str, wgsl: &str) -> Result<(), String> {
        if let Err(e) = Self::validate_wgsl(path, wgsl) {
            tracing::warn!(
                "[custom_shader] '{path}' is not valid WGSL; keeping the previously compiled pipeline:\n{e}"
            );
            return Err(e);
        }
        self.device.push_error_scope(wgpu::ErrorFilter::Validation);
        let shader = self
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("custom shader"),
                source: wgpu::ShaderSource::Wgsl(wgsl.into()),
            });
        let vertex_attrs = [
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x3,
                offset: 0,
                shader_location: 0,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x3,
                offset: 12,
                shader_location: 1,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x3,
                offset: 24,
                shader_location: 2,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x2,
                offset: 36,
                shader_location: 3,
            },
        ];
        let vbl = wgpu::VertexBufferLayout {
            array_stride: VERTEX_STRIDE,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &vertex_attrs,
        };
        let pipeline = self
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("custom pipeline"),
                layout: Some(&self.pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: "vs_main",
                    buffers: &[vbl],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: "fs_main",
                    targets: &[Some(wgpu::ColorTargetState {
                        format: crate::post_process::HDR_FORMAT,
                        blend: Some(wgpu::BlendState::REPLACE),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    cull_mode: Some(wgpu::Face::Back),
                    front_face: wgpu::FrontFace::Ccw,
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: DEPTH_FORMAT,
                    depth_write_enabled: true,
                    depth_compare: wgpu::CompareFunction::Less,
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                }),
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
                cache: None,
            });
        // `pop_error_scope`'s future is already resolved on every native
        // backend (wgpu-core reports validation errors synchronously into the
        // error sink and hands back a ready future), so this neither blocks nor
        // needs the device polled.
        if let Some(err) = pollster::block_on(self.device.pop_error_scope()) {
            let msg = err.to_string();
            tracing::warn!(
                "[custom_shader] the GPU device rejected '{path}'; keeping the previously compiled pipeline: {msg}"
            );
            return Err(msg);
        }
        self.custom_pipelines.insert(path.to_string(), pipeline);
        Ok(())
    }

    /// Whether a custom shader has already been compiled and stored for `path`.
    pub fn has_custom_shader(&self, path: &str) -> bool {
        self.custom_pipelines.contains_key(path)
    }

    /// Compiles a standalone WGSL source string into a shader module, without
    /// building a pipeline or storing it.
    pub fn compile_shader(device: &wgpu::Device, src: &str) -> wgpu::ShaderModule {
        device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("wgsl shader"),
            source: wgpu::ShaderSource::Wgsl(src.into()),
        })
    }
}

impl Drop for WgpuSurface {
    fn drop(&mut self) {
        // Defense in depth for hosts that drop a WgpuSurface normally --
        // this crate's own tests build an offscreen surface and let it go
        // out of scope at the end of the test function, which does run
        // Drop. It does NOT cover headless E2E replays or MCP test
        // sessions: both tear their process down by means that skip Rust
        // destructors entirely (`std::process::exit` in
        // `bsengine-runtime/src/main.rs`, `Child::kill` in
        // `bsengine-mcp/src/session.rs`), so this never runs on either of
        // those paths. `render_frame`'s inline `device.poll(Maintain::Wait)`
        // on the offscreen branch (see there) is what actually covers those
        // -- this Drop impl alone was tried first and confirmed
        // insufficient: CI failed identically with only this in place.
        self.device.poll(wgpu::Maintain::Wait);
    }
}

/// ECS resource wrapping the app's [`WgpuSurface`].
#[derive(Resource)]
pub struct WgpuSurfaceResource(pub WgpuSurface);

#[cfg(test)]
mod tests {
    use super::*;

    // A `(view_proj, cam_proj)` pair of the exact shape `render_frame`
    // receives. The camera is deliberately off-origin and rotated: with a
    // camera at the origin looking down -Z the view matrix is the identity,
    // `view_proj == cam_proj`, and jittering the combined matrix would be
    // indistinguishable from jittering the projection -- the test would
    // certify nothing.
    fn test_camera() -> (Mat4, Mat4) {
        let proj = Mat4::perspective_rh(60.0_f32.to_radians(), 16.0 / 9.0, 0.1, 100.0);
        let view = Mat4::look_at_rh(
            Vec3::new(3.0, 2.0, 5.0),
            Vec3::new(-1.0, 0.5, -2.0),
            Vec3::Y,
        );
        (proj * view, proj)
    }

    fn ndc(view_proj: Mat4, world: Vec3) -> (f32, f32) {
        let clip = view_proj * world.extend(1.0);
        (clip.x / clip.w, clip.y / clip.w)
    }

    #[test]
    fn jitter_shifts_ndc_by_the_same_amount_at_every_depth() {
        // The whole reason the offset goes on the projection's third column
        // (the one scaled by `w`) rather than onto the combined
        // view-projection: a sub-pixel nudge has to move near and far
        // geometry by the SAME amount in NDC. Applied to the combined matrix
        // the shift would scale with world-space z, shearing the scene
        // instead of resampling it, and TAA would blur rather than resolve.
        let (view_proj, proj) = test_camera();
        let jitter = (0.004_f32, -0.003_f32);
        let jittered = jittered_view_proj(view_proj, proj, jitter);

        // Four points spread over two decades of distance and off the view
        // axis in both screen directions, so a shift that scaled with depth
        // (or with a world coordinate) could not hide.
        let probes = [
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(-2.0, 1.5, -1.0),
            Vec3::new(4.0, -3.0, -20.0),
            Vec3::new(-6.0, 2.0, -55.0),
        ];
        let shifts: Vec<(f32, f32)> = probes
            .iter()
            .map(|p| {
                let (a, b) = ndc(view_proj, *p);
                let (c, d) = ndc(jittered, *p);
                (c - a, d - b)
            })
            .collect();
        for (i, s) in shifts.iter().enumerate() {
            assert!(
                (s.0 - shifts[0].0).abs() < 1e-5 && (s.1 - shifts[0].1).abs() < 1e-5,
                "jitter must be depth-independent, but probe {i} at {:?} shifted \
                 by {s:?} while the first shifted by {:?}",
                probes[i],
                shifts[0]
            );
        }
        // ...and by exactly the amount asked for. The sign flips because the
        // third column is multiplied by view-space z while the divide is by
        // `w = -z`, so a `+jitter` column entry lands as a `-jitter` NDC
        // shift. Which direction is irrelevant to TAA -- the Halton offsets
        // are symmetric about zero -- but pinning it keeps the magnitude
        // assertion from passing on a matrix that merely moved *somewhere*.
        assert!(
            (shifts[0].0 + jitter.0).abs() < 1e-5 && (shifts[0].1 + jitter.1).abs() < 1e-5,
            "expected an NDC shift of {:?}, got {:?}",
            (-jitter.0, -jitter.1),
            shifts[0]
        );
    }

    #[test]
    fn zero_jitter_leaves_the_matrix_bit_for_bit_alone() {
        // Every camera without a `Taa` component takes this path, so any
        // drift here would change what all eight existing pixel tests render.
        let (view_proj, proj) = test_camera();
        assert_eq!(jittered_view_proj(view_proj, proj, (0.0, 0.0)), view_proj);
    }

    #[test]
    fn a_degenerate_projection_falls_back_instead_of_producing_nans() {
        // `InspectorState::editor_proj` starts as an all-zero matrix, and the
        // editor override installs it before the orbit camera has ever run.
        // Inverting it would poison the whole frame.
        let (view_proj, _) = test_camera();
        let jittered = jittered_view_proj(view_proj, Mat4::ZERO, (0.004, -0.003));
        assert_eq!(jittered, view_proj);
    }

    #[test]
    fn near_and_far_come_back_out_of_the_projection_they_went_into() {
        // The froxel slicing spans exactly this range and the apply pass
        // inverts that span, so a wrong pair puts the fog at the wrong
        // distance -- which reads as a density problem, not a mapping one.
        for (near, far) in [(0.1_f32, 100.0_f32), (0.05, 500.0), (1.0, 20.0)] {
            let proj = Mat4::perspective_rh(60.0_f32.to_radians(), 16.0 / 9.0, near, far);
            let (n, f) = camera_near_far(proj);
            assert!(
                (n - near).abs() < near * 1e-3,
                "near {n} should have been {near}"
            );
            assert!(
                (f - far).abs() < far * 1e-3,
                "far {f} should have been {far}"
            );
        }
    }

    #[test]
    fn a_degenerate_projection_yields_usable_near_and_far() {
        // Same all-zero `editor_proj` as the jitter fallback above. A zero
        // near would divide by `log(far/0)` in the froxel mapping and fill
        // every slice with NaN.
        let (near, far) = camera_near_far(Mat4::ZERO);
        assert!(
            near > 0.0 && far > near && near.is_finite() && far.is_finite(),
            "expected a usable range, got near {near}, far {far}"
        );
    }

    #[test]
    fn point_light_face_view_projs_all_invertible() {
        let vps = point_light_face_view_projs(Vec3::new(1.0, 2.0, 3.0), 10.0);
        for (i, vp) in vps.iter().enumerate() {
            assert!(
                vp.determinant().abs() > 1e-6,
                "face {i} view-proj should be invertible"
            );
        }
    }

    #[test]
    fn point_light_face_view_projs_point_along_face_direction_projects_near_center() {
        let light_pos = Vec3::ZERO;
        let vps = point_light_face_view_projs(light_pos, 10.0);
        let dirs = [
            Vec3::X,
            Vec3::NEG_X,
            Vec3::Y,
            Vec3::NEG_Y,
            Vec3::Z,
            Vec3::NEG_Z,
        ];
        for (vp, dir) in vps.iter().zip(dirs.iter()) {
            let world = light_pos + *dir * 5.0;
            let clip = *vp * world.extend(1.0);
            let ndc = clip.truncate() / clip.w;
            assert!(
                ndc.x.abs() < 1e-4 && ndc.y.abs() < 1e-4,
                "point along face direction {dir:?} should project near NDC (0,0), got {ndc:?}"
            );
        }
    }

    #[test]
    fn point_shadow_wgsl_face_selection_matches_view_proj_ndc() {
        // Independently reimplements point_shadow_factor's manual cube-face
        // selection + UV math from MESH_WGSL in Rust, and checks it agrees
        // with the ACTUAL NDC the corresponding point_light_face_view_projs
        // matrix produces for the same point -- that matrix is what actually
        // gets rendered into each shadow face, so agreement here is what
        // proves the shader samples the correct texel. Deliberately uses
        // off-axis points (not purely along one axis) so a U/V swap or sign
        // error would be caught -- a point exactly on-axis projects to NDC
        // (0,0) either way and can't distinguish those bugs (see the
        // "projects_near_center" test above, which only checks that case).
        fn wgsl_face_uv(to_frag: Vec3) -> (usize, f32, f32) {
            let ax = to_frag.x.abs();
            let ay = to_frag.y.abs();
            let az = to_frag.z.abs();
            let (face, u, v, ma) = if ax >= ay && ax >= az {
                if to_frag.x > 0.0 {
                    (0, -to_frag.z, -to_frag.y, ax)
                } else {
                    (1, to_frag.z, -to_frag.y, ax)
                }
            } else if ay >= ax && ay >= az {
                if to_frag.y > 0.0 {
                    (2, to_frag.x, to_frag.z, ay)
                } else {
                    (3, to_frag.x, -to_frag.z, ay)
                }
            } else if to_frag.z > 0.0 {
                (4, to_frag.x, -to_frag.y, az)
            } else {
                (5, -to_frag.x, -to_frag.y, az)
            };
            (face, u / ma, v / ma)
        }

        let light_pos = Vec3::ZERO;
        let vps = point_light_face_view_projs(light_pos, 10.0);
        let cases: [(Vec3, usize); 6] = [
            (Vec3::new(3.0, 0.5, -1.0), 0),
            (Vec3::new(-3.0, 0.5, -1.0), 1),
            (Vec3::new(0.5, 3.0, -1.0), 2),
            (Vec3::new(0.5, -3.0, -1.0), 3),
            (Vec3::new(0.5, -1.0, 3.0), 4),
            (Vec3::new(0.5, -1.0, -3.0), 5),
        ];
        for (to_frag, expected_face) in cases {
            let (face, u, v) = wgsl_face_uv(to_frag);
            assert_eq!(
                face, expected_face,
                "face selection mismatch for {to_frag:?}"
            );
            let world = light_pos + to_frag;
            let clip = vps[face] * world.extend(1.0);
            let ndc = clip.truncate() / clip.w;
            assert!(
                (ndc.x - u).abs() < 1e-4 && (ndc.y - v).abs() < 1e-4,
                "face {face}: WGSL-equivalent uv=({u},{v}) does not match actual NDC {ndc:?} for {to_frag:?}"
            );
        }
    }

    #[test]
    fn mesh_shader_compiles() {
        let (device, _queue) = pollster::block_on(WgpuSurface::headless_device_for_testing());
        let _module = WgpuSurface::compile_shader(&device, MESH_WGSL);
    }

    #[test]
    fn camera_uniform_data_time_field_at_correct_byte_offset() {
        let data = CameraUniformData {
            view_proj: [[0.0; 4]; 4],
            light_view_proj: [[0.0; 4]; 4],
            cam_pos: [1.0, 2.0, 3.0],
            time: 42.5,
        };
        assert_eq!(
            std::mem::size_of::<CameraUniformData>(),
            CAMERA_UNIFORM_SIZE as usize
        );
        let bytes = bytemuck::bytes_of(&data);
        // view_proj(64) + light_view_proj(64) + cam_pos(12) = 140, time starts at 140... but
        // cam_pos:vec3<f32> requires 16-byte alignment in the uniform buffer layout this struct
        // mirrors, so time actually lands at offset 140 within this Rust repr(C) struct (no
        // padding is inserted by Rust here since f32 has 4-byte alignment) -- what matters for
        // the GPU is CAMERA_UNIFORM_SIZE staying 144 and this field being the last 4 bytes.
        let time_bytes = &bytes[140..144];
        assert_eq!(f32::from_ne_bytes(time_bytes.try_into().unwrap()), 42.5);
    }

    #[test]
    fn map_keycode_to_egui_covers_digits() {
        use bsengine_input::KeyCode;
        assert_eq!(map_keycode_to_egui(KeyCode::Key0), Some(egui::Key::Num0));
        assert_eq!(map_keycode_to_egui(KeyCode::Key9), Some(egui::Key::Num9));
    }

    #[test]
    fn map_keycode_to_egui_covers_editing_keys() {
        use bsengine_input::KeyCode;
        assert_eq!(
            map_keycode_to_egui(KeyCode::Backspace),
            Some(egui::Key::Backspace)
        );
        assert_eq!(
            map_keycode_to_egui(KeyCode::Delete),
            Some(egui::Key::Delete)
        );
        assert_eq!(map_keycode_to_egui(KeyCode::Enter), Some(egui::Key::Enter));
        assert_eq!(
            map_keycode_to_egui(KeyCode::Escape),
            Some(egui::Key::Escape)
        );
        assert_eq!(map_keycode_to_egui(KeyCode::Minus), Some(egui::Key::Minus));
        assert_eq!(
            map_keycode_to_egui(KeyCode::Period),
            Some(egui::Key::Period)
        );
    }

    #[test]
    fn map_keycode_to_egui_excludes_modifier_keys() {
        use bsengine_input::KeyCode;
        assert_eq!(map_keycode_to_egui(KeyCode::ControlLeft), None);
        assert_eq!(map_keycode_to_egui(KeyCode::ShiftLeft), None);
        assert_eq!(map_keycode_to_egui(KeyCode::AltLeft), None);
        assert_eq!(map_keycode_to_egui(KeyCode::Unknown), None);
    }

    #[test]
    fn skybox_shader_compiles() {
        let (device, _queue) = pollster::block_on(WgpuSurface::headless_device_for_testing());
        let _module = WgpuSurface::compile_shader(&device, SKYBOX_WGSL);
    }

    #[test]
    fn custom_shader_wgsl_compiles() {
        let (device, _queue) = pollster::block_on(WgpuSurface::headless_device_for_testing());
        let wgsl = r#"
@vertex fn vs_main(@builtin(vertex_index) vi: u32) -> @builtin(position) vec4<f32> {
    return vec4<f32>(0.0, 0.0, 0.0, 1.0);
}
@fragment fn fs_main() -> @location(0) vec4<f32> {
    return vec4<f32>(1.0, 0.0, 0.0, 1.0);
}
"#;
        let _module = WgpuSurface::compile_shader(&device, wgsl);
    }

    /// The shape `compile_and_store_shader` builds a pipeline from: a vertex
    /// entry named `vs_main` and a fragment entry named `fs_main`.
    const VALID_CUSTOM_WGSL: &str = r#"
@vertex fn vs_main(@builtin(vertex_index) vi: u32) -> @builtin(position) vec4<f32> {
    return vec4<f32>(0.0, 0.0, 0.0, 1.0);
}
@fragment fn fs_main() -> @location(0) vec4<f32> {
    return vec4<f32>(1.0, 0.0, 0.0, 1.0);
}
"#;

    #[test]
    fn validate_wgsl_accepts_a_valid_custom_shader() {
        assert_eq!(
            WgpuSurface::validate_wgsl("assets/glow.wgsl", VALID_CUSTOM_WGSL),
            Ok(()),
            "the shader shape custom pipelines are built from must pass \
             validation; if it does not, hot reload rejects every edit"
        );
    }

    #[test]
    fn terrain_wgsl_is_valid() {
        assert_eq!(
            WgpuSurface::validate_wgsl("terrain.wgsl", TERRAIN_WGSL),
            Ok(()),
            "TERRAIN_WGSL must pass the same validation MESH_WGSL does"
        );
    }

    // The message has to be good enough to fix the shader from, not just
    // "error": this fires while someone is mid-edit, and the log line is all
    // they get. Asserts on the concrete line:column and the offending token so
    // a regression to a bare "invalid shader" string fails here.
    #[test]
    fn validate_wgsl_rejects_a_syntax_error_naming_line_column_and_token() {
        let broken = r#"
@fragment fn fs_main() -> @location(0) vec4<f32> {
    return vec4<f32>(1.0, 0.0, 0.0 1.0);
}
"#;
        let err = WgpuSurface::validate_wgsl("assets/broken.wgsl", broken)
            .expect_err("a missing argument separator is not valid WGSL");
        assert!(
            err.contains("assets/broken.wgsl:3:36"),
            "the error must locate the mistake by path, line and column; got: {err}"
        );
        assert!(
            err.contains("expected ')'"),
            "the error must say what was expected where; got: {err}"
        );
    }

    // The half that `naga::front::wgsl::parse_str` alone would miss, and the
    // reason `validate_wgsl` runs `Validator` as well: all three of these parse
    // cleanly and are rejected only by validation -- which is precisely what
    // `create_shader_module` runs on the device, where a rejection panics the
    // process. Delete the `Validator` call and this test fails.
    #[test]
    fn validate_wgsl_rejects_what_parsing_alone_accepts() {
        let vertex_without_position = r#"
@vertex fn vs_main() -> @location(0) vec4<f32> {
    return vec4<f32>(0.0, 0.0, 0.0, 1.0);
}
"#;
        let wrong_return_type = r#"
@fragment fn fs_main() -> @location(0) vec4<f32> {
    return 1.0;
}
"#;
        let conflicting_bindings = r#"
@group(0) @binding(0) var<uniform> a: vec4<f32>;
@group(0) @binding(0) var<uniform> b: vec4<f32>;
@fragment fn fs_main() -> @location(0) vec4<f32> {
    return a + b;
}
"#;
        for (src, expected_fragment) in [
            (vertex_without_position, "@builtin(position)"),
            (
                wrong_return_type,
                "does not match the function return value",
            ),
            (conflicting_bindings, "conflict with other resource"),
        ] {
            assert!(
                wgpu::naga::front::wgsl::parse_str(src).is_ok(),
                "precondition: this source is supposed to parse and fail only \
                 in validation, which is what makes it a test of the validator \
                 pass; it no longer parses: {src}"
            );
            let err = WgpuSurface::validate_wgsl("assets/broken.wgsl", src)
                .expect_err("the validator rejects this, so validate_wgsl must too");
            assert!(
                err.contains(expected_fragment),
                "expected the validator's reason ({expected_fragment}) in: {err}"
            );
        }
    }

    // `compile_and_store_shader` cannot be called without a real
    // `WgpuSurface` (which needs a real winit window -- see
    // `compile_pending_shaders_runs_before_render_frame` in bsengine-render),
    // so this pins the mechanism it relies on at the level a test can reach: a
    // validation error scope turns a device rejection into a returned error
    // instead of the default uncaptured-error handler's panic. Without the
    // scope this test aborts the process rather than failing.
    #[test]
    fn a_validation_error_scope_captures_a_device_rejection_instead_of_panicking() {
        let (device, _queue) = pollster::block_on(WgpuSurface::headless_device_for_testing());

        device.push_error_scope(wgpu::ErrorFilter::Validation);
        let _module = WgpuSurface::compile_shader(
            &device,
            "@fragment fn fs_main() -> @location(0) vec4<f32> { return vec4<f32>(0.0 1.0); }",
        );
        let err = pollster::block_on(device.pop_error_scope());
        assert!(
            err.is_some(),
            "a broken shader must surface as a captured error; None means the \
             rejection went to the uncaptured handler, which panics"
        );

        device.push_error_scope(wgpu::ErrorFilter::Validation);
        let _module = WgpuSurface::compile_shader(&device, VALID_CUSTOM_WGSL);
        assert!(
            pollster::block_on(device.pop_error_scope()).is_none(),
            "a valid shader must leave the scope empty; otherwise every reload \
             would be treated as a failure and no pipeline would ever update"
        );
    }

    #[test]
    fn shadow_shader_compiles() {
        let (device, _queue) = pollster::block_on(WgpuSurface::headless_device_for_testing());
        let _module = WgpuSurface::compile_shader(&device, SHADOW_WGSL);
    }

    #[test]
    fn point_shadow_shader_compiles() {
        let (device, _queue) = pollster::block_on(WgpuSurface::headless_device_for_testing());
        let _module = WgpuSurface::compile_shader(&device, POINT_SHADOW_WGSL);
    }

    #[test]
    fn probe_capture_wgsl_is_valid() {
        assert_eq!(
            WgpuSurface::validate_wgsl("probe_capture.wgsl", PROBE_CAPTURE_WGSL),
            Ok(()),
            "PROBE_CAPTURE_WGSL must pass the same validation MESH_WGSL does"
        );
    }

    #[test]
    fn probe_capture_sky_wgsl_is_valid() {
        assert_eq!(
            WgpuSurface::validate_wgsl("probe_capture_sky.wgsl", PROBE_CAPTURE_SKY_WGSL),
            Ok(()),
            "PROBE_CAPTURE_SKY_WGSL must pass the same validation SKYBOX_WGSL does"
        );
    }

    /// The capture shades **direct light only**, and this is the guard that
    /// keeps it that way. Sampling IBL (or, later, probes) from the capture
    /// would make the bake feed on its own output: probes lit by probes, and
    /// sky light counted twice over -- once as the captured background and
    /// again as the irradiance convolved from that same sky. The result is
    /// meant to be exactly one bounce.
    ///
    /// A source-level assertion rather than a rendered one because the failure
    /// it guards against is *plausible-looking*: an IBL-contaminated bake is
    /// brighter, not obviously broken, and no pixel test would flag it.
    #[test]
    fn the_probe_capture_shader_never_reads_the_ibl_resources() {
        for forbidden in [
            "irradiance_cube",
            "prefilter_cube",
            "brdf_lut",
            "ibl_sampler",
            // The branch itself, not the struct field: `ibl_enabled` must
            // still be *declared*, since this shader shares `light_buffer`
            // with MESH_WGSL and the layouts have to match byte for byte.
            "light.ibl_enabled",
        ] {
            assert!(
                !PROBE_CAPTURE_WGSL.contains(forbidden),
                "the probe capture shader references `{forbidden}`; capturing \
                 IBL into a probe makes the bake recursive"
            );
        }
        assert!(
            PROBE_CAPTURE_WGSL.contains("ibl_enabled: u32,"),
            "LightUniform must still declare ibl_enabled: this shader reads the \
             same buffer MESH_WGSL does, and a shorter struct would misalign \
             every field after it"
        );
    }

    /// Inside a volume the probe irradiance **replaces** the ambient/IBL
    /// irradiance; it must never be added to it.
    ///
    /// The probes captured the real scene including its skybox background and
    /// its flat ambient term, so their SH already carries both. Adding would
    /// count sky light twice and wash the scene out -- and, like the recursive
    /// bake this file's other source-level guard catches, the failure is merely
    /// *brighter* rather than obviously broken, so no pixel test would flag it.
    #[test]
    fn the_scene_shader_replaces_the_ibl_irradiance_inside_a_probe_volume() {
        assert!(
            MESH_WGSL.contains("ambient_term = probe_irradiance * albedo * kd_ibl + specular_ibl;"),
            "the probe branch must assign `ambient_term`, replacing whatever the \
             ambient/IBL path produced"
        );
        for adding in ["ambient_term +=", "ambient_term = ambient_term +"] {
            assert!(
                !MESH_WGSL.contains(adding),
                "`{adding}` accumulates onto the ambient term; probe irradiance \
                 already contains the sky, so adding double-counts it"
            );
        }
    }

    /// The shader's coefficient array is a hand-written literal, so nothing but
    /// this ties it to `MAX_PROBES`. Raising `MAX_PROBES` without raising it
    /// would leave the upper probes writing past the end of the uniform.
    #[test]
    fn the_scene_shader_probe_array_is_sized_from_max_probes() {
        let expected = format!(
            "array<vec4<f32>, {}>",
            bsengine_core::MAX_PROBES * crate::sh::SH_COEFF_COUNT
        );
        assert!(
            MESH_WGSL.contains(&expected),
            "MESH_WGSL must declare `{expected}`; MAX_PROBES is \
             {} and there are {} coefficients per probe",
            bsengine_core::MAX_PROBES,
            crate::sh::SH_COEFF_COUNT
        );
        // The shader's own out-of-range clamp, which cannot be written in terms
        // of a Rust constant.
        assert!(
            MESH_WGSL.contains(&format!("min(idx, {}u)", bsengine_core::MAX_PROBES - 1)),
            "the shader's probe-index clamp must be MAX_PROBES - 1"
        );
    }

    /// Closes the loop on the per-texel direction: for each face, take the
    /// direction `probe_face_texel_direction` reports for a texel, project it
    /// through the **actual** matrix that face was rendered with, and require
    /// the result to land back on that texel's NDC.
    ///
    /// Deliberately uses off-centre texels including the corners. A texel at
    /// the face centre projects to NDC (0, 0) under any axis swap or sign
    /// error, so a centre-only check certifies nothing.
    #[test]
    fn probe_face_texel_direction_matches_the_capture_matrix_ndc() {
        let probe_pos = Vec3::new(2.0, -1.0, 4.0);
        let view_projs = point_light_face_view_projs(probe_pos, PROBE_CAPTURE_RANGE);
        let n = PROBE_FACE_SIZE;
        let texels = [
            (0, 0),
            (n - 1, 0),
            (0, n - 1),
            (n - 1, n - 1),
            (n / 2, n / 2),
            (3, n - 4),
            (n - 2, 5),
        ];
        for (face, vp) in view_projs.iter().enumerate() {
            for (x, y) in texels {
                let dir = probe_face_texel_direction(face, x, y);
                assert!(
                    (dir.length() - 1.0).abs() < 1e-5,
                    "face {face} texel ({x},{y}): direction must be unit length, got {dir:?}"
                );
                // A point out along the ray, from the probe. Any positive
                // distance inside the frustum projects to the same NDC.
                let world = probe_pos + dir * 10.0;
                let clip = *vp * world.extend(1.0);
                let got = (clip.x / clip.w, clip.y / clip.w);
                let want = probe_face_texel_ndc(x, y);
                assert!(
                    (got.0 - want.0).abs() < 1e-4 && (got.1 - want.1).abs() < 1e-4,
                    "face {face} texel ({x},{y}): the direction we attribute this \
                     texel to projects to NDC {got:?}, but the texel sits at \
                     {want:?} -- the capture and the projection disagree about \
                     which way this texel looks"
                );
            }
        }
    }

    /// The six faces' texels must tile the whole sphere exactly once. This is
    /// what makes the projection an integral over the sphere rather than an
    /// arbitrarily scaled sum, and it is the assertion a wrong normalisation
    /// cannot survive.
    #[test]
    fn probe_face_texel_solid_angles_sum_to_the_full_sphere() {
        let mut total = 0.0f64;
        for _face in 0..6 {
            for y in 0..PROBE_FACE_SIZE {
                for x in 0..PROBE_FACE_SIZE {
                    total += probe_face_texel_solid_angle(x, y) as f64;
                }
            }
        }
        let four_pi = 4.0 * std::f64::consts::PI;
        assert!(
            (total - four_pi).abs() < 0.02,
            "the six faces must cover 4*pi steradians, got {total} (want {four_pi})"
        );
    }

    /// The specific mistake the projected solid angle exists to avoid: a flat
    /// `4*pi / (6*n*n)` per texel. It sums to 4*pi too, so the test above
    /// would not catch it -- what distinguishes the two is that a real cube
    /// face's corner texels are further away and seen edge-on, so they cover
    /// *less* sky than the centre ones, not the same amount.
    #[test]
    fn a_corner_texel_covers_less_sky_than_a_centre_texel() {
        let n = PROBE_FACE_SIZE;
        let centre = probe_face_texel_solid_angle(n / 2, n / 2);
        let corner = probe_face_texel_solid_angle(0, 0);
        assert!(
            corner < centre,
            "a corner texel ({corner}) must subtend less solid angle than a \
             centre texel ({centre}); equal values mean the flat \
             4*pi/(6*n*n) shortcut crept back in and the face corners are \
             being over-weighted"
        );
        let flat = 4.0 * std::f32::consts::PI / (6.0 * (n * n) as f32);
        assert!(
            corner < flat * 0.75,
            "the corner texel's real solid angle ({corner}) should be well \
             below the flat share ({flat}); if it is not, the foreshortening \
             factor is not being applied"
        );
    }

    #[test]
    fn f16_decodes_normal_subnormal_and_special_values() {
        assert_eq!(f16_bits_to_f32(0x0000), 0.0);
        assert_eq!(f16_bits_to_f32(0x3C00), 1.0);
        assert_eq!(f16_bits_to_f32(0xBC00), -1.0);
        assert_eq!(f16_bits_to_f32(0x4000), 2.0);
        assert!((f16_bits_to_f32(0x3555) - 1.0 / 3.0).abs() < 1e-3);
        // Largest subnormal: 1023 * 2^-24.
        assert!((f16_bits_to_f32(0x03FF) - 1023.0 * 2.0f32.powi(-24)).abs() < 1e-12);
        assert!(f16_bits_to_f32(0x7C00).is_infinite());
        assert!(f16_bits_to_f32(0x7E00).is_nan());
    }

    /// One emissive quad, standing off-axis, plus a probe at the origin --
    /// everything a bake needs and nothing else.
    ///
    /// The scene is deliberately minimal: the directional light and the
    /// ambient term are both black, so the *only* radiance in the capture is
    /// the quad's emissive and the pass's own background clear. That makes
    /// the l=1 coefficients an almost pure readout of where the quad is.
    struct ProbeBakeScene {
        surface: WgpuSurface,
        registry: crate::mesh::GpuMeshRegistry,
        draw_calls: Vec<(u64, Mat4, Option<u64>, MaterialParams, Option<String>)>,
    }

    impl ProbeBakeScene {
        /// `corners` are the quad's four vertices in counter-clockwise order
        /// *as seen from the origin*, so the front face points at the probe
        /// and back-face culling does not swallow it.
        fn new(corners: [Vec3; 4], emissive: Vec3) -> Self {
            Self::build(corners, emissive, false)
        }

        fn build(corners: [Vec3; 4], emissive: Vec3, fast_render: bool) -> Self {
            let surface = pollster::block_on(WgpuSurface::new_offscreen(64, 64, fast_render))
                .expect("an offscreen surface is what every GPU test here uses");
            let mut registry = crate::mesh::GpuMeshRegistry::new(surface.device.clone());
            let normal = (corners[1] - corners[0])
                .cross(corners[2] - corners[0])
                .normalize();
            let vertices: Vec<crate::mesh::Vertex> = corners
                .iter()
                .map(|p| crate::mesh::Vertex {
                    position: p.to_array(),
                    color: [1.0, 1.0, 1.0],
                    normal: normal.to_array(),
                    uv: [0.0, 0.0],
                })
                .collect();
            let mesh_id = registry.register(&vertices, &[0, 1, 2, 0, 2, 3]);
            let draw_calls = vec![(
                mesh_id,
                Mat4::IDENTITY,
                None,
                MaterialParams {
                    metallic: 0.0,
                    roughness: 1.0,
                    emissive,
                    // Black albedo: nothing reflects, so the quad contributes
                    // its emissive and only its emissive.
                    base_color: Vec3::ZERO,
                    opacity: 1.0,
                },
                None,
            )];
            let scene = Self {
                surface,
                registry,
                draw_calls,
            };
            scene.upload_uniforms();
            scene
        }

        /// `capture_probes` reads `model_buffer` and `light_buffer` rather
        /// than writing them -- `render_frame` fills both before any pass runs.
        /// A test that skips this bakes whatever was left in GPU memory.
        fn upload_uniforms(&self) {
            for (i, (_, model, _, mat, _)) in self.draw_calls.iter().enumerate() {
                let data = ModelUniformData {
                    model: model.to_cols_array_2d(),
                    metallic: mat.metallic,
                    roughness: mat.roughness,
                    _pad0: 0.0,
                    _pad1: 0.0,
                    emissive: mat.emissive.to_array(),
                    _pad2: 0.0,
                    base_color: mat.base_color.to_array(),
                    opacity: mat.opacity,
                };
                self.surface.queue.write_buffer(
                    &self.surface.model_buffer,
                    i as u64 * MODEL_STRIDE,
                    bytemuck::cast_slice(&[data]),
                );
            }
            // Zeroed: no point or spot lights, no IBL, black sun, black
            // ambient. `direction` still has to be a real unit vector because
            // the shader normalizes it.
            let mut light_data: LightUniformData = bytemuck::Zeroable::zeroed();
            light_data.direction = [0.0, -1.0, 0.0];
            self.surface.queue.write_buffer(
                &self.surface.light_buffer,
                0,
                bytemuck::cast_slice(&[light_data]),
            );
        }

        fn bake(&self, positions: &[Vec3]) -> Vec<crate::sh::ShL2> {
            let mut encoder =
                self.surface
                    .device
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                        label: Some("probe bake test encoder"),
                    });
            self.surface.capture_probes(
                &mut encoder,
                // No shadow map has been rendered, and the directional light
                // is black anyway; any matrix that keeps `shadow_factor`'s
                // lookup in range would do.
                Mat4::orthographic_rh(-20.0, 20.0, -20.0, 20.0, 0.1, 60.0)
                    * Mat4::look_at_rh(Vec3::new(0.0, 20.0, 0.0), Vec3::ZERO, Vec3::Z),
                &self.draw_calls,
                &self.registry,
                None,
                positions,
            );
            self.surface.queue.submit(std::iter::once(encoder.finish()));
            self.surface.project_captures_to_sh(positions.len())
        }

        /// One real `render_frame`, with everything this scene does not care
        /// about at its neutral value. The `Result` is returned rather than
        /// swallowed so a wgpu validation error fails the test instead of
        /// becoming a log line.
        fn render(
            &mut self,
            volume: Option<ProbeVolumeParams>,
        ) -> Result<std::collections::HashSet<String>, String> {
            let ui_state = bsengine_core::UiState::default();
            self.surface.render_frame(
                Mat4::IDENTITY,
                Vec3::new(0.0, 0.0, 5.0),
                Mat4::IDENTITY,
                None,
                &self.draw_calls,
                &[],
                0,
                &self.registry,
                LightData::default(),
                None,
                &std::collections::HashMap::new(),
                &ui_state,
                0.0,
                0.0,
                false,
                false,
                Mat4::IDENTITY,
                None,
                None,
                None,
                None,
                &[],
                false,
                false,
                false,
                None,
                None,
                0.0,
                &[],
                None,
                (0.0, 0.0),
                0,
                Mat4::IDENTITY,
                volume,
                None,
            )
        }

        /// Repaints the quad, so a later frame's capture would differ from an
        /// earlier one's -- the lever that makes "it did not re-bake"
        /// observable rather than merely asserted about a cached field.
        fn set_emissive(&mut self, emissive: Vec3) {
            self.draw_calls[0].3.emissive = emissive;
        }
    }

    /// A test volume big enough to hold `ProbeBakeScene`'s quad.
    fn test_volume(origin: Vec3) -> ProbeVolumeParams {
        ProbeVolumeParams {
            origin,
            extent: Vec3::splat(8.0),
            resolution: [2, 2, 2],
        }
    }

    fn a_red_quad_scene() -> ProbeBakeScene {
        ProbeBakeScene::new(
            [
                Vec3::new(3.0, 0.75, 0.0),
                Vec3::new(3.0, 0.75, 1.5),
                Vec3::new(3.0, 2.25, 1.5),
                Vec3::new(3.0, 2.25, 0.0),
            ],
            Vec3::new(20.0, 0.0, 0.0),
        )
    }

    /// The direction an L2 expansion attributes its brightest lobe to, read
    /// out of the l=1 band of one colour channel. `sh_basis` orders l=1 as
    /// `(y, z, x)`, so this un-permutes it.
    fn dominant_direction_red(sh: &crate::sh::ShL2) -> Vec3 {
        Vec3::new(sh.coeffs[3].x, sh.coeffs[1].x, sh.coeffs[2].x).normalize()
    }

    /// The end-to-end version of `probe_face_texel_direction_matches_...`:
    /// render a real emissive quad, project the real capture, and require the
    /// reconstructed lobe to point at where the quad actually is.
    ///
    /// Two placements, on two different cube faces, because face 0 (+X, up
    /// -Y) and face 2 (+Y, up +Z) use different up-vectors -- a bug that
    /// permutes faces or flips one face's V axis survives a single-face test.
    /// Both quads are off-axis in two coordinates at once, so no axis swap can
    /// hide.
    #[test]
    fn a_baked_probe_points_its_brightest_lobe_at_the_emissive_quad() {
        let cases: [([Vec3; 4], &str); 2] = [
            (
                [
                    Vec3::new(3.0, 0.75, 0.0),
                    Vec3::new(3.0, 0.75, 1.5),
                    Vec3::new(3.0, 2.25, 1.5),
                    Vec3::new(3.0, 2.25, 0.0),
                ],
                "+X face",
            ),
            (
                [
                    Vec3::new(0.75, 3.0, 0.0),
                    Vec3::new(2.25, 3.0, 0.0),
                    Vec3::new(2.25, 3.0, 1.5),
                    Vec3::new(0.75, 3.0, 1.5),
                ],
                "+Y face",
            ),
        ];
        for (corners, label) in cases {
            let centre = (corners[0] + corners[1] + corners[2] + corners[3]) / 4.0;
            let expected = centre.normalize();
            let scene = ProbeBakeScene::new(corners, Vec3::new(20.0, 0.0, 0.0));
            let sh = scene.bake(&[Vec3::ZERO]);
            assert_eq!(sh.len(), 1);

            // The pass's own 0.08 grey background alone integrates to
            // 0.282095 * 4*pi * 0.08 ~= 0.28 in *every* channel, so "is
            // anything there?" has to be asked as "is red above grey?", not
            // as an absolute floor. A capture that drew no geometry leaves
            // these two equal.
            assert!(
                sh[0].coeffs[0].x > sh[0].coeffs[0].y * 2.0 && sh[0].coeffs[0].x > 1.0,
                "{label}: l=0 is ({}, {}) -- red barely exceeds green, so the \
                 red quad was never rendered into the probe and all that was \
                 captured is the grey background",
                sh[0].coeffs[0].x,
                sh[0].coeffs[0].y
            );
            let got = dominant_direction_red(&sh[0]);
            assert!(
                got.dot(expected) > 0.9,
                "{label}: the probe's brightest red direction is {got:?} but the \
                 quad is at {expected:?} (dot {}). A probe lit from the wrong \
                 side still looks like lighting, which is why this is asserted \
                 rather than eyeballed",
                got.dot(expected)
            );
            // The quad is pure red; the only other radiance is the grey
            // background clear, which is isotropic and so cancels in l=1.
            assert!(
                sh[0].coeffs[3].x.abs() > sh[0].coeffs[3].y.abs() * 3.0,
                "{label}: the red channel must carry the directional signal, \
                 not the grey background -- got red {} vs green {}",
                sh[0].coeffs[3].x,
                sh[0].coeffs[3].y
            );
        }
    }

    /// CI's headless replays re-render the same scenes and compare pixels, so
    /// a bake that wobbled between runs would make every downstream test
    /// flaky rather than failing here.
    #[test]
    fn baking_the_same_scene_twice_gives_the_same_coefficients() {
        let scene = ProbeBakeScene::new(
            [
                Vec3::new(3.0, 0.75, 0.0),
                Vec3::new(3.0, 0.75, 1.5),
                Vec3::new(3.0, 2.25, 1.5),
                Vec3::new(3.0, 2.25, 0.0),
            ],
            Vec3::new(20.0, 8.0, 2.0),
        );
        // More than one probe, and none of them at the origin: a bake that
        // reused one probe's capture for all of them, or that mixed up the
        // layer indices, would still pass a single-probe check.
        let positions = [
            Vec3::ZERO,
            Vec3::new(0.5, -0.5, 0.25),
            Vec3::new(-1.0, 0.75, 1.5),
        ];
        let first = scene.bake(&positions);
        let second = scene.bake(&positions);
        assert_eq!(first.len(), positions.len());
        assert_eq!(
            first, second,
            "two bakes of one unchanged scene must produce identical \
             coefficients; CI's replay tests depend on that reproducibility"
        );
        // A determinism check passes trivially if both bakes are all zeros.
        assert!(
            first.iter().any(|sh| sh.coeffs[0].length() > 0.5),
            "the bake produced nothing to compare -- every probe's l=0 \
             coefficient is ~0, so this test would pass on a no-op capture"
        );
        // The probes sit at different points and the quad is close, so their
        // coefficients must actually differ from each other.
        assert_ne!(
            first[0], first[1],
            "two probes at different positions saw identical radiance, which \
             means the per-probe view-projections are not being applied"
        );
    }

    /// Probes sit on the grid's lattice points and are indexed x-fastest, and
    /// the scene shader's `probe_coeff_base` hardcodes that same order. Nothing
    /// but this ties the two together -- swap them and every probe would be
    /// read from the wrong lattice corner, which still looks like lighting.
    #[test]
    fn probe_positions_sit_on_the_lattice_corners_of_the_box() {
        let params = ProbeVolumeParams {
            origin: Vec3::new(-2.0, 1.0, 5.0),
            extent: Vec3::new(4.0, 6.0, 10.0),
            // Deliberately unequal per axis: an implementation that walked the
            // grid in the wrong nesting order still passes on a cube.
            resolution: [2, 3, 4],
        };
        let positions = probe_positions(&params);
        assert_eq!(positions.len(), 2 * 3 * 4);
        assert_eq!(
            positions[0], params.origin,
            "the first probe must sit exactly on the box's minimum corner"
        );
        assert_eq!(
            positions[positions.len() - 1],
            params.origin + params.extent,
            "the last probe must sit exactly on the box's maximum corner -- \
             probes on cell centres would leave the outer half-cell of the \
             volume extrapolating"
        );
        // x fastest, then y, then z: index (z*ry + y)*rx + x, exactly what
        // `probe_coeff_base` computes in MESH_WGSL.
        let [rx, ry, _rz] = params.resolution;
        for (i, p) in positions.iter().enumerate() {
            let x = i as u32 % rx;
            let y = (i as u32 / rx) % ry;
            let z = i as u32 / (rx * ry);
            let expected = params.origin
                + params.extent
                    * Vec3::new(
                        x as f32 / (rx - 1) as f32,
                        y as f32 / (ry - 1) as f32,
                        z as f32 / 3.0,
                    );
            assert!(
                (*p - expected).length() < 1e-5,
                "probe {i} is at {p:?} but x-fastest ordering puts it at \
                 {expected:?}"
            );
        }
    }

    /// "Automatic bake once after load", as actually enforced: a volume is
    /// baked when it appears, is **not** re-baked while it is unchanged even
    /// though the scene inside it moved, is re-baked when the volume itself
    /// changes, and is replaced by a disabled grid when it goes away.
    ///
    /// The middle case is the one worth writing a GPU test for. It is the
    /// documented cost of baking once -- move a wall after load and the floor
    /// keeps the old colour -- and it is asserted here by repainting the quad
    /// between two frames and requiring the uploaded coefficients not to
    /// budge. Asserting on `baked_probe_volume` alone would only restate the
    /// cache key back to itself.
    #[test]
    fn a_volume_is_baked_once_and_rebaked_only_when_the_volume_itself_changes() {
        let mut scene = a_red_quad_scene();
        assert_eq!(scene.surface.baked_probe_volume, None);
        assert_eq!(
            scene.surface.read_probe_uniform().enabled,
            0,
            "a fresh surface must start with probes disabled, or every scene \
             that never places a volume would be lit by whatever the buffer \
             happened to contain"
        );

        let volume = test_volume(Vec3::splat(-4.0));
        scene.render(Some(volume)).expect(
            "a frame with a probe volume must render without a \
                     validation error",
        );
        assert_eq!(scene.surface.baked_probe_volume, Some(volume));
        let baked = scene.surface.read_probe_uniform();
        assert_eq!(baked.enabled, 1);
        assert_eq!(baked.origin, volume.origin.to_array());
        assert_eq!(baked.extent, volume.extent.to_array());
        assert_eq!(baked.resolution, volume.resolution);
        assert!(
            baked.coeffs[..8]
                .iter()
                .all(|probe| probe[0][0].abs() > 1e-4),
            "every probe of a 2x2x2 grid must have captured something; an \
             all-zero l=0 means the bake ran but drew nothing"
        );

        // Same volume, different scene. The bake must not re-run.
        scene.set_emissive(Vec3::new(0.0, 20.0, 0.0));
        scene
            .render(Some(volume))
            .expect("a second frame must render");
        let after = scene.surface.read_probe_uniform();
        assert_eq!(
            after.coeffs[0], baked.coeffs[0],
            "the volume did not change, so no re-bake should have happened -- \
             repainting the quad red-to-green must leave the baked \
             coefficients exactly as they were"
        );

        // Moving the volume is a change to the volume, so it does re-bake --
        // and now picks up the green quad.
        let moved = test_volume(Vec3::splat(-3.0));
        scene
            .render(Some(moved))
            .expect("a third frame must render");
        assert_eq!(scene.surface.baked_probe_volume, Some(moved));
        let rebaked = scene.surface.read_probe_uniform();
        assert_eq!(rebaked.origin, moved.origin.to_array());
        assert!(
            rebaked.coeffs[0][0][1] > rebaked.coeffs[0][0][0],
            "the re-bake must see the *current* scene: the quad is green now, \
             so probe 0's l=0 should be green-dominant, got {:?}",
            rebaked.coeffs[0][0]
        );

        // Removing the volume uploads the disabled grid rather than leaving
        // the last bake lighting the scene forever.
        scene.render(None).expect("a fourth frame must render");
        assert_eq!(scene.surface.baked_probe_volume, None);
        let cleared = scene.surface.read_probe_uniform();
        assert_eq!(cleared.enabled, 0);
        assert_eq!(cleared.coeffs[0][0], [0.0; 4]);
    }

    /// `fast_render` -- the mode CI's headless replays use -- must never bake.
    /// A bake is up to 192 render passes and a synchronous readback, which is
    /// exactly the expensive work that mode exists to skip; the neutral result
    /// is a disabled grid, the same way the shadow pass still clears its map
    /// to "nothing occludes anything".
    #[test]
    fn a_fast_render_surface_never_bakes_a_probe_volume() {
        let mut scene = ProbeBakeScene::build(
            [
                Vec3::new(3.0, 0.75, 0.0),
                Vec3::new(3.0, 0.75, 1.5),
                Vec3::new(3.0, 2.25, 1.5),
                Vec3::new(3.0, 2.25, 0.0),
            ],
            Vec3::new(20.0, 0.0, 0.0),
            true,
        );
        scene
            .render(Some(test_volume(Vec3::splat(-4.0))))
            .expect("a fast_render frame with a volume must still render");
        assert_eq!(
            scene.surface.baked_probe_volume, None,
            "fast_render must leave the surface with nothing baked, so the \
             next frame does not try again"
        );
        assert_eq!(
            scene.surface.read_probe_uniform().enabled,
            0,
            "fast_render skips the bake, so probes must be disabled rather \
             than enabled over zeroed coefficients"
        );
    }
}
