//! Ray-traced reflections (`bsengine_core::RayTracedReflections`): the share
//! of each pixel's reflection the screen-space trace did not find, traced
//! through the realtime GI's scene.
//!
//! # Where it sits
//!
//! The screen-space trace (`post_process::trace_ssr`) writes its reflection
//! premultiplied -- light times weight, and the weight -- into the target the
//! opaque pass reads, before that pass. These passes run right after it, over
//! the same prepass depth and normals, and the last of them draws *under* it:
//! blended as `src * (1 - dst.a) + dst`, so a ray answers only the share the
//! screen left, and the target still holds one premultiplied reflection the
//! mesh shader reads as it always has. Nothing about the mesh shader changes
//! -- which matters, because its sampled-texture budget (16) is spent.
//! HDRP's "Mixed" mode and Lumen's reflections order the two the same way:
//! the screen first, rays for what it missed.
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
//! # Glossy surfaces: one sample a frame, and the frames between them
//!
//! A surface with any roughness reflects a lobe, not a line. Each pixel
//! traces one direction a frame, drawn from the GGX lobe of its roughness
//! (Heitz's visible-normal sampling -- what HDRP's and Unreal's ray-traced
//! reflections draw from), turned every frame; the frames are then blended
//! into a history of where the same surface was last frame, as the
//! screen-space GI's temporal filter does (`post_process`'s
//! `SSGI_TEMPORAL_WGSL`): reprojected by its velocity, or by its depth and
//! the two cameras where it has none, and thrown away where the depth the
//! history recorded there is another surface's. A mirror has a lobe of one
//! direction and no noise to average, and nothing of last frame is kept for
//! it -- or a moving thing's reflection in it would trail.
//!
//! So three passes: the trace, into a target of its own; the temporal blend,
//! into one of the two histories (each frame writes the other); and the
//! composite, under the screen's reflection.
//!
//! # What it needs
//!
//! The GI's scene, so a realtime probe volume that has updated at least once.
//! Without one the camera keeps its screen-space reflections alone, and the
//! renderer says so once.
//!
//! The pipelines are built on a thread, as the GI's are: the trace walks the
//! same tree, and FXC takes seconds over that (a minute and more on CI's
//! software D3D12). Until they are built, frames reflect as if it were off.

use std::sync::{Arc, Mutex};

/// The passes' own parameters: `RtParams` in every shader here.
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct RtParams {
    /// Unjittered, as the screen-space trace's: the prepass depth is.
    pub inv_view_proj: [[f32; 4]; 4],
    /// The same camera forward: a point's view depth is its clip w.
    pub view_proj: [[f32; 4]; 4],
    /// Last frame's unjittered view-projection: where a surface was then.
    pub prev_view_proj: [[f32; 4]; 4],
    /// xyz the camera, w the intensity.
    pub cam_pos: [f32; 4],
    pub max_roughness: f32,
    pub max_distance: f32,
    pub instance_count: u32,
    /// Turns the lobe's samples every frame.
    pub frame: u32,
    /// Set by [`RtReflections::encode`]: 1 when the history holds the frame
    /// before this one, at this size.
    pub history_valid: u32,
    pub _pad: [u32; 3],
}

/// The params, as each shader declares them.
const PARAMS_WGSL: &str = r#"
struct RtParams {
    inv_view_proj: mat4x4<f32>,
    view_proj: mat4x4<f32>,
    prev_view_proj: mat4x4<f32>,
    cam_pos: vec4<f32>,
    max_roughness: f32,
    max_distance: f32,
    instance_count: u32,
    frame: u32,
    history_valid: u32,
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
};

@vertex
fn vs_fullscreen(@builtin(vertex_index) vi: u32) -> @builtin(position) vec4<f32> {
    var p = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0),
    );
    return vec4<f32>(p[vi], 0.0, 1.0);
}
"#;

/// The trace: a fullscreen triangle whose fragments each trace one
/// direction of their reflection's lobe. Compiled after `TRACE_WGSL`, the
/// probe grid, `INSTANCES_WGSL` and [`PARAMS_WGSL`].
const TRACE_PASS_WGSL: &str = r#"
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

