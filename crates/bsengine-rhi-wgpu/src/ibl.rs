//! Image-based lighting: turns the skybox into the diffuse irradiance and
//! prefiltered specular maps the PBR shader samples in place of a flat
//! ambient constant.
//!
//! Four GPU passes, on two different schedules:
//!   - The BRDF integration LUT depends only on the BRDF -- not the
//!     environment, scene, or camera -- so it is built once at startup and
//!     reused for every skybox thereafter.
//!   - Equirect->cubemap, irradiance, and prefilter all depend on the
//!     environment, so they rerun whenever the skybox changes.
//!
//! See `docs/superpowers/specs/2026-08-30-ibl-design.md`.

use crate::profiler::{create_tracked_texture, TrackedTexture};

/// Edge length of each face of the environment cubemap.
pub const ENV_CUBE_SIZE: u32 = 256;
/// Edge length of each irradiance cubemap face. Small deliberately:
/// irradiance is very low-frequency, so resolution buys nothing.
pub const IRRADIANCE_CUBE_SIZE: u32 = 32;
/// Edge length of the prefiltered cubemap's mip 0.
pub const PREFILTER_CUBE_SIZE: u32 = 128;
/// Mip levels in the prefiltered cubemap. Mip 0 is mirror-sharp; the last
/// is fully rough. The shader maps `roughness` linearly onto this range.
pub const PREFILTER_MIP_LEVELS: u32 = 5;
/// Edge length of the square BRDF integration LUT.
pub const BRDF_LUT_SIZE: u32 = 512;

/// Format of the BRDF integration LUT: two 16-bit floats holding the `f0`
/// scale (`.r`) and bias (`.g`). Two channels because that is all the
/// split-sum second term produces, and 16-bit float rather than 8-bit unorm
/// because the scale term is read directly as a multiplier, where banding
/// would show up as stepped reflectance.
pub const BRDF_LUT_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rg16Float;

/// Format of the environment cubemap and the maps convolved from it.
///
/// Half float rather than the 8-bit sRGB the skybox itself is stored in: these
/// are *linear* radiance values that the irradiance and prefilter passes then
/// sum thousands of samples of, and an 8-bit linear intermediate bands visibly
/// in the dark end of that sum. Filterable on every backend, so the cube views
/// still get hardware trilinear filtering across face seams.
pub const ENV_CUBE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

/// A cubemap has six faces; in wgpu they are six array layers of a 2D
/// texture, in the fixed order +X, -X, +Y, -Y, +Z, -Z.
const CUBE_FACES: u32 = 6;

/// Stride between the per-face uniform records the face passes index with a
/// dynamic offset. 256 rather than the record's actual size because that is
/// wgpu's minimum uniform-buffer offset alignment on every backend -- the same
/// convention `surface.rs` uses for `MODEL_STRIDE` and `POINT_SHADOW_STRIDE`.
const FACE_UNIFORM_STRIDE: u64 = 256;

/// Size of one face-uniform record: a single `u32` face index padded out to a
/// `vec4<u32>`, because WGSL's uniform layout rules round struct members up to
/// 16 bytes anyway.
const FACE_UNIFORM_SIZE: u64 = 16;

/// Creates a cube texture: `size` x `size`, six array layers, `mip_levels`
/// mips.
///
/// In wgpu a cubemap *is* a six-layer 2D texture -- the cube-ness lives on
/// the view ([`cube_view`]), not on the texture -- so callers that want to
/// render into one face at a time can just take a [`face_view`] of the same
/// texture.
///
/// Allocation goes through [`crate::profiler::create_tracked_texture`] so
/// this memory is counted in the profiler totals exactly like every other
/// render target this crate creates.
pub fn create_cubemap(
    device: &wgpu::Device,
    label: &str,
    size: u32,
    mip_levels: u32,
    format: wgpu::TextureFormat,
    usage: wgpu::TextureUsages,
) -> TrackedTexture {
    create_tracked_texture(
        device,
        &wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: CUBE_FACES,
            },
            mip_level_count: mip_levels,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage,
            view_formats: &[],
        },
    )
}

/// A `TextureViewDimension::Cube` view over all six faces and every mip, for
/// binding as a `texture_cube<f32>` and sampling by direction.
///
/// The point-light cube shadows in `surface.rs` deliberately do *not* do this
/// -- they bind a `texture_2d_array` and pick the face by hand -- because
/// their `R32Float` depth format is not natively filterable. IBL maps are
/// ordinary filterable colour formats, so a real cube view applies here and
/// gets correct hardware filtering across face seams for free.
pub fn cube_view(texture: &wgpu::Texture) -> wgpu::TextureView {
    texture.create_view(&wgpu::TextureViewDescriptor {
        label: Some("ibl cube view"),
        dimension: Some(wgpu::TextureViewDimension::Cube),
        ..Default::default()
    })
}

/// A single-layer, single-mip 2D view of one cube face, for use as a render
/// attachment: the preprocessing passes render one face at a time, and the
/// prefilter chain one (face, mip) pair at a time.
///
/// `face` indexes wgpu's cubemap layer order: +X, -X, +Y, -Y, +Z, -Z as 0..5.
pub fn face_view(texture: &wgpu::Texture, face: u32, mip: u32) -> wgpu::TextureView {
    texture.create_view(&wgpu::TextureViewDescriptor {
        label: Some("ibl cube face view"),
        dimension: Some(wgpu::TextureViewDimension::D2),
        base_array_layer: face,
        array_layer_count: Some(1),
        base_mip_level: mip,
        mip_level_count: Some(1),
        ..Default::default()
    })
}

