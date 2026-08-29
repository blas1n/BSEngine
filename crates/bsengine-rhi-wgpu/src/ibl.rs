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

/// The bind group layout every face-rendering pass shares: a dynamic-offset
/// uniform record saying which face (and, for the prefilter, which roughness)
/// this pass is writing, the source environment, and its sampler.
///
/// `source_dimension` is the only thing that varies between the three passes:
/// the equirect conversion reads a flat 2D panorama, while both convolutions
/// read the cubemap that conversion produced.
fn face_pass_bind_group_layout(
    device: &wgpu::Device,
    label: &str,
    uniform_size: u64,
    source_dimension: wgpu::TextureViewDimension,
) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some(label),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: true,
                    min_binding_size: wgpu::BufferSize::new(uniform_size),
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: source_dimension,
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
    })
}

/// The bind group for [`face_pass_bind_group_layout`], built once and reused by
/// every face pass with a differing dynamic offset.
fn face_pass_bind_group(
    device: &wgpu::Device,
    label: &str,
    layout: &wgpu::BindGroupLayout,
    uniforms: &wgpu::Buffer,
    uniform_size: u64,
    source: &wgpu::TextureView,
    sampler: &wgpu::Sampler,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some(label),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                // An explicit binding size, not `as_entire_binding`: with a
                // dynamic offset the bound range starts at the offset, so a
                // whole-buffer size would run past the end on every pass but
                // the first.
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: uniforms,
                    offset: 0,
                    size: wgpu::BufferSize::new(uniform_size),
                }),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(source),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
        ],
    })
}

/// The fullscreen-triangle pipeline every face pass runs: no vertex buffers, no
/// depth, one colour target, `vs_fullscreen` from [`COMMON_WGSL`] paired with
/// whichever fragment entry the pass supplies.
fn face_pass_pipeline(
    device: &wgpu::Device,
    label: &str,
    wgsl: &str,
    fragment_entry: &str,
    bgl: &wgpu::BindGroupLayout,
    format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(label),
        source: wgpu::ShaderSource::Wgsl(wgsl.into()),
    });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some(label),
        bind_group_layouts: &[bgl],
        push_constant_ranges: &[],
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(label),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: "vs_fullscreen",
            buffers: &[],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: fragment_entry,
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

/// Draws the shared fullscreen triangle into `target`, selecting this pass's
/// uniform record with `dynamic_offset`.
fn draw_face_pass(
    encoder: &mut wgpu::CommandEncoder,
    label: &str,
    target: &wgpu::TextureView,
    pipeline: &wgpu::RenderPipeline,
    bind_group: &wgpu::BindGroup,
    dynamic_offset: u64,
) {
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some(label),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: target,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: None,
        ..Default::default()
    });
    pass.set_pipeline(pipeline);
    pass.set_bind_group(0, bind_group, &[dynamic_offset as wgpu::DynamicOffset]);
    pass.draw(0..3, 0..1);
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

/// GGX importance sampling, shared by the BRDF LUT and the specular prefilter.
///
/// Kept apart from either pass rather than duplicated into both: the two are
/// the *same* integral split in half -- the LUT integrates the BRDF with the
/// environment factored out, the prefilter integrates the environment with the
/// BRDF factored out -- so if their sample distributions ever drifted apart,
/// recombining the halves in the material shader would no longer reconstruct
/// the integral they came from.
const IMPORTANCE_SAMPLING_WGSL: &str = r#"
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
"#;

