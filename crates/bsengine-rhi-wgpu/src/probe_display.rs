//! The editor's light-probe display: every probe the scene shades from,
//! drawn as a small sphere where it is, lit by its own SH.
//!
//! What Unity's Rendering Debugger "Display Probes" and Unreal's Volumetric
//! Lightmap show flag do: the spheres are what the baked (or realtime)
//! lighting *is*, so a dark probe inside a wall, a level of bricks that never
//! got cut, or light leaking through a floor can be seen rather than
//! inferred from the shading it causes.
//!
//! Read from the probe buffer the scene shader reads, through the same
//! `probe_grid_wgsl` -- positions, relocation offsets, the half shading uses
//! -- so the display cannot disagree with the lighting by being fed a
//! second copy. A sphere's radius is a fraction of its probe's spacing, so
//! an adaptive volume's levels show as sizes: finer bricks, smaller spheres
//! (Unity scales its probe spheres by brick size the same way).
//!
//! No vertex buffer: each sphere's vertices come from `vertex_index`, the
//! probe from `instance_index`.

use crate::pipeline_cache::CachedCreate;
use crate::post_process::HDR_FORMAT;
use std::sync::Arc;

/// Longitude segments and latitude rings of each sphere. 144 triangles: a
/// volume can hold [`bsengine_core::MAX_ADAPTIVE_PROBES`], and at the
/// handful of pixels a sphere covers a finer one looks no rounder.
const SEGMENTS: u32 = 12;
const RINGS: u32 = 6;

/// Vertices one sphere is drawn with: two triangles per segment per ring.
pub(crate) const VERTICES_PER_PROBE: u32 = SEGMENTS * RINGS * 6;

/// The pass's pipelines, built the first time the display is turned on: a
/// game never turns it on, and should not compile it at start.
struct Pipelines {
    pipeline: Arc<wgpu::RenderPipeline>,
    /// For a multisampled pass (`None` where the adapter cannot).
    pipeline_msaa: Option<Arc<wgpu::RenderPipeline>>,
    probes_bgl: Arc<wgpu::BindGroupLayout>,
}

/// The light-probe display pass.
pub(crate) struct ProbeDisplay {
    camera_bgl: Arc<wgpu::BindGroupLayout>,
    msaa_supported: bool,
    pipelines: Option<Pipelines>,
}

impl ProbeDisplay {
    /// Against the scene's camera bind group layout, so the pass reads the
    /// camera uniform the frame already wrote. Builds nothing yet.
    pub(crate) fn new(camera_bgl: Arc<wgpu::BindGroupLayout>, msaa_supported: bool) -> Self {
        Self {
            camera_bgl,
            msaa_supported,
            pipelines: None,
        }
    }

