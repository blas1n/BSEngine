use crate::mesh::GpuMeshRegistry;
use bsengine_ecs::Resource;
use glam::{Mat4, Vec3};
use std::sync::Arc;

const MESH_WGSL: &str = r#"
struct CameraUniform {
    view_proj: mat4x4<f32>,
};
struct ModelUniform {
    model: mat4x4<f32>,
};
struct LightUniform {
    direction: vec3<f32>,
    _pad0: f32,
    color: vec3<f32>,
    _pad1: f32,
    ambient: vec3<f32>,
    _pad2: f32,
};
@group(0) @binding(0) var<uniform> camera: CameraUniform;
@group(1) @binding(0) var<uniform> model_data: ModelUniform;
@group(2) @binding(0) var<uniform> light: LightUniform;
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
}
@vertex
fn vs_main(in: VertIn) -> VertOut {
    var out: VertOut;
    out.clip_pos = camera.view_proj * model_data.model * vec4<f32>(in.pos, 1.0);
    out.col = in.col;
    let normal_matrix = mat3x3<f32>(
        model_data.model[0].xyz,
        model_data.model[1].xyz,
        model_data.model[2].xyz,
    );
    out.world_normal = normalize(normal_matrix * in.normal);
    out.uv = in.uv;
    return out;
}
@fragment
fn fs_main(in: VertOut) -> @location(0) vec4<f32> {
    let n = normalize(in.world_normal);
    let diff = max(dot(n, -light.direction), 0.0);
    let tex_color = textureSample(t_diffuse, s_diffuse, in.uv).rgb;
    let lit = tex_color * in.col * (light.ambient + diff * light.color);
    return vec4<f32>(lit, 1.0);
}
"#;

const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
const MAX_OBJECTS: usize = 1024;
const MODEL_STRIDE: u64 = 256;
const CAMERA_UNIFORM_SIZE: u64 = 64;
const LIGHT_UNIFORM_SIZE: u64 = 48;
// Vertex stride: position(12) + color(12) + normal(12) + uv(8) = 44 bytes
const VERTEX_STRIDE: u64 = 44;

/// Light parameters passed per frame.
pub struct LightData {
    pub direction: Vec3,
    pub color: Vec3,
    pub ambient: Vec3,
}

impl Default for LightData {
    fn default() -> Self {
        Self {
            direction: Vec3::new(-0.4, -0.8, -0.4).normalize(),
            color: Vec3::ONE,
            ambient: Vec3::splat(0.15),
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct LightUniformData {
    direction: [f32; 3],
    _pad0: f32,
    color: [f32; 3],
    _pad1: f32,
    ambient: [f32; 3],
    _pad2: f32,
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
                visibility: wgpu::ShaderStages::VERTEX,
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
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: true,
                    min_binding_size: wgpu::BufferSize::new(64),
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
                    size: wgpu::BufferSize::new(64),
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
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: wgpu::BufferSize::new(LIGHT_UNIFORM_SIZE),
                },
                count: None,
            }],
        });
        let light_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("light bg"),
            layout: &light_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: light_buffer.as_entire_binding(),
            }],
        });

        let (white_texture, sampler, texture_bgl, default_texture_bind_group) =
            Self::create_default_texture(&device, &queue);

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
                buffers: &[wgpu::VertexBufferLayout {
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
                }],
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
        &self,
        view_proj: Mat4,
        draw_calls: &[(u64, Mat4)],
        registry: &GpuMeshRegistry,
        light: LightData,
    ) -> Result<(), String> {
        let camera_data: [[f32; 4]; 4] = view_proj.to_cols_array_2d();
        self.queue
            .write_buffer(&self.camera_buffer, 0, bytemuck::cast_slice(&camera_data));

        let light_data = LightUniformData {
            direction: light.direction.normalize().to_array(),
            _pad0: 0.0,
            color: light.color.to_array(),
            _pad1: 0.0,
            ambient: light.ambient.to_array(),
            _pad2: 0.0,
        };
        self.queue
            .write_buffer(&self.light_buffer, 0, bytemuck::cast_slice(&[light_data]));

        for (i, (_, model)) in draw_calls.iter().enumerate() {
            if i >= MAX_OBJECTS {
                break;
            }
            let data: [[f32; 4]; 4] = model.to_cols_array_2d();
            self.queue.write_buffer(
                &self.model_buffer,
                i as u64 * MODEL_STRIDE,
                bytemuck::cast_slice(&data),
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
            pass.set_bind_group(3, &self.default_texture_bind_group, &[]);

            for (i, (mesh_id, _)) in draw_calls.iter().enumerate() {
                if i >= MAX_OBJECTS {
                    break;
                }
                let Some(mesh) = registry.get(*mesh_id) else {
                    continue;
                };
                let offset = (i as u64 * MODEL_STRIDE) as u32;
                pass.set_bind_group(1, &self.model_bind_group, &[offset]);
                pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..mesh.index_count, 0, 0..1);
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
