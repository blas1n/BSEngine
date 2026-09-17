//! Decals, projected into a buffer the opaque pass reads before it lights.
//!
//! # Why a buffer and not a pass over the top
//!
//! A decal drawn after the opaque pass would be painted onto light that has
//! already been computed, so a blood splat in a dark corner would glow. What
//! goes here instead is Unity URP's DBuffer: decals are accumulated into a
//! screen-sized texture *before* the opaque pass, and the mesh shader folds
//! that into its albedo before any light touches it. A decal in shadow is then
//! dark, because it is the surface's own albedo that changed.
//!
//! The price is a depth prepass. The buffer is projected onto the depth the
//! opaque geometry will have, so that depth has to exist before the opaque pass
//! runs. There is no way around that in a forward renderer -- Unreal gets
//! decals without one only because it is deferred and already has a G-buffer.
//!
//! # Why the box, and why the fade
//!
//! Unity's `DecalProjector`, Unreal's `DecalActor` and Godot's `Decal` are all
//! a box with a texture, projected down one of its axes. All three also fade
//! the decal out as the receiving surface turns away from that axis, because a
//! box has no idea which surfaces it was meant for -- without the fade, a decal
//! that barely clips a wall smears down it in long streaks.

use wgpu::util::DeviceExt;

use crate::profiler::TrackedTexture;

/// What the renderer needs to draw one decal.
///
/// The matrix arrives already composed, the same way draw calls carry a model
/// matrix rather than a transform: the caller knows about entities, this does
/// not.
#[derive(Debug, Clone, Copy)]
pub struct DecalDraw {
    /// Box-to-world, including the decal's authored size as scale.
    pub model: glam::Mat4,
    /// How strongly it replaces what is under it, already clamped to `0..=1`.
    pub opacity: f32,
    /// Dot product between surface normal and projection axis below which the
    /// decal fades out.
    pub normal_fade: f32,
    /// Texture id in the [`GpuTextureRegistry`](crate::GpuTextureRegistry), or
    /// `None` for a path that has not resolved yet.
    pub texture: Option<u64>,
}

/// Format of the decal buffer.
///
/// sRGB rather than linear: what accumulates here is albedo, and the mesh
/// shader's own base colour is sRGB too. Accumulating in linear and folding
/// that into an sRGB albedo would make every decal read darker than the
/// texture its author looked at.
pub const DBUFFER_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

/// Bytes between one decal's uniform and the next.
///
/// 256 because that is `min_uniform_buffer_offset_alignment` on every adapter
/// this engine targets, and the buffer is bound as a window with a dynamic
/// offset. ⚠️ The binding's `size` must be this stride and not `None`: `None`
/// binds the whole buffer, which makes every non-zero offset a validation
/// error, and a size that is too small is accepted and quietly reads across
/// into the next decal.
pub const DECAL_STRIDE: u64 = 256;

/// One decal's uniform, matching `DecalUniform` in the shader.
///
/// `inv_view_proj` is repeated per decal rather than read from the camera
/// group, which does not carry one. Adding it there would mean changing
/// `CameraUniform`, and that struct is declared in six separate shaders that
/// must all agree on its size; 64 duplicated bytes per decal is the cheaper
/// mistake.
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct DecalUniform {
    inv_view_proj: [[f32; 4]; 4],
    model: [[f32; 4]; 4],
    inv_model: [[f32; 4]; 4],
    /// `opacity`, `normal_fade`, and padding.
    params: [f32; 4],
}

/// The shader that accumulates decals into the buffer.
///
/// # Why nothing is discarded
///
/// A pixel outside the box returns zero alpha instead of calling `discard`.
/// Both leave the buffer untouched under this blend, but `discard` makes
/// everything after it non-uniform control flow, and WGSL forbids `dpdx`/`dpdy`
/// there -- and the surface normal is *derived* from the reconstructed
/// position's derivatives, because a forward renderer has no normal buffer to
/// read. Computing unconditionally and weighing at the end keeps them legal.
const DECAL_WGSL: &str = r#"
// Must match the layout every other shader binding this buffer declares.
struct CameraUniform {
    view_proj: mat4x4<f32>,
    cascade_view_proj: array<mat4x4<f32>, 4>,
    cam_pos: vec3<f32>,
    time: f32,
    cam_forward: vec3<f32>,
    cascade_blend: f32,
    cascade_splits: vec4<f32>,
    cascade_count: u32,
};

