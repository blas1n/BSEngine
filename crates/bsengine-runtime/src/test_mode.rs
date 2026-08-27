//! `bsengine-runtime --test <game>`: runs a game headless (no window, no
//! renderer) and drives it via newline-delimited JSON commands on stdin,
//! writing one JSON response per command to stdout. See
//! `docs/superpowers/specs/2026-07-22-ai-gameplay-e2e-testing-design.md`.

use std::io::{self, BufRead, Write};

use bevy_app::App;
use bevy_ecs::event::Events;
use bsengine_app::{LifetimePlugin, NavMeshPlugin, ParticlePlugin, TerrainPlugin, TimePlugin};
use bsengine_asset::{AssetIdentityPlugin, AssetPlugin, AssetStatusPlugin};
use bsengine_audio::AudioPlugin;
use bsengine_core::{EditorPlayState, InspectorState};
use bsengine_input::{ElementState, InputPlugin, KeyCode, KeyInput, MouseButton, MouseInput};
use bsengine_physics::PhysicsPlugin;
use bsengine_scene::ScenePlugin;
use bsengine_scripting::{ScriptingPlugin, KEY_MAPPINGS};
use serde_json::{json, Value};

use bsengine_gltf::{GltfPlugin, SkinnedMeshPlugin};
use bsengine_render::RenderPlugin;
use bsengine_rhi_wgpu::WgpuRHIPlugin;

use crate::scene_systems::{register_scene_systems, ProjectManifest};
use crate::test_protocol::{Command, CommandResponse};
use crate::test_query::{eval_op, eval_path, run_query};

