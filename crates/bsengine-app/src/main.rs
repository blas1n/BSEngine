use bevy_app::PostStartup;
use bsengine_app::new_app;
use bsengine_core::{Camera, DirectionalLight, GlobalTransform, Parent, PointLight, Transform};
use bsengine_ecs::{Commands, ResMut};
use bsengine_input::InputPlugin;
use bsengine_render::{MeshRenderer, RenderPlugin};
use bsengine_rhi_wgpu::{cube_vertices, GpuMeshRegistry, WgpuRHIPlugin};
use bsengine_window::WindowPlugin;
use glam::Vec3;

fn main() {
    new_app()
        .add_plugins(WgpuRHIPlugin)
        .add_plugins(WindowPlugin::default())
        .add_plugins(InputPlugin)
        .add_plugins(RenderPlugin)
        .add_systems(PostStartup, setup)
        .run();
}

fn setup(mut commands: Commands, registry: Option<ResMut<GpuMeshRegistry>>) {
    // Camera at (0, 1.5, 4) with default rotation looks down -Z toward origin
    commands.spawn((
        Camera::perspective(60.0, 16.0 / 9.0),
        Transform::from_translation(Vec3::new(0.0, 1.5, 4.0)),
    ));

    let Some(mut registry) = registry else { return };
    let (verts, indices) = cube_vertices();
    let cube_id = registry.register(&verts, &indices);

    let parent = commands
        .spawn((
            MeshRenderer { mesh_id: cube_id },
            Transform::from_translation(Vec3::new(-1.2, 0.0, 0.0)),
            GlobalTransform::default(),
        ))
        .id();

    commands.spawn((
        MeshRenderer { mesh_id: cube_id },
        Transform::from_translation(Vec3::new(1.2, 0.0, 0.0)),
        GlobalTransform::default(),
        Parent(parent),
    ));

    commands.spawn(DirectionalLight::default());

    // Orange point light above the scene
    commands.spawn((
        PointLight {
            color: Vec3::new(1.0, 0.5, 0.1),
            intensity: 2.0,
            range: 8.0,
        },
        Transform::from_translation(Vec3::new(0.0, 3.0, 1.0)),
        GlobalTransform::default(),
    ));
}
