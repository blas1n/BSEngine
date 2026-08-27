//! Billboarded particle quads, drawn instanced.
//!
//! There is no vertex buffer for the geometry: the quad's six corners come from
//! `vertex_index`, the same trick the skybox pass uses for its fullscreen
//! triangle. What varies per particle rides an instance buffer rewritten each
//! frame.

use crate::post_process::HDR_FORMAT;
use crate::texture::GpuTextureRegistry;

/// One particle, as the GPU sees it.
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ParticleInstance {
    /// World-space centre.
    pub position: [f32; 3],
    /// Billboard half-size, in world units.
    pub size: f32,
    /// Colour with alpha.
    pub color: [f32; 4],
}

/// Every particle from one emitter, plus the texture they share.
///
/// Batched per emitter because the texture is bound once per draw. Particles
/// within a batch cost one instance each.
pub struct ParticleBatch {
    /// The emitter's texture, or `None` for a flat quad.
    pub texture_id: Option<u64>,
    /// The particles to draw.
    pub instances: Vec<ParticleInstance>,
}

/// The most instances one frame can draw, across every emitter.
///
/// The per-emitter cap in `ParticlePlugin` is 4096; this is the buffer behind
/// all of them together. Excess is dropped with a warning rather than growing
/// the buffer mid-frame.
const MAX_INSTANCES: usize = 16384;

/// Pipeline and instance buffer for the particle pass.
pub struct ParticleRenderer {
    pipeline: wgpu::RenderPipeline,
    instances: wgpu::Buffer,
    /// Structurally identical to the one `GpuTextureRegistry` builds its bind
    /// groups with, which is what lets those bind groups be used here.
    _texture_bgl: wgpu::BindGroupLayout,
}

