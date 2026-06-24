use crate::mesh::GpuMeshRegistry;
use bsengine_ecs::Resource;
use glam::Mat4;
use std::sync::Arc;

const MESH_WGSL: &str = r#"
struct CameraUniform {
    view_proj: mat4x4<f32>,
};
struct ModelUniform {
    model: mat4x4<f32>,
};
@group(0) @binding(0) var<uniform> camera: CameraUniform;
@group(1) @binding(0) var<uniform> model_data: ModelUniform;

struct VertIn {
    @location(0) pos: vec3<f32>,
    @location(1) col: vec3<f32>,
}
struct VertOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) col: vec3<f32>,
}
@vertex
fn vs_main(in: VertIn) -> VertOut {
    var out: VertOut;
    out.clip_pos = camera.view_proj * model_data.model * vec4<f32>(in.pos, 1.0);
    out.col = in.col;
    return out;
}
@fragment
fn fs_main(in: VertOut) -> @location(0) vec4<f32> {
    return vec4<f32>(in.col, 1.0);
}
"#;

const MAX_OBJECTS: usize = 1024;
// Padded to 256 bytes (typical min_uniform_buffer_offset_alignment).
// Mat4 = 64 bytes; pad to 256 so dynamic offsets work on all devices.
const MODEL_STRIDE: u64 = 256;
const CAMERA_UNIFORM_SIZE: u64 = 64;

pub struct WgpuSurface {
    _window: Arc<winit::window::Window>,
    pub(crate) surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    pub(crate) device: Arc<wgpu::Device>,
    pub(crate) queue: Arc<wgpu::Queue>,
    pipeline: wgpu::RenderPipeline,
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    model_buffer: wgpu::Buffer,
    model_bind_group: wgpu::BindGroup,
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

        // Camera uniform buffer (group 0)
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

        // Model uniform buffer with dynamic offsets (group 1)
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

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("mesh shader"),
            source: wgpu::ShaderSource::Wgsl(MESH_WGSL.into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("mesh pipeline layout"),
            bind_group_layouts: &[&camera_bgl, &model_bgl],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("mesh pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: 24, // 6 x f32
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
            depth_stencil: None,
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
            camera_buffer,
            camera_bind_group,
            model_buffer,
            model_bind_group,
        })
    }

    /// Render a frame. `draw_calls` is a list of `(mesh_id, model_matrix)`.
    pub fn render_frame(
        &self,
        view_proj: Mat4,
        draw_calls: &[(u64, Mat4)],
        registry: &GpuMeshRegistry,
    ) -> Result<(), String> {
        // Upload camera matrix
        let camera_data: [[f32; 4]; 4] = view_proj.to_cols_array_2d();
        self.queue
            .write_buffer(&self.camera_buffer, 0, bytemuck::cast_slice(&camera_data));

        // Upload model matrices (padded to MODEL_STRIDE)
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
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.camera_bind_group, &[]);

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