// A half vector from the GGX distribution of visible normals (Heitz 2018,
// "Sampling the GGX Distribution of Visible Normals"), in the surface's
// tangent space with the normal along z: `v` toward the viewer, `alpha` the
// GGX width (roughness squared, as the mesh shader's `distribution_ggx`
// takes it). Drawing visible normals rather than all of them spends no
// sample on a facet the viewer cannot see.
fn sample_vndf(v: vec3<f32>, alpha: f32, u: vec2<f32>) -> vec3<f32> {
    let vh = normalize(vec3<f32>(alpha * v.x, alpha * v.y, v.z));
    let lensq = vh.x * vh.x + vh.y * vh.y;
    var t1 = vec3<f32>(1.0, 0.0, 0.0);
    if lensq > 0.0 {
        t1 = vec3<f32>(-vh.y, vh.x, 0.0) / sqrt(lensq);
    }
    let t2 = cross(vh, t1);
    let r = sqrt(u.x);
    let phi = 2.0 * PI * u.y;
    let p1 = r * cos(phi);
    let s = 0.5 * (1.0 + vh.z);
    let p2 = (1.0 - s) * sqrt(max(1.0 - p1 * p1, 0.0)) + s * (r * sin(phi));
    let nh = p1 * t1 + p2 * t2 + sqrt(max(1.0 - p1 * p1 - p2 * p2, 0.0)) * vh;
    return normalize(vec3<f32>(alpha * nh.x, alpha * nh.y, max(nh.z, 0.0)));
}

@fragment
fn fs_trace(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
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
    let roughness = nr.w;
    let share = clamp(1.0 - (roughness - rt.max_roughness) / 0.2, 0.0, 1.0) * rt.cam_pos.w;
    if share <= 0.0 {
        return vec4<f32>(0.0);
    }
    let uv = frag.xy / dims;
    let clip = rt.inv_view_proj * vec4<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, depth, 1.0);
    let world = clip.xyz / clip.w;
    let n = normalize(nr.xyz);
    let to_surface = world - rt.cam_pos.xyz;
    let v = -normalize(to_surface);
    // This frame's direction in the lobe: from the pixel and the frame,
    // never from scheduling, so a frame is the same bytes every run.
    rng_state = pcg(u32(coord.x) ^ pcg(u32(coord.y) ^ pcg(rt.frame)));
    let u = vec2<f32>(next_f32(), next_f32());
    let tb = basis(n);
    let v_local = vec3<f32>(dot(v, tb[0]), dot(v, tb[1]), dot(v, n));
    let h_local = sample_vndf(v_local, roughness * roughness, u);
    let h = tb[0] * h_local.x + tb[1] * h_local.y + n * h_local.z;
    var r = reflect(-v, h);
    // A facet can send the ray under the surface at a grazing view; the
    // mirror direction instead, which is the lobe's middle.
    if dot(r, n) <= 0.0 {
        r = reflect(-v, n);
    }
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

/// The temporal blend: this frame's trace into the history of where the
/// same surface was last frame. Compiled after [`PARAMS_WGSL`].
const TEMPORAL_PASS_WGSL: &str = r#"
@group(0) @binding(0) var current_tex: texture_2d<f32>;
@group(0) @binding(1) var history_tex: texture_2d<f32>;
@group(0) @binding(2) var history_depth_tex: texture_2d<f32>;
@group(0) @binding(3) var history_samp: sampler;
@group(0) @binding(4) var<uniform> rt: RtParams;
@group(1) @binding(0) var depth_tex: texture_2d<f32>;
@group(2) @binding(0) var normal_tex: texture_2d<f32>;
@group(2) @binding(1) var velocity_tex: texture_2d<f32>;

struct TemporalOut {
    @location(0) light: vec4<f32>,
    @location(1) depth: vec4<f32>,
};

// The share of this frame in the history: an exponential mean over about
// the last ten, as the screen-space GI's (`SSGI_HISTORY_BLEND`) -- for a
// glossy surface. A mirror's lobe is one direction: nothing to average, and
// last frame's reflection of something that moved would trail behind it.
// Between 0.05 and 0.2 of roughness, from all of this frame to a tenth.
const HISTORY_BLEND: f32 = 0.1;

