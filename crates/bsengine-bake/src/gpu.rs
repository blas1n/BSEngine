//! The GPU backend: the same bake, its texels traced by a compute shader.
//!
//! Everything but the texels' light is the CPU backend's own code -- packing,
//! rasterising, the BVH build, dilation (`crate::prepare` / `crate::finish`)
//! -- so the two compute the same texels at the same places. What runs on
//! the GPU is `gpu.wgsl`, the CPU tracer moved to WGSL step for step: the
//! same tree traversed the same way, the same light formulas, the same path.
//! Two things cannot be the same, and the result is therefore the same
//! *estimate*, not the same bits: WGSL has no 64-bit integers, so the random
//! numbers come from a 32-bit PCG hash rather than SplitMix64, and float
//! arithmetic is ordered differently. `CpuBaker` stays the reference; the
//! tests hold this one to it.
//!
//! # Fitting the WebGPU baseline
//!
//! Four storage buffers per stage is what `downlevel_defaults` guarantees,
//! and the bake uses exactly four (nodes, triangles, texel jobs, output): the
//! triangles are uploaded in the tree's leaf order, so a leaf names its
//! triangles directly and the index buffer the CPU tree keeps is not needed.
//! Lights go in a uniform, the sky in a texture.
//!
//! # Not one long dispatch -- and not a thin one either
//!
//! A single dispatch that runs for seconds is what Windows resets a GPU for
//! (TDR), so the work is split and each part submitted and waited for. It is
//! split by *samples*: every pass traces every texel (as many threads as there
//! are texels, which is what keeps the GPU busy) for a slice of the samples,
//! adding into a running sum. A first version split by texels instead, sizing
//! batches by rays: at 1024 samples a batch was 512 texels -- eight workgroups
//! -- and the GPU bake ran ten times slower than the CPU's, and four times
//! slower for twice the samples.

use crate::scene::{BakeLight, Contribution, Sky};
use crate::{BakeInput, BakeOutput, BakeSettings, LightmapBaker};
use glam::Vec3;
use std::sync::Arc;
use wgpu::util::DeviceExt;

/// Most lights one bake can carry: the uniform's fixed array.
pub const MAX_GPU_LIGHTS: usize = 64;