const BRDF_LUT_WGSL: &str = r#"
// Sample count for the Monte Carlo integration. 1024 Hammersley points is the
// standard figure; because this LUT is generated once ever, the cost is paid
// at startup and never again.
const SAMPLE_COUNT: u32 = 1024u;

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
        source: wgpu::ShaderSource::Wgsl(
            format!("{COMMON_WGSL}{IMPORTANCE_SAMPLING_WGSL}{BRDF_LUT_WGSL}").into(),
        ),
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

    let face_buffer = write_face_uniforms(device, queue, "ibl equirect to cube uniform");

    let bgl = face_pass_bind_group_layout(
        device,
        "ibl equirect to cube bgl",
        FACE_UNIFORM_SIZE,
        wgpu::TextureViewDimension::D2,
    );
    let bind_group = face_pass_bind_group(
        device,
        "ibl equirect to cube bg",
        &bgl,
        &face_buffer,
        FACE_UNIFORM_SIZE,
        equirect_view,
        sampler,
    );
    let pipeline = face_pass_pipeline(
        device,
        "ibl equirect to cube",
        &format!("{COMMON_WGSL}{EQUIRECT_TO_CUBE_WGSL}"),
        "fs_equirect_to_cube",
        &bgl,
        ENV_CUBE_FORMAT,
    );

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("ibl equirect to cube encoder"),
    });
    for face in 0..CUBE_FACES {
        draw_face_pass(
            &mut encoder,
            "ibl equirect to cube pass",
            &face_view(&texture, face, 0),
            &pipeline,
            &bind_group,
            face as u64 * FACE_UNIFORM_STRIDE,
        );
    }
    queue.submit(std::iter::once(encoder.finish()));

    let view = cube_view(&texture);
    (texture, view)
}

/// Fills a uniform buffer with one [`FACE_UNIFORM_SIZE`] record per cube face,
/// each at its own [`FACE_UNIFORM_STRIDE`] offset, so the six face passes share
/// a single buffer and bind group and differ only by dynamic offset.
fn write_face_uniforms(device: &wgpu::Device, queue: &wgpu::Queue, label: &str) -> wgpu::Buffer {
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size: FACE_UNIFORM_STRIDE * CUBE_FACES as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    for face in 0..CUBE_FACES {
        let mut record = [0u8; FACE_UNIFORM_SIZE as usize];
        record[..4].copy_from_slice(&face.to_le_bytes());
        queue.write_buffer(&buffer, face as u64 * FACE_UNIFORM_STRIDE, &record);
    }
    buffer
}

const IRRADIANCE_WGSL: &str = r#"
// Steps around the normal and from the normal down to the horizon. 128 x 32
// midpoint samples put the cosine-weighted integral of a constant environment
// within 0.05% of the analytic answer -- far tighter than half-float storage
// can even represent, so more steps would buy nothing measurable.
const PHI_STEPS: u32 = 128u;
const THETA_STEPS: u32 = 32u;

struct FaceUniform {
    face: vec4<u32>,
};
@group(0) @binding(0) var<uniform> face_data: FaceUniform;
@group(0) @binding(1) var t_env: texture_cube<f32>;
@group(0) @binding(2) var s_env: sampler;

@fragment
fn fs_irradiance(in: FullscreenOut) -> @location(0) vec4<f32> {
    let n = cube_face_direction(face_data.face.x, in.uv);

    // Any frame perpendicular to n will do: the integral covers the whole
    // hemisphere, so rolling the frame about n cannot change the answer. The
    // select() guards the one case that is not free -- crossing n with an up
    // vector parallel to it gives a zero-length vector, and normalizing that
    // writes NaN into exactly the texels nearest the poles.
    let up = select(vec3<f32>(1.0, 0.0, 0.0), vec3<f32>(0.0, 1.0, 0.0), abs(n.y) < 0.999);
    let right = normalize(cross(up, n));
    let forward = cross(n, right);

    let d_phi = 2.0 * PI / f32(PHI_STEPS);
    let d_theta = 0.5 * PI / f32(THETA_STEPS);

    var sum = vec3<f32>(0.0);
    for (var i = 0u; i < PHI_STEPS; i++) {
        // Midpoint, not left edge: a left-edge sum over theta double-counts
        // the pole and misses the horizon, and lands about 0.5% high.
        let phi = (f32(i) + 0.5) * d_phi;
        let cos_phi = cos(phi);
        let sin_phi = sin(phi);
        for (var j = 0u; j < THETA_STEPS; j++) {
            let theta = (f32(j) + 0.5) * d_theta;
            let sin_theta = sin(theta);
            let cos_theta = cos(theta);
            let dir = right * (sin_theta * cos_phi)
                + forward * (sin_theta * sin_phi)
                + n * cos_theta;
            // cos_theta is Lambert's law; sin_theta is the Jacobian of the
            // (theta, phi) parametrisation of solid angle, not part of the
            // BRDF. Dropping either one is the classic wrong-normalisation
            // bug, and both produce a map that still looks plausible.
            sum += textureSampleLevel(t_env, s_env, dir, 0.0).rgb * cos_theta * sin_theta;
        }
    }

    // sum * d_theta * d_phi is the irradiance E, which is PI * L for a
    // constant environment of radiance L. What gets stored is E / PI: the
    // Lambert BRDF is albedo / PI, so folding the 1/PI in here lets the
    // material shader write `irradiance * albedo` with no stray constant, and
    // makes a uniform environment come back out as its own colour.
    let irradiance = sum * d_theta * d_phi / PI;
    return vec4<f32>(irradiance, 1.0);
}
"#;

