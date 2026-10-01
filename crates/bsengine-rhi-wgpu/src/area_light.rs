//! Rectangular area lights, shaded with linearly transformed cosines (LTC).
//!
//! Heitz, Dupuy, Hill and Neubelt, "Real-Time Polygonal-Light Shading with
//! Linearly Transformed Cosines" (2016): the rectangle is carried into the
//! space where the GGX lobe at this pixel is a clamped cosine, clipped to the
//! horizon exactly (the paper's `ClipQuadToHorizon`), and its form factor
//! summed from the clipped polygon's edges. The edge integral, the Fresnel
//! split and the table layout follow three.js's `RectAreaLight` (MIT); its
//! horizon *approximation* (Hill 2016) is not used, being exact only for a
//! panel squarely facing the surface. Unreal's rect lights and HDRP's area
//! lights are LTC too.
//!
//! The two 64x64 tables are the authors' (BSD, `ltc/LICENSE-LTC.txt`), as
//! three.js embeds them (`ltc/LICENSE-three.js.txt`), stored as half floats:
//! a 32-bit float texture is not filterable in WebGPU, and the lookup is
//! bilinear.

use glam::Vec3;

/// How many rect lights a frame shades; the rest are ignored, as point and
/// spot lights past their caps are.
pub const MAX_RECT_LIGHTS: usize = 4;

/// The inverse LTC matrix per (roughness, angle): `(m00, m02, m20, m22)`.
const LTC_1: &[u8] = include_bytes!("ltc/ltc_1.bin");
/// The magnitude and Fresnel terms per (roughness, angle).
const LTC_2: &[u8] = include_bytes!("ltc/ltc_2.bin");
const LTC_SIZE: u32 = 64;

/// One rect light as the renderer takes it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RectLightEntry {
    /// World-space centre of the rectangle.
    pub position: Vec3,
    /// Half the rectangle along its width: the light's local x axis, in
    /// world space, scaled by half the width.
    pub half_width: Vec3,
    /// Half the rectangle along its height (the local y axis). The light
    /// shines out of the side `half_width x half_height` points away from:
    /// its local -z, as spot lights do.
    pub half_height: Vec3,
    /// Light colour.
    pub color: Vec3,
    /// The emitting surface's brightness: the radiance it sends out is
    /// `color * intensity`, so a larger light at the same intensity lights
    /// more.
    pub intensity: f32,
    /// Distance from the centre at which the light has faded to nothing.
    pub range: f32,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct RectLightGpu {
    position: [f32; 3],
    range: f32,
    half_width: [f32; 3],
    _p0: f32,
    half_height: [f32; 3],
    _p1: f32,
    radiance: [f32; 3],
    _p2: f32,
}

/// The uniform `RECT_LIGHT_WGSL` declares at group 2, binding 14.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct RectLightUniform {
    count: u32,
    _pad: [u32; 3],
    lights: [RectLightGpu; MAX_RECT_LIGHTS],
}

impl RectLightUniform {
    /// The first [`MAX_RECT_LIGHTS`] of `entries`.
    pub(crate) fn new(entries: &[RectLightEntry]) -> Self {
        let mut out = <Self as bytemuck::Zeroable>::zeroed();
        for (slot, e) in out.lights.iter_mut().zip(entries) {
            *slot = RectLightGpu {
                position: e.position.to_array(),
                range: e.range,
                half_width: e.half_width.to_array(),
                _p0: 0.0,
                half_height: e.half_height.to_array(),
                _p1: 0.0,
                radiance: (e.color * e.intensity).to_array(),
                _p2: 0.0,
            };
        }
        out.count = entries.len().min(MAX_RECT_LIGHTS) as u32;
        out
    }
}

/// The two LTC tables and the sampler that reads them.
pub(crate) struct LtcTables {
    _ltc_1: crate::profiler::TrackedTexture,
    pub ltc_1_view: wgpu::TextureView,
    _ltc_2: crate::profiler::TrackedTexture,
    pub ltc_2_view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
}

impl LtcTables {
    pub(crate) fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        let table = |label: &str, data: &[u8]| {
            crate::profiler::create_tracked_texture_with_data(
                device,
                queue,
                &wgpu::TextureDescriptor {
                    label: Some(label),
                    size: wgpu::Extent3d {
                        width: LTC_SIZE,
                        height: LTC_SIZE,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Rgba16Float,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                    view_formats: &[],
                },
                wgpu::util::TextureDataOrder::LayerMajor,
                data,
            )
        };
        let ltc_1 = table("ltc 1", LTC_1);
        let ltc_2 = table("ltc 2", LTC_2);
        Self {
            ltc_1_view: ltc_1.create_view(&Default::default()),
            _ltc_1: ltc_1,
            ltc_2_view: ltc_2.create_view(&Default::default()),
            _ltc_2: ltc_2,
            sampler: device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("ltc sampler"),
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            }),
        }
    }
}

