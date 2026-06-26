use bevy_ecs::prelude::*;
use bsengine_app::{new_app, Update};
use bsengine_core::{Camera, DirectionalLight, GlobalTransform, Material, Transform};
use bsengine_gltf::GltfPlugin;
use bsengine_render::{MeshRenderer, RenderPlugin};
use bsengine_rhi_wgpu::{cube_vertices, GpuMeshRegistry, WgpuRHIPlugin};
use bsengine_window::{WindowDescriptor, WindowPlugin};
use glam::{Quat, Vec3};

fn setup(mut commands: Commands, registry: Option<ResMut<GpuMeshRegistry>>, mut done: Local<bool>) {
    if *done {
        return;
    }
    let Some(mut reg) = registry else {
        return;
    };

    let (verts, indices) = cube_vertices();
    let mesh_id = reg.register(&verts, &indices);

    commands.spawn((
        MeshRenderer { mesh_id },
        Transform::from_translation(Vec3::new(0.0, 0.0, -4.0)),
        GlobalTransform::default(),
        Material {
            metallic: 0.1,
            roughness: 0.6,
            ..Default::default()
        },
    ));

    commands.spawn((
        Camera::default(),
        Transform::from_translation(Vec3::new(0.0, 1.5, 3.0)),
    ));

    commands.spawn(DirectionalLight {
        direction: Vec3::new(-0.5, -1.0, -0.5).normalize(),
        color: Vec3::ONE,
        ambient: Vec3::splat(0.15),
    });

    tracing::info!("Demo scene ready — cube spawned");
    *done = true;
}

fn spin(mut query: Query<&mut Transform, With<MeshRenderer>>) {
    for mut t in query.iter_mut() {
        t.rotation *= Quat::from_rotation_y(0.01);
    }
}

fn main() {
    bsengine_core::init_logging();

    let mut app = new_app();
    app.add_plugins(WindowPlugin {
        descriptor: WindowDescriptor {
            title: "BSEngine Demo".to_string(),
            width: 1280,
            height: 720,
            resizable: true,
        },
    });
    app.add_plugins(WgpuRHIPlugin);
    app.add_plugins(RenderPlugin);
    app.add_plugins(GltfPlugin);
    app.add_systems(Update, (setup, spin));
    app.run();
}
