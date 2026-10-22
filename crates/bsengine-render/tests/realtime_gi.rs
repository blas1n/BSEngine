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
        tangent: [0.0; 4],
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
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(600); // see rhi tests/realtime_gi.rs `run`
    while surface_of(&app).realtime_gi_updates() < 2 {
        assert!(
            std::time::Instant::now() < deadline,
            "the realtime volume never reached the surface's update"
        );
        app.update();
        // Frames a few times a second while the pipelines build: enough to
        // land them, not thousands of WARP frames beside the compile.
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    assert!(
        !surface_of(&app).has_baked_probe_volume(),
        "a realtime volume is not also baked"
    );
    assert!(
        surface_of(&app).realtime_gi_bricks_for_testing().is_none(),
        "a grid volume has no bricks"
    );

    // Adaptive: the component's spacings reach the layout. The 2 m box is
    // one top brick (the max, 0.75, rounds the 2 m to whole bricks of three
    // spacings, 0.67), and 0.2 lets it be cut once (0.22).
    {
        let mut v = app.world_mut().get_mut::<LightProbeVolume>(volume).unwrap();
        v.placement = bsengine_core::ProbePlacement::Adaptive;
        v.min_probe_spacing = 0.2;
        v.max_probe_spacing = 0.75;
    }
    let updates = surface_of(&app).realtime_gi_updates();
    while surface_of(&app).realtime_gi_updates() < updates + 2 {
        assert!(std::time::Instant::now() < deadline, "no adaptive update");
        app.update();
    }
    let layout = surface_of(&app)
        .realtime_gi_bricks_for_testing()
        .expect("an adaptive volume has bricks");
    assert_eq!(layout.top, [1, 1, 1]);
    assert_eq!(layout.levels, 2, "min_probe_spacing reached the layout");
    assert!((layout.bricks[0].size - Vec3::splat(2.0)).length() < 1e-4);

    app.world_mut()
        .get_mut::<LightProbeVolume>(volume)
        .unwrap()
        .update = ProbeUpdate::Once;
    let updates = surface_of(&app).realtime_gi_updates();
    app.update();
    app.update();
    assert!(surface_of(&app).has_baked_probe_volume(), "now it is baked");
    assert_eq!(
        surface_of(&app).realtime_gi_updates(),
        updates,
        "and the realtime update has stopped"
    );
}

/// Which renderers the realtime volume traces as moving instances: every
/// one by default, but not a `LightmapStatic` one (that is the static tree),
/// not a hidden one, and not one marked `ExcludeFromGi`. Five renderers, one
/// of each and two plain, trace as two instances -- and moving one of them
/// reads nothing back: the readback count stays at the first.
#[test]
fn every_moving_renderer_takes_part_unless_excluded_hidden_or_static() {
    use bsengine_core::{ExcludeFromGi, LightProbeVolume, LightmapStatic, ProbeUpdate, Visible};
    let mut app = new_app();
    app.add_plugins(bsengine_asset::AssetPlugin);
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
        tangent: [0.0; 4],
    };
    let mesh_id = app.world_mut().resource_mut::<GpuMeshRegistry>().register(
        &[v(-1.0, 1.0), v(1.0, 1.0), v(1.0, -1.0), v(-1.0, -1.0)],
        &[0, 1, 2, 0, 2, 3],
    );
    app.world_mut().spawn((
        Camera::default(),
        Transform::from_position(Vec3::new(0.0, 3.0, 10.0)),
    ));
    let plain = app
        .world_mut()
        .spawn((MeshRenderer { mesh_id }, Transform::default()))
        .id();
    app.world_mut().spawn((
        MeshRenderer { mesh_id },
        Transform::from_position(Vec3::new(2.0, 0.0, 0.0)),
        Material::default(),
    ));
    app.world_mut().spawn((
        MeshRenderer { mesh_id },
        Transform::default(),
        ExcludeFromGi,
    ));
    app.world_mut().spawn((
        MeshRenderer { mesh_id },
        Transform::default(),
        Visible::hidden(),
    ));
    app.world_mut().spawn((
        MeshRenderer { mesh_id },
        Transform::default(),
        LightmapStatic::default(),
    ));
    app.world_mut().spawn((
        LightProbeVolume {
            update: ProbeUpdate::Realtime,
            resolution: [2, 2, 2],
            half_extents: Vec3::splat(1.0).into(),
            ..LightProbeVolume::default()
        },
        Transform::from_position(Vec3::new(0.0, 1.5, 0.0)),
    ));
    let run = |app: &mut bevy_app::App, n: u32| {
        let target = surface_of(app).realtime_gi_updates() + n;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(600); // see rhi tests/realtime_gi.rs `run`
        while surface_of(app).realtime_gi_updates() < target {
            assert!(
                std::time::Instant::now() < deadline,
                "the realtime volume never updated"
            );
            app.update();
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    };
    run(&mut app, 2);
    assert_eq!(
        surface_of(&app).realtime_gi_instances(),
        2,
        "the two plain renderers, and none of the excluded, hidden or static ones"
    );
    let readbacks = surface_of(&app).realtime_gi_readbacks();
    app.world_mut()
        .get_mut::<Transform>(plain)
        .unwrap()
        .position = Vec3::new(0.0, 0.5, 0.0).into();
    run(&mut app, 2);
    assert_eq!(
        surface_of(&app).realtime_gi_readbacks(),
        readbacks,
        "a renderer that moves is traced where it is, with nothing read back"
    );
}

