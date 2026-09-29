//! Tests of the editor plugin, one file per tool group plus `systems` for
//! the ECS side driven directly. Imports and shared fixtures live here.

use super::{build_entity_descriptors, save_entities_as_prefab, EditorPlugin};
use bsengine_app::new_app;
use bsengine_core::{InspectorCmd, InspectorState, Parent, Transform};
use bsengine_mcp::{McpPlugin, McpRegistryResource};
use bsengine_scene::Name;
use glam::Vec3;
use serde_json::json;

use crate::snapshot::{
    EditorCommand, EditorCommandQueueResource, EditorSelectionResource, EditorSnapshotResource,
    EntityInfo, Tags,
};

fn entity_info(id: u64, name: &str, parent_id: Option<u64>) -> EntityInfo {
    EntityInfo {
        id,
        name: Some(name.to_string()),
        parent_id,
        ..Default::default()
    }
}

/// An editor app with two emitters: a burst-only one and a continuous
/// one, each holding a few live particles, so a restart has something
/// to drop and the two kinds of "start" can be told apart.
fn app_with_two_emitters() -> (
    bevy_app::App,
    bevy_ecs::entity::Entity,
    bevy_ecs::entity::Entity,
) {
    use bsengine_core::{Particle, ParticleEmitter};
    let mut app = new_app();
    app.add_plugins(EditorPlugin);
    let live = || {
        vec![
            Particle {
                position: glam::Vec3::ZERO,
                velocity: glam::Vec3::Y,
                age: 0.1,
            };
            3
        ]
    };
    let burst_only = app
        .world_mut()
        .spawn((
            Name("Sparks".into()),
            Transform::default(),
            ParticleEmitter {
                rate: 0.0,
                burst_count: 7,
                live: live(),
                spawn_debt: 0.4,
                ..Default::default()
            },
        ))
        .id();
    let continuous = app
        .world_mut()
        .spawn((
            Name("Smoke".into()),
            Transform::default(),
            ParticleEmitter {
                rate: 12.0,
                burst_count: 5,
                live: live(),
                spawn_debt: 0.4,
                ..Default::default()
            },
        ))
        .id();
    (app, burst_only, continuous)
}

fn emitter(app: &bevy_app::App, e: bevy_ecs::entity::Entity) -> &bsengine_core::ParticleEmitter {
    app.world()
        .get::<bsengine_core::ParticleEmitter>(e)
        .unwrap()
}

/// A directory of its own with one fake texture in it, for the import
/// settings round trip below. The bytes are not a PNG on purpose: nothing
/// here decodes them, and the sidecar hashes whatever is there.
struct TextureProbe {
    dir: std::path::PathBuf,
    path: String,
}

impl TextureProbe {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "bsengine-editor-import-{tag}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("wall.png");
        std::fs::write(&file, b"not a png").unwrap();
        Self {
            path: file.to_string_lossy().to_string(),
            dir,
        }
    }
}

impl Drop for TextureProbe {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.dir).ok();
    }
}

/// A throwaway project with a manifest, so the references walk has an
/// entry scene to start from: `main.ron` names the model and the
/// script, the script names a second scene, and `unused.png` is named
/// by nothing.
struct ProjectProbe {
    dir: std::path::PathBuf,
}

impl ProjectProbe {
    fn new(tag: &str) -> Self {
        let dir =
            std::env::temp_dir().join(format!("bsengine-editor-refs-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let probe = Self { dir };
        probe.write(
            "project.toml",
            "[project]\nname = \"P\"\nentry_scene = \"assets/scenes/main.ron\"\n",
        );
        probe.write(
            "assets/scenes/main.ron",
            r#"(entities: [(name: "Hero", gltf: Some(Path("assets/models/hero.glb")), script: Some(Path("assets/scripts/hero.js")))])"#,
        );
        probe.write(
            "assets/scripts/hero.js",
            "const NEXT = \"assets/scenes/level2.ron\";",
        );
        probe.write(
            "assets/scenes/level2.ron",
            r#"(entities: [(name: "Hero", gltf: Some(Path("assets/models/hero.glb")))])"#,
        );
        probe.write("assets/models/hero.glb", "glb");
        probe.write("assets/textures/unused.png", "png");
        probe
    }

    fn write(&self, relative: &str, contents: &str) {
        let path = self.dir.join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, contents).unwrap();
    }
}

impl Drop for ProjectProbe {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.dir).ok();
    }
}

mod components;
mod entities;
mod hierarchy;
mod query;
mod scene;
mod selection;
mod systems;
mod tags;
mod transform;
mod visibility;