@fragment
fn fs_temporal(@builtin(position) frag: vec4<f32>) -> TemporalOut {
    let coord = vec2<i32>(frag.xy);
    let dims = vec2<f32>(textureDimensions(depth_tex, 0));
    let current = textureLoad(current_tex, coord, 0);
    let depth = textureLoad(depth_tex, coord, 0).x;
    var out: TemporalOut;
    out.light = current;
    out.depth = vec4<f32>(0.0);
    if depth >= 1.0 {
        return out;
    }
    let uv = (vec2<f32>(coord) + vec2<f32>(0.5)) / dims;
    let ndc = vec4<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, depth, 1.0);
    let world_h = rt.inv_view_proj * ndc;
    let world = world_h.xyz / world_h.w;
    // The view depth, as clip w: what the history records of this pixel.
    let w_now = (rt.view_proj * vec4<f32>(world, 1.0)).w;
    out.depth = vec4<f32>(w_now, 0.0, 0.0, 0.0);
    if rt.history_valid == 0u {
        return out;
    }
    let roughness = textureLoad(normal_tex, coord, 0).w;
    let blend = mix(1.0, HISTORY_BLEND, clamp((roughness - 0.05) / 0.15, 0.0, 1.0));
    if blend >= 1.0 {
        return out;
    }
    let prev_clip = rt.prev_view_proj * vec4<f32>(world, 1.0);
    if prev_clip.w <= 0.0 {
        return out;
    }
    // Where the surface was: by its velocity, or -- with none (terrain, a
    // custom shader) -- by its depth and last frame's camera, as TAA finds it.
    let velocity = textureLoad(velocity_tex, coord, 0).xy;
    var prev_uv = uv - velocity;
    if velocity.x > 1000.0 {
        let prev_ndc = prev_clip.xyz / prev_clip.w;
        prev_uv = vec2<f32>(prev_ndc.x * 0.5 + 0.5, 0.5 - prev_ndc.y * 0.5);
    }
    if any(prev_uv < vec2<f32>(0.0)) || any(prev_uv > vec2<f32>(1.0)) {
        return out;
    }
    // The same surface last frame stood where the previous camera saw it at
    // `prev_clip.w`; a history that recorded something else there is
    // another surface's -- the one this pixel's surface was hidden behind --
    // and is not this one's past.
    let recorded = textureSampleLevel(history_depth_tex, history_samp, prev_uv, 0.0).r;
    if recorded <= 0.0 || abs(recorded - prev_clip.w) > 0.05 * prev_clip.w {
        return out;
    }
    let history = textureSampleLevel(history_tex, history_samp, prev_uv, 0.0);
    out.light = mix(history, current, blend);
    return out;
}
"#;

/// The composite: the history this frame wrote, under the screen's
/// reflection. Compiled after [`PARAMS_WGSL`].
const COMPOSITE_PASS_WGSL: &str = r#"
@group(0) @binding(0) var light_tex: texture_2d<f32>;

@fragment
fn fs_composite(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    return textureLoad(light_tex, vec2<i32>(frag.xy), 0);
}
"#;

/// The history's view depth: a half float is within the 5% the rejection
/// compares at (as the screen-space GI's), and filterable.
const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rg16Float;

/// The three passes' pipelines.
pub(crate) struct Pipelines {
    trace: wgpu::RenderPipeline,
    temporal: wgpu::RenderPipeline,
    composite: wgpu::RenderPipeline,
}

/// The layouts the passes' own bind groups are made with (the depth and
/// normals are the screen-space trace's).
struct Layouts {
    scene: wgpu::BindGroupLayout,
    temporal: wgpu::BindGroupLayout,
    composite: wgpu::BindGroupLayout,
}

enum State {
    #[cfg(not(target_arch = "wasm32"))]
    Compiling(std::thread::JoinHandle<Pipelines>),
    Ready(Arc<Pipelines>),
    /// The build panicked: logged once, and reflections stay screen-space.
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    Failed,
}

/// One device's layouts and pipelines: built once per device, as the GI's
/// are, so every surface on it -- every test sharing the test device --
/// pays for the build once.
struct Shared {
    device: Arc<wgpu::Device>,
    layouts: Arc<Layouts>,
    state: Arc<Mutex<State>>,
}

static SHARED: Mutex<Vec<Shared>> = Mutex::new(Vec::new());

fn texture_entry(binding: u32, filterable: bool) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    }
}

