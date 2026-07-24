/// Texture format used for the HDR scene-color render target, before tonemapping.
pub const HDR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
const BLOOM_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
const AO_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const CONFIG_SIZE: u64 = 64;
const SSAO_CAM_SIZE: u64 = 128;

const BLOOM_WGSL: &str = r#"
struct FullscreenOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
}
struct PostProcessConfig {
    bloom_threshold: f32, bloom_softness: f32, bloom_intensity: f32, bloom_radius: f32,
    bloom_enabled: u32, tonemap_mode: u32, tonemap_exposure: f32, tonemap_enabled: u32,
    ssao_radius: f32, ssao_bias: f32, ssao_intensity: f32, ssao_sample_count: u32,
    ssao_enabled: u32, _pad0: f32, _pad1: f32, _pad2: f32,
}
@group(0) @binding(0) var hdr_tex: texture_2d<f32>;
@group(0) @binding(1) var tex_sampler: sampler;
@group(1) @binding(0) var<uniform> config: PostProcessConfig;

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

@fragment
fn fs_bloom(in: FullscreenOut) -> @location(0) vec4<f32> {
    if config.bloom_enabled == 0u {
        return vec4<f32>(0.0, 0.0, 0.0, 0.0);
    }
    let dims = vec2<f32>(textureDimensions(hdr_tex, 0));
    let texel = vec2<f32>(1.0 / dims.x, 1.0 / dims.y);
    let radius_px = config.bloom_radius;
    var accum = vec3<f32>(0.0);
    var weight_sum = 0.0;
    let steps = 4;
    for (var dy = -steps; dy <= steps; dy++) {
        for (var dx = -steps; dx <= steps; dx++) {
            let offset = vec2<f32>(f32(dx), f32(dy)) * texel * (radius_px / f32(steps));
            let sample_uv = clamp(in.uv + offset, vec2<f32>(0.0), vec2<f32>(1.0));
            let color = textureSample(hdr_tex, tex_sampler, sample_uv).rgb;
            let lum = dot(color, vec3<f32>(0.2126, 0.7152, 0.0722));
            let t = config.bloom_threshold;
            let soft = config.bloom_softness * t;
            let knee_low = t - soft;
            let knee_high = t + soft;
            let blend = clamp((lum - knee_low) / max(knee_high - knee_low, 0.0001), 0.0, 1.0);
            let bright = color * blend;
            let w = exp(-0.5 * f32(dx * dx + dy * dy) / f32(steps * steps));
            accum += bright * w;
            weight_sum += w;
        }
    }
    return vec4<f32>(accum / weight_sum * config.bloom_intensity, 1.0);
}
"#;

const SSAO_WGSL: &str = r#"
struct FullscreenOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
}
struct PostProcessConfig {
    bloom_threshold: f32, bloom_softness: f32, bloom_intensity: f32, bloom_radius: f32,
    bloom_enabled: u32, tonemap_mode: u32, tonemap_exposure: f32, tonemap_enabled: u32,
    ssao_radius: f32, ssao_bias: f32, ssao_intensity: f32, ssao_sample_count: u32,
    ssao_enabled: u32, _pad0: f32, _pad1: f32, _pad2: f32,
}
struct SsaoCamera {
    proj: mat4x4<f32>,
    inv_proj: mat4x4<f32>,
}
@group(0) @binding(0) var depth_tex: texture_depth_2d;
@group(1) @binding(0) var<uniform> config: PostProcessConfig;
@group(2) @binding(0) var<uniform> cam: SsaoCamera;

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

fn load_depth(uv: vec2<f32>) -> f32 {
    let dims = vec2<i32>(textureDimensions(depth_tex, 0));
    let coord = clamp(vec2<i32>(uv * vec2<f32>(dims)), vec2<i32>(0), dims - vec2<i32>(1));
    return textureLoad(depth_tex, coord, 0);
}

fn reconstruct_view_pos(uv: vec2<f32>, depth: f32) -> vec3<f32> {
    let ndc = vec4<f32>(uv.x * 2.0 - 1.0, -(uv.y * 2.0 - 1.0), depth, 1.0);
    let view_pos = cam.inv_proj * ndc;
    return view_pos.xyz / view_pos.w;
}