/// Rays aimed for per pass: texels x samples-in-pass x (bounces + 1) x (1 +
/// lights). Tens of millions is a fraction of a second on a desktop GPU and
/// keeps a software adapter (CI's lavapipe or WARP) under its timeouts.
const RAYS_PER_PASS: u64 = 1 << 25;
/// Most workgroups one dispatch may have along x (`maxComputeWorkgroupsPerDimension`).
const MAX_GROUPS: usize = 65535;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct GpuNode {
    lo: [f32; 3],
    start: u32,
    hi: [f32; 3],
    count: u32,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct GpuTri {
    p0: [f32; 4],
    p1: [f32; 4],
    p2: [f32; 4],
    n: [f32; 4],
    albedo: [f32; 4],
    emissive: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct GpuJob {
    pos: [f32; 3],
    layer: u32,
    normal: [f32; 3],
    xy: u32,
}

/// One light: `kind` 0 directional (`a` = direction, `c` = colour), 1 point
/// (`a` = position, `c` = colour), 2 spot (`a` position, `b` direction, `c`
/// colour, `d.x` inner cos, `d.y` outer cos), 3 rect (`a` centre, `b` half
/// width, `c` half height, `d` radiance).
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct GpuLight {
    kind: u32,
    contribution: u32,
    range: f32,
    intensity: f32,
    a: [f32; 4],
    b: [f32; 4],
    c: [f32; 4],
    d: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct GpuParams {
    total_samples: u32,
    sample_offset: u32,
    sample_count: u32,
    bounces: u32,
    seed: u32,
    job_offset: u32,
    job_count: u32,
    light_count: u32,
    bake_direct: u32,
    node_count: u32,
    sky_kind: u32,
    sky_w: u32,
    sky_h: u32,
    _p0: u32,
    _p1: u32,
    _p2: u32,
    sky_uniform: [f32; 4],
}

fn v4(v: Vec3) -> [f32; 4] {
    [v.x, v.y, v.z, 0.0]
}

fn gpu_light(light: &BakeLight) -> GpuLight {
    let contribution = u32::from(light.contribution() == Contribution::Baked);
    let zero = [0.0; 4];
    match *light {
        BakeLight::Directional {
            direction, color, ..
        } => GpuLight {
            kind: 0,
            contribution,
            range: 0.0,
            intensity: 0.0,
            a: v4(direction),
            b: zero,
            c: v4(color),
            d: zero,
        },
        BakeLight::Point {
            position,
            color,
            intensity,
            range,
            ..
        } => GpuLight {
            kind: 1,
            contribution,
            range,
            intensity,
            a: v4(position),
            b: zero,
            c: v4(color),
            d: zero,
        },
        BakeLight::Spot {
            position,
            direction,
            color,
            intensity,
            range,
            inner_cos,
            outer_cos,
            ..
        } => GpuLight {
            kind: 2,
            contribution,
            range,
            intensity,
            a: v4(position),
            b: v4(direction),
            c: v4(color),
            d: [inner_cos, outer_cos, 0.0, 0.0],
        },
        BakeLight::Rect {
            position,
            half_width,
            half_height,
            radiance,
            range,
            ..
        } => GpuLight {
            kind: 3,
            contribution,
            range,
            intensity: 0.0,
            a: v4(position),
            b: v4(half_width),
            c: v4(half_height),
            d: v4(radiance),
        },
    }
}

/// Whether an adapter of this type is real hardware, for
/// [`GpuBaker::new_hardware`]. Only `Cpu` is software; `Other` (an adapter
/// that does not say) is given the benefit of the doubt, since a backend that
/// cannot name its device is far more often a real one than a rasteriser.
pub fn is_hardware(device_type: wgpu::DeviceType) -> bool {
    device_type != wgpu::DeviceType::Cpu
}

/// The compute tracer, on one device.
pub struct GpuBaker {
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    pipeline: wgpu::ComputePipeline,
    layout: wgpu::BindGroupLayout,
}

impl GpuBaker {
    /// A baker on a device of its own: the first adapter the system offers
    /// (a high-performance one if there is a choice), a software one
    /// included. `Err` when there is none.
    pub fn new() -> Result<Self, String> {
        Self::open(true)
    }

    /// [`Self::new`], but `Err` when the only adapter is a software one --
    /// what `--backend auto` asks for. A software rasteriser (CI's lavapipe,
    /// Windows' WARP) is the CPU emulating a GPU, and [`crate::CpuBaker`] is
    /// the direct way to use those same cores. (Not measured against each
    /// other; Unity's GPU lightmapper likewise falls back to its CPU one when
    /// it finds no suitable GPU, rather than failing the bake.)
    pub fn new_hardware() -> Result<Self, String> {
        Self::open(false)
    }

    fn open(allow_software: bool) -> Result<Self, String> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::default());
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: None,
            force_fallback_adapter: false,
        }))
        .ok_or("no GPU adapter")?;
        let info = adapter.get_info();
        if !allow_software && !is_hardware(info.device_type) {
            return Err(format!(
                "the only adapter is a software one ({})",
                info.name
            ));
        }
        let have = adapter.limits();
        // The baseline, except for buffer sizes: a scene's triangles and
        // texels are as large as the scene, and the adapter knows its own.
        let limits = wgpu::Limits {
            max_storage_buffer_binding_size: have.max_storage_buffer_binding_size,
            max_buffer_size: have.max_buffer_size,
            ..wgpu::Limits::downlevel_defaults()
        };
        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("lightmap baker"),
                required_features: wgpu::Features::empty(),
                required_limits: limits,
                memory_hints: wgpu::MemoryHints::default(),
            },
            None,
        ))
        .map_err(|e| format!("cannot open the GPU: {e}"))?;
        Ok(Self::with_device(Arc::new(device), Arc::new(queue)))
    }

    /// A baker on a device the caller already has.
    pub fn with_device(device: Arc<wgpu::Device>, queue: Arc<wgpu::Queue>) -> Self {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("lightmap bake"),
            source: wgpu::ShaderSource::Wgsl(include_str!("gpu.wgsl").into()),
        });
        let storage = |binding: u32, read_only: bool| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage { read_only },
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };
        let uniform = |binding: u32| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("lightmap bake bgl"),
            entries: &[
                storage(0, true),
                storage(1, true),
                storage(2, true),
                storage(3, false),
                uniform(4),
                uniform(5),
                wgpu::BindGroupLayoutEntry {
                    binding: 6,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("lightmap bake layout"),
            bind_group_layouts: &[&layout],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("lightmap bake"),
            layout: Some(&pipeline_layout),
            module: &module,
            entry_point: "cs_bake",
            compilation_options: Default::default(),
            cache: None,
        });
        Self {
            device,
            queue,
            pipeline,
            layout,
        }
    }

    /// Every job's `E / PI`, traced on the GPU.
    fn trace(
        &self,
        prepared: &crate::Prepared,
        input: &BakeInput,
        settings: &BakeSettings,
    ) -> Vec<Vec3> {
        let jobs = &prepared.jobs;
        if jobs.is_empty() {
            return Vec::new();
        }
        // The shader's traversal stack is sized for `MAX_DEPTH`; the builder
        // promises no deeper, and this is where a broken promise would show.
        debug_assert!(prepared.bvh.depth() <= crate::bvh::MAX_DEPTH + 1);
        let (flat_nodes, order) = prepared.bvh.flat();
        // At least one element: a storage binding cannot be empty. The
        // shader reads `node_count`, so a placeholder is never traversed.
        let mut nodes: Vec<GpuNode> = flat_nodes
            .iter()
            .map(|&(lo, start, hi, count)| GpuNode {
                lo,
                start,
                hi,
                count,
            })
            .collect();
        let node_count = nodes.len() as u32;
        if nodes.is_empty() {
            nodes.push(bytemuck::Zeroable::zeroed());
        }
        let mut tris: Vec<GpuTri> = order
            .iter()
            .map(|&i| {
                let t = &input.triangles[i as usize];
                let [a, b, c] = t.positions;
                // `trace.rs`'s shading normal: the geometric one, turned to
                // the side the vertex normals face.
                let mut n = (b - a).cross(c - a).normalize_or_zero();
                if n.dot(t.normals[0] + t.normals[1] + t.normals[2]) < 0.0 {
                    n = -n;
                }
                GpuTri {
                    p0: v4(a),
                    p1: v4(b),
                    p2: v4(c),
                    n: v4(n),
                    albedo: v4(t.albedo),
                    emissive: v4(t.emissive),
                }
            })
            .collect();
        if tris.is_empty() {
            tris.push(bytemuck::Zeroable::zeroed());
        }
        let gpu_jobs: Vec<GpuJob> = jobs
            .iter()
            .map(|j| GpuJob {
                pos: j.position.to_array(),
                layer: j.layer,
                normal: j.normal.to_array(),
                xy: (j.x & 0xffff) | (j.y << 16),
            })
            .collect();
        if input.lights.len() > MAX_GPU_LIGHTS {
            tracing_warn(&format!(
                "the GPU bake takes the first {MAX_GPU_LIGHTS} of {} lights",
                input.lights.len()
            ));
        }
        let mut lights = [<GpuLight as bytemuck::Zeroable>::zeroed(); MAX_GPU_LIGHTS];
        for (slot, l) in lights.iter_mut().zip(input.lights) {
            *slot = gpu_light(l);
        }
        let light_count = input.lights.len().min(MAX_GPU_LIGHTS) as u32;

        let (sky_kind, sky_w, sky_h, sky_uniform, sky_texels) = match input.sky {
            Sky::Uniform(l) => (0u32, 1u32, 1u32, v4(*l), vec![[0.0f32; 4]]),
            Sky::Equirect {
                width,
                height,
                texels,
            } => (
                1,
                *width,
                *height,
                [0.0; 4],
                texels.iter().map(|t| [t.x, t.y, t.z, 1.0]).collect(),
            ),
        };
        let sky = self.device.create_texture_with_data(
            &self.queue,
            &wgpu::TextureDescriptor {
                label: Some("lightmap bake sky"),
                size: wgpu::Extent3d {
                    width: sky_w,
                    height: sky_h,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba32Float,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            bytemuck::cast_slice(&sky_texels),
        );
        let sky_view = sky.create_view(&Default::default());

        let buffer = |label, contents: &[u8], usage| {
            self.device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some(label),
                    contents,
                    usage,
                })
        };
        let storage = wgpu::BufferUsages::STORAGE;
        let nodes_buf = buffer("bake nodes", bytemuck::cast_slice(&nodes), storage);
        let tris_buf = buffer("bake triangles", bytemuck::cast_slice(&tris), storage);
        let jobs_buf = buffer("bake jobs", bytemuck::cast_slice(&gpu_jobs), storage);
        let lights_buf = buffer(
            "bake lights",
            bytemuck::cast_slice(&lights),
            wgpu::BufferUsages::UNIFORM,
        );
        let out_size = (jobs.len() * 16) as u64;
        let out_buf = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("bake out"),
            size: out_size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let params_buf = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("bake params"),
            size: std::mem::size_of::<GpuParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("lightmap bake bg"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: nodes_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: tris_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: jobs_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: out_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: params_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: lights_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: wgpu::BindingResource::TextureView(&sky_view),
                },
            ],
        });

        let trace = settings.trace;
        let total = trace.samples.max(1);
        let rays_per_sample =
            jobs.len() as u64 * u64::from(trace.bounces + 1) * (1 + u64::from(light_count));
        let per_pass = (RAYS_PER_PASS / rays_per_sample.max(1)).clamp(1, u64::from(total)) as u32;
        let chunk = MAX_GROUPS * 64;
        let mut sample_offset = 0u32;
        while sample_offset < total {
            let sample_count = per_pass.min(total - sample_offset);
            let mut job_offset = 0usize;
            while job_offset < jobs.len() {
                let count = chunk.min(jobs.len() - job_offset);
                let params = GpuParams {
                    total_samples: total,
                    sample_offset,
                    sample_count,
                    bounces: trace.bounces,
                    // The CPU's seed is 64-bit; both halves reach the hash.
                    seed: (trace.seed ^ (trace.seed >> 32)) as u32,
                    job_offset: job_offset as u32,
                    job_count: count as u32,
                    light_count,
                    bake_direct: u32::from(settings.bake_direct),
                    node_count,
                    sky_kind,
                    sky_w,
                    sky_h,
                    _p0: 0,
                    _p1: 0,
                    _p2: 0,
                    sky_uniform,
                };
                self.queue
                    .write_buffer(&params_buf, 0, bytemuck::bytes_of(&params));
                let mut encoder =
                    self.device
                        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                            label: Some("lightmap bake"),
                        });
                {
                    let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                        label: Some("lightmap bake"),
                        timestamp_writes: None,
                    });
                    pass.set_pipeline(&self.pipeline);
                    pass.set_bind_group(0, &bind_group, &[]);
                    pass.dispatch_workgroups((count as u32).div_ceil(64), 1, 1);
                }
                self.queue.submit(Some(encoder.finish()));
                // Waited for here, pass by pass: see the module docs.
                self.device.poll(wgpu::Maintain::Wait);
                job_offset += count;
            }
            sample_offset += sample_count;
        }

        let staging = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("bake readback"),
            size: out_size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("bake readback"),
            });
        encoder.copy_buffer_to_buffer(&out_buf, 0, &staging, 0, out_size);
        self.queue.submit(Some(encoder.finish()));
        let slice = staging.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        self.device.poll(wgpu::Maintain::Wait);
        rx.recv()
            .expect("map_async reported nothing")
            .expect("the bake output could not be read back");
        let values: Vec<[f32; 4]> = bytemuck::cast_slice(&slice.get_mapped_range()).to_vec();
        let n = total as f32;
        values
            .iter()
            .map(|v| Vec3::new(v[0], v[1], v[2]) / n)
            .collect()
    }
}

