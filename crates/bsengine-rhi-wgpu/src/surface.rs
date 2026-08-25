use crate::mesh::GpuMeshRegistry;
use bsengine_ecs::Resource;
use glam::{Mat4, Vec3};
use std::sync::Arc;

const MAX_POINT_LIGHTS: usize = 8;
const MAX_SPOT_LIGHTS: usize = 8;

const MESH_WGSL: &str = r#"
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
    _pad2: f32,
    _pad3: f32,
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

pub(crate) const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
const MAX_OBJECTS: usize = 1024;
const MODEL_STRIDE: u64 = 256;
// view_proj(64) + light_view_proj(64) + cam_pos(12) + pad(4) = 144
const CAMERA_UNIFORM_SIZE: u64 = 144;
// inv_vp mat4x4<f32> = 64 bytes
const SKY_UNIFORM_SIZE: u64 = 64;
// direction(16) + color(16) + ambient+count(16) + 8×PointLightGpu(48=384) +
// num_spot+pad(16) + 8×SpotLightGpu(64=512) = 960
const LIGHT_UNIFORM_SIZE: u64 = 960;
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
    _pad2: f32,
    _pad3: f32,
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
    _texture: wgpu::Texture,
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

/// Computes the 6 face view-projection matrices for a point light's cube shadow
/// map, one per axis-aligned direction, using the standard cubemap face
/// orientation convention (+Y/-Y up-vectors on the X/Z faces to avoid a
/// degenerate look-at when looking straight up/down).
fn point_light_face_view_projs(position: Vec3, range: f32) -> [Mat4; 6] {
    let far = range.max(0.5);
    let proj = Mat4::perspective_rh(std::f32::consts::FRAC_PI_2, 1.0, 0.05, far);
    let dirs: [(Vec3, Vec3); 6] = [
        (Vec3::X, Vec3::NEG_Y),
        (Vec3::NEG_X, Vec3::NEG_Y),
        (Vec3::Y, Vec3::Z),
        (Vec3::NEG_Y, Vec3::NEG_Z),
        (Vec3::Z, Vec3::NEG_Y),
        (Vec3::NEG_Z, Vec3::NEG_Y),
    ];
    dirs.map(|(dir, up)| proj * Mat4::look_at_rh(position, position + dir, up))
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
    depth_texture: wgpu::Texture,
    depth_view: wgpu::TextureView,
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    model_buffer: wgpu::Buffer,
    model_bind_group: wgpu::BindGroup,
    light_buffer: wgpu::Buffer,
    light_bind_group: wgpu::BindGroup,
    _white_texture: wgpu::Texture,
    _sampler: wgpu::Sampler,
    default_texture_bind_group: wgpu::BindGroup,
    shadow_pipeline: wgpu::RenderPipeline,
    _shadow_map_texture: wgpu::Texture,
    shadow_map_view: wgpu::TextureView,
    _shadow_comparison_sampler: wgpu::Sampler,
    point_shadow_pipeline: wgpu::RenderPipeline,
    _point_shadow_color_texture: wgpu::Texture,
    _point_shadow_depth_texture: wgpu::Texture,
    point_shadow_depth_view: wgpu::TextureView,
    _point_shadow_sampler: wgpu::Sampler,
    point_shadow_uniform_buffer: wgpu::Buffer,
    point_shadow_bind_group: wgpu::BindGroup,
    egui_ctx: egui::Context,
    egui_renderer: egui_wgpu::Renderer,
    skybox: Option<SkyboxState>,
    loaded_skybox_path: Option<String>,
    pipeline_layout: wgpu::PipelineLayout,
    custom_pipelines: std::collections::HashMap<String, wgpu::RenderPipeline>,
    post_process: crate::post_process::PostProcessState,
    start_time: std::time::Instant,
    dock_state: Option<egui_dock::DockState<String>>,
    last_saved_layout_json: Option<String>,
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

        let (adapter, device, queue) = Self::request_device(&instance, Some(&surface)).await?;

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
        )
    }

    /// Creates a renderer with no window at all.
    ///
    /// The frame goes to a texture this renderer owns and can be read back with
    /// [`Self::read_pixels`]. Pipelines come from the same [`Self::build`] the
    /// windowed path uses, so pixels observed here are the output of the
    /// pipelines that draw to a window.
    pub async fn new_offscreen(width: u32, height: u32) -> Result<Self, String> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });
        let (_adapter, device, queue) = Self::request_device(&instance, None).await?;
        let texture = crate::output::create_offscreen_texture(&device, width, height);
        Self::build(
            device,
            queue,
            crate::output::Output::Offscreen {
                texture,
                width,
                height,
            },
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
    async fn request_device(
        instance: &wgpu::Instance,
        compatible_surface: Option<&wgpu::Surface<'static>>,
    ) -> Result<(wgpu::Adapter, Arc<wgpu::Device>, Arc<wgpu::Queue>), String> {
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::None,
                compatible_surface,
                force_fallback_adapter: false,
            })
            .await
            .ok_or("No adapter found")?;

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("BSEngine surface device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::downlevel_defaults(),
                    memory_hints: wgpu::MemoryHints::default(),
                },
                None,
            )
            .await
            .map_err(|e| format!("Device request failed: {e}"))?;

        Ok((adapter, Arc::new(device), Arc::new(queue)))
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
    ) -> Result<Self, String> {
        let format = output.format();
        let width = output.width();
        let height = output.height();

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
            ],
        });

        let (white_texture, sampler, texture_bgl, default_texture_bind_group) =
            Self::create_default_texture(&device, &queue);

        // --- shadow map ---
        let shadow_map_texture = device.create_texture(&wgpu::TextureDescriptor {
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
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
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
        let point_shadow_color_texture = device.create_texture(&wgpu::TextureDescriptor {
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
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let point_shadow_color_full_view =
            point_shadow_color_texture.create_view(&wgpu::TextureViewDescriptor {
                label: Some("point shadow color array view (full)"),
                dimension: Some(wgpu::TextureViewDimension::D2Array),
                ..Default::default()
            });

        let point_shadow_depth_texture = device.create_texture(&wgpu::TextureDescriptor {
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
        });
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

        let light_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("light bg"),
            layout: &light_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: light_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&shadow_comparison_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&shadow_map_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&point_shadow_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(&point_shadow_color_full_view),
                },
            ],
        });

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
                buffers: &[vertex_buffer_layout.clone()],
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
                buffers: &[vertex_buffer_layout],
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

        let particles = crate::particles::ParticleRenderer::new(&device, &camera_bgl);

        let egui_ctx = egui::Context::default();
        crate::theme::apply(&egui_ctx);
        let egui_renderer = egui_wgpu::Renderer::new(&device, format, None, 1, false);

        let post_process =
            crate::post_process::PostProcessState::new(&device, width, height, &depth_view, format);

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
            _white_texture: white_texture,
            _sampler: sampler,
            default_texture_bind_group,
            shadow_pipeline,
            _shadow_map_texture: shadow_map_texture,
            shadow_map_view,
            _shadow_comparison_sampler: shadow_comparison_sampler,
            point_shadow_pipeline,
            _point_shadow_color_texture: point_shadow_color_texture,
            _point_shadow_depth_texture: point_shadow_depth_texture,
            point_shadow_depth_view,
            _point_shadow_sampler: point_shadow_sampler,
            point_shadow_uniform_buffer,
            point_shadow_bind_group,
            egui_ctx,
            egui_renderer,
            skybox: None,
            loaded_skybox_path: None,
            pipeline_layout,
            custom_pipelines: std::collections::HashMap::new(),
            post_process,
            start_time: std::time::Instant::now(),
            dock_state: None,
            last_saved_layout_json: None,
        })
    }

    fn create_depth_texture(
        device: &wgpu::Device,
        width: u32,
        height: u32,
    ) -> (wgpu::Texture, wgpu::TextureView) {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
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
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        (texture, view)
    }

    fn create_default_texture(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> (
        wgpu::Texture,
        wgpu::Sampler,
        wgpu::BindGroupLayout,
        wgpu::BindGroup,
    ) {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
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
        });
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
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
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
        });
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

        let sky_tex_bgl = self
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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
        let texture_bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("sky tex bg"),
            layout: &sky_tex_bgl,
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
                bind_group_layouts: &[&sky_uniform_bgl, &sky_tex_bgl],
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
    }

    /// Whether a skybox is currently loaded and will be rendered.
    pub fn has_skybox(&self) -> bool {
        self.skybox.is_some()
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
    ) -> Result<std::collections::HashSet<String>, String> {
        let camera_data = CameraUniformData {
            view_proj: view_proj.to_cols_array_2d(),
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
            _pad2: 0.0,
            _pad3: 0.0,
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
                _pad0: 0.0,
                _pad1: 0.0,
                _pad2: 0.0,
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
        }

        // --- shadow pass ---
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
                timestamp_writes: None,
                occlusion_query_set: None,
            });
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
            for (light_idx, _pl) in active_lights.iter().enumerate() {
                for face in 0..6usize {
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
                            timestamp_writes: None,
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
                    }
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
                timestamp_writes: None,
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
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            sky_pass.set_pipeline(&sky.pipeline);
            sky_pass.set_bind_group(0, &sky.uniform_bg, &[]);
            sky_pass.set_bind_group(1, &sky.texture_bg, &[]);
            sky_pass.draw(0..3, 0..1);
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
                timestamp_writes: None,
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
            }
        }

        // --- particle pass (after transparency, so sparks read over glass) ---
        if !particles.is_empty() {
            self.particles.draw(
                &mut encoder,
                &self.queue,
                &self.post_process.hdr_view,
                &self.depth_view,
                &self.camera_bind_group,
                particles,
                tex_registry,
                &self.default_texture_bind_group,
            );
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
        self.post_process.apply(&mut encoder, &view);

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
                            crate::panels::ensure_builtin_panels(registry);
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

        self.queue.submit(std::iter::once(encoder.finish()));
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
    use crate::rhi::WgpuRHI;

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
        let rhi = pollster::block_on(WgpuRHI::new_headless()).expect("headless rhi");
        let _module = WgpuSurface::compile_shader(&rhi.device, MESH_WGSL);
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
        let rhi = pollster::block_on(WgpuRHI::new_headless()).expect("headless rhi");
        let _module = WgpuSurface::compile_shader(&rhi.device, SKYBOX_WGSL);
    }

    #[test]
    fn custom_shader_wgsl_compiles() {
        let rhi = pollster::block_on(WgpuRHI::new_headless()).expect("headless rhi");
        let wgsl = r#"
@vertex fn vs_main(@builtin(vertex_index) vi: u32) -> @builtin(position) vec4<f32> {
    return vec4<f32>(0.0, 0.0, 0.0, 1.0);
}
@fragment fn fs_main() -> @location(0) vec4<f32> {
    return vec4<f32>(1.0, 0.0, 0.0, 1.0);
}
"#;
        let _module = WgpuSurface::compile_shader(&rhi.device, wgsl);
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
        let rhi = pollster::block_on(WgpuRHI::new_headless()).expect("headless rhi");

        rhi.device.push_error_scope(wgpu::ErrorFilter::Validation);
        let _module = WgpuSurface::compile_shader(
            &rhi.device,
            "@fragment fn fs_main() -> @location(0) vec4<f32> { return vec4<f32>(0.0 1.0); }",
        );
        let err = pollster::block_on(rhi.device.pop_error_scope());
        assert!(
            err.is_some(),
            "a broken shader must surface as a captured error; None means the \
             rejection went to the uncaptured handler, which panics"
        );

        rhi.device.push_error_scope(wgpu::ErrorFilter::Validation);
        let _module = WgpuSurface::compile_shader(&rhi.device, VALID_CUSTOM_WGSL);
        assert!(
            pollster::block_on(rhi.device.pop_error_scope()).is_none(),
            "a valid shader must leave the scope empty; otherwise every reload \
             would be treated as a failure and no pipeline would ever update"
        );
    }

    #[test]
    fn shadow_shader_compiles() {
        let rhi = pollster::block_on(WgpuRHI::new_headless()).expect("headless rhi");
        let _module = WgpuSurface::compile_shader(&rhi.device, SHADOW_WGSL);
    }

    #[test]
    fn point_shadow_shader_compiles() {
        let rhi = pollster::block_on(WgpuRHI::new_headless()).expect("headless rhi");
        let _module = WgpuSurface::compile_shader(&rhi.device, POINT_SHADOW_WGSL);
    }
}