/// WGSL every IBL pass needs, prepended to each pass's own source rather than
/// copied into it.
///
/// The fullscreen triangle is shared because it is what decides what `uv`
/// means, and [`cube_face_direction`](self) is built directly on that: if one
/// pass's triangle disagreed with another's, their faces would come out
/// flipped relative to each other with nothing to say so.
const COMMON_WGSL: &str = r#"
const PI: f32 = 3.14159265359;

struct FullscreenOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

// One oversized triangle covering the whole target. `uv` is (0, 0) at the
// target's top-left texel and (1, 1) at its bottom-right -- image order, the
// same order the texels are stored in, not NDC order.
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

// The direction the cube-face texel at `uv` on `face` looks along.
//
// `face` is wgpu's cubemap layer order, +X, -X, +Y, -Y, +Z, -Z as 0..5, and
// the per-face axes below are the fixed cubemap convention that order belongs
// to: wgpu hands a Cube view straight to the backend's cube image type
// (`vk::ImageViewType::CUBE` in wgpu-hal's Vulkan `conv.rs`, `TextureCube` in
// D3D12), so the sampling convention is the backends', not wgpu's to choose.
// This function is the inverse of their shared (major axis, sc, tc) table,
// with `uv` in stored image order -- which is why every face's vertical axis
// comes out negated.
//
// Do not "tidy" a sign here. Every one of them is load-bearing and a wrong one
// produces a plausible-looking image, mirrored, that only a direction test
// catches.
fn cube_face_direction(face: u32, uv: vec2<f32>) -> vec3<f32> {
    let a = 2.0 * uv.x - 1.0;
    let b = 2.0 * uv.y - 1.0;
    var dir = vec3<f32>(0.0, 0.0, 1.0);
    switch face {
        case 0u: { dir = vec3<f32>( 1.0,   -b,   -a); }
        case 1u: { dir = vec3<f32>(-1.0,   -b,    a); }
        case 2u: { dir = vec3<f32>(   a,  1.0,    b); }
        case 3u: { dir = vec3<f32>(   a, -1.0,   -b); }
        case 4u: { dir = vec3<f32>(   a,   -b,  1.0); }
        default: { dir = vec3<f32>(  -a,   -b, -1.0); }
    }
    return normalize(dir);
}
"#;

const BRDF_LUT_WGSL: &str = r#"
// Sample count for the Monte Carlo integration. 1024 Hammersley points is the
// standard figure; because this LUT is generated once ever, the cost is paid
// at startup and never again.
const SAMPLE_COUNT: u32 = 1024u;

// Van der Corput radical inverse: reverse the bits of `bits_in` and read them
// back as a binary fraction.
fn radical_inverse_vdc(bits_in: u32) -> f32 {
    var bits = bits_in;
    bits = (bits << 16u) | (bits >> 16u);
    bits = ((bits & 0x55555555u) << 1u) | ((bits & 0xAAAAAAAAu) >> 1u);
    bits = ((bits & 0x33333333u) << 2u) | ((bits & 0xCCCCCCCCu) >> 2u);
    bits = ((bits & 0x0F0F0F0Fu) << 4u) | ((bits & 0xF0F0F0F0u) >> 4u);
    bits = ((bits & 0x00FF00FFu) << 8u) | ((bits & 0xFF00FF00u) >> 8u);
    return f32(bits) * 2.3283064365386963e-10; // 1 / 2^32
}

// The Hammersley low-discrepancy sequence: i/N paired with its radical
// inverse. Converges far faster than uniform random sampling for the same
// sample count, which is what keeps 1024 samples enough.
fn hammersley(i: u32, n: u32) -> vec2<f32> {
    return vec2<f32>(f32(i) / f32(n), radical_inverse_vdc(i));
}

// GGX/Trowbridge-Reitz importance sampling: maps a uniform point in the unit
// square onto a half-vector distributed by the GGX normal distribution around
// `n`, so samples land where the specular lobe actually has energy.
fn importance_sample_ggx(xi: vec2<f32>, n: vec3<f32>, roughness: f32) -> vec3<f32> {
    let a = roughness * roughness;
    let phi = 2.0 * PI * xi.x;
    let cos_theta = sqrt((1.0 - xi.y) / (1.0 + (a * a - 1.0) * xi.y));
    // max() against 0: cos_theta is <= 1 analytically, but rounding can push
    // it a hair past, and sqrt of a negative is NaN.
    let sin_theta = sqrt(max(1.0 - cos_theta * cos_theta, 0.0));
    let h_tangent = vec3<f32>(cos(phi) * sin_theta, sin(phi) * sin_theta, cos_theta);
    let up = select(vec3<f32>(1.0, 0.0, 0.0), vec3<f32>(0.0, 0.0, 1.0), abs(n.z) < 0.999);
    let tangent = normalize(cross(up, n));
    let bitangent = cross(n, tangent);
    return normalize(tangent * h_tangent.x + bitangent * h_tangent.y + n * h_tangent.z);
}

// Smith geometry term with the IBL remapping k = a^2/2. Direct lighting uses
// k = (roughness + 1)^2 / 8 instead; substituting the direct k here would
// visibly darken the split-sum result at high roughness.
fn geometry_schlick_ggx(n_dot_x: f32, roughness: f32) -> f32 {
    let a = roughness;
    let k = (a * a) / 2.0;
    return n_dot_x / (n_dot_x * (1.0 - k) + k);
}

