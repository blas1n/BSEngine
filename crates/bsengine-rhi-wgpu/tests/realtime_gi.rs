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
                None,
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
                Mat4::IDENTITY,
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
/// backend, through FXC -- off the frame on a hardware adapter and in place on
/// a software one. Every
/// other test takes whatever adapter comes first -- Vulkan, on most machines
/// -- so a construct FXC rejects (an array-returning function was one) would
/// pass them all and fail only where D3D12 happened to be chosen.
#[cfg(windows)]
#[test]
fn the_update_compiles_for_d3d12() {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::DX12,
        ..Default::default()
    });
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::LowPower,
        compatible_surface: None,
        force_fallback_adapter: false,
    }))
    .expect("Windows has a D3D12 adapter (WARP at worst); a skip would look like a pass");
    let (device, _queue) = pollster::block_on(adapter.request_device(
        &wgpu::DeviceDescriptor {
            label: Some("d3d12 compile check"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::downlevel_defaults(),
            memory_hints: wgpu::MemoryHints::default(),
        },
        None,
    ))
    .expect("a D3D12 device");
    let device = std::sync::Arc::new(device);
    // What the surface decides by: a hardware adapter here locally, WARP on
    // Windows CI -- so each machine checks one side of the choice.
    let software = adapter.get_info().device_type == wgpu::DeviceType::Cpu;
    device.push_error_scope(wgpu::ErrorFilter::Validation);
    let (returned, built, ready) =
        bsengine_rhi_wgpu::realtime_gi::build_pipelines_for_testing(&device, software);
    let error = pollster::block_on(device.pop_error_scope());
    assert!(
        error.is_none(),
        "the update does not compile for D3D12: {error:?}"
    );
    assert!(ready, "the pipelines were built");
    // The premise -- the compile is slow enough to matter -- is FXC's,
    // seconds on any machine.
    assert!(
        built.as_secs_f32() > 0.5,
        "premise: FXC takes a while: {built:?}"
    );
    if software {
        // In place on a software adapter: a frame running beside the compile
        // waited on a fence WARP would not signal until it was done, past
        // wgpu's 60 s limit, and wgpu then freed what the GPU still used
        // (`realtime_gi::PipelineState`).
        assert!(
            returned * 10 >= built * 9,
            "on a software adapter the update builds in place: {returned:?} of {built:?}"
        );
    } else {
        // Off the frame: `new` hands the compile to a thread and returns.
        assert!(
            returned < built / 4,
            "creating the update must not wait for its compile: {returned:?} of {built:?}"
        );
    }
}

// ---- the realtime probe volume -------------------------------------------

/// A scene for the realtime volume: [`Surface`]'s surface and
/// registry, with quads registered as the static set (their draws are
/// irrelevant -- realtime GI traces `statics`, not the draw list).
struct GiScene {
    scene: Surface,
    statics: Vec<bsengine_rhi_wgpu::realtime_gi::GiStatic>,
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

    fn input(
        &self,
        volume: ProbeVolumeParams,
        hysteresis: f32,
        rays_per_frame: u32,
    ) -> bsengine_rhi_wgpu::realtime_gi::RealtimeGiInput {
        bsengine_rhi_wgpu::realtime_gi::RealtimeGiInput {
            volume,
            rays_per_probe: 256,
            hysteresis,
            rays_per_frame,
            statics: self.statics.clone(),
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
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(300);
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

/// The budget: rays for a quarter of the probes a frame means each
/// frame updates the next quarter, and four frames cover every probe
/// exactly -- the round robin neither skips nor repeats.
#[test]
fn the_ray_budget_updates_the_grid_a_slice_at_a_time() {
    let mut s = furnace_scene(0.4, 0.5);
    let volume = ProbeVolumeParams {
        resolution: [4, 4, 4],
        ..inner_volume()
    };
    let input = s.input(volume, 0.5, 256 * 16);
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
    s.run(&input, 3);
    let n = 30;
    let device = s.scene.surface.device_arc();
    let t = std::time::Instant::now();
    for _ in 0..n {
        s.run(&input, 1);
        device.poll(wgpu::Maintain::Wait);
    }
    eprintln!(
        "BENCH realtime gi: 4096 probes x 64 rays, {} triangles: {:.2} ms per frame (update + frame)",
        s.statics.len() * 2,
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

/// A white quad filling the frame at z = 0.5, facing the camera (+z), and
/// a 2 x 2 x 2 realtime grid around it from z = 0 to z = 1.6: the scene of
/// the two visibility tests. Returns the centre pixel after a few updates
/// with visibility on or off.
fn visibility_scene(s: &mut GiScene, visibility: bool) -> [u8; 3] {
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
        // The middle row, pixels 17..47: x from -0.47 to 0.47, across the
        // probes at x = +-0.44 (cells 0.89 wide). Not wider: the renderer
        // brightens the frame's outer 16-pixel tiles by a step of its own,
        // with or without a probe volume (a screen-space pass, not GI).
        (17..47)
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
            Vec3::splat(2.0),
        );
        s
    };
    let darkened = visibility_scene(&mut build(), false);
    let seen = visibility_scene(&mut build(), true);
    assert!(
        (0..3).all(|c| seen[c] as i32 > darkened[c] as i32 + 40),
        "the buried probes drop out and the far ones light the quad: {darkened:?} -> {seen:?}"
    );
}
