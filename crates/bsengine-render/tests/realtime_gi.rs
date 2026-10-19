//! The realtime light-probe volume through the render plugin: the world's
//! `LightProbeVolume { update: Realtime }` reaching the surface's update.
//!
//! A process of its own, for the reason `bsengine-rhi-wgpu/tests/realtime_gi.rs`
//! gives: on Windows CI the update's pipelines take over a minute to build on
//! the software D3D12 adapter, and run beside the crate's other GPU tests that
//! crashed the test process on teardown, three runs out of three.

use bsengine_app::new_app;
use bsengine_core::{Camera, Material, Transform};
use bsengine_render::{MeshRenderer, RenderPlugin};
use bsengine_rhi_wgpu::{GpuMeshRegistry, Vertex, WgpuRHIPlugin};
use glam::Vec3;

fn surface_of(app: &bevy_app::App) -> &bsengine_rhi_wgpu::surface::WgpuSurface {
    &app.world()
        .resource::<bsengine_rhi_wgpu::WgpuSurfaceResource>()
        .0
}

/// A `Realtime` volume reaches the surface's realtime update -- through
/// `feed_realtime_gi`, with the static set read from the world -- and is
/// not baked; switched to `Once`, it is baked instead and the update
/// stops. The two paths share one probe grid, so this is the hand-over
/// both ways.
#[test]
fn a_realtime_volume_is_traced_not_baked_and_a_once_volume_baked() {
    use bsengine_core::{LightProbeVolume, LightmapStatic, ProbeUpdate};
    let mut app = new_app();
    app.add_plugins(bsengine_asset::AssetPlugin);
    // The windowed plugin with no window builds no surface; the test's comes
    // from the process's shared test device, the way the crate's other GPU
    // tests get theirs -- one device, rather than a fresh one compiling the
    // update's pipelines under FXC beside CI's software D3D12 (that, with a
    // frame loop spinning beside the build, is what crashed the process).
    app.add_plugins(WgpuRHIPlugin::windowed());
    app.add_plugins(RenderPlugin);
    let surface = bsengine_rhi_wgpu::surface::WgpuSurface::offscreen_for_testing(64, 64)
        .expect("these tests need an adapter; a skip here would look like a pass");
    app.insert_resource(GpuMeshRegistry::new(surface.device_arc()));
    app.insert_resource(bsengine_rhi_wgpu::GpuTextureRegistry::new(
        surface.device_arc(),
        surface.queue_arc(),
    ));
    app.insert_resource(bsengine_rhi_wgpu::WgpuSurfaceResource(surface));
    app.update();
    let v = |x: f32, z: f32| Vertex {
        position: [x, 0.0, z],
        color: [1.0, 1.0, 1.0],
        normal: [0.0, 1.0, 0.0],
        uv: [0.0, 0.0],
        uv2: [0.0, 0.0],
    };
    let mesh_id = app.world_mut().resource_mut::<GpuMeshRegistry>().register(
        &[v(-5.0, 5.0), v(5.0, 5.0), v(5.0, -5.0), v(-5.0, -5.0)],
        &[0, 1, 2, 0, 2, 3],
    );
    app.world_mut().spawn((
        Camera::default(),
        Transform::from_position(Vec3::new(0.0, 3.0, 10.0)),
    ));
    app.world_mut().spawn((
        MeshRenderer { mesh_id },
        LightmapStatic::default(),
        Transform::default(),
        Material {
            emissive: Vec3::splat(1.0).into(),
            ..Material::default()
        },
    ));
    let volume = app
        .world_mut()
        .spawn((
            LightProbeVolume {
                update: ProbeUpdate::Realtime,
                resolution: [2, 2, 2],
                half_extents: Vec3::splat(1.0).into(),
                ..LightProbeVolume::default()
            },
            Transform::from_position(Vec3::new(0.0, 1.5, 0.0)),
        ))
        .id();
    // Against the clock, not a frame count: the update's pipelines are
    // built on a thread, which under FXC on CI's software D3D12 adapter
    // takes over a minute (`realtime_gi::PipelineState`).
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(300);
    let started = std::time::Instant::now();
    let mut seen = u64::MAX;
    while surface_of(&app).realtime_gi_updates() < 2 {
        assert!(
            std::time::Instant::now() < deadline,
            "the realtime volume never reached the surface's update"
        );
        let n = surface_of(&app).realtime_gi_updates() as u64;
        if n != seen {
            tracing::warn!("DIAG updates={n} at {:?}", started.elapsed());
            seen = n;
        }
        tracing::warn!("DIAG frame start at {:?}", started.elapsed());
        app.update();
        tracing::warn!("DIAG frame end at {:?}", started.elapsed());
        // Frames a few times a second while the pipelines build: enough to
        // land them, not thousands of WARP frames beside the compile.
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    assert!(
        !surface_of(&app).has_baked_probe_volume(),
        "a realtime volume is not also baked"
    );

    app.world_mut()
        .get_mut::<LightProbeVolume>(volume)
        .unwrap()
        .update = ProbeUpdate::Once;
    let updates = surface_of(&app).realtime_gi_updates();
    tracing::warn!("DIAG switched to Once at {:?}", started.elapsed());
    app.update();
    tracing::warn!("DIAG bake frame 1 done at {:?}", started.elapsed());
    app.update();
    tracing::warn!("DIAG bake frame 2 done at {:?}", started.elapsed());
    assert!(surface_of(&app).has_baked_probe_volume(), "now it is baked");
    assert_eq!(
        surface_of(&app).realtime_gi_updates(),
        updates,
        "and the realtime update has stopped"
    );
    tracing::warn!("DIAG dropping the app at {:?}", started.elapsed());
    drop(app);
    tracing::warn!("DIAG dropped at {:?}", started.elapsed());
}