    fn pipelines(&mut self, device: &wgpu::Device) -> &Pipelines {
        let (camera_bgl, msaa_supported) = (&self.camera_bgl, self.msaa_supported);
        self.pipelines.get_or_insert_with(|| {
            let probes_bgl = device.cached_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("probe display bgl"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    // The vertex stage places the sphere (position, offset,
                    // spacing); the fragment stage shades it (SH).
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });
            let shader = device.cached_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("probe display shader"),
                source: wgpu::ShaderSource::Wgsl(probe_display_wgsl().into()),
            });
            let layout = device.cached_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("probe display pipeline layout"),
                bind_group_layouts: &[Some(camera_bgl), Some(&probes_bgl)],
                immediate_size: 0,
            });
            let desc = wgpu::RenderPipelineDescriptor {
                label: Some("probe display pipeline"),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_probe"),
                    buffers: &[],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_probe"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: HDR_FORMAT,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    // Solid spheres: the depth test hides their far halves,
                    // so the winding never has to be got right for culling.
                    cull_mode: None,
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: crate::surface::DEPTH_FORMAT,
                    // Opaque: hidden by the walls in front of them, hiding
                    // each other and what is behind.
                    depth_write_enabled: Some(true),
                    depth_compare: Some(wgpu::CompareFunction::Less),
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                }),
                multisample: wgpu::MultisampleState::default(),
                multiview_mask: None,
                cache: None,
            };
            Pipelines {
                pipeline: device.cached_render_pipeline(&desc),
                pipeline_msaa: crate::msaa::variant(device, &desc, msaa_supported),
                probes_bgl,
            }
        })
    }

    /// Draws `count` probes from `probe_buffer` into the scene's colour and
    /// depth, after everything else in the scene and before post-processing
    /// -- so the spheres are tonemapped, and hidden by geometry, as the scene
    /// is. Returns the triangles drawn.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn draw(
        &mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        scene_color: &wgpu::TextureView,
        scene_resolve: Option<&wgpu::TextureView>,
        scene_depth: &wgpu::TextureView,
        msaa_on: bool,
        camera_bg: &wgpu::BindGroup,
        probe_buffer: &wgpu::Buffer,
        count: u32,
    ) -> u64 {
        let pipelines = self.pipelines(device);
        // Made each frame it is drawn, not kept: the probe buffer is replaced
        // when an adaptive volume grows it, and a kept group would go on
        // binding the old one.
        let probes_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("probe display bg"),
            layout: &pipelines.probes_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: probe_buffer.as_entire_binding(),
            }],
        });
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("probe display pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: scene_color,
                resolve_target: scene_resolve,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: scene_depth,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(
            pipelines
                .pipeline_msaa
                .as_ref()
                .filter(|_| msaa_on)
                .unwrap_or(&pipelines.pipeline),
        );
        pass.set_bind_group(0, camera_bg, &[]);
        pass.set_bind_group(1, &probes_bg, &[]);
        pass.draw(0..VERTICES_PER_PROBE, 0..count);
        u64::from(count) * u64::from(VERTICES_PER_PROBE / 3)
    }
}

/// The pass's shader: the probe grid's own structure and lookups, then where
/// each probe is and how its SH lights a sphere.
fn probe_display_wgsl() -> String {
    format!(
        "{}{}const SEGMENTS: u32 = {SEGMENTS}u;\nconst RINGS: u32 = {RINGS}u;\n{}",
        r#"
const PI: f32 = 3.141592653589793;
// The scene's camera uniform, as far as this pass reads it: the same prefix
// the particle shader declares.
struct CameraUniform {
    view_proj: mat4x4<f32>,
    cascade_view_proj: array<mat4x4<f32>, 4>,
    cam_pos: vec3<f32>,
};
@group(0) @binding(0) var<uniform> camera: CameraUniform;
@group(1) @binding(0) var<storage, read> probes: ProbeGrid;
"#,
        crate::realtime_gi::probe_grid_wgsl!(),
        r#"
// A sphere's radius, as a fraction of its probe's spacing: small enough that
// neighbours a spacing apart stay apart, large enough to read its shading.
const RADIUS: f32 = 0.15;

// Where probe `i` is, as shading takes it: an adaptive volume's from its
// table, a grid's on its lattice point (x fastest, then y, then z -- the order
// `probe_cell_index` reads them in); moved by its relocation offset when the
// volume has them (header bit 0, as `eval_probe_sh_at` reads them).
fn display_probe_position(i: u32) -> vec3<f32> {
    var p: vec3<f32>;
    if probes_adaptive() {
        p = brick_probe_position(i);
    } else {
        let r = probes.resolution;
        let c = vec3<u32>(i % r.x, (i / r.x) % r.y, i / (r.x * r.y));
        p = probes.origin + display_grid_spacing() * vec3<f32>(c);
    }
    if (probes.flags & 1u) != 0u {
        p += probe_offset(min(i, last_probe()), probes.coeff_offset).xyz;
    }
    return p;
}
// A grid's lattice spacing; an axis with one probe has none, and is left out
// of the radius below.
fn display_grid_spacing() -> vec3<f32> {
    let cells = max(vec3<f32>(probes.resolution) - vec3<f32>(1.0), vec3<f32>(1.0));
    return probes.extent / cells;
}
fn display_probe_radius(i: u32) -> f32 {
    var s: vec3<f32>;
    if probes_adaptive() {
        s = brick_probe_spacing(i);
    } else {
        s = select(vec3<f32>(1e30), display_grid_spacing(), probes.resolution > vec3<u32>(1u));
    }
    let m = min(min(s.x, s.y), s.z);
    // A one-probe grid has no spacing at all: a tenth of the box, then.
    return RADIUS * select(m, 0.1 * length(probes.extent), m >= 1e29);
}

struct VertOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) @interpolate(flat) probe: u32,
};