struct DecalUniform {
    inv_view_proj: mat4x4<f32>,
    model: mat4x4<f32>,
    inv_model: mat4x4<f32>,
    // opacity, normal_fade, unused, unused
    params: vec4<f32>,
};

@group(0) @binding(0) var<uniform> camera: CameraUniform;
@group(1) @binding(0) var<uniform> decal: DecalUniform;
@group(2) @binding(0) var depth_tex: texture_depth_2d;
@group(3) @binding(0) var t_decal: texture_2d<f32>;
@group(3) @binding(1) var s_decal: sampler;

@vertex
fn vs_decal(@location(0) pos: vec3<f32>) -> @builtin(position) vec4<f32> {
    // A unit box in decal space; `model` carries position, rotation and size.
    return camera.view_proj * decal.model * vec4<f32>(pos, 1.0);
}

@fragment
fn fs_decal(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    let dims = vec2<f32>(textureDimensions(depth_tex, 0));
    let coord = vec2<i32>(frag.xy);
    let depth = textureLoad(depth_tex, coord, 0);

    // Screen pixel back to the world position of whatever was drawn there.
    let uv = frag.xy / dims;
    let ndc = vec3<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, depth);
    let world4 = decal.inv_view_proj * vec4<f32>(ndc, 1.0);
    let world = world4.xyz / world4.w;

    // Taken unconditionally: this is the only normal available to a forward
    // renderer here, and derivatives must be in uniform control flow.
    let surface_normal = normalize(cross(dpdx(world), dpdy(world)));

    let local = (decal.inv_model * vec4<f32>(world, 1.0)).xyz;
    let inside = all(abs(local) <= vec3<f32>(0.5));

    // Projected down the box's local -Y: the direction a decal placed with no
    // rotation sprays at a floor.
    let axis = normalize((decal.model * vec4<f32>(0.0, -1.0, 0.0, 0.0)).xyz);
    let facing = abs(dot(surface_normal, axis));
    // A clamped ramp rather than `smoothstep(fade, 1.0, facing)`. WGSL's
    // smoothstep is only defined for edge0 < edge1, so a threshold of exactly
    // 1 -- or above it, which a caller reaching this type directly can pass --
    // returns something other than "fade everything out". Written out, it is
    // monotone at every threshold and degenerates to a hard step at 1.
    let fade_span = max(1.0 - decal.params.y, 1e-4);
    let fade = clamp((facing - decal.params.y) / fade_span, 0.0, 1.0);

    // An explicit level: the box's screen footprint has nothing to do with the
    // texture's, so an implicit derivative would pick a mip from the wrong rate
    // of change.
    let tex_uv = vec2<f32>(local.x + 0.5, 0.5 - local.z);
    let texel = textureSampleLevel(t_decal, s_decal, tex_uv, 0.0);

    // The far plane is not a surface: nothing was drawn at this pixel.
    let hit = select(0.0, 1.0, depth < 1.0);
    let a = texel.a * decal.params.x * fade * select(0.0, 1.0, inside) * hit;

    // Premultiplied. The buffer accumulates `sum(colour * a)` in rgb and
    // `product(1 - a)` in alpha, so the mesh shader finishes with
    // `albedo * dst.a + dst.rgb`.
    return vec4<f32>(texel.rgb * a, a);
}
"#;

/// A unit cube centred on the origin.
///
/// Its own buffer rather than a mesh from the registry: a decal must draw
/// whether or not the project happens to contain a cube asset, and a box is
/// twelve triangles.
const CUBE_CORNERS: [[f32; 3]; 8] = [
    [-0.5, -0.5, -0.5],
    [0.5, -0.5, -0.5],
    [0.5, 0.5, -0.5],
    [-0.5, 0.5, -0.5],
    [-0.5, -0.5, 0.5],
    [0.5, -0.5, 0.5],
    [0.5, 0.5, 0.5],
    [-0.5, 0.5, 0.5],
];