/// The rect lights' declarations and shading, appended to the mesh and
/// terrain shaders (a macro so `concat!` can join it to their literals).
/// `rect_light_radiance` returns what every rect light adds to a surface.
macro_rules! rect_light_wgsl {
    () => {
        r#"
struct RectLightEntry {
    position: vec3<f32>,
    range: f32,
    half_width: vec3<f32>,
    _p0: f32,
    half_height: vec3<f32>,
    _p1: f32,
    radiance: vec3<f32>,
    _p2: f32,
};
struct RectLightUniform {
    count: u32,
    _q0: u32,
    _q1: u32,
    _q2: u32,
    lights: array<RectLightEntry, 4>,
};
@group(2) @binding(14) var<uniform> rect_lights: RectLightUniform;
@group(2) @binding(15) var ltc_1: texture_2d<f32>;
@group(2) @binding(16) var ltc_2: texture_2d<f32>;
@group(2) @binding(17) var ltc_sampler: sampler;

// Where in the tables a surface's lobe is: parameterised by perceptual
// roughness (sqrt of GGX alpha) and sqrt(1 - cos(theta_v)), at texel centres.
fn ltc_uv(n_dot_v: f32, roughness: f32) -> vec2<f32> {
    let size = 64.0;
    let uv = vec2<f32>(roughness, sqrt(1.0 - n_dot_v));
    return uv * ((size - 1.0) / size) + vec2<f32>(0.5 / size);
}

// One edge's share of the vector form factor, both ends on the unit sphere:
// a rational fit of theta / sin(theta) / 2pi.
fn ltc_edge(v1: vec3<f32>, v2: vec3<f32>) -> vec3<f32> {
    let x = dot(v1, v2);
    let y = abs(x);
    let a = 0.8543985 + (0.4965155 + 0.0145206 * y) * y;
    let b = 3.4175940 + (4.1616724 + y) * y;
    let v = a / b;
    var theta_sintheta = v;
    if x <= 0.0 {
        theta_sintheta = 0.5 * inverseSqrt(max(1.0 - x * x, 1e-7)) - v;
    }
    return cross(v1, v2) * theta_sintheta;
}

// A point where the edge `a -> b` crosses the horizon (z = 0): only called
// with `a` and `b` on opposite sides of it.
fn horizon_cut(a: vec3<f32>, b: vec3<f32>) -> vec3<f32> {
    return -a.z * b + b.z * a;
}

// The form factor of the rectangle `c0..c3` (counter-clockwise seen from
// the side it lights) through the lobe `m_inv`, seen from `p`.
//
// The rectangle is clipped to the horizon exactly -- Heitz et al.'s
// `ClipQuadToHorizon`, which leaves 0, 3, 4 or 5 corners -- and the clipped
// polygon's edges integrated. Not the sphere approximation three.js uses
// (Hill 2016): that one is exact only for a panel squarely facing the
// surface, and it put a third too much light on a wall from a panel
// half behind it.
fn ltc_evaluate(
    n: vec3<f32>, v: vec3<f32>, p: vec3<f32>, m_inv: mat3x3<f32>,
    c0: vec3<f32>, c1: vec3<f32>, c2: vec3<f32>, c3: vec3<f32>,
) -> f32 {
    // A frame around the normal with the view in its xz plane, which is how
    // the tables are oriented. Looking straight down the normal the view
    // gives no direction, and any tangent will do: the lobe is symmetric.
    var t1 = v - n * dot(v, n);
    if dot(t1, t1) < 1e-8 {
        t1 = cross(n, select(vec3<f32>(1.0, 0.0, 0.0), vec3<f32>(0.0, 1.0, 0.0), abs(n.x) > 0.9));
    }
    t1 = normalize(t1);
    let t2 = -cross(n, t1);
    let m = m_inv * transpose(mat3x3<f32>(t1, t2, n));
    var l0 = m * (c0 - p);
    var l1 = m * (c1 - p);
    var l2 = m * (c2 - p);
    var l3 = m * (c3 - p);
    var l4 = vec3<f32>(0.0);

    // Which corners are above the horizon, as four bits.
    var config = 0u;
    if l0.z > 0.0 { config += 1u; }
    if l1.z > 0.0 { config += 2u; }
    if l2.z > 0.0 { config += 4u; }
    if l3.z > 0.0 { config += 8u; }
    var count = 0u;
    switch config {
        case 1u: {
            count = 3u;
            l1 = horizon_cut(l1, l0);
            l2 = horizon_cut(l3, l0);
        }
        case 2u: {
            count = 3u;
            l0 = horizon_cut(l0, l1);
            l2 = horizon_cut(l2, l1);
        }
        case 3u: {
            count = 4u;
            l2 = horizon_cut(l2, l1);
            l3 = horizon_cut(l3, l0);
        }
        case 4u: {
            count = 3u;
            l0 = horizon_cut(l3, l2);
            l1 = horizon_cut(l1, l2);
        }
        case 6u: {
            count = 4u;
            l0 = horizon_cut(l0, l1);
            l3 = horizon_cut(l3, l2);
        }
        case 7u: {
            count = 5u;
            l4 = horizon_cut(l3, l0);
            l3 = horizon_cut(l3, l2);
        }
        case 8u: {
            count = 3u;
            l0 = horizon_cut(l0, l3);
            l1 = horizon_cut(l2, l3);
            l2 = l3;
        }
        case 9u: {
            count = 4u;
            l1 = horizon_cut(l1, l0);
            l2 = horizon_cut(l2, l3);
        }
        case 11u: {
            count = 5u;
            l4 = l3;
            l3 = horizon_cut(l2, l3);
            l2 = horizon_cut(l2, l1);
        }
        case 12u: {
            count = 4u;
            l1 = horizon_cut(l1, l2);
            l0 = horizon_cut(l0, l3);
        }
        case 13u: {
            count = 5u;
            l4 = l3;
            l3 = l2;
            l2 = horizon_cut(l1, l2);
            l1 = horizon_cut(l1, l0);
        }
        case 14u: {
            count = 5u;
            l4 = horizon_cut(l0, l3);
            l0 = horizon_cut(l0, l1);
        }
        case 15u: {
            count = 4u;
        }
        // 0 is wholly below the horizon; 5 and 10 cannot happen to a
        // rectangle (opposite corners up, the others down).
        default: {}
    }
    if count == 0u {
        return 0.0;
    }
    l0 = normalize(l0);
    l1 = normalize(l1);
    l2 = normalize(l2);
    var sum = ltc_edge(l0, l1).z + ltc_edge(l1, l2).z;
    if count == 3u {
        sum += ltc_edge(l2, l0).z;
    } else {
        l3 = normalize(l3);
        sum += ltc_edge(l2, l3).z;
        if count == 4u {
            sum += ltc_edge(l3, l0).z;
        } else {
            l4 = normalize(l4);
            sum += ltc_edge(l3, l4).z + ltc_edge(l4, l0).z;
        }
    }
    return max(sum, 0.0);
}

fn rect_light_radiance(
    n: vec3<f32>, v: vec3<f32>, p: vec3<f32>,
    albedo: vec3<f32>, f0: vec3<f32>, metallic: f32, roughness: f32,
) -> vec3<f32> {
    var sum = vec3<f32>(0.0);
    // Capped on the Rust side (`RectLightUniform::new`), the one place that
    // knows `MAX_RECT_LIGHTS`.
    let count = rect_lights.count;
    if count == 0u {
        return sum;
    }
    let n_dot_v = clamp(dot(n, v), 0.0, 1.0);
    let uv = ltc_uv(n_dot_v, roughness);
    let t1 = textureSampleLevel(ltc_1, ltc_sampler, uv, 0.0);
    let t2 = textureSampleLevel(ltc_2, ltc_sampler, uv, 0.0);
    let m_inv = mat3x3<f32>(
        vec3<f32>(t1.x, 0.0, t1.y),
        vec3<f32>(0.0, 1.0, 0.0),
        vec3<f32>(t1.z, 0.0, t1.w),
    );
    let identity = mat3x3<f32>(
        vec3<f32>(1.0, 0.0, 0.0),
        vec3<f32>(0.0, 1.0, 0.0),
        vec3<f32>(0.0, 0.0, 1.0),
    );
    // The lobe's magnitude and Fresnel split (Hill 2016).
    let fresnel = f0 * t2.x + (vec3<f32>(1.0) - f0) * t2.y;
    let diffuse = albedo * (1.0 - metallic);
    for (var i = 0u; i < count; i++) {
        let rl = rect_lights.lights[i];
        let c0 = rl.position + rl.half_width - rl.half_height;
        let c1 = rl.position - rl.half_width - rl.half_height;
        let c2 = rl.position - rl.half_width + rl.half_height;
        let c3 = rl.position + rl.half_width + rl.half_height;
        // One-sided, as Unreal's and HDRP's are: behind its plane it lights
        // nothing.
        if dot(cross(c1 - c0, c3 - c0), p - c0) < 0.0 {
            continue;
        }
        // Faded to nothing at `range` from the centre, smoothly, by the
        // window Unreal and Frostbite put on their lights' attenuation radius.
        let d = length(rl.position - p) / max(rl.range, 1e-4);
        let w = clamp(1.0 - d * d * d * d, 0.0, 1.0);
        let window = w * w;
        if window <= 0.0 {
            continue;
        }
        let spec = ltc_evaluate(n, v, p, m_inv, c0, c1, c2, c3);
        let diff = ltc_evaluate(n, v, p, identity, c0, c1, c2, c3);
        sum += rl.radiance * window * (fresnel * spec + diffuse * diff);
    }
    return sum;
}
"#
    };
}
pub(crate) use rect_light_wgsl;
