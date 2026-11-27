//! The realtime light-probe volume (`bsengine_rhi_wgpu::realtime_gi`),
//! against closed forms and at the pixel.
//!
//! In a process of its own, not among the crate's unit tests: on Windows CI
//! every GPU test runs on the software D3D12 adapter, where building the
//! update's pipelines (FXC, then WARP's own compile) takes over a minute.
//! Run beside the crate's other GPU tests, that minute of contention crashed
//! the test process on teardown, three runs out of three; alone, it passes.
//! Every test here shares one device (`WgpuSurface::offscreen_for_testing`),
//! and the pipelines are built once per device, so the minute is paid once.

use bsengine_rhi_wgpu::mesh::{GpuMeshRegistry, Vertex};
use bsengine_rhi_wgpu::surface::WgpuSurface;
use bsengine_rhi_wgpu::{LightData, MaterialParams, ProbeVolumeParams};
use glam::{Mat4, Vec3};

type DrawCall = (u64, Mat4, Option<u64>, MaterialParams, Option<String>);

/// A 64 x 64 offscreen surface on the shared test device, its registry, and
/// the quads drawn each frame. The view-projection is the identity, so world
/// xy is clip xy and a quad at z in 0..1 is on screen.
struct Surface {
    surface: WgpuSurface,
    registry: GpuMeshRegistry,
    draw_calls: Vec<DrawCall>,
    /// The view-projection and the eye: the identity and (0, 0, 5) unless a
    /// test sets a real camera.
    camera: (Mat4, Vec3),
    /// The sun's colour and the ambient: `LightData::default()`'s unless a
    /// test darkens them.
    sun: Option<(Vec3, Vec3)>,
    /// The camera's screen-space reflections: none unless a test sets them.
    ssr: Option<bsengine_core::ScreenSpaceReflections>,
}

impl Surface {
    /// With one quad, `corners` in counter-clockwise order as seen from its
    /// front, emitting `emissive`.
    fn new(corners: [Vec3; 4], emissive: Vec3) -> Self {
        let surface = WgpuSurface::offscreen_for_testing(64, 64)
            .expect("these tests need an adapter; a skip would look like a pass");
        let registry = GpuMeshRegistry::new(surface.device_arc());
        let mut s = Self {
            surface,
            registry,
            draw_calls: Vec::new(),
            camera: (Mat4::IDENTITY, Vec3::new(0.0, 0.0, 5.0)),
            sun: None,
            ssr: None,
        };
        s.push_quad(corners, emissive);
        s
    }

    /// Adds another quad, black, emitting `emissive`.
    fn push_quad(&mut self, corners: [Vec3; 4], emissive: Vec3) {
        let normal = (corners[1] - corners[0])
            .cross(corners[2] - corners[0])
            .normalize();
        let vertices: Vec<Vertex> = corners
            .iter()
            .map(|p| Vertex {
                position: p.to_array(),
                color: [1.0, 1.0, 1.0],
                normal: normal.to_array(),
                uv: [0.0, 0.0],
                uv2: [0.0, 0.0],
                tangent: [0.0; 4],
            })
            .collect();
        let mesh_id = self.registry.register(&vertices, &[0, 1, 2, 0, 2, 3]);
        self.draw_calls.push((
            mesh_id,
            Mat4::IDENTITY,
            None,
            MaterialParams {
                metallic: 0.0,
                roughness: 1.0,
                emissive,
                base_color: Vec3::ZERO,
                opacity: 1.0,
                lightmap: None,
                ..MaterialParams::default()
            },
            None,
        ));
    }

    /// One real `render_frame`, everything this file does not care about at
    /// its neutral value, and no baked volume.
    fn render(&mut self, _volume: Option<ProbeVolumeParams>) -> Result<(), String> {
        let ui_state = bsengine_core::UiState::default();
        self.surface
            .render_frame(
                self.camera.0,
                self.camera.1,
                &bsengine_rhi_wgpu::shadow::DirectionalCascades::new(
                    Vec3::new(0.0, -1.0, 0.0),
                    Mat4::IDENTITY,
                    bsengine_core::shadow_config::DEFAULT_SHADOW_DISTANCE,
                    bsengine_core::shadow_config::DEFAULT_CASCADES,
                    0.0,
                ),
                None,
                &self.draw_calls,
                &[],
                &[],
                0,
                &self.registry,
                match self.sun {
                    None => LightData::default(),
                    Some((color, ambient)) => LightData {
                        color,
                        ambient,
                        ..LightData::default()
                    },
                },
                None,
                &std::collections::HashMap::new(),
                &ui_state,
                &std::collections::HashMap::new(),
                0.0,
                0.0,
                false,
                false,
                Mat4::IDENTITY,
                None,
                None,
                None,
                self.ssr,
                None,
                &[],
                false,
                false,
                false,
                None,
                None,
                0.0,
                &[],
                None,
                (0.0, 0.0),
                0,
                // The unjittered view-projection: what the screen-space
                // passes reconstruct positions through. The identity here
                // put every pixel's reflection somewhere else.
                self.camera.0,
                None,
                &[],
                None,
                None,
                None,
                None,
                None,
                None,
                &[],
            )
            .map(|_| ())
    }
}

/// The update compiles where Windows players run it -- wgpu's D3D12
/// backend, through FXC -- and without making a frame wait for it. Every
/// other test takes whatever adapter comes first -- Vulkan, on most machines
/// -- so a construct FXC rejects (an array-returning function was one) would
/// pass them all and fail only where D3D12 happened to be chosen.
#[cfg(windows)]
#[test]
fn the_update_compiles_for_d3d12() {
    // FXC by name, not by default: the engine's own devices compile with DXC
    // where its libraries are found, and FXC is the fallback a machine without
    // them -- every single-file build -- still runs this update through.
    let instance = wgpu::Instance::new({
        let mut d = wgpu::InstanceDescriptor::new_without_display_handle();
        d.backends = wgpu::Backends::DX12;
        d.backend_options.dx12.shader_compiler = wgpu::Dx12Compiler::Fxc;
        d
    });
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::LowPower,
        compatible_surface: None,
        force_fallback_adapter: false,
        apply_limit_buckets: false,
    }))
    .expect("Windows has a D3D12 adapter (WARP at worst); a skip would look like a pass");
    let (device, _queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("d3d12 compile check"),
        required_features: wgpu::Features::empty(),
        required_limits: wgpu::Limits::downlevel_defaults(),
        memory_hints: wgpu::MemoryHints::default(),
        experimental_features: wgpu::ExperimentalFeatures::disabled(),
        trace: wgpu::Trace::Off,
    }))
    .expect("a D3D12 device");
    let device = std::sync::Arc::new(device);
    let error_scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
    let (returned, built, ready) =
        bsengine_rhi_wgpu::realtime_gi::build_pipelines_for_testing(&device);
    let error = pollster::block_on(error_scope.pop());
    assert!(
        error.is_none(),
        "the update does not compile for D3D12: {error:?}"
    );
    assert!(ready, "the pipelines were built");
    // And off the frame: `new` hands the compile to a thread and returns.
    // The premise -- the compile is slow enough to matter -- is FXC's,
    // seconds on any machine.
    assert!(
        built.as_secs_f32() > 0.5,
        "premise: FXC takes a while: {built:?}"
    );
    assert!(
        returned < built / 4,
        "creating the update must not wait for its compile: {returned:?} of {built:?}"
    );
}

/// The ray-traced reflections' pass compiles for D3D12 through FXC too: the
/// same walk of the same tree as the update, in a fragment shader this time.
#[cfg(windows)]
#[test]
fn the_reflections_compile_for_d3d12() {
    let instance = wgpu::Instance::new({
        let mut d = wgpu::InstanceDescriptor::new_without_display_handle();
        d.backends = wgpu::Backends::DX12;
        d.backend_options.dx12.shader_compiler = wgpu::Dx12Compiler::Fxc;
        d
    });
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::LowPower,
        compatible_surface: None,
        force_fallback_adapter: false,
        apply_limit_buckets: false,
    }))
    .expect("Windows has a D3D12 adapter (WARP at worst); a skip would look like a pass");
    let (device, _queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("d3d12 compile check"),
        required_features: wgpu::Features::empty(),
        required_limits: wgpu::Limits::downlevel_defaults(),
        memory_hints: wgpu::MemoryHints::default(),
        experimental_features: wgpu::ExperimentalFeatures::disabled(),
        trace: wgpu::Trace::Off,
    }))
    .expect("a D3D12 device");
    let error_scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
    let started = std::time::Instant::now();
    bsengine_rhi_wgpu::rt_reflections::build_pipeline_for_testing(&device);
    let built = started.elapsed();
    let error = pollster::block_on(error_scope.pop());
    assert!(
        error.is_none(),
        "the reflections do not compile for D3D12: {error:?}"
    );
    eprintln!("rt reflections FXC compile: {built:?}");
}

// ---- the realtime probe volume -------------------------------------------

/// A scene for the realtime volume: [`Surface`]'s surface and
/// registry, with quads registered as the static set (their draws are
/// irrelevant -- realtime GI traces `statics`, not the draw list).
struct GiScene {
    scene: Surface,
    statics: Vec<bsengine_rhi_wgpu::realtime_gi::GiStatic>,
    dynamics: Vec<bsengine_rhi_wgpu::realtime_gi::GiDynamic>,
    lights: Vec<bsengine_bake::BakeLight>,
    sky: std::sync::Arc<bsengine_bake::Sky>,
}

/// Four corners of a quad from `min` along `u` and `v`, facing `u x v`.
fn quad_corners(min: Vec3, u: Vec3, v: Vec3) -> [Vec3; 4] {
    [min, min + u, min + u + v, min + v]
}

impl GiScene {
    fn new(sky: Vec3) -> Self {
        // A first quad far away, so `Surface` has one; not
        // a static.
        let scene = Surface::new(
            quad_corners(Vec3::new(500.0, 0.0, 0.0), Vec3::X, Vec3::Y),
            Vec3::ZERO,
        );
        Self {
            scene,
            statics: Vec::new(),
            dynamics: Vec::new(),
            lights: Vec::new(),
            sky: std::sync::Arc::new(bsengine_bake::Sky::Uniform(sky)),
        }
    }

    fn add(&mut self, corners: [Vec3; 4], albedo: Vec3, emissive: Vec3) {
        self.scene.push_quad(corners, Vec3::ZERO);
        let mesh_id = self.scene.draw_calls.last().unwrap().0;
        self.statics.push(bsengine_rhi_wgpu::realtime_gi::GiStatic {
            mesh_id,
            model: Mat4::IDENTITY,
            albedo,
            emissive,
        });
    }

    /// A moving entity: a quad with `corners` in its own space, placed by
    /// `model`. Returns its index in `dynamics`, to move it by.
    fn add_dynamic(
        &mut self,
        corners: [Vec3; 4],
        model: Mat4,
        albedo: Vec3,
        emissive: Vec3,
    ) -> usize {
        self.scene.push_quad(corners, Vec3::ZERO);
        let mesh_id = self.scene.draw_calls.last().unwrap().0;
        self.dynamics
            .push(bsengine_rhi_wgpu::realtime_gi::GiDynamic {
                mesh_id,
                model,
                albedo,
                emissive,
            });
        self.dynamics.len() - 1
    }

    /// A moving entity drawing `vertices` -- its own mesh, any shape -- placed
    /// at the origin. Returns its mesh's id.
    fn add_dynamic_mesh(
        &mut self,
        vertices: &[Vertex],
        indices: &[u32],
        albedo: Vec3,
        emissive: Vec3,
    ) -> u64 {
        let mesh_id = self.scene.registry.register(vertices, indices);
        self.dynamics
            .push(bsengine_rhi_wgpu::realtime_gi::GiDynamic {
                mesh_id,
                model: Mat4::IDENTITY,
                albedo,
                emissive,
            });
        mesh_id
    }

    /// A moving entity drawing a skinned mesh: `vertices` at rest, each
    /// bound by `skin` to the joints of a palette of `joints`. Posed by
    /// [`Self::pose`]; returns its mesh's id.
    fn add_skinned(
        &mut self,
        vertices: &[Vertex],
        skin: &[bsengine_rhi_wgpu::skinning::GpuVertexSkin],
        indices: &[u32],
        joints: u32,
        albedo: Vec3,
        emissive: Vec3,
    ) -> u64 {
        let mesh_id = self
            .scene
            .registry
            .register_skinned(vertices, skin, indices, joints);
        self.dynamics
            .push(bsengine_rhi_wgpu::realtime_gi::GiDynamic {
                mesh_id,
                model: Mat4::IDENTITY,
                albedo,
                emissive,
            });
        mesh_id
    }

    /// Poses skinned mesh `mesh` with `joints` and runs the skinning pass,
    /// as the skinning system does ahead of the frame.
    fn pose(&mut self, mesh: u64, joints: &[Mat4]) {
        let queue = self.scene.surface.queue_arc();
        assert!(self.scene.registry.skin(&queue, mesh, joints), "posed");
        self.scene.registry.flush_skinning(&queue);
    }

    fn input(
        &self,
        volume: ProbeVolumeParams,
        hysteresis: f32,
        rays_per_frame: u32,
    ) -> bsengine_rhi_wgpu::realtime_gi::RealtimeGiInput {
        bsengine_rhi_wgpu::realtime_gi::RealtimeGiInput {
            volume,
            adaptive: None,
            rays_per_probe: 256,
            hysteresis,
            rays_per_frame,
            statics: self.statics.clone(),
            dynamics: self.dynamics.clone(),
            lights: self.lights.clone(),
            sky: self.sky.clone(),
        }
    }

    fn updates(&self) -> u32 {
        self.scene.surface.realtime_gi_updates()
    }

