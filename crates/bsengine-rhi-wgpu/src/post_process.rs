/// Texture format used for the HDR scene-color render target, before tonemapping.
pub const HDR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
const BLOOM_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
const AO_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const CONFIG_SIZE: u64 = 64;
const SSAO_CAM_SIZE: u64 = 128;
const TAA_CAM_SIZE: u64 = 128;

const BLOOM_WGSL: &str = r#"
struct FullscreenOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
}
struct PostProcessConfig {
    bloom_threshold: f32, bloom_softness: f32, bloom_intensity: f32, bloom_radius: f32,
    bloom_enabled: u32, tonemap_mode: u32, tonemap_exposure: f32, tonemap_enabled: u32,
    ssao_radius: f32, ssao_bias: f32, ssao_intensity: f32, ssao_sample_count: u32,
    ssao_enabled: u32, taa_enabled: u32, taa_history_blend: f32, taa_clamp_strength: f32,
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
    ssao_enabled: u32, taa_enabled: u32, taa_history_blend: f32, taa_clamp_strength: f32,
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
    ssao_enabled: u32, taa_enabled: u32, taa_history_blend: f32, taa_clamp_strength: f32,
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

/// The temporal-antialiasing resolve pass.
///
/// It reads the composite pass's LDR result, reprojects the previous frame's
/// resolved image through the camera's motion, clamps that history to the
/// local 3x3 neighbourhood of the current frame, and blends the two. The
/// result is written twice -- to the swapchain and to the history target the
/// next frame will reproject from -- which is why the pass exists as a
/// separate blit at all: a pass cannot sample the texture it renders into.
///
/// Reprojection here is **camera-only**: there are no per-object motion
/// vectors, so a moving object's history lands at the wrong pixel by design.
/// The neighbourhood clamp is what keeps that error from smearing across the
/// frame -- it is not an optional quality knob.
///
/// With `config.taa_enabled == 0u` the shader returns the current frame
/// untouched, so the pass stays the exact passthrough it replaced.
const TAA_WGSL: &str = r#"
struct FullscreenOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
}
struct PostProcessConfig {
    bloom_threshold: f32, bloom_softness: f32, bloom_intensity: f32, bloom_radius: f32,
    bloom_enabled: u32, tonemap_mode: u32, tonemap_exposure: f32, tonemap_enabled: u32,
    ssao_radius: f32, ssao_bias: f32, ssao_intensity: f32, ssao_sample_count: u32,
    ssao_enabled: u32, taa_enabled: u32, taa_history_blend: f32, taa_clamp_strength: f32,
}
struct TaaCamera {
    inv_view_proj: mat4x4<f32>,
    prev_view_proj: mat4x4<f32>,
}
@group(0) @binding(0) var ldr_tex: texture_2d<f32>;
@group(0) @binding(1) var ldr_sampler: sampler;
@group(1) @binding(0) var history_tex: texture_2d<f32>;
@group(1) @binding(1) var history_sampler: sampler;
@group(2) @binding(0) var depth_tex: texture_depth_2d;
// Both uniforms share group 3: four groups is the WebGPU baseline
// `max_bind_groups`, and LDR/history/depth already take three.
@group(3) @binding(0) var<uniform> config: PostProcessConfig;
@group(3) @binding(1) var<uniform> cam: TaaCamera;

// The resolved colour goes to two attachments: @location(0) is the swapchain,
// @location(1) is next frame's history. They are always the same colour.
struct TaaOut {
    @location(0) color: vec4<f32>,
    @location(1) history: vec4<f32>,
}

