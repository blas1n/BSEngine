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
    /// Whether it casts a shadow (`RectLight::shadows`).
    pub shadows: bool,
    /// Its direct light is in the lightmaps (`LightBakeMode::Baked`): a
    /// lightmapped draw skips it, everything else is lit by it as usual.
    pub baked: bool,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct RectLightGpu {
    position: [f32; 3],
    range: f32,
    half_width: [f32; 3],
    /// The light's shadow map layer (`crate::spot_shadow`).
    shadow_layer: f32,
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
    /// Bit `i` set when rect light `i` is baked; a lightmapped draw skips it.
    baked_mask: u32,
    _pad: [u32; 2],
    lights: [RectLightGpu; MAX_RECT_LIGHTS],
}

impl RectLightUniform {
    /// The first [`MAX_RECT_LIGHTS`] of `entries`.
    pub(crate) fn new(entries: &[RectLightEntry]) -> Self {
        let mut out = <Self as bytemuck::Zeroable>::zeroed();
        for (i, (slot, e)) in out.lights.iter_mut().zip(entries).enumerate() {
            *slot = RectLightGpu {
                position: e.position.to_array(),
                range: e.range,
                half_width: e.half_width.to_array(),
                // -1 for none: the shader's test, as the spot lights' is.
                shadow_layer: if e.shadows {
                    (crate::spot_shadow::RECT_SHADOW_BASE + i) as f32
                } else {
                    -1.0
                },
                half_height: e.half_height.to_array(),
                _p1: 0.0,
                radiance: (e.color * e.intensity).to_array(),
                _p2: 0.0,
            };
        }
        out.count = entries.len().min(MAX_RECT_LIGHTS) as u32;
        out.baked_mask = entries
            .iter()
            .take(MAX_RECT_LIGHTS)
            .enumerate()
            .fold(0, |m, (i, e)| m | (u32::from(e.baked) << i));
        out
    }
}

/// The two LTC tables, as the two layers of one array texture, and the
/// sampler that reads them.
///
/// One texture and not two because of WebGPU's 16 sampled textures per
/// shader stage, which a pipeline layout is held to across all its bind
/// groups: the terrain's pipeline -- group 2's lighting textures plus its own
/// five layer and weight textures -- reached 17 when spot shadows were
/// added, and the browser refused it. The tables share a size and a format,
/// so one array of two layers costs nothing. (Native wgpu checks each bind
/// group separately and accepted it; see
/// `surface::tests::every_pipeline_fits_webgpus_sampled_texture_limit`.)
pub(crate) struct LtcTables {
    _tables: crate::profiler::TrackedTexture,
    /// Layer 0 is the inverse matrix, layer 1 the magnitude and Fresnel.
    pub view: wgpu::TextureView,
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
                        depth_or_array_layers: 2,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Rgba16Float,
                    // COPY_SRC only so `each_table_is_its_own_layer` can read it back.
                    usage: wgpu::TextureUsages::TEXTURE_BINDING
                        | wgpu::TextureUsages::COPY_DST
                        | wgpu::TextureUsages::COPY_SRC,
                    view_formats: &[],
                },
                wgpu::util::TextureDataOrder::LayerMajor,
                data,
            )
        };
        // Layer-major: all of layer 0, then all of layer 1.
        let tables = table("ltc tables", &[LTC_1, LTC_2].concat());
        Self {
            view: tables.create_view(&wgpu::TextureViewDescriptor {
                label: Some("ltc tables view"),
                dimension: Some(wgpu::TextureViewDimension::D2Array),
                ..Default::default()
            }),
            _tables: tables,
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

/// The rect lights' uniform alone, at group 2 binding 14: what the
/// volumetric fog's injection pass shares with the scene shaders. Split from
/// [`rect_light_wgsl`] so the fog does not carry the LTC tables' bindings and
/// functions, which have no meaning without a surface.
macro_rules! rect_light_decl_wgsl {
    () => {
        r#"
struct RectLightEntry {
    position: vec3<f32>,
    range: f32,
    half_width: vec3<f32>,
    // Layer of the light's shadow map (`shadow_layer_lookup`).
    shadow_layer: f32,
    half_height: vec3<f32>,
    _p1: f32,
    radiance: vec3<f32>,
    _p2: f32,
};
struct RectLightUniform {
    count: u32,
    baked_mask: u32,
    _q1: u32,
    _q2: u32,
    lights: array<RectLightEntry, 4>,
};
@group(2) @binding(14) var<uniform> rect_lights: RectLightUniform;
"#
    };
}
pub(crate) use rect_light_decl_wgsl;

/// The rect lights' declarations and shading, appended to the mesh and
/// terrain shaders (a macro so `concat!` can join it to their literals).
/// `rect_light_radiance` returns what every rect light adds to a surface.
macro_rules! rect_light_wgsl {
    () => {
        concat!(
            $crate::area_light::rect_light_decl_wgsl!(),
            r#"
// Layer 0: the inverse LTC matrix; layer 1: magnitude and Fresnel.
@group(2) @binding(15) var ltc_tables: texture_2d_array<f32>;
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
    // Seen from behind its plane the rectangle winds the other way and the
    // sum comes out negative: clamping it is what makes the light
    // one-sided, as Unreal's and HDRP's are.
    return max(sum, 0.0);
}

// `skip` has bit `i` set for each panel this draw must leave out: the mesh
// shader passes the baked panels' bits for a lightmapped draw, every other
// caller 0.
fn rect_light_radiance(
    n: vec3<f32>, v: vec3<f32>, p: vec3<f32>,
    albedo: vec3<f32>, f0: vec3<f32>, metallic: f32, roughness: f32, soft: u32,
    skip: u32,
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
    let t1 = textureSampleLevel(ltc_tables, ltc_sampler, uv, 0, 0.0);
    let t2 = textureSampleLevel(ltc_tables, ltc_sampler, uv, 1, 0.0);
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
        if ((skip >> i) & 1u) != 0u {
            continue;
        }
        let rl = rect_lights.lights[i];
        let c0 = rl.position + rl.half_width - rl.half_height;
        let c1 = rl.position - rl.half_width - rl.half_height;
        let c2 = rl.position - rl.half_width + rl.half_height;
        let c3 = rl.position + rl.half_width + rl.half_height;
        // Faded to nothing at `range` from the centre, smoothly, by the
        // window Unreal and Frostbite put on their lights' attenuation radius.
        let d = length(rl.position - p) / max(rl.range, 1e-4);
        let w = clamp(1.0 - d * d * d * d, 0.0, 1.0);
        let window = w * w;
        if window <= 0.0 {
            continue;
        }
        // The panel's shadow, as its centre casts it (`rect_shadow_view_proj`).
        var lit = 1.0;
        if rl.shadow_layer >= 0.0 {
            lit = shadow_layer_lookup(i32(rl.shadow_layer), p, soft);
            if lit <= 0.0 {
                continue;
            }
        }
        let spec = ltc_evaluate(n, v, p, m_inv, c0, c1, c2, c3);
        let diff = ltc_evaluate(n, v, p, identity, c0, c1, c2, c3);
        sum += rl.radiance * (window * lit) * (fresnel * spec + diffuse * diff);
    }
    return sum;
}
"#
        )
    };
}
pub(crate) use rect_light_wgsl;

