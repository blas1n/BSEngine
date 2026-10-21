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
/// backend, through FXC -- and without making a frame wait for it. Every
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
    device.push_error_scope(wgpu::ErrorFilter::Validation);
    let (returned, built, ready) =
        bsengine_rhi_wgpu::realtime_gi::build_pipelines_for_testing(&device);
    let error = pollster::block_on(device.pop_error_scope());
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
        "BENCH realtime gi: 4096 probes x 64 rays, {} triangles, {} instances: {:.2} ms per frame (update + frame)",
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
    let bg = at(0, 0);
    let rows: Vec<usize> = (0..64).filter(|&y| at(32, y) != bg).collect();
    let cols: Vec<usize> = (0..64).filter(|&x| at(x, 32) != bg).collect();
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
