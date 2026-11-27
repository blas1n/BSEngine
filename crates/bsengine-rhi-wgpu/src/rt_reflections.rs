//! Ray-traced reflections (`bsengine_core::RayTracedReflections`): the share
//! of each pixel's reflection the screen-space trace did not find, traced
//! through the realtime GI's scene.
//!
//! # Where it sits
//!
//! The screen-space trace (`post_process::trace_ssr`) writes its reflection
//! premultiplied -- light times weight, and the weight -- into the target the
//! opaque pass reads, before that pass. This pass runs right after it, over
//! the same prepass depth and normals, and draws *under* it: blended as
//! `src * (1 - dst.a) + dst`, so a ray answers only the share the screen left,
//! and the target still holds one premultiplied reflection the mesh shader
//! reads as it always has. Nothing about the mesh shader changes -- which
//! matters, because its sampled-texture budget (16) is spent. HDRP's "Mixed"
//! mode and Lumen's reflections order the two the same way: the screen
//! first, rays for what it missed.
//!
//! What a ray finds is shaded as the probes' own rays shade it
//! (`realtime_gi`'s `trace_ray`): the material colour the GI keeps for the
//! surface, lit by every light with its shadow ray, plus the probes' bounce
//! -- the same tree, the same instances, the same formulas, so a reflection
//! and the GI cannot disagree about the scene. A ray that finds nothing, or
//! finds the back of something, answers nothing: the share stays with the
//! reflection probes and the sky (and, inside a probe volume, the probes'
//! stand-in for the sky), exactly as for a pixel the screen missed.
//!
//! # What it needs
//!
//! The GI's scene, so a realtime probe volume that has updated at least once.
//! Without one the camera keeps its screen-space reflections alone, and the
//! renderer says so once.
//!
//! The pipeline is built on a thread, as the GI's are: its shader walks the
//! same tree, and FXC takes seconds over that (a minute and more on CI's
//! software D3D12). Until it is built, frames reflect as if it were off.

use std::sync::{Arc, Mutex};

/// The pass's own parameters: `RtParams` in [`RT_WGSL`].
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct RtParams {
    /// Unjittered, as the screen-space trace's: the prepass depth is.
    pub inv_view_proj: [[f32; 4]; 4],
    /// xyz the camera, w the intensity.
    pub cam_pos: [f32; 4],
    pub max_roughness: f32,
    pub max_distance: f32,
    pub instance_count: u32,
    pub _pad: u32,
}

/// The pass: a fullscreen triangle whose fragments trace the reflection.
/// Compiled after `TRACE_WGSL`, the probe grid and `INSTANCES_WGSL`.
const RT_WGSL: &str = r#"
struct RtParams {
    inv_view_proj: mat4x4<f32>,
    cam_pos: vec4<f32>,
    max_roughness: f32,
    max_distance: f32,
    instance_count: u32,
    _pad: u32,
};

// The GI's scene, as its update binds it -- but the probes read-only, and
// every one of them in the fragment stage: four storage buffers, WebGPU's
// downlevel limit per stage exactly.
@group(0) @binding(0) var<storage, read> nodes: array<Node>;
@group(0) @binding(1) var<storage, read> tris: array<Tri>;
@group(0) @binding(2) var<storage, read> probes: ProbeGrid;
@group(0) @binding(3) var<uniform> scene: Scene;
@group(0) @binding(4) var sky: texture_2d<f32>;
@group(0) @binding(5) var<uniform> rt: RtParams;
@group(0) @binding(6) var<storage, read> instances: array<GiInstance>;
// The prepass's depth, and its normals and roughness: the screen-space
// trace's own bind groups.
@group(1) @binding(0) var depth_tex: texture_2d<f32>;
@group(2) @binding(0) var normal_tex: texture_2d<f32>;

fn traced_instance_count() -> u32 {
    return rt.instance_count;
}

@vertex
fn vs_fullscreen(@builtin(vertex_index) vi: u32) -> @builtin(position) vec4<f32> {
    var p = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0),
    );
    return vec4<f32>(p[vi], 0.0, 1.0);
}

