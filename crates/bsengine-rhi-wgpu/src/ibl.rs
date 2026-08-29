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

/// A cubemap has six faces; in wgpu they are six array layers of a 2D
/// texture, in the fixed order +X, -X, +Y, -Y, +Z, -Z.
const CUBE_FACES: u32 = 6;

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

const BRDF_LUT_WGSL: &str = r#"
const PI: f32 = 3.14159265359;
// Sample count for the Monte Carlo integration. 1024 Hammersley points is the
// standard figure; because this LUT is generated once ever, the cost is paid
// at startup and never again.
const SAMPLE_COUNT: u32 = 1024u;

struct FullscreenOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
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
        source: wgpu::ShaderSource::Wgsl(BRDF_LUT_WGSL.into()),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::surface::WgpuSurface;

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
}