/// Triangles of [`CUBE_CORNERS`].
const CUBE_INDICES: [u32; 36] = [
    0, 2, 1, 0, 3, 2, // -Z
    4, 5, 6, 4, 6, 7, // +Z
    0, 4, 7, 0, 7, 3, // -X
    1, 2, 6, 1, 6, 5, // +X
    0, 1, 5, 0, 5, 4, // -Y
    3, 7, 6, 3, 6, 2, // +Y
];

/// Everything the decal pass owns.
pub struct DecalResources {
    /// Screen-sized buffer the opaque pass reads.
    pub view: wgpu::TextureView,
    _buffer: TrackedTexture,
    pipeline: wgpu::RenderPipeline,
    uniform: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,
    depth_bind_group: wgpu::BindGroup,
    depth_bgl: wgpu::BindGroupLayout,
    cube_vertices: wgpu::Buffer,
    cube_indices: wgpu::Buffer,
}

impl std::fmt::Debug for DecalResources {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DecalResources").finish_non_exhaustive()
    }
}

impl DecalResources {
    /// Builds the buffer, pipeline and geometry.
    ///
    /// `camera_bgl` and `texture_bgl` are the renderer's own, so a decal's
    /// texture is bound exactly the way a mesh's is.
    pub fn new(
        device: &wgpu::Device,
        width: u32,
        height: u32,
        depth_view: &wgpu::TextureView,
        camera_bgl: &wgpu::BindGroupLayout,
        texture_bgl: &wgpu::BindGroupLayout,
        max_decals: usize,
    ) -> Self {
        let (buffer, view) = Self::create_buffer(device, width, height);

        let uniform_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("decal uniform bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: true,
                    // ⚠️ The stride, never `None`. See `DECAL_STRIDE`.
                    min_binding_size: wgpu::BufferSize::new(DECAL_STRIDE),
                },
                count: None,
            }],
        });
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("decal uniform"),
            size: DECAL_STRIDE * max_decals as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("decal uniform bg"),
            layout: &uniform_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &uniform,
                    offset: 0,
                    size: wgpu::BufferSize::new(DECAL_STRIDE),
                }),
            }],
        });

        let depth_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("decal depth bgl"),
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
        let depth_bind_group = Self::create_depth_bind_group(device, &depth_bgl, depth_view);

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("decal shader"),
            source: wgpu::ShaderSource::Wgsl(DECAL_WGSL.into()),
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("decal pipeline layout"),
            bind_group_layouts: &[camera_bgl, &uniform_bgl, &depth_bgl, texture_bgl],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("decal pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_decal",
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: 12,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[wgpu::VertexAttribute {
                        format: wgpu::VertexFormat::Float32x3,
                        offset: 0,
                        shader_location: 0,
                    }],
                }],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_decal",
                targets: &[Some(wgpu::ColorTargetState {
                    format: DBUFFER_FORMAT,
                    // Premultiplied accumulation. rgb sums the contributions;
                    // alpha multiplies out to how much of the base albedo is
                    // left, which is what the mesh shader scales by.
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::Zero,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                // Front faces culled, not back: the camera is often *inside* a
                // decal's box, and culling the near faces is what keeps the
                // box covering its screen footprint from in there.
                cull_mode: Some(wgpu::Face::Front),
                front_face: wgpu::FrontFace::Ccw,
                ..Default::default()
            },
            // No depth attachment at all. The pass reads the depth texture as a
            // texture, and a depth buffer cannot be both at once.
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let cube_vertices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("decal cube vertices"),
            contents: bytemuck::cast_slice(&CUBE_CORNERS),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let cube_indices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("decal cube indices"),
            contents: bytemuck::cast_slice(&CUBE_INDICES),
            usage: wgpu::BufferUsages::INDEX,
        });

        Self {
            view,
            _buffer: buffer,
            pipeline,
            uniform,
            uniform_bind_group,
            depth_bind_group,
            depth_bgl,
            cube_vertices,
            cube_indices,
        }
    }

    fn create_buffer(
        device: &wgpu::Device,
        width: u32,
        height: u32,
    ) -> (TrackedTexture, wgpu::TextureView) {
        let texture = crate::profiler::create_tracked_texture(
            device,
            &wgpu::TextureDescriptor {
                label: Some("decal buffer"),
                size: wgpu::Extent3d {
                    width: width.max(1),
                    height: height.max(1),
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: DBUFFER_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
        );
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        (texture, view)
    }

    fn create_depth_bind_group(
        device: &wgpu::Device,
        bgl: &wgpu::BindGroupLayout,
        depth_view: &wgpu::TextureView,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("decal depth bg"),
            layout: bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(depth_view),
            }],
        })
    }

    /// Rebuilds the screen-sized buffer and the depth binding after a resize.
    pub fn resize(
        &mut self,
        device: &wgpu::Device,
        width: u32,
        height: u32,
        depth_view: &wgpu::TextureView,
    ) {
        let (buffer, view) = Self::create_buffer(device, width, height);
        self._buffer = buffer;
        self.view = view;
        self.depth_bind_group = Self::create_depth_bind_group(device, &self.depth_bgl, depth_view);
    }

    /// Writes this frame's decals into the uniform buffer.
    ///
    /// Returns how many will be drawn, which is the input count clamped to what
    /// the buffer holds -- an over-specified scene still renders, the same way
    /// an over-specified probe volume does.
    pub fn upload(
        &self,
        queue: &wgpu::Queue,
        decals: &[DecalDraw],
        view_proj: glam::Mat4,
        max_decals: usize,
    ) -> usize {
        let count = decals.len().min(max_decals);
        let inv_view_proj = view_proj.inverse().to_cols_array_2d();
        let mut bytes = vec![0u8; count * DECAL_STRIDE as usize];
        for (i, decal) in decals[..count].iter().enumerate() {
            let uniform = DecalUniform {
                inv_view_proj,
                model: decal.model.to_cols_array_2d(),
                inv_model: decal.model.inverse().to_cols_array_2d(),
                params: [decal.opacity, decal.normal_fade, 0.0, 0.0],
            };
            let at = i * DECAL_STRIDE as usize;
            bytes[at..at + std::mem::size_of::<DecalUniform>()]
                .copy_from_slice(bytemuck::bytes_of(&uniform));
        }
        if !bytes.is_empty() {
            queue.write_buffer(&self.uniform, 0, &bytes);
        }
        count
    }

    /// Draws `count` decals into the buffer.
    ///
    /// The caller has already begun the pass with the buffer attached and
    /// cleared to `(0, 0, 0, 1)` -- nothing projected yet means all of the base
    /// albedo survives.
    pub fn draw<'a>(
        &'a self,
        pass: &mut wgpu::RenderPass<'a>,
        camera_bind_group: &'a wgpu::BindGroup,
        decals: &[DecalDraw],
        count: usize,
        tex_registry: Option<&'a crate::GpuTextureRegistry>,
        default_texture_bind_group: &'a wgpu::BindGroup,
    ) {
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, camera_bind_group, &[]);
        pass.set_bind_group(2, &self.depth_bind_group, &[]);
        pass.set_vertex_buffer(0, self.cube_vertices.slice(..));
        pass.set_index_buffer(self.cube_indices.slice(..), wgpu::IndexFormat::Uint32);
        for (i, decal) in decals[..count].iter().enumerate() {
            let tex_bg = decal
                .texture
                .and_then(|id| tex_registry.and_then(|r| r.get_bind_group(id)))
                .unwrap_or(default_texture_bind_group);
            pass.set_bind_group(
                1,
                &self.uniform_bind_group,
                &[(i as u64 * DECAL_STRIDE) as u32],
            );
            pass.set_bind_group(3, tex_bg, &[]);
            pass.draw_indexed(0..CUBE_INDICES.len() as u32, 0, 0..1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_cube_is_closed() {
        // Twelve triangles, every corner used: a box missing a face would let
        // the camera see through it from one side, and the decal would vanish
        // from that angle only.
        assert_eq!(CUBE_INDICES.len(), 36);
        for corner in 0..8u32 {
            assert!(
                CUBE_INDICES.contains(&corner),
                "corner {corner} is in no triangle"
            );
        }
    }

    #[test]
    fn the_uniform_fits_the_stride() {
        // ⚠️ A uniform larger than the stride would have each decal read into
        // the next one's bytes, with no validation error anywhere.
        assert!(
            std::mem::size_of::<DecalUniform>() as u64 <= DECAL_STRIDE,
            "{} > {DECAL_STRIDE}",
            std::mem::size_of::<DecalUniform>()
        );
    }
}