@fragment
fn fs_rt(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    let coord = vec2<i32>(frag.xy);
    let dims = vec2<f32>(textureDimensions(depth_tex, 0));
    let depth = textureLoad(depth_tex, coord, 0).x;
    let nr = textureLoad(normal_tex, coord, 0);
    // The normal target clears to zero, which is no unit vector: nothing
    // opaque was drawn here.
    if depth >= 1.0 || dot(nr.xyz, nr.xyz) < 0.25 {
        return vec4<f32>(0.0);
    }
    // Up to `max_roughness`, all of it; gone 0.2 later -- the band where the
    // mesh shader hands the probes' stand-in from the sky's reflection to
    // the probes' own light, so the two fade across the same roughness.
    let share = clamp(1.0 - (nr.w - rt.max_roughness) / 0.2, 0.0, 1.0) * rt.cam_pos.w;
    if share <= 0.0 {
        return vec4<f32>(0.0);
    }
    let uv = frag.xy / dims;
    let clip = rt.inv_view_proj * vec4<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, depth, 1.0);
    let world = clip.xyz / clip.w;
    let n = normalize(nr.xyz);
    let to_surface = world - rt.cam_pos.xyz;
    let r = reflect(normalize(to_surface), n);
    // Off the surface by more than its depth's error, which grows with
    // distance: a ray from just below it would hit the surface itself.
    let o = world + n * (EPS + 0.002 * length(to_surface));
    let hit = traverse(o, r, rt.max_distance, false);
    if hit.tri == NO_HIT {
        return vec4<f32>(0.0);
    }
    let surface = hit_surface(hit);
    // The back of something: the ray started inside it, or the scene is
    // open there (a one-sided wall seen from behind). Neither is a colour to
    // reflect.
    if dot(surface.n, r) >= 0.0 {
        return vec4<f32>(0.0);
    }
    let y = o + r * hit.t + surface.n * EPS;
    let e = all_lights(y, surface.n) / PI + eval_probe_sh(y, surface.n);
    let light = surface.emissive + surface.albedo * e;
    return vec4<f32>(light * share, share);
}
"#;

enum State {
    #[cfg(not(target_arch = "wasm32"))]
    Compiling(std::thread::JoinHandle<wgpu::RenderPipeline>),
    Ready(Arc<wgpu::RenderPipeline>),
    /// The build panicked: logged once, and reflections stay screen-space.
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    Failed,
}

/// One device's layout and pipeline: built once per device, as the GI's
/// are, so every surface on it -- every test sharing the test device --
/// pays for the build once.
struct Shared {
    device: Arc<wgpu::Device>,
    layout: Arc<wgpu::BindGroupLayout>,
    state: Arc<Mutex<State>>,
}

static SHARED: Mutex<Vec<Shared>> = Mutex::new(Vec::new());

fn build_pipeline(
    device: &wgpu::Device,
    scene: &wgpu::BindGroupLayout,
    depth: &wgpu::BindGroupLayout,
    normals: &wgpu::BindGroupLayout,
) -> wgpu::RenderPipeline {
    let source = format!(
        "{}{}{}{}",
        bsengine_bake::gpu_scene::TRACE_WGSL,
        crate::realtime_gi::probe_grid_wgsl!(),
        crate::realtime_gi::INSTANCES_WGSL,
        RT_WGSL
    );
    // Without naga's loop bounding, as the GI's update: the tree walk ends on
    // its own, and the bound is what DXC miscompiled the update's walk with
    // (`realtime_gi::build_pipelines`).
    let module = unsafe {
        crate::pipeline_cache::create_shader_module_trusted(
            device,
            wgpu::ShaderModuleDescriptor {
                label: Some("rt reflections"),
                source: wgpu::ShaderSource::Wgsl(source.into()),
            },
            wgpu::ShaderRuntimeChecks {
                force_loop_bounding: false,
                ..wgpu::ShaderRuntimeChecks::checked()
            },
        )
    };
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("rt reflections layout"),
        bind_group_layouts: &[Some(scene), Some(depth), Some(normals)],
        immediate_size: 0,
    });
    // Under what the screen found: the ray's light fills the share it left.
    let under = wgpu::BlendComponent {
        src_factor: wgpu::BlendFactor::OneMinusDstAlpha,
        dst_factor: wgpu::BlendFactor::One,
        operation: wgpu::BlendOperation::Add,
    };
    crate::pipeline_cache::create_render_pipeline(
        device,
        &wgpu::RenderPipelineDescriptor {
            label: Some("rt reflections pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs_fullscreen"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs_rt"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: crate::post_process::HDR_FORMAT,
                    blend: Some(wgpu::BlendState {
                        color: under,
                        alpha: under,
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
        },
    )
}

/// `device`'s layout and pipeline state, starting the build on the first ask.
fn shared(
    device: &Arc<wgpu::Device>,
    depth: Arc<wgpu::BindGroupLayout>,
    normals: Arc<wgpu::BindGroupLayout>,
) -> (Arc<wgpu::BindGroupLayout>, Arc<Mutex<State>>) {
    use bsengine_bake::gpu_scene::{sky_layout_entry, storage_layout_entry, uniform_layout_entry};
    let mut all = SHARED.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(s) = all.iter().find(|s| Arc::ptr_eq(&s.device, device)) {
        return (Arc::clone(&s.layout), Arc::clone(&s.state));
    }
    let fragment = wgpu::ShaderStages::FRAGMENT;
    let layout = Arc::new(
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("rt reflections bgl"),
            entries: &[
                storage_layout_entry(0, fragment, true),
                storage_layout_entry(1, fragment, true),
                storage_layout_entry(2, fragment, true),
                uniform_layout_entry(3, fragment),
                sky_layout_entry(4, fragment),
                uniform_layout_entry(5, fragment),
                storage_layout_entry(6, fragment, true),
            ],
        }),
    );
    #[cfg(not(target_arch = "wasm32"))]
    let state = {
        let (device, layout) = (Arc::clone(device), Arc::clone(&layout));
        State::Compiling(
            std::thread::Builder::new()
                .name("rt reflections pipeline".into())
                .spawn(move || build_pipeline(&device, &layout, &depth, &normals))
                .expect("a thread for the ray-traced reflections' pipeline"),
        )
    };
    #[cfg(target_arch = "wasm32")]
    let state = State::Ready(Arc::new(build_pipeline(device, &layout, &depth, &normals)));
    let state = Arc::new(Mutex::new(state));
    all.push(Shared {
        device: Arc::clone(device),
        layout: Arc::clone(&layout),
        state: Arc::clone(&state),
    });
    (layout, state)
}

