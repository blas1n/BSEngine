//! Multisample antialiasing for the geometry passes.
//!
//! The geometry passes -- opaque, skybox, transparent, particles -- draw into
//! multisampled colour, normal and depth targets and resolve the colour and
//! the normal into the single-sample targets every post pass reads, as Unity
//! URP and Godot do. WebGPU has no depth resolve, so the depth is resolved by
//! a small fullscreen pass of its own (Unity URP's CopyDepth, for the same
//! reason): SSAO, fog, depth of field, motion blur, the TAA reprojection and
//! SSR then read one depth per pixel as they always have.
//!
//! Only 4x: the one count WebGPU guarantees for multisample-capable formats.
//! Unity and Godot offer 2x and 8x too, which are per-adapter here.

/// The sample count MSAA uses.
pub const MSAA_SAMPLES: u32 = 4;

/// `desc` again with [`MSAA_SAMPLES`] samples, when the adapter supports
/// them: the pipeline a multisampled pass draws with. Everything else about
/// the two pipelines is the same, so they are made from one descriptor.
pub fn variant(
    device: &wgpu::Device,
    desc: &wgpu::RenderPipelineDescriptor<'_>,
    supported: bool,
) -> Option<wgpu::RenderPipeline> {
    supported.then(|| {
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            multisample: wgpu::MultisampleState {
                count: MSAA_SAMPLES,
                ..desc.multisample
            },
            ..desc.clone()
        })
    })
}

/// The multisampled targets the geometry passes draw into.
pub struct MsaaTargets {
    /// The size they were made at.
    pub size: (u32, u32),
    _hdr: crate::profiler::TrackedTexture,
    /// The scene colour; resolves into the post pass's HDR target.
    pub hdr_view: wgpu::TextureView,
    _normal: crate::profiler::TrackedTexture,
    /// The screen-space normal; resolves into the post pass's normal target.
    pub normal_view: wgpu::TextureView,
    /// The velocity, resolved like the normal (see `MESH_WGSL`'s
    /// `SceneOut::velocity`).
    pub velocity_view: wgpu::TextureView,
    _velocity: crate::profiler::TrackedTexture,
    _depth: crate::profiler::TrackedTexture,
    /// The depth; resolved by [`DepthResolve`].
    pub depth_view: wgpu::TextureView,
    /// [`DepthResolve`]'s read of `depth_view`.
    pub depth_resolve_bg: wgpu::BindGroup,
}

impl MsaaTargets {
    /// Targets at `width` x `height`.
    pub fn new(device: &wgpu::Device, resolve: &DepthResolve, width: u32, height: u32) -> Self {
        let make = |label: &str, format: wgpu::TextureFormat, extra: wgpu::TextureUsages| {
            let texture = crate::profiler::create_tracked_texture(
                device,
                &wgpu::TextureDescriptor {
                    label: Some(label),
                    size: wgpu::Extent3d {
                        width,
                        height,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: MSAA_SAMPLES,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT | extra,
                    view_formats: &[],
                },
            );
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            (texture, view)
        };
        let (hdr, hdr_view) = make(
            "msaa hdr",
            crate::post_process::HDR_FORMAT,
            wgpu::TextureUsages::empty(),
        );
        let (normal, normal_view) = make(
            "msaa normal",
            crate::post_process::NORMAL_FORMAT,
            wgpu::TextureUsages::empty(),
        );
        let (velocity, velocity_view) = make(
            "msaa velocity",
            crate::post_process::VELOCITY_FORMAT,
            wgpu::TextureUsages::empty(),
        );
        // Sampled as well as drawn into: the depth resolve reads it.
        let (depth, depth_view) = make(
            "msaa depth",
            crate::surface::DEPTH_FORMAT,
            wgpu::TextureUsages::TEXTURE_BINDING,
        );
        let depth_resolve_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("msaa depth resolve bg"),
            layout: &resolve.bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&depth_view),
            }],
        });
        Self {
            size: (width, height),
            _hdr: hdr,
            hdr_view,
            _normal: normal,
            normal_view,
            _velocity: velocity,
            velocity_view,
            _depth: depth,
            depth_view,
            depth_resolve_bg,
        }
    }
}

/// The depth resolve: a fullscreen pass that writes each pixel's sample 0 of
/// the multisampled depth as its fragment depth into the single-sample
/// depth target.
///
/// Sample 0 rather than the nearest or farthest sample: an edge pixel's
/// depth then belongs to one surface, as it would without MSAA, instead of a
/// mix the colour resolve has averaged but a depth cannot be. Unity URP's
/// CopyDepth reads one sample the same way.
pub struct DepthResolve {
    /// The layout of [`MsaaTargets::depth_resolve_bg`].
    pub bgl: wgpu::BindGroupLayout,
    /// The pass.
    pub pipeline: wgpu::RenderPipeline,
}

const DEPTH_RESOLVE_WGSL: &str = r#"
@group(0) @binding(0) var ms_depth: texture_depth_multisampled_2d;

@vertex
fn vs_fullscreen(@builtin(vertex_index) vi: u32) -> @builtin(position) vec4<f32> {
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -3.0), vec2<f32>(-1.0, 1.0), vec2<f32>(3.0, 1.0),
    );
    return vec4<f32>(positions[vi], 0.0, 1.0);
}

@fragment
fn fs_resolve(@builtin(position) pos: vec4<f32>) -> @builtin(frag_depth) f32 {
    return textureLoad(ms_depth, vec2<i32>(pos.xy), 0);
}
"#;

impl DepthResolve {
    /// The pipeline and its layout.
    pub fn new(device: &wgpu::Device) -> Self {
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("msaa depth resolve bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Depth,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: true,
                },
                count: None,
            }],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("msaa depth resolve shader"),
            source: wgpu::ShaderSource::Wgsl(DEPTH_RESOLVE_WGSL.into()),
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("msaa depth resolve pll"),
            bind_group_layouts: &[&bgl],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("msaa depth resolve pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_fullscreen",
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_resolve",
                targets: &[],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            // Every pixel is written: `Always`, over a target the pass clears.
            depth_stencil: Some(wgpu::DepthStencilState {
                format: crate::surface::DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Always,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });
        Self { bgl, pipeline }
    }
}
