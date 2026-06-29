use std::env;

use bevy_app::PostStartup;
use bsengine_app::new_app;
use bsengine_ecs::{Commands, Entity, Query, ResMut};
use bsengine_input::InputPlugin;
use bsengine_render::{MeshRenderer, RenderPlugin};
use bsengine_rhi_wgpu::{cube_vertices, GpuMeshRegistry, WgpuRHIPlugin};
use bsengine_scene::{Primitive, PrimitiveMesh, ScenePlugin};
use bsengine_window::{WindowDescriptor, WindowPlugin};
use serde::Deserialize;

#[derive(Deserialize)]
struct ProjectManifest {
    project: ProjectSection,
    #[serde(default)]
    window: WindowSection,
}

#[derive(Deserialize)]
struct ProjectSection {
    name: String,
    entry_scene: String,
}

#[derive(Deserialize, Default)]
struct WindowSection {
    #[serde(default)]
    title: Option<String>,
    #[serde(default = "default_width")]
    width: u32,
    #[serde(default = "default_height")]
    height: u32,
    #[serde(default = "default_true")]
    resizable: bool,
}

fn default_width() -> u32 {
    1280
}
fn default_height() -> u32 {
    720
}
fn default_true() -> bool {
    true
}

fn main() {
    let project_dir = env::args().nth(1).unwrap_or_else(|| ".".to_string());
    let manifest_path = format!("{}/project.toml", project_dir);

    let manifest_str = std::fs::read_to_string(&manifest_path)
        .unwrap_or_else(|e| panic!("Cannot read {manifest_path}: {e}"));
    let manifest: ProjectManifest = toml::from_str(&manifest_str)
        .unwrap_or_else(|e| panic!("Cannot parse {manifest_path}: {e}"));

    let scene_path = format!("{}/{}", project_dir, manifest.project.entry_scene);
    let title = manifest
        .window
        .title
        .unwrap_or_else(|| manifest.project.name.clone());

    new_app()
        .add_plugins(WgpuRHIPlugin)
        .add_plugins(WindowPlugin {
            descriptor: WindowDescriptor {
                title,
                width: manifest.window.width,
                height: manifest.window.height,
                resizable: manifest.window.resizable,
            },
        })
        .add_plugins(InputPlugin)
        .add_plugins(RenderPlugin)
        .add_plugins(ScenePlugin::from_file(&scene_path))
        .add_systems(PostStartup, resolve_primitives)
        .run();
}

fn resolve_primitives(
    query: Query<(Entity, &PrimitiveMesh)>,
    mut commands: Commands,
    registry: Option<ResMut<GpuMeshRegistry>>,
) {
    let Some(mut registry) = registry else { return };

    let mut cube_id: Option<u64> = None;

    for (entity, prim) in query.iter() {
        match &prim.0 {
            Primitive::Cube => {
                let id = *cube_id.get_or_insert_with(|| {
                    let (verts, indices) = cube_vertices();
                    registry.register(&verts, &indices)
                });
                commands.entity(entity).insert(MeshRenderer { mesh_id: id });
            }
        }
    }
}
