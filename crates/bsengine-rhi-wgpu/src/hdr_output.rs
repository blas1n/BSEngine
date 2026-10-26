//! HDR display output's UI layer, and the HDR10 output pass.
//!
//! Under HDR output (`bsengine_core::HdrOutputSettings`) the frame is linear
//! scRGB in a float target, where 1.0 is 80 nits -- the swapchain itself under
//! scRGB output, and under HDR10 a frame of its own that [`PqOutput`] encodes
//! into the swapchain. egui cannot draw into that:
//! egui-wgpu picks its output encoding from whether the target is sRGB, and a
//! float target is not, so it would write gamma-encoded values that the
//! display reads as linear -- every colour too bright and washed out -- and at
//! 80 nits for white, a third of paper white. So the UI is drawn into an 8-bit
//! sRGB layer of its own, exactly as in SDR, and that layer is laid over the
//! frame here at paper white, as Unity and Unreal composite their UI under HDR
//! (Unreal's `r.HDR.UI.Level`).

use crate::pipeline_cache::CachedCreate;
use crate::profiler::TrackedTexture;

/// The format the UI is drawn in under HDR output: what it would be in SDR.
pub(crate) const UI_LAYER_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

const UI_COMPOSITE_WGSL: &str = r#"
struct Ui {
    // SDR white in the target: paper white nits / 80.
    scale: f32,
    _p0: f32, _p1: f32, _p2: f32,
}
@group(0) @binding(0) var ui_tex: texture_2d<f32>;
@group(0) @binding(1) var<uniform> ui: Ui;

@vertex
fn vs_fullscreen(@builtin(vertex_index) vi: u32) -> @builtin(position) vec4<f32> {
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -3.0), vec2<f32>(-1.0, 1.0), vec2<f32>(3.0, 1.0),
    );
    return vec4<f32>(positions[vi], 0.0, 1.0);
}

// The layer as egui left it: premultiplied, and linear once the sRGB texture
// is read. Its colour goes up to paper white; its coverage stays as it is,
// for the blend (one, one minus source alpha) to lay it over the frame.
@fragment
fn fs_ui(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let c = textureLoad(ui_tex, vec2<i32>(pos.xy), 0);
    return vec4<f32>(c.rgb * ui.scale, c.a);
}
"#;

/// The UI layer and the pass that lays it over an HDR frame.
pub(crate) struct UiLayer {
    texture: TrackedTexture,
    view: wgpu::TextureView,
    layout: std::sync::Arc<wgpu::BindGroupLayout>,
    uniform: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    pipeline: std::sync::Arc<wgpu::RenderPipeline>,
}

