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

/// Where HDR output puts SDR white and the display's peak, settled from the
/// project's settings and the display's own ([`resolve_hdr_levels`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HdrLevels {
    /// SDR white as an scRGB value: what the frame writes for it, and the UI's
    /// white. Paper white nits / 80 where the signal is in absolute units.
    pub white: f32,
    /// The display's peak as a multiple of SDR white, 1 at least: where the
    /// highlights roll off to.
    pub peak: f32,
}

/// [`HdrLevels`] for `settings` on the display `display` describes, sending
/// `signal`. A level the project sets wins; one it leaves out is the display's,
/// and where the display says nothing, the defaults
/// (`bsengine_core::hdr_output::DEFAULT_PAPER_WHITE_NITS` / `DEFAULT_MAX_NITS`).
///
/// The display's levels come in two kinds, which is what this settles:
///
/// - **Absolute nits** (Windows, through DXGI): paper white is the luminance
///   the OS shows SDR white at -- the "SDR content brightness" slider -- and the
///   peak the display's own. scRGB is absolute there (1.0 = 80 nits) and so is
///   HDR10's PQ, so SDR white is paper white / 80 under both.
/// - **EDR headroom** (macOS): a multiple of the system's current SDR white,
///   and no nits at all. scRGB on Apple's EDR is relative -- 1.0 *is* the
///   system's SDR white -- so SDR white is 1.0 there whatever paper white is,
///   and the peak is the headroom: the 2.5 that 200 nits / 80 gives would put
///   the UI and every mid-tone two and a half times above the desktop's white,
///   and clip them where the headroom is smaller. HDR10 stays absolute on
///   macOS too, so it takes paper white in nits as elsewhere.
pub fn resolve_hdr_levels(
    settings: &bsengine_core::HdrOutputSettings,
    display: &wgpu::DisplayHdrInfo,
    signal: crate::surface::HdrSignal,
) -> HdrLevels {
    use bsengine_core::hdr_output::{DEFAULT_MAX_NITS, DEFAULT_PAPER_WHITE_NITS};
    // A level the OS reports as 0 or less is no level (DXGI can say 0).
    let positive = |v: f32| v > 0.0;
    let luminance = display.luminance.unwrap_or_default();
    let paper = settings
        .paper_white_nits
        .or(luminance.sdr_white_nits.filter(|&v| positive(v)))
        .filter(|&v| positive(v))
        .unwrap_or(DEFAULT_PAPER_WHITE_NITS);
    if let (crate::surface::HdrSignal::Scrgb, Some(headroom)) = (signal, display.headroom) {
        // The headroom there is now, ahead of the most there could be: it is
        // what the display shows at this brightness, and Apple's EDR clips
        // above it. A project's own peak is in nits, so as a multiple of its
        // paper white.
        let peak = settings
            .max_nits
            .map(|max| max / paper)
            .or(headroom.current.filter(|&v| positive(v)))
            .or(headroom.potential.filter(|&v| positive(v)))
            .unwrap_or(DEFAULT_MAX_NITS / DEFAULT_PAPER_WHITE_NITS);
        return HdrLevels {
            white: 1.0,
            peak: peak.max(1.0),
        };
    }
    let max = settings
        .max_nits
        .or(luminance.max_nits.filter(|&v| positive(v)))
        .filter(|&v| positive(v))
        .unwrap_or(DEFAULT_MAX_NITS);
    HdrLevels {
        white: paper / 80.0,
        peak: (max / paper).max(1.0),
    }
}

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

    /// Puts the UI's white at `white` (scRGB) from the next frame.
    pub(crate) fn set_white(&self, queue: &wgpu::Queue, white: f32) {
        queue.write_buffer(
            &self.uniform,
            0,
            bytemuck::cast_slice(&[white, 0.0, 0.0, 0.0]),
        );
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

    /// Puts the UI's white at `white` (scRGB) from the next frame.
    pub(crate) fn set_white(&self, queue: &wgpu::Queue, white: f32) {
        queue.write_buffer(
            &self.uniform,
            0,
            bytemuck::cast_slice(&[white, 0.0, 0.0, 0.0]),
        );
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::surface::HdrSignal;
    use bsengine_core::HdrOutputSettings;

    fn on(paper: Option<f32>, max: Option<f32>) -> HdrOutputSettings {
        HdrOutputSettings {
            enabled: true,
            paper_white_nits: paper,
            max_nits: max,
            ..Default::default()
        }
    }

    /// A Windows display: SDR white at 280 nits by the brightness slider, a
    /// 1500-nit panel.
    fn windows() -> wgpu::DisplayHdrInfo {
        wgpu::DisplayHdrInfo {
            luminance: Some(wgpu::DisplayLuminance {
                max_nits: Some(1500.0),
                sdr_white_nits: Some(280.0),
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    /// A Mac in EDR with 3.5x headroom now, 16x at best.
    fn mac() -> wgpu::DisplayHdrInfo {
        wgpu::DisplayHdrInfo {
            headroom: Some(wgpu::DisplayHeadroom {
                current: Some(3.5),
                potential: Some(16.0),
                reference: None,
            }),
            ..Default::default()
        }
    }

    fn close(a: HdrLevels, white: f32, peak: f32) -> bool {
        (a.white - white).abs() < 1e-4 && (a.peak - peak).abs() < 1e-4
    }

    /// The display's own levels where the project leaves them out, on both
    /// signals, which are absolute on Windows: SDR white 280 / 80 = 3.5 in
    /// scRGB, peak 1500 / 280.
    #[test]
    fn a_windows_display_gives_its_sdr_white_and_peak() {
        for signal in [HdrSignal::Scrgb, HdrSignal::Hdr10] {
            let l = resolve_hdr_levels(&on(None, None), &windows(), signal);
            assert!(close(l, 3.5, 1500.0 / 280.0), "{signal:?}: {l:?}");
        }
    }

    /// The project's own levels win over the display's, each on its own: a
    /// paper white set alone still takes the display's peak, and the other way
    /// about.
    #[test]
    fn the_projects_levels_win_over_the_displays() {
        let s = HdrSignal::Hdr10;
        let both = resolve_hdr_levels(&on(Some(160.0), Some(800.0)), &windows(), s);
        assert!(close(both, 2.0, 5.0), "{both:?}");
        let paper = resolve_hdr_levels(&on(Some(160.0), None), &windows(), s);
        assert!(close(paper, 2.0, 1500.0 / 160.0), "{paper:?}");
        let max = resolve_hdr_levels(&on(None, Some(800.0)), &windows(), s);
        assert!(close(max, 3.5, 800.0 / 280.0), "{max:?}");
    }

    /// A display that reports nothing (Linux, a browser) gets the defaults --
    /// 200 nits, 1000 -- and so does one that reports a level of 0, which DXGI
    /// can; a peak at or under paper white is still 1x, never less.
    #[test]
    fn nothing_reported_is_the_defaults() {
        let s = HdrSignal::Scrgb;
        let none = resolve_hdr_levels(&on(None, None), &wgpu::DisplayHdrInfo::default(), s);
        assert!(close(none, 2.5, 5.0), "{none:?}");
        let zeros = wgpu::DisplayHdrInfo {
            luminance: Some(wgpu::DisplayLuminance {
                max_nits: Some(0.0),
                sdr_white_nits: Some(0.0),
                ..Default::default()
            }),
            ..Default::default()
        };
        let z = resolve_hdr_levels(&on(None, None), &zeros, s);
        assert!(close(z, 2.5, 5.0), "{z:?}");
        let dim = resolve_hdr_levels(&on(Some(300.0), Some(250.0)), &zeros, s);
        assert!(close(dim, 3.75, 1.0), "{dim:?}");
    }

    /// On a Mac's EDR, scRGB 1.0 is the system's SDR white, so that is where
    /// SDR white goes -- not at 200 nits / 80, two and a half times above the
    /// desktop -- and the peak is the headroom there is now, not the most
    /// there could be. HDR10 is absolute on a Mac too: paper white in nits,
    /// as anywhere else.
    #[test]
    fn a_mac_puts_sdr_white_at_edr_white_and_peaks_at_its_headroom() {
        let l = resolve_hdr_levels(&on(None, None), &mac(), HdrSignal::Scrgb);
        assert!(close(l, 1.0, 3.5), "scRGB on EDR: {l:?}");
        let pq = resolve_hdr_levels(&on(None, None), &mac(), HdrSignal::Hdr10);
        assert!(close(pq, 2.5, 5.0), "HDR10 on a Mac is absolute: {pq:?}");
        // A project's own peak is in nits: as a multiple of its paper white.
        let set = resolve_hdr_levels(&on(Some(250.0), Some(1000.0)), &mac(), HdrSignal::Scrgb);
        assert!(close(set, 1.0, 4.0), "{set:?}");
        // No headroom now reported: the most there could be.
        let potential_only = wgpu::DisplayHdrInfo {
            headroom: Some(wgpu::DisplayHeadroom {
                current: None,
                potential: Some(16.0),
                reference: None,
            }),
            ..Default::default()
        };
        let p = resolve_hdr_levels(&on(None, None), &potential_only, HdrSignal::Scrgb);
        assert!(close(p, 1.0, 16.0), "{p:?}");
    }
}