fn make_layouts(device: &wgpu::Device) -> Layouts {
    use bsengine_bake::gpu_scene::{sky_layout_entry, storage_layout_entry, uniform_layout_entry};
    let fragment = wgpu::ShaderStages::FRAGMENT;
    Layouts {
        scene: device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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
        temporal: device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("rt reflections temporal bgl"),
            entries: &[
                texture_entry(0, false),
                texture_entry(1, true),
                texture_entry(2, true),
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: fragment,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                uniform_layout_entry(4, fragment),
            ],
        }),
        composite: device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("rt reflections composite bgl"),
            entries: &[texture_entry(0, false)],
        }),
    }
}

/// A fullscreen pipeline over `module`'s `entry`, into `targets`.
fn fullscreen(
    device: &wgpu::Device,
    label: &str,
    module: &wgpu::ShaderModule,
    entry: &str,
    groups: &[&wgpu::BindGroupLayout],
    targets: &[Option<wgpu::ColorTargetState>],
) -> wgpu::RenderPipeline {
    let groups: Vec<Option<&wgpu::BindGroupLayout>> = groups.iter().map(|g| Some(*g)).collect();
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some(label),
        bind_group_layouts: &groups,
        immediate_size: 0,
    });
    crate::pipeline_cache::create_render_pipeline(
        device,
        &wgpu::RenderPipelineDescriptor {
            label: Some(label),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module,
                entry_point: Some("vs_fullscreen"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module,
                entry_point: Some(entry),
                targets,
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

fn build_pipelines(
    device: &wgpu::Device,
    layouts: &Layouts,
    depth: &wgpu::BindGroupLayout,
    normals: &wgpu::BindGroupLayout,
) -> Pipelines {
    let source = format!(
        "{}{}{}{}{}",
        bsengine_bake::gpu_scene::TRACE_WGSL,
        crate::realtime_gi::probe_grid_wgsl!(),
        crate::realtime_gi::INSTANCES_WGSL,
        PARAMS_WGSL,
        TRACE_PASS_WGSL
    );
    // Without naga's loop bounding, as the GI's update: the tree walk ends on
    // its own, and the bound is what DXC miscompiled the update's walk with
    // (`realtime_gi::build_pipelines`).
    let trace_module = unsafe {
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
    let temporal_module = crate::pipeline_cache::create_shader_module(
        device,
        wgpu::ShaderModuleDescriptor {
            label: Some("rt reflections temporal"),
            source: wgpu::ShaderSource::Wgsl(format!("{PARAMS_WGSL}{TEMPORAL_PASS_WGSL}").into()),
        },
    );
    let composite_module = crate::pipeline_cache::create_shader_module(
        device,
        wgpu::ShaderModuleDescriptor {
            label: Some("rt reflections composite"),
            source: wgpu::ShaderSource::Wgsl(format!("{PARAMS_WGSL}{COMPOSITE_PASS_WGSL}").into()),
        },
    );
    let hdr = |blend| {
        Some(wgpu::ColorTargetState {
            format: crate::post_process::HDR_FORMAT,
            blend,
            write_mask: wgpu::ColorWrites::ALL,
        })
    };
    // Under what the screen found: the ray's light fills the share it left.
    let under = wgpu::BlendComponent {
        src_factor: wgpu::BlendFactor::OneMinusDstAlpha,
        dst_factor: wgpu::BlendFactor::One,
        operation: wgpu::BlendOperation::Add,
    };
    Pipelines {
        trace: fullscreen(
            device,
            "rt reflections trace",
            &trace_module,
            "fs_trace",
            &[&layouts.scene, depth, normals],
            &[hdr(None)],
        ),
        temporal: fullscreen(
            device,
            "rt reflections temporal",
            &temporal_module,
            "fs_temporal",
            &[&layouts.temporal, depth, normals],
            &[
                hdr(None),
                Some(wgpu::ColorTargetState {
                    format: DEPTH_FORMAT,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                }),
            ],
        ),
        composite: fullscreen(
            device,
            "rt reflections composite",
            &composite_module,
            "fs_composite",
            &[&layouts.composite],
            &[hdr(Some(wgpu::BlendState {
                color: under,
                alpha: under,
            }))],
        ),
    }
}

/// Builds the passes' pipelines on `device`, here and now, with layouts of
/// their own that match what the shaders declare: what a test compiles
/// under D3D12 (FXC by name), where every other test's adapter is Vulkan
/// and a construct only FXC rejects would pass them all; and what the GLES
/// gate translates.
#[doc(hidden)]
pub fn build_pipeline_for_testing(device: &wgpu::Device) {
    use bsengine_bake::gpu_scene::sky_layout_entry;
    let fragment = wgpu::ShaderStages::FRAGMENT;
    let layouts = make_layouts(device);
    let depth = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("rt reflections depth bgl (test)"),
        entries: &[sky_layout_entry(0, fragment)],
    });
    let normals = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("rt reflections normals bgl (test)"),
        entries: &[sky_layout_entry(0, fragment), sky_layout_entry(1, fragment)],
    });
    build_pipelines(device, &layouts, &depth, &normals);
}