@fragment
fn fs_ssao(in: FullscreenOut) -> @location(0) vec4<f32> {
    if config.ssao_enabled == 0u {
        return vec4<f32>(1.0, 1.0, 1.0, 1.0);
    }
    let depth = load_depth(in.uv);
    if depth >= 1.0 {
        return vec4<f32>(1.0, 1.0, 1.0, 1.0);
    }
    let pos = reconstruct_view_pos(in.uv, depth);
    let dims = vec2<f32>(textureDimensions(depth_tex, 0));
    let texel = vec2<f32>(1.0 / dims.x, 1.0 / dims.y);
    let pos_r = reconstruct_view_pos(in.uv + vec2<f32>(texel.x, 0.0), load_depth(in.uv + vec2<f32>(texel.x, 0.0)));
    let pos_u = reconstruct_view_pos(in.uv + vec2<f32>(0.0, texel.y), load_depth(in.uv + vec2<f32>(0.0, texel.y)));
    let normal = normalize(cross(pos_r - pos, pos_u - pos));
    var hemisphere: array<vec3<f32>, 8> = array<vec3<f32>, 8>(
        vec3<f32>( 0.5411,  0.5,     0.5),
        vec3<f32>(-0.5411,  0.5,     0.5),
        vec3<f32>( 0.5,    -0.5411,  0.5),
        vec3<f32>(-0.5,    -0.5411,  0.5),
        vec3<f32>( 0.7071,  0.0,     0.3),
        vec3<f32>(-0.7071,  0.0,     0.3),
        vec3<f32>( 0.0,     0.7071,  0.3),
        vec3<f32>( 0.0,    -0.7071,  0.3),
    );
    var occlusion = 0.0;
    let max_samples = min(config.ssao_sample_count, 8u);
    for (var i: u32 = 0u; i < max_samples; i++) {
        let s = normalize(hemisphere[i] + normal * 0.5);
        let sample_pos = pos + s * config.ssao_radius;
        let clip = cam.proj * vec4<f32>(sample_pos, 1.0);
        if clip.w <= 0.0 { continue; }
        let ndc = clip.xyz / clip.w;
        let sample_uv = vec2<f32>(ndc.x * 0.5 + 0.5, -ndc.y * 0.5 + 0.5);
        if sample_uv.x < 0.0 || sample_uv.x > 1.0 || sample_uv.y < 0.0 || sample_uv.y > 1.0 {
            continue;
        }
        let actual_pos = reconstruct_view_pos(sample_uv, load_depth(sample_uv));
        let range_check = smoothstep(0.0, 1.0, config.ssao_radius / max(abs(pos.z - actual_pos.z), 0.0001));
        if actual_pos.z >= sample_pos.z + config.ssao_bias {
            occlusion += range_check;
        }
    }
    let ao = 1.0 - (occlusion / f32(max_samples)) * config.ssao_intensity;
    return vec4<f32>(ao, ao, ao, 1.0);
}
"#;

const COMPOSITE_WGSL: &str = r#"
struct FullscreenOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
}
struct PostProcessConfig {
    bloom_threshold: f32, bloom_softness: f32, bloom_intensity: f32, bloom_radius: f32,
    bloom_enabled: u32, tonemap_mode: u32, tonemap_exposure: f32, tonemap_enabled: u32,
    ssao_radius: f32, ssao_bias: f32, ssao_intensity: f32, ssao_sample_count: u32,
    ssao_enabled: u32, _pad0: f32, _pad1: f32, _pad2: f32,
}
@group(0) @binding(0) var hdr_tex: texture_2d<f32>;
@group(0) @binding(1) var hdr_sampler: sampler;
@group(1) @binding(0) var bloom_tex: texture_2d<f32>;
@group(1) @binding(1) var bloom_sampler: sampler;
@group(2) @binding(0) var ao_tex: texture_2d<f32>;
@group(2) @binding(1) var ao_sampler: sampler;
@group(3) @binding(0) var<uniform> config: PostProcessConfig;

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