/// Builds the headless app rooted at `project_dir`. Loads `scene_override`
/// (a path relative to `project_dir`, e.g. `"assets/scenes/level3.ron"`) if
/// given, otherwise falls back to `project.toml`'s `entry_scene` — lets a
/// replay log pin its own starting scene instead of always depending on
/// whatever the project's entry scene currently is (which changes as a
/// multi-level game's "real" entry point evolves during development).
pub fn build_test_app(project_dir: &str, scene_override: Option<&str>, fast_render: bool) -> App {
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
        // Historical note, since this comment predates the `RenderPlugin`/
        // `GltfPlugin`/`SkinnedMeshPlugin` stack added below: before those
        // existed here, nothing in this app ever requested a mesh, a shader
        // or a texture, so replaying `games/mini-arena` recorded zero such
        // paths while the same game windowed recorded its `fox.glb` and
        // `glow.wgsl` as `Loaded`. See
        // `mini_arenas_fox_mesh_loads_now_that_rendering_is_on` below for the
        // regression test that closed that gap; mesh/shader/texture requests
        // are tracked here the same as in the windowed runtime now. Sounds
        // were always tracked too (`AudioPlugin` and `playSound` are both
        // present). The distinction below still matters for anything this
        // app genuinely never requests: an empty map means "genuinely
        // nothing was requested", which is a true answer, whereas a missing
        // resource would have meant "the engine cannot say" while sounding
        // identical to a script.
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
        // The point of this task: renders every frame now, offscreen
        // instead of not at all. `WgpuRHIPlugin::offscreen` takes its
        // dimensions from `manifest.window` rather than a fixed constant so
        // a headless run renders at the same resolution the windowed
        // runtime would actually open a window at -- letting the two modes
        // diverge here would undermine any pixel comparison built on top of
        // this later. `RenderPlugin`, `GltfPlugin` and `SkinnedMeshPlugin`
        // are what actually request and draw a mesh or texture; without
        // them `get_asset_status` could never answer anything about a
        // `.glb`, which is the gap
        // `mini_arenas_fox_mesh_loads_now_that_rendering_is_on` below closes.
        //
        // Both this block's position (between `PhysicsPlugin` and
        // `NavMeshPlugin`) and its internal order deliberately mirror
        // `main.rs`'s windowed chain -- a headless GPU stack built in a
        // different order than the real runtime is a regression that would
        // pass every test here while behaving differently windowed.
        .add_plugins(WgpuRHIPlugin::offscreen(
            manifest.window.width,
            manifest.window.height,
            fast_render,
        ))
        .add_plugins(InputPlugin)
        .add_plugins(AudioPlugin)
        .add_plugins(PhysicsPlugin)
        .add_plugins(RenderPlugin)
        .add_plugins(GltfPlugin)
        .add_plugins(SkinnedMeshPlugin)
        .add_plugins(NavMeshPlugin)
        // Same two as the windowed runtime, and for the reason item 11/12
        // recorded: a plugin present in one host and absent in the other is a
        // feature that works when you look at it and not when you test it.
        // Both are pure CPU counters -- no thread, no GPU, no wall clock -- so
        // a replay stays exactly as reproducible with them as without.
        .add_plugins(LifetimePlugin)
        .add_plugins(ParticlePlugin)
        // Mirrors the windowed runtime (main.rs's run_windowed): see the
        // comment there for why this was missing entirely until roadmap item
        // 44's demo project needed it. Without it here, a `Terrain` entity in
        // a headless E2E replay or `--test` session would never grow chunks
        // either -- the same silent-inert failure, just in the other host.
        .add_plugins(TerrainPlugin)
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
    let mut app = build_test_app(project_dir, None, false);
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

    let mut app = build_test_app(project_dir, log.scene.as_deref(), true);
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
    use bevy_ecs::prelude::Entity;
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
        let mut app = build_test_app(dir.path().to_str().unwrap(), None, false);
        app.update();

        let names = crate::test_query::get_entity_names(app.world_mut());
        let names: Vec<String> = serde_json::from_value(names).unwrap();
        assert!(names.contains(&"SceneA".to_string()), "names: {names:?}");
        assert!(!names.contains(&"SceneB".to_string()), "names: {names:?}");
    }

    #[test]
    fn a_lifetime_actually_counts_down_in_the_test_app() {
        // LifetimePlugin was defined, exported and installed by nobody, so
        // `Bsengine.setLifetime()` had never despawned anything in a running
        // game. Asserting the plugin is "in the list" would not have caught
        // that -- there was no list entry to check. Asserting the behaviour
        // would have.
        let dir = write_two_scene_project();
        let mut app = build_test_app(dir.path().to_str().unwrap(), None, false);
        let doomed = app
            .world_mut()
            .spawn(bsengine_core::Lifetime::from_seconds(0.05))
            .id();
        assert!(app.world().get_entity(doomed).is_some());

        // The test app pins its step to 1/60, so four frames is past 0.05s.
        for _ in 0..4 {
            app.update();
        }
        assert!(
            app.world().get_entity(doomed).is_none(),
            "an expired Lifetime has to despawn its entity"
        );
    }

    #[test]
    fn a_particle_emitter_actually_emits_in_the_test_app() {
        // Same guard, same reason: a simulation plugin present in one host and
        // absent in the other is a feature that works when you look at it and
        // not when you test it.
        let dir = write_two_scene_project();
        let mut app = build_test_app(dir.path().to_str().unwrap(), None, false);
        let e = app
            .world_mut()
            .spawn((
                bsengine_core::Transform::default(),
                bsengine_core::ParticleEmitter {
                    rate: 0.0,
                    burst_count: 6,
                    particle_lifetime: 100.0,
                    ..Default::default()
                },
            ))
            .id();
        app.world_mut()
            .get_mut::<bsengine_core::ParticleEmitter>(e)
            .unwrap()
            .burst();

        app.update();

        assert_eq!(
            app.world()
                .get::<bsengine_core::ParticleEmitter>(e)
                .unwrap()
                .live
                .len(),
            6,
            "a queued burst has to be emitted by the headless app too"
        );
    }

    #[test]
    fn build_test_app_with_override_loads_that_scene_instead() {
        let dir = write_two_scene_project();
        let mut app = build_test_app(
            dir.path().to_str().unwrap(),
            Some("assets/scenes/b.ron"),
            false,
        );
        app.update();

        let names = crate::test_query::get_entity_names(app.world_mut());
        let names: Vec<String> = serde_json::from_value(names).unwrap();
        assert!(names.contains(&"SceneB".to_string()), "names: {names:?}");
        assert!(!names.contains(&"SceneA".to_string()), "names: {names:?}");
    }

    #[test]
    fn build_test_app_with_fast_render_true_produces_a_fast_render_surface() {
        let dir = write_two_scene_project();
        let mut app = build_test_app(dir.path().to_str().unwrap(), None, true);
        // `WgpuSurfaceResource` is inserted by a `Startup` system, which only
        // runs on the first `app.update()` -- not during `build_test_app`
        // itself. See `get_pixel_reads_the_last_rendered_frame` and
        // `screenshot_returns_a_decodable_png` below for the same pattern.
        app.update();
        let surface = app
            .world()
            .resource::<bsengine_rhi_wgpu::surface::WgpuSurfaceResource>();
        assert!(
            surface.0.is_fast_render(),
            "build_test_app(.., fast_render: true) must produce a WgpuSurface with \
             is_fast_render() true -- this is what run_replay_mode relies on for CI E2E \
             replay speed"
        );
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
        let mut app = build_test_app(&project_dir, None, false);
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
        let mut app = build_test_app(&project_dir, None, false);
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
        let mut app = build_test_app(&project_dir, None, false);
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
        let mut app = build_test_app(&project_dir, None, false);
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
        let mut app = build_test_app(&project_dir, None, false);
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
        let mut app = build_test_app(&project_dir, None, false);
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

    // mini-arena is the one recording whose scene names a real mesh
    // (fox.glb) and a real texture (the floor). Until this task, the
    // headless app built no RenderPlugin/GltfPlugin, so nothing here ever
    // requested either and get_asset_status answered "unknown" for both no
    // matter how they were spelled -- see game_content.rs's
    // mini_arenas_floor_is_textured for the test that had to work around
    // that gap by reading the scene file directly instead.
    #[test]
    fn mini_arenas_fox_mesh_loads_now_that_rendering_is_on() {
        let project_dir = format!("{}/../../games/mini-arena", env!("CARGO_MANIFEST_DIR"));
        let mut app = build_test_app(&project_dir, None, false);
        let mut status = Value::Null;
        for _ in 0..200 {
            app.update();
            status = crate::test_query::get_asset_status(app.world_mut(), "assets/models/fox.glb");
            if status == json!("loaded") {
                break;
            }
        }
        assert_eq!(
            status,
            json!("loaded"),
            "fox.glb should load now that build_test_app includes GltfPlugin; last status: {status}"
        );
    }

    #[test]
    fn get_pixel_reads_the_last_rendered_frame() {
        let project_dir = format!("{}/../../games/cube-evader", env!("CARGO_MANIFEST_DIR"));
        let mut app = build_test_app(&project_dir, None, false);
        app.update();

        let result =
            crate::test_query::run_query(app.world_mut(), "get_pixel", &json!({"x": 0, "y": 0}))
                .expect("get_pixel should succeed once RenderPlugin has drawn a frame");
        assert!(
            result.get("luma").and_then(|v| v.as_f64()).is_some(),
            "get_pixel should report a luma value, got {result:?}"
        );
    }

    #[test]
    fn screenshot_returns_a_decodable_png() {
        let project_dir = format!("{}/../../games/cube-evader", env!("CARGO_MANIFEST_DIR"));
        let mut app = build_test_app(&project_dir, None, false);
        app.update();

        let result = crate::test_query::run_query(app.world_mut(), "screenshot", &json!({}))
            .expect("screenshot should succeed once RenderPlugin has drawn a frame");
        let data_base64 = result["data_base64"]
            .as_str()
            .expect("screenshot should return a data_base64 string");
        use base64::Engine;
        let png_bytes = base64::engine::general_purpose::STANDARD
            .decode(data_base64)
            .expect("data_base64 should be valid base64");
        assert!(
            png_bytes.starts_with(&[0x89, b'P', b'N', b'G']),
            "screenshot bytes should start with the PNG magic number"
        );

        let decoded = image::load_from_memory(&png_bytes).expect("PNG should actually decode");
        assert_eq!(
            decoded.width(),
            result["width"].as_u64().unwrap() as u32,
            "decoded PNG width should match the reported width"
        );
        assert_eq!(
            decoded.height(),
            result["height"].as_u64().unwrap() as u32,
            "decoded PNG height should match the reported height"
        );
    }

    // Proves a scene-file `parent:` chain actually renders, not just that it
    // parses into a Parent component (Task 3's own tests already cover
    // that): moving only the grandparent's Transform must move the child on
    // screen, even though the scene file never names the grandparent on the
    // child directly -- only Parent -> GrandParent, one level at a time.
    #[test]
    fn moving_a_grandparent_moves_the_grandchild_on_screen() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("assets/scenes")).unwrap();
        std::fs::write(
            root.join("project.toml"),
            "[project]\nname = \"Hierarchy Test\"\nentry_scene = \"assets/scenes/hierarchy.ron\"\n",
        )
        .unwrap();
        std::fs::write(
            root.join("assets/scenes/hierarchy.ron"),
            r#"SceneDescriptor(entities: [
    EntityDescriptor(
        name: "Camera",
        camera: true,
        transform: Some((
            position: (0.0, 0.0, 8.0),
        )),
    ),
    EntityDescriptor(
        name: "GrandParent",
        transform: Some((
            position: (0.0, 0.0, 0.0),
        )),
    ),
    EntityDescriptor(
        name: "Parent",
        parent: Some("GrandParent"),
        transform: Some((
            position: (2.0, 0.0, 0.0),
        )),
    ),
    EntityDescriptor(
        name: "Child",
        parent: Some("Parent"),
        primitive: Some(Cube),
        emissive: Some((1.0, 0.0, 0.0)),
        transform: Some((
            position: (0.0, 0.0, 0.0),
        )),
    ),
])"#,
        )
        .unwrap();

        let project_dir = root.to_str().unwrap().to_string();
        let mut app = build_test_app(&project_dir, None, false);
        app.update();

        // Window defaults to 1280x720 (no [window] table -- see
        // WindowSection::default() in scene_systems.rs), so the exact
        // screen center is always (640, 360).
        let before = crate::test_query::run_query(
            app.world_mut(),
            "get_pixel",
            &json!({"x": 640, "y": 360}),
        )
        .expect("get_pixel should succeed once RenderPlugin has drawn a frame");
        let before_r = before["r"].as_u64().unwrap();
        let before_g = before["g"].as_u64().unwrap();
        assert!(
            before_r <= before_g + 20,
            "GrandParent is at the origin, so Child sits at world x=2 -- off the \
             camera's center axis (x=0) and should not read as red at the center \
             pixel yet, got {before:?}"
        );

        // Move only the grandparent. Child's world position becomes
        // GrandParent(-2,0,0) + Parent-local(2,0,0) + Child-local(0,0,0) =
        // (0,0,0) -- dead center of a camera looking straight down -Z.
        let mut grandparent_query = app.world_mut().query::<(&bsengine_scene::Name, Entity)>();
        let grandparent = grandparent_query
            .iter(app.world())
            .find(|(n, _)| n.0 == "GrandParent")
            .map(|(_, e)| e)
            .expect("GrandParent should have spawned from the scene file");
        app.world_mut()
            .get_mut::<bsengine_core::Transform>(grandparent)
            .unwrap()
            .position = glam::Vec3::new(-2.0, 0.0, 0.0).into();

        app.update();

        let after = crate::test_query::run_query(
            app.world_mut(),
            "get_pixel",
            &json!({"x": 640, "y": 360}),
        )
        .expect("get_pixel should succeed once RenderPlugin has drawn a frame");
        let after_r = after["r"].as_u64().unwrap();
        let after_g = after["g"].as_u64().unwrap();
        assert!(
            after_r > after_g + 20,
            "after moving only GrandParent's Transform, Child (parented to Parent, \
             which is parented to GrandParent -- never parented to GrandParent \
             directly) should now be dead center and red. This is what proves a \
             scene-file parent chain actually renders through more than one \
             level, not just that spawn_scene_entities attaches a Parent \
             component. got {after:?}"
        );
    }

    // Proves a scene-file `prefab:` reference actually renders end to end
    // (parse -> load the referenced .ron -> instantiate -> propagate ->
    // draw), not just that entities appear.
    #[test]
    fn prefab_referenced_from_a_scene_file_renders_at_the_instantiation_points_position() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("assets/prefabs")).unwrap();
        std::fs::create_dir_all(root.join("assets/scenes")).unwrap();
        std::fs::write(
            root.join("project.toml"),
            "[project]\nname = \"Prefab Test\"\nentry_scene = \"assets/scenes/main.ron\"\n",
        )
        .unwrap();
        std::fs::write(
            root.join("assets/prefabs/blip.ron"),
            r#"PrefabDescriptor(entities: [
    EntityDescriptor(
        name: "Blip",
        primitive: Some(Cube),
        emissive: Some((1.0, 0.0, 0.0)),
    ),
])"#,
        )
        .unwrap();
        std::fs::write(
            root.join("assets/scenes/main.ron"),
            r#"SceneDescriptor(entities: [
    EntityDescriptor(
        name: "Camera",
        camera: true,
        transform: Some((
            position: (0.0, 0.0, 8.0),
        )),
    ),
    EntityDescriptor(
        name: "Spawn1",
        prefab: Some("assets/prefabs/blip.ron"),
        transform: Some((position: (0.0, 0.0, 0.0))),
    ),
])"#,
        )
        .unwrap();

        let project_dir = root.to_str().unwrap().to_string();
        let mut app = build_test_app(&project_dir, None, false);
        app.update();

        // Window defaults to 1280x720 (no [window] table), so the exact
        // screen center is always (640, 360). The camera at (0,0,8) with
        // identity rotation looks straight down -Z, so a prefab
        // instantiated at the world origin lands dead center on screen.
        let pixel = crate::test_query::run_query(
            app.world_mut(),
            "get_pixel",
            &json!({"x": 640, "y": 360}),
        )
        .expect("get_pixel should succeed once RenderPlugin has drawn a frame");
        let r = pixel["r"].as_u64().unwrap();
        let g = pixel["g"].as_u64().unwrap();
        assert!(
            r > g + 20,
            "the prefab's emissive-red cube should render dead center, got {pixel:?}"
        );

        let mut q = app.world_mut().query::<&bsengine_scene::Name>();
        let names: Vec<String> = q.iter(app.world()).map(|n| n.0.clone()).collect();
        // The prefab's root (its only entity, "Blip") takes over the
        // instantiation point's own name verbatim -- see Task 3's identical
        // assertion and its comment for why. This prefab has no non-root
        // entity, so there's nothing to check a suffix on here; that's
        // already covered by Task 3's own unit tests. This test's job is
        // purely to prove the pixel actually renders, which the assertion
        // above already did.
        assert_eq!(
            names.iter().filter(|n| n.as_str() == "Spawn1").count(),
            1,
            "the prefab's root should be named exactly 'Spawn1', the instantiation \
             point's own name, taken over verbatim, names: {names:?}"
        );
        assert_eq!(
            names.len(),
            2,
            "exactly Camera and the instantiated prefab root should exist, names: {names:?}"
        );
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
        let mut app = build_test_app(dir.path().to_str().unwrap(), None, false);
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

    /// End-to-end walkability proof for roadmap item 44's terrain core
    /// (`games/terrain-demo`): a scene-authored `Terrain` entity, loaded and
    /// chunked through the *production* app stack (`build_test_app`, the same
    /// plugin list `--test`/E2E replays and `main.rs`'s windowed runtime use)
    /// rather than a hand-built app that only adds `TerrainPlugin` in
    /// isolation, supports a dropped dynamic body at the terrain's actual
    /// sampled height -- specifically at a point straddling the boundary
    /// between two chunks, which is the scenario the whole chunk-boundary
    /// duplicate-data design exists to make safe.
    ///
    /// `bsengine-app`'s own suite (`terrain.rs`'s
    /// `a_dropped_body_lands_on_the_chunk_it_visually_sits_above`) already
    /// proved a single flat chunk supports a dropped body with `TerrainPlugin`
    /// added directly to a minimal app; this proves the same physical
    /// property survives (a) a real, committed 16-bit heightmap PNG instead
    /// of a synthetic flat fixture, (b) a multi-chunk terrain at the seam
    /// between chunks instead of one chunk's interior, and (c) the actual
    /// runtime plugin list, which is not automatically the same list as (a) —
    /// `TerrainPlugin` was never registered in `build_test_app` or `main.rs`'s
    /// `run_windowed` until this test's own development surfaced that gap
    /// (see this file's and `main.rs`'s `TerrainPlugin` registration
    /// comments).
    #[test]
    fn a_dynamic_body_dropped_above_terrain_demo_comes_to_rest_supported_by_the_heightfield() {
        let project_dir = format!("{}/../../games/terrain-demo", env!("CARGO_MANIFEST_DIR"));
        let mut app = build_test_app(&project_dir, None, false);

        // Step until the scene's own "Ground" entity (the authored `Terrain`)
        // has finished generating its chunks -- the heightmap loads
        // asynchronously (`PendingTerrain`), so the exact landing frame isn't
        // fixed. A generous budget: this app also carries the full render/
        // gltf/physics stack, unlike `bsengine-app`'s narrower terrain tests.
        let mut terrain_ready = false;
        for _ in 0..200 {
            app.update();
            let mut q = app.world_mut().query::<(
                &bsengine_scene::Name,
                Option<&bsengine_app::terrain::TerrainChunksGenerated>,
            )>();
            if q.iter(app.world())
                .any(|(name, generated)| name.0 == "Ground" && generated.is_some())
            {
                terrain_ready = true;
                break;
            }
        }
        assert!(
            terrain_ready,
            "terrain-demo's Ground entity never finished generating chunks within 200 \
             frames -- either its Terrain component never spawned (heightmap_path \
             resolution regression?) or TerrainPlugin is missing from build_test_app again"
        );

        // The source of truth this assertion checks against is the committed
        // PNG itself, decoded independently here -- not a second copy of
        // whatever formula generated it -- so this test keeps meaning the
        // same thing even if the heightmap's own pixel content ever changes.
        let heightmap_path = format!("{project_dir}/assets/terrain/heightmap.png");
        let heightmap = image::open(&heightmap_path)
            .unwrap_or_else(|e| panic!("failed to open {heightmap_path}: {e}"))
            .to_luma16();

        // The scene's Terrain: 16x16 heightmap, chunk_count (2, 2), chunk_size
        // 20.0 -> 8 texels per chunk -> 2.5 world units per texel, and texel
        // column 8 is the shared boundary between chunk (0,*) and chunk
        // (1,*) (16 texels split into 2 chunks of 8 -- the last chunk's own
        // left edge and the first chunk's right edge both read texel column
        // 8, per `generate_chunks`' "+1 boundary column, absorbed from the
        // same underlying texel data" design). World x = 8 * 2.5 = 20.0 sits
        // exactly on that seam. z uses texel row 4 (world z = 10.0), safely
        // inside a single chunk on that axis -- so this drop point exercises
        // exactly one chunk-to-chunk boundary, not the (four-collider)
        // corner where all four chunks meet.
        //
        // (8, 4) specifically (not just any point on the x=8 seam): the
        // heightmap's generator placed a local flat extremum there in *both*
        // axes (see its own comment for why: a `cos`-based wave has its
        // gradient at zero exactly at the quarter-points its chunk boundary
        // sits on). A real dynamic sphere dropped anywhere else on a rolling
        // hill rolls downhill before settling -- which is real physics, not a
        // bug, but it also means the ball's final resting XZ would not be its
        // drop XZ, breaking this assertion's premise that resting height
        // tracks the heightmap's value *at the drop position*. Landing on a
        // local flat spot keeps that premise true while still proving the
        // property this test exists for: the collider is exactly where the
        // heightmap says the surface is, right at a chunk seam.
        let raw = heightmap.get_pixel(8, 4).0[0];
        let height_scale = 6.0f32;
        let expected_height = (raw as f32 / u16::MAX as f32) * height_scale;
        assert!(
            expected_height > 0.1 && expected_height < height_scale - 0.1,
            "sanity: the chosen drop point should read a non-trivial, non-extreme height \
             from the heightmap, got {expected_height} (raw {raw})"
        );

        let radius = 0.5f32;
        let start = glam::Vec3::new(20.0, expected_height + 10.0, 10.0);
        let ball = app
            .world_mut()
            .spawn((
                bsengine_core::Transform::from_position(start),
                bsengine_physics::RigidBody::dynamic(),
                bsengine_physics::Collider::ball(radius),
                bsengine_physics::PhysicsInput {
                    position: start.into(),
                    rotation: glam::Quat::IDENTITY.into(),
                },
            ))
            .id();

        for _ in 0..200 {
            app.update();
        }

        let y = app
            .world()
            .get::<bsengine_core::Transform>(ball)
            .unwrap()
            .position
            .0
            .y;
        let expected = expected_height + radius;
        assert!(
            (y - expected).abs() < 0.2,
            "expected the dropped ball to rest at y ~= {expected} (heightmap-decoded \
             terrain height {expected_height} at the chunk boundary + radius {radius}), \
             but it settled at y={y} -- either it fell through a gap at the chunk \
             boundary, or the collider isn't where the heightmap says the surface is"
        );
    }
}