    /// Frames until `n` more updates have run. The first waits for the
    /// update's pipelines (built on a thread: seconds under FXC, over a
    /// minute on CI's software D3D12) and for the static meshes to come
    /// back from the GPU -- polled against the clock, never assumed (a
    /// fixed frame count is what flakes on a cold CI machine).
    fn run(&mut self, input: &bsengine_rhi_wgpu::realtime_gi::RealtimeGiInput, n: u32) {
        let target = self.updates() + n;
        // Ten minutes, against a build measured at about 2 min 40 s on CI's
        // software D3D12 (the first of these tests landed then, #1987's
        // run) and 37% longer since the update traces instances (9.3 s
        // against 6.8 s under FXC on a desktop). Five minutes was already
        // within a factor of two of failing for no fault of the code.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(600);
        while self.updates() < target {
            assert!(
                std::time::Instant::now() < deadline,
                "the realtime volume never updated ({} of {target})",
                self.updates()
            );
            self.scene.surface.set_realtime_gi(Some(input.clone()));
            self.scene.render(None).expect("the frame renders");
            // Not a spin: while the pipelines build (a minute under FXC on
            // CI's software D3D12) frames beside the build are what crashed
            // the process there.
            if self.updates() < target {
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
        }
    }

    /// Probe `probe`'s E / PI for a surface facing `n`, as shading reads it.
    fn irradiance(&self, probe: usize, n: Vec3) -> Vec3 {
        let grid = self.scene.surface.read_probe_grid_for_testing();
        let c = grid.coeffs[probe];
        let sh = bsengine_rhi_wgpu::sh::ShL2 {
            coeffs: c.map(|v| Vec3::new(v[0], v[1], v[2])),
        };
        sh.eval_irradiance(n) / std::f32::consts::PI
    }

    /// The centre pixel of the last frame.
    fn centre(&self) -> [u8; 3] {
        let pixels = self
            .scene
            .surface
            .read_pixels()
            .expect("offscreen readback");
        let at = (32 * 64 + 32) * 4;
        [pixels[at], pixels[at + 1], pixels[at + 2]]
    }

    /// Frames until the ray-traced reflections' pass has drawn `n` more:
    /// its pipeline builds on a thread, so this waits against the clock,
    /// never for a fixed count of frames.
    fn run_reflected(&mut self, input: &bsengine_rhi_wgpu::realtime_gi::RealtimeGiInput, n: u32) {
        let target = self.scene.surface.rt_reflection_frames() + n;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(600);
        while self.scene.surface.rt_reflection_frames() < target {
            assert!(
                std::time::Instant::now() < deadline,
                "the ray-traced reflections never ran"
            );
            self.scene.surface.set_realtime_gi(Some(input.clone()));
            self.scene.render(None).expect("the frame renders");
            if self.scene.surface.rt_reflection_frames() < target {
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
        }
    }

    /// How many of the first `n` probes have been updated at least once.
    fn with_history(&self, n: usize) -> usize {
        let grid = self.scene.surface.read_probe_grid_for_testing();
        grid.coeffs[..n].iter().filter(|c| c[0][3] > 0.5).count()
    }
}

/// A closed unit box, every face facing in, all emissive `e` with albedo
/// `rho`.
fn furnace_scene(e: f32, rho: f32) -> GiScene {
    let mut s = GiScene::new(Vec3::splat(100.0));
    for (min, u, v) in [
        (Vec3::new(0.0, 0.0, 1.0), Vec3::X, Vec3::NEG_Z),
        (Vec3::new(0.0, 1.0, 0.0), Vec3::X, Vec3::Z),
        (Vec3::new(0.0, 0.0, 0.0), Vec3::Y, Vec3::Z),
        (Vec3::new(1.0, 0.0, 1.0), Vec3::Y, Vec3::NEG_Z),
        (Vec3::new(0.0, 0.0, 0.0), Vec3::X, Vec3::Y),
        (Vec3::new(1.0, 0.0, 1.0), Vec3::NEG_X, Vec3::Y),
    ] {
        s.add(quad_corners(min, u, v), Vec3::splat(rho), Vec3::splat(e));
    }
    s
}

/// A 2x2x2 grid well inside the unit box.
fn inner_volume() -> ProbeVolumeParams {
    ProbeVolumeParams {
        origin: Vec3::splat(0.3),
        extent: Vec3::splat(0.4),
        resolution: [2, 2, 2],
    }
}

/// Without realtime GI (`RenderCaps::realtime_gi`, off on OpenGL ES, whose
/// one GL context the GI's pipeline thread held past wgpu's lock timeout) a
/// realtime volume starts nothing: no GI object, so no pipeline thread, and
/// the frames still render. The same input on a renderer that has it starts
/// GI on its first frame -- observed there, not after updates, since the
/// pipelines take seconds and "no update yet" would hold either way.
#[test]
fn without_realtime_gi_a_volume_starts_nothing() {
    let mut with = furnace_scene(0.4, 0.5);
    let input = with.input(inner_volume(), 0.0, 1 << 20);
    with.scene.surface.set_realtime_gi(Some(input.clone()));
    with.scene.render(None).expect("the frame renders");
    assert!(
        with.scene.surface.realtime_gi_started(),
        "premise: a renderer with realtime GI starts it on the first frame"
    );

    let mut without = furnace_scene(0.4, 0.5);
    without.scene.surface =
        WgpuSurface::offscreen_for_testing_with_caps(64, 64, |caps| caps.realtime_gi = false)
            .expect("these tests need an adapter; a skip would look like a pass");
    for _ in 0..3 {
        without.scene.surface.set_realtime_gi(Some(input.clone()));
        without.scene.render(None).expect("the frame renders");
    }
    assert!(!without.scene.surface.realtime_gi_started());
    assert_eq!(without.updates(), 0);
}

/// Closed form, the furnace: inside a closed box whose every surface
/// emits `e` with albedo `rho`, the radiance everywhere is the fixed point
/// of `L = e + rho * L`, `e / (1 - rho)`. With no history kept
/// (hysteresis 0), update k reads `e (1 - rho^k) / (1 - rho)` -- the
/// first exactly `e`, each next one a bounce more, the bounces carried
/// by the probes themselves. Pins the bounce term, the SH projection's
/// weight and the units the update writes in. The sky (100) would show
/// any ray that escaped.
#[test]
fn realtime_probes_in_a_furnace_follow_the_geometric_series() {
    let (e, rho) = (0.4f32, 0.5f32);
    let mut s = furnace_scene(e, rho);
    let input = s.input(inner_volume(), 0.0, 1 << 20);
    s.run(&input, 1);
    let first = s.irradiance(0, Vec3::Y);
    assert!(
        (first.x - e).abs() < e * 0.01,
        "premise: the first update sees the walls' own emission, {e}: {first}"
    );
    for k in 2..=12 {
        s.run(&input, 1);
        let want = e * (1.0 - rho.powi(k)) / (1.0 - rho);
        let got = s.irradiance(0, Vec3::Y).x;
        assert!(
            (got - want).abs() < want * 0.01,
            "update {k}: {got} vs {want}"
        );
    }
    for probe in 1..8 {
        let got = s.irradiance(probe, Vec3::NEG_X).x;
        assert!(
            (got - e / (1.0 - rho)).abs() < 0.01,
            "probe {probe}, any direction: {got}"
        );
    }
}

/// The profiler times the probe update: the frame's most expensive work when
/// a volume is realtime, and the pass that showed D3D12 tracing half again
/// slower than Vulkan on the same GPU -- time no other pass accounted for.
/// Timings arrive a few frames late, so the volume keeps updating until they
/// do.
///
/// A time above zero alone proves nothing: a slot nobody wrote reads back
/// whatever it held (an update that dropped its timestamps passed that, at a
/// tenth of the real time). So the time is measured against itself: an
/// update doing eight times the work must read clearly longer, which a stale
/// slot cannot do. It used to be measured against the frame's other passes
/// instead -- "longer than all of them together" -- which held on desktop
/// GPUs and not on CI's lavapipe, which rasterises on the CPU: four shadow
/// cascades outweighed 64 probes (25.8 ms against 41.6 ms) and the test
/// failed with nothing wrong.
#[test]
fn the_probe_update_is_timed_on_the_gpu() {
    let Some(small) = probe_update_ms([4, 4, 4]) else {
        return;
    };
    let large = probe_update_ms([8, 8, 8]).expect("timestamps are per device");
    // Printed on success too, so a passing CI log still says by what margin.
    println!("probe update: 64 probes {small} ms, 512 probes {large} ms");
    assert!(
        large > 2.0 * small,
        "eight times the probes, and the update read {large} ms against {small} ms"
    );
}

/// The GPU time of one realtime GI update over a furnace volume of
/// `resolution` probes, every probe traced each update (a budget well above
/// `probes * 256`). `None` where the device has no timestamp queries -- and
/// then no GI time may be reported either.
///
/// The least of several updates, after the first timings have arrived: on
/// WARP (Windows CI's D3D12) a shader is compiled on its first dispatch, and
/// that dispatch's timestamps include the compile -- 4.8 s for 64 probes,
/// where the next read 4 ms. Whichever volume ran first would carry it.
fn probe_update_ms(resolution: [u32; 3]) -> Option<f32> {
    let mut s = furnace_scene(0.4, 0.5);
    let volume = ProbeVolumeParams {
        resolution,
        ..inner_volume()
    };
    let input = s.input(volume, 0.0, 1 << 20);
    s.run(&input, 1);
    let timed = |s: &mut GiScene| {
        s.run(&input, 1);
        s.scene.surface.latest_frame_stats().expect("frame stats")
    };
    // Timings arrive a few frames late: update until the frame's own passes
    // have been timed at all.
    let mut stats = timed(&mut s);
    for _ in 0..16 {
        if stats.gpu_pass_times_ms.iter().any(|p| p.name == "main") {
            break;
        }
        stats = timed(&mut s);
    }
    if !stats.gpu_timestamps_supported {
        let gi = stats
            .gpu_pass_times_ms
            .iter()
            .find(|p| p.name == "realtime_gi");
        assert!(gi.is_none(), "no timestamps, yet a GI time: {gi:?}");
        return None;
    }
    let mut least = f32::INFINITY;
    for _ in 0..6 {
        let names: Vec<_> = stats.gpu_pass_times_ms.iter().map(|p| &p.name).collect();
        assert!(
            names.iter().any(|n| *n == "main"),
            "premise: the frame's own passes were timed: {names:?}"
        );
        let gi = stats
            .gpu_pass_times_ms
            .iter()
            .find(|p| p.name == "realtime_gi")
            .unwrap_or_else(|| panic!("the probe update is not among the timed passes: {names:?}"));
        least = least.min(gi.duration_ms);
        stats = timed(&mut s);
    }
    Some(least)
}

/// The same scene, kept history: each update is `mix(sample, previous,
/// h)`, so the value one update after a probe's first is `e * (1 - h) +
/// e * h` from a sample of `e (1 + rho)` -- pinned exactly, which a
/// blend that ignored `h` (or ignored the history flag and blended into
/// the zeroed buffer) would miss.
#[test]
fn hysteresis_blends_each_update_into_the_last() {
    let (e, rho, h) = (0.4f32, 0.5f32, 0.75f32);
    let mut s = furnace_scene(e, rho);
    let input = s.input(inner_volume(), h, 1 << 20);
    s.run(&input, 1);
    assert!(
        (s.irradiance(0, Vec3::Y).x - e).abs() < e * 0.01,
        "the first update takes the sample whole, history or not"
    );
    s.run(&input, 1);
    let want = (1.0 - h) * e * (1.0 + rho) + h * e;
    let got = s.irradiance(0, Vec3::Y).x;
    assert!((got - want).abs() < want * 0.01, "{got} vs {want}");
}

/// The budget: rays for a quarter of the probes a frame -- each probe's 256
/// shading rays and its 32 relocation rays -- means each frame updates the
/// next quarter, and four frames cover every probe exactly -- the round robin
/// neither skips nor repeats.
#[test]
fn the_ray_budget_updates_the_grid_a_slice_at_a_time() {
    let mut s = furnace_scene(0.4, 0.5);
    let volume = ProbeVolumeParams {
        resolution: [4, 4, 4],
        ..inner_volume()
    };
    let input = s.input(volume, 0.5, (256 + 32) * 16);
    for frame in 1..=4 {
        s.run(&input, 1);
        assert_eq!(s.with_history(64), 16 * frame, "after {frame} frames");
    }
}

/// Closed form, an open floor: under a uniform sky `S` and a sun of
/// irradiance `E` straight down, an infinite floor of albedo `rho`
/// reflects `rho * (E / pi + S)` -- its own upper hemisphere is all sky
/// -- and a probe above it, facing down, sees nothing but floor. So its
/// E / pi facing down is that radiance: 0.5 * (3/pi + 0.2) = 0.577 lit,
/// and 0.5 * 0.2 = 0.1 once the sun is switched off and a few updates
/// have run. Pins the direct-light term at a hit (the bake's own
/// `irradiance`, with its shadow ray), the sky, and that the lights are
/// uploaded every frame rather than once.
#[test]
fn a_light_switched_off_leaves_the_probes() {
    let (sky, rho, sun) = (0.2f32, 0.5f32, 3.0f32);
    let mut s = GiScene::new(Vec3::splat(sky));
    s.add(
        quad_corners(
            Vec3::new(-50.0, 0.0, 50.0),
            Vec3::X * 100.0,
            Vec3::NEG_Z * 100.0,
        ),
        Vec3::splat(rho),
        Vec3::ZERO,
    );
    s.lights = vec![bsengine_bake::BakeLight::Directional {
        direction: Vec3::NEG_Y,
        color: Vec3::splat(sun),
        contribution: bsengine_bake::Contribution::Indirect,
    }];
    let volume = ProbeVolumeParams {
        origin: Vec3::new(-1.0, 0.5, -1.0),
        extent: Vec3::splat(2.0),
        resolution: [2, 2, 2],
    };
    let lit_input = s.input(volume, 0.0, 1 << 20);
    s.run(&lit_input, 6);
    let lit = s.irradiance(0, Vec3::NEG_Y).x;
    let want_lit = rho * (sun / std::f32::consts::PI + sky);
    assert!(
        (lit - want_lit).abs() < want_lit * 0.03,
        "lit: {lit} vs {want_lit}"
    );
    s.lights.clear();
    let dark_input = s.input(volume, 0.0, 1 << 20);
    s.run(&dark_input, 6);
    let dark = s.irradiance(0, Vec3::NEG_Y).x;
    let want_dark = rho * sky;
    assert!(
        (dark - want_dark).abs() < want_dark * 0.03,
        "the sun is off: {dark} vs {want_dark}"
    );
}

/// The same update is the same bytes: every ray's direction and seed come
/// from the frame, the probe and the ray -- never from which workgroup ran
/// first -- and a volume changed and changed back starts over (history,
/// round robin, frame count). The box's faces glow differently and
/// reflect nothing, so each update is a fresh estimate that depends on
/// where its rays went: two frames differ (the premise -- in a uniform
/// furnace every ray set gives the same answer, and determinism would be
/// unobservable), and the first frame after a restart equals the first
/// frame before it.
#[test]
fn realtime_updates_are_deterministic() {
    let mut s = GiScene::new(Vec3::ZERO);
    for (i, (min, u, v)) in [
        (Vec3::new(0.0, 0.0, 1.0), Vec3::X, Vec3::NEG_Z),
        (Vec3::new(0.0, 1.0, 0.0), Vec3::X, Vec3::Z),
        (Vec3::new(0.0, 0.0, 0.0), Vec3::Y, Vec3::Z),
        (Vec3::new(1.0, 0.0, 1.0), Vec3::Y, Vec3::NEG_Z),
        (Vec3::new(0.0, 0.0, 0.0), Vec3::X, Vec3::Y),
        (Vec3::new(1.0, 0.0, 1.0), Vec3::NEG_X, Vec3::Y),
    ]
    .into_iter()
    .enumerate()
    {
        s.add(
            quad_corners(min, u, v),
            Vec3::ZERO,
            Vec3::splat(0.2 * (i + 1) as f32),
        );
    }
    let volume = ProbeVolumeParams {
        resolution: [4, 4, 4],
        ..inner_volume()
    };
    let grid = |s: &GiScene| s.scene.surface.read_probe_grid_for_testing().coeffs[..64].to_vec();
    let input = s.input(volume, 0.0, 1 << 20);
    s.run(&input, 1);
    let first = grid(&s);
    s.run(&input, 1);
    assert_ne!(first, grid(&s), "premise: the ray set turns every frame");
    let other = s.input(inner_volume(), 0.0, 1 << 20);
    s.run(&other, 1);
    s.run(&input, 1);
    assert_eq!(first, grid(&s), "a restart replays the first frame exactly");
}

/// At the pixel: a realtime volume lights a surface. A white quad fills
/// the frame (the view-projection is the identity, so world xy is clip xy)
/// inside the volume, facing +z; a glowing static panel in front of it at
/// z = 2 is outside the clip volume, so the camera never sees it -- only
/// the probes do. Without the realtime volume the quad is its ambient;
/// with it, the panel's light is on it.
#[test]
fn a_realtime_volume_lights_the_pixels_it_covers() {
    let mut s = GiScene::new(Vec3::ZERO);
    s.scene.push_quad(
        [
            Vec3::new(-1.0, -1.0, 0.5),
            Vec3::new(1.0, -1.0, 0.5),
            Vec3::new(1.0, 1.0, 0.5),
            Vec3::new(-1.0, 1.0, 0.5),
        ],
        Vec3::ZERO,
    );
    let quad = s.scene.draw_calls.len() - 1;
    s.scene.draw_calls[quad].3.base_color = Vec3::ONE;
    // Facing -z, toward the quad.
    s.add(
        quad_corners(
            Vec3::new(5.0, -5.0, 2.0),
            Vec3::NEG_X * 10.0,
            Vec3::Y * 10.0,
        ),
        Vec3::ZERO,
        Vec3::splat(1.0),
    );
    let centre = |s: &GiScene| {
        let pixels = s.scene.surface.read_pixels().expect("offscreen readback");
        let at = (32 * 64 + 32) * 4;
        [pixels[at], pixels[at + 1], pixels[at + 2]]
    };
    s.scene.render(None).expect("the frame renders");
    let without = centre(&s);
    let volume = ProbeVolumeParams {
        origin: Vec3::new(-1.2, -1.2, 0.0),
        extent: Vec3::new(2.4, 2.4, 1.0),
        resolution: [2, 2, 2],
    };
    let input = s.input(volume, 0.0, 1 << 20);
    s.run(&input, 3);
    let with = centre(&s);
    assert!(
        without.iter().all(|&c| c < 200),
        "premise: room to brighten: {without:?}"
    );
    assert!(
        (0..3).all(|c| with[c] as i32 > without[c] as i32 + 30),
        "the panel's light reaches the quad through the probes: {without:?} -> {with:?}"
    );
    // And handed back: a frame with no realtime volume is the frame
    // before it.
    s.scene.render(None).expect("the frame renders");
    assert_eq!(
        centre(&s),
        without,
        "the grid is cleared when the volume goes"
    );
}

/// The editor's light-probe display shows a realtime volume's probes too
/// (`WgpuSurface::set_show_light_probes`; the baked ones are
/// `probe_display.rs`'s): a sphere on each lattice point while the volume
/// updates, lit by the panel the probes see -- where the same frame without
/// the display has only the background -- and nothing between them.
#[test]
fn the_probe_display_shows_a_realtime_volumes_probes() {
    let mut s = GiScene::new(Vec3::ZERO);
    // Facing -z, toward the volume, so its probes are lit.
    s.add(
        quad_corners(
            Vec3::new(5.0, -5.0, 2.0),
            Vec3::NEG_X * 10.0,
            Vec3::Y * 10.0,
        ),
        Vec3::ZERO,
        Vec3::splat(1.0),
    );
    // 2 x 2 x 2 probes, a unit apart: on screen (the identity camera) at
    // x, y = +-0.5, spheres 0.15 across.
    let volume = ProbeVolumeParams {
        origin: Vec3::new(-0.5, -0.5, 0.2),
        extent: Vec3::new(1.0, 1.0, 0.5),
        resolution: [2, 2, 2],
    };
    let input = s.input(volume, 0.0, 1 << 20);
    let at = |s: &GiScene, x: f32, y: f32| {
        let pixels = s.scene.surface.read_pixels().expect("offscreen readback");
        let (px, py) = (((x + 1.0) * 32.0) as usize, ((1.0 - y) * 32.0) as usize);
        let i = (py * 64 + px) * 4;
        [pixels[i], pixels[i + 1], pixels[i + 2]]
    };
    s.run(&input, 2);
    let corners = [(-0.5, -0.5), (0.5, -0.5), (-0.5, 0.5), (0.5, 0.5)];
    let background = at(&s, 0.0, 0.0);
    let without: Vec<[u8; 3]> = corners.iter().map(|&(x, y)| at(&s, x, y)).collect();
    assert!(
        without.iter().all(|&c| c == background),
        "premise: nothing but the background at the probes' places: {without:?}"
    );

    s.scene.surface.set_show_light_probes(true);
    s.run(&input, 1);
    for &(x, y) in &corners {
        let c = at(&s, x, y);
        assert!(
            c != background,
            "a sphere on the probe at ({x}, {y}): {c:?}, the background {background:?}"
        );
    }
    assert_eq!(at(&s, 0.0, 0.0), background, "and nothing between them");
}

/// Not a test: what a full-grid realtime update costs, for the PR.
/// `cargo test -p bsengine-rhi-wgpu --release --lib -- --ignored
/// --nocapture bench_realtime_gi` -- release, because a debug build's
/// validation layers dominate any GPU timing (CLAUDE.md).
#[test]
#[ignore]
fn bench_realtime_gi() {
    let mut s = furnace_scene(0.4, 0.5);
    // A few hundred triangles more inside the room, so the tree is not
    // trivially shallow: a grid of small panels on the floor.
    for i in 0..100 {
        let (x, z) = ((i % 10) as f32 * 0.1, (i / 10) as f32 * 0.1);
        s.add(
            quad_corners(
                Vec3::new(x, 0.1, z + 0.05),
                Vec3::X * 0.05,
                Vec3::NEG_Z * 0.05,
            ),
            Vec3::splat(0.6),
            Vec3::ZERO,
        );
    }
    // `BENCH_INSTANCES=n`: as many moving panels as well, one mesh shared, in
    // a row across the room above the floor -- what the instance list costs
    // every ray.
    let instances: usize = std::env::var("BENCH_INSTANCES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    for i in 0..instances {
        let x = 0.1 + 0.8 * i as f32 / instances.max(1) as f32;
        let model = Mat4::from_translation(Vec3::new(x, 0.5, 0.5));
        if i == 0 {
            s.add_dynamic(
                quad_corners(Vec3::splat(-0.02), Vec3::X * 0.04, Vec3::Y * 0.04),
                model,
                Vec3::splat(0.6),
                Vec3::ZERO,
            );
        } else {
            let first = s.dynamics[0];
            s.dynamics
                .push(bsengine_rhi_wgpu::realtime_gi::GiDynamic { model, ..first });
        }
    }
    // `BENCH_SKINNED=n`: as many skinned meshes, each its own -- a 32 x 32
    // grid, 2048 triangles, a character's worth -- small in the room and
    // posed anew before every update: what refitting their trees costs.
    let skinned: usize = std::env::var("BENCH_SKINNED")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let mut characters = Vec::new();
    for i in 0..skinned {
        const N: u32 = 32;
        let at = Vec3::new(0.2 + 0.6 * i as f32 / skinned.max(1) as f32, 0.3, 0.5);
        let mut vertices = Vec::new();
        for row in 0..=N {
            for col in 0..=N {
                let p = Vec3::new(col as f32 / N as f32, row as f32 / N as f32, 0.0) * 0.1;
                vertices.push(vertex_at(at + p, Vec3::Z));
            }
        }
        let mut indices = Vec::new();
        for row in 0..N {
            for col in 0..N {
                let a = row * (N + 1) + col;
                indices.extend([a, a + 1, a + N + 2, a, a + N + 2, a + N + 1]);
            }
        }
        let skin = vec![bound_to(0); vertices.len()];
        let mesh = s.add_skinned(&vertices, &skin, &indices, 1, Vec3::splat(0.6), Vec3::ZERO);
        s.pose(mesh, &[Mat4::IDENTITY]);
        characters.push(mesh);
    }
    s.lights = vec![bsengine_bake::BakeLight::Point {
        position: Vec3::splat(0.5),
        color: Vec3::ONE,
        intensity: 2.0,
        range: 3.0,
        contribution: bsengine_bake::Contribution::Indirect,
    }];
    let volume = ProbeVolumeParams {
        origin: Vec3::splat(0.05),
        extent: Vec3::splat(0.9),
        resolution: [16, 16, 16],
    };
    let mut input = s.input(volume, 0.97, 1 << 18);
    input.rays_per_probe = 64;
    // `BENCH_ADAPTIVE=1`: the same box as adaptive bricks, fine (0.033)
    // around the floor's panels and coarse (0.3) above -- what the brick walk
    // costs shading and the bounce, at the same ray budget.
    if std::env::var("BENCH_ADAPTIVE").is_ok() {
        input = adaptive(input, 0.1 / 3.0, 0.3);
    }
    s.run(&input, 3);
    let probes = s
        .scene
        .surface
        .realtime_gi_bricks_for_testing()
        .map_or(16 * 16 * 16, |l| l.probes.len());
    if let Some(l) = s.scene.surface.realtime_gi_bricks_for_testing() {
        eprintln!(
            "BENCH adaptive: {} probes in {} bricks (truncated {})",
            l.probes.len(),
            l.bricks.len(),
            l.truncated
        );
    }
    let n = 30;
    let device = s.scene.surface.device_arc();
    let t = std::time::Instant::now();
    for k in 0..n {
        for &mesh in &characters {
            let sway = Mat4::from_translation(Vec3::new(0.0, 0.01 * (k as f32).sin(), 0.0));
            s.pose(mesh, &[sway]);
        }
        s.run(&input, 1);
        let _ = device.poll(wgpu::PollType::wait_indefinitely());
    }
    eprintln!(
        "BENCH realtime gi: {probes} probes x 64 rays, {} triangles, {} instances: {:.2} ms per frame (update + frame)",
        s.statics.len() * 2,
        s.dynamics.len(),
        t.elapsed().as_secs_f64() * 1000.0 / n as f64
    );
}

/// The static set is read back once per change, and traced once it lands:
/// moving the floor away (a new model matrix) starts exactly one more
/// readback, and after it the probes see the sky where the floor was.
#[test]
fn a_changed_static_set_is_read_back_once_and_then_traced() {
    let sky = 0.2f32;
    let mut s = GiScene::new(Vec3::splat(sky));
    s.add(
        quad_corners(
            Vec3::new(-50.0, 0.0, 50.0),
            Vec3::X * 100.0,
            Vec3::NEG_Z * 100.0,
        ),
        Vec3::ZERO,
        Vec3::splat(2.0),
    );
    let volume = ProbeVolumeParams {
        origin: Vec3::new(-1.0, 0.5, -1.0),
        extent: Vec3::splat(2.0),
        resolution: [2, 2, 2],
    };
    let input = s.input(volume, 0.0, 1 << 20);
    s.run(&input, 3);
    assert_eq!(s.scene.surface.realtime_gi_readbacks(), 1);
    let glowing = s.irradiance(0, Vec3::NEG_Y).x;
    assert!(
        (glowing - 2.0).abs() < 0.05,
        "premise: the floor glows below: {glowing}"
    );
    // Far below and out of sight.
    s.statics[0].model = Mat4::from_translation(Vec3::new(0.0, -1.0e4, 0.0));
    let moved = s.input(volume, 0.0, 1 << 20);
    s.run(&moved, 3);
    s.run(&moved, 3);
    assert_eq!(
        s.scene.surface.realtime_gi_readbacks(),
        2,
        "one readback for the change, not one per frame"
    );
    let open = s.irradiance(0, Vec3::NEG_Y).x;
    assert!(
        (open - sky).abs() < 0.01,
        "the floor is gone, the sky remains: {open}"
    );
}

// ---- moving geometry -----------------------------------------------------

/// The grid of the floor tests: probes at y = 0.5 and 2.5.
fn floor_volume() -> ProbeVolumeParams {
    ProbeVolumeParams {
        origin: Vec3::new(-1.0, 0.5, -1.0),
        extent: Vec3::splat(2.0),
        resolution: [2, 2, 2],
    }
}

/// A 100 x 100 quad at y = 0 in its own space, facing +y.
fn floor_quad() -> [Vec3; 4] {
    quad_corners(
        Vec3::new(-50.0, 0.0, 50.0),
        Vec3::X * 100.0,
        Vec3::NEG_Z * 100.0,
    )
}

/// The floor quad turned over and lifted to y = 3: a ceiling, facing down.
fn ceiling_model() -> Mat4 {
    Mat4::from_translation(Vec3::new(0.0, 3.0, 0.0)) * Mat4::from_rotation_x(std::f32::consts::PI)
}

/// A moving renderer is traced where it is this frame, and moving it
/// rebuilds nothing. The static test above, with the floor an instance: it
/// glows below the probes, then moves far away and the sky shows where it
/// was -- with no readback beyond the first, which built its mesh's tree
/// once. (A static floor moved the same way costs a second readback and a
/// tree rebuilt over the whole static set.)
#[test]
fn a_moving_renderer_is_traced_where_it_is_with_nothing_rebuilt() {
    let sky = 0.2f32;
    let mut s = GiScene::new(Vec3::splat(sky));
    let floor = s.add_dynamic(floor_quad(), Mat4::IDENTITY, Vec3::ZERO, Vec3::splat(2.0));
    let input = s.input(floor_volume(), 0.0, 1 << 20);
    s.run(&input, 3);
    assert_eq!(
        s.scene.surface.realtime_gi_instances(),
        1,
        "premise: traced"
    );
    let glowing = s.irradiance(0, Vec3::NEG_Y).x;
    assert!(
        (glowing - 2.0).abs() < 0.05,
        "the moving floor glows below the probes: {glowing}"
    );
    s.dynamics[floor].model = Mat4::from_translation(Vec3::new(0.0, -1.0e4, 0.0));
    let moved = s.input(floor_volume(), 0.0, 1 << 20);
    s.run(&moved, 3);
    let open = s.irradiance(0, Vec3::NEG_Y).x;
    assert!(
        (open - sky).abs() < 0.01,
        "moved away, it leaves the sky where it was: {open}"
    );
    assert_eq!(
        s.scene.surface.realtime_gi_readbacks(),
        1,
        "moving an instance reads nothing back and rebuilds no tree"
    );
}

/// The transform is applied both ways -- the ray into the mesh's space, the
/// normal back out of it. The floor quad, turned over and lifted, is a
/// ceiling glowing down at the probes: they see it above (facing up, its
/// emission) and the sky below. Untransformed, the ray would meet the quad
/// where its mesh is, at y = 0 below the probes; with the normal left in
/// mesh space, the ceiling's underside would face away from them -- a back
/// face, which reads black.
#[test]
fn a_turned_and_lifted_renderer_faces_where_it_is_placed() {
    let sky = 0.2f32;
    let mut s = GiScene::new(Vec3::splat(sky));
    s.add_dynamic(floor_quad(), ceiling_model(), Vec3::ZERO, Vec3::splat(2.0));
    let input = s.input(floor_volume(), 0.0, 1 << 20);
    s.run(&input, 3);
    let up = s.irradiance(0, Vec3::Y).x;
    let down = s.irradiance(0, Vec3::NEG_Y).x;
    assert!((up - 2.0).abs() < 0.05, "the ceiling glows above: {up}");
    // 0.05, not the 0.01 the open-sky tests hold: here the sky below sits
    // under a hemisphere ten times brighter, and 256 rays project that step
    // to SH with about 1% of the contrast in error (0.187 measured). Either
    // failure this test is for is far outside it -- the untransformed quad
    // lies below the probes and reads 2 here, the untransformed normal reads
    // 0 above.
    assert!((down - sky).abs() < 0.05, "and the sky is below: {down}");
}

/// Distances in a scaled mesh's space are the world's: the ray's direction
/// goes in unnormalised, so the parameter of a hit there is the parameter in
/// the world, and an instance nearer than the static tree's hit wins. A
/// black slab, its mesh 200 wide scaled to 20, lies at y = 0.25 over a
/// static floor glowing at y = 0. From a probe at y = 0.5 the slab is 0.25
/// away and the floor 0.5: the slab hides the floor. (Renormalised, the
/// slab's parameter would read ten times its distance, 2.5 -- behind the
/// floor -- and the floor would show through.)
#[test]
fn a_scaled_renderer_hides_what_is_behind_it() {
    let mut s = GiScene::new(Vec3::splat(0.2));
    s.add(floor_quad(), Vec3::ZERO, Vec3::splat(2.0));
    let input = s.input(floor_volume(), 0.0, 1 << 20);
    s.run(&input, 3);
    let bare = s.irradiance(0, Vec3::NEG_Y).x;
    assert!(
        (bare - 2.0).abs() < 0.05,
        "premise: the floor glows below: {bare}"
    );
    s.add_dynamic(
        quad_corners(
            Vec3::new(-100.0, 0.0, 100.0),
            Vec3::X * 200.0,
            Vec3::NEG_Z * 200.0,
        ),
        Mat4::from_translation(Vec3::new(0.0, 0.25, 0.0)) * Mat4::from_scale(Vec3::splat(0.1)),
        Vec3::ZERO,
        Vec3::ZERO,
    );
    let covered = s.input(floor_volume(), 0.0, 1 << 20);
    s.run(&covered, 3);
    let hidden = s.irradiance(0, Vec3::NEG_Y).x;
    assert!(hidden < 0.05, "the slab hides the floor: {hidden}");
}

/// A moving renderer casts the shadows the probes see: the sun's shadow ray
/// at a hit point (`light.wgsl`'s `occluded`, which stops at the first hit)
/// meets instances as the probe rays do. The open floor of
/// `a_light_switched_off_leaves_the_probes` reflects 0.577 under the sun;
/// with a black ceiling above the probes, the floor below is in its shadow
/// and the probes see almost nothing there.
#[test]
fn a_moving_renderer_shadows_the_light_bounced_to_the_probes() {
    let (sky, rho, sun) = (0.2f32, 0.5f32, 3.0f32);
    let mut s = GiScene::new(Vec3::splat(sky));
    s.add(floor_quad(), Vec3::splat(rho), Vec3::ZERO);
    s.lights = vec![bsengine_bake::BakeLight::Directional {
        direction: Vec3::NEG_Y,
        color: Vec3::splat(sun),
        contribution: bsengine_bake::Contribution::Indirect,
    }];
    let open = s.input(floor_volume(), 0.0, 1 << 20);
    s.run(&open, 6);
    let lit = s.irradiance(0, Vec3::NEG_Y).x;
    let want_lit = rho * (sun / std::f32::consts::PI + sky);
    assert!(
        (lit - want_lit).abs() < want_lit * 0.03,
        "premise: the open floor is lit: {lit} vs {want_lit}"
    );
    s.add_dynamic(floor_quad(), ceiling_model(), Vec3::ZERO, Vec3::ZERO);
    let roofed = s.input(floor_volume(), 0.0, 1 << 20);
    s.run(&roofed, 6);
    let shaded = s.irradiance(0, Vec3::NEG_Y).x;
    assert!(
        shaded < 0.1 * want_lit,
        "under the ceiling, the floor is in shadow: {shaded}"
    );
}

/// One mesh drawn by two moving entities is one tree, read back once, and
/// each entity bounces its own material: the floor quad below glowing 2, the
/// same quad above as a ceiling glowing 1. The triangles are shared; the
/// colour comes from the instance a ray met them through.
#[test]
fn renderers_sharing_a_mesh_share_its_tree_and_keep_their_materials() {
    let mut s = GiScene::new(Vec3::ZERO);
    s.add_dynamic(floor_quad(), Mat4::IDENTITY, Vec3::ZERO, Vec3::splat(2.0));
    let mesh_id = s.dynamics[0].mesh_id;
    s.dynamics.push(bsengine_rhi_wgpu::realtime_gi::GiDynamic {
        mesh_id,
        model: ceiling_model(),
        albedo: Vec3::ZERO,
        emissive: Vec3::splat(1.0),
    });
    let input = s.input(floor_volume(), 0.0, 1 << 20);
    s.run(&input, 3);
    assert_eq!(s.scene.surface.realtime_gi_instances(), 2, "premise: both");
    let down = s.irradiance(0, Vec3::NEG_Y).x;
    let up = s.irradiance(0, Vec3::Y).x;
    assert!((down - 2.0).abs() < 0.05, "the floor's own glow: {down}");
    assert!((up - 1.0).abs() < 0.05, "the ceiling's own glow: {up}");
    assert_eq!(s.scene.surface.realtime_gi_readbacks(), 1);
}

/// A white quad filling the frame at z = 0.5, facing the camera (+z), and
/// a 2 x 2 x 2 realtime grid around it from z = 0 to z = 1.6: the scene of
/// the two visibility tests. Returns the centre pixel after a few updates
/// with visibility on or off.
fn visibility_scene(s: &mut GiScene, visibility: bool, relocation: bool) -> [u8; 3] {
    // z from 0 to 1.6: the probes at z = 1.6 are beyond the wall (at 1.2),
    // and the cell is shallow enough that a surface's bias (0.3 of the 1.6
    // spacing, 0.48) keeps the quad's lookup point in front of the wall
    // rather than through it.
    let volume = ProbeVolumeParams {
        origin: Vec3::new(-1.2, -1.2, 0.0),
        extent: Vec3::new(2.4, 2.4, 1.6),
        resolution: [2, 2, 2],
    };
    let input = s.input(volume, 0.0, 1 << 20);
    s.run(&input, 1);
    s.scene
        .surface
        .set_realtime_gi_visibility_for_testing(visibility);
    s.scene
        .surface
        .set_realtime_gi_relocation_for_testing(relocation);
    s.run(&input, 4);
    let pixels = s.scene.surface.read_pixels().expect("offscreen readback");
    let at = (32 * 64 + 32) * 4;
    [pixels[at], pixels[at + 1], pixels[at + 2]]
}

fn white_quad_in_view(s: &mut GiScene) {
    s.scene.push_quad(
        [
            Vec3::new(-1.0, -1.0, 0.5),
            Vec3::new(1.0, -1.0, 0.5),
            Vec3::new(1.0, 1.0, 0.5),
            Vec3::new(-1.0, 1.0, 0.5),
        ],
        Vec3::ZERO,
    );
    let quad = s.scene.draw_calls.len() - 1;
    s.scene.draw_calls[quad].3.base_color = Vec3::ONE;
}

/// sRGB-encoded 8-bit to linear, to compare light rather than encodings.
fn linear(c: u8) -> f32 {
    let c = f32::from(c) / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// The leak DDGI's visibility exists for, at the proportions it is built
/// for: two rooms on a floor, a wall between them at x = 0, probes a metre
/// apart with the wall halfway between two of them, and a glowing panel in
/// the right-hand room -- emissive, so only GI carries its light anywhere.
/// The camera looks down at the left room's floor 0.3 m from the wall,
/// where trilinear alone takes a fifth of its light from the probes beyond
/// the wall (the premise: without visibility, it does leak). Nearer the wall
/// than that, DDGI's test cannot fully tell the wall from the floor -- at
/// 0.1 m the probes beyond recorded the wall at 0.61 and see the floor at
/// 0.75, and a seventh of the leak got through -- the known price of probes
/// a metre apart. With
/// visibility those probes recorded the wall nearer than the floor and drop
/// out, in shading and in the bounce: what reaches the floor is under a tenth
/// of the leak. Both measured as light added over the same scene with the
/// panel dark (the harness's sun lights the floor either way).
#[test]
fn visibility_stops_light_leaking_through_a_wall() {
    let build = |panel: f32| {
        let mut s = GiScene::new(Vec3::ZERO);
        // The floor, facing up, drawn and static.
        let floor = quad_corners(Vec3::new(-3.0, 0.0, 3.0), Vec3::X * 6.0, Vec3::NEG_Z * 6.0);
        s.add(floor, Vec3::splat(0.5), Vec3::ZERO);
        let i = s.scene.draw_calls.len() - 1;
        s.scene.draw_calls[i].3.base_color = Vec3::ONE;
        // Both faces of the wall, a hair apart, so each side is a front.
        s.add(
            quad_corners(Vec3::new(0.01, 0.0, 3.0), Vec3::NEG_Z * 6.0, Vec3::Y * 3.0),
            Vec3::splat(0.5),
            Vec3::ZERO,
        );
        s.add(
            quad_corners(Vec3::new(-0.01, 0.0, -3.0), Vec3::Z * 6.0, Vec3::Y * 3.0),
            Vec3::splat(0.5),
            Vec3::ZERO,
        );
        // The panel: the right room's ceiling at y = 2, facing down.
        s.add(
            quad_corners(Vec3::new(0.2, 2.0, -3.0), Vec3::X * 2.8, Vec3::Z * 6.0),
            Vec3::ZERO,
            Vec3::splat(panel),
        );
        s.scene.camera = (
            Mat4::perspective_rh(0.8, 1.0, 0.05, 50.0)
                * Mat4::look_at_rh(
                    Vec3::new(-0.3, 1.5, 0.6),
                    Vec3::new(-0.3, 0.0, 0.0),
                    Vec3::Y,
                ),
            Vec3::new(-0.3, 1.5, 0.6),
        );
        s
    };
    let shade = |s: &mut GiScene, visibility: bool| {
        // Probes at x = -1.5, -0.5, 0.5, 1.5: the wall halfway between the
        // middle two. y from -0.25 (under the floor) to 1.75, z -1 to 1.
        let volume = ProbeVolumeParams {
            origin: Vec3::new(-1.5, -0.25, -1.0),
            extent: Vec3::new(3.0, 2.0, 2.0),
            resolution: [4, 3, 3],
        };
        let input = s.input(volume, 0.0, 1 << 20);
        s.run(&input, 1);
        s.scene
            .surface
            .set_realtime_gi_visibility_for_testing(visibility);
        s.run(&input, 6);
        let pixels = s.scene.surface.read_pixels().expect("offscreen readback");
        let at = (32 * 64 + 32) * 4;
        pixels[at]
    };
    let base = shade(&mut build(0.0), true);
    let leaked = shade(&mut build(4.0), false);
    let seen = shade(&mut build(4.0), true);
    let leak = linear(leaked) - linear(base);
    let through = linear(seen) - linear(base);
    assert!(
        leak > 0.05,
        "premise: without visibility the panel leaks under the wall: {base} -> {leaked}"
    );
    assert!(
        through < 0.1 * leak,
        "visibility drops the probes beyond the wall: added {through} against a leak of          {leak} (pixels {base}, {leaked}, {seen})"
    );
}

/// A closed room's wall is lit as a furnace's is: evenly, and at the light
/// every point there sees. Every wall of a 6 x 6 x 6 box glows and reflects
/// alike, and the volume is a little larger than the box, as in
/// games/gi-demo: a layer of probes stands 0.1 behind the back wall, seeing
/// only its back, and black. The camera looks at that wall (the
/// view-projection is the identity, the wall at z = 0.5), whose lookups
/// straddle those black probes and the lit ones in front. Against the same
/// wall shaded by a volume with no probes behind it, the row must match --
/// letting the black probes in darkens it -- and be flat across the probe
/// cells: with the bias at 0.1 of the spacing, the demo's walls showed one
/// bright patch per probe.
#[test]
fn a_closed_room_is_lit_evenly_through_the_volume() {
    let row_with = |origin_z: f32| {
        let mut s = GiScene::new(Vec3::ZERO);
        let (e, rho) = (0.15f32, 0.5f32);
        let (lo, hi) = (Vec3::new(-3.0, -3.0, 0.5), Vec3::new(3.0, 3.0, 6.5));
        let size = hi - lo;
        for (min, u, v) in [
            // The back wall first: the one drawn, facing +z, at z = 0.5.
            (lo, Vec3::X * size.x, Vec3::Y * size.y),
            (
                Vec3::new(hi.x, lo.y, hi.z),
                Vec3::NEG_X * size.x,
                Vec3::Y * size.y,
            ),
            (lo, Vec3::Y * size.y, Vec3::Z * size.z),
            (
                Vec3::new(hi.x, lo.y, hi.z),
                Vec3::Y * size.y,
                Vec3::NEG_Z * size.z,
            ),
            (
                Vec3::new(lo.x, lo.y, hi.z),
                Vec3::X * size.x,
                Vec3::NEG_Z * size.z,
            ),
            (
                Vec3::new(lo.x, hi.y, lo.z),
                Vec3::X * size.x,
                Vec3::Z * size.z,
            ),
        ] {
            s.add(quad_corners(min, u, v), Vec3::splat(rho), Vec3::splat(e));
        }
        // Drawn as the trace sees it; the other walls lie outside the frame.
        let back = s.scene.draw_calls.len() - 6;
        s.scene.draw_calls[back].3.base_color = Vec3::splat(rho);
        s.scene.draw_calls[back].3.emissive = Vec3::splat(e);
        s.scene.sun = Some((Vec3::ZERO, Vec3::ZERO));
        let volume = ProbeVolumeParams {
            origin: Vec3::new(-3.1, -3.1, origin_z),
            extent: Vec3::new(6.2, 6.2, 6.6 - origin_z),
            resolution: [8, 8, 8],
        };
        let input = s.input(volume, 0.5, 1 << 20);
        s.run(&input, 24);
        let pixels = s.scene.surface.read_pixels().expect("offscreen readback");
        // The middle row, all but the edge pixels: across six probe cells
        // (0.89 wide). It used to stop at 17..47, because SSAO darkened every
        // flat surface except a band along the frame's edge, where its
        // samples left the screen -- a step of 16 levels at pixel 14 that
        // looked like a tiling of the frame and was not GI.
        (1..63)
            .map(|x| pixels[(32 * 64 + x) * 4])
            .collect::<Vec<u8>>()
    };
    // Probes from 0.1 behind the wall, as in the demo; and, for reference,
    // from the wall itself -- every probe in the room.
    let row = row_with(0.4);
    let reference = row_with(0.5);
    let (lo, hi) = (*row.iter().min().unwrap(), *row.iter().max().unwrap());
    let r = reference[reference.len() / 2];
    assert!(r > 60, "premise: the wall is lit: {reference:?}");
    assert!(
        hi - lo <= 4,
        "inside a furnace every point sees the same light; the wall varies {lo}..{hi}: {row:?}"
    );
    assert!(
        row.iter().all(|&p| (p as i32 - r as i32).abs() <= 4),
        "the probes behind the wall must not darken it: {row:?} against {r}"
    );
}

/// A probe buried inside geometry sees only back faces: black, and close.
/// The grid's near probes (z = 0) each sit inside a small closed box; the
/// far ones see a glowing panel. Trilinear alone lets the buried probes
/// darken the quad (most of its weight is theirs: the premise); with
/// visibility they drop out, and the quad takes the far probes' light.
#[test]
fn a_buried_probe_drops_out_of_the_lookup() {
    let build = || {
        let mut s = GiScene::new(Vec3::ZERO);
        white_quad_in_view(&mut s);
        let inward = [
            (Vec3::new(0.0, 0.0, 1.0), Vec3::X, Vec3::NEG_Z),
            (Vec3::new(0.0, 1.0, 0.0), Vec3::X, Vec3::Z),
            (Vec3::new(0.0, 0.0, 0.0), Vec3::Y, Vec3::Z),
            (Vec3::new(1.0, 0.0, 1.0), Vec3::Y, Vec3::NEG_Z),
            (Vec3::new(0.0, 0.0, 0.0), Vec3::X, Vec3::Y),
            (Vec3::new(1.0, 0.0, 1.0), Vec3::NEG_X, Vec3::Y),
        ];
        for (x, y) in [(-1.2f32, -1.2f32), (1.2, -1.2), (-1.2, 1.2), (1.2, 1.2)] {
            let (c, h) = (Vec3::new(x, y, 0.0), 0.05f32);
            for (min, u, v) in inward {
                // Swapping u and v turns each face outward: a solid box.
                s.add(
                    quad_corners(c - Vec3::splat(h) + min * 2.0 * h, v * 2.0 * h, u * 2.0 * h),
                    Vec3::splat(0.5),
                    Vec3::ZERO,
                );
            }
        }
        s.add(
            quad_corners(
                Vec3::new(10.0, -10.0, 3.0),
                Vec3::NEG_X * 20.0,
                Vec3::Y * 20.0,
            ),
            Vec3::ZERO,
            // Half what it was, and the harness sun off: the quad is lit by
            // the probes alone and sits mid-range, where a change shows. At
            // 2.0 with the sun on it read 223 -> 248, against the tonemap's
            // ceiling -- the margin it had came from SSAO darkening the flat
            // quad, which it no longer does.
            Vec3::splat(1.0),
        );
        s.scene.sun = Some((Vec3::ZERO, Vec3::ZERO));
        s
    };
    // Relocation off: it would move these probes out of boxes this small,
    // and the test is about what visibility does with a probe that stays in.
    let darkened = visibility_scene(&mut build(), false, false);
    let seen = visibility_scene(&mut build(), true, false);
    assert!(
        (0..3).all(|c| seen[c] as i32 > darkened[c] as i32 + 40),
        "the buried probes drop out and the far ones light the quad: {darkened:?} -> {seen:?}"
    );
}

/// The furnace room of the test above, but seen whole: from inside, the back
/// wall to its edges, where it meets the floor, the ceiling and the side walls.
/// The probes stand 0.1 behind every wall, as in games/gi-demo. Returns the
/// back wall's pixels (red channel), all but its outermost ring.
/// A closed furnace room, x and y from -3 to 3, z from 0.5 to 6.5, every
/// wall glowing 0.15 with albedo 0.5; the back wall (z = 0.5, facing +z) is
/// the one drawn.
fn furnace_room() -> GiScene {
    let mut s = GiScene::new(Vec3::ZERO);
    let (e, rho) = (0.15f32, 0.5f32);
    let (lo, hi) = (Vec3::new(-3.0, -3.0, 0.5), Vec3::new(3.0, 3.0, 6.5));
    let size = hi - lo;
    for (min, u, v) in [
        (lo, Vec3::X * size.x, Vec3::Y * size.y),
        (
            Vec3::new(hi.x, lo.y, hi.z),
            Vec3::NEG_X * size.x,
            Vec3::Y * size.y,
        ),
        (lo, Vec3::Y * size.y, Vec3::Z * size.z),
        (
            Vec3::new(hi.x, lo.y, hi.z),
            Vec3::Y * size.y,
            Vec3::NEG_Z * size.z,
        ),
        (
            Vec3::new(lo.x, lo.y, hi.z),
            Vec3::X * size.x,
            Vec3::NEG_Z * size.z,
        ),
        (
            Vec3::new(lo.x, hi.y, lo.z),
            Vec3::X * size.x,
            Vec3::Z * size.z,
        ),
    ] {
        s.add(quad_corners(min, u, v), Vec3::splat(rho), Vec3::splat(e));
    }
    let back = s.scene.draw_calls.len() - 6;
    s.scene.draw_calls[back].3.base_color = Vec3::splat(rho);
    s.scene.draw_calls[back].3.emissive = Vec3::splat(e);
    s.scene.sun = Some((Vec3::ZERO, Vec3::ZERO));
    s
}

fn whole_back_wall(relocation: bool) -> Vec<f32> {
    let mut s = furnace_room();
    let eye = Vec3::new(0.0, 0.0, 6.2);
    s.scene.camera = (
        Mat4::perspective_rh(1.2, 1.0, 0.05, 50.0)
            * Mat4::look_at_rh(eye, Vec3::new(0.0, 0.0, 0.0), Vec3::Y),
        eye,
    );
    let volume = ProbeVolumeParams {
        origin: Vec3::new(-3.1, -3.1, 0.4),
        extent: Vec3::new(6.2, 6.2, 6.2),
        resolution: [8, 8, 8],
    };
    let input = s.input(volume, 0.5, 1 << 20);
    s.run(&input, 1);
    s.scene
        .surface
        .set_realtime_gi_relocation_for_testing(relocation);
    s.run(&input, 32);
    let pixels = s.scene.surface.read_pixels().expect("offscreen readback");
    // The wall's pixels by distance from its edge, in pixels: the mean of
    // each ring, 1 to 8 pixels in, and of everything further in.
    let at = |x: usize, y: usize| pixels[(y * 64 + x) * 4];
    // The back wall is the only face drawn with an albedo; the other five
    // are black and show only the probes' specular sheen, a few levels that
    // vary with the view angle -- so "brighter than them by a margin", not
    // "different from the corner pixel", which counted the side walls as
    // back wall once that sheen existed.
    let bg = at(0, 0);
    let wall = |v: u8| v as i32 > bg as i32 + 40;
    let rows: Vec<usize> = (0..64).filter(|&y| wall(at(32, y))).collect();
    let cols: Vec<usize> = (0..64).filter(|&x| wall(at(x, 32))).collect();
    let (x0, x1, y0, y1) = (cols[0], cols[cols.len() - 1], rows[0], rows[rows.len() - 1]);
    let mut sums = [(0u32, 0u32); 10];
    for y in y0..=y1 {
        for x in x0..=x1 {
            let d = (x - x0).min(x1 - x).min(y - y0).min(y1 - y).min(9);
            sums[d].0 += at(x, y) as u32;
            sums[d].1 += 1;
        }
    }
    sums.iter()
        .map(|&(s, n)| s as f32 / n.max(1) as f32)
        .collect()
}

/// Relocation: a furnace room is as bright at its edges as in its middle.
/// The probes on the volume's boundary stand 0.1 behind the walls, as in
/// games/gi-demo; visibility drops them, and near an edge of the wall the
/// surviving probes are too few -- a dark band ran round every wall, notched
/// once per probe (the demo's "wavy bands"). Moved in front of the walls they
/// light it like the rest. Measured as the mean of the ring one pixel in from
/// the wall's edge (0.12 m) against the middle: 38 below it without
/// relocation (the premise, from a frame with relocation off), 5 with.
#[test]
fn a_closed_room_is_lit_evenly_to_its_edges() {
    let fixed = whole_back_wall(false);
    assert!(
        fixed[1] < fixed[9] - 20.0,
        "premise: without relocation a dark band runs round the wall: {fixed:.1?}"
    );
    let moved = whole_back_wall(true);
    assert!(
        (moved[1] - moved[9]).abs() <= 12.0,
        "with the probes moved in front of the walls, the edge matches the middle: {moved:.1?}"
    );
}

/// A probe behind a one-sided wall, 0.1 from it, sees the wall's back faces
/// in half its relocation rays: inside something, as far as it can tell. It
/// moves through the nearest one and a little beyond -- to in front of the
/// wall, within 0.45 of the spacing of its lattice point -- while a probe
/// with nothing near it stays put.
#[test]
fn a_probe_behind_a_wall_is_moved_in_front_of_it() {
    let mut s = GiScene::new(Vec3::splat(0.2));
    // Facing +z, at z = 0.1: the probes at z = 0 are behind it.
    s.add(
        quad_corners(Vec3::new(-5.0, -5.0, 0.1), Vec3::X * 10.0, Vec3::Y * 10.0),
        Vec3::splat(0.5),
        Vec3::ZERO,
    );
    // Spacing 1.2 across, 0.8 in depth: z = 0, 0.8 and 1.6.
    let volume = ProbeVolumeParams {
        origin: Vec3::new(-1.2, -1.2, 0.0),
        extent: Vec3::new(2.4, 2.4, 1.6),
        resolution: [3, 3, 3],
    };
    // A third of the probes a frame, so the probes the last frame did not
    // update keep their places only by being carried from half to half.
    let input = s.input(volume, 0.5, (256 + 32) * 9);
    s.run(&input, 24);
    let grid = s.scene.surface.read_probe_grid_for_testing();
    for i in 0..27 {
        let z = i / 9;
        let o = grid.offsets[i];
        if z == 0 {
            // Along the fixed ray nearest the wall's normal, so not exactly
            // straight: in front of the wall, and inside the box relocation
            // keeps a probe to (0.45 of the spacing on each axis).
            assert!(
                o[2] > 0.1
                    && o[2] <= 0.45 * 0.8
                    && o[0].abs() <= 0.45 * 1.2
                    && o[1].abs() <= 0.45 * 1.2,
                "probe {i}, 0.1 behind the wall, should have moved through it: {o:?}"
            );
        } else {
            assert!(
                o[0] == 0.0 && o[1] == 0.0 && o[2] == 0.0,
                "probe {i}, with the wall 0.7 or more away, should stay put: {o:?}"
            );
        }
    }
    // The wall taken away, three metres back: nothing near any probe now,
    // and each drifts back to its lattice point.
    s.statics[0].model = Mat4::from_translation(Vec3::new(0.0, 0.0, -3.0));
    let input = s.input(volume, 0.5, 1 << 20);
    s.run(&input, 8);
    let grid = s.scene.surface.read_probe_grid_for_testing();
    for i in 0..27 {
        let o = grid.offsets[i];
        assert!(
            o[0].abs() < 1e-4 && o[1].abs() < 1e-4 && o[2].abs() < 1e-4,
            "probe {i}, with the wall gone, should be back on its lattice point: {o:?}"
        );
    }
}

/// A probe deeper inside geometry than relocation may move it -- 0.45 of the
/// spacing -- stays on its lattice point rather than ending up somewhere in
/// the middle of the solid.
#[test]
fn a_probe_too_deep_inside_stays_where_it_is() {
    let mut s = GiScene::new(Vec3::splat(0.2));
    // A solid slab from z = -1 to z = 0.5, its faces outward: the probes at
    // z = 0 are 0.5 below its top, beyond the 0.36 they may move.
    let (c, h) = (Vec3::new(0.0, 0.0, -0.25), Vec3::new(5.0, 5.0, 0.75));
    let inward = [
        (Vec3::new(0.0, 0.0, 1.0), Vec3::X, Vec3::NEG_Z),
        (Vec3::new(0.0, 1.0, 0.0), Vec3::X, Vec3::Z),
        (Vec3::new(0.0, 0.0, 0.0), Vec3::Y, Vec3::Z),
        (Vec3::new(1.0, 0.0, 1.0), Vec3::Y, Vec3::NEG_Z),
        (Vec3::new(0.0, 0.0, 0.0), Vec3::X, Vec3::Y),
        (Vec3::new(1.0, 0.0, 1.0), Vec3::NEG_X, Vec3::Y),
    ];
    for (min, u, v) in inward {
        s.add(
            quad_corners(c - h + min * 2.0 * h, v * 2.0 * h, u * 2.0 * h),
            Vec3::splat(0.5),
            Vec3::ZERO,
        );
    }
    let volume = ProbeVolumeParams {
        origin: Vec3::new(-1.2, -1.2, 0.0),
        extent: Vec3::new(2.4, 2.4, 1.6),
        resolution: [3, 3, 3],
    };
    let input = s.input(volume, 0.5, 1 << 20);
    s.run(&input, 8);
    let grid = s.scene.surface.read_probe_grid_for_testing();
    for i in 0..9 {
        let o = grid.offsets[i];
        assert!(
            o[0] == 0.0 && o[1] == 0.0 && o[2] == 0.0,
            "probe {i}, 0.5 deep, should not have moved: {o:?}"
        );
    }
}

/// A probe too near a surface moves away from it. In a closed room, a probe
/// 0.05 in front of the back wall -- nearer than a quarter of the spacing --
/// moves toward the room's most open direction, away from the wall, until it
/// stands about that quarter-spacing off. (A probe that near reads the wall's
/// own shading through its moments, and leaks.)
#[test]
fn a_probe_too_near_a_surface_moves_away_from_it() {
    let mut s = furnace_room();
    // The first layer 0.05 in front of the back wall (z = 0.5).
    let volume = ProbeVolumeParams {
        origin: Vec3::new(-2.0, -2.0, 0.55),
        extent: Vec3::new(4.0, 4.0, 4.0),
        resolution: [3, 3, 3],
    };
    let input = s.input(volume, 0.5, 1 << 20);
    s.run(&input, 8);
    let grid = s.scene.surface.read_probe_grid_for_testing();
    for i in 0..9 {
        let o = grid.offsets[i];
        assert!(
            o[2] >= 0.1,
            "probe {i}, 0.05 off the wall, should have moved away from it: {o:?}"
        );
    }
}

/// A probe moved out of a solid is traced from where it moved to. A solid
/// cube stands in the furnace room with lattice probes inside it, near its
/// faces; relocation moves them out, and from there they see the room. In a
/// furnace every surface sees the same light, so the cube's front face must
/// read flat and as bright as the room's walls. Traced from their lattice
/// points instead -- inside the cube, seeing only its back faces -- those
/// probes carry no light, and the face reads 158 against walls of 132.
///
/// What a furnace cannot show is shading's own use of the moved position (the
/// direction and distance to the probe in the visibility weights): every probe
/// here holds the same light, so how they are weighted changes nothing. Its
/// effect was measured on games/gi-demo instead: with shading looking for the
/// probes at their lattice points, the boulder's lower front came out
/// blotched (up to 54 levels off); three harness scenes -- this cube, lit
/// from one side, resting on the floor -- moved by 2 levels at most.
#[test]
fn a_surface_is_lit_by_the_probes_moved_out_from_behind_it() {
    let mut s = furnace_room();
    let (e, rho) = (0.15f32, 0.5f32);
    // Half-size 0.6 round (0.003, 0.003, 3.2): the lattice point (0.443,
    // 0.443, 3.057) is inside, 0.16 from the top and the right faces --
    // close enough to move out within 0.45 of the spacing (0.40).
    let (c, h) = (Vec3::new(0.003, 0.003, 3.2), Vec3::splat(0.6));
    let inward = [
        (Vec3::new(0.0, 0.0, 1.0), Vec3::X, Vec3::NEG_Z),
        (Vec3::new(0.0, 1.0, 0.0), Vec3::X, Vec3::Z),
        (Vec3::new(0.0, 0.0, 0.0), Vec3::Y, Vec3::Z),
        (Vec3::new(1.0, 0.0, 1.0), Vec3::Y, Vec3::NEG_Z),
        (Vec3::new(0.0, 0.0, 0.0), Vec3::X, Vec3::Y),
        (Vec3::new(1.0, 0.0, 1.0), Vec3::NEG_X, Vec3::Y),
    ];
    for (min, u, v) in inward {
        s.add(
            quad_corners(c - h + min * 2.0 * h, v * 2.0 * h, u * 2.0 * h),
            Vec3::splat(rho),
            Vec3::splat(e),
        );
        let i = s.scene.draw_calls.len() - 1;
        s.scene.draw_calls[i].3.base_color = Vec3::splat(rho);
        s.scene.draw_calls[i].3.emissive = Vec3::splat(e);
    }
    let eye = Vec3::new(0.003, 0.003, 6.2);
    s.scene.camera = (
        Mat4::perspective_rh(1.2, 1.0, 0.05, 50.0)
            * Mat4::look_at_rh(eye, Vec3::new(0.003, 0.003, 0.0), Vec3::Y),
        eye,
    );
    let volume = ProbeVolumeParams {
        origin: Vec3::new(-3.1, -3.1, 0.4),
        extent: Vec3::new(6.2, 6.2, 6.2),
        resolution: [8, 8, 8],
    };
    let input = s.input(volume, 0.5, 1 << 20);
    s.run(&input, 32);
    let grid = s.scene.surface.read_probe_grid_for_testing();
    // Premise: a probe was moved out of the cube -- lattice (4, 4, 3).
    let moved = grid.offsets[(3 * 8 + 4) * 8 + 4];
    assert!(
        moved[0].abs() + moved[1].abs() + moved[2].abs() > 0.1,
        "premise: the probe inside the cube moved out: {moved:?}"
    );
    let pixels = s.scene.surface.read_pixels().expect("offscreen readback");
    let at = |x: usize, y: usize| pixels[(y * 64 + x) * 4];
    // The front face (z = 3.8, 2.4 from the eye) spans pixels 21..43; the
    // part well inside it.
    let face: Vec<u8> = (24..41)
        .flat_map(|y| (24..41).map(move |x| (x, y)))
        .map(|(x, y)| at(x, y))
        .collect();
    let (lo, hi) = (*face.iter().min().unwrap(), *face.iter().max().unwrap());
    // The room's own walls in a furnace, measured by the test above: 174.
    let wall = whole_back_wall(true)[9];
    assert!(
        hi - lo <= 6 && (lo as f32 - wall).abs() <= 10.0,
        "the cube's face should be flat and as bright as the walls ({wall:.0}): {lo}..{hi}"
    );
}

// ---- adaptive bricks -------------------------------------------------------

/// `input` made adaptive, with these spacings.
fn adaptive(
    mut input: bsengine_rhi_wgpu::realtime_gi::RealtimeGiInput,
    min: f32,
    max: f32,
) -> bsengine_rhi_wgpu::realtime_gi::RealtimeGiInput {
    input.adaptive = Some(bsengine_rhi_wgpu::realtime_gi::AdaptiveSpacing { min, max });
    input
}

/// The shader finds a point's eight probes where the layout says they are:
/// for points all through a three-level layout, the GPU's walk down the
/// brick tree (`probe_cell`, `probe_cell_index`) gives the corners
/// `BrickLayout::locate` and the brick's own table give, and the same place
/// in the cell. A point on a cell's face could go either way by a rounding,
/// on either side; those are left out.
#[test]
fn the_shader_finds_an_adaptive_volumes_probes_as_the_layout_does() {
    let mut s = GiScene::new(Vec3::ZERO);
    // A patch of floor inside one of the 1.5 m bricks (and clear of its
    // neighbours' reach): cut to the finest level there, 27 bricks of 0.5,
    // and within the budget -- a floor across the whole box would need some
    // 7800 probes at that level, and the level would not be cut at all.
    s.add(
        quad_corners(Vec3::new(0.3, 2.3, 1.1), Vec3::X * 0.8, Vec3::NEG_Z * 0.8),
        Vec3::splat(0.5),
        Vec3::ZERO,
    );
    let volume = ProbeVolumeParams {
        origin: Vec3::ZERO,
        extent: Vec3::new(4.5, 9.0, 4.5),
        resolution: [2, 2, 2],
    };
    let input = adaptive(s.input(volume, 0.0, 1 << 20), 1.5 / 9.0, 1.5);
    s.run(&input, 1);
    let layout = s
        .scene
        .surface
        .realtime_gi_bricks_for_testing()
        .expect("the volume is adaptive");
    let deepest = layout.bricks.iter().map(|b| b.level).max().unwrap();
    assert_eq!(deepest, 2, "premise: three levels in use");
    assert!(
        layout.bricks.iter().any(|b| b.level == 0),
        "premise: and a coarse brick away from the floor"
    );

    let mut state = 11u32;
    let mut next = || {
        state = state.wrapping_mul(1664525).wrapping_add(1013904223);
        (state >> 8) as f32 / (1u32 << 24) as f32
    };
    let mut points = Vec::new();
    let mut want = Vec::new();
    while points.len() < 4000 {
        let p = Vec3::new(next(), next(), next()) * volume.extent;
        let (b, local) = layout.locate(p).unwrap();
        if (local - local.round()).abs().min_element() < 1e-3 {
            continue;
        }
        let brick = &layout.bricks[b as usize];
        let i0 = local.floor().clamp(Vec3::ZERO, Vec3::splat(2.0));
        let corners: [u32; 8] = std::array::from_fn(|c| {
            let c = c as u32;
            let ci = i0.as_uvec3() + glam::UVec3::new(c & 1, (c >> 1) & 1, c >> 2);
            brick.probes[(ci.x + 4 * (ci.y + 4 * ci.z)) as usize]
        });
        points.push(p);
        want.push((corners, local - i0, brick.level));
    }
    let got = s.scene.surface.probe_cells_for_testing(&points);
    let mut levels = [0u32; 3];
    for ((p, (corners, frac, level)), (g_corners, g_frac)) in points.iter().zip(&want).zip(&got) {
        assert_eq!(g_corners, corners, "{p}: corners");
        assert!(
            (*g_frac - *frac).abs().max_element() < 1e-3,
            "{p}: {g_frac} vs {frac}"
        );
        levels[*level as usize] += 1;
    }
    assert!(
        levels.iter().all(|&n| n > 0),
        "every level was looked up: {levels:?}"
    );
}

/// The furnace again, through bricks: a volume filling the closed box, cut
/// finer next to its walls, every probe at `e / (1 - rho)` after a dozen
/// updates -- each traced from where the brick table puts it. A probe traced
/// from anywhere else (its index read as a grid's, say) sits on a wall or
/// outside the box, and sees the sky of 100.
#[test]
fn adaptive_probes_in_a_furnace_follow_the_geometric_series() {
    let (e, rho) = (0.4f32, 0.5f32);
    let mut s = furnace_scene(e, rho);
    let volume = ProbeVolumeParams {
        origin: Vec3::splat(0.05),
        extent: Vec3::splat(0.9),
        resolution: [2, 2, 2],
    };
    let input = adaptive(s.input(volume, 0.0, 1 << 20), 0.1, 0.3);
    s.run(&input, 12);
    let layout = s.scene.surface.realtime_gi_bricks_for_testing().unwrap();
    assert!(
        layout.bricks.iter().all(|b| b.level == 1) && layout.probes.len() == 1000,
        "premise: the walls cut the one top brick into 27 ({} probes)",
        layout.probes.len()
    );
    let want = e * (1.0 - rho.powi(12)) / (1.0 - rho);
    for probe in 0..layout.probes.len() {
        let got = s.irradiance(probe, Vec3::Y).x;
        assert!(
            (got - want).abs() < want * 0.02,
            "probe {probe} at {}: {got} vs {want}",
            layout.probes[probe].position
        );
    }
}

/// What the bricks are for: a small lamp just above a floor throws a pool of
/// light a coarse grid cannot hold. The floor around the lamp, seen from
/// above, through three volumes over the same box: a grid at the top level's
/// spacing (0.75), a dense grid at the finest (0.25), and an adaptive volume
/// between the two, fine around the floor and coarse above it. Where the
/// bricks are fine, their probes are the dense grid's lattice points -- and
/// with no analytic light the update is deterministic, so they hold the same
/// light -- so the adaptive volume shades the floor as the dense grid does,
/// with far fewer probes; the coarse grid does not (the premise: the pool is
/// finer than it).
#[test]
fn adaptive_bricks_shade_the_floor_as_a_dense_grid_does() {
    let shade = |volume: ProbeVolumeParams, spacing: Option<(f32, f32)>| {
        let mut s = GiScene::new(Vec3::ZERO);
        let floor = quad_corners(Vec3::new(-4.0, 0.0, 4.0), Vec3::X * 8.0, Vec3::NEG_Z * 8.0);
        s.add(floor, Vec3::splat(0.8), Vec3::ZERO);
        let i = s.scene.draw_calls.len() - 1;
        s.scene.draw_calls[i].3.base_color = Vec3::ONE;
        // The lamp: 0.3 square, 0.15 over the floor, facing down.
        s.add(
            quad_corners(Vec3::new(-0.15, 0.15, -0.15), Vec3::X * 0.3, Vec3::Z * 0.3),
            Vec3::ZERO,
            Vec3::splat(1.5),
        );
        s.scene.sun = Some((Vec3::ZERO, Vec3::ZERO));
        let eye = Vec3::new(0.55, 1.4, 0.0);
        s.scene.camera = (
            Mat4::perspective_rh(0.9, 1.0, 0.05, 50.0)
                * Mat4::look_at_rh(eye, Vec3::new(0.55, 0.0, 0.0), Vec3::NEG_Z),
            eye,
        );
        let mut input = s.input(volume, 0.0, 1 << 20);
        if let Some((min, max)) = spacing {
            input = adaptive(input, min, max);
        }
        s.run(&input, 8);
        let probes = s
            .scene
            .surface
            .realtime_gi_bricks_for_testing()
            .map(|l| l.probes.len());
        let pixels = s.scene.surface.read_pixels().expect("offscreen readback");
        let row: Vec<f32> = (4..60).map(|x| linear(pixels[(32 * 64 + x) * 4])).collect();
        (row, probes)
    };
    let box_at = |resolution: [u32; 3]| ProbeVolumeParams {
        origin: Vec3::new(-1.125, -0.375, -1.125),
        extent: Vec3::new(2.25, 4.5, 2.25),
        resolution,
    };
    let (coarse, _) = shade(box_at([4, 7, 4]), None);
    let (dense, _) = shade(box_at([10, 19, 10]), None);
    let (bricks, probes) = shade(box_at([2, 2, 2]), Some((0.25, 0.75)));
    let probes = probes.expect("adaptive");
    let worst = |a: &[f32], b: &[f32]| {
        a.iter()
            .zip(b)
            .map(|(x, y)| (x - y).abs() / y.max(1e-3))
            .fold(0.0f32, f32::max)
    };
    assert!(
        dense.iter().cloned().fold(0.0, f32::max) > 0.05,
        "premise: the lamp lights the floor: {dense:.3?}"
    );
    assert!(
        worst(&coarse, &dense) > 0.25,
        "premise: the coarse grid misses the pool: {coarse:.3?} against {dense:.3?}"
    );
    assert!(
        worst(&bricks, &dense) < 0.03,
        "the bricks shade the floor as the dense grid: {bricks:.3?} against {dense:.3?}"
    );
    assert!(
        probes < 10 * 19 * 10 * 6 / 10,
        "with well under the dense grid's probes: {probes}"
    );
}

/// The bricks follow the static set: cut around it as it is, and cut again
/// when it changes. A volume with nothing static inside is all top-level
/// bricks; a floor added through it refines the bricks it passes through,
/// with no change to the volume itself.
#[test]
fn an_adaptive_volume_is_cut_again_when_the_static_set_changes() {
    let mut s = GiScene::new(Vec3::ZERO);
    // Something static, far outside the volume, so the set is not empty.
    s.add(
        quad_corners(Vec3::new(50.0, 0.0, 0.0), Vec3::X, Vec3::Y),
        Vec3::splat(0.5),
        Vec3::ZERO,
    );
    let volume = ProbeVolumeParams {
        origin: Vec3::ZERO,
        extent: Vec3::splat(2.25),
        resolution: [2, 2, 2],
    };
    let input = adaptive(s.input(volume, 0.0, 1 << 20), 0.25, 0.75);
    s.run(&input, 1);
    let before = s.scene.surface.realtime_gi_bricks_for_testing().unwrap();
    assert_eq!(before.bricks.len(), 1, "nothing to refine around");
    s.add(
        quad_corners(Vec3::new(-1.0, 1.0, 3.0), Vec3::X * 4.0, Vec3::NEG_Z * 4.0),
        Vec3::splat(0.5),
        Vec3::ZERO,
    );
    let input = adaptive(s.input(volume, 0.0, 1 << 20), 0.25, 0.75);
    let readbacks = s.scene.surface.realtime_gi_readbacks();
    // Until the new set is read back, the old one is traced: run until it
    // has been, then once more.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(600);
    loop {
        s.run(&input, 1);
        let cut = s.scene.surface.realtime_gi_bricks_for_testing().unwrap();
        if cut.bricks.len() > 1 {
            assert_eq!(cut.bricks.len(), 27, "the floor cuts the one brick into 27");
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "the bricks were never cut around the new floor"
        );
    }
    assert_eq!(
        s.scene.surface.realtime_gi_readbacks(),
        readbacks + 1,
        "premise: the new set was read back"
    );
}

/// The furnace room's static quads as the bake reads them: each of
/// `furnace_room`'s six walls, two triangles facing in.
fn furnace_room_triangles() -> Vec<bsengine_bake::BakeTriangle> {
    let (e, rho) = (0.15f32, 0.5f32);
    let (lo, hi) = (Vec3::new(-3.0, -3.0, 0.5), Vec3::new(3.0, 3.0, 6.5));
    let size = hi - lo;
    let mut out = Vec::new();
    for (min, u, v) in [
        (lo, Vec3::X * size.x, Vec3::Y * size.y),
        (
            Vec3::new(hi.x, lo.y, hi.z),
            Vec3::NEG_X * size.x,
            Vec3::Y * size.y,
        ),
        (lo, Vec3::Y * size.y, Vec3::Z * size.z),
        (
            Vec3::new(hi.x, lo.y, hi.z),
            Vec3::Y * size.y,
            Vec3::NEG_Z * size.z,
        ),
        (
            Vec3::new(lo.x, lo.y, hi.z),
            Vec3::X * size.x,
            Vec3::NEG_Z * size.z,
        ),
        (
            Vec3::new(lo.x, hi.y, lo.z),
            Vec3::X * size.x,
            Vec3::Z * size.z,
        ),
    ] {
        let p = quad_corners(min, u, v);
        let n = u.cross(v).normalize();
        for [a, b, c] in [[0, 1, 2], [0, 2, 3]] {
            out.push(bsengine_bake::BakeTriangle {
                positions: [p[a], p[b], p[c]],
                normals: [n; 3],
                uv2: [glam::Vec2::ZERO; 3],
                albedo: Vec3::splat(rho),
                emissive: Vec3::splat(e),
                renderer: 0,
            });
        }
    }
    out
}

/// Baked bricks shade as realtime ones do: the furnace room's back wall,
/// lit through an adaptive volume whose probes were traced once on the CPU
/// (`bsengine_bake::probe_bake`, enough bounces to reach the furnace's fixed
/// point) and uploaded, against the same wall lit by the realtime update's
/// bricks after it has settled -- the row matches. The premise: with no
/// volume at all the wall is far darker (its own glow only). Cleared, the
/// grid is handed back: the wall is that dark again.
#[test]
fn baked_bricks_shade_as_the_realtime_bricks_do() {
    let row = |s: &GiScene| {
        let pixels = s.scene.surface.read_pixels().expect("offscreen readback");
        (1..63)
            .map(|x| pixels[(32 * 64 + x) * 4])
            .collect::<Vec<u8>>()
    };
    let volume = ProbeVolumeParams {
        origin: Vec3::new(-3.1, -3.1, 0.4),
        extent: Vec3::splat(6.2),
        resolution: [2, 2, 2],
    };
    let mut s = furnace_room();
    s.scene.render(None).expect("the frame renders");
    let none = row(&s);

    let input = adaptive(s.input(volume, 0.5, 1 << 20), 1.0, 1.5);
    s.run(&input, 40);
    let realtime = row(&s);
    let layout = s.scene.surface.realtime_gi_bricks_for_testing().unwrap();

    let mut s = furnace_room();
    let tris = furnace_room_triangles();
    let baked = bsengine_bake::probe_bake::bake_probes(
        &bsengine_bake::BakeInput {
            triangles: &tris,
            renderers: &[bsengine_bake::RendererInfo {
                resolution_scale: 1.0,
            }],
            lights: &[],
            sky: &bsengine_bake::Sky::Uniform(Vec3::ZERO),
        },
        layout,
        &bsengine_bake::probe_bake::ProbeBakeSettings {
            rays: 256,
            bounces: 30,
            seed: 0,
        },
    );
    s.scene
        .surface
        .set_baked_probe_bricks(Some(std::sync::Arc::new(baked)));
    s.scene.render(None).expect("the frame renders");
    assert!(s.scene.surface.has_baked_probe_bricks());
    let from_bake = row(&s);

    let mid = none.len() / 2;
    assert!(
        realtime[mid] as i32 > none[mid] as i32 + 20,
        "premise: the probes light the wall: {none:?} -> {realtime:?}"
    );
    assert!(
        from_bake
            .iter()
            .zip(&realtime)
            .all(|(a, b)| (*a as i32 - *b as i32).abs() <= 4),
        "baked bricks shade the wall as realtime ones: {from_bake:?} against {realtime:?}"
    );

    // A realtime volume takes the grid over while it runs; when it stops,
    // the baked bricks are uploaded again rather than left behind the grid
    // the realtime update emptied.
    let input = adaptive(s.input(volume, 0.0, 1 << 20), 1.0, 1.5);
    s.run(&input, 1);
    assert!(
        !s.scene.surface.has_baked_probe_bricks(),
        "the update owns the grid"
    );
    s.scene.render(None).expect("the frame renders");
    assert!(s.scene.surface.has_baked_probe_bricks());
    assert_eq!(row(&s), from_bake, "the bake is back once the update stops");

    s.scene.surface.set_baked_probe_bricks(None);
    s.scene.render(None).expect("the frame renders");
    assert!(!s.scene.surface.has_baked_probe_bricks());
    assert_eq!(row(&s), none, "cleared, the grid is empty again");
}

/// A volume whose bricks need more probes than the buffer starts with grows
/// it, rather than being cut coarser: the furnace's box, refined three
/// levels deep along its walls, is well past 4096 probes, and the buffer
/// grows to hold them all. Every table is then where the shader looks at the
/// larger capacity -- its lookup agrees with the layout's, and every probe,
/// the ones past the old limit included, reaches the furnace's light.
#[test]
fn an_adaptive_layout_past_the_buffer_grows_it() {
    let (e, rho) = (0.4f32, 0.5f32);
    let mut s = furnace_scene(e, rho);
    assert_eq!(
        s.scene.surface.probe_capacity(),
        4096,
        "premise: the default"
    );
    let volume = ProbeVolumeParams {
        origin: Vec3::splat(0.05),
        extent: Vec3::splat(0.9),
        resolution: [2, 2, 2],
    };
    let input = adaptive(s.input(volume, 0.0, 1 << 24), 0.1 / 3.0, 0.3);
    s.run(&input, 12);
    let layout = s.scene.surface.realtime_gi_bricks_for_testing().unwrap();
    assert!(!layout.truncated, "the finest level was cut");
    assert!(
        layout.probes.len() > 4096,
        "premise: past the old limit: {}",
        layout.probes.len()
    );
    let capacity = s.scene.surface.probe_capacity();
    assert!(
        capacity as usize >= layout.probes.len() && capacity.is_power_of_two(),
        "the buffer grew to hold them: {capacity}"
    );

    let mut state = 5u32;
    let mut next = || {
        state = state.wrapping_mul(1664525).wrapping_add(1013904223);
        (state >> 8) as f32 / (1u32 << 24) as f32
    };
    let mut points = Vec::new();
    let mut want = Vec::new();
    while points.len() < 2000 {
        let p = volume.origin + Vec3::new(next(), next(), next()) * volume.extent;
        let (b, local) = layout.locate(p).unwrap();
        if (local - local.round()).abs().min_element() < 1e-3 {
            continue;
        }
        let brick = &layout.bricks[b as usize];
        let i0 = local.floor().clamp(Vec3::ZERO, Vec3::splat(2.0));
        let corners: [u32; 8] = std::array::from_fn(|c| {
            let c = c as u32;
            let ci = i0.as_uvec3() + glam::UVec3::new(c & 1, (c >> 1) & 1, c >> 2);
            brick.probes[(ci.x + 4 * (ci.y + 4 * ci.z)) as usize]
        });
        points.push(p);
        want.push(corners);
    }
    let got = s.scene.surface.probe_cells_for_testing(&points);
    for ((p, w), (g, _)) in points.iter().zip(&want).zip(&got) {
        assert_eq!(g, w, "{p}: the lookup at the grown capacity");
    }

    let target = e * (1.0 - rho.powi(12)) / (1.0 - rho);
    let past = (4096..layout.probes.len()).step_by(97);
    for probe in (0..4096).step_by(211).chain(past) {
        let got = s.irradiance(probe, Vec3::Y).x;
        assert!(
            (got - target).abs() < target * 0.02,
            "probe {probe}: {got} vs {target}"
        );
    }
}

/// Baked bricks past the buffer grow it too, on upload, and land whole:
/// the probes past the old limit hold what the bake wrote for them.
#[test]
fn baked_bricks_past_the_buffer_grow_it() {
    let mut s = furnace_room();
    let tris = furnace_room_triangles();
    let layout = bsengine_bake::probe_bricks::BrickLayout::build(
        &bsengine_bake::probe_bricks::BrickSettings {
            origin: Vec3::new(-3.1, -3.1, 0.4),
            extent: Vec3::splat(6.2),
            min_spacing: 0.2,
            max_spacing: 1.8,
            max_probes: bsengine_core::MAX_ADAPTIVE_PROBES,
            max_bricks: bsengine_core::MAX_PROBE_BRICKS,
        },
        &tris.iter().map(|t| t.positions).collect::<Vec<_>>(),
    );
    assert!(
        layout.probes.len() > 4096,
        "premise: {}",
        layout.probes.len()
    );
    let baked = bsengine_bake::probe_bake::bake_probes(
        &bsengine_bake::BakeInput {
            triangles: &tris,
            renderers: &[bsengine_bake::RendererInfo {
                resolution_scale: 1.0,
            }],
            lights: &[],
            sky: &bsengine_bake::Sky::Uniform(Vec3::ZERO),
        },
        layout,
        &bsengine_bake::probe_bake::ProbeBakeSettings {
            rays: 64,
            bounces: 2,
            seed: 0,
        },
    );
    let n = baked.probes.len();
    let last = baked.probes[n - 1].sh;
    let last_moments = baked.probes[n - 1].moments.clone();
    let centre = |s: &GiScene| {
        let pixels = s.scene.surface.read_pixels().expect("offscreen readback");
        pixels[(32 * 64 + 32) * 4]
    };
    s.scene.render(None).expect("the frame renders");
    let none = centre(&s);
    s.scene
        .surface
        .set_baked_probe_bricks(Some(std::sync::Arc::new(baked)));
    s.scene.render(None).expect("the frame renders");
    assert!(s.scene.surface.has_baked_probe_bricks());
    assert!(s.scene.surface.probe_capacity() as usize >= n);
    // And shading reads the grown buffer, not the one it replaced (which
    // holds nothing): the wall takes the probes' light.
    let lit = centre(&s);
    assert!(
        lit as i32 > none as i32 + 20,
        "the wall is lit through the grown buffer: {none} -> {lit}"
    );
    let grid = s.scene.surface.read_probe_grid_for_testing();
    let got = grid.coeffs[n - 1];
    for (c, want) in got.iter().zip(last) {
        assert!(
            (Vec3::new(c[0], c[1], c[2]) - want).length() < 1e-6,
            "the last probe's SH: {c:?} vs {want}"
        );
    }
    // And its moments, where the shader reads them at this capacity: written
    // anywhere else, a probe reads another's -- distances much like its own,
    // which no pixel here tells from the right ones.
    assert_eq!(
        grid.moments[n - 1],
        last_moments,
        "the last probe's moments"
    );
}

// ---- skinned renderers -----------------------------------------------------

/// A vertex at `p` facing `n`.
fn vertex_at(p: Vec3, n: Vec3) -> Vertex {
    Vertex {
        position: p.to_array(),
        color: [1.0, 1.0, 1.0],
        normal: n.to_array(),
        uv: [0.0, 0.0],
        uv2: [0.0, 0.0],
        tangent: [0.0; 4],
    }
}

/// Bound to joint `j` alone.
fn bound_to(j: u32) -> bsengine_rhi_wgpu::skinning::GpuVertexSkin {
    bsengine_rhi_wgpu::skinning::GpuVertexSkin {
        joints: [j, 0, 0, 0],
        weights: [1.0, 0.0, 0.0, 0.0],
    }
}

/// The 100 x 100 floor at y = 0 cut into 16 x 16 squares, 512 triangles
/// wound to face up: enough that its tree has interior nodes several deep,
/// which a refit has to reach -- two triangles would be one leaf, and a
/// refit of leaves alone would pass for a whole one.
fn floor_grid() -> (Vec<Vec3>, Vec<u32>) {
    const N: u32 = 16;
    let mut points = Vec::new();
    for row in 0..=N {
        for col in 0..=N {
            points.push(Vec3::new(
                -50.0 + 100.0 * col as f32 / N as f32,
                0.0,
                50.0 - 100.0 * row as f32 / N as f32,
            ));
        }
    }
    let mut indices = Vec::new();
    for row in 0..N {
        for col in 0..N {
            let a = row * (N + 1) + col;
            let (b, c, d) = (a + 1, a + N + 2, a + N + 1);
            indices.extend([a, b, c, a, c, d]);
        }
    }
    (points, indices)
}

/// A skinned renderer is traced as it is posed, with nothing rebuilt: the
/// floor grid glowing, its rest pose ten thousand below the probes -- where
/// its tree is built, read back as it first stands -- is posed up under
/// them, and the first update after sees it glow; posed back, the sky. The
/// tree's shape is the rest pose's; its triangles, its boxes and its
/// instance's box in the world follow the pose. Leave out any of the three
/// and the posed floor is missed: the triangles still far below, or a box
/// that does not reach up to them, or the instance's box where the floor
/// was read back. Its triangles are wound against its normals, so a
/// refitted face left unturned is a back face, black.
#[test]
fn a_skinned_renderer_is_traced_as_it_is_posed() {
    let sky = 0.2f32;
    let mut s = GiScene::new(Vec3::splat(sky));
    let below = Vec3::new(0.0, -1.0e4, 0.0);
    let (points, mut indices) = floor_grid();
    // Wound clockwise from above, against its normals: the refit's shading
    // normal has to be turned to them, as the tree built on the CPU turns
    // it, or the probes would meet the floor's back.
    for t in indices.chunks_exact_mut(3) {
        t.swap(1, 2);
    }
    let vertices: Vec<Vertex> = points
        .iter()
        .map(|&p| vertex_at(p + below, Vec3::Y))
        .collect();
    let skin = vec![bound_to(0); vertices.len()];
    let mesh = s.add_skinned(&vertices, &skin, &indices, 1, Vec3::ZERO, Vec3::splat(2.0));
    s.pose(mesh, &[Mat4::IDENTITY]);
    let input = s.input(floor_volume(), 0.0, 1 << 20);
    s.run(&input, 3);
    assert_eq!(
        s.scene.surface.realtime_gi_instances(),
        1,
        "premise: traced"
    );
    assert!(
        s.scene.surface.realtime_gi_refit_depths() >= 4,
        "premise: the refit reaches up through interior nodes: {} depths",
        s.scene.surface.realtime_gi_refit_depths()
    );
    let at_rest = s.irradiance(0, Vec3::NEG_Y).x;
    assert!(
        (at_rest - sky).abs() < 0.01,
        "premise: at rest it is far below, and the sky shows: {at_rest}"
    );
    // One update: the first after the pose already traces it. (A refit
    // that made a node's box before its children's would lag a depth an
    // update, and only catch up over several.)
    s.pose(mesh, &[Mat4::from_translation(-below)]);
    s.run(&input, 1);
    let posed = s.irradiance(0, Vec3::NEG_Y).x;
    assert!(
        (posed - 2.0).abs() < 0.05,
        "posed under the probes, it glows below them: {posed}"
    );
    s.pose(mesh, &[Mat4::IDENTITY]);
    s.run(&input, 1);
    let back = s.irradiance(0, Vec3::NEG_Y).x;
    assert!(
        (back - sky).abs() < 0.01,
        "posed back, the sky again: {back}"
    );
    assert_eq!(
        s.scene.surface.realtime_gi_readbacks(),
        1,
        "posing reads nothing back and rebuilds no tree"
    );
}

/// The floor grid under the probes, folded at z = 0: the far half (z < 0,
/// bound to joint 1) turned up by 60 degrees about the hinge, the rest bound
/// to joint 0. Its vertices at rest, each one's joint, and the two joints'
/// matrices for the fold.
fn folded_grid() -> (Vec<Vertex>, Vec<u32>, Vec<u32>, [Mat4; 2]) {
    let (points, indices) = floor_grid();
    let vertices = points.iter().map(|&p| vertex_at(p, Vec3::Y)).collect();
    let joint = points.iter().map(|p| u32::from(p.z < 0.0)).collect();
    let fold = Mat4::from_rotation_x(60f32.to_radians());
    (vertices, joint, indices, [Mat4::IDENTITY, fold])
}

/// A skinned renderer is traced where the skinning pass put each vertex:
/// the grid folded by its second joint gives the probes what a rigid mesh
/// with the folded vertices -- each placed and turned by its own joint on
/// the CPU -- gives them, in every direction of all eight probes, its tree
/// built flat. The premise: folded, the probes see something other than the
/// flat grid.
///
/// Relocation is switched off before the fold, in both: a probe traces from
/// where the update before moved it, so a skinned probe moved for the flat
/// grid and a rigid one moved for the folded grid from the start would look
/// from different places -- the two under the fold differed by 1.8 for that
/// alone, while the other six matched to the bit. Two updates without it,
/// and both trace from the lattice.
#[test]
fn a_skinned_renderer_is_traced_as_its_folded_vertices_are() {
    let (rest, joint, indices, joints) = folded_grid();
    let run = |skinned: bool, pose: [Mat4; 2]| {
        let mut s = GiScene::new(Vec3::splat(0.2));
        let input;
        if skinned {
            let skin: Vec<_> = joint.iter().map(|&j| bound_to(j)).collect();
            let mesh = s.add_skinned(&rest, &skin, &indices, 2, Vec3::ZERO, Vec3::splat(2.0));
            // Read back flat, its tree built flat; then folded.
            s.pose(mesh, &[Mat4::IDENTITY; 2]);
            input = s.input(floor_volume(), 0.0, 1 << 20);
            s.run(&input, 2);
            s.scene
                .surface
                .set_realtime_gi_relocation_for_testing(false);
            s.pose(mesh, &pose);
            s.run(&input, 2);
        } else {
            let posed: Vec<Vertex> = rest
                .iter()
                .zip(&joint)
                .map(|(v, &j)| {
                    let m = pose[j as usize];
                    vertex_at(
                        m.transform_point3(Vec3::from(v.position)),
                        m.transform_vector3(Vec3::from(v.normal)),
                    )
                })
                .collect();
            s.add_dynamic_mesh(&posed, &indices, Vec3::ZERO, Vec3::splat(2.0));
            input = s.input(floor_volume(), 0.0, 1 << 20);
            s.run(&input, 2);
            s.scene
                .surface
                .set_realtime_gi_relocation_for_testing(false);
            s.run(&input, 2);
        }
        let dirs = [
            Vec3::X,
            Vec3::NEG_X,
            Vec3::Y,
            Vec3::NEG_Y,
            Vec3::Z,
            Vec3::NEG_Z,
        ];
        (0..8)
            .flat_map(|p| dirs.map(|d| s.irradiance(p, d).x))
            .collect::<Vec<f32>>()
    };
    let skinned = run(true, joints);
    let rigid = run(false, joints);
    let flat = run(false, [Mat4::IDENTITY; 2]);
    let most = |a: &[f32], b: &[f32]| {
        a.iter()
            .zip(b)
            .map(|(x, y)| (x - y).abs())
            .fold(0.0f32, f32::max)
    };
    assert!(
        most(&rigid, &flat) > 0.2,
        "premise: folded, the probes see another shape: {}",
        most(&rigid, &flat)
    );
    assert!(
        most(&skinned, &rigid) < 0.01,
        "the skinned grid is traced as its folded vertices: off by {}",
        most(&skinned, &rigid)
    );
}

/// A closed grey room under a blue sky, lit only by a warm point lamp the
/// probes trace, with a white metal floor the camera looks down at --
/// metallic, so the floor has no diffuse term and everything it shows is
/// its specular environment. Returns the floor's centre pixel: before any
/// GI update (`gi_updates` 0 -- the frame shades as if no volume existed,
/// so its specular is the sky's), or after that many.
///
/// The sky is a skybox for the mesh shader (its reflection and its
/// irradiance) and nothing to the probes inside a closed room. Every face of
/// the room is drawn black but the floor, and the harness sun is off, so no
/// direct light reaches the floor at all.
fn covered_metal_floor(roughness: f32, lamp: bool, volume: Option<ProbeVolumeParams>) -> [u8; 3] {
    covered_metal_floor_glowing(roughness, lamp, None, volume)
}

/// [`covered_metal_floor`] with one wall glowing: `Some((BACK_WALL, c))`
/// for the one the floor's centre pixel reflects, `FRONT_WALL` for the one
/// behind the camera.
fn covered_metal_floor_glowing(
    roughness: f32,
    lamp: bool,
    glowing: Option<(usize, Vec3)>,
    volume: Option<ProbeVolumeParams>,
) -> [u8; 3] {
    let mut s = covered_room(roughness, lamp, glowing);
    match volume {
        None => s.scene.render(None).expect("the frame renders"),
        Some(volume) => {
            let input = s.input(volume, 0.5, 1 << 20);
            s.run(&input, 16);
        }
    }
    s.centre()
}

/// [`covered_metal_floor_glowing`]'s room, camera and lights, not yet drawn.
fn covered_room(roughness: f32, lamp: bool, glowing: Option<(usize, Vec3)>) -> GiScene {
    let mut s = GiScene::new(Vec3::new(0.1, 0.3, 1.0));
    let (lo, hi) = (Vec3::new(-3.0, 0.0, -3.0), Vec3::new(3.0, 3.0, 3.0));
    let size = hi - lo;
    let grey = Vec3::splat(0.6);
    // The floor first, facing up.
    s.add(
        quad_corners(
            Vec3::new(lo.x, lo.y, hi.z),
            Vec3::X * size.x,
            Vec3::NEG_Z * size.z,
        ),
        grey,
        Vec3::ZERO,
    );
    let floor = s.scene.draw_calls.len() - 1;
    s.scene.draw_calls[floor].3.base_color = Vec3::ONE;
    s.scene.draw_calls[floor].3.metallic = 1.0;
    s.scene.draw_calls[floor].3.roughness = roughness;
    for (min, u, v) in [
        // Ceiling, facing down.
        (
            Vec3::new(lo.x, hi.y, lo.z),
            Vec3::X * size.x,
            Vec3::Z * size.z,
        ),
        // Back (+z), front (-z), left (+x), right (-x).
        (lo, Vec3::X * size.x, Vec3::Y * size.y),
        (
            Vec3::new(hi.x, lo.y, hi.z),
            Vec3::NEG_X * size.x,
            Vec3::Y * size.y,
        ),
        (lo, Vec3::Y * size.y, Vec3::Z * size.z),
        (
            Vec3::new(hi.x, lo.y, hi.z),
            Vec3::Y * size.y,
            Vec3::NEG_Z * size.z,
        ),
    ] {
        s.add(quad_corners(min, u, v), grey, Vec3::ZERO);
    }
    if let Some((wall, glow)) = glowing {
        let first = s.statics.len() - 6;
        s.statics[first + wall].emissive = glow;
    }
    if lamp {
        s.lights = vec![bsengine_bake::BakeLight::Point {
            position: Vec3::new(0.0, 2.5, 0.0),
            color: Vec3::new(1.0, 0.55, 0.25),
            intensity: 1.0,
            range: 10.0,
            contribution: bsengine_bake::Contribution::Indirect,
        }];
    }
    s.scene.sun = Some((Vec3::ZERO, Vec3::ZERO));
    s.scene
        .surface
        .set_skybox_from_rgba(1, 1, &[40, 100, 255, 255]);
    let eye = Vec3::new(0.0, 1.2, 2.5);
    s.scene.camera = (
        Mat4::perspective_rh(1.2, 1.0, 0.05, 50.0)
            * Mat4::look_at_rh(eye, Vec3::new(0.0, 0.0, -0.5), Vec3::Y),
        eye,
    );
    s
}

/// The room's volume: probes from just outside its faces, as in
/// games/gi-demo, relocated in front of them.
fn room_volume() -> ProbeVolumeParams {
    ProbeVolumeParams {
        origin: Vec3::new(-3.1, -0.1, -3.1),
        extent: Vec3::new(6.2, 3.2, 6.2),
        resolution: [6, 4, 6],
    }
}

fn blue_bias(p: [u8; 3]) -> i32 {
    p[2] as i32 - p[0] as i32
}

/// The pixel's light, summed over its channels, decoded from sRGB.
fn linear_sum(p: [u8; 3]) -> f32 {
    p.iter().map(|&c| linear(c)).sum()
}

/// Inside a probe volume the probes stand in for the sky's specular where
/// nothing sharper answers (`MESH_WGSL`, "Inside a light probe volume the
/// probes also stand in for the sky's specular"). A rough metal floor in a
/// closed room reflected the blue sky it cannot see -- the premise, from
/// the frame without the volume -- and with it reflects the lamp-lit room:
/// warm, not blue. With the lamp off it goes dark, which a reflection of
/// the sky never did.
///
/// Measured: sky `[13, 72, 207]`, lamp-lit room `[78, 55, 52]`, lamp off
/// `[2, 9, 33]` -- not black: the probes on the volume's faces start
/// outside the room's one-sided walls and see some sky before they are
/// moved in, so the dark room keeps a faint blue. An eighth of the lit
/// room's light, against the sky's four and a half times it.
#[test]
fn a_rough_metal_floor_in_a_closed_room_reflects_the_room_not_the_sky() {
    let sky = covered_metal_floor(0.9, true, None);
    let room = covered_metal_floor(0.9, true, Some(room_volume()));
    let dark = covered_metal_floor(0.9, false, Some(room_volume()));
    assert!(
        blue_bias(sky) > 40,
        "premise: without the volume the floor reflects the blue sky: {sky:?}"
    );
    assert!(
        room.iter().any(|&c| c > 30),
        "premise: the room's light is there to see: {room:?}"
    );
    assert!(
        blue_bias(room) < 0,
        "with the volume it reflects the lamp's warm light: {sky:?} -> {room:?}"
    );
    assert!(
        linear_sum(dark) < 0.3 * linear_sum(room),
        "with the lamp off the room, and so its reflection, is dark: \
         {room:?} -> {dark:?}"
    );
}

/// A smooth metal floor keeps the sky's reflection -- the probes have no
/// picture of it to give -- but only as much of it as the probes say
/// arrives from the reflected direction (HDRP's APV reflection
/// normalization): in a closed room, darker, and still blue. Replacing it
/// with the probes' light, as for a rough floor, would have turned it warm
/// (`[78, 55, 52]` for the same room, rough); not scaling it would have left
/// it the sky's.
///
/// Measured: sky `[27, 115, 231]`, lamp-lit room `[16, 81, 213]` (the
/// room's light is about the sky's, so only a little is taken), lamp off
/// `[2, 15, 94]`.
#[test]
fn a_smooth_metal_floor_in_a_closed_room_keeps_a_dimmed_sky() {
    let sky = covered_metal_floor(0.1, true, None);
    let room = covered_metal_floor(0.1, true, Some(room_volume()));
    let dark = covered_metal_floor(0.1, false, Some(room_volume()));
    assert!(
        blue_bias(sky) > 40,
        "premise: without the volume the floor reflects the blue sky: {sky:?}"
    );
    assert!(
        blue_bias(room) > 40,
        "with the volume it is still the sky's colour, not the room's: {room:?}"
    );
    assert!(
        linear_sum(room) < 0.9 * linear_sum(sky),
        "the hidden sky's reflection is dimmed: {sky:?} -> {room:?}"
    );
    assert!(
        linear_sum(dark) < 0.3 * linear_sum(sky),
        "and with the lamp off, by most of it: {sky:?} -> {dark:?}"
    );
}

/// Outside the volume nothing changes: a volume that does not hold the
/// floor's pixel leaves it what the frame without one draws.
#[test]
fn a_metal_floor_outside_the_volume_keeps_the_sky() {
    let sky = covered_metal_floor(0.9, true, None);
    let beside = ProbeVolumeParams {
        origin: Vec3::new(-3.1, 1.0, -3.1),
        ..room_volume()
    };
    let outside = covered_metal_floor(0.9, true, Some(beside));
    assert_eq!(outside, sky, "the floor is below the volume");
}

/// Which of [`covered_metal_floor_glowing`]'s statics a wall is: the
/// floor is 0, the ceiling 1.
const BACK_WALL: usize = 2;
const FRONT_WALL: usize = 3;

/// The probes' specular is read facing the reflection, not the normal: a
/// rough metal floor is green when the green wall glows where its
/// reflection runs (the back wall), and far less when the same glow is
/// behind the camera. Read facing the normal -- straight up -- the two walls
/// are about the same to it, since the cosine lobe around "up" sees them
/// both from the side.
///
/// Measured: ahead `[78, 154, 52]`, behind `[78, 85, 52]`; read facing the
/// normal instead, `[76, 119, 49]` and `[76, 103, 49]` -- 16 apart, not 69.
#[test]
fn the_probes_specular_faces_the_reflection() {
    let glow = Vec3::new(0.0, 0.6, 0.0);
    let ahead =
        covered_metal_floor_glowing(0.9, true, Some((BACK_WALL, glow)), Some(room_volume()));
    let behind =
        covered_metal_floor_glowing(0.9, true, Some((FRONT_WALL, glow)), Some(room_volume()));
    let green = |p: [u8; 3]| p[1] as i32 - (p[0] as i32 + p[2] as i32) / 2;
    assert!(
        green(behind) > 0,
        "premise: the glow behind the camera reaches the floor too: {behind:?}"
    );
    assert!(
        green(ahead) > green(behind) + 30,
        "the floor reflects the wall it faces: behind {behind:?}, ahead {ahead:?}"
    );
}

/// What the mirror room's wall behind the camera is.
#[derive(Clone, Copy)]
enum Behind {
    /// Glowing red.
    Red,
    /// Grey, lit (or not) by the room's lamp.
    Grey,
    /// Glowing blue, with a red card in front of it for x > 0: an edge for
    /// a reflection to blur, reflected down the middle of the mirror. A
    /// card in front, not two halves side by side: a ray down the seam
    /// between two quads can slip through it.
    Split,
}

/// A closed room whose back wall is a mirror -- white metal, nearly smooth --
/// and a camera facing it square on, so the centre pixel reflects straight
/// back past the camera to the wall behind it. Ray-traced reflections exist
/// for exactly this: the reflection runs back toward the viewer, which the
/// screen-space trace gives up on by construction (`fs_ssr`'s `facing`), and
/// what it would show is not on screen anyway. The other walls are drawn
/// black, the sun is off, and the sky -- a blue skybox -- is outside.
struct MirrorRoom {
    s: GiScene,
    /// The mirror's draw.
    mirror: usize,
    /// A moving red panel hung in front of the wall behind the camera, if
    /// the test asked for one.
    panel: Option<usize>,
}

impl MirrorRoom {
    fn new(behind: Behind, lamp: bool, panel: bool) -> Self {
        let mut s = GiScene::new(Vec3::new(0.1, 0.3, 1.0));
        let (lo, hi) = (Vec3::new(-3.0, 0.0, -3.0), Vec3::new(3.0, 3.0, 3.0));
        let size = hi - lo;
        let grey = Vec3::splat(0.6);
        let walls = [
            // Floor (up), ceiling (down), back (+z), front (-z), left, right.
            (
                Vec3::new(lo.x, lo.y, hi.z),
                Vec3::X * size.x,
                Vec3::NEG_Z * size.z,
            ),
            (
                Vec3::new(lo.x, hi.y, lo.z),
                Vec3::X * size.x,
                Vec3::Z * size.z,
            ),
            (lo, Vec3::X * size.x, Vec3::Y * size.y),
            (
                Vec3::new(hi.x, lo.y, hi.z),
                Vec3::NEG_X * size.x,
                Vec3::Y * size.y,
            ),
            (lo, Vec3::Y * size.y, Vec3::Z * size.z),
            (
                Vec3::new(hi.x, lo.y, hi.z),
                Vec3::Y * size.y,
                Vec3::NEG_Z * size.z,
            ),
        ];
        let mut mirror = 0;
        for (i, (min, u, v)) in walls.into_iter().enumerate() {
            if i == 3 && matches!(behind, Behind::Split) {
                s.add(quad_corners(min, u, v), grey, Vec3::new(0.0, 0.0, 2.0));
                s.add(
                    quad_corners(min + Vec3::new(0.0, 0.0, -0.05), u * 0.5, v),
                    grey,
                    Vec3::new(2.0, 0.0, 0.0),
                );
                continue;
            }
            let emissive = match (i, behind) {
                (3, Behind::Red) => Vec3::new(2.0, 0.0, 0.0),
                _ => Vec3::ZERO,
            };
            s.add(quad_corners(min, u, v), grey, emissive);
            if i == 2 {
                mirror = s.scene.draw_calls.len() - 1;
                s.scene.draw_calls[mirror].3.base_color = Vec3::ONE;
                s.scene.draw_calls[mirror].3.metallic = 1.0;
                s.scene.draw_calls[mirror].3.roughness = 0.05;
            }
        }
        let panel = panel.then(|| {
            // Facing -z, toward the mirror, a little in front of the wall
            // behind the camera; moved by its model, with nothing rebuilt.
            s.add_dynamic(
                quad_corners(Vec3::new(1.0, -1.0, 0.0), Vec3::NEG_X * 2.0, Vec3::Y * 2.0),
                Mat4::from_translation(Vec3::new(0.0, 1.5, 2.8)),
                Vec3::ZERO,
                Vec3::new(2.0, 0.0, 0.0),
            )
        });
        if lamp {
            s.lights = vec![bsengine_bake::BakeLight::Point {
                position: Vec3::new(0.0, 2.5, 1.0),
                color: Vec3::new(1.0, 0.55, 0.25),
                intensity: 3.0,
                range: 10.0,
                contribution: bsengine_bake::Contribution::Indirect,
            }];
        }
        s.scene.sun = Some((Vec3::ZERO, Vec3::ZERO));
        s.scene
            .surface
            .set_skybox_from_rgba(1, 1, &[40, 100, 255, 255]);
        let eye = Vec3::new(0.0, 1.5, 2.0);
        s.scene.camera = (
            Mat4::perspective_rh(1.0, 1.0, 0.05, 50.0)
                * Mat4::look_at_rh(eye, Vec3::new(0.0, 1.5, -3.0), Vec3::Y),
            eye,
        );
        Self { s, mirror, panel }
    }

    /// The room's volume input.
    fn input(&self) -> bsengine_rhi_wgpu::realtime_gi::RealtimeGiInput {
        let volume = ProbeVolumeParams {
            origin: Vec3::new(-3.1, -0.1, -3.1),
            extent: Vec3::new(6.2, 3.2, 6.2),
            resolution: [6, 4, 6],
        };
        self.s.input(volume, 0.5, 1 << 20)
    }

    /// The centre pixel once the probes have settled -- and, with `rt`
    /// enabled, once the reflections' pass has drawn two more frames (its
    /// pipeline builds on a thread, so this waits for it against the clock,
    /// never for a fixed count of frames). `ssr` switches the screen-space
    /// trace on.
    fn centre(&mut self, rt: Option<bsengine_core::RayTracedReflections>, ssr: bool) -> [u8; 3] {
        self.s.scene.ssr = ssr.then(bsengine_core::ScreenSpaceReflections::default);
        self.s.scene.surface.set_ray_traced_reflections(rt);
        let input = self.input();
        self.s.run(&input, 6);
        if rt.is_some_and(|r| r.enabled) {
            self.s.run_reflected(&input, 2);
        }
        self.s.centre()
    }

    /// Lights the room with one white lamp at `at` instead of its own.
    fn lamp_at(&mut self, at: Vec3) {
        self.s.lights = vec![bsengine_bake::BakeLight::Point {
            position: at,
            color: Vec3::ONE,
            intensity: 4.0,
            range: 10.0,
            contribution: bsengine_bake::Contribution::Indirect,
        }];
    }

    /// Adds a small square card, static, centred on `centre` and facing
    /// `facing`: something to cast a shadow.
    fn card(&mut self, centre: Vec3, facing: Vec3) {
        let n = facing.normalize();
        let u = n.cross(Vec3::Y).normalize() * 0.5;
        let v = u.cross(n).normalize() * 0.5;
        self.s.add(
            quad_corners(centre - (u + v) * 0.5, u, v),
            Vec3::splat(0.6),
            Vec3::ZERO,
        );
    }

    /// Moves the panel by `by`.
    fn move_panel(&mut self, by: Vec3) {
        let i = self.panel.expect("a room with a panel");
        self.s.dynamics[i].model = Mat4::from_translation(by) * self.s.dynamics[i].model;
    }
}

fn red_bias(p: [u8; 3]) -> i32 {
    p[0] as i32 - (p[1] as i32 + p[2] as i32) / 2
}

/// The mirror shows the glowing wall behind the camera -- which the screen
/// cannot: the premise is that the screen-space trace changes nothing at
/// that pixel (the same frame with it on and off), so whatever turns it red
/// is the rays'.
///
/// Measured: `[16, 81, 214]` without the screen's trace, the same with it,
/// `[246, 9, 33]` with the rays.
///
/// Each frame from a room of its own: the probes go on converging update by
/// update, so the same room measured twice differs by that alone.
#[test]
fn a_mirror_reflects_the_wall_behind_the_camera() {
    let room = || MirrorRoom::new(Behind::Red, false, false);
    let no_ssr = room().centre(None, false);
    let screen = room().centre(None, true);
    let rays = room().centre(Some(bsengine_core::RayTracedReflections::default()), true);
    assert_eq!(
        screen, no_ssr,
        "premise: a reflection running back past the camera is nothing the screen can find"
    );
    assert!(
        red_bias(rays) > red_bias(screen) + 60,
        "the rays find the red wall: {screen:?} -> {rays:?}"
    );
}

/// The hit is lit by the lights as they are now: a grey wall behind the
/// camera, warm in the mirror with the lamp on, dark with it off.
///
/// Measured: lit `[207, 174, 129]`, dark `[2, 9, 33]`, and without the rays
/// `[28, 116, 232]` (the probes' stand-in for the sky).
#[test]
fn a_reflection_is_lit_by_the_lights_it_sees() {
    let rt = Some(bsengine_core::RayTracedReflections::default());
    let lit = MirrorRoom::new(Behind::Grey, true, false).centre(rt, false);
    let dark = MirrorRoom::new(Behind::Grey, false, false).centre(rt, false);
    let without = MirrorRoom::new(Behind::Grey, true, false).centre(None, false);
    assert!(
        red_bias(lit) > red_bias(without) + 20,
        "the lamp-lit wall is warm in the mirror: {without:?} -> {lit:?}"
    );
    assert!(
        linear_sum(dark) < 0.3 * linear_sum(lit),
        "and dark without the lamp: {lit:?} -> {dark:?}"
    );
}

/// A moving renderer is reflected where it is: the red panel behind the
/// camera turns the mirror red, and moved aside, by its model alone, it
/// leaves the mirror.
///
/// Measured: there `[245, 0, 0]`, moved `[56, 8, 31]`.
#[test]
fn a_moved_renderer_is_reflected_where_it_is() {
    let rt = Some(bsengine_core::RayTracedReflections::default());
    let mut room = MirrorRoom::new(Behind::Grey, false, true);
    let there = room.centre(rt, false);
    room.move_panel(Vec3::new(2.5, 0.0, 0.0));
    let moved = room.centre(rt, false);
    assert!(
        red_bias(there) > 60,
        "premise: the panel is in the mirror: {there:?}"
    );
    assert!(
        red_bias(moved) < red_bias(there) - 50,
        "moved aside, it is gone from it: {there:?} -> {moved:?}"
    );
}

/// Off is off: switched off, or with no realtime volume to trace, the frame
/// is the one with no component at all, and the pass never runs.
#[test]
fn ray_traced_reflections_with_nothing_to_trace_change_nothing() {
    let none = MirrorRoom::new(Behind::Red, false, false).centre(None, false);
    let mut room = MirrorRoom::new(Behind::Red, false, false);
    let off = room.centre(
        Some(bsengine_core::RayTracedReflections {
            enabled: false,
            ..Default::default()
        }),
        false,
    );
    assert_eq!(off, none, "switched off");
    assert_eq!(room.s.scene.surface.rt_reflection_frames(), 0);
    // No realtime volume: the surface has no scene to trace.
    let mut bare = MirrorRoom::new(Behind::Red, false, false);
    bare.s
        .scene
        .surface
        .set_ray_traced_reflections(Some(bsengine_core::RayTracedReflections::default()));
    bare.s.scene.render(None).expect("the frame renders");
    bare.s.scene.render(None).expect("the frame renders");
    let with = bare.s.scene.surface.read_pixels().expect("readback");
    bare.s.scene.surface.set_ray_traced_reflections(None);
    bare.s.scene.render(None).expect("the frame renders");
    let without = bare.s.scene.surface.read_pixels().expect("readback");
    assert_eq!(with, without, "no volume, no rays");
    assert_eq!(bare.s.scene.surface.rt_reflection_frames(), 0);
}

/// The rays fill only the share the screen left: where the screen-space
/// trace found the reflection, its share stands, and the rays add only the
/// rest -- the pass draws *under* it. A smooth metal floor reflecting the
/// back wall in view, which the screen and the rays see differently on
/// purpose: drawn red (what the screen reads from the last frame), traced
/// green (the GI's emissive). At this slant the screen's weight is about
/// 0.7 (`fs_ssr`'s `facing`), so the floor is mostly red with the screen
/// alone -- the premise -- and green with the rays alone. With both, the
/// red is the screen's red, unchanged (drawing the rays over the screen's
/// result would wash it out), and the green is well under the rays' own
/// (adding them in full, on top, would leave it whole).
///
/// Measured: screen `[230, 58, 191]`, rays `[2, 218, 31]`, both
/// `[230, 165, 11]` -- the screen's red, and the rays' green in the share
/// the sky had.
#[test]
fn the_rays_fill_only_what_the_screen_did_not_find() {
    let glow = Some((BACK_WALL, Vec3::new(0.0, 0.6, 0.0)));
    let measure = |ssr: bool, rt: bool| {
        let mut s = covered_room(0.05, false, glow);
        let back = s.scene.draw_calls.len() - 6 + BACK_WALL;
        s.scene.draw_calls[back].3.emissive = Vec3::new(1.5, 0.0, 0.0);
        s.scene.ssr = ssr.then(bsengine_core::ScreenSpaceReflections::default);
        s.scene
            .surface
            .set_ray_traced_reflections(rt.then(bsengine_core::RayTracedReflections::default));
        let input = s.input(room_volume(), 0.5, 1 << 20);
        s.run(&input, 16);
        if rt {
            s.run_reflected(&input, 2);
        }
        s.centre()
    };
    let screen = measure(true, false);
    let rays = measure(false, true);
    let both = measure(true, true);
    assert!(
        red_bias(screen) > 60,
        "premise: the screen finds the red wall: {screen:?}"
    );
    assert!(
        rays[1] as i32 > rays[0] as i32 + 60,
        "premise: the rays find it green: {rays:?}"
    );
    assert!(
        (both[0] as i32 - screen[0] as i32).abs() <= 6,
        "the screen's share stands: screen {screen:?}, both {both:?}"
    );
    assert!(
        (both[1] as i32) < rays[1] as i32 - 30,
        "the rays add only the share the screen left: rays {rays:?}, both {both:?}"
    );
}

/// Rougher than `max_roughness` and the band after it, a surface traces
/// nothing: one mirror ray is the wrong answer there, and the probes'
/// stand-in answers instead. A rough metal floor reflecting a glowing green
/// wall is the frame without the rays; the same floor smooth is not (the
/// premise: there is something for the rays to change).
#[test]
fn a_rough_surface_is_left_to_the_probes() {
    let glow = Some((BACK_WALL, Vec3::new(0.0, 0.6, 0.0)));
    let measure = |roughness: f32, rt: bool| {
        let mut s = covered_room(roughness, false, glow);
        s.scene
            .surface
            .set_ray_traced_reflections(rt.then(bsengine_core::RayTracedReflections::default));
        let input = s.input(room_volume(), 0.5, 1 << 20);
        s.run(&input, 16);
        // Two more frames either way: the probes go on converging frame by
        // frame, so a frame count that differed would differ by that alone.
        // The pipeline is built by the first measurement (the premise's), so
        // after it the rays' two frames are exactly two.
        if rt {
            s.run_reflected(&input, 2);
        } else {
            for _ in 0..2 {
                s.scene.surface.set_realtime_gi(Some(input.clone()));
                s.scene.render(None).expect("the frame renders");
            }
        }
        s.centre()
    };
    assert_ne!(measure(0.05, true), measure(0.05, false), "premise");
    assert_eq!(
        measure(0.9, true),
        measure(0.9, false),
        "a rough floor traces nothing"
    );
}

/// The hit's own light is traced, shadow ray and all: a card between the
/// lamp and the spot of the wall behind the camera that the mirror shows
/// darkens that spot in the mirror. The probes, a metre and more apart,
/// cannot draw a shadow that small -- with the hit lit by the probes alone,
/// the card changed the reflection by a few levels.
///
/// Measured: lit `[210, 211, 212]`, shadowed `[164, 166, 170]` (the grey
/// room's bounce stays in the shadow: 0.58 of the light); with the hit lit
/// by the probes alone, `[169, 171, 175]` and `[163, 164, 169]` (0.93).
#[test]
fn a_reflection_shows_the_shadow_on_what_it_reflects() {
    let rt = Some(bsengine_core::RayTracedReflections::default());
    // The spot the centre pixel reflects is (0, 1.5, 3); the lamp is off to
    // the side, and the card halfway between them, clear of the ray from the
    // mirror (which runs along x = 0) and behind the camera.
    let (lamp, spot) = (Vec3::new(1.6, 1.5, 2.0), Vec3::new(0.0, 1.5, 3.0));
    let measure = |card: bool| {
        let mut room = MirrorRoom::new(Behind::Grey, false, false);
        room.lamp_at(lamp);
        if card {
            room.card((lamp + spot) * 0.5, lamp - spot);
        }
        room.centre(rt, false)
    };
    let lit = measure(false);
    let shadowed = measure(true);
    assert!(
        lit.iter().any(|&c| c > 60),
        "premise: the spot is lit: {lit:?}"
    );
    assert!(
        linear_sum(shadowed) < 0.75 * linear_sum(lit),
        "the card's shadow is in the mirror: {lit:?} -> {shadowed:?}"
    );
}

impl MirrorRoom {
    /// The mirror's roughness.
    fn mirror_roughness(&mut self, r: f32) {
        self.s.scene.draw_calls[self.mirror].3.roughness = r;
    }

    /// Settles the probes, then runs the reflections `frames` frames, and
    /// returns the middle row of the last.
    fn middle_row(&mut self, frames: u32) -> Vec<[u8; 3]> {
        let rt = Some(bsengine_core::RayTracedReflections::default());
        self.s.scene.surface.set_ray_traced_reflections(rt);
        let input = self.input();
        self.s.run(&input, 6);
        self.s.run_reflected(&input, frames);
        self.row()
    }

    /// The middle row of the last frame.
    fn row(&self) -> Vec<[u8; 3]> {
        let pixels = self
            .s
            .scene
            .surface
            .read_pixels()
            .expect("offscreen readback");
        (0..64)
            .map(|x| {
                let at = (32 * 64 + x) * 4;
                [pixels[at], pixels[at + 1], pixels[at + 2]]
            })
            .collect()
    }
}

/// How many of the pixels the wall behind the camera fills in the mirror
/// (the middle columns: the mirror image is twice as far as the mirror, and
/// the side walls show beyond them) are neither plainly red nor plainly blue
/// -- one channel over the other by 100 levels: the width of the edge
/// between them. Not a ratio of the two: the probes carry each glow onto
/// the other half, and the mirror's plain blue reads `[87, 10, 246]`, its
/// plain red `[246, 8, 106]`.
fn edge_width(row: &[[u8; 3]]) -> usize {
    row[20..44]
        .iter()
        .filter(|p| (p[0] as i32 - p[2] as i32).abs() < 100)
        .count()
}

/// A glossy surface reflects a lobe, not a line: the edge between the red
/// and the blue halves of the wall behind the camera is sharp in a mirror
/// and spread in a surface of roughness 0.4 -- each frame one direction of
/// its GGX lobe, the frames blended. With one mirror ray a pixel (the rays
/// before the lobe was sampled) the two widths were the same.
///
/// Measured: 0 pixels of the 24 in the mirror, 23 at roughness 0.4 -- one
/// sample a frame, ten-odd frames' worth in the history, still grainy
/// (the spatial filter is the next step's).
#[test]
fn a_glossy_surface_blurs_what_it_reflects() {
    let mut sharp = MirrorRoom::new(Behind::Split, false, false);
    let mirror = sharp.middle_row(2);
    let mut glossy = MirrorRoom::new(Behind::Split, false, false);
    glossy.mirror_roughness(0.4);
    let blurred = glossy.middle_row(40);
    let (sharp_w, glossy_w) = (edge_width(&mirror), edge_width(&blurred));
    assert!(
        mirror[20..44]
            .iter()
            .any(|p| p[0] as i32 > p[2] as i32 + 100)
            && mirror[20..44]
                .iter()
                .any(|p| p[2] as i32 > p[0] as i32 + 100),
        "premise: the mirror shows the edge, red one side and blue the other: {mirror:?}"
    );
    assert!(sharp_w <= 2, "a mirror keeps the edge sharp: {sharp_w}");
    assert!(
        glossy_w >= sharp_w + 4,
        "a glossy surface spreads it: {glossy_w} pixels against {sharp_w}"
    );
}

/// The frames are blended: one sample of the lobe a frame is noise, and
/// the history settles it. Two frames in a row of the glossy edge differ
/// by little with the blend, and by a lot without it (the premise -- the
/// single samples do flicker).
///
/// Measured: the largest change of a channel along the row, 56 with the
/// history and 199 without.
#[test]
fn a_glossy_reflection_settles_over_frames() {
    let flicker = |history: bool| {
        let mut room = MirrorRoom::new(Behind::Split, false, false);
        room.mirror_roughness(0.4);
        room.middle_row(1);
        room.s
            .scene
            .surface
            .set_rt_reflection_history_for_testing(history);
        let a = room.middle_row(40);
        let b = room.middle_row(1);
        a.iter()
            .zip(&b)
            .map(|(p, q)| {
                (0..3)
                    .map(|c| (p[c] as i32 - q[c] as i32).abs())
                    .max()
                    .unwrap()
            })
            .max()
            .unwrap()
    };
    let (with, without) = (flicker(true), flicker(false));
    assert!(
        without > 20,
        "premise: one sample a frame flickers: {without}"
    );
    assert!(
        with * 3 < without,
        "the history settles it: {with} against {without}"
    );
}

/// The history is thrown away where the surface is not the one it recorded:
/// a panel brought in front of a glossy mirror that has been reflecting the
/// red wall for frames is, the first frame it is there, what it is in a
/// room where it always was -- not that plus the mirror's history, which
/// its depth says belongs to something else. (Not "not red": the panel is
/// rough metal facing the red wall, and the probes' stand-in for the sky
/// makes it red of its own accord.)
///
/// Measured: the mirror `[241, 11, 39]`; the panel brought in `[234, 45,
/// 164]`, and always there `[230, 45, 164]`.
#[test]
fn a_surface_uncovered_does_not_take_the_history_of_what_was_there() {
    let panel = |room: &mut MirrorRoom| {
        room.s.scene.push_quad(
            [
                Vec3::new(-1.0, 0.5, 0.0),
                Vec3::new(1.0, 0.5, 0.0),
                Vec3::new(1.0, 2.5, 0.0),
                Vec3::new(-1.0, 2.5, 0.0),
            ],
            Vec3::ZERO,
        );
        let i = room.s.scene.draw_calls.len() - 1;
        room.s.scene.draw_calls[i].3.base_color = Vec3::ONE;
        room.s.scene.draw_calls[i].3.metallic = 1.0;
        room.s.scene.draw_calls[i].3.roughness = 0.5;
    };
    let mut room = MirrorRoom::new(Behind::Red, false, false);
    room.mirror_roughness(0.3);
    let before = room.middle_row(30)[32];
    panel(&mut room);
    let after = room.middle_row(1)[32];
    let mut always = MirrorRoom::new(Behind::Red, false, false);
    always.mirror_roughness(0.3);
    panel(&mut always);
    always.middle_row(30);
    let reference = always.middle_row(1)[32];
    assert!(
        red_bias(before) > 60,
        "premise: the mirror was red: {before:?}"
    );
    assert!(
        (0..3).all(|c| (after[c] as i32 - reference[c] as i32).abs() <= 6),
        "the panel is what it would have been, not the mirror's history: \
         {after:?} against {reference:?}"
    );
}

/// By default the reflections trace one pixel in each 2 x 2 block -- a
/// quarter of the rays -- and every pixel with `full_resolution`.
#[test]
fn reflections_trace_at_half_resolution_unless_asked_for_full() {
    let size = |full: bool| {
        let mut room = MirrorRoom::new(Behind::Red, false, false);
        room.s.scene.surface.set_ray_traced_reflections(Some(
            bsengine_core::RayTracedReflections {
                full_resolution: full,
                ..Default::default()
            },
        ));
        let input = room.input();
        room.s.run(&input, 2);
        room.s.run_reflected(&input, 1);
        room.s.scene.surface.rt_reflection_trace_size()
    };
    assert_eq!(size(false), Some((32, 32)));
    assert_eq!(size(true), Some((64, 64)));
}

/// The grain of a row over the plainly red part of a glossy mirror (columns
/// 34 to 46, inside the card's reflection): the mean size of the green and
/// blue's second difference from pixel to pixel. A second difference, not a
/// spread: the blur's own gradient across those columns -- the blue of the
/// wall beyond the card, fading in -- is a slope, which this does not count.
/// Not the red, which is all but saturated there.
fn grain(row: &[[u8; 3]]) -> f32 {
    let v: Vec<f32> = row[34..46]
        .iter()
        .map(|p| f32::from(p[1]) + f32::from(p[2]))
        .collect();
    let d: Vec<f32> = v
        .windows(3)
        .map(|w| (w[0] - 2.0 * w[1] + w[2]).abs())
        .collect();
    d.iter().sum::<f32>() / d.len() as f32
}

/// The spatial filter takes out the grain the history leaves: over the
/// plainly red part of a glossy mirror, the red's spread along the row is
/// well under what it is without the filter (the premise: without it,
/// there is grain to take out).
///
/// Measured: 1.7 with the filter, 8.7 without.
#[test]
fn the_spatial_filter_takes_out_the_grain() {
    let measure = |spatial: bool| {
        let mut room = MirrorRoom::new(Behind::Split, false, false);
        room.mirror_roughness(0.4);
        room.middle_row(1);
        room.s
            .scene
            .surface
            .set_rt_reflection_spatial_for_testing(spatial);
        grain(&room.middle_row(40))
    };
    let (with, without) = (measure(true), measure(false));
    assert!(without > 8.0, "premise: there is grain: {without:.1}");
    assert!(
        with * 2.0 < without,
        "the filter takes it out: {with:.1} against {without:.1}"
    );
}

/// Neither the filter nor the upsample carries one surface's reflection
/// onto another across an edge: a narrow rough panel in front of a glossy
/// mirror reflecting red. The panel traces nothing (too rough), so what
/// would spill is its *nothing* onto the mirror beside it, whose reflection
/// would then fall away toward the panel -- the mirror's blue climbing
/// column by column as the probes' stand-in shows through -- and the
/// mirror's red onto the panel's edges. With the neighbours weighed by
/// depth, the mirror runs smoothly up to the panel and the panel is the same
/// from edge to edge. (The panel's own red is the probes' stand-in for the
/// sky, the same all across it.)
///
/// Measured: the mirror's blue toward the panel 71, 71, 70, 69, 68; with the
/// filter blind to depth, 71, 108, 134, 161, 179, and the panel's edges 3
/// bluer than its middle.
#[test]
fn a_reflection_does_not_spill_across_an_edge() {
    let mut room = MirrorRoom::new(Behind::Red, false, false);
    room.mirror_roughness(0.3);
    // x from -0.3 to 0.3 at z = 0, two metres from the camera: columns 23
    // to 40 of the row.
    room.s.scene.push_quad(
        [
            Vec3::new(-0.3, 0.5, 0.0),
            Vec3::new(0.3, 0.5, 0.0),
            Vec3::new(0.3, 2.5, 0.0),
            Vec3::new(-0.3, 2.5, 0.0),
        ],
        Vec3::ZERO,
    );
    let i = room.s.scene.draw_calls.len() - 1;
    room.s.scene.draw_calls[i].3.base_color = Vec3::ONE;
    room.s.scene.draw_calls[i].3.metallic = 1.0;
    room.s.scene.draw_calls[i].3.roughness = 0.9;
    let row = room.middle_row(30);
    let blue = |c: usize| row[c][2] as i32;
    // Told apart by their blue: both are red, the panel by the probes.
    assert!(
        blue(20) > blue(32) + 30,
        "premise: the mirror beside the panel is another colour: {:?} against {:?}",
        row[20],
        row[32]
    );
    let mirror_steps = (18..22)
        .chain(41..45)
        .map(|c| (blue(c + 1) - blue(c)).abs())
        .max()
        .unwrap();
    assert!(
        mirror_steps <= 10,
        "the mirror runs smoothly up to the panel: steps of {mirror_steps} ({:?})",
        &row[18..47]
    );
    let panel_spread = (24..40).map(|c| (blue(c) - blue(32)).abs()).max().unwrap();
    assert!(
        panel_spread <= 2,
        "the panel is the same from edge to edge: {panel_spread} ({:?})",
        &row[23..41]
    );
}