/// An adaptive volume baked `Once` with baked probes (`BakedLightProbes`, from
/// the scene's lightmap data) reaches the surface through
/// `feed_baked_probes`: the file is read, the grid holds its bricks, and the
/// raster bake does not run. Taken away, the grid is baked as before; a file
/// that cannot be read is said once, and the grid is baked too.
#[test]
fn baked_probes_reach_the_surface_and_hand_back_to_the_grid() {
    use bsengine_core::{BakedLightProbes, LightProbeVolume, ProbePlacement, ProbeUpdate};
    let mut app = new_app();
    app.add_plugins(bsengine_asset::AssetPlugin);
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
    app.world_mut().spawn((
        Camera::default(),
        Transform::from_position(Vec3::new(0.0, 3.0, 10.0)),
    ));

    // A small baked volume: one brick, its probes all a uniform sky.
    let layout = bsengine_bake::probe_bricks::BrickLayout::build(
        &bsengine_bake::probe_bricks::BrickSettings {
            origin: Vec3::splat(-1.0),
            extent: Vec3::splat(2.0),
            min_spacing: 0.5,
            max_spacing: 0.7,
            max_probes: 4096,
            max_bricks: 512,
        },
        &[],
    );
    let baked = bsengine_bake::probe_bake::bake_probes(
        &bsengine_bake::BakeInput {
            triangles: &[],
            renderers: &[],
            lights: &[],
            sky: &bsengine_bake::Sky::Uniform(Vec3::splat(0.5)),
        },
        layout,
        &bsengine_bake::probe_bake::ProbeBakeSettings {
            rays: 64,
            bounces: 1,
            seed: 0,
        },
    );
    let file =
        std::env::temp_dir().join(format!("bsengine_baked_probes_{}.bin", std::process::id()));
    std::fs::write(&file, bsengine_bake::probe_bake::encode(&baked)).unwrap();
    let path = file.to_string_lossy().replace('\\', "/");

    let volume = app
        .world_mut()
        .spawn((
            LightProbeVolume {
                placement: ProbePlacement::Adaptive,
                update: ProbeUpdate::Once,
                half_extents: Vec3::splat(1.0).into(),
                ..LightProbeVolume::default()
            },
            BakedLightProbes { path: path.clone() },
            Transform::default(),
        ))
        .id();
    app.update();
    app.update();
    assert!(
        surface_of(&app).has_baked_probe_bricks(),
        "the baked probes are what the grid holds"
    );
    assert!(
        !surface_of(&app).has_baked_probe_volume(),
        "and the raster bake did not run"
    );

    app.world_mut()
        .entity_mut(volume)
        .remove::<BakedLightProbes>();
    app.update();
    app.update();
    assert!(!surface_of(&app).has_baked_probe_bricks());
    assert!(
        surface_of(&app).has_baked_probe_volume(),
        "without them the volume bakes its grid"
    );

    app.world_mut().entity_mut(volume).insert(BakedLightProbes {
        path: format!("{path}.missing"),
    });
    app.update();
    app.update();
    assert!(
        !surface_of(&app).has_baked_probe_bricks(),
        "an unreadable file is none"
    );
    assert!(surface_of(&app).has_baked_probe_volume());
    let _ = std::fs::remove_file(&file);
}
