use std::collections::{HashMap, HashSet};

use bevy_app::{App, Plugin, Startup, Update};
use bevy_ecs::prelude::*;
use bsengine_core::Transform;
use bsengine_input::{Input, KeyCode};
use bsengine_scene::{Name, ScriptPath};
use glam::Vec3;

use crate::ops::{ScriptCommand, BOOTSTRAP_JS, COMMAND_BUFFER, KEY_SNAPSHOT, TRANSFORM_SNAPSHOT};
use crate::runtime::ScriptRuntime;

/// Root directory of the current project — used to resolve relative script paths.
#[derive(Resource, Default)]
pub struct ProjectDir(pub String);

/// Loaded JS source for a scripted entity.
#[derive(Component)]
pub struct Script {
    pub source: String,
}

// Not Send/Sync — stored as a non-send resource via insert_non_send_resource.
pub struct ScriptRuntimeResource(pub ScriptRuntime);

pub struct ScriptingPlugin {
    pub project_dir: String,
}

impl Default for ScriptingPlugin {
    fn default() -> Self {
        Self {
            project_dir: ".".to_string(),
        }
    }
}

impl Plugin for ScriptingPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ProjectDir(self.project_dir.clone()));
        app.insert_non_send_resource(ScriptRuntimeResource(ScriptRuntime::new_with_ops()));
        app.add_systems(Startup, load_scripts);
        app.add_systems(Update, run_scripts);
    }
}

fn load_scripts(world: &mut World) {
    let project_dir = world
        .get_resource::<ProjectDir>()
        .map(|pd| pd.0.clone())
        .unwrap_or_default();

    let scripts: Vec<(Entity, String)> = {
        let mut q = world.query::<(Entity, &ScriptPath)>();
        q.iter(world)
            .map(|(e, sp)| {
                let path = if project_dir.is_empty() {
                    sp.0.clone()
                } else {
                    format!("{}/{}", project_dir, sp.0)
                };
                (e, path)
            })
            .collect()
    };

    if scripts.is_empty() {
        return;
    }

    if let Some(mut rt) = world.get_non_send_resource_mut::<ScriptRuntimeResource>() {
        if let Err(e) = rt.0.exec_source(BOOTSTRAP_JS, "<bootstrap>") {
            tracing::error!("Failed to load bootstrap: {e}");
        }
    }

    for (entity, path) in scripts {
        match std::fs::read_to_string(&path) {
            Ok(source) => {
                world.entity_mut(entity).insert(Script {
                    source: source.clone(),
                });
                if let Some(mut rt) = world.get_non_send_resource_mut::<ScriptRuntimeResource>() {
                    if let Err(e) = rt.0.exec_source(&source, &path) {
                        tracing::error!("Script error in {path}: {e}");
                    }
                }
            }
            Err(e) => tracing::error!("Cannot read script {path}: {e}"),
        }
    }
}

fn run_scripts(world: &mut World) {
    {
        let mut q = world.query::<&Script>();
        if q.iter(world).next().is_none() {
            return;
        }
    }

    let transform_snapshot: HashMap<String, Vec3> = {
        let mut q = world.query::<(&Name, &Transform)>();
        q.iter(world)
            .map(|(n, t)| (n.0.clone(), t.translation))
            .collect()
    };

    let key_snapshot: HashSet<String> = {
        let mappings = [
            (KeyCode::W, "W"),
            (KeyCode::A, "A"),
            (KeyCode::S, "S"),
            (KeyCode::D, "D"),
            (KeyCode::Space, "Space"),
            (KeyCode::Enter, "Enter"),
            (KeyCode::Escape, "Escape"),
            (KeyCode::Up, "Up"),
            (KeyCode::Down, "Down"),
            (KeyCode::Left, "Left"),
            (KeyCode::Right, "Right"),
        ];
        if let Some(input) = world.get_resource::<Input<KeyCode>>() {
            mappings
                .iter()
                .filter(|(code, _)| input.is_pressed(code))
                .map(|(_, name)| name.to_string())
                .collect()
        } else {
            HashSet::new()
        }
    };

    TRANSFORM_SNAPSHOT.with(|s| *s.borrow_mut() = transform_snapshot);
    KEY_SNAPSHOT.with(|k| *k.borrow_mut() = key_snapshot);
    COMMAND_BUFFER.with(|c| c.borrow_mut().clear());

    if let Some(mut rt) = world.get_non_send_resource_mut::<ScriptRuntimeResource>() {
        if let Err(e) = rt.0.call_fn("onUpdate") {
            tracing::error!("onUpdate error: {e}");
        }
    }

    let commands: Vec<ScriptCommand> = COMMAND_BUFFER.with(|c| c.borrow().clone());
    for cmd in commands {
        match cmd {
            ScriptCommand::SetTransform { name, x, y, z } => {
                let mut q = world.query::<(&Name, &mut Transform)>();
                for (n, mut t) in q.iter_mut(world) {
                    if n.0 == name {
                        t.translation = Vec3::new(x, y, z);
                        break;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ScriptRuntimeResource, ScriptingPlugin};
    use bsengine_app::new_app;

    #[test]
    fn scripting_plugin_registers_runtime() {
        let mut app = new_app();
        app.add_plugins(ScriptingPlugin::default());
        assert!(app
            .world()
            .get_non_send_resource::<ScriptRuntimeResource>()
            .is_some());
    }

    #[test]
    fn scripting_plugin_runtime_can_eval() {
        let mut app = new_app();
        app.add_plugins(ScriptingPlugin::default());

        let result = app
            .world_mut()
            .get_non_send_resource_mut::<ScriptRuntimeResource>()
            .expect("ScriptRuntimeResource not found")
            .0
            .eval("40 + 2");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "42");
    }
}