impl ParticleRenderer {
    /// Builds the pipeline against an existing camera bind group layout, so the
    /// pass can reuse the camera uniform the rest of the frame already wrote.
    pub fn new(device: &wgpu::Device, camera_bgl: &wgpu::BindGroupLayout) -> Self {
        let texture_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("particle texture bgl"),
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

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("particle shader"),
            source: wgpu::ShaderSource::Wgsl(PARTICLE_WGSL.into()),
        });

        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("particle pipeline layout"),
            bind_group_layouts: &[camera_bgl, &texture_bgl],
            push_constant_ranges: &[],
        });

        let instance_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<ParticleInstance>() as u64,
            // The whole point: one step per particle, not per vertex.
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x3,
                    offset: 0,
                    shader_location: 0,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32,
                    offset: 12,
                    shader_location: 1,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x4,
                    offset: 16,
                    shader_location: 2,
                },
            ],
        };

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("particle pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_particle",
                buffers: &[instance_layout],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_particle",
                targets: &[Some(wgpu::ColorTargetState {
                    format: HDR_FORMAT,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                // A billboard is seen from both sides depending on where the
                // camera is; culling one would make half of them vanish.
                cull_mode: None,
                front_face: wgpu::FrontFace::Ccw,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: crate::surface::DEPTH_FORMAT,
                // Test but do not write, exactly as the transparent pass does:
                // particles are hidden by solid geometry in front of them, and
                // do not occlude each other.
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let instances = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("particle instances"),
            size: (MAX_INSTANCES * std::mem::size_of::<ParticleInstance>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            pipeline,
            instances,
            _texture_bgl: texture_bgl,
        }
    }

    /// Draws every batch into `target`, one instanced draw per emitter.
    ///
    /// Returns the `(draw_calls, triangles)` issued by this call, so the
    /// caller can fold them into its own per-frame counters.
    #[allow(clippy::too_many_arguments)] // one pass's worth of frame state
    pub fn draw(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        queue: &wgpu::Queue,
        target: &wgpu::TextureView,
        depth: &wgpu::TextureView,
        camera_bind_group: &wgpu::BindGroup,
        batches: &[ParticleBatch],
        tex_registry: Option<&GpuTextureRegistry>,
        default_texture: &wgpu::BindGroup,
    ) -> (u32, u64) {
        // Pack every batch into the one buffer, remembering each one's slice.
        let mut packed: Vec<ParticleInstance> = Vec::new();
        let mut ranges: Vec<(Option<u64>, u32, u32)> = Vec::new();
        for batch in batches {
            let start = packed.len();
            let room = MAX_INSTANCES.saturating_sub(start);
            if room == 0 {
                tracing::warn!(
                    "[particles] over the {MAX_INSTANCES}-instance frame budget; \
                     dropping the rest of this frame's emitters"
                );
                break;
            }
            let take = batch.instances.len().min(room);
            packed.extend_from_slice(&batch.instances[..take]);
            if take > 0 {
                ranges.push((batch.texture_id, start as u32, take as u32));
            }
        }
        if packed.is_empty() {
            return (0, 0);
        }
        queue.write_buffer(&self.instances, 0, bytemuck::cast_slice(&packed));

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("particle pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                resolve_target: None,
                ops: wgpu::Operations {
                    // Load, not Clear: this pass blends over the scene that the
                    // opaque, skybox and transparent passes already drew.
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: depth,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, camera_bind_group, &[]);
        pass.set_vertex_buffer(0, self.instances.slice(..));
        let mut draw_calls = 0u32;
        let mut triangles = 0u64;
        for (texture_id, start, count) in ranges {
            let bind_group = texture_id
                .and_then(|id| tex_registry.and_then(|r| r.get_bind_group(id)))
                .unwrap_or(default_texture);
            pass.set_bind_group(1, bind_group, &[]);
            pass.draw(0..6, start..(start + count));
            draw_calls += 1;
            triangles += (count as u64) * 2; // 2 triangles per instance (6-vertex billboard quad)
        }
        (draw_calls, triangles)
    }
}

const PARTICLE_WGSL: &str = r#"
struct CameraUniform {
    view_proj: mat4x4<f32>,
    light_view_proj: mat4x4<f32>,
    cam_pos: vec3<f32>,
    time: f32,
};
@group(0) @binding(0) var<uniform> camera: CameraUniform;
@group(1) @binding(0) var t_diffuse: texture_2d<f32>;
@group(1) @binding(1) var s_diffuse: sampler;

struct Instance {
    @location(0) position: vec3<f32>,
    @location(1) size: f32,
    @location(2) color: vec4<f32>,
};

struct VertOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
};

@vertex
fn vs_particle(@builtin(vertex_index) vi: u32, inst: Instance) -> VertOut {
    // Two triangles over the unit square, corners from the index rather than
    // from a buffer.
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, -1.0), vec2<f32>(-1.0, 1.0),
        vec2<f32>(-1.0, 1.0), vec2<f32>(1.0, -1.0), vec2<f32>(1.0, 1.0),
    );
    let corner = corners[vi];

    // The quad is built in the camera's plane, which is what makes it face the
    // camera from any angle. The first two rows of the view-projection are the
    // camera's right and up in world space, scaled by the projection -- so the
    // directions survive normalising even though the lengths do not.
    let right = normalize(vec3<f32>(
        camera.view_proj[0][0], camera.view_proj[1][0], camera.view_proj[2][0]));
    let up = normalize(vec3<f32>(
        camera.view_proj[0][1], camera.view_proj[1][1], camera.view_proj[2][1]));
    let world = inst.position + right * corner.x * inst.size + up * corner.y * inst.size;

    var out: VertOut;
    out.clip = camera.view_proj * vec4<f32>(world, 1.0);
    out.uv = corner * 0.5 + vec2<f32>(0.5, 0.5);
    out.color = inst.color;
    return out;
}

@fragment
fn fs_particle(in: VertOut) -> @location(0) vec4<f32> {
    return textureSample(t_diffuse, s_diffuse, in.uv) * in.color;
}
"#;