fn aces(x: vec3<f32>) -> vec3<f32> {
    let a = 2.51; let b = 0.03; let c = 2.43; let d = 0.59; let e = 0.14;
    return clamp((x * (a * x + b)) / (x * (c * x + d) + e), vec3<f32>(0.0), vec3<f32>(1.0));
}
fn reinhard(x: vec3<f32>) -> vec3<f32> {
    return x / (x + vec3<f32>(1.0));
}
fn reinhard_lum(x: vec3<f32>) -> vec3<f32> {
    let lum = dot(x, vec3<f32>(0.2126, 0.7152, 0.0722));
    let mapped = lum / (lum + 1.0);
    return x * (mapped / max(lum, 0.0001));
}
fn filmic(x: vec3<f32>) -> vec3<f32> {
    let tmp = max(vec3<f32>(0.0), x - vec3<f32>(0.004));
    return (tmp * (6.2 * tmp + vec3<f32>(0.5))) / (tmp * (6.2 * tmp + vec3<f32>(1.7)) + vec3<f32>(0.06));
}

fn apply_tonemap(color: vec3<f32>) -> vec3<f32> {
    let scaled = color * pow(2.0, config.tonemap_exposure);
    if config.tonemap_enabled == 0u { return clamp(scaled, vec3<f32>(0.0), vec3<f32>(1.0)); }
    if config.tonemap_mode == 0u { return clamp(scaled, vec3<f32>(0.0), vec3<f32>(1.0)); }
    if config.tonemap_mode == 1u { return reinhard(scaled); }
    if config.tonemap_mode == 2u { return reinhard_lum(scaled); }
    if config.tonemap_mode == 4u { return filmic(scaled); }
    return aces(scaled);
}

@fragment
fn fs_composite(in: FullscreenOut) -> @location(0) vec4<f32> {
    let hdr   = textureSample(hdr_tex,   hdr_sampler,   in.uv).rgb;
    let bloom = textureSample(bloom_tex, bloom_sampler, in.uv).rgb;
    let ao    = textureSample(ao_tex,    ao_sampler,    in.uv).r;
    let combined = hdr * ao + bloom;
    return vec4<f32>(apply_tonemap(combined), 1.0);
}
"#;

/// GPU-uniform-buffer layout for bloom/tonemap/SSAO settings, matching the
/// `PostProcessConfig` struct declared in the WGSL shaders above.
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct PostProcessConfigGpu {
    /// Luminance level above which pixels start contributing to bloom.
    pub bloom_threshold: f32,
    /// Width of the soft knee around `bloom_threshold`.
    pub bloom_softness: f32,
    /// Multiplier applied to the bloom contribution during composite.
    pub bloom_intensity: f32,
    /// Blur sample radius in pixels for the bloom pass.
    pub bloom_radius: f32,
    /// Nonzero to enable the bloom pass; zero disables it.
    pub bloom_enabled: u32,
    /// Selects the tonemap curve (0=clamp, 1=Reinhard, 2=Reinhard-luminance, 4=filmic, else ACES).
    pub tonemap_mode: u32,
    /// Exposure adjustment (stops, applied as `2^exposure`) before tonemapping.
    pub tonemap_exposure: f32,
    /// Nonzero to enable tonemapping; zero passes color through clamped but linear.
    pub tonemap_enabled: u32,
    /// World/view-space sample radius for SSAO occlusion checks.
    pub ssao_radius: f32,
    /// Depth bias added to avoid SSAO self-occlusion artifacts.
    pub ssao_bias: f32,
    /// Multiplier applied to the computed occlusion amount.
    pub ssao_intensity: f32,
    /// Number of hemisphere samples to take per pixel (capped at 8 in the shader).
    pub ssao_sample_count: u32,
    /// Nonzero to enable the SSAO pass; zero always returns full visibility.
    pub ssao_enabled: u32,
    /// Padding to satisfy uniform buffer alignment; unused.
    pub _pad0: f32,
    /// Padding to satisfy uniform buffer alignment; unused.
    pub _pad1: f32,
    /// Padding to satisfy uniform buffer alignment; unused.
    pub _pad2: f32,
}

/// GPU-uniform-buffer layout for the camera matrices the SSAO pass needs to
/// reconstruct view-space position from depth.
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SsaoCameraGpu {
    /// Camera projection matrix.
    pub proj: [[f32; 4]; 4],
    /// Inverse of `proj`, used to unproject depth back to view space.
    pub inv_proj: [[f32; 4]; 4],
}