fn tracing_warn(message: &str) {
    eprintln!("warning: {message}");
}

impl LightmapBaker for GpuBaker {
    fn name(&self) -> &'static str {
        "gpu"
    }

    fn bake(&self, input: &BakeInput, settings: &BakeSettings) -> BakeOutput {
        let prepared = crate::prepare(input, settings);
        let values = self.trace(&prepared, input, settings);
        crate::finish(prepared, values, settings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BakeTriangle, CpuBaker, RendererInfo, TraceSettings};
    use glam::Vec2;
    use std::sync::OnceLock;

    /// One baker -- one device -- for every test: each `request_device`
    /// costs real VRAM, and a dozen of them fail Windows CI with
    /// `OutOfMemory` in whichever test asked last. Panics without an adapter:
    /// a skipped GPU test reads exactly like a passing one.
    fn baker() -> &'static GpuBaker {
        static BAKER: OnceLock<GpuBaker> = OnceLock::new();
        BAKER.get_or_init(|| GpuBaker::new().expect("these tests need a GPU adapter"))
    }

    /// A quad from `min` along `u` and `v`, facing `u x v`, its `uv2` the
    /// unit square.
    fn quad(renderer: u32, min: Vec3, u: Vec3, v: Vec3, albedo: Vec3) -> [BakeTriangle; 2] {
        let n = u.cross(v).normalize();
        let p = [min, min + u, min + u + v, min + v];
        let uv = [
            Vec2::new(0.0, 0.0),
            Vec2::new(1.0, 0.0),
            Vec2::new(1.0, 1.0),
            Vec2::new(0.0, 1.0),
        ];
        let tri = |a: usize, b: usize, c: usize| BakeTriangle {
            positions: [p[a], p[b], p[c]],
            normals: [n; 3],
            uv2: [uv[a], uv[b], uv[c]],
            albedo,
            emissive: Vec3::ZERO,
            renderer,
        };
        [tri(0, 1, 2), tri(0, 2, 3)]
    }

    /// A unit floor at the origin facing +Y.
    fn floor(renderer: u32, albedo: Vec3) -> [BakeTriangle; 2] {
        quad(
            renderer,
            Vec3::new(0.0, 0.0, 1.0),
            Vec3::X,
            Vec3::NEG_Z,
            albedo,
        )
    }

    fn settings(samples: u32, bounces: u32, bake_direct: bool) -> BakeSettings {
        BakeSettings {
            trace: TraceSettings {
                samples,
                bounces,
                seed: 3,
            },
            texels_per_unit: 8.0,
            layer_size: 64,
            max_layers: 4,
            bake_direct,
        }
    }

    fn renderers(n: usize) -> Vec<RendererInfo> {
        vec![
            RendererInfo {
                resolution_scale: 1.0
            };
            n
        ]
    }

    /// Every texel the renderer's rectangle covers, from a bake.
    fn texels_of(out: &BakeOutput, renderer: usize) -> Vec<Vec3> {
        let p = out.placements[renderer];
        let size = out.layer_size;
        (p.y..p.y + p.side)
            .flat_map(|y| (p.x..p.x + p.side).map(move |x| (x, y)))
            .map(|(x, y)| out.layers[p.layer as usize][(y * size + x) as usize])
            .collect()
    }

    /// Closed form, exactly: an open floor under a uniform sky stores the
    /// sky, every sample alike -- so a traversal that hit something it should
    /// not, or an estimate that is not a plain mean, shows.
    #[test]
    fn an_open_floor_under_a_uniform_sky_stores_the_sky() {
        let tris = floor(0, Vec3::splat(0.5));
        let sky = Sky::Uniform(Vec3::new(0.25, 0.5, 0.75));
        let out = baker().bake(
            &BakeInput {
                triangles: &tris,
                renderers: &renderers(1),
                lights: &[],
                sky: &sky,
            },
            &settings(16, 2, false),
        );
        let texels = texels_of(&out, 0);
        assert_eq!(texels.len(), 64, "premise: 1 m2 at 8 texels per unit");
        for t in texels {
            assert!((t - Vec3::new(0.25, 0.5, 0.75)).length() < 1e-5, "{t}");
        }
    }

    /// Closed form: a tall black wall at the floor's edge hides half the
    /// cosine-weighted sky from the texels beside it. The wall is 0.01 off
    /// the floor's +X edge; the column of texels nearest it reads about 0.5.
    #[test]
    fn a_black_wall_beside_the_floor_hides_half_the_sky() {
        let mut tris = floor(0, Vec3::splat(0.5)).to_vec();
        // A 200 x 200 wall in x = 1.01, facing -X.
        tris.extend(quad(
            1,
            Vec3::new(1.01, -100.0, 100.0),
            Vec3::new(0.0, 200.0, 0.0),
            Vec3::new(0.0, 0.0, -200.0),
            Vec3::ZERO,
        ));
        let sky = Sky::Uniform(Vec3::ONE);
        let out = baker().bake(
            &BakeInput {
                triangles: &tris,
                renderers: &renderers(2),
                lights: &[],
                sky: &sky,
            },
            &settings(4096, 1, false),
        );
        let p = out.placements[0];
        let size = out.layer_size;
        // The floor's +X edge is uv2.x = 1: its rectangle's last column.
        let x = p.x + p.side - 1;
        let edge: Vec<f32> = (p.y..p.y + p.side)
            .map(|y| out.layers[p.layer as usize][(y * size + x) as usize].x)
            .collect();
        let mean = edge.iter().sum::<f32>() / edge.len() as f32;
        // Half, to well within the noise: 4096 samples give each texel a
        // standard error near 0.008, and the wall's finite height and the half
        // texel to its foot move the answer by far less. (A first version
        // asked for 0.5..0.6, reasoning that the texel sits a little away from
        // the wall -- an effect of 0.0004, under the noise, and the GPU's
        // 0.4965 was within one and a half standard errors of 0.5.)
        assert!(
            (mean - 0.5).abs() < 0.02,
            "about half the sky: {mean} {edge:?}"
        );
    }

    /// The direct term has no noise: a `Baked` sun on an open floor under a
    /// black sky stores exactly its E / pi, and an `Indirect` one stores
    /// nothing -- on both backends, to the same digits.
    #[test]
    fn a_baked_suns_direct_light_matches_the_cpu_exactly() {
        let tris = floor(0, Vec3::splat(0.5));
        let sky = Sky::Uniform(Vec3::ZERO);
        let sun = |contribution| {
            [BakeLight::Directional {
                direction: Vec3::new(0.3, -1.0, 0.2).normalize(),
                color: Vec3::splat(2.0),
                contribution,
            }]
        };
        let bake = |baker: &dyn LightmapBaker, lights: &[BakeLight]| {
            texels_of(
                &baker.bake(
                    &BakeInput {
                        triangles: &tris,
                        renderers: &renderers(1),
                        lights,
                        sky: &sky,
                    },
                    &settings(4, 1, true),
                ),
                0,
            )
        };
        let want = 2.0 * Vec3::new(0.3, -1.0, 0.2).normalize().y.abs() / std::f32::consts::PI;
        for t in bake(baker(), &sun(Contribution::Baked)) {
            assert!((t.x - want).abs() < 1e-5, "{t} vs {want}");
        }
        for t in bake(baker(), &sun(Contribution::Indirect)) {
            assert_eq!(
                t,
                Vec3::ZERO,
                "an Indirect light's direct light is not baked"
            );
        }
        let cpu = bake(&CpuBaker, &sun(Contribution::Baked));
        let gpu = bake(baker(), &sun(Contribution::Baked));
        for (c, g) in cpu.iter().zip(&gpu) {
            assert!((*c - *g).length() < 1e-5, "{c} vs {g}");
        }
    }

    /// The reference: a room of a red wall, a white wall and a floor, lit by
    /// an indirect sun, a baked point light, a baked spot and a baked rect
    /// panel under a blue sky -- every light kind, bounces, occlusion. At
    /// enough samples that Monte Carlo noise is a few percent, the floor's
    /// mean agrees within 3% and 95% of its texels within 15%. A GPU path
    /// that dropped a bounce, a light kind or a shadow would miss by far more.
    #[test]
    fn the_gpu_bake_agrees_with_the_cpu_reference() {
        let mut tris = floor(0, Vec3::splat(0.8)).to_vec();
        tris.extend(quad(
            1,
            Vec3::new(1.0, 0.0, 1.0),
            Vec3::new(0.0, 0.0, -1.0),
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(0.9, 0.1, 0.1),
        ));
        tris.extend(quad(
            2,
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::splat(0.8),
        ));
        let lights = [
            BakeLight::Directional {
                direction: Vec3::new(0.4, -1.0, -0.3).normalize(),
                color: Vec3::splat(1.5),
                contribution: Contribution::Indirect,
            },
            BakeLight::Point {
                position: Vec3::new(0.5, 0.8, 0.5),
                color: Vec3::new(1.0, 0.9, 0.7),
                intensity: 2.0,
                range: 3.0,
                contribution: Contribution::Baked,
            },
            BakeLight::Spot {
                position: Vec3::new(0.2, 0.9, 0.8),
                direction: Vec3::new(0.2, -1.0, -0.2).normalize(),
                color: Vec3::ONE,
                intensity: 3.0,
                range: 4.0,
                inner_cos: 25f32.to_radians().cos(),
                outer_cos: 35f32.to_radians().cos(),
                contribution: Contribution::Baked,
            },
            BakeLight::Rect {
                position: Vec3::new(0.5, 1.2, 0.5),
                half_width: Vec3::X * 0.2,
                half_height: Vec3::NEG_Z * 0.2,
                radiance: Vec3::splat(4.0),
                range: 5.0,
                contribution: Contribution::Baked,
            },
        ];
        let sky = Sky::Uniform(Vec3::new(0.1, 0.15, 0.3));
        let bake = |baker: &dyn LightmapBaker| {
            texels_of(
                &baker.bake(
                    &BakeInput {
                        triangles: &tris,
                        renderers: &renderers(3),
                        lights: &lights,
                        sky: &sky,
                    },
                    &settings(512, 3, true),
                ),
                0,
            )
        };
        let cpu = bake(&CpuBaker);
        let gpu = bake(baker());
        let mean = |v: &[Vec3]| v.iter().map(|t| t.x + t.y + t.z).sum::<f32>() / v.len() as f32;
        let (mc, mg) = (mean(&cpu), mean(&gpu));
        assert!(mc > 0.05, "premise: the floor is lit: {mc}");
        assert!((mg / mc - 1.0).abs() < 0.03, "means: cpu {mc}, gpu {mg}");
        let mut errors: Vec<f32> = cpu
            .iter()
            .zip(&gpu)
            .map(|(c, g)| (*c - *g).length() / c.length().max(1e-3))
            .collect();
        errors.sort_by(f32::total_cmp);
        let p95 = errors[errors.len() * 95 / 100];
        assert!(p95 < 0.15, "95th percentile per-texel error {p95}");
    }

    /// Six inward-facing quads of a unit box around the origin's cell, every
    /// one with albedo `rho` and emission `e`.
    fn furnace(rho: f32, e: f32) -> Vec<BakeTriangle> {
        let mut tris = Vec::new();
        let faces = [
            // floor (+Y), ceiling (-Y), and the four walls, all facing in.
            (Vec3::new(0.0, 0.0, 1.0), Vec3::X, Vec3::NEG_Z),
            (Vec3::new(0.0, 1.0, 0.0), Vec3::X, Vec3::Z),
            (Vec3::new(0.0, 0.0, 0.0), Vec3::Y, Vec3::Z),
            (Vec3::new(1.0, 0.0, 1.0), Vec3::Y, Vec3::NEG_Z),
            (Vec3::new(0.0, 0.0, 0.0), Vec3::X, Vec3::Y),
            (Vec3::new(1.0, 0.0, 1.0), Vec3::NEG_X, Vec3::Y),
        ];
        for (i, (min, u, v)) in faces.into_iter().enumerate() {
            let mut q = quad(i as u32, min, u, v, Vec3::splat(rho));
            for t in &mut q {
                t.emissive = Vec3::splat(e);
            }
            tris.extend(q);
        }
        tris
    }

    /// Closed form, exactly -- the furnace test: inside a closed box whose
    /// every surface emits `e` with albedo `rho`, every path sees the same
    /// thing, so a texel stores `e * (1 + rho + ... + rho^(B-1))` with no
    /// noise at all. Pins the path's throughput and its bounce count, which
    /// the reference scene is too open to resolve.
    #[test]
    fn the_furnace_stores_the_geometric_series_on_both_backends() {
        let (rho, e, bounces) = (0.5f32, 0.4f32, 3u32);
        let tris = furnace(rho, e);
        let sky = Sky::Uniform(Vec3::splat(100.0));
        let want = e * (1.0 - rho.powi(bounces as i32)) / (1.0 - rho);
        for baker in [baker() as &dyn LightmapBaker, &CpuBaker] {
            let out = baker.bake(
                &BakeInput {
                    triangles: &tris,
                    renderers: &renderers(6),
                    lights: &[],
                    sky: &sky,
                },
                &settings(8, bounces, false),
            );
            for t in texels_of(&out, 0) {
                assert!(
                    (t.x - want).abs() < 1e-4,
                    "{}: {} vs {want} (the sky, 100, would show any leak)",
                    baker.name(),
                    t.x
                );
            }
        }
    }

    /// Direct light has no noise, so its shadow is exact: a plank over the
    /// floor's middle third, a `Baked` sun straight down. Under the plank the
    /// floor stores 0, beside it E / pi, and the GPU agrees with the CPU texel
    /// for texel. The shadow ray is the only thing that tells the two halves
    /// apart.
    #[test]
    fn a_baked_lights_shadow_is_exact_and_matches_the_cpu() {
        let mut tris = floor(0, Vec3::splat(0.5)).to_vec();
        // A plank 0.5 up, over x in [1/3, 2/3], facing up.
        tris.extend(quad(
            1,
            Vec3::new(1.0 / 3.0, 0.5, 1.0),
            Vec3::X / 3.0,
            Vec3::NEG_Z,
            Vec3::splat(0.5),
        ));
        let sky = Sky::Uniform(Vec3::ZERO);
        let lights = [BakeLight::Directional {
            direction: Vec3::NEG_Y,
            color: Vec3::ONE,
            contribution: Contribution::Baked,
        }];
        let bake = |baker: &dyn LightmapBaker| {
            baker.bake(
                &BakeInput {
                    triangles: &tris,
                    renderers: &renderers(2),
                    lights: &lights,
                    sky: &sky,
                },
                // No bounces of interest: the sky is black and the plank's
                // top is what the sun lights, which the floor cannot see.
                &settings(4, 1, true),
            )
        };
        let gpu = bake(baker());
        let cpu = bake(&CpuBaker);
        let p = gpu.placements[0];
        let size = gpu.layer_size;
        let at = |o: &BakeOutput, x: u32| {
            o.layers[p.layer as usize][((p.y + 3) * size + p.x + x) as usize]
        };
        let lit = 1.0 / std::f32::consts::PI;
        assert!(
            (at(&gpu, 0).x - lit).abs() < 1e-5,
            "beside the plank: {}",
            at(&gpu, 0)
        );
        assert_eq!(at(&gpu, 4), Vec3::ZERO, "under the plank: shadowed");
        for x in 0..p.side {
            assert!((at(&gpu, x) - at(&cpu, x)).length() < 1e-5, "x = {x}");
        }
    }

    /// A surface's back is inside something and gives no light: a floor under
    /// a plank that faces *up*, under a black sky, with the sun lighting the
    /// plank's top. Every ray the floor sends either leaves into the black sky
    /// or meets the plank from behind -- so it stores 0, on the CPU and the
    /// GPU alike. Shading a back face as if it were a front lights it.
    #[test]
    fn a_back_face_gives_no_light() {
        let mut tris = floor(0, Vec3::splat(0.5)).to_vec();
        tris.extend(quad(
            1,
            Vec3::new(-2.0, 0.5, 3.0),
            Vec3::X * 5.0,
            Vec3::NEG_Z * 5.0,
            Vec3::splat(0.9),
        ));
        let sky = Sky::Uniform(Vec3::ZERO);
        let lights = [BakeLight::Directional {
            direction: Vec3::NEG_Y,
            color: Vec3::splat(3.0),
            contribution: Contribution::Indirect,
        }];
        for baker in [baker() as &dyn LightmapBaker, &CpuBaker] {
            let out = baker.bake(
                &BakeInput {
                    triangles: &tris,
                    renderers: &renderers(2),
                    lights: &lights,
                    sky: &sky,
                },
                &settings(64, 2, false),
            );
            for t in texels_of(&out, 0) {
                assert_eq!(t, Vec3::ZERO, "{}", baker.name());
            }
        }
    }

    /// Not a test: the CPU and GPU backends' times on a mid-sized scene, for
    /// the PR. `cargo test -p bsengine-bake --release -- --ignored
    /// --nocapture bench` -- release, because a debug build's times say
    /// nothing about either backend.
    #[test]
    #[ignore]
    fn bench_cpu_against_gpu() {
        let mut tris = Vec::new();
        let mut renderers_n = 0;
        // A 10 x 10 floor and `BENCH_SIDE`^2 half-metre boxes on it (8 x 8 by
        // default), five faces each.
        tris.extend(quad(
            0,
            Vec3::new(-5.0, 0.0, 5.0),
            Vec3::X * 10.0,
            Vec3::NEG_Z * 10.0,
            Vec3::splat(0.7),
        ));
        renderers_n += 1;
        let side = std::env::var("BENCH_SIDE")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(8u32);
        let spacing = 9.6 / side as f32;
        for i in 0..side * side {
            let (x, z) = (
                (i % side) as f32 * spacing - 4.8,
                (i / side) as f32 * spacing - 4.8,
            );
            let o = Vec3::new(x, 0.0, z);
            for (min, u, v) in [
                (
                    o + Vec3::new(0.0, 0.5, 0.5),
                    Vec3::X * 0.5,
                    Vec3::NEG_Z * 0.5,
                ),
                (o + Vec3::new(0.0, 0.0, 0.5), Vec3::X * 0.5, Vec3::Y * 0.5),
                (
                    o + Vec3::new(0.5, 0.0, 0.0),
                    Vec3::NEG_X * 0.5,
                    Vec3::Y * 0.5,
                ),
                (o + Vec3::new(0.0, 0.0, 0.0), Vec3::Z * 0.5, Vec3::Y * 0.5),
                (
                    o + Vec3::new(0.5, 0.0, 0.5),
                    Vec3::NEG_Z * 0.5,
                    Vec3::Y * 0.5,
                ),
            ] {
                tris.extend(quad(renderers_n, min, u, v, Vec3::splat(0.6)));
                renderers_n += 1;
            }
        }
        let lights = [BakeLight::Directional {
            direction: Vec3::new(0.3, -1.0, 0.2).normalize(),
            color: Vec3::splat(2.0),
            contribution: Contribution::Baked,
        }];
        let sky = Sky::Uniform(Vec3::new(0.2, 0.3, 0.5));
        let s = BakeSettings {
            trace: TraceSettings {
                samples: std::env::var("BENCH_SAMPLES")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(128),
                bounces: 3,
                seed: 0,
            },
            texels_per_unit: 16.0,
            layer_size: 1024,
            max_layers: 4,
            bake_direct: true,
        };
        let input = BakeInput {
            triangles: &tris,
            renderers: &renderers(renderers_n as usize),
            lights: &lights,
            sky: &sky,
        };
        let texels = crate::prepare(&input, &s).jobs.len();
        let _warm = baker();
        for b in [&CpuBaker as &dyn LightmapBaker, baker()] {
            let t = std::time::Instant::now();
            b.bake(&input, &s);
            eprintln!(
                "BENCH {}: {} triangles, {texels} texels, {} samples, 3 bounces: {:.2}s",
                b.name(),
                tris.len(),
                s.trace.samples,
                t.elapsed().as_secs_f32()
            );
        }
    }

    /// `auto` refuses only a software rasteriser: every real kind of GPU, and
    /// one that does not say what it is, counts as hardware.
    #[test]
    fn only_a_cpu_adapter_is_software() {
        use wgpu::DeviceType::*;
        assert!(!is_hardware(Cpu));
        for t in [DiscreteGpu, IntegratedGpu, VirtualGpu, Other] {
            assert!(is_hardware(t), "{t:?}");
        }
    }

    /// The same bake twice is the same bytes: every texel seeds from where it
    /// is, and nothing races.
    #[test]
    fn a_gpu_bake_is_deterministic() {
        let mut tris = floor(0, Vec3::splat(0.8)).to_vec();
        tris.extend(quad(
            1,
            Vec3::new(0.0, 0.5, 1.0),
            Vec3::X,
            Vec3::NEG_Z,
            Vec3::splat(0.8),
        ));
        let sky = Sky::Uniform(Vec3::ONE);
        let bake = || {
            baker().bake(
                &BakeInput {
                    triangles: &tris,
                    renderers: &renderers(2),
                    lights: &[],
                    sky: &sky,
                },
                &settings(32, 2, false),
            )
        };
        let bits = |o: &BakeOutput| -> Vec<u32> {
            o.layers
                .iter()
                .flatten()
                .flat_map(|v| v.to_array().map(f32::to_bits))
                .collect()
        };
        assert_eq!(bits(&bake()), bits(&bake()));
    }
}