#[cfg(test)]
mod tests {
    use super::*;

    /// Each table is its own layer of the array, in order and intact: what
    /// the GPU holds at layer 0 is `ltc_1.bin` byte for byte, and at layer 1
    /// `ltc_2.bin`. Read back rather than rendered, because the rect light
    /// pixel tests do not tell the tables apart -- reading the matrix from
    /// the wrong layer passes all of them (a rough surface's matrix is close
    /// to the identity, and the glossy test checks only the highlight's
    /// shape).
    #[test]
    fn each_table_is_its_own_layer() {
        let (device, queue) =
            pollster::block_on(crate::surface::WgpuSurface::headless_device_for_testing());
        let tables = LtcTables::new(&device, &queue);
        let row = LTC_SIZE * 8; // Rgba16Float: 512 bytes, already 256-aligned.
        for (layer, expected) in [(0, LTC_1), (1, LTC_2)] {
            let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("ltc read back"),
                size: u64::from(row * LTC_SIZE),
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });
            let mut encoder = device.create_command_encoder(&Default::default());
            encoder.copy_texture_to_buffer(
                wgpu::ImageCopyTexture {
                    texture: &tables._tables,
                    mip_level: 0,
                    origin: wgpu::Origin3d {
                        x: 0,
                        y: 0,
                        z: layer,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::ImageCopyBuffer {
                    buffer: &buffer,
                    layout: wgpu::ImageDataLayout {
                        offset: 0,
                        bytes_per_row: Some(row),
                        rows_per_image: Some(LTC_SIZE),
                    },
                },
                wgpu::Extent3d {
                    width: LTC_SIZE,
                    height: LTC_SIZE,
                    depth_or_array_layers: 1,
                },
            );
            queue.submit(Some(encoder.finish()));
            let slice = buffer.slice(..);
            slice.map_async(wgpu::MapMode::Read, |_| {});
            device.poll(wgpu::Maintain::Wait);
            let got = slice.get_mapped_range().to_vec();
            assert_eq!(
                got.len(),
                expected.len(),
                "premise: one whole table per layer"
            );
            assert!(got == expected, "layer {layer} is not its table");
        }
    }
}