impl UiLayer {
    pub(crate) fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        width: u32,
        height: u32,
        target_format: wgpu::TextureFormat,
        paper_white_scale: f32,
    ) -> Self {
        let layout = device.cached_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("hdr ui layer bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(16),
                    },
                    count: None,
                },
            ],
        });
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("hdr ui layer uniform"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(
            &uniform,
            0,
            bytemuck::cast_slice(&[paper_white_scale, 0.0, 0.0, 0.0]),
        );
        let shader = device.cached_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("hdr ui layer shader"),
            source: wgpu::ShaderSource::Wgsl(UI_COMPOSITE_WGSL.into()),
        });
        let pipeline_layout = device.cached_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("hdr ui layer pll"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let blend = wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::One,
            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
            operation: wgpu::BlendOperation::Add,
        };
        let pipeline = device.cached_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("hdr ui layer pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_fullscreen"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_ui"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    blend: Some(wgpu::BlendState {
                        color: blend,
                        alpha: blend,
                    }),
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
        let (texture, view) = Self::target(device, width, height);
        let bind_group = Self::bind(device, &layout, &view, &uniform);
        Self {
            texture,
            view,
            layout,
            uniform,
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
                label: Some("hdr ui layer"),
                size: wgpu::Extent3d {
                    width: width.max(1),
                    height: height.max(1),
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: UI_LAYER_FORMAT,
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
        view: &wgpu::TextureView,
        uniform: &wgpu::Buffer,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("hdr ui layer bg"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: uniform.as_entire_binding(),
                },
            ],
        })
    }

    /// Follows the output's size.
    pub(crate) fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        let (texture, view) = Self::target(device, width, height);
        self.bind_group = Self::bind(device, &self.layout, &view, &self.uniform);
        self.texture = texture;
        self.view = view;
    }

    /// Where egui draws, cleared to transparent each frame.
    pub(crate) fn view(&self) -> &wgpu::TextureView {
        &self.view
    }

    /// Lays the layer over `target` at paper white.
    pub(crate) fn composite(&self, encoder: &mut wgpu::CommandEncoder, target: &wgpu::TextureView) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("hdr ui layer composite"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
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

/// The format of the frame the post-process chain writes under HDR10 output:
/// the scRGB one it writes straight into the swapchain under scRGB output.
pub(crate) const PQ_FRAME_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

/// The swapchain format of HDR10 output: ten bits a channel, which PQ needs
/// to show its 0 to 10,000 nits without banding (eight is not enough).
pub(crate) const PQ_SWAPCHAIN_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgb10a2Unorm;

const PQ_ENCODE_WGSL: &str = r#"
struct Pq {
    // SDR white in scRGB, as the UI layer's: paper white nits / 80.
    ui_scale: f32,
    _p0: f32, _p1: f32, _p2: f32,
}
@group(0) @binding(0) var frame_tex: texture_2d<f32>;
@group(0) @binding(1) var ui_tex: texture_2d<f32>;
@group(0) @binding(2) var<uniform> pq: Pq;

@vertex
fn vs_fullscreen(@builtin(vertex_index) vi: u32) -> @builtin(position) vec4<f32> {
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -3.0), vec2<f32>(-1.0, 1.0), vec2<f32>(3.0, 1.0),
    );
    return vec4<f32>(positions[vi], 0.0, 1.0);
}

// BT.709 to BT.2020 primaries (ITU-R BT.2087), both D65, on linear light.
// WGSL builds a matrix from its columns: the first is where linear red goes.
const REC709_TO_REC2020 = mat3x3<f32>(
    vec3<f32>(0.6274040, 0.0690970, 0.0163916),
    vec3<f32>(0.3292820, 0.9195400, 0.0880132),
    vec3<f32>(0.0433136, 0.0113612, 0.8955950),
);

// SMPTE ST 2084's inverse EOTF: absolute nits to the PQ signal, 0..1 for
// 0..10,000 nits.
fn pq_encode(nits: vec3<f32>) -> vec3<f32> {
    let m1 = 0.1593017578125;
    let m2 = 78.84375;
    let c1 = 0.8359375;
    let c2 = 18.8515625;
    let c3 = 18.6875;
    let y = pow(clamp(nits / 10000.0, vec3<f32>(0.0), vec3<f32>(1.0)), vec3<f32>(m1));
    return pow((c1 + c2 * y) / (1.0 + c3 * y), vec3<f32>(m2));
}

// scRGB (linear BT.709, 1.0 = 80 nits) to the HDR10 signal. Negative scRGB --
// a colour outside BT.709 -- is clipped first: the frame has none to keep.
fn to_hdr10(scrgb: vec3<f32>) -> vec4<f32> {
    let nits = REC709_TO_REC2020 * max(scrgb, vec3<f32>(0.0)) * 80.0;
    return vec4<f32>(pq_encode(nits), 1.0);
}

@fragment
fn fs_encode(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    return to_hdr10(textureLoad(frame_tex, vec2<i32>(pos.xy), 0).rgb);
}

// The UI layer laid over the frame first, in linear light at paper white, as
// `fs_ui` does under scRGB output -- a blend the PQ signal itself cannot
// take, being far from linear.
@fragment
fn fs_encode_ui(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let p = vec2<i32>(pos.xy);
    let frame = textureLoad(frame_tex, p, 0).rgb;
    let ui = textureLoad(ui_tex, p, 0);
    return to_hdr10(frame * (1.0 - ui.a) + ui.rgb * pq.ui_scale);
}
"#;