/// `device`'s layouts and pipeline state, starting the build on the first ask.
fn shared(
    device: &Arc<wgpu::Device>,
    depth: Arc<wgpu::BindGroupLayout>,
    normals: Arc<wgpu::BindGroupLayout>,
) -> (Arc<Layouts>, Arc<Mutex<State>>) {
    let mut all = SHARED.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(s) = all.iter().find(|s| Arc::ptr_eq(&s.device, device)) {
        return (Arc::clone(&s.layouts), Arc::clone(&s.state));
    }
    let layouts = Arc::new(make_layouts(device));
    #[cfg(not(target_arch = "wasm32"))]
    let state = {
        let (device, layouts) = (Arc::clone(device), Arc::clone(&layouts));
        State::Compiling(
            std::thread::Builder::new()
                .name("rt reflections pipelines".into())
                .spawn(move || build_pipelines(&device, &layouts, &depth, &normals))
                .expect("a thread for the ray-traced reflections' pipelines"),
        )
    };
    #[cfg(target_arch = "wasm32")]
    let state = State::Ready(Arc::new(build_pipelines(
        device, &layouts, &depth, &normals,
    )));
    let state = Arc::new(Mutex::new(state));
    all.push(Shared {
        device: Arc::clone(device),
        layouts: Arc::clone(&layouts),
        state: Arc::clone(&state),
    });
    (layouts, state)
}

/// One of the two histories: the blended light, and the view depth it was
/// blended at.
struct History {
    light: wgpu::TextureView,
    depth: wgpu::TextureView,
    _textures: [wgpu::Texture; 2],
}

/// The targets at the screen's size: this frame's trace, and the histories.
struct Targets {
    size: (u32, u32),
    current: wgpu::TextureView,
    _current: wgpu::Texture,
    histories: [History; 2],
}

fn target(
    device: &wgpu::Device,
    label: &str,
    size: (u32, u32),
    format: wgpu::TextureFormat,
) -> (wgpu::Texture, wgpu::TextureView) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width: size.0.max(1),
            height: size.1.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let view = texture.create_view(&Default::default());
    (texture, view)
}

impl Targets {
    fn new(device: &wgpu::Device, size: (u32, u32)) -> Self {
        let hdr = crate::post_process::HDR_FORMAT;
        let (current_texture, current) = target(device, "rt reflections trace", size, hdr);
        let history = |i: usize| {
            let (lt, light) = target(device, &format!("rt reflections history {i}"), size, hdr);
            let (dt, depth) = target(
                device,
                &format!("rt reflections history depth {i}"),
                size,
                DEPTH_FORMAT,
            );
            History {
                light,
                depth,
                _textures: [lt, dt],
            }
        };
        Self {
            size,
            current,
            _current: current_texture,
            histories: [history(0), history(1)],
        }
    }
}

/// The screen-space trace's bind groups and target, which these passes read
/// from and draw into (`PostProcess::rt_reflection_inputs`).
pub(crate) struct ScreenInputs<'a> {
    pub depth: &'a wgpu::BindGroup,
    pub normals: &'a wgpu::BindGroup,
    pub target: &'a wgpu::TextureView,
    pub size: (u32, u32),
}

/// A surface's ray-traced reflections: the shared pipelines, and this
/// surface's parameters, targets and history.
pub(crate) struct RtReflections {
    layouts: Arc<Layouts>,
    state: Arc<Mutex<State>>,
    params: wgpu::Buffer,
    sampler: wgpu::Sampler,
    targets: Option<Targets>,
    /// The history last written (0 or 1): next frame's to read.
    current: usize,
    /// Whether that history holds the frame before the next one: false at
    /// first, after a resize, and after a frame without the pass.
    history_valid: bool,
    /// Whether frames are blended into the history at all: always, but for
    /// the test that measures what the blend takes away.
    pub(crate) history_enabled: bool,
    /// Frames the passes ran, for the tests that wait for them, and the seed
    /// that turns each frame's samples.
    pub(crate) frames_traced: u32,
}