/// Convolves an environment cubemap into a diffuse irradiance cubemap: an
/// [`IRRADIANCE_CUBE_SIZE`]-square [`ENV_CUBE_FORMAT`] texture whose texel for
/// direction `n` holds the cosine-weighted average of the environment over the
/// hemisphere around `n`, divided by PI.
///
/// That division is why a uniform environment convolves to its own colour, and
/// why the material shader can multiply the result straight by albedo: the
/// Lambert BRDF's 1/PI is already folded in here.
///
/// [`IRRADIANCE_CUBE_SIZE`] is 32 on purpose. Irradiance is the environment
/// smeared over an entire hemisphere, so it has no detail above the very
/// lowest frequencies -- a larger map would store the same image at more
/// expense.
///
/// One render pass per face, six in one submit. Returns the tracked texture,
/// which the caller must keep alive for the profiler to keep counting it,
/// alongside a `Cube` view for binding.
pub fn irradiance_from_cubemap(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    env_cube_view: &wgpu::TextureView,
    sampler: &wgpu::Sampler,
) -> (TrackedTexture, wgpu::TextureView) {
    let texture = create_cubemap(
        device,
        "ibl irradiance cube",
        IRRADIANCE_CUBE_SIZE,
        1,
        ENV_CUBE_FORMAT,
        wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::TEXTURE_BINDING
            // COPY_SRC so the convolution can be read back and checked against
            // the one environment whose answer is known exactly -- a constant
            // one, which must convolve to that same constant. A map that is
            // uniformly twice too bright looks entirely fine on screen.
            | wgpu::TextureUsages::COPY_SRC,
    );

    let face_buffer = write_face_uniforms(device, queue, "ibl irradiance uniform");
    let bgl = face_pass_bind_group_layout(
        device,
        "ibl irradiance bgl",
        FACE_UNIFORM_SIZE,
        wgpu::TextureViewDimension::Cube,
    );
    let bind_group = face_pass_bind_group(
        device,
        "ibl irradiance bg",
        &bgl,
        &face_buffer,
        FACE_UNIFORM_SIZE,
        env_cube_view,
        sampler,
    );
    let pipeline = face_pass_pipeline(
        device,
        "ibl irradiance",
        &format!("{COMMON_WGSL}{IRRADIANCE_WGSL}"),
        "fs_irradiance",
        &bgl,
        ENV_CUBE_FORMAT,
    );

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("ibl irradiance encoder"),
    });
    for face in 0..CUBE_FACES {
        draw_face_pass(
            &mut encoder,
            "ibl irradiance pass",
            &face_view(&texture, face, 0),
            &pipeline,
            &bind_group,
            face as u64 * FACE_UNIFORM_STRIDE,
        );
    }
    queue.submit(std::iter::once(encoder.finish()));

    let view = cube_view(&texture);
    (texture, view)
}

/// Size of one prefilter uniform record: the cube face this pass writes and the
/// roughness it convolves at, each rounded up to 16 bytes by WGSL's uniform
/// layout rules.
const PREFILTER_UNIFORM_SIZE: u64 = 32;

const PREFILTER_WGSL: &str = r#"
// 1024 GGX-importance-sampled directions per texel. Matches the BRDF LUT's
// count deliberately: the two are the same integral split in half, and the
// prefilter is the half where too few samples show up as visible fireflies
// around bright spots rather than as a small bias.
const SAMPLE_COUNT: u32 = 1024u;

struct PrefilterUniform {
    // .x is the cube face this pass renders.
    face: vec4<u32>,
    // .x is the roughness this mip represents.
    params: vec4<f32>,
};
@group(0) @binding(0) var<uniform> pf: PrefilterUniform;
@group(0) @binding(1) var t_env: texture_cube<f32>;
@group(0) @binding(2) var s_env: sampler;

