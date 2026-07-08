use bevy_app::{PostStartup, Update};
use bsengine_app::new_app;
use bsengine_core::{Camera, DirectionalLight, GlobalTransform, Parent, PointLight, Transform};
use bsengine_ecs::{Added, Commands, Entity, Query, Res, ResMut};
use bsengine_editor::EditorPlugin;
use bsengine_input::InputPlugin;
use bsengine_mcp::{McpPlugin, McpRegistryResource, McpServer};
use bsengine_render::{MeshRenderer, RenderPlugin};
use bsengine_rhi_wgpu::{
    capsule_vertices, cube_vertices, plane_vertices, sphere_vertices, GpuMeshRegistry,
    WgpuRHIPlugin,
};
use bsengine_scene::{Primitive, PrimitiveMesh};
use bsengine_window::WindowPlugin;
use glam::Vec3;

fn main() {
    new_app()
        .add_plugins(WgpuRHIPlugin)
        .add_plugins(WindowPlugin::default())
        .add_plugins(InputPlugin)
        .add_plugins(RenderPlugin)
        .add_plugins(McpPlugin)
        .add_plugins(EditorPlugin)
        .add_systems(PostStartup, (setup, start_mcp_server))
        .add_systems(Update, resolve_primitives)
        .run();
}

fn start_mcp_server(registry: Res<McpRegistryResource>) {
    let arc = registry.0.clone();
    std::thread::spawn(move || {
        McpServer::new(arc).run_stdio();
    });
}

fn resolve_primitives(
    query: Query<(Entity, &PrimitiveMesh), Added<PrimitiveMesh>>,
    mut commands: Commands,
    registry: Option<ResMut<GpuMeshRegistry>>,
) {
    let Some(mut registry) = registry else { return };
    let mut cube_id: Option<u64> = None;
    let mut sphere_id: Option<u64> = None;
    let mut plane_id: Option<u64> = None;
    let mut capsule_id: Option<u64> = None;
    for (entity, prim) in query.iter() {
        let mesh_id = match &prim.0 {
            Primitive::Cube => *cube_id.get_or_insert_with(|| {
                let (v, i) = cube_vertices();
                registry.register(&v, &i)
            }),
            Primitive::Sphere => *sphere_id.get_or_insert_with(|| {
                let (v, i) = sphere_vertices();
                registry.register(&v, &i)
            }),
            Primitive::Plane => *plane_id.get_or_insert_with(|| {
                let (v, i) = plane_vertices();
                registry.register(&v, &i)
            }),
            Primitive::Capsule => *capsule_id.get_or_insert_with(|| {
                let (v, i) = capsule_vertices();
                registry.register(&v, &i)
            }),
        };
        commands.entity(entity).insert(MeshRenderer { mesh_id });
    }
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
