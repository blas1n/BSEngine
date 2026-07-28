//! `bsengine-runtime --test <game>`: runs a game headless (no window, no
//! renderer) and drives it via newline-delimited JSON commands on stdin,
//! writing one JSON response per command to stdout. See
//! `docs/superpowers/specs/2026-07-22-ai-gameplay-e2e-testing-design.md`.

use std::io::{self, BufRead, Write};

use bevy_app::App;
use bsengine_audio::AudioPlugin;
use bsengine_input::{Input, InputPlugin, KeyCode, MouseButton};
use bsengine_physics::PhysicsPlugin;
use bsengine_scene::ScenePlugin;
use bsengine_scripting::{ScriptingPlugin, KEY_MAPPINGS};
use serde_json::{json, Value};

use crate::scene_systems::{register_scene_systems, ProjectManifest};
use crate::test_protocol::{Command, CommandResponse};
use crate::test_query::{eval_op, eval_path, run_query};

/// Builds the headless app rooted at `project_dir`. Loads `scene_override`
/// (a path relative to `project_dir`, e.g. `"assets/scenes/level3.ron"`) if
/// given, otherwise falls back to `project.toml`'s `entry_scene` — lets a
/// replay log pin its own starting scene instead of always depending on
/// whatever the project's entry scene currently is (which changes as a
/// multi-level game's "real" entry point evolves during development).
pub fn build_test_app(project_dir: &str, scene_override: Option<&str>) -> App {
    let manifest_path = format!("{project_dir}/project.toml");
    let manifest_str = std::fs::read_to_string(&manifest_path)
        .unwrap_or_else(|e| panic!("Cannot read {manifest_path}: {e}"));
    let manifest: ProjectManifest = toml::from_str(&manifest_str)
        .unwrap_or_else(|e| panic!("Cannot parse {manifest_path}: {e}"));
    let relative_scene = scene_override.unwrap_or(&manifest.project.entry_scene);
    let scene_path = format!("{project_dir}/{relative_scene}");

    let mut app = bsengine_app::new_app();
    app.add_plugins(InputPlugin)
        .add_plugins(AudioPlugin)
        .add_plugins(PhysicsPlugin)
        .add_plugins(ScenePlugin::from_file(&scene_path))
        .add_plugins(ScriptingPlugin {
            project_dir: project_dir.to_string(),
        });
    register_scene_systems(&mut app);
    app
}

fn key_from_str(key: &str) -> Option<KeyCode> {
    KEY_MAPPINGS
        .iter()
        .find(|(_, name)| *name == key)
        .map(|(code, _)| *code)
}

fn mouse_button_from_u8(button: u8) -> Option<MouseButton> {
    match button {
        0 => Some(MouseButton::Left),
        1 => Some(MouseButton::Right),
        2 => Some(MouseButton::Middle),
        _ => None,
    }
}

pub fn run_test_mode(project_dir: &str) {
    let mut app = build_test_app(project_dir, None);
    let mut frame: u64 = 0;

    let stdin = io::stdin();
    let mut stdout = io::stdout();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        if line.trim().is_empty() {
            continue;
        }

        let command: Command = match serde_json::from_str(&line) {
            Ok(c) => c,
            Err(e) => {
                write_response(
                    &mut stdout,
                    &CommandResponse::err(format!("parse error: {e}")),
                );
                continue;
            }
        };

        let (response, should_stop) = execute_command(&mut app, &mut frame, command);
        write_response(&mut stdout, &response);
        if should_stop {
            break;
        }
    }
}

/// Runs one protocol command against `app`, returning its response and
/// whether the caller should stop the loop (true only for `Shutdown`).
/// Shared by the interactive stdin loop above and a replay loop added in a
/// later task — both must execute commands identically for replay fidelity.
pub fn execute_command(
    app: &mut App,
    frame: &mut u64,
    command: Command,
) -> (CommandResponse, bool) {
    match command {
        Command::Step { frames } => {
            for _ in 0..frames {
                app.update();
                *frame += 1;
            }
            (CommandResponse::ok(json!({"frame": *frame})), false)
        }
        Command::PressKey { key } => match key_from_str(&key) {
            Some(code) => {
                app.world_mut().resource_mut::<Input<KeyCode>>().press(code);
                (CommandResponse::ok(json!({})), false)
            }
            None => (CommandResponse::err(format!("unknown key: {key}")), false),
        },
        Command::ReleaseKey { key } => match key_from_str(&key) {
            Some(code) => {
                app.world_mut()
                    .resource_mut::<Input<KeyCode>>()
                    .release(code);
                (CommandResponse::ok(json!({})), false)
            }
            None => (CommandResponse::err(format!("unknown key: {key}")), false),
        },
        Command::PressMouse { button } => match mouse_button_from_u8(button) {
            Some(b) => {
                app.world_mut()
                    .resource_mut::<Input<MouseButton>>()
                    .press(b);
                (CommandResponse::ok(json!({})), false)
            }
            None => (
                CommandResponse::err(format!("unknown mouse button: {button}")),
                false,
            ),
        },
        Command::ReleaseMouse { button } => match mouse_button_from_u8(button) {
            Some(b) => {
                app.world_mut()
                    .resource_mut::<Input<MouseButton>>()
                    .release(b);
                (CommandResponse::ok(json!({})), false)
            }
            None => (
                CommandResponse::err(format!("unknown mouse button: {button}")),
                false,
            ),
        },
        Command::Query { tool, args } => match run_query(app.world_mut(), &tool, &args) {
            Ok(result) => (CommandResponse::ok(result), false),
            Err(e) => (CommandResponse::err(e), false),
        },
        Command::Assert {
            query,
            path,
            op,
            value,
            label,
        } => match run_query(app.world_mut(), &query.tool, &query.args) {
            Ok(result) => {
                let actual = eval_path(&result, &path).cloned().unwrap_or(Value::Null);
                match eval_op(&actual, &op, &value) {
                    Ok(passed) => (
                        CommandResponse::ok(
                            json!({"passed": passed, "actual": actual, "label": label}),
                        ),
                        false,
                    ),
                    Err(e) => (CommandResponse::err(e), false),
                }
            }
            Err(e) => (CommandResponse::err(e), false),
        },
        Command::Shutdown => (CommandResponse::ok(json!({})), true),
    }
}