@fragment
fn fs_prefilter(in: FullscreenOut) -> @location(0) vec4<f32> {
    let n = cube_face_direction(pf.face.x, in.uv);
    // The split-sum approximation's first half is evaluated with n = v = r, so
    // one map per roughness can be indexed by reflection direction alone.
    // That is what makes this a cubemap instead of a five-dimensional table;
    // the price is that grazing reflections come out a little too round, which
    // is the standard trade every real-time IBL implementation makes.
    let v = n;
    let roughness = pf.params.x;

    var color = vec3<f32>(0.0);
    var weight = 0.0;
    for (var i = 0u; i < SAMPLE_COUNT; i++) {
        let xi = hammersley(i, SAMPLE_COUNT);
        let h = importance_sample_ggx(xi, n, roughness);
        let l = normalize(2.0 * dot(v, h) * h - v);
        let n_dot_l = dot(n, l);
        if (n_dot_l > 0.0) {
            // Weighted by n_dot_l rather than averaged flat: grazing samples
            // contribute proportionally less, which is measurably closer to
            // ground truth at any sample count that is affordable here.
            color += textureSampleLevel(t_env, s_env, l, 0.0).rgb * n_dot_l;
            weight += n_dot_l;
        }
    }
    // At roughness 0 the GGX lobe collapses to a single direction, every
    // sample lands on n, and this returns the environment unblurred. Nothing
    // special-cases that -- it falls out of the same loop.
    return vec4<f32>(color / max(weight, 1e-4), 1.0);
}
"#;

/// Convolves an environment cubemap into the prefiltered specular map: a
/// [`PREFILTER_CUBE_SIZE`]-square [`ENV_CUBE_FORMAT`] cubemap with
/// [`PREFILTER_MIP_LEVELS`] mips, mip `m` holding the environment convolved
/// with the GGX lobe at `roughness = m / (PREFILTER_MIP_LEVELS - 1)`.
///
/// So mip 0 is roughness 0 -- a mirror, the environment unchanged -- and the
/// last mip is roughness 1. The material shader picks a level with
/// `roughness * (PREFILTER_MIP_LEVELS - 1)` and lets hardware trilinear
/// filtering interpolate between the two nearest.
///
/// **This runs 6 x [`PREFILTER_MIP_LEVELS`] = 30 render passes**, one per
/// (face, mip) pair, because each pair convolves at a different roughness and
/// writes a different [`face_view`]. That is not a mistake to optimise away:
/// it happens once per skybox change, not once per frame.
///
/// Returns the tracked texture, which the caller must keep alive for the
/// profiler to keep counting it, alongside a `Cube` view spanning every mip.
pub fn prefilter_from_cubemap(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    env_cube_view: &wgpu::TextureView,
    sampler: &wgpu::Sampler,
) -> (TrackedTexture, wgpu::TextureView) {
    let texture = create_cubemap(
        device,
        "ibl prefilter cube",
        PREFILTER_CUBE_SIZE,
        PREFILTER_MIP_LEVELS,
        ENV_CUBE_FORMAT,
        wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::TEXTURE_BINDING
            // COPY_SRC so a test can read mip 0 and the last mip back and
            // confirm they are not the same image. If the roughness-per-mip
            // mapping were dropped entirely, every downstream test would still
            // pass while specular IBL was silently a mirror at every gloss.
            | wgpu::TextureUsages::COPY_SRC,
    );

    // One record per (face, mip) pair, so all 30 passes share one buffer and
    // one bind group and differ only by dynamic offset.
    let pass_count = CUBE_FACES * PREFILTER_MIP_LEVELS;
    let uniforms = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("ibl prefilter uniform"),
        size: FACE_UNIFORM_STRIDE * pass_count as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let prefilter_offset =
        |face: u32, mip: u32| (mip * CUBE_FACES + face) as u64 * FACE_UNIFORM_STRIDE;
    for mip in 0..PREFILTER_MIP_LEVELS {
        // Mip 0 is a mirror and the last mip is fully rough; everything in
        // between is spaced evenly, which is exactly the mapping the material
        // shader inverts when it converts a material's roughness into a level.
        let roughness = mip as f32 / (PREFILTER_MIP_LEVELS - 1) as f32;
        for face in 0..CUBE_FACES {
            let mut record = [0u8; PREFILTER_UNIFORM_SIZE as usize];
            record[..4].copy_from_slice(&face.to_le_bytes());
            record[16..20].copy_from_slice(&roughness.to_le_bytes());
            queue.write_buffer(&uniforms, prefilter_offset(face, mip), &record);
        }
    }

    let bgl = face_pass_bind_group_layout(
        device,
        "ibl prefilter bgl",
        PREFILTER_UNIFORM_SIZE,
        wgpu::TextureViewDimension::Cube,
    );
    let bind_group = face_pass_bind_group(
        device,
        "ibl prefilter bg",
        &bgl,
        &uniforms,
        PREFILTER_UNIFORM_SIZE,
        env_cube_view,
        sampler,
    );
    let pipeline = face_pass_pipeline(
        device,
        "ibl prefilter",
        &format!("{COMMON_WGSL}{IMPORTANCE_SAMPLING_WGSL}{PREFILTER_WGSL}"),
        "fs_prefilter",
        &bgl,
        ENV_CUBE_FORMAT,
    );

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("ibl prefilter encoder"),
    });
    for mip in 0..PREFILTER_MIP_LEVELS {
        for face in 0..CUBE_FACES {
            draw_face_pass(
                &mut encoder,
                "ibl prefilter pass",
                &face_view(&texture, face, mip),
                &pipeline,
                &bind_group,
                prefilter_offset(face, mip),
            );
        }
    }
    queue.submit(std::iter::once(encoder.finish()));

    let view = cube_view(&texture);
    (texture, view)
}