struct PostProcessTargets {
    hdr_texture: wgpu::Texture,
    hdr_view: wgpu::TextureView,
    hdr_bg: wgpu::BindGroup,
    bloom_texture: wgpu::Texture,
    bloom_view: wgpu::TextureView,
    bloom_bg: wgpu::BindGroup,
    ao_texture: wgpu::Texture,
    ao_view: wgpu::TextureView,
    ao_bg: wgpu::BindGroup,
    depth_bg: wgpu::BindGroup,
}

/// Owns the render targets, bind groups, and pipelines for the post-process
/// chain (bloom -> SSAO -> tonemapped composite onto the swapchain).
pub struct PostProcessState {
    /// View of the HDR scene-color render target the main pass writes into.
    pub hdr_view: wgpu::TextureView,
    _hdr_texture: wgpu::Texture,
    bloom_view: wgpu::TextureView,
    _bloom_texture: wgpu::Texture,
    ao_view: wgpu::TextureView,
    _ao_texture: wgpu::Texture,
    hdr_bg: wgpu::BindGroup,
    bloom_bg: wgpu::BindGroup,
    ao_bg: wgpu::BindGroup,
    depth_bg: wgpu::BindGroup,
    sampler: wgpu::Sampler,
    config_buffer: wgpu::Buffer,
    config_bg: wgpu::BindGroup,
    ssao_cam_buffer: wgpu::Buffer,
    ssao_cam_bg: wgpu::BindGroup,
    /// Pipeline for the bright-pass bloom extraction shader.
    pub bloom_pipeline: wgpu::RenderPipeline,
    /// Pipeline for the SSAO occlusion shader.
    pub ssao_pipeline: wgpu::RenderPipeline,
    /// Pipeline for the final composite (HDR + bloom + AO, tonemapped) shader.
    pub composite_pipeline: wgpu::RenderPipeline,
    tex2d_bgl: wgpu::BindGroupLayout,
    depth_bgl: wgpu::BindGroupLayout,
}

