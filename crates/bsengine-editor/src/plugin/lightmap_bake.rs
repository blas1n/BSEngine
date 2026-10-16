//! The editor's Bake Lightmaps button.
//!
//! The bake itself is `bsengine-runtime --bake` (see that crate's `bake`): it
//! loads the scene with the runtime's whole plugin stack, which this crate does
//! not link, so the editor runs it as a child -- as `bsengine-mcp`'s
//! `game_bake_lightmaps` does -- on a thread of its own, and the editor keeps
//! drawing while it runs. When it finishes, the scene in the editor is given
//! the new lightmaps (`bsengine_scene::lightmaps::apply`), so the viewport shows
//! the bake without reloading the scene and losing unsaved edits.

use bevy_ecs::prelude::{Entity, Resource, World};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::thread::JoinHandle;

/// Where `bsengine-runtime` is: `BSENGINE_RUNTIME_BIN` (the variable
/// `bsengine-mcp`'s server reads), else beside the editor's own executable --
/// both are built into the same target directory -- else on `PATH`.
#[derive(Resource, Clone, Debug)]
pub struct LightmapBakerBinary(pub PathBuf);

impl Default for LightmapBakerBinary {
    fn default() -> Self {
        if let Ok(path) = std::env::var("BSENGINE_RUNTIME_BIN") {
            return Self(PathBuf::from(path));
        }
        let name = format!("bsengine-runtime{}", std::env::consts::EXE_SUFFIX);
        let beside = std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(|dir| dir.join(&name)));
        Self(
            beside
                .filter(|p| p.is_file())
                .unwrap_or_else(|| PathBuf::from(name)),
        )
    }
}

/// The bake in progress, if any: the scene it is for, and the thread running
/// the child, which returns its JSON report or the reason it failed.
#[derive(Resource, Default)]
pub(crate) struct LightmapBakeJob(Mutex<Option<Running>>);

struct Running {
    scene_path: String,
    handle: JoinHandle<Result<serde_json::Value, String>>,
}

/// The project a scene file belongs to -- the nearest directory above it with
/// a `project.toml` -- and the scene's path relative to it, which is how
/// `--bake --scene` names it.
fn project_of(scene_path: &str) -> Option<(PathBuf, String)> {
    let scene = Path::new(scene_path);
    let mut dir = scene.parent();
    while let Some(d) = dir {
        if d.join("project.toml").is_file() {
            let relative = scene.strip_prefix(d).ok()?;
            return Some((
                d.to_path_buf(),
                relative.to_string_lossy().replace('\\', "/"),
            ));
        }
        dir = d.parent();
    }
    None
}

/// Starts a bake of `scene_path`, unless one is already running.
pub(crate) fn start(world: &mut World, scene_path: String) {
    let job = world.get_resource_or_insert_with(LightmapBakeJob::default);
    if job.0.lock().unwrap().is_some() {
        return;
    }
    let Some((project, relative)) = project_of(&scene_path) else {
        set_status(
            world,
            false,
            format!("Bake failed: {scene_path} is not inside a project (no project.toml above it)"),
        );
        return;
    };
    let binary = world
        .get_resource_or_insert_with(LightmapBakerBinary::default)
        .0
        .clone();
    let handle = std::thread::spawn(move || {
        let output = std::process::Command::new(&binary)
            .arg("--bake")
            .arg(&project)
            .arg("--scene")
            .arg(&relative)
            .arg("--json")
            .output()
            .map_err(|e| format!("cannot run {}: {e}", binary.display()))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let last = stdout
            .lines()
            .rev()
            .find(|l| !l.trim().is_empty())
            .unwrap_or("");
        let report: serde_json::Value = serde_json::from_str(last.trim()).map_err(|_| {
            let stderr = String::from_utf8_lossy(&output.stderr);
            format!(
                "the bake printed no report: {}",
                stderr.lines().last().unwrap_or("no output")
            )
        })?;
        if output.status.success() {
            Ok(report)
        } else {
            Err(report["error"]
                .as_str()
                .unwrap_or("unknown error")
                .to_string())
        }
    });
    *world.resource::<LightmapBakeJob>().0.lock().unwrap() = Some(Running { scene_path, handle });
    set_status(world, true, "Baking lightmaps...".to_string());
}

fn set_status(world: &mut World, running: bool, status: String) {
    if let Some(mut insp) = world.get_resource_mut::<bsengine_core::InspectorState>() {
        insp.lightmap_bake_running = running;
        insp.lightmap_bake_status = Some(status);
    }
}

/// Collects a finished bake: reports it in the Inspector's status, and gives
/// the scene in the editor its new lightmaps. Every `Lightmap` is cleared
/// first, so an entity the new bake no longer covers stops showing the old.
pub(crate) fn poll(world: &mut World) {
    let finished = {
        let Some(job) = world.get_resource::<LightmapBakeJob>() else {
            return;
        };
        let mut slot = job.0.lock().unwrap();
        if !slot.as_ref().is_some_and(|r| r.handle.is_finished()) {
            return;
        }
        slot.take().expect("checked above")
    };
    let result = finished
        .handle
        .join()
        .unwrap_or_else(|_| Err("the bake thread panicked".to_string()));
    match result {
        Ok(report) => {
            let lit: Vec<Entity> = world
                .query_filtered::<Entity, bevy_ecs::query::With<bsengine_core::Lightmap>>()
                .iter(world)
                .collect();
            for e in lit {
                world.entity_mut(e).remove::<bsengine_core::Lightmap>();
            }
            let applied = std::fs::read_to_string(&finished.scene_path)
                .map(|text| bsengine_scene::lightmaps::apply(world, &finished.scene_path, &text))
                .unwrap_or(0);
            set_status(
                world,
                false,
                format!(
                    "Baked {} renderer(s) into {} layer(s); {applied} lit in this scene",
                    report["renderers"], report["layers"]
                ),
            );
        }
        Err(e) => {
            tracing::warn!("lightmap bake failed: {e}");
            set_status(world, false, format!("Bake failed: {e}"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_scene_is_found_relative_to_the_project_above_it() {
        let dir =
            std::env::temp_dir().join(format!("bsengine_bake_project_{}", std::process::id()));
        std::fs::create_dir_all(dir.join("assets/scenes")).unwrap();
        std::fs::write(dir.join("project.toml"), "").unwrap();
        let scene = dir.join("assets/scenes/main.ron");
        let (root, relative) = project_of(&scene.to_string_lossy()).expect("inside a project");
        assert_eq!(root, dir);
        assert_eq!(relative, "assets/scenes/main.ron");
        assert!(project_of(&std::env::temp_dir().join("x.ron").to_string_lossy()).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