/// The environment-dependent IBL maps, rebuilt whenever the skybox changes.
///
/// **The BRDF integration LUT is deliberately not in here.** It depends only on
/// the BRDF -- not the environment, the scene, or the camera -- so it is
/// generated once at startup, lives on the surface, and outlives every skybox.
/// These two are convolutions *of* a particular environment, so they are thrown
/// away and rebuilt each time that environment changes. Two schedules, two
/// homes: putting the LUT here would mean re-integrating it on every skybox
/// swap for a table that cannot have changed.
pub struct IblMaps {
    /// Cube view of the irradiance map: diffuse IBL, sampled by surface normal.
    pub irradiance_view: wgpu::TextureView,
    /// Held only to keep the texture `irradiance_view` looks at alive -- and
    /// counted in the profiler -- for as long as that view can be bound.
    _irradiance: TrackedTexture,
    /// Cube view of the roughness-mipped prefiltered map: specular IBL, sampled
    /// by reflection direction at a mip level chosen from roughness.
    pub prefilter_view: wgpu::TextureView,
    /// Held alive for the same reason as [`Self::_irradiance`].
    _prefilter: TrackedTexture,
}

impl IblMaps {
    /// Runs all three environment passes against an equirectangular source:
    /// [`equirect_to_cubemap`], then [`irradiance_from_cubemap`] and
    /// [`prefilter_from_cubemap`] over the cubemap that produced.
    ///
    /// `equirect_view` and `sampler` are the skybox's own texture view and
    /// sampler, so the environment reflected off materials is read through the
    /// same texture, with the same sRGB decode and the same filtering, as the
    /// sky the camera sees.
    ///
    /// **The sharp environment cubemap is an intermediate and is not kept.**
    /// Nothing downstream samples it: the sky itself is still drawn from the
    /// original equirectangular texture by `fs_sky`, and materials read only the
    /// two convolutions. Dropping it here is safe even though its passes have
    /// only been submitted, not completed -- wgpu keeps a resource alive until
    /// the submissions referencing it finish.
    ///
    /// This is not cheap: 6 + 6 + 30 render passes, each Monte-Carlo
    /// integrating per texel. It runs once per skybox change, never per frame.
    pub fn generate(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        equirect_view: &wgpu::TextureView,
        sampler: &wgpu::Sampler,
    ) -> Self {
        let (_env_cube, env_view) = equirect_to_cubemap(device, queue, equirect_view, sampler);
        let (irradiance, irradiance_view) =
            irradiance_from_cubemap(device, queue, &env_view, sampler);
        let (prefilter, prefilter_view) = prefilter_from_cubemap(device, queue, &env_view, sampler);
        Self {
            irradiance_view,
            _irradiance: irradiance,
            prefilter_view,
            _prefilter: prefilter,
        }
    }
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

