//! The sRGB encode, done by a pass of our own, for an output that cannot do it.
//!
//! The frame is linear light right up to the target, and it is the *target's*
//! format that encodes it to sRGB on the way in: every desktop swapchain
//! offers an `…Srgb` format, and a WebGPU canvas, which does not, takes an
//! sRGB *view* of its `Rgba8Unorm` / `Bgra8Unorm` one (`srgb_view_of` in
//! `surface.rs`). An Android GLES surface offers neither -- `Rgba8Unorm` only,
//! and no `SURFACE_VIEW_FORMATS` -- so until this module its frame went out
//! with linear values stored as they were: a mid-grey 0.2 showed as 51/255,
//! not 124/255, and the whole game looked darker than on Vulkan (measured on
//! the android-35 emulator's GLES 3.1 translator, mini-arena).
//!
//! Here the frame is drawn, exactly as anywhere else, into an `Rgba8UnormSrgb`
//! texture of our own, and one full-screen pass at the end of the frame reads
//! it back (linear again: the sampler decodes sRGB) and writes
//! `linear_to_srgb` of it into the target. Every pass before that -- the
//! composite, FXAA/SMAA, decals' own encode, egui -- sees the same sRGB target
//! format it sees on a desktop, so none of them needs a second variant, and
//! one place owns the conversion.
//!
//! Engines converge on this: Bevy always draws into a texture of its own and
//! blits it to the surface, and Godot's compatibility (GLES3) renderer does the
//! linear-to-sRGB conversion in its own final pass. The cost is one full-screen
//! pass and one screen-sized texture, only where it is needed: an output
//! [`crate::output::Output::needs_srgb_encode`] says yes to.

use crate::pipeline_cache::CachedCreate;
use crate::profiler::TrackedTexture;

/// What the frame is drawn in when this pass encodes it: the format every
/// other output's frame has, so every pipeline built for it is the one a
/// desktop builds.
pub(crate) const ENCODED_FRAME_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

const SRGB_ENCODE_WGSL: &str = r#"
@group(0) @binding(0) var frame: texture_2d<f32>;

@vertex
fn vs_fullscreen(@builtin(vertex_index) vi: u32) -> @builtin(position) vec4<f32> {
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -3.0), vec2<f32>(-1.0, 1.0), vec2<f32>(3.0, 1.0),
    );
    return vec4<f32>(positions[vi], 0.0, 1.0);
}

// The IEC 61966-2-1 curve, the one an `…Srgb` target applies on a store.
fn linear_to_srgb(c: vec3<f32>) -> vec3<f32> {
    let lo = c * 12.92;
    let hi = 1.055 * pow(c, vec3<f32>(1.0 / 2.4)) - 0.055;
    return select(hi, lo, c <= vec3<f32>(0.0031308));
}

// One texel for one pixel: the frame and the target are the same size, so a
// load, not a sample -- no filtering to blur it and no sampler to bind.
@fragment
fn fs_encode(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let c = textureLoad(frame, vec2<i32>(pos.xy), 0);
    return vec4<f32>(linear_to_srgb(clamp(c.rgb, vec3<f32>(0.0), vec3<f32>(1.0))), c.a);
}
"#;

/// The frame texture and the pass that encodes it into the output.
pub(crate) struct SrgbEncode {
    frame: TrackedTexture,
    frame_view: wgpu::TextureView,
    layout: std::sync::Arc<wgpu::BindGroupLayout>,
    bind_group: wgpu::BindGroup,
    pipeline: std::sync::Arc<wgpu::RenderPipeline>,
}

impl SrgbEncode {
    /// For an output of `target_format` (`Rgba8Unorm` or `Bgra8Unorm`) at
    /// `width` x `height`.
    pub(crate) fn new(
        device: &wgpu::Device,
        width: u32,
        height: u32,
        target_format: wgpu::TextureFormat,
    ) -> Self {
        let layout = device.cached_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("srgb encode bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            }],
        });
        let shader = device.cached_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("srgb encode shader"),
            source: wgpu::ShaderSource::Wgsl(SRGB_ENCODE_WGSL.into()),
        });
        let pipeline_layout = device.cached_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("srgb encode pll"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.cached_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("srgb encode pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_fullscreen"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_encode"),
                // Written whole: the frame is the output, nothing is under it.
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });
        let (frame, frame_view) = Self::target(device, width, height);
        let bind_group = Self::bind(device, &layout, &frame_view);
        Self {
            frame,
            frame_view,
            layout,
            bind_group,
            pipeline,
        }
    }

    fn target(
        device: &wgpu::Device,
        width: u32,
        height: u32,
    ) -> (TrackedTexture, wgpu::TextureView) {
        let texture = crate::profiler::create_tracked_texture(
            device,
            &wgpu::TextureDescriptor {
                label: Some("srgb encode frame"),
                size: wgpu::Extent3d {
                    width: width.max(1),
                    height: height.max(1),
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: ENCODED_FRAME_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
        );
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        (texture, view)
    }

    fn bind(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        frame_view: &wgpu::TextureView,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("srgb encode bg"),
            layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(frame_view),
            }],
        })
    }

    /// Follows the output's size.
    pub(crate) fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        let (frame, frame_view) = Self::target(device, width, height);
        self.bind_group = Self::bind(device, &self.layout, &frame_view);
        self.frame = frame;
        self.frame_view = frame_view;
    }

    /// Where every pass of the frame draws instead of the output.
    pub(crate) fn frame_view(&self) -> &wgpu::TextureView {
        &self.frame_view
    }

    /// Encodes the finished frame into `target`, the output itself. The last
    /// pass of the frame: anything drawn after it would go into a texture no
    /// one reads.
    pub(crate) fn encode(&self, encoder: &mut wgpu::CommandEncoder, target: &wgpu::TextureView) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("srgb encode"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    // Every pixel is written; not loading spares a tiled GPU
                    // reading the old swapchain contents in.
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            ..Default::default()
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.draw(0..3, 0..1);
    }
}