impl PostProcessState {
    /// Builds every pipeline, bind group layout, and sized render target needed
    /// for the post-process chain at the given resolution.
    pub fn new(
        device: &wgpu::Device,
        width: u32,
        height: u32,
        depth_view: &wgpu::TextureView,
        surface_format: wgpu::TextureFormat,
    ) -> Self {
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("pp sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let tex2d_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("pp tex2d bgl"),
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

        let depth_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("pp depth bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Depth,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            }],
        });

        let config_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("pp config bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: wgpu::BufferSize::new(CONFIG_SIZE),
                },
                count: None,
            }],
        });

        let ssao_cam_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("pp ssao cam bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: wgpu::BufferSize::new(SSAO_CAM_SIZE),
                },
                count: None,
            }],
        });

        let config_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("pp config buffer"),
            size: CONFIG_SIZE,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let config_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("pp config bg"),
            layout: &config_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: config_buffer.as_entire_binding(),
            }],
        });

        let ssao_cam_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("pp ssao cam buffer"),
            size: SSAO_CAM_SIZE,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let ssao_cam_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("pp ssao cam bg"),
            layout: &ssao_cam_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: ssao_cam_buffer.as_entire_binding(),
            }],
        });

        let bloom_pipeline = Self::make_bloom_pipeline(device, &tex2d_bgl, &config_bgl);
        let ssao_pipeline =
            Self::make_ssao_pipeline(device, &depth_bgl, &config_bgl, &ssao_cam_bgl);
        let composite_pipeline =
            Self::make_composite_pipeline(device, &tex2d_bgl, &config_bgl, surface_format);

        let targets = Self::create_targets(
            device, width, height, depth_view, &sampler, &tex2d_bgl, &depth_bgl,
        );

        Self {
            hdr_view: targets.hdr_view,
            _hdr_texture: targets.hdr_texture,
            bloom_view: targets.bloom_view,
            _bloom_texture: targets.bloom_texture,
            ao_view: targets.ao_view,
            _ao_texture: targets.ao_texture,
            hdr_bg: targets.hdr_bg,
            bloom_bg: targets.bloom_bg,
            ao_bg: targets.ao_bg,
            depth_bg: targets.depth_bg,
            sampler,
            config_buffer,
            config_bg,
            ssao_cam_buffer,
            ssao_cam_bg,
            bloom_pipeline,
            ssao_pipeline,
            composite_pipeline,
            tex2d_bgl,
            depth_bgl,
        }
    }

    fn create_targets(
        device: &wgpu::Device,
        width: u32,
        height: u32,
        depth_view: &wgpu::TextureView,
        sampler: &wgpu::Sampler,
        tex2d_bgl: &wgpu::BindGroupLayout,
        depth_bgl: &wgpu::BindGroupLayout,
    ) -> PostProcessTargets {
        let make_tex = |label: &str, fmt: wgpu::TextureFormat| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: fmt,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            })
        };

        let hdr_texture = make_tex("pp hdr", HDR_FORMAT);
        let hdr_view = hdr_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bloom_texture = make_tex("pp bloom", BLOOM_FORMAT);
        let bloom_view = bloom_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let ao_texture = make_tex("pp ao", AO_FORMAT);
        let ao_view = ao_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let make_tex2d_bg = |label: &str, view: &wgpu::TextureView| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(label),
                layout: tex2d_bgl,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(sampler),
                    },
                ],
            })
        };

        let hdr_bg = make_tex2d_bg("pp hdr bg", &hdr_view);
        let bloom_bg = make_tex2d_bg("pp bloom bg", &bloom_view);
        let ao_bg = make_tex2d_bg("pp ao bg", &ao_view);

        let depth_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("pp depth bg"),
            layout: depth_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(depth_view),
            }],
        });

        PostProcessTargets {
            hdr_texture,
            hdr_view,
            hdr_bg,
            bloom_texture,
            bloom_view,
            bloom_bg,
            ao_texture,
            ao_view,
            ao_bg,
            depth_bg,
        }
    }

    fn make_bloom_pipeline(
        device: &wgpu::Device,
        tex2d_bgl: &wgpu::BindGroupLayout,
        config_bgl: &wgpu::BindGroupLayout,
    ) -> wgpu::RenderPipeline {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("bloom shader"),
            source: wgpu::ShaderSource::Wgsl(BLOOM_WGSL.into()),
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("bloom pll"),
            bind_group_layouts: &[tex2d_bgl, config_bgl],
            push_constant_ranges: &[],
        });
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("bloom pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_fullscreen",
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_bloom",
                targets: &[Some(wgpu::ColorTargetState {
                    format: BLOOM_FORMAT,
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

    fn make_ssao_pipeline(
        device: &wgpu::Device,
        depth_bgl: &wgpu::BindGroupLayout,
        config_bgl: &wgpu::BindGroupLayout,
        ssao_cam_bgl: &wgpu::BindGroupLayout,
    ) -> wgpu::RenderPipeline {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("ssao shader"),
            source: wgpu::ShaderSource::Wgsl(SSAO_WGSL.into()),
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("ssao pll"),
            bind_group_layouts: &[depth_bgl, config_bgl, ssao_cam_bgl],
            push_constant_ranges: &[],
        });
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("ssao pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_fullscreen",
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_ssao",
                targets: &[Some(wgpu::ColorTargetState {
                    format: AO_FORMAT,
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

    fn make_composite_pipeline(
        device: &wgpu::Device,
        tex2d_bgl: &wgpu::BindGroupLayout,
        config_bgl: &wgpu::BindGroupLayout,
        surface_format: wgpu::TextureFormat,
    ) -> wgpu::RenderPipeline {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("composite shader"),
            source: wgpu::ShaderSource::Wgsl(COMPOSITE_WGSL.into()),
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("composite pll"),
            bind_group_layouts: &[tex2d_bgl, tex2d_bgl, tex2d_bgl, config_bgl],
            push_constant_ranges: &[],
        });
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("composite pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_fullscreen",
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_composite",
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
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

    /// Recreates the sized render targets (HDR/bloom/AO/depth bind group) for
    /// a new surface resolution, leaving pipelines and samplers untouched.
    pub fn resize_targets(
        &mut self,
        device: &wgpu::Device,
        depth_view: &wgpu::TextureView,
        width: u32,
        height: u32,
    ) {
        let t = Self::create_targets(
            device,
            width,
            height,
            depth_view,
            &self.sampler,
            &self.tex2d_bgl,
            &self.depth_bgl,
        );
        self.hdr_view = t.hdr_view;
        self._hdr_texture = t.hdr_texture;
        self.bloom_view = t.bloom_view;
        self._bloom_texture = t.bloom_texture;
        self.ao_view = t.ao_view;
        self._ao_texture = t.ao_texture;
        self.hdr_bg = t.hdr_bg;
        self.bloom_bg = t.bloom_bg;
        self.ao_bg = t.ao_bg;
        self.depth_bg = t.depth_bg;
    }

    /// Uploads new bloom/tonemap/SSAO settings to the config uniform buffer.
    pub fn update_config(&self, queue: &wgpu::Queue, config: PostProcessConfigGpu) {
        queue.write_buffer(&self.config_buffer, 0, bytemuck::cast_slice(&[config]));
    }

    /// Uploads the current frame's camera projection matrices for SSAO depth reconstruction.
    pub fn update_ssao_camera(&self, queue: &wgpu::Queue, cam: SsaoCameraGpu) {
        queue.write_buffer(&self.ssao_cam_buffer, 0, bytemuck::cast_slice(&[cam]));
    }

    /// Runs the bloom, SSAO, and composite passes in sequence, writing the
    /// final tonemapped result into `surface_view`.
    pub fn apply(&self, encoder: &mut wgpu::CommandEncoder, surface_view: &wgpu::TextureView) {
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("bloom pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.bloom_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                ..Default::default()
            });
            pass.set_pipeline(&self.bloom_pipeline);
            pass.set_bind_group(0, &self.hdr_bg, &[]);
            pass.set_bind_group(1, &self.config_bg, &[]);
            pass.draw(0..3, 0..1);
        }

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("ssao pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.ao_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::WHITE),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                ..Default::default()
            });
            pass.set_pipeline(&self.ssao_pipeline);
            pass.set_bind_group(0, &self.depth_bg, &[]);
            pass.set_bind_group(1, &self.config_bg, &[]);
            pass.set_bind_group(2, &self.ssao_cam_bg, &[]);
            pass.draw(0..3, 0..1);
        }

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("composite pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: surface_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                ..Default::default()
            });
            pass.set_pipeline(&self.composite_pipeline);
            pass.set_bind_group(0, &self.hdr_bg, &[]);
            pass.set_bind_group(1, &self.bloom_bg, &[]);
            pass.set_bind_group(2, &self.ao_bg, &[]);
            pass.set_bind_group(3, &self.config_bg, &[]);
            pass.draw(0..3, 0..1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rhi::WgpuRHI;

    #[test]
    fn config_gpu_size() {
        assert_eq!(std::mem::size_of::<PostProcessConfigGpu>(), 64);
    }

    #[test]
    fn ssao_cam_gpu_size() {
        assert_eq!(std::mem::size_of::<SsaoCameraGpu>(), 128);
    }

    #[test]
    fn hdr_format_is_rgba16float() {
        assert_eq!(HDR_FORMAT, wgpu::TextureFormat::Rgba16Float);
    }

    #[test]
    fn bloom_shader_compiles() {
        let rhi = pollster::block_on(WgpuRHI::new_headless()).expect("headless rhi");
        let _m = rhi
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: None,
                source: wgpu::ShaderSource::Wgsl(BLOOM_WGSL.into()),
            });
    }

    #[test]
    fn ssao_shader_compiles() {
        let rhi = pollster::block_on(WgpuRHI::new_headless()).expect("headless rhi");
        let _m = rhi
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: None,
                source: wgpu::ShaderSource::Wgsl(SSAO_WGSL.into()),
            });
    }

    #[test]
    fn composite_shader_compiles() {
        let rhi = pollster::block_on(WgpuRHI::new_headless()).expect("headless rhi");
        let _m = rhi
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: None,
                source: wgpu::ShaderSource::Wgsl(COMPOSITE_WGSL.into()),
            });
    }
}