    /// Reads one mip of one array layer of an [`ENV_CUBE_FORMAT`] texture back
    /// as linear RGBA floats.
    ///
    /// Same padded-row idiom as [`crate::output::read_pixels`], which Task 1
    /// reused for the LUT, widened where a cube face does not fit it: that one
    /// is hard-wired to mip 0 of array layer 0 at four bytes per texel, and a
    /// prefiltered face is mip `mip` of layer `layer` at eight bytes per texel.
    fn read_rgba16f_layer(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        texture: &wgpu::Texture,
        width: u32,
        height: u32,
        layer: u32,
        mip: u32,
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
                mip_level: mip,
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
        build_equirect(encode_direction)
    }

    /// Builds an equirectangular panorama by asking `texel` what colour the
    /// direction each texel represents should be.
    ///
    /// The direction it passes is the inverse of `fs_sky`'s mapping (`u` is
    /// longitude, `v` is latitude measured downwards), so every environment
    /// built through here is by construction the thing that mapping expects to
    /// read -- and every test environment agrees with every other about where a
    /// given direction lives.
    fn build_equirect(texel: impl Fn(Vec3) -> [u8; 4]) -> Vec<u8> {
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
                pixels.extend_from_slice(&texel(dir));
            }
        }
        pixels
    }

    /// Uploads [`direction_encoded_equirect`] with a skybox-shaped sampler.
    fn upload_direction_encoded_equirect(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> (wgpu::Texture, wgpu::TextureView, wgpu::Sampler) {
        upload_equirect(device, queue, &direction_encoded_equirect())
    }

    /// Uploads an equirectangular panorama and returns it with a sampler
    /// configured exactly as `set_skybox_from_rgba`'s is -- `Repeat` across the
    /// longitude seam, `ClampToEdge` at the poles, linear both ways.
    ///
    /// **`Rgba8Unorm`, not the `Rgba8UnormSrgb` the real skybox uses.** These
    /// texels are encoded test data -- a direction, or a known linear radiance
    /// -- rather than photographic colour, and an sRGB decode would bend both
    /// out of shape before they could be checked against the numbers they were
    /// written as. Every pass here reads whatever the view's format declares
    /// and does nothing format-specific, so this narrows nothing about what is
    /// under test: where directions land and how energy is normalised, not
    /// colour management.
    fn upload_equirect(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        pixels: &[u8],
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
            pixels,
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

        read_rgba16f_layer(device, queue, &target, PROBE_COUNT as u32, 1, 0, 0)
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
                0,
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

    // ---------------------------------------------------------------------
    // Irradiance convolution
    // ---------------------------------------------------------------------

    #[test]
    fn irradiance_of_a_uniform_environment_is_that_same_colour() {
        let (device, queue) = pollster::block_on(WgpuSurface::headless_device_for_testing());
        device.push_error_scope(wgpu::ErrorFilter::Validation);

        // 51, 153 and 102 over 255 are exactly 0.2, 0.6 and 0.4 -- three
        // different values on purpose, so a swapped channel or a collapse to
        // luminance shows up as the wrong colour rather than merely the wrong
        // brightness.
        const WANT: [f32; 3] = [0.2, 0.6, 0.4];
        let (_src, src_view, src_sampler) =
            upload_equirect(&device, &queue, &build_equirect(|_| [51, 153, 102, 255]));
        let (_env, env_view) = equirect_to_cubemap(&device, &queue, &src_view, &src_sampler);
        let (irradiance, _irradiance_view) =
            irradiance_from_cubemap(&device, &queue, &env_view, &src_sampler);

        assert_eq!(irradiance.width(), IRRADIANCE_CUBE_SIZE);
        assert_eq!(irradiance.height(), IRRADIANCE_CUBE_SIZE);
        assert_eq!(irradiance.size().depth_or_array_layers, CUBE_FACES);

        // The cosine-weighted hemisphere integral of a constant radiance L is
        // exactly PI * L, and this pass divides by PI, so every texel of every
        // face must come back as L itself. That is the point of checking a
        // uniform environment: the answer is a number, not a judgement. A pass
        // that dropped the sin(theta) Jacobian, or divided by the sample count
        // instead of scaling by the step sizes, produces a map that is
        // uniformly too bright or too dark -- and a uniformly 2x map looks
        // entirely plausible on screen.
        //
        // The tolerance covers three known, bounded error sources and nothing
        // else: the midpoint rule's 0.04% discretisation bias, and half-float
        // rounding of both the environment cube and this map, each ~0.05%.
        // Measured worst case across all 6144 texels is 0.065%, so 1% leaves
        // fifteen times that for another GPU's rounding, while anything
        // structurally wrong misses by tens of percent at least.
        const TOLERANCE: f32 = 0.01;

        let mut worst = 0.0f32;
        for face in 0..CUBE_FACES {
            let texels = read_rgba16f_layer(
                &device,
                &queue,
                &irradiance,
                IRRADIANCE_CUBE_SIZE,
                IRRADIANCE_CUBE_SIZE,
                face,
                0,
            );
            assert_eq!(
                texels.len(),
                (IRRADIANCE_CUBE_SIZE * IRRADIANCE_CUBE_SIZE) as usize
            );
            for (i, texel) in texels.iter().enumerate() {
                let x = i as u32 % IRRADIANCE_CUBE_SIZE;
                let y = i as u32 / IRRADIANCE_CUBE_SIZE;
                for (channel, want) in WANT.iter().enumerate() {
                    let got = texel[channel];
                    // is_finite first: the tangent frame degenerates where the
                    // normal is parallel to the up vector, and an unguarded
                    // normalize() there writes NaN into the texels nearest the
                    // poles -- which a relative comparison alone reports as a
                    // baffling magnitude failure instead.
                    assert!(
                        got.is_finite(),
                        "face {face} texel ({x}, {y}) channel {channel} is not finite: {got}"
                    );
                    let relative_error = (got - want).abs() / want;
                    worst = worst.max(relative_error);
                    assert!(
                        relative_error < TOLERANCE,
                        "a uniform environment must convolve to its own colour: face {face} \
                         texel ({x}, {y}) channel {channel} is {got}, want {want} \
                         (relative error {relative_error})"
                    );
                }
            }
        }

        // Not a restatement of the loop: it pins how much of the budget the
        // pass actually uses, so a future change that quietly eats most of the
        // tolerance shows up here rather than at the moment it finally crosses
        // the line.
        assert!(
            worst < TOLERANCE * 0.5,
            "the convolution should sit well inside its tolerance, not at the \
             edge of it; worst relative error was {worst}"
        );

        device.poll(wgpu::Maintain::Wait);
        assert!(
            pollster::block_on(device.pop_error_scope()).is_none(),
            "the irradiance convolution must not raise a validation error"
        );
    }

    // ---------------------------------------------------------------------
    // Specular prefilter
    // ---------------------------------------------------------------------

    /// Minimum, maximum and mean of the RGB average over one prefiltered face.
    ///
    /// The prefilter test's environment is white-on-grey, so all three channels
    /// carry the same signal and averaging them is only noise reduction.
    fn face_stats(texels: &[[f32; 4]]) -> (f32, f32, f32) {
        let mut min = f32::INFINITY;
        let mut max = f32::NEG_INFINITY;
        let mut total = 0.0;
        for texel in texels {
            let value = (texel[0] + texel[1] + texel[2]) / 3.0;
            assert!(
                value.is_finite(),
                "prefiltered texel is not finite: {texel:?}"
            );
            min = min.min(value);
            max = max.max(value);
            total += value;
        }
        (min, max, total / texels.len() as f32)
    }

    #[test]
    fn prefilter_mip_zero_is_sharp_and_the_last_mip_is_blurred() {
        let (device, queue) = pollster::block_on(WgpuSurface::headless_device_for_testing());
        device.push_error_scope(wgpu::ErrorFilter::Validation);

        // A dim environment with one bright spot straight down +Z, which is the
        // direction the centre of cube face 4 looks along. A smooth environment
        // could not tell a sharp convolution from a blurred one: blurring
        // something already flat changes nothing.
        //
        // The spot's angular radius is not free to pick. The last mip's face is
        // only 8 x 8, each texel covering about 11 degrees, so a spot smaller
        // than that falls between texel centres and vanishes from the last mip
        // *whatever* roughness it was convolved at -- and this test would then
        // pass on an unblurred map, which is exactly the failure it exists to
        // catch. 20 degrees is comfortably wider than one texel there while
        // still occupying only a tenth of the face at mip 0.
        const SPOT_RADIUS_DEGREES: f32 = 20.0;
        const SPOT_FACE: u32 = 4;
        let cos_radius = SPOT_RADIUS_DEGREES.to_radians().cos();
        let pixels = build_equirect(|dir| {
            if dir.normalize().dot(Vec3::Z) >= cos_radius {
                [255, 255, 255, 255]
            } else {
                [5, 5, 5, 255]
            }
        });
        let (_src, src_view, src_sampler) = upload_equirect(&device, &queue, &pixels);
        let (_env, env_view) = equirect_to_cubemap(&device, &queue, &src_view, &src_sampler);
        let (prefiltered, _prefiltered_view) =
            prefilter_from_cubemap(&device, &queue, &env_view, &src_sampler);

        assert_eq!(prefiltered.width(), PREFILTER_CUBE_SIZE);
        assert_eq!(prefiltered.height(), PREFILTER_CUBE_SIZE);
        assert_eq!(prefiltered.size().depth_or_array_layers, CUBE_FACES);
        assert_eq!(prefiltered.mip_level_count(), PREFILTER_MIP_LEVELS);

        let last_mip = PREFILTER_MIP_LEVELS - 1;
        let last_size = PREFILTER_CUBE_SIZE >> last_mip;
        let read = |mip: u32, size: u32| {
            read_rgba16f_layer(&device, &queue, &prefiltered, size, size, SPOT_FACE, mip)
        };
        let (sharp_min, sharp_max, sharp_mean) = face_stats(&read(0, PREFILTER_CUBE_SIZE));
        let (blurred_min, blurred_max, blurred_mean) = face_stats(&read(last_mip, last_size));
        let sharp_contrast = sharp_max - sharp_min;
        let blurred_contrast = blurred_max - blurred_min;

        // Mip 0 is roughness 0, where the GGX lobe collapses to a single
        // direction: the spot must still be a spot, at very nearly its full
        // brightness against the dim background.
        assert!(
            sharp_max > 0.9,
            "mip 0 is a mirror and must still hold the bright spot at full \
             brightness, but its brightest texel is only {sharp_max}"
        );
        assert!(
            sharp_contrast > 0.8,
            "mip 0 must keep the spot sharply separated from its surroundings, \
             but its contrast is only {sharp_contrast} \
             (min {sharp_min}, max {sharp_max})"
        );

        // The last mip is roughness 1, where the lobe covers the hemisphere and
        // smears the spot across the whole face. If both mips came out equally
        // sharp, the roughness-per-mip mapping is not being applied at all --
        // and every downstream test would still pass while specular IBL was
        // silently a mirror at every gloss.
        assert!(
            blurred_contrast < sharp_contrast * 0.2,
            "the last mip must be visibly blurrier than mip 0, but their \
             contrasts are {blurred_contrast} and {sharp_contrast} -- the \
             roughness-per-mip mapping is not being applied"
        );
        // The peak says the same thing from the other side, and independently
        // of the background: a mirror-sharp last mip still reads 1.0 at the
        // spot, while a properly convolved one cannot, because the spot's
        // energy is now spread over the whole lobe.
        assert!(
            blurred_max < sharp_max * 0.5,
            "the last mip must not still contain the spot at near-full \
             brightness: mip 0 peak {sharp_max} vs last mip peak {blurred_max}"
        );

        // The opposite-direction regression, and the reason those two are not
        // enough on their own: a pass that wrote black, or garbage, or nothing
        // at all into the high mips would have neither contrast nor a peak and
        // would sail through both. Blurring redistributes energy, it does not
        // destroy it, so the blurred face's mean must stay in the same
        // neighbourhood as the sharp one's.
        assert!(
            blurred_mean > sharp_mean * 0.5 && blurred_mean < sharp_mean * 2.0,
            "blurring must redistribute the environment's energy, not discard \
             it: mip 0 mean {sharp_mean} vs last mip mean {blurred_mean}"
        );

        device.poll(wgpu::Maintain::Wait);
        assert!(
            pollster::block_on(device.pop_error_scope()).is_none(),
            "the specular prefilter must not raise a validation error"
        );
    }
}
