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

pub fn build_test_app(project_dir: &str) -> App {
    let manifest_path = format!("{project_dir}/project.toml");
    let manifest_str = std::fs::read_to_string(&manifest_path)
        .unwrap_or_else(|e| panic!("Cannot read {manifest_path}: {e}"));
    let manifest: ProjectManifest = toml::from_str(&manifest_str)
        .unwrap_or_else(|e| panic!("Cannot parse {manifest_path}: {e}"));
    let scene_path = format!("{project_dir}/{}", manifest.project.entry_scene);

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
    let mut app = build_test_app(project_dir);
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
                write_response(&mut stdout, &CommandResponse::err(format!("parse error: {e}")));
                continue;
            }
        };

        match command {
            Command::Step { frames } => {
                for _ in 0..frames {
                    app.update();
                    frame += 1;
                }
                write_response(&mut stdout, &CommandResponse::ok(json!({"frame": frame})));
            }
            Command::PressKey { key } => match key_from_str(&key) {
                Some(code) => {
                    app.world_mut().resource_mut::<Input<KeyCode>>().press(code);
                    write_response(&mut stdout, &CommandResponse::ok(json!({})));
                }
                None => write_response(
                    &mut stdout,
                    &CommandResponse::err(format!("unknown key: {key}")),
                ),
            },
            Command::ReleaseKey { key } => match key_from_str(&key) {
                Some(code) => {
                    app.world_mut()
                        .resource_mut::<Input<KeyCode>>()
                        .release(code);
                    write_response(&mut stdout, &CommandResponse::ok(json!({})));
                }
                None => write_response(
                    &mut stdout,
                    &CommandResponse::err(format!("unknown key: {key}")),
                ),
            },
            Command::PressMouse { button } => match mouse_button_from_u8(button) {
                Some(b) => {
                    app.world_mut()
                        .resource_mut::<Input<MouseButton>>()
                        .press(b);
                    write_response(&mut stdout, &CommandResponse::ok(json!({})));
                }
                None => write_response(
                    &mut stdout,
                    &CommandResponse::err(format!("unknown mouse button: {button}")),
                ),
            },
            Command::ReleaseMouse { button } => match mouse_button_from_u8(button) {
                Some(b) => {
                    app.world_mut()
                        .resource_mut::<Input<MouseButton>>()
                        .release(b);
                    write_response(&mut stdout, &CommandResponse::ok(json!({})));
                }
                None => write_response(
                    &mut stdout,
                    &CommandResponse::err(format!("unknown mouse button: {button}")),
                ),
            },
            Command::Query { tool, args } => match run_query(app.world_mut(), &tool, &args) {
                Ok(result) => write_response(&mut stdout, &CommandResponse::ok(result)),
                Err(e) => write_response(&mut stdout, &CommandResponse::err(e)),
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
                        Ok(passed) => write_response(
                            &mut stdout,
                            &CommandResponse::ok(
                                json!({"passed": passed, "actual": actual, "label": label}),
                            ),
                        ),
                        Err(e) => write_response(&mut stdout, &CommandResponse::err(e)),
                    }
                }
                Err(e) => write_response(&mut stdout, &CommandResponse::err(e)),
            },
            Command::Shutdown => {
                write_response(&mut stdout, &CommandResponse::ok(json!({})));
                break;
            }
        }
    }
}

fn write_response(stdout: &mut io::Stdout, response: &CommandResponse) {
    if let Ok(s) = serde_json::to_string(response) {
        let _ = writeln!(stdout, "{s}");
        let _ = stdout.flush();
    }
}