fn geometry_smith(n_dot_v: f32, n_dot_l: f32, roughness: f32) -> f32 {
    return geometry_schlick_ggx(n_dot_v, roughness) * geometry_schlick_ggx(n_dot_l, roughness);
}

// The second half of the split-sum approximation: with Fresnel's F0 factored
// out of the Cook-Torrance integral, what remains is a scale and a bias that
// depend only on n_dot_v and roughness. That is the whole reason this LUT can
// be generated once and reused for every environment.
fn integrate_brdf(n_dot_v: f32, roughness: f32) -> vec2<f32> {
    // Work in a canonical frame: the normal is +Z and the view vector lies in
    // the XZ plane at the requested n_dot_v. The integral is rotationally
    // symmetric about the normal, so this loses nothing.
    let n = vec3<f32>(0.0, 0.0, 1.0);
    let v = vec3<f32>(sqrt(max(1.0 - n_dot_v * n_dot_v, 0.0)), 0.0, n_dot_v);
    var scale = 0.0;
    var bias = 0.0;
    for (var i = 0u; i < SAMPLE_COUNT; i++) {
        let xi = hammersley(i, SAMPLE_COUNT);
        let h = importance_sample_ggx(xi, n, roughness);
        let l = normalize(2.0 * dot(v, h) * h - v);
        let n_dot_l = max(l.z, 0.0);
        let n_dot_h = max(h.z, 0.0);
        // clamp, not max: dot() of two normalized vectors can land a hair
        // above 1.0, and pow() of a negative base below is a NaN.
        let v_dot_h = clamp(dot(v, h), 0.0, 1.0);
        if (n_dot_l > 0.0) {
            let g = geometry_smith(n_dot_v, n_dot_l, roughness);
            let g_vis = (g * v_dot_h) / (n_dot_h * n_dot_v);
            let fc = pow(1.0 - v_dot_h, 5.0);
            scale += (1.0 - fc) * g_vis;
            bias += fc * g_vis;
        }
    }
    return vec2<f32>(scale, bias) / f32(SAMPLE_COUNT);
}

@fragment
fn fs_brdf_lut(in: FullscreenOut) -> @location(0) vec2<f32> {
    // U is n_dot_v, V is roughness. Both are clamped off zero: n_dot_v == 0
    // divides by zero in the visibility term, and a zero-area microfacet
    // distribution is degenerate. Texel centres never actually reach zero at
    // this resolution, so the clamp is a guard rather than a bias.
    let n_dot_v = clamp(in.uv.x, 0.001, 1.0);
    let roughness = clamp(in.uv.y, 0.001, 1.0);
    return integrate_brdf(n_dot_v, roughness);
}
"#;

/// Generates the BRDF integration LUT: a [`BRDF_LUT_SIZE`]-square
/// [`BRDF_LUT_FORMAT`] texture whose `.r` is the `f0` scale and `.g` the
/// `f0` bias of the split-sum approximation, indexed by `n_dot_v` along U
/// and `roughness` along V.
///
/// **Schedule.** This is the one IBL pass that depends on nothing but the
/// BRDF itself -- not the environment, not the scene, not the camera -- so it
/// is generated **once at startup and reused for every skybox thereafter**.
/// The three environment passes (equirect->cubemap, irradiance, prefilter)
/// rerun whenever the skybox changes; this one never does.
///
/// Returns the tracked texture, which the caller must keep alive for the
/// profiler to keep counting it, alongside a 2D view for binding.
pub fn generate_brdf_lut(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> (TrackedTexture, wgpu::TextureView) {
    let texture = create_tracked_texture(
        device,
        &wgpu::TextureDescriptor {
            label: Some("ibl brdf lut"),
            size: wgpu::Extent3d {
                width: BRDF_LUT_SIZE,
                height: BRDF_LUT_SIZE,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: BRDF_LUT_FORMAT,
            // COPY_SRC so the generated table can be read back and asserted
            // on; it is a pure function of the BRDF, so its values are
            // checkable numbers rather than something only an eyeball test
            // could judge.
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        },
    );
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("ibl brdf lut shader"),
        source: wgpu::ShaderSource::Wgsl(format!("{COMMON_WGSL}{BRDF_LUT_WGSL}").into()),
    });
    // No bind groups at all: the integration reads nothing but its own
    // fragment coordinates.
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("ibl brdf lut pll"),
        bind_group_layouts: &[],
        push_constant_ranges: &[],
    });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("ibl brdf lut pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: "vs_fullscreen",
            buffers: &[],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: "fs_brdf_lut",
            targets: &[Some(wgpu::ColorTargetState {
                format: BRDF_LUT_FORMAT,
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
    });

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("ibl brdf lut encoder"),
    });
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("ibl brdf lut pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            ..Default::default()
        });
        pass.set_pipeline(&pipeline);
        pass.draw(0..3, 0..1);
    }
    queue.submit(std::iter::once(encoder.finish()));

    (texture, view)
}

const EQUIRECT_TO_CUBE_WGSL: &str = r#"
struct FaceUniform {
    // .x is the cube face this pass renders; the rest is padding to the
    // 16-byte uniform stride.
    face: vec4<u32>,
};
@group(0) @binding(0) var<uniform> face_data: FaceUniform;
@group(0) @binding(1) var t_equirect: texture_2d<f32>;
@group(0) @binding(2) var s_equirect: sampler;