fn taa_out(c: vec4<f32>) -> TaaOut {
    var out: TaaOut;
    out.color = c;
    out.history = c;
    return out;
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

@fragment
fn fs_taa(in: FullscreenOut) -> TaaOut {
    let current = textureSample(ldr_tex, ldr_sampler, in.uv);
    if config.taa_enabled == 0u {
        return taa_out(current);
    }

    let dims = vec2<f32>(textureDimensions(ldr_tex, 0));
    let texel = vec2<f32>(1.0 / dims.x, 1.0 / dims.y);

    // Neighbourhood bounds, gathered from THIS frame before any history is
    // consulted: the clamp below is only meaningful against the range the
    // current frame actually contains. These `textureSample` calls also have
    // to happen here, above the per-pixel early-outs, because implicit
    // derivatives are only legal in uniform control flow.
    var lo = current.rgb;
    var hi = current.rgb;
    for (var dy: i32 = -1; dy <= 1; dy = dy + 1) {
        for (var dx: i32 = -1; dx <= 1; dx = dx + 1) {
            let s = textureSample(
                ldr_tex, ldr_sampler,
                in.uv + vec2<f32>(f32(dx), f32(dy)) * texel
            ).rgb;
            lo = min(lo, s);
            hi = max(hi, s);
        }
    }

    let depth_dims = vec2<i32>(textureDimensions(depth_tex, 0));
    let coord = clamp(
        vec2<i32>(in.uv * vec2<f32>(depth_dims)),
        vec2<i32>(0),
        depth_dims - vec2<i32>(1)
    );
    let depth = textureLoad(depth_tex, coord, 0);
    // Nothing was drawn here: sky/background has no reliable surface to
    // reproject, so trust the current frame.
    if depth >= 1.0 {
        return taa_out(current);
    }

    // Pixel + depth -> world position -> where it was last frame.
    let ndc = vec4<f32>(in.uv.x * 2.0 - 1.0, 1.0 - in.uv.y * 2.0, depth, 1.0);
    let world_h = cam.inv_view_proj * ndc;
    let world = world_h.xyz / world_h.w;
    let prev_clip = cam.prev_view_proj * vec4<f32>(world, 1.0);
    // Behind the eye last frame: there is no previous-frame pixel to find.
    if prev_clip.w <= 0.0 {
        return taa_out(current);
    }
    let prev_ndc = prev_clip.xyz / prev_clip.w;
    let prev_uv = vec2<f32>(prev_ndc.x * 0.5 + 0.5, 0.5 - prev_ndc.y * 0.5);

    // History from outside the previous frame simply does not exist.
    if prev_uv.x < 0.0 || prev_uv.x > 1.0 || prev_uv.y < 0.0 || prev_uv.y > 1.0 {
        return taa_out(current);
    }

    // `textureSampleLevel`, not `textureSample`: this read sits under the
    // per-pixel branches above, and an implicit-derivative sample there is
    // non-uniform control flow. The history texture has a single mip, so
    // level 0 is the only level either call could have read.
    let history = textureSampleLevel(history_tex, history_sampler, prev_uv, 0.0).rgb;

    // Clamp history into the local range. This is what stops a moving
    // object -- which this version reprojects incorrectly, by design --
    // from smearing across the frame.
    let centre = (lo + hi) * 0.5;
    let extent = (hi - lo) * 0.5 * config.taa_clamp_strength;
    let clamped = clamp(history, centre - extent, centre + extent);

    return taa_out(vec4<f32>(
        mix(current.rgb, clamped, config.taa_history_blend),
        current.a
    ));
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
    /// Nonzero to enable the TAA pass; zero passes the composite result
    /// through unchanged.
    pub taa_enabled: u32,
    /// See `Taa::history_blend`.
    pub taa_history_blend: f32,
    /// See `Taa::clamp_strength`.
    pub taa_clamp_strength: f32,
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

/// GPU-uniform-buffer layout for the matrices the TAA pass needs to
/// reproject a pixel into the previous frame.
///
/// Both matrices are deliberately **unjittered**. Jitter belongs to
/// rasterization only: reprojecting with jittered matrices would chase the
/// jitter instead of the camera, and the accumulated image would never
/// converge.
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct TaaCameraGpu {
    /// Inverse of this frame's UNJITTERED view-projection, used to turn a
    /// pixel plus its depth back into a world-space position.
    pub inv_view_proj: [[f32; 4]; 4],
    /// The PREVIOUS frame's unjittered view-projection, used to find where
    /// that world position appeared last frame.
    pub prev_view_proj: [[f32; 4]; 4],
}

struct PostProcessTargets {
    hdr_texture: crate::profiler::TrackedTexture,
    hdr_view: wgpu::TextureView,
    hdr_bg: wgpu::BindGroup,
    bloom_texture: crate::profiler::TrackedTexture,
    bloom_view: wgpu::TextureView,
    bloom_bg: wgpu::BindGroup,
    ao_texture: crate::profiler::TrackedTexture,
    ao_view: wgpu::TextureView,
    ao_bg: wgpu::BindGroup,
    ldr_texture: crate::profiler::TrackedTexture,
    ldr_view: wgpu::TextureView,
    ldr_bg: wgpu::BindGroup,
    history_textures: [crate::profiler::TrackedTexture; 2],
    history_views: [wgpu::TextureView; 2],
    history_bgs: [wgpu::BindGroup; 2],
    depth_bg: wgpu::BindGroup,
}

/// Owns the render targets, bind groups, and pipelines for the post-process
/// chain (bloom -> SSAO -> tonemapped composite onto the swapchain).
pub struct PostProcessState {
    /// View of the HDR scene-color render target the main pass writes into.
    pub hdr_view: wgpu::TextureView,
    _hdr_texture: crate::profiler::TrackedTexture,
    bloom_view: wgpu::TextureView,
    _bloom_texture: crate::profiler::TrackedTexture,
    ao_view: wgpu::TextureView,
    _ao_texture: crate::profiler::TrackedTexture,
    /// LDR output of the composite pass. Composite used to write straight to
    /// the swapchain; it writes here instead so the TAA pass has a readable
    /// copy of this frame's finished colour -- a pass cannot sample the
    /// texture it is rendering into.
    ldr_view: wgpu::TextureView,
    _ldr_texture: crate::profiler::TrackedTexture,
    /// Two history targets, swapped each frame: one holds the previous
    /// frame's TAA output while the other receives this frame's. A single
    /// texture cannot be sampled and rendered to in the same pass.
    history_views: [wgpu::TextureView; 2],
    _history_textures: [crate::profiler::TrackedTexture; 2],
    history_bgs: [wgpu::BindGroup; 2],
    /// Index of the history texture holding the PREVIOUS frame's result.
    /// Flipped at the end of every `apply`.
    history_read: usize,
    /// False until a frame has been written to history. The first frame has
    /// no history to reproject, so it must pass through unblended rather
    /// than blending against an uninitialised texture.
    history_valid: bool,
    hdr_bg: wgpu::BindGroup,
    bloom_bg: wgpu::BindGroup,
    ao_bg: wgpu::BindGroup,
    ldr_bg: wgpu::BindGroup,
    depth_bg: wgpu::BindGroup,
    sampler: wgpu::Sampler,
    config_buffer: wgpu::Buffer,
    config_bg: wgpu::BindGroup,
    ssao_cam_buffer: wgpu::Buffer,
    ssao_cam_bg: wgpu::BindGroup,
    taa_cam_buffer: wgpu::Buffer,
    /// The TAA pass's single uniform group: the shared config buffer at
    /// binding 0 and `taa_cam_buffer` at binding 1. One group rather than the
    /// two the other passes use, because the resolve is already at the
    /// four-bind-group WebGPU baseline limit.
    taa_uniform_bg: wgpu::BindGroup,
    /// Pipeline for the bright-pass bloom extraction shader.
    pub bloom_pipeline: wgpu::RenderPipeline,
    /// Pipeline for the SSAO occlusion shader.
    pub ssao_pipeline: wgpu::RenderPipeline,
    /// Pipeline for the final composite (HDR + bloom + AO, tonemapped) shader.
    pub composite_pipeline: wgpu::RenderPipeline,
    /// Pipeline for the temporal-antialiasing resolve. Writes both the
    /// swapchain image and next frame's history target.
    pub taa_pipeline: wgpu::RenderPipeline,
    tex2d_bgl: wgpu::BindGroupLayout,
    depth_bgl: wgpu::BindGroupLayout,
    /// Format of the final swapchain image. The LDR intermediate target must
    /// match it exactly, or the round trip through it would requantize the
    /// frame; kept here so `resize_targets` can recreate that target.
    surface_format: wgpu::TextureFormat,
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

        // The resolve pass already needs four groups (LDR, history, depth,
        // uniforms) and `max_bind_groups` is 4 on the WebGPU baseline, so the
        // config and camera uniforms share one group rather than taking two.
        let taa_uniform_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("pp taa uniform bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(CONFIG_SIZE),
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(TAA_CAM_SIZE),
                    },
                    count: None,
                },
            ],
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

        let taa_cam_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("pp taa cam buffer"),
            size: TAA_CAM_SIZE,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let taa_uniform_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("pp taa uniform bg"),
            layout: &taa_uniform_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: config_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: taa_cam_buffer.as_entire_binding(),
                },
            ],
        });

        let bloom_pipeline = Self::make_bloom_pipeline(device, &tex2d_bgl, &config_bgl);
        let ssao_pipeline =
            Self::make_ssao_pipeline(device, &depth_bgl, &config_bgl, &ssao_cam_bgl);
        let composite_pipeline =
            Self::make_composite_pipeline(device, &tex2d_bgl, &config_bgl, surface_format);
        let taa_pipeline = Self::make_taa_pipeline(
            device,
            &tex2d_bgl,
            &depth_bgl,
            &taa_uniform_bgl,
            surface_format,
        );

        let targets = Self::create_targets(
            device,
            width,
            height,
            depth_view,
            &sampler,
            &tex2d_bgl,
            &depth_bgl,
            surface_format,
        );

        Self {
            hdr_view: targets.hdr_view,
            _hdr_texture: targets.hdr_texture,
            bloom_view: targets.bloom_view,
            _bloom_texture: targets.bloom_texture,
            ao_view: targets.ao_view,
            _ao_texture: targets.ao_texture,
            ldr_view: targets.ldr_view,
            _ldr_texture: targets.ldr_texture,
            history_views: targets.history_views,
            _history_textures: targets.history_textures,
            history_bgs: targets.history_bgs,
            history_read: 0,
            history_valid: false,
            hdr_bg: targets.hdr_bg,
            bloom_bg: targets.bloom_bg,
            ao_bg: targets.ao_bg,
            ldr_bg: targets.ldr_bg,
            depth_bg: targets.depth_bg,
            sampler,
            config_buffer,
            config_bg,
            ssao_cam_buffer,
            ssao_cam_bg,
            taa_cam_buffer,
            taa_uniform_bg,
            bloom_pipeline,
            ssao_pipeline,
            composite_pipeline,
            taa_pipeline,
            tex2d_bgl,
            depth_bgl,
            surface_format,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn create_targets(
        device: &wgpu::Device,
        width: u32,
        height: u32,
        depth_view: &wgpu::TextureView,
        sampler: &wgpu::Sampler,
        tex2d_bgl: &wgpu::BindGroupLayout,
        depth_bgl: &wgpu::BindGroupLayout,
        surface_format: wgpu::TextureFormat,
    ) -> PostProcessTargets {
        let make_tex = |label: &str, fmt: wgpu::TextureFormat| {
            crate::profiler::create_tracked_texture(
                device,
                &wgpu::TextureDescriptor {
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
                },
            )
        };

        let hdr_texture = make_tex("pp hdr", HDR_FORMAT);
        let hdr_view = hdr_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bloom_texture = make_tex("pp bloom", BLOOM_FORMAT);
        let bloom_view = bloom_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let ao_texture = make_tex("pp ao", AO_FORMAT);
        let ao_view = ao_texture.create_view(&wgpu::TextureViewDescriptor::default());
        // `surface_format`, not HDR_FORMAT: this holds the already-tonemapped
        // LDR image on its way to the swapchain, so matching the swapchain's
        // format keeps the round trip through it exact.
        let ldr_texture = make_tex("pp ldr", surface_format);
        let ldr_view = ldr_texture.create_view(&wgpu::TextureViewDescriptor::default());
        // The TAA history ping-pong pair. Both hold a finished LDR frame, so
        // like the LDR target they use `surface_format` at the full surface
        // size; a pair rather than one texture because the resolve pass has
        // to sample last frame's history while rendering this frame's.
        let history_textures = [
            make_tex("pp history 0", surface_format),
            make_tex("pp history 1", surface_format),
        ];
        let history_views = [
            history_textures[0].create_view(&wgpu::TextureViewDescriptor::default()),
            history_textures[1].create_view(&wgpu::TextureViewDescriptor::default()),
        ];

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
        let ldr_bg = make_tex2d_bg("pp ldr bg", &ldr_view);
        let history_bgs = [
            make_tex2d_bg("pp history bg 0", &history_views[0]),
            make_tex2d_bg("pp history bg 1", &history_views[1]),
        ];

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
            ldr_texture,
            ldr_view,
            ldr_bg,
            history_textures,
            history_views,
            history_bgs,
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

    /// Two color targets, both `surface_format`: `@location(0)` is the
    /// swapchain and `@location(1)` is next frame's history. Writing both in
    /// one pass is why the history pair can hold exactly the image that was
    /// presented, with no extra copy.
    fn make_taa_pipeline(
        device: &wgpu::Device,
        tex2d_bgl: &wgpu::BindGroupLayout,
        depth_bgl: &wgpu::BindGroupLayout,
        taa_uniform_bgl: &wgpu::BindGroupLayout,
        surface_format: wgpu::TextureFormat,
    ) -> wgpu::RenderPipeline {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("taa shader"),
            source: wgpu::ShaderSource::Wgsl(TAA_WGSL.into()),
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("taa pll"),
            bind_group_layouts: &[tex2d_bgl, tex2d_bgl, depth_bgl, taa_uniform_bgl],
            push_constant_ranges: &[],
        });
        let color_target = Some(wgpu::ColorTargetState {
            format: surface_format,
            blend: Some(wgpu::BlendState::REPLACE),
            write_mask: wgpu::ColorWrites::ALL,
        });
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("taa pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_fullscreen",
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_taa",
                targets: &[color_target.clone(), color_target],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        })
    }

    /// Recreates the sized render targets (HDR/bloom/AO/LDR/depth bind group)
    /// for a new surface resolution, leaving pipelines and samplers untouched.
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
            self.surface_format,
        );
        self.hdr_view = t.hdr_view;
        self._hdr_texture = t.hdr_texture;
        self.bloom_view = t.bloom_view;
        self._bloom_texture = t.bloom_texture;
        self.ao_view = t.ao_view;
        self._ao_texture = t.ao_texture;
        self.ldr_view = t.ldr_view;
        self._ldr_texture = t.ldr_texture;
        self.history_views = t.history_views;
        self._history_textures = t.history_textures;
        self.history_bgs = t.history_bgs;
        // The freshly created history pair is uninitialised at the new
        // resolution, so nothing in it may be blended against.
        self.invalidate_history();
        self.hdr_bg = t.hdr_bg;
        self.bloom_bg = t.bloom_bg;
        self.ao_bg = t.ao_bg;
        self.ldr_bg = t.ldr_bg;
        self.depth_bg = t.depth_bg;
    }

    /// Discards the accumulated TAA history, so the next resolve blends the
    /// frame with itself instead of with a stale or uninitialised image.
    ///
    /// Anything that makes the history textures stop describing the frame the
    /// next resolve will produce has to call this -- a resize above being the
    /// standing example, since the reallocated pair holds garbage at the new
    /// resolution.
    pub fn invalidate_history(&mut self) {
        self.history_valid = false;
    }

    /// Uploads new bloom/tonemap/SSAO settings to the config uniform buffer.
    pub fn update_config(&self, queue: &wgpu::Queue, config: PostProcessConfigGpu) {
        queue.write_buffer(&self.config_buffer, 0, bytemuck::cast_slice(&[config]));
    }

    /// Uploads the current frame's camera projection matrices for SSAO depth reconstruction.
    pub fn update_ssao_camera(&self, queue: &wgpu::Queue, cam: SsaoCameraGpu) {
        queue.write_buffer(&self.ssao_cam_buffer, 0, bytemuck::cast_slice(&[cam]));
    }

    /// Uploads the unjittered view-projection matrices the TAA pass reprojects
    /// with: this frame's inverse and the previous frame's forward matrix.
    pub fn update_taa_camera(&self, queue: &wgpu::Queue, cam: TaaCameraGpu) {
        queue.write_buffer(&self.taa_cam_buffer, 0, bytemuck::cast_slice(&[cam]));
    }

    /// Runs the bloom, SSAO, composite, and TAA passes in sequence, writing
    /// the final tonemapped result into `surface_view`.
    ///
    /// Composite writes into an intermediate LDR target rather than straight
    /// into `surface_view`; the TAA pass then reads that target and writes both
    /// the swapchain and next frame's history target. With `taa_enabled` zero
    /// the resolve returns the composite result untouched, so the image
    /// reaching `surface_view` is the same one composite used to write there
    /// directly.
    ///
    /// Takes `&mut self` because the resolve advances the history ping-pong:
    /// the target just written becomes next frame's read source.
    ///
    /// `fast_render` skips the bloom/SSAO shading work but still clears both
    /// targets to their neutral values (black = "no bloom contribution",
    /// white = "fully visible") -- see `WgpuSurface::is_fast_render`. Never
    /// skip the clear itself: the composite pass below always samples both
    /// textures, and an un-cleared AO texture reads as 0.0 ("fully
    /// occluded") instead of 1.0, which would render every pixel black.
    ///
    /// Returns the `(draw_calls, triangles)` issued by this call, so the
    /// caller can fold them into its own per-frame counters.
    pub fn apply(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        surface_view: &wgpu::TextureView,
        fast_render: bool,
    ) -> (u32, u64) {
        let mut draw_calls = 0u32;
        let mut triangles = 0u64;
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
            if !fast_render {
                pass.set_pipeline(&self.bloom_pipeline);
                pass.set_bind_group(0, &self.hdr_bg, &[]);
                pass.set_bind_group(1, &self.config_bg, &[]);
                pass.draw(0..3, 0..1);
                draw_calls += 1;
                triangles += 1;
            }
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
            if !fast_render {
                pass.set_pipeline(&self.ssao_pipeline);
                pass.set_bind_group(0, &self.depth_bg, &[]);
                pass.set_bind_group(1, &self.config_bg, &[]);
                pass.set_bind_group(2, &self.ssao_cam_bg, &[]);
                pass.draw(0..3, 0..1);
                draw_calls += 1;
                triangles += 1;
            }
        }

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("composite pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.ldr_view,
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
            draw_calls += 1;
            triangles += 1;
        }

        {
            // Without a valid history the source is this frame's own LDR, so
            // the blend degenerates to `mix(current, current, blend)` -- a
            // no-op -- instead of reading an uninitialised texture.
            let history_src = if self.history_valid {
                &self.history_bgs[self.history_read]
            } else {
                &self.ldr_bg
            };
            // Write into the half NOT being sampled; the flip below makes it
            // next frame's read source.
            let history_dst = &self.history_views[1 - self.history_read];
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("taa pass"),
                color_attachments: &[
                    Some(wgpu::RenderPassColorAttachment {
                        view: surface_view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                            store: wgpu::StoreOp::Store,
                        },
                    }),
                    Some(wgpu::RenderPassColorAttachment {
                        view: history_dst,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                            store: wgpu::StoreOp::Store,
                        },
                    }),
                ],
                depth_stencil_attachment: None,
                ..Default::default()
            });
            pass.set_pipeline(&self.taa_pipeline);
            pass.set_bind_group(0, &self.ldr_bg, &[]);
            pass.set_bind_group(1, history_src, &[]);
            pass.set_bind_group(2, &self.depth_bg, &[]);
            pass.set_bind_group(3, &self.taa_uniform_bg, &[]);
            pass.draw(0..3, 0..1);
            draw_calls += 1;
            triangles += 1;
        }

        // The target just written holds the frame the next resolve reprojects.
        self.history_read = 1 - self.history_read;
        self.history_valid = true;

        (draw_calls, triangles)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::surface::WgpuSurface;

    #[test]
    fn config_gpu_size() {
        assert_eq!(std::mem::size_of::<PostProcessConfigGpu>(), 64);
    }

    #[test]
    fn ssao_cam_gpu_size() {
        assert_eq!(std::mem::size_of::<SsaoCameraGpu>(), 128);
    }

    #[test]
    fn taa_cam_gpu_size() {
        assert_eq!(std::mem::size_of::<TaaCameraGpu>(), 128);
    }

    #[test]
    fn hdr_format_is_rgba16float() {
        assert_eq!(HDR_FORMAT, wgpu::TextureFormat::Rgba16Float);
    }

    #[test]
    fn bloom_shader_compiles() {
        let (device, _queue) = pollster::block_on(WgpuSurface::headless_device_for_testing());
        let _m = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(BLOOM_WGSL.into()),
        });
    }

    #[test]
    fn ssao_shader_compiles() {
        let (device, _queue) = pollster::block_on(WgpuSurface::headless_device_for_testing());
        let _m = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(SSAO_WGSL.into()),
        });
    }

    #[test]
    fn composite_shader_compiles() {
        let (device, _queue) = pollster::block_on(WgpuSurface::headless_device_for_testing());
        let _m = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(COMPOSITE_WGSL.into()),
        });
    }

    #[test]
    fn taa_shader_compiles() {
        let (device, _queue) = pollster::block_on(WgpuSurface::headless_device_for_testing());
        let _m = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(TAA_WGSL.into()),
        });
    }

    /// The resolve binds five groups and writes two colour attachments, and
    /// every one of them has to line up with the WGSL: a wrong group index, a
    /// binding the layout does not declare, or a second target the fragment
    /// entry does not return is a `wgpu` validation error. Compiling the
    /// shader alone proves none of that -- outside this test the mismatch
    /// would surface only as a panic deep inside a pixel test.
    ///
    /// Two `apply` calls, because the two history states take different
    /// branches: the first has no history and binds the current LDR, the
    /// second binds what the first wrote.
    #[test]
    fn taa_pass_records_without_validation_errors() {
        let (device, queue) = pollster::block_on(WgpuSurface::headless_device_for_testing());
        let width = 64;
        let height = 64;
        let depth_texture = crate::profiler::create_tracked_texture(
            &device,
            &wgpu::TextureDescriptor {
                label: Some("taa test depth"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: crate::surface::DEPTH_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
        );
        let depth_view = depth_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let target = crate::output::create_offscreen_texture(&device, width, height);
        let target_view = target.create_view(&wgpu::TextureViewDescriptor::default());

        device.push_error_scope(wgpu::ErrorFilter::Validation);
        let mut pp = PostProcessState::new(
            &device,
            width,
            height,
            &depth_view,
            crate::output::OFFSCREEN_FORMAT,
        );

        let mut encoder =
            device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        pp.apply(&mut encoder, &target_view, false);
        queue.submit(Some(encoder.finish()));
        assert!(
            pp.history_valid,
            "the first resolve writes a history target, so the second one has \
             something to reproject"
        );
        assert_eq!(
            pp.history_read, 1,
            "the ping-pong must advance: the half just written is next \
             frame's read source, and never the half read this frame"
        );

        let mut encoder =
            device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        pp.apply(&mut encoder, &target_view, false);
        queue.submit(Some(encoder.finish()));
        assert_eq!(pp.history_read, 0, "the second resolve flips it back");

        device.poll(wgpu::Maintain::Wait);
        assert!(
            pollster::block_on(device.pop_error_scope()).is_none(),
            "the TAA resolve pass must record cleanly; a validation error here \
             means the pipeline layout, the bind groups, or the two colour \
             attachments disagree with the shader"
        );
    }
}