fn write_response(stdout: &mut io::Stdout, response: &CommandResponse) {
    if let Ok(s) = serde_json::to_string(response) {
        let _ = writeln!(stdout, "{s}");
        let _ = stdout.flush();
    }
}

#[derive(serde::Deserialize)]
struct RecordedLog {
    /// Path (relative to `project_dir`) of the scene this log was recorded
    /// against, e.g. `"assets/scenes/level3.ron"`. When present, replay
    /// loads this scene directly instead of the project's current
    /// `entry_scene` — needed once a game has more than one independently
    /// replayable level, since only one `entry_scene` can be active at a
    /// time. Absent for older logs recorded before this field existed,
    /// which fall back to `entry_scene` as before.
    #[serde(default)]
    scene: Option<String>,
    actions: Vec<Command>,
}

/// Runs a saved action log to completion with no stdin/AI involvement.
/// Returns `true` if every command succeeded and every `Assert` passed;
/// on the first failure, prints details to stderr and returns `false`.
pub fn run_replay_mode(project_dir: &str, log_path: &str) -> bool {
    let log_str = std::fs::read_to_string(log_path)
        .unwrap_or_else(|e| panic!("cannot read replay log {log_path}: {e}"));
    let log: RecordedLog = serde_json::from_str(&log_str)
        .unwrap_or_else(|e| panic!("cannot parse replay log {log_path}: {e}"));

    let mut app = build_test_app(project_dir, log.scene.as_deref());
    let mut frame: u64 = 0;

    for command in log.actions {
        let is_assert = matches!(command, Command::Assert { .. });
        let (response, _) = execute_command(&mut app, &mut frame, command);

        if !response.ok {
            eprintln!(
                "REPLAY FAILED: {}",
                response
                    .error
                    .unwrap_or_else(|| "unknown error".to_string())
            );
            return false;
        }

        if is_assert {
            let passed = response
                .data
                .as_ref()
                .and_then(|d| d.get("passed"))
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            if !passed {
                let label = response
                    .data
                    .as_ref()
                    .and_then(|d| d.get("label"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("(unlabeled assertion)");
                let actual = response
                    .data
                    .as_ref()
                    .and_then(|d| d.get("actual"))
                    .cloned()
                    .unwrap_or(Value::Null);
                eprintln!("REPLAY FAILED: {label} — actual: {actual}");
                return false;
            }
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_two_scene_project() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("assets/scenes")).unwrap();
        std::fs::write(
            root.join("project.toml"),
            "[project]\nname = \"Test\"\nentry_scene = \"assets/scenes/a.ron\"\n",
        )
        .unwrap();
        std::fs::write(
            root.join("assets/scenes/a.ron"),
            "SceneDescriptor(entities: [EntityDescriptor(name: \"SceneA\")])",
        )
        .unwrap();
        std::fs::write(
            root.join("assets/scenes/b.ron"),
            "SceneDescriptor(entities: [EntityDescriptor(name: \"SceneB\")])",
        )
        .unwrap();
        dir
    }

    #[test]
    fn build_test_app_with_no_override_loads_entry_scene() {
        let dir = write_two_scene_project();
        let mut app = build_test_app(dir.path().to_str().unwrap(), None);
        app.update();

        let names = crate::test_query::get_entity_names(app.world_mut());
        let names: Vec<String> = serde_json::from_value(names).unwrap();
        assert!(names.contains(&"SceneA".to_string()), "names: {names:?}");
        assert!(!names.contains(&"SceneB".to_string()), "names: {names:?}");
    }

    #[test]
    fn build_test_app_with_override_loads_that_scene_instead() {
        let dir = write_two_scene_project();
        let mut app = build_test_app(dir.path().to_str().unwrap(), Some("assets/scenes/b.ron"));
        app.update();

        let names = crate::test_query::get_entity_names(app.world_mut());
        let names: Vec<String> = serde_json::from_value(names).unwrap();
        assert!(names.contains(&"SceneB".to_string()), "names: {names:?}");
        assert!(!names.contains(&"SceneA".to_string()), "names: {names:?}");
    }
}
