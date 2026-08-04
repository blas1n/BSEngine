//! `bsengine-runtime --test <game>`: runs a game headless (no window, no
//! renderer) and drives it via newline-delimited JSON commands on stdin,
//! writing one JSON response per command to stdout. See
//! `docs/superpowers/specs/2026-07-22-ai-gameplay-e2e-testing-design.md`.

use std::io::{self, BufRead, Write};

use bevy_app::App;
use bevy_ecs::event::Events;
use bsengine_app::{NavMeshPlugin, TimePlugin};
use bsengine_asset::{AssetIdentityPlugin, AssetPlugin, AssetStatusPlugin};
use bsengine_audio::AudioPlugin;
use bsengine_core::{EditorPlayState, InspectorState};
use bsengine_input::{ElementState, InputPlugin, KeyCode, KeyInput, MouseButton, MouseInput};
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
    app.add_plugins(TimePlugin)
        .add_plugins(AssetPlugin)
        // Included here, unlike `AssetWatcherPlugin` (see main.rs's
        // run_windowed for why that one is windowed-only). The two reasons
        // the watcher is excluded are that it starts a background thread and
        // that it introduces frame-to-frame variation in the one mode that
        // pins its clocks to stay reproducible. Neither applies to this one:
        // it starts nothing, and it only *reads* `bevy_asset`'s own per-frame
        // state into a resource — it drives no entity, no clock and no
        // physics, so a replay behaves identically with or without it.
        //
        // The positive reason is stronger than the absence of a cost.
        // "Did this asset actually load?" is exactly the question a headless
        // E2E recording should be able to assert on, and it is the question
        // that went unanswered while `games/mini-arena` ran with no mesh and
        // no shader across two phases of work. Leaving it out would make
        // `Bsengine.getAssetStatus` answer `unknown` for every path in the
        // mode most likely to be automating that check — the same
        // registered-nowhere failure this plugin's absence caused before.
        //
        // What it records here today is narrower than in the windowed
        // runtime, and worth knowing before someone concludes it is broken:
        // this app has no `RenderPlugin`, `GltfPlugin` or `SkinnedMeshPlugin`,
        // so nothing ever requests a mesh, a shader or a texture. Replaying
        // `games/mini-arena` records zero paths, while the same game windowed
        // records its `fox.glb` and `glow.wgsl` as `Loaded`. Sounds *are*
        // requested here (`AudioPlugin` and `playSound` are both present), so
        // those are tracked. The distinction matters: an empty map means
        // "genuinely nothing was requested", which is a true answer, whereas
        // a missing resource would have meant "the engine cannot say" while
        // sounding identical to a script.
        .add_plugins(AssetStatusPlugin)
        // Here for a blunter reason than the status plugin above: this app
        // adds `ScenePlugin`, and a scene is where an asset reference lives.
        // Leave it out and a replay resolves every reference by stored path
        // while the windowed runtime resolves the same scene by identity —
        // two hosts loading different files from one scene file, in the mode
        // whose whole job is to reproduce what the game does. The E2E
        // recordings are the one place a rename that broke a game would be
        // caught automatically, and they can only catch it if they run the
        // resolution the game runs.
        //
        // Unlike the render-stack asymmetry described for the status plugin,
        // there is none here: resolution is a pair of map lookups performed by
        // `spawn_scene_entities`, which this app runs in full.
        //
        // Costs one walk of `<project_dir>/assets` at Startup, no thread and
        // no per-frame work, so a replay stays as reproducible as it was.
        .add_plugins(AssetIdentityPlugin)
        .add_plugins(InputPlugin)
        .add_plugins(AudioPlugin)
        .add_plugins(PhysicsPlugin)
        .add_plugins(NavMeshPlugin)
        .add_plugins(ScenePlugin::from_file(&scene_path))
        .add_plugins(ScriptingPlugin {
            project_dir: project_dir.to_string(),
        });

    // Replays must be reproducible, so script time advances a fixed step per
    // frame here instead of reading the wall clock. The step matches Rapier's
    // (`IntegrationParameters::default().dt`, 1/60), which is the whole point:
    // physics already advances exactly 1/60 per frame regardless of how long
    // the frame took, so any script reading `getTime()` was on a different
    // clock -- headless frames take well under a millisecond, so a
    // `getTime()`-driven obstacle crawled while a physics-driven ball raced,
    // by a ratio that changed with machine speed. That is what made
    // tilt-run's level-5 recordings pass locally and fail on CI.
    // Both clocks, not just one: `Time` drives nav-mesh agents, animation,
    // tweens and timers, while `ScriptTimingState` drives getTime()/
    // getDeltaTime(). Fixing only one leaves them disagreeing with each other
    // instead of with physics, which is worse -- it made mini-arena's player
    // (script-driven) outrun its Enemy (nav-mesh-driven) and whiff every
    // attack.
    const FIXED_DT: f32 = 1.0 / 60.0;
    app.insert_resource(bsengine_core::Time::fixed(FIXED_DT));
    app.insert_resource(bsengine_scripting::ScriptTimingState::fixed(FIXED_DT));

    register_scene_systems(&mut app);

    // Same reflect-type registrations EditorPlugin does (again, not the full
    // plugin -- see below), needed so `spawn_scene_entities` can actually
    // deserialize a scene's `components:` entries (Shield, SaveData,
    // AnimationStateMachine, NavMeshAgent, Bloom, ToneMap, ...) instead of
    // silently dropping every one of them (logged only as a `tracing::warn!`
    // "unknown reflected type path"). Without this, e.g. a Shield-gated dead
    // check reads a permanently-0 shield in headless mode and the entity
    // never behaves as scripted, even though the identical scene plays
    // correctly in the windowed runtime.
    bsengine_scene::register_gameplay_reflect_types(&mut app);

    // The windowed runtime (main.rs's run_windowed) always runs with
    // EditorPlugin, which gates script execution behind `editor_mode &&
    // play_state == Stopped` (see bsengine-scripting's run_scripts) unless
    // something forces play_state to Playing — which run_windowed does,
    // since "run a game" should play it, not silently boot into a stopped
    // editor. Mirror that same InspectorState (not the full EditorPlugin,
    // which requires the render/window stack this headless app doesn't
    // have) here so headless tests exercise the same gate production does
    // — otherwise a regression here (e.g. someone removing run_windowed's
    // override) would pass every headless test while being unplayable in
    // the real windowed runtime, exactly as happened before this comment
    // was written.
    let mut inspector_state = InspectorState::editor();
    inspector_state.play_state = EditorPlayState::Playing;
    inspector_state.current_scene_path = Some(scene_path.clone());
    app.insert_resource(inspector_state);

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
        // PressKey/ReleaseKey/PressMouse/ReleaseMouse send through the same
        // `Events<KeyInput>`/`Events<MouseInput>` queue the real windowed
        // runtime's window-event handler uses, rather than mutating
        // `Input<T>` directly. That distinction matters: `Input<T>`'s
        // `just_pressed`/`just_released` ("edge") state is cleared every
        // frame by `clear_input_state`, which runs first in `PreUpdate`
        // ahead of the event-draining systems, precisely so that ordering
        // (clear old edge state, then set new edge state from this frame's
        // events) gives scripts a one-frame-wide, correctly-timed window to
        // observe `isKeyDown`/`isKeyUp`. A direct `.press()`/`.release()`
        // call happens outside any schedule, strictly *before* the next
        // `Step`'s first `app.update()` -- so that same `clear_input_state`
        // wipes the edge flag before `run_scripts` (in `Update`, which runs
        // after `PreUpdate`) ever sees it. The held/level `is_pressed` state
        // isn't cleared this way, so continuous-movement scripts using
        // `isKeyPressed` never noticed; only edge-triggered `isKeyDown`/
        // `isKeyUp` checks (attack, pause toggle, checkpoint reload, ...)
        // were silently untestable through this protocol until this fix.
        Command::PressKey { key } => match key_from_str(&key) {
            Some(code) => {
                app.world_mut()
                    .resource_mut::<Events<KeyInput>>()
                    .send(KeyInput {
                        key_code: code,
                        state: ElementState::Pressed,
                        text: None,
                    });
                (CommandResponse::ok(json!({})), false)
            }
            None => (CommandResponse::err(format!("unknown key: {key}")), false),
        },
        Command::ReleaseKey { key } => match key_from_str(&key) {
            Some(code) => {
                app.world_mut()
                    .resource_mut::<Events<KeyInput>>()
                    .send(KeyInput {
                        key_code: code,
                        state: ElementState::Released,
                        text: None,
                    });
                (CommandResponse::ok(json!({})), false)
            }
            None => (CommandResponse::err(format!("unknown key: {key}")), false),
        },
        Command::PressMouse { button } => match mouse_button_from_u8(button) {
            Some(b) => {
                app.world_mut()
                    .resource_mut::<Events<MouseInput>>()
                    .send(MouseInput {
                        button: b,
                        state: ElementState::Pressed,
                    });
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
                    .resource_mut::<Events<MouseInput>>()
                    .send(MouseInput {
                        button: b,
                        state: ElementState::Released,
                    });
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
        // Evaluate first, then step -- so a predicate that already holds
        // costs zero frames and never pads the recording's timeline.
        Command::WaitUntil {
            query,
            path,
            op,
            value,
            max_frames,
            label,
        } => {
            let mut waited: u32 = 0;
            loop {
                // A failed query (unknown tool, malformed args) can never
                // start succeeding by waiting -- report it immediately.
                let result = match run_query(app.world_mut(), &query.tool, &query.args) {
                    Ok(result) => result,
                    // Named, for the same reason the budget-exhausted error
                    // below is: a recording can hold many waits, and a bare
                    // "unknown query tool" says nothing about which one.
                    Err(e) => {
                        return (CommandResponse::err(format!("{label}: {e}")), false);
                    }
                };
                let actual = eval_path(&result, &path).cloned().unwrap_or(Value::Null);
                match eval_op(&actual, &op, &value) {
                    Ok(true) => {
                        return (
                            CommandResponse::ok(json!({
                                "passed": true,
                                "actual": actual,
                                "label": label,
                                "waited_frames": waited,
                            })),
                            false,
                        );
                    }
                    Ok(false) => {}
                    // Not yet evaluable is the normal opening state of a
                    // wait, not a protocol error: before the scene's
                    // Startup systems have run, the entity a query names
                    // does not exist and its value is null, which no
                    // numeric comparison accepts. Erroring out here would
                    // make wait_until unusable for the case it exists for.
                    // Keep stepping; if the predicate is still not
                    // evaluable once the frame budget is gone (a genuinely
                    // bad op or path, or an entity destroyed mid-wait), the
                    // error surfaces then -- carrying the label and frame
                    // count, without which a replay failure reads as a bare
                    // type error with no clue which wait produced it.
                    Err(e) => {
                        if waited >= max_frames {
                            return (
                                CommandResponse::err(format!(
                                    "{label}: predicate never became evaluable after {waited} frames: {e}"
                                )),
                                false,
                            );
                        }
                    }
                }

                // `actual` is this iteration's observation, and evaluation
                // always precedes the timeout check, so it is also the last
                // value observed before giving up.
                if waited >= max_frames {
                    return (
                        CommandResponse::ok(json!({
                            "passed": false,
                            "actual": actual,
                            "label": label,
                            "waited_frames": waited,
                        })),
                        false,
                    );
                }

                app.update();
                *frame += 1;
                waited += 1;
            }
        }
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
        // wait_until reports pass/fail in exactly the same shape as assert
        // (a timeout is passed:false, not a protocol error), so it needs the
        // same result check — otherwise a timed-out wait would replay as a
        // silent success.
        let is_assert = matches!(command, Command::Assert { .. } | Command::WaitUntil { .. });
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
                let waited = response
                    .data
                    .as_ref()
                    .and_then(|d| d.get("waited_frames"))
                    .and_then(|v| v.as_u64());
                match waited {
                    Some(frames) => eprintln!(
                        "REPLAY FAILED: {label} — timed out after {frames} frames, last actual: {actual}"
                    ),
                    None => eprintln!("REPLAY FAILED: {label} — actual: {actual}"),
                }
                return false;
            }
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use bsengine_input::Input;

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

    // Roadmap item 30. This app builds its own plugin list, so the windowed
    // runtime registering `AssetIdentityPlugin` says nothing about whether
    // this one does; dropping it here would make a replay resolve references
    // by stored path while the game it reproduces resolved them by identity.
    //
    // Asserted end to end — a real project on disk, a real sidecar, the real
    // `build_test_app` — because both halves can fail independently and
    // neither leaves a trace: an unregistered plugin and a plugin that
    // published its index after the spawn had already looked for it produce
    // the same passing, silent, wrong answer.
    #[test]
    fn a_replay_app_resolves_a_scene_reference_by_identity() {
        const CURRENT: &str = "assets/models/fox.glb";
        const STALE: &str = "assets/models/vulpes.glb";

        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("assets/models")).unwrap();
        std::fs::create_dir_all(root.join("assets/scenes")).unwrap();
        std::fs::write(root.join(CURRENT), b"fake glb").unwrap();
        // Mint the sidecar first, the way a project that has been opened once
        // already carries it, and take the identity from it: the app has to
        // arrive at the same one by reading the same `.meta`.
        let guid = bsengine_asset::identity::scan(root)
            .expect("scan the probe project")
            .guid_for_path(CURRENT)
            .expect("the scan must identify the probe asset");
        std::fs::write(
            root.join("project.toml"),
            "[project]\nname = \"Test\"\nentry_scene = \"assets/scenes/a.ron\"\n",
        )
        .unwrap();
        std::fs::write(
            root.join("assets/scenes/a.ron"),
            format!(
                r#"SceneDescriptor(entities: [EntityDescriptor(name: "Fox", gltf: Some((guid: "{guid}", path: "{STALE}")))])"#
            ),
        )
        .unwrap();

        let project_dir = root.to_str().unwrap().to_string();
        let mut app = build_test_app(&project_dir, None);
        app.update();

        let mut q = app.world_mut().query::<&bsengine_gltf::GltfAsset>();
        let resolved: Vec<String> = q.iter(app.world()).map(|g| g.path.clone()).collect();
        assert_eq!(
            resolved,
            vec![format!("{project_dir}/{CURRENT}")],
            "the replay app loaded the path the scene stores instead of the \
             asset its identity names — either this app never publishes an \
             index, or it publishes one too late for the spawn to see"
        );
    }

    // Regression test for the "Play resets the scene" crash: EditorPlugin
    // pushing InspectorCmd::ReloadScene used to make handle_scene_load
    // construct a brand-new V8 isolate mid-session, which corrupted V8's
    // isolate state whenever EditorPlugin's stack was also active — the
    // game crashed with "Cannot create a handle without a HandleScope" the
    // next time a script ran. The fix (var Bsengine in BOOTSTRAP_JS, no
    // isolate recreation in handle_scene_load) means this combination is
    // now safe to exercise headlessly, unlike before that fix. Uses
    // cube-evader (not the synthetic two-scene fixture above) because it
    // has a real Player script — a reload with no scripted entities never
    // touches BOOTSTRAP_JS/V8 at all and wouldn't exercise the bug.
    #[test]
    fn editor_plugin_reload_scene_does_not_corrupt_scripting() {
        let project_dir = format!("{}/../../games/cube-evader", env!("CARGO_MANIFEST_DIR"));
        let mut app = build_test_app(&project_dir, None);
        let scene_path = app
            .world()
            .resource::<InspectorState>()
            .current_scene_path
            .clone()
            .expect("build_test_app should set current_scene_path");
        app.add_plugins(bsengine_editor::EditorPlugin);
        {
            let mut inspector = app.world_mut().resource_mut::<InspectorState>();
            inspector.play_state = EditorPlayState::Playing;
            inspector.current_scene_path = Some(scene_path);
        }

        app.world_mut()
            .resource_mut::<Input<KeyCode>>()
            .press(KeyCode::W);
        for _ in 0..20 {
            app.update();
        }

        app.world_mut()
            .resource_mut::<Input<KeyCode>>()
            .release(KeyCode::W);
        app.world_mut()
            .resource_mut::<InspectorState>()
            .cmd_queue
            .push(bsengine_core::InspectorCmd::ReloadScene);
        for _ in 0..3 {
            app.update();
        }

        let z = crate::test_query::get_transform(app.world_mut(), "Player")["z"]
            .as_f64()
            .expect("Player should exist with a transform after reload");
        assert!(
            z.abs() < 0.01,
            "Player should be back at its authored z=0.0 after reload, got {z}"
        );
    }

    // wait_until must cost zero frames when the predicate already holds --
    // otherwise every wait in a recording would pad the frame count and
    // slowly desynchronize everything after it.
    #[test]
    fn wait_until_returns_immediately_when_already_satisfied() {
        let project_dir = format!("{}/../../games/cube-evader", env!("CARGO_MANIFEST_DIR"));
        let mut app = build_test_app(&project_dir, None);
        let mut frame: u64 = 0;
        app.update();
        frame += 1;

        let (response, _) = execute_command(
            &mut app,
            &mut frame,
            Command::WaitUntil {
                query: crate::test_protocol::QuerySpec {
                    tool: "get_transform".to_string(),
                    args: json!({"name": "Player"}),
                },
                path: "x".to_string(),
                op: "exists".to_string(),
                value: Value::Null,
                max_frames: 100,
                label: "player exists".to_string(),
            },
        );

        assert!(response.ok, "response should be ok: {response:?}");
        let data = response.data.expect("wait_until returns data");
        assert_eq!(data["passed"], json!(true));
        assert_eq!(
            data["waited_frames"],
            json!(0),
            "already-true predicate must not step any frames"
        );
        assert_eq!(frame, 1, "frame counter must not advance");
    }

    // On timeout wait_until reports passed:false rather than erroring, so
    // run_replay_mode can render it with the same label/actual output it
    // already uses for a failed assert.
    #[test]
    fn wait_until_times_out_with_last_actual_value() {
        let project_dir = format!("{}/../../games/cube-evader", env!("CARGO_MANIFEST_DIR"));
        let mut app = build_test_app(&project_dir, None);
        let mut frame: u64 = 0;

        let (response, _) = execute_command(
            &mut app,
            &mut frame,
            Command::WaitUntil {
                query: crate::test_protocol::QuerySpec {
                    tool: "get_transform".to_string(),
                    args: json!({"name": "Player"}),
                },
                path: "x".to_string(),
                op: ">".to_string(),
                value: json!(1.0e9),
                max_frames: 3,
                label: "player reaches an impossible x".to_string(),
            },
        );

        assert!(response.ok, "timeout is not a protocol error: {response:?}");
        let data = response.data.expect("wait_until returns data");
        assert_eq!(data["passed"], json!(false));
        assert_eq!(data["waited_frames"], json!(3));
        assert_eq!(data["label"], json!("player reaches an impossible x"));
        assert!(
            data["actual"].is_number(),
            "timeout must report the last observed value, got {}",
            data["actual"]
        );
        assert_eq!(frame, 3, "frame counter must advance by the frames stepped");
    }

    // Pins the deviation this handler makes deliberately: an eval_op error
    // (here, a numeric comparison against a not-yet-spawned entity's null)
    // means "not satisfied yet", not "abort". Scene entities appear in a
    // Startup system, so before the first app.update() every query is null
    // -- if that errored out, wait_until could never be used to wait for
    // something to appear, which is its main purpose.
    #[test]
    fn wait_until_waits_through_a_not_yet_evaluable_predicate() {
        let project_dir = format!("{}/../../games/cube-evader", env!("CARGO_MANIFEST_DIR"));
        let mut app = build_test_app(&project_dir, None);
        let mut frame: u64 = 0;

        let (response, _) = execute_command(
            &mut app,
            &mut frame,
            Command::WaitUntil {
                query: crate::test_protocol::QuerySpec {
                    tool: "get_transform".to_string(),
                    args: json!({"name": "Player"}),
                },
                path: "x".to_string(),
                // Any real number satisfies this; the only way to reach it
                // is to get past the pre-Startup null that errors.
                op: ">".to_string(),
                value: json!(-1.0e9),
                max_frames: 50,
                label: "player spawns and has a numeric x".to_string(),
            },
        );

        assert!(
            response.ok,
            "should not abort on the pre-Startup null: {response:?}"
        );
        let data = response.data.expect("wait_until returns data");
        assert_eq!(data["passed"], json!(true));
        assert!(
            data["waited_frames"].as_u64().unwrap() > 0,
            "must have stepped at least one frame to get past the null, got {}",
            data["waited_frames"]
        );
    }

    // The other half of that deviation: waiting through a non-evaluable
    // predicate must not bury a genuinely broken one. A path that never
    // becomes numeric (the transform JSON has no "name" field, so it is
    // always null) has to surface as a hard error once the budget is gone,
    // not as a silent passed:false timeout that reads like the game simply
    // didn't get there in time.
    #[test]
    fn wait_until_errors_when_predicate_is_still_not_evaluable_at_timeout() {
        let project_dir = format!("{}/../../games/cube-evader", env!("CARGO_MANIFEST_DIR"));
        let mut app = build_test_app(&project_dir, None);
        let mut frame: u64 = 0;

        let (response, _) = execute_command(
            &mut app,
            &mut frame,
            Command::WaitUntil {
                query: crate::test_protocol::QuerySpec {
                    tool: "get_transform".to_string(),
                    args: json!({"name": "Player"}),
                },
                path: "name".to_string(),
                op: ">".to_string(),
                value: json!(1.0),
                max_frames: 2,
                label: "never-evaluable path".to_string(),
            },
        );

        assert!(
            !response.ok,
            "a permanently non-evaluable predicate must be an error, not a timeout: {response:?}"
        );
        assert!(
            response.data.is_none(),
            "an error response carries no data payload"
        );
        let error = response.error.expect("error response carries a message");
        assert!(
            error.contains("never-evaluable path"),
            "error must name which wait failed, got {error:?}"
        );
        assert!(
            error.contains('2'),
            "error must report how many frames were consumed, got {error:?}"
        );
        assert_eq!(frame, 2, "frames stepped before giving up must be counted");
    }

    // Regression test for the PressKey/ReleaseKey protocol commands: they
    // must route through Events<KeyInput>, not mutate Input<KeyCode>
    // directly, or edge-triggered checks (just_pressed/just_released --
    // what Bsengine.isKeyDown/isKeyUp read) are never observable through
    // this protocol (see execute_command's PressKey doc comment for why).
    // Exercises execute_command exactly as run_test_mode/run_replay_mode
    // do -- one command at a time, with an explicit Step between a key
    // event and the point where its effect is checked.
    #[test]
    fn press_key_is_observable_as_pressed_and_just_pressed_after_one_step() {
        let dir = write_two_scene_project();
        let mut app = build_test_app(dir.path().to_str().unwrap(), None);
        let mut frame: u64 = 0;

        let (resp, _) = execute_command(
            &mut app,
            &mut frame,
            Command::PressKey {
                key: "W".to_string(),
            },
        );
        assert!(resp.ok, "PressKey should succeed: {:?}", resp.error);

        let (resp, _) = execute_command(&mut app, &mut frame, Command::Step { frames: 1 });
        assert!(resp.ok);

        let input = app.world().resource::<Input<KeyCode>>();
        assert!(
            input.is_pressed(&KeyCode::W),
            "W should be held after PressKey + one step"
        );
        assert!(
            input.just_pressed(&KeyCode::W),
            "W should be just_pressed on the exact frame after PressKey"
        );

        // A second step with no new key event: just_pressed's one-frame
        // window has closed (clear_input_state ran again), but the level
        // "pressed" state persists until an explicit ReleaseKey.
        let (resp, _) = execute_command(&mut app, &mut frame, Command::Step { frames: 1 });
        assert!(resp.ok);
        let input = app.world().resource::<Input<KeyCode>>();
        assert!(input.is_pressed(&KeyCode::W), "W should still be held");
        assert!(
            !input.just_pressed(&KeyCode::W),
            "just_pressed should not still be true one frame later"
        );

        let (resp, _) = execute_command(
            &mut app,
            &mut frame,
            Command::ReleaseKey {
                key: "W".to_string(),
            },
        );
        assert!(resp.ok, "ReleaseKey should succeed: {:?}", resp.error);
        let (resp, _) = execute_command(&mut app, &mut frame, Command::Step { frames: 1 });
        assert!(resp.ok);

        let input = app.world().resource::<Input<KeyCode>>();
        assert!(!input.is_pressed(&KeyCode::W), "W should no longer be held");
        assert!(
            input.just_released(&KeyCode::W),
            "W should be just_released on the exact frame after ReleaseKey"
        );
    }
}
