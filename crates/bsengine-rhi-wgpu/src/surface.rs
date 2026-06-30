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
    _pad3: f32,
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
            lo += (kd * albedo / PI + specular) * pl.color * (pl.intensity * t * t) * n_dot_l;
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
    return vec4<f32>(color, 1.0);
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

const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
const MAX_OBJECTS: usize = 1024;
const MODEL_STRIDE: u64 = 256;
// view_proj(64) + light_view_proj(64) + cam_pos(12) + pad(4) = 144
const CAMERA_UNIFORM_SIZE: u64 = 144;
// direction(16) + color(16) + ambient+count(16) + 8×PointLightGpu(48=384) +
// num_spot+pad(16) + 8×SpotLightGpu(64=512) = 960
const LIGHT_UNIFORM_SIZE: u64 = 960;
// Vertex stride: position(12) + color(12) + normal(12) + uv(8) = 44 bytes
const VERTEX_STRIDE: u64 = 44;
const SHADOW_MAP_SIZE: u32 = 2048;

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct CameraUniformData {
    view_proj: [[f32; 4]; 4],
    light_view_proj: [[f32; 4]; 4],
    cam_pos: [f32; 3],
    _pad: f32,
}

/// Material parameters uploaded per draw call.
pub struct MaterialParams {
    pub metallic: f32,
    pub roughness: f32,
    pub emissive: Vec3,
    pub base_color: Vec3,
}