/// Builds the pass's pipeline on `device`, here and now, with layouts of its
/// own that match what the shader declares: what a test compiles under
/// D3D12 (FXC by name), where every other test's adapter is Vulkan and a
/// construct only FXC rejects would pass them all.
#[doc(hidden)]
pub fn build_pipeline_for_testing(device: &wgpu::Device) {
    use bsengine_bake::gpu_scene::{sky_layout_entry, storage_layout_entry, uniform_layout_entry};
    let fragment = wgpu::ShaderStages::FRAGMENT;
    let scene = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("rt reflections bgl (test)"),
        entries: &[
            storage_layout_entry(0, fragment, true),
            storage_layout_entry(1, fragment, true),
            storage_layout_entry(2, fragment, true),
            uniform_layout_entry(3, fragment),
            sky_layout_entry(4, fragment),
            uniform_layout_entry(5, fragment),
            storage_layout_entry(6, fragment, true),
        ],
    });
    // One unfilterable float texture each, as the prepass depth and normals
    // are bound at 1 and 2.
    let one_texture = || {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("rt reflections screen bgl (test)"),
            entries: &[sky_layout_entry(0, fragment)],
        })
    };
    build_pipeline(device, &scene, &one_texture(), &one_texture());
}

/// A surface's ray-traced reflections: the shared pipeline, and this
/// surface's parameters.
pub(crate) struct RtReflections {
    layout: Arc<wgpu::BindGroupLayout>,
    state: Arc<Mutex<State>>,
    params: wgpu::Buffer,
    /// Frames the pass ran, for the tests that wait for it.
    pub(crate) frames_traced: u32,
}

impl RtReflections {
    pub(crate) fn new(
        device: &Arc<wgpu::Device>,
        depth: Arc<wgpu::BindGroupLayout>,
        normals: Arc<wgpu::BindGroupLayout>,
    ) -> Self {
        let (layout, state) = shared(device, depth, normals);
        let params = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("rt reflections params"),
            size: std::mem::size_of::<RtParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self {
            layout,
            state,
            params,
            frames_traced: 0,
        }
    }

    /// The pipeline, once built -- collected from its thread the first time
    /// it is. Never waits.
    pub(crate) fn pipeline(&self) -> Option<Arc<wgpu::RenderPipeline>> {
        #[cfg_attr(target_arch = "wasm32", allow(unused_mut))]
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        #[cfg(not(target_arch = "wasm32"))]
        if matches!(&*state, State::Compiling(h) if h.is_finished()) {
            let State::Compiling(handle) = std::mem::replace(&mut *state, State::Failed) else {
                unreachable!("matched above");
            };
            match handle.join() {
                Ok(p) => *state = State::Ready(Arc::new(p)),
                Err(_) => tracing::error!(
                    "ray-traced reflections: building the pipeline failed; reflections stay \
                     screen-space"
                ),
            }
        }
        match &*state {
            State::Ready(p) => Some(Arc::clone(p)),
            _ => None,
        }
    }

    /// The pass's scene bind group for this frame: the GI's scene, the probe
    /// grid, and `params` uploaded.
    pub(crate) fn scene_bind_group(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        scene: &crate::realtime_gi::TraceScene<'_>,
        probes: &wgpu::Buffer,
        mut params: RtParams,
    ) -> wgpu::BindGroup {
        params.instance_count = scene.instance_count;
        queue.write_buffer(&self.params, 0, bytemuck::bytes_of(&params));
        let sky = scene.sky.create_view(&Default::default());
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("rt reflections bg"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: scene.nodes.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: scene.tris.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: probes.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: scene.scene.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(&sky),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: self.params.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: scene.instances.as_entire_binding(),
                },
            ],
        })
    }
}