/// HDR10 output: the frame the post-process chain writes, in scRGB, and the
/// pass that turns it into the PQ signal the swapchain carries.
///
/// A pass of its own rather than PQ written by the composite: the UI has to be
/// laid over the frame in linear light, and after the composite come FXAA and
/// SMAA, whose thresholds are for the scRGB frame scaled by paper white (see
/// `post_process`). So everything up to the UI runs exactly as under scRGB
/// output, into [`PQ_FRAME_FORMAT`], and this is the one step that differs --
/// Unreal composites its UI in a pass of its own under HDR too.
pub(crate) struct PqOutput {
    frame: TrackedTexture,
    frame_view: wgpu::TextureView,
    layout: std::sync::Arc<wgpu::BindGroupLayout>,
    uniform: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    encode: std::sync::Arc<wgpu::RenderPipeline>,
    encode_ui: std::sync::Arc<wgpu::RenderPipeline>,
}

impl PqOutput {
    pub(crate) fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        width: u32,
        height: u32,
        ui_view: &wgpu::TextureView,
        paper_white_scale: f32,
    ) -> Self {
        let texture_entry = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: false },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };
        let layout = device.cached_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("hdr10 output bgl"),
            entries: &[
                texture_entry(0),
                texture_entry(1),
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(16),
                    },
                    count: None,
                },
            ],
        });
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("hdr10 output uniform"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(
            &uniform,
            0,
            bytemuck::cast_slice(&[paper_white_scale, 0.0, 0.0, 0.0]),
        );
        let shader = device.cached_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("hdr10 output shader"),
            source: wgpu::ShaderSource::Wgsl(PQ_ENCODE_WGSL.into()),
        });
        let pipeline_layout = device.cached_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("hdr10 output pll"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = |entry_point: &str| {
            device.cached_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("hdr10 output pipeline"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_fullscreen"),
                    buffers: &[],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some(entry_point),
                    // Written whole: the frame is opaque, and the signal is
                    // not something to blend onto.
                    targets: &[Some(wgpu::ColorTargetState {
                        format: PQ_SWAPCHAIN_FORMAT,
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
            })
        };
        let encode = pipeline("fs_encode");
        let encode_ui = pipeline("fs_encode_ui");
        let (frame, frame_view) = Self::target(device, width, height);
        let bind_group = Self::bind(device, &layout, &frame_view, ui_view, &uniform);
        Self {
            frame,
            frame_view,
            layout,
            uniform,
            bind_group,
            encode,
            encode_ui,
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
                label: Some("hdr10 frame"),
                size: wgpu::Extent3d {
                    width: width.max(1),
                    height: height.max(1),
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: PQ_FRAME_FORMAT,
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
        ui_view: &wgpu::TextureView,
        uniform: &wgpu::Buffer,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("hdr10 output bg"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(frame_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(ui_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: uniform.as_entire_binding(),
                },
            ],
        })
    }

    /// Follows the output's size. `ui_view` is the UI layer's new view: the
    /// layer resizes with the output too, so it is passed in after it has.
    pub(crate) fn resize(
        &mut self,
        device: &wgpu::Device,
        width: u32,
        height: u32,
        ui_view: &wgpu::TextureView,
    ) {
        let (frame, frame_view) = Self::target(device, width, height);
        self.bind_group = Self::bind(device, &self.layout, &frame_view, ui_view, &self.uniform);
        self.frame = frame;
        self.frame_view = frame_view;
    }

    /// Where the post-process chain writes the frame.
    pub(crate) fn frame_view(&self) -> &wgpu::TextureView {
        &self.frame_view
    }

    /// Encodes the frame into `target`, the swapchain, with the UI layer laid
    /// over it when `with_ui` -- that is, when egui drew into the layer this
    /// frame; otherwise the layer still holds the last frame that had a UI.
    pub(crate) fn encode(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        with_ui: bool,
    ) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("hdr10 output"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            ..Default::default()
        });
        pass.set_pipeline(if with_ui {
            &self.encode_ui
        } else {
            &self.encode
        });
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.draw(0..3, 0..1);
    }
}