impl Default for MaterialParams {
    fn default() -> Self {
        Self {
            metallic: 0.0,
            roughness: 0.5,
            emissive: Vec3::ZERO,
            base_color: Vec3::ONE,
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
    _pad3: f32,
}

/// A single point light entry for the GPU buffer.
pub struct PointLightEntry {
    pub position: Vec3,
    pub color: Vec3,
    pub intensity: f32,
    pub range: f32,
}

/// A single spot light entry for the GPU buffer.
pub struct SpotLightEntry {
    pub position: Vec3,
    pub direction: Vec3,
    pub color: Vec3,
    pub intensity: f32,
    pub range: f32,
    pub inner_angle: f32,
    pub outer_angle: f32,
}

/// Light parameters passed per frame.
pub struct LightData {
    pub direction: Vec3,
    pub color: Vec3,
    pub ambient: Vec3,
    pub point_lights: Vec<PointLightEntry>,
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

pub struct WgpuSurface {
    _window: Arc<winit::window::Window>,
    pub(crate) surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    pub(crate) device: Arc<wgpu::Device>,
    pub(crate) queue: Arc<wgpu::Queue>,
    pipeline: wgpu::RenderPipeline,
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
    egui_ctx: egui::Context,
    egui_renderer: egui_wgpu::Renderer,
}

impl WgpuSurface {
    pub async fn new(window: Arc<winit::window::Window>) -> Result<Self, String> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let surface = instance
            .create_surface(window.clone())
            .map_err(|e| e.to_string())?;

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::None,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .ok_or("No adapter found compatible with surface")?;

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

        let device = Arc::new(device);
        let queue = Arc::new(queue);

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

        let (depth_texture, depth_view) =
            Self::create_depth_texture(&device, config.width, config.height);

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
                    min_binding_size: wgpu::BufferSize::new(96),
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
                    size: wgpu::BufferSize::new(96),
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
            compare: Some(wgpu::CompareFunction::GreaterEqual),
            ..Default::default()
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
                buffers: &[vertex_buffer_layout.clone()],
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
                buffers: &[vertex_buffer_layout],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format,
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

        let egui_ctx = egui::Context::default();
        let egui_renderer =
            egui_wgpu::Renderer::new(&device, format, None, 1, false);

        Ok(Self {
            _window: window,
            surface,
            config,
            device,
            queue,
            pipeline,
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
            egui_ctx,
            egui_renderer,
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
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
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

    pub fn render_frame(
        &mut self,
        view_proj: Mat4,
        cam_pos: Vec3,
        light_view_proj: Mat4,
        draw_calls: &[(u64, Mat4, Option<u64>, MaterialParams)],
        registry: &GpuMeshRegistry,
        light: LightData,
        tex_registry: Option<&crate::texture::GpuTextureRegistry>,
        hud_texts: &std::collections::HashMap<String, String>,
    ) -> Result<(), String> {
        let camera_data = CameraUniformData {
            view_proj: view_proj.to_cols_array_2d(),
            light_view_proj: light_view_proj.to_cols_array_2d(),
            cam_pos: cam_pos.to_array(),
            _pad: 0.0,
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

        for (i, (_, model, _, mat)) in draw_calls.iter().enumerate() {
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
                _pad3: 0.0,
            };
            self.queue.write_buffer(
                &self.model_buffer,
                i as u64 * MODEL_STRIDE,
                bytemuck::cast_slice(&[data]),
            );
        }

        let output = self
            .surface
            .get_current_texture()
            .map_err(|e| e.to_string())?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("render encoder"),
            });

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
            for (i, (mesh_id, _, _, _)) in draw_calls.iter().enumerate() {
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

        // --- main pass ---
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("render pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
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

            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.camera_bind_group, &[]);
            pass.set_bind_group(2, &self.light_bind_group, &[]);

            for (i, (mesh_id, _, tex_id, _)) in draw_calls.iter().enumerate() {
                if i >= MAX_OBJECTS {
                    break;
                }
                let Some(mesh) = registry.get(*mesh_id) else {
                    continue;
                };
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

        // HUD overlay via egui
        if !hud_texts.is_empty() {
            let screen_rect = egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(self.config.width as f32, self.config.height as f32),
            );
            let raw_input = egui::RawInput {
                screen_rect: Some(screen_rect),
                ..Default::default()
            };
            let full_output = self.egui_ctx.run(raw_input, |ctx| {
                let painter = ctx.layer_painter(egui::LayerId::new(
                    egui::Order::Foreground,
                    egui::Id::new("hud"),
                ));
                let mut sorted_keys: Vec<&String> = hud_texts.keys().collect();
                sorted_keys.sort();
                for (i, key) in sorted_keys.iter().enumerate() {
                    let text = &hud_texts[*key];
                    painter.text(
                        egui::pos2(8.0, 8.0 + i as f32 * 24.0),
                        egui::Align2::LEFT_TOP,
                        text,
                        egui::FontId::proportional(20.0),
                        egui::Color32::WHITE,
                    );
                }
            });

            let clipped_primitives = self
                .egui_ctx
                .tessellate(full_output.shapes, full_output.pixels_per_point);
            let screen_descriptor = egui_wgpu::ScreenDescriptor {
                size_in_pixels: [self.config.width, self.config.height],
                pixels_per_point: full_output.pixels_per_point,
            };

            for (id, image_delta) in &full_output.textures_delta.set {
                self.egui_renderer.update_texture(
                    &self.device,
                    &self.queue,
                    *id,
                    image_delta,
                );
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
                        label: Some("egui hud pass"),
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
                self.egui_renderer.render(
                    &mut egui_pass,
                    &clipped_primitives,
                    &screen_descriptor,
                );
            }
            for id in &full_output.textures_delta.free {
                self.egui_renderer.free_texture(id);
            }
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();
        Ok(())
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
        let (depth_texture, depth_view) = Self::create_depth_texture(&self.device, width, height);
        self.depth_texture = depth_texture;
        self.depth_view = depth_view;
    }

    pub fn compile_shader(device: &wgpu::Device, src: &str) -> wgpu::ShaderModule {
        device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("wgsl shader"),
            source: wgpu::ShaderSource::Wgsl(src.into()),
        })
    }
}

#[derive(Resource)]
pub struct WgpuSurfaceResource(pub WgpuSurface);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rhi::WgpuRHI;

    #[test]
    fn mesh_shader_compiles() {
        let rhi = pollster::block_on(WgpuRHI::new_headless()).expect("headless rhi");
        let _module = WgpuSurface::compile_shader(&rhi.device, MESH_WGSL);
    }
}