// Direction -> equirectangular UV.
//
// COPIED VERBATIM from `fs_sky` in `surface.rs`, which is what actually draws
// the sky the player sees. It is not derived here and must not be "cleaned
// up": an independently derived mapping that looks equivalent is how the IBL
// environment ends up rotated or mirrored against the visible skybox, which
// reads as plausible reflections that are subtly, permanently wrong.
fn equirect_uv(dir: vec3<f32>) -> vec2<f32> {
    let phi = atan2(dir.z, dir.x);
    let theta = asin(clamp(dir.y, -1.0, 1.0));
    let u = phi / (2.0 * PI) + 0.5;
    let v = 0.5 - theta / PI;
    return vec2<f32>(u, v);
}

@fragment
fn fs_equirect_to_cube(in: FullscreenOut) -> @location(0) vec4<f32> {
    let dir = cube_face_direction(face_data.face.x, in.uv);
    // SampleLevel, not Sample: `u` jumps by a full turn across the longitude
    // seam, so the implicit derivative there is enormous and would pick a mip
    // that is not the one this pass means to read.
    return textureSampleLevel(t_equirect, s_equirect, equirect_uv(dir), 0.0);
}
"#;

/// Projects an equirectangular (lat-long) environment image onto a real
/// cubemap: an [`ENV_CUBE_SIZE`]-square [`ENV_CUBE_FORMAT`] texture of six
/// faces, plus a `Cube` view of it for sampling by direction.
///
/// One render pass per face, each writing a [`face_view`] of the target. The
/// fragment shader turns the face texel's `uv` into a world direction with
/// [`COMMON_WGSL`]'s `cube_face_direction`, then reads the source through the
/// **same** direction->UV mapping `surface.rs`'s `fs_sky` uses, so the
/// environment the material shader reflects sits exactly where the sky the
/// camera sees does.
///
/// `equirect_view` and `sampler` are the skybox's own texture view and
/// sampler; nothing about the source's format is assumed beyond its being
/// filterable, so the sRGB decode the real skybox texture carries applies
/// here exactly as it does in `fs_sky`.
///
/// Handles the equirectangular projection only. `SkyboxProjection::Cubemap`
/// exists in `bsengine-core` but nothing reads it -- neither the loader nor
/// `fs_sky` branches on it -- so a cross-layout image is already sampled as
/// though it were equirectangular today, and this pass changes nothing about
/// that.
pub fn equirect_to_cubemap(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    equirect_view: &wgpu::TextureView,
    sampler: &wgpu::Sampler,
) -> (TrackedTexture, wgpu::TextureView) {
    let texture = create_cubemap(
        device,
        "ibl env cube",
        ENV_CUBE_SIZE,
        1,
        ENV_CUBE_FORMAT,
        wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::TEXTURE_BINDING
            // COPY_SRC so the faces can be read back and checked against the
            // directions they are supposed to hold. A mirrored conversion is
            // invisible in any smooth environment, so "it rendered something"
            // is not evidence of anything.
            | wgpu::TextureUsages::COPY_SRC,
    );

    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("ibl equirect to cube shader"),
        source: wgpu::ShaderSource::Wgsl(format!("{COMMON_WGSL}{EQUIRECT_TO_CUBE_WGSL}").into()),
    });

    // One record per face, indexed by dynamic offset, so all six passes share
    // a single buffer and bind group and the whole conversion is one submit.
    let face_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("ibl face uniform"),
        size: FACE_UNIFORM_STRIDE * CUBE_FACES as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    for face in 0..CUBE_FACES {
        let mut record = [0u8; FACE_UNIFORM_SIZE as usize];
        record[..4].copy_from_slice(&face.to_le_bytes());
        queue.write_buffer(&face_buffer, face as u64 * FACE_UNIFORM_STRIDE, &record);
    }

    let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("ibl equirect to cube bgl"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: true,
                    min_binding_size: wgpu::BufferSize::new(FACE_UNIFORM_SIZE),
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
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ],
    });
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("ibl equirect to cube bg"),
        layout: &bgl,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                // An explicit binding size, not `as_entire_binding`: with a
                // dynamic offset the bound range starts at the offset, so a
                // whole-buffer size would run past the end on every face but
                // the first.
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &face_buffer,
                    offset: 0,
                    size: wgpu::BufferSize::new(FACE_UNIFORM_SIZE),
                }),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(equirect_view),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
        ],
    });

    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("ibl equirect to cube pll"),
        bind_group_layouts: &[&bgl],
        push_constant_ranges: &[],
    });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("ibl equirect to cube pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: "vs_fullscreen",
            buffers: &[],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: "fs_equirect_to_cube",
            targets: &[Some(wgpu::ColorTargetState {
                format: ENV_CUBE_FORMAT,
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
    });

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("ibl equirect to cube encoder"),
    });
    for face in 0..CUBE_FACES {
        let target = face_view(&texture, face, 0);
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("ibl equirect to cube pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &target,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            ..Default::default()
        });
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(
            0,
            &bind_group,
            &[(face as u64 * FACE_UNIFORM_STRIDE) as wgpu::DynamicOffset],
        );
        pass.draw(0..3, 0..1);
    }
    queue.submit(std::iter::once(encoder.finish()));

    let view = cube_view(&texture);
    (texture, view)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::surface::WgpuSurface;
    use glam::Vec3;

    /// Decodes one IEEE-754 binary16 value. `Rg16Float` readback comes back
    /// as raw half-floats, and nothing in this crate's dependency tree
    /// decodes them. Deliberately preserves infinities and NaNs rather than
    /// sanitising them, because "every texel is finite" is one of the
    /// properties under test.
    fn f16_to_f32(bits: u16) -> f32 {
        let sign = if bits & 0x8000 != 0 { -1.0f32 } else { 1.0 };
        let exp = ((bits >> 10) & 0x1f) as i32;
        let frac = (bits & 0x03ff) as f32;
        if exp == 0 {
            // Subnormal (and zero): value is frac * 2^-24.
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

    /// Generates the LUT on a real device and reads it back as
    /// `(scale, bias)` pairs in row-major order, row 0 being roughness ~= 0.
    ///
    /// Reuses `crate::output::read_pixels`, the crate's existing texture
    /// readback: its 4-bytes-per-texel assumption is exactly right for
    /// `Rg16Float` (two halves), and at 512 wide the rows are already
    /// 256-byte aligned.
    fn read_back_lut() -> Vec<(f32, f32)> {
        let (device, queue) = pollster::block_on(WgpuSurface::headless_device_for_testing());
        let (texture, _view) = generate_brdf_lut(&device, &queue);
        let raw =
            crate::output::read_pixels(&device, &queue, &texture, BRDF_LUT_SIZE, BRDF_LUT_SIZE);
        assert_eq!(raw.len(), (BRDF_LUT_SIZE * BRDF_LUT_SIZE * 4) as usize);
        raw.chunks_exact(4)
            .map(|t| {
                (
                    f16_to_f32(u16::from_le_bytes([t[0], t[1]])),
                    f16_to_f32(u16::from_le_bytes([t[2], t[3]])),
                )
            })
            .collect()
    }

    fn texel(lut: &[(f32, f32)], n_dot_v_index: u32, roughness_index: u32) -> (f32, f32) {
        lut[(roughness_index * BRDF_LUT_SIZE + n_dot_v_index) as usize]
    }

    #[test]
    fn brdf_lut_values_are_in_range_and_scale_approaches_one_at_normal_incidence() {
        let lut = read_back_lut();

        // Column 511 is n_dot_v ~= 0.999 (texel centre 511.5/512), row 0 is
        // roughness ~= 0.001. At n_dot_v = 1, roughness = 0 a perfect mirror
        // reflects f0 exactly, so the scale term must approach 1 and the bias
        // approach 0. A LUT that is merely non-empty would pass a weaker
        // assertion while being completely wrong.
        let (mirror_scale, mirror_bias) = texel(&lut, BRDF_LUT_SIZE - 1, 0);
        assert!(
            mirror_scale > 0.99 && mirror_scale <= 1.0,
            "at n_dot_v ~= 1, roughness ~= 0 the f0 scale must approach 1, got {mirror_scale}"
        );
        assert!(
            mirror_bias.abs() < 0.01,
            "at n_dot_v ~= 1, roughness ~= 0 the f0 bias must approach 0, got {mirror_bias}"
        );

        // The opposite end of the same row: a fully rough surface at the same
        // viewing angle loses a large part of that energy to the geometry
        // term. Without this, a shader that simply wrote (1, 0) everywhere
        // would satisfy the assertions above.
        let (rough_scale, _) = texel(&lut, BRDF_LUT_SIZE - 1, BRDF_LUT_SIZE - 1);
        assert!(
            rough_scale < mirror_scale - 0.1,
            "the f0 scale must fall off with roughness -- a constant table is \
             not an integration; mirror {mirror_scale} vs rough {rough_scale}"
        );

        // Grazing incidence at mirror smoothness is the other known corner:
        // the Fresnel weight moves almost entirely into the bias term.
        let (grazing_scale, grazing_bias) = texel(&lut, 0, 0);
        assert!(
            grazing_bias > grazing_scale,
            "at grazing incidence and zero roughness the split-sum weight \
             belongs to the bias, got scale {grazing_scale} bias {grazing_bias}"
        );
    }

    #[test]
    fn brdf_lut_texels_are_all_finite_and_within_zero_to_one() {
        let lut = read_back_lut();
        // One f16 ulp at 1.0 is 2^-10. The analytic bound is [0, 1] and this
        // GPU lands exactly on it (measured scale range [0.0043, 1.0], bias
        // [0.0, 0.9937]); the slack only tolerates a different GPU's rounding
        // pushing a 1.0 up by a single representable step, not a real
        // out-of-range result.
        const F16_SLACK: f32 = 0.000_976_562_5;
        for (i, &(scale, bias)) in lut.iter().enumerate() {
            let n_dot_v_index = i as u32 % BRDF_LUT_SIZE;
            let roughness_index = i as u32 / BRDF_LUT_SIZE;
            assert!(
                scale.is_finite() && bias.is_finite(),
                "texel (n_dot_v {n_dot_v_index}, roughness {roughness_index}) is not finite: \
                 scale {scale}, bias {bias}"
            );
            assert!(
                (-F16_SLACK..=1.0 + F16_SLACK).contains(&scale),
                "f0 scale out of [0, 1] at (n_dot_v {n_dot_v_index}, roughness \
                 {roughness_index}): {scale}"
            );
            assert!(
                (-F16_SLACK..=1.0 + F16_SLACK).contains(&bias),
                "f0 bias out of [0, 1] at (n_dot_v {n_dot_v_index}, roughness \
                 {roughness_index}): {bias}"
            );
        }
    }

    /// A cube view over a texture that is not six-layered, or a face view
    /// whose layer range runs past the end, is a `wgpu` validation error and
    /// nothing else -- it produces no wrong pixels to notice later. The error
    /// scope is the only thing that surfaces it.
    #[test]
    fn cubemap_views_are_valid_for_the_gpu() {
        let (device, _queue) = pollster::block_on(WgpuSurface::headless_device_for_testing());
        device.push_error_scope(wgpu::ErrorFilter::Validation);

        let cube = create_cubemap(
            &device,
            "cubemap helper test",
            PREFILTER_CUBE_SIZE,
            PREFILTER_MIP_LEVELS,
            wgpu::TextureFormat::Rgba16Float,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        );
        assert_eq!(
            cube.size().depth_or_array_layers,
            6,
            "a cubemap is six array layers; anything else cannot be viewed as a cube"
        );
        assert_eq!(cube.mip_level_count(), PREFILTER_MIP_LEVELS);

        let _cube_view = cube_view(&cube);
        // Every (face, mip) pair the prefilter pass will render into.
        for face in 0..CUBE_FACES {
            for mip in 0..PREFILTER_MIP_LEVELS {
                let _face_view = face_view(&cube, face, mip);
            }
        }

        device.poll(wgpu::Maintain::Wait);
        assert!(
            pollster::block_on(device.pop_error_scope()).is_none(),
            "creating the cube and face views must not raise a validation error"
        );
    }

    #[test]
    fn brdf_lut_is_two_channel_half_float() {
        // The shader's fragment entry returns a vec2<f32>; a format with a
        // different channel count would be a pipeline validation error.
        assert_eq!(BRDF_LUT_FORMAT, wgpu::TextureFormat::Rg16Float);
    }

    // ---------------------------------------------------------------------
    // Equirect -> cubemap
    // ---------------------------------------------------------------------

    /// Reads one array layer of an [`ENV_CUBE_FORMAT`] texture back as linear
    /// RGBA floats.
    ///
    /// Same padded-row idiom as [`crate::output::read_pixels`], which Task 1
    /// reused for the LUT, widened where a cube face does not fit it: that one
    /// is hard-wired to array layer 0 and four bytes per texel, and a face is
    /// layer `layer` of eight-byte texels.
    fn read_rgba16f_layer(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        texture: &wgpu::Texture,
        width: u32,
        height: u32,
        layer: u32,
    ) -> Vec<[f32; 4]> {
        const BYTES_PER_TEXEL: u32 = 8;
        let unpadded_bytes_per_row = width * BYTES_PER_TEXEL;
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let padded_bytes_per_row = unpadded_bytes_per_row.div_ceil(align) * align;

        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ibl face readback"),
            size: (padded_bytes_per_row * height) as u64,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("ibl face readback encoder"),
        });
        encoder.copy_texture_to_buffer(
            wgpu::ImageCopyTexture {
                texture,
                mip_level: 0,
                // The layer selector: for a 2D array, `origin.z` is the layer.
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
                    bytes_per_row: Some(padded_bytes_per_row),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        queue.submit(std::iter::once(encoder.finish()));

        let slice = buffer.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        device.poll(wgpu::Maintain::Wait);
        rx.recv()
            .expect("map_async never reported a result")
            .expect("mapping the readback buffer failed");

        let mapped = slice.get_mapped_range();
        let mut texels = Vec::with_capacity((width * height) as usize);
        for row in 0..height {
            let start = (row * padded_bytes_per_row) as usize;
            let end = start + unpadded_bytes_per_row as usize;
            for t in mapped[start..end].chunks_exact(BYTES_PER_TEXEL as usize) {
                texels.push([
                    f16_to_f32(u16::from_le_bytes([t[0], t[1]])),
                    f16_to_f32(u16::from_le_bytes([t[2], t[3]])),
                    f16_to_f32(u16::from_le_bytes([t[4], t[5]])),
                    f16_to_f32(u16::from_le_bytes([t[6], t[7]])),
                ]);
            }
        }
        drop(mapped);
        buffer.unmap();
        texels
    }

    /// Size of the synthetic equirectangular source. 2:1, as every lat-long
    /// panorama is.
    const TEST_EQUIRECT_W: u32 = 512;
    const TEST_EQUIRECT_H: u32 = 256;

    /// Encodes a unit direction as a colour: `[-1, 1]` onto `[0, 1]` per
    /// component.
    fn encode_direction(d: Vec3) -> [u8; 4] {
        let q = |x: f32| ((x * 0.5 + 0.5) * 255.0).round().clamp(0.0, 255.0) as u8;
        [q(d.x), q(d.y), q(d.z), 255]
    }

    /// The inverse of [`encode_direction`]. Not renormalised here -- callers
    /// that want a unit vector say so, and the raw length is occasionally the
    /// thing that tells you the readback itself went wrong.
    fn decode_direction(c: [f32; 4]) -> Vec3 {
        Vec3::new(c[0] * 2.0 - 1.0, c[1] * 2.0 - 1.0, c[2] * 2.0 - 1.0)
    }

    /// An equirectangular panorama whose every texel stores **the direction
    /// that texel represents**, encoded as its colour.
    ///
    /// This is what makes the conversion checkable at all. A photograph, a
    /// gradient, or six flat octant colours all survive a mirrored or rotated
    /// conversion looking entirely reasonable; a direction-encoded source does
    /// not, because whatever comes out of a cube face texel decodes straight
    /// back into the direction the source believed it was.
    ///
    /// Built from the inverse of `fs_sky`'s mapping (`u` is longitude, `v` is
    /// latitude measured downwards), so it is by construction the thing that
    /// mapping expects to read.
    fn direction_encoded_equirect() -> Vec<u8> {
        let mut pixels = Vec::with_capacity((TEST_EQUIRECT_W * TEST_EQUIRECT_H * 4) as usize);
        for j in 0..TEST_EQUIRECT_H {
            let v = (j as f32 + 0.5) / TEST_EQUIRECT_H as f32;
            let theta = (0.5 - v) * std::f32::consts::PI;
            for i in 0..TEST_EQUIRECT_W {
                let u = (i as f32 + 0.5) / TEST_EQUIRECT_W as f32;
                let phi = (u - 0.5) * 2.0 * std::f32::consts::PI;
                let dir = Vec3::new(
                    theta.cos() * phi.cos(),
                    theta.sin(),
                    theta.cos() * phi.sin(),
                );
                pixels.extend_from_slice(&encode_direction(dir));
            }
        }
        pixels
    }

    /// Uploads [`direction_encoded_equirect`] and returns it with a sampler
    /// configured exactly as `set_skybox_from_rgba`'s is -- `Repeat` across the
    /// longitude seam, `ClampToEdge` at the poles, linear both ways.
    ///
    /// **`Rgba8Unorm`, not the `Rgba8UnormSrgb` the real skybox uses.** These
    /// texels are an encoded direction rather than a colour, and an sRGB decode
    /// would bend the encoding out of shape before it could be read back. The
    /// conversion pass reads whatever the view's format declares and does
    /// nothing format-specific, so this narrows nothing about what is under
    /// test: where directions land, not colour management.
    fn upload_direction_encoded_equirect(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> (wgpu::Texture, wgpu::TextureView, wgpu::Sampler) {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("direction-encoded equirect"),
            size: wgpu::Extent3d {
                width: TEST_EQUIRECT_W,
                height: TEST_EQUIRECT_H,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            texture.as_image_copy(),
            &direction_encoded_equirect(),
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(4 * TEST_EQUIRECT_W),
                rows_per_image: Some(TEST_EQUIRECT_H),
            },
            wgpu::Extent3d {
                width: TEST_EQUIRECT_W,
                height: TEST_EQUIRECT_H,
                depth_or_array_layers: 1,
            },
        );
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("direction-encoded equirect sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        (texture, view, sampler)
    }

    /// How many directions [`sample_cube_by_direction`] probes at once. Must
    /// match the array length hard-coded in `PROBE_WGSL`.
    const PROBE_COUNT: usize = 16;
    const _: () = assert!(PROBE_COUNT == 16, "PROBE_WGSL declares array<_, 16>");

    const PROBE_WGSL: &str = r#"
struct Probes {
    dirs: array<vec4<f32>, 16>,
};
@group(0) @binding(0) var<uniform> probes: Probes;
@group(0) @binding(1) var t_cube: texture_cube<f32>;
@group(0) @binding(2) var s_cube: sampler;

@vertex
fn vs_probe(@builtin(vertex_index) vi: u32) -> @builtin(position) vec4<f32> {
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -3.0), vec2<f32>(-1.0, 1.0), vec2<f32>(3.0, 1.0),
    );
    let p = positions[vi];
    return vec4<f32>(p.x, p.y, 0.0, 1.0);
}

// The target is PROBE_COUNT x 1, so the fragment's x is the probe index.
@fragment
fn fs_probe(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let i = u32(pos.x);
    return textureSampleLevel(t_cube, s_cube, normalize(probes.dirs[i].xyz), 0.0);
}
"#;

    /// Samples `cube` at each of `dirs` **through the GPU's own cube sampler**
    /// and returns what came back.
    ///
    /// This is the half of the direction test that is not circular. Reading
    /// face texels back and comparing them against a Rust copy of the same
    /// face-direction table the shader uses would pass just as happily if both
    /// copies were mirrored. Asking the hardware to sample by direction cannot
    /// be fooled that way: the cubemap convention it applies is fixed by the
    /// backend, so if the conversion wrote a face mirrored, the colour returned
    /// for direction `d` decodes to a different direction and the assertion
    /// fails.
    fn sample_cube_by_direction(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        cube: &wgpu::TextureView,
        dirs: &[Vec3; PROBE_COUNT],
    ) -> Vec<Vec3> {
        let mut records = [[0f32; 4]; PROBE_COUNT];
        for (record, dir) in records.iter_mut().zip(dirs) {
            let d = dir.normalize();
            *record = [d.x, d.y, d.z, 0.0];
        }
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ibl probe uniform"),
            size: (PROBE_COUNT * 16) as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&uniform, 0, bytemuck::cast_slice(records.as_slice()));

        let target = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("ibl probe target"),
            size: wgpu::Extent3d {
                width: PROBE_COUNT as u32,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: ENV_CUBE_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let target_view = target.create_view(&wgpu::TextureViewDescriptor::default());

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("ibl probe shader"),
            source: wgpu::ShaderSource::Wgsl(PROBE_WGSL.into()),
        });
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("ibl probe bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new((PROBE_COUNT * 16) as u64),
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::Cube,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("ibl probe sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("ibl probe bg"),
            layout: &bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(cube),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("ibl probe pll"),
            bind_group_layouts: &[&bgl],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("ibl probe pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_probe",
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_probe",
                targets: &[Some(wgpu::ColorTargetState {
                    format: ENV_CUBE_FORMAT,
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
        });

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("ibl probe encoder"),
        });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("ibl probe pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &target_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                ..Default::default()
            });
            pass.set_pipeline(&pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.draw(0..3, 0..1);
        }
        queue.submit(std::iter::once(encoder.finish()));

        read_rgba16f_layer(device, queue, &target, PROBE_COUNT as u32, 1, 0)
            .into_iter()
            .map(decode_direction)
            .collect()
    }

    #[test]
    fn equirect_to_cubemap_puts_each_direction_on_the_right_face() {
        let (device, queue) = pollster::block_on(WgpuSurface::headless_device_for_testing());
        device.push_error_scope(wgpu::ErrorFilter::Validation);

        let (_src, src_view, src_sampler) = upload_direction_encoded_equirect(&device, &queue);
        let (cube, cube_v) = equirect_to_cubemap(&device, &queue, &src_view, &src_sampler);

        assert_eq!(cube.width(), ENV_CUBE_SIZE);
        assert_eq!(cube.height(), ENV_CUBE_SIZE);
        assert_eq!(cube.size().depth_or_array_layers, CUBE_FACES);

        // Part 1: every face looks along its own axis.
        //
        // The centre texel of face `i` must decode to the axis wgpu's layer
        // order assigns to layer `i`. This is what catches a permuted or
        // off-by-one face loop -- the failure that puts the sky on the floor.
        let axes = [
            (Vec3::X, "+X"),
            (Vec3::NEG_X, "-X"),
            (Vec3::Y, "+Y"),
            (Vec3::NEG_Y, "-Y"),
            (Vec3::Z, "+Z"),
            (Vec3::NEG_Z, "-Z"),
        ];
        let centre = (ENV_CUBE_SIZE / 2 * ENV_CUBE_SIZE + ENV_CUBE_SIZE / 2) as usize;
        for (face, (axis, name)) in axes.iter().enumerate() {
            let texels = read_rgba16f_layer(
                &device,
                &queue,
                &cube,
                ENV_CUBE_SIZE,
                ENV_CUBE_SIZE,
                face as u32,
            );
            assert_eq!(texels.len(), (ENV_CUBE_SIZE * ENV_CUBE_SIZE) as usize);
            let got = decode_direction(texels[centre]);
            assert!(
                got.length() > 0.5,
                "face {face} ({name}) centre carries no direction at all -- \
                 the pass wrote nothing there: {got:?}"
            );
            let alignment = got.normalize().dot(*axis);
            assert!(
                alignment > 0.999,
                "face {face} must look along {name}, but its centre texel \
                 decodes to {:?} (alignment {alignment})",
                got.normalize(),
            );
        }

        // Part 2: nothing inside a face is mirrored or rotated.
        //
        // Face centres alone cannot see this: mirror a face about either of
        // its own axes and its centre texel does not move. So probe the cube
        // through the hardware's cube sampler at directions that are
        // asymmetric within their face -- two per face, off-centre in both
        // axes -- and require each to hand back the direction it asked for.
        // Any mirror, any 90-degree rotation, moves the answer far outside the
        // tolerance below.
        let probes = [
            Vec3::X,
            Vec3::NEG_X,
            Vec3::Y,
            Vec3::NEG_Y,
            Vec3::Z,
            Vec3::NEG_Z,
            Vec3::new(1.0, 0.5, 0.3),
            Vec3::new(1.0, -0.6, 0.2),
            Vec3::new(-1.0, 0.4, 0.7),
            Vec3::new(-1.0, -0.3, -0.55),
            Vec3::new(0.3, 1.0, 0.6),
            Vec3::new(-0.55, 1.0, -0.25),
            Vec3::new(0.5, -1.0, -0.3),
            Vec3::new(-0.2, -1.0, 0.65),
            Vec3::new(0.2, 0.6, 1.0),
            Vec3::new(-0.4, 0.3, -1.0),
        ];
        let sampled = sample_cube_by_direction(&device, &queue, &cube_v, &probes);
        assert_eq!(sampled.len(), PROBE_COUNT);
        for (probe, got) in probes.iter().zip(&sampled) {
            let want = probe.normalize();
            let alignment = got.normalize().dot(want);
            // 0.999 is ~2.6 degrees. The real error is quantisation of the
            // 8-bit source encoding plus one bilinear tap, both well under
            // half a degree; the smallest mistake this is guarding against --
            // a mirror about a face axis -- costs far more than 2.6 degrees
            // for every one of the ten asymmetric probes.
            assert!(
                alignment > 0.999,
                "sampling the cube along {want:?} returned the encoding of \
                 {:?} (alignment {alignment}) -- the conversion is mirrored \
                 or rotated within a face",
                got.normalize(),
            );
        }

        device.poll(wgpu::Maintain::Wait);
        assert!(
            pollster::block_on(device.pop_error_scope()).is_none(),
            "the conversion must not raise a validation error"
        );
    }
}