impl RtReflections {
    pub(crate) fn new(
        device: &Arc<wgpu::Device>,
        depth: Arc<wgpu::BindGroupLayout>,
        normals: Arc<wgpu::BindGroupLayout>,
    ) -> Self {
        let (layouts, state) = shared(device, depth, normals);
        let params = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("rt reflections params"),
            size: std::mem::size_of::<RtParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("rt reflections history sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });
        Self {
            layouts,
            state,
            params,
            sampler,
            targets: None,
            current: 0,
            history_valid: false,
            history_enabled: true,
            frames_traced: 0,
        }
    }

    /// The pipelines, once built -- collected from their thread the first
    /// time they are. Never waits.
    pub(crate) fn pipelines(&self) -> Option<Arc<Pipelines>> {
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
                    "ray-traced reflections: building the pipelines failed; reflections stay \
                     screen-space"
                ),
            }
        }
        match &*state {
            State::Ready(p) => Some(Arc::clone(p)),
            _ => None,
        }
    }

    /// A frame without the passes: whatever the history holds is no longer
    /// the frame before the next one.
    pub(crate) fn skip_frame(&mut self) {
        self.history_valid = false;
    }

    /// Encodes the three passes: the trace into this frame's target, the
    /// temporal blend into the history the last frame did not write, and
    /// that history under the screen's reflection.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn encode(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        pipelines: &Pipelines,
        scene: &crate::realtime_gi::TraceScene<'_>,
        probes: &wgpu::Buffer,
        screen: ScreenInputs<'_>,
        mut params: RtParams,
    ) {
        if self.targets.as_ref().is_none_or(|t| t.size != screen.size) {
            self.targets = Some(Targets::new(device, screen.size));
            self.history_valid = false;
        }
        let targets = self.targets.as_ref().expect("made above");
        params.instance_count = scene.instance_count;
        params.frame = self.frames_traced;
        params.history_valid = u32::from(self.history_valid && self.history_enabled);
        queue.write_buffer(&self.params, 0, bytemuck::bytes_of(&params));

        let sky = scene.sky.create_view(&Default::default());
        let scene_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("rt reflections bg"),
            layout: &self.layouts.scene,
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
        });
        let read = &targets.histories[self.current];
        let write = &targets.histories[1 - self.current];
        let temporal_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("rt reflections temporal bg"),
            layout: &self.layouts.temporal,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&targets.current),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&read.light),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&read.depth),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: self.params.as_entire_binding(),
                },
            ],
        });
        let composite_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("rt reflections composite bg"),
            layout: &self.layouts.composite,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&write.light),
            }],
        });
        let attachment = |view, load| {
            Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load,
                    store: wgpu::StoreOp::Store,
                },
            })
        };
        let clear = wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT);
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("rt reflections trace"),
                color_attachments: &[attachment(&targets.current, clear)],
                ..Default::default()
            });
            pass.set_pipeline(&pipelines.trace);
            pass.set_bind_group(0, &scene_bg, &[]);
            pass.set_bind_group(1, screen.depth, &[]);
            pass.set_bind_group(2, screen.normals, &[]);
            pass.draw(0..3, 0..1);
        }
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("rt reflections temporal"),
                color_attachments: &[
                    attachment(&write.light, clear),
                    attachment(&write.depth, clear),
                ],
                ..Default::default()
            });
            pass.set_pipeline(&pipelines.temporal);
            pass.set_bind_group(0, &temporal_bg, &[]);
            pass.set_bind_group(1, screen.depth, &[]);
            pass.set_bind_group(2, screen.normals, &[]);
            pass.draw(0..3, 0..1);
        }
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("rt reflections composite"),
                color_attachments: &[attachment(screen.target, wgpu::LoadOp::Load)],
                ..Default::default()
            });
            pass.set_pipeline(&pipelines.composite);
            pass.set_bind_group(0, &composite_bg, &[]);
            pass.draw(0..3, 0..1);
        }
        self.current = 1 - self.current;
        self.history_valid = true;
        self.frames_traced += 1;
    }
}
