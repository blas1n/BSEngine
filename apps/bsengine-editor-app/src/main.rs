use bsengine_app::{new_app, Startup};
use bsengine_core::{Camera, DirectionalLight, GlobalTransform, InspectorState, Transform};
use bsengine_ecs::{Commands, ResMut};
use bsengine_editor::EditorPlugin;
use bsengine_gltf::GltfPlugin;
use bsengine_input::InputPlugin;
use bsengine_render::{MeshRenderer, RenderPlugin};
use bsengine_rhi_wgpu::{GpuMeshRegistry, WgpuRHIPlugin};
use bsengine_scene::ScenePlugin;
use bsengine_window::{WindowDescriptor, WindowPlugin};
use glam::{Quat, Vec3};
use std::env;

fn main() {
    let scene_path = env::args().nth(1);

    let mut app = new_app();
    app.add_plugins(WgpuRHIPlugin)
        .add_plugins(WindowPlugin {
            descriptor: WindowDescriptor {
                title: "BSEngine Editor".to_string(),
                width: 1600,
                height: 900,
                resizable: true,
            },
        })
        .add_plugins(InputPlugin)
        .add_plugins(GltfPlugin)
        .add_plugins(RenderPlugin)
        .add_plugins(EditorPlugin)
        .insert_resource(InspectorState::editor());

    match scene_path {
        Some(path) => {
            app.add_plugins(ScenePlugin::from_file(&path));
        }
        None => {
            app.add_systems(Startup, setup_empty_scene);
        }
    }

    app.run();
}

fn setup_empty_scene(mut commands: Commands, mut registry: Option<ResMut<GpuMeshRegistry>>) {
    commands.spawn((
        Camera::perspective(60.0, 16.0 / 9.0),
        Transform::from_translation(Vec3::new(0.0, 3.0, 10.0)),
    ));

    commands.spawn(DirectionalLight::default());

    // Default ground plane so the viewport is not completely empty
    if let Some(ref mut reg) = registry {
        let (verts, indices) = bsengine_rhi_wgpu::cube_vertices();
        let mesh_id = reg.register(&verts, &indices);
        commands.spawn((
            MeshRenderer { mesh_id },
            Transform {
                translation: Vec3::new(0.0, -0.1, 0.0),
                rotation: Quat::IDENTITY,
                scale: Vec3::new(20.0, 0.2, 20.0),
            },
            GlobalTransform::default(),
        ));
    }
}