@vertex
fn vs_probe(@builtin(vertex_index) vi: u32, @builtin(instance_index) probe: u32) -> VertOut {
    // Two triangles per segment of each ring, their corners from the index.
    var du = array<u32, 6>(0u, 1u, 0u, 0u, 1u, 1u);
    var dv = array<u32, 6>(0u, 0u, 1u, 1u, 0u, 1u);
    let quad = vi / 6u;
    let k = vi % 6u;
    let theta = f32(quad / SEGMENTS + dv[k]) / f32(RINGS) * PI;
    let phi = f32(quad % SEGMENTS + du[k]) / f32(SEGMENTS) * 2.0 * PI;
    let n = vec3<f32>(sin(theta) * cos(phi), cos(theta), sin(theta) * sin(phi));
    let world = display_probe_position(probe) + n * display_probe_radius(probe);
    var out: VertOut;
    out.clip = camera.view_proj * vec4<f32>(world, 1.0);
    out.normal = n;
    out.probe = probe;
    return out;
}

// The probe's own SH, not the blend `eval_probe_sh_at` makes of eight: what
// this one probe holds, as the light a white Lambertian surface facing `n`
// would send back -- E / PI, the units shading multiplies albedo by, so a
// sphere is as bright as a white wall lit by that probe alone. The constants
// are `eval_probe_sh_at`'s (and `ShL2::eval_irradiance`'s, which asserts
// them).
@fragment
fn fs_probe(in: VertOut) -> @location(0) vec4<f32> {
    let n = normalize(in.normal);
    let base = (probes.coeff_offset + min(in.probe, last_probe())) * 9u;
    let a0 = 3.141593;
    let a1 = 2.094395;
    let a2 = 0.785398;
    var e = probes.coeffs[base].rgb * (0.282095 * a0);
    e = e + probes.coeffs[base + 1u].rgb * (0.488603 * n.y * a1);
    e = e + probes.coeffs[base + 2u].rgb * (0.488603 * n.z * a1);
    e = e + probes.coeffs[base + 3u].rgb * (0.488603 * n.x * a1);
    e = e + probes.coeffs[base + 4u].rgb * (1.092548 * n.x * n.y * a2);
    e = e + probes.coeffs[base + 5u].rgb * (1.092548 * n.y * n.z * a2);
    e = e + probes.coeffs[base + 6u].rgb * (0.315392 * (3.0 * n.z * n.z - 1.0) * a2);
    e = e + probes.coeffs[base + 7u].rgb * (1.092548 * n.x * n.z * a2);
    e = e + probes.coeffs[base + 8u].rgb * (0.546274 * (n.x * n.x - n.y * n.y) * a2);
    return vec4<f32>(max(e, vec3<f32>(0.0)) / PI, 1.0);
}
"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The shader parses and validates as naga sees it, without a device.
    #[test]
    fn the_display_shader_validates() {
        let module = wgpu::naga::front::wgsl::parse_str(&probe_display_wgsl())
            .unwrap_or_else(|e| panic!("{}", e.emit_to_string(&probe_display_wgsl())));
        wgpu::naga::valid::Validator::new(
            wgpu::naga::valid::ValidationFlags::all(),
            wgpu::naga::valid::Capabilities::all(),
        )
        .validate(&module)
        .expect("valid");
    }
}
