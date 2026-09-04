//! `bsengine-runtime --test <game>`: runs a game headless (no window, no
//! renderer) and drives it via newline-delimited JSON commands on stdin,
//! writing one JSON response per command to stdout. See
//! `docs/superpowers/specs/2026-07-22-ai-gameplay-e2e-testing-design.md`.

use std::io::{self, BufRead, Write};

use bevy_app::App;
use bevy_ecs::event::Events;
use bsengine_app::{
    AnimationPlugin, AnimationStateMachinePlugin, LifetimePlugin, NavMeshPlugin, ParticlePlugin,
    TerrainBrushPlugin, TerrainPlugin, TimePlugin,
};
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
    app.insert_resource(bsengine_core::OcclusionCullingEnabled(
        manifest.render.occlusion_culling,
    ));
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
        // Mirrors the windowed runtime (main.rs's run_windowed): the animation
        // clip sampler and the state-machine system that drives it. Without
        // AnimationStateMachinePlugin here, `advance_state_machines` is never
        // registered, so ASM triggers accumulate but never fire a transition —
        // the character stays in "locomotion" forever in headless mode while
        // the same scene transitions correctly when windowed.
        .add_plugins(AnimationPlugin)
        .add_plugins(AnimationStateMachinePlugin)
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
        // Mirrors the windowed runtime (main.rs's run_windowed): the terrain
        // brush's picking system, kept in both hosts for the same reason
        // TerrainPlugin itself is -- see the comment there.
        .add_plugins(TerrainBrushPlugin)
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

    /// Proves terrain texture splatting actually renders more than one
    /// blended color, not just "doesn't crash" -- the E2E-observable claim
    /// this whole sub-step exists to satisfy (roadmap item 44's "데모
    /// 씬에서 검증"). Deliberately does not assume exact camera-to-world
    /// pixel math: it samples a coarse grid across the whole rendered frame
    /// and checks for at least 2 distinct (coarsely-bucketed) colors, which
    /// is what a single-material, un-blended terrain (this test's own
    /// regression target) could never produce, while staying robust to the
    /// demo scene's exact camera framing.
    #[test]
    fn terrain_demo_renders_more_than_one_blended_color() {
        let project_dir = format!("{}/../../games/terrain-demo", env!("CARGO_MANIFEST_DIR"));
        let mut app = build_test_app(&project_dir, None, false);

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
            "terrain-demo's Ground entity never finished generating chunks"
        );

        // One more frame so the freshly-generated chunks (and their just-
        // uploaded splat textures) actually get drawn before sampling.
        app.update();

        let mut colors = std::collections::HashSet::new();
        for gx in 0..8u32 {
            for gy in 0..8u32 {
                let x = 60 + gx * 150;
                let y = 40 + gy * 80;
                let result = crate::test_query::run_query(
                    app.world_mut(),
                    "get_pixel",
                    &serde_json::json!({"x": x, "y": y}),
                )
                .expect("get_pixel should succeed once RenderPlugin has drawn a frame");
                // Coarse 24-unit buckets so shading/AA noise across one
                // material's surface doesn't inflate the distinct-color
                // count -- only a genuinely different albedo crosses a
                // bucket boundary.
                let r = result["r"].as_u64().unwrap_or(0) / 24;
                let g = result["g"].as_u64().unwrap_or(0) / 24;
                let b = result["b"].as_u64().unwrap_or(0) / 24;
                colors.insert((r, g, b));
            }
        }
        assert!(
            colors.len() >= 2,
            "expected at least 2 visually distinct colors across the sampled terrain \
             surface (proving splat layers actually blend, not just the default white \
             fallback or one flat layer), got {} distinct color buckets: {:?}",
            colors.len(),
            colors
        );
    }

    /// Task 10 (roadmap item 44's final sub-step): proves a terrain brush
    /// edit committed through the real `pick_terrain_under_cursor` ->
    /// `preview_terrain_brush_stroke` -> `commit_terrain_brush_stroke` chain
    /// (`TerrainBrushPlugin`, registered in `build_test_app` above the same
    /// way `TerrainPlugin` is) actually survives a full scene reload -- not
    /// just that the heightmap PNG on disk changed
    /// (`bsengine-app::terrain_brush`'s own
    /// `committing_a_height_stroke_writes_the_edited_heightmap_to_disk`
    /// already proved that against a synthetic in-memory `Terrain`), but
    /// that a *second*, independently-built `build_test_app` -- standing in
    /// for "close and reopen the project" -- reads the edited PNG back off
    /// disk and a dropped body actually lands at the new height, not the
    /// original.
    ///
    /// Deliberately builds a small throwaway project under
    /// `tempfile::tempdir()` (the same pattern
    /// `moving_a_grandparent_moves_the_grandchild_on_screen` above uses)
    /// rather than driving this against the committed `games/terrain-demo`
    /// fixture: `heightmap.png` there is a shared, committed fixture two
    /// other tests in this file
    /// (`a_dynamic_body_dropped_above_terrain_demo_comes_to_rest_
    /// supported_by_the_heightfield`,
    /// `terrain_demo_renders_more_than_one_blended_color`) read as ground
    /// truth, and `commit_terrain_brush_stroke` writes `std::fs::
    /// write(&terrain.heightmap_path, ..)` for real. Mutating that shared
    /// file here would need a guaranteed-restore mechanism (an RAII guard,
    /// since a failed assertion must not skip the restore) that nothing
    /// else in this codebase has needed before -- no existing test mutates
    /// a committed fixture in place. A fresh temp project sidesteps that
    /// entirely: nothing under `games/terrain-demo` is ever opened for
    /// writing, so there is nothing to restore and no risk to `git status`
    /// or to those other tests' fixture data.
    ///
    /// The temp project's scene authors `Terrain.heightmap_path`/
    /// `layer*_texture_path` project-relative (`"assets/terrain/
    /// heightmap.png"`, the same convention `games/terrain-demo`'s own
    /// `main.ron` uses), *not* pre-built as absolute strings: `bsengine-
    /// scene`'s scene deserializer (`plugin.rs`, the `Terrain`-specific
    /// branch of its generic `components:` handling) already resolves
    /// exactly those five fields through `bsengine_core::resolve_project_
    /// path(project_dir, path)` -- i.e. `format!("{project_dir}/{path}")`
    /// -- before the `Terrain` component is ever inserted. Since this
    /// test's `project_dir` (passed to `build_test_app` below) is itself
    /// absolute (`tempdir()`'s own path), the `Terrain` component that
    /// actually lands on the `Ground` entity ends up with a fully absolute
    /// `heightmap_path` regardless of the test binary's CWD -- which is
    /// exactly what `commit_terrain_brush_stroke`'s verbatim `std::fs::
    /// write(&terrain.heightmap_path, ..)` needs. (An earlier version of
    /// this test pre-resolved the paths itself and embedded *those* in the
    /// RON; `resolve_project_path` then prefixed `project_dir` onto an
    /// already-absolute path a second time, producing a malformed
    /// `<project_dir>/C:/Users/...` string and an OS "invalid filename"
    /// error from `bevy_asset` -- project-relative paths in the RON, left
    /// for the scene loader to resolve exactly once, are the correct
    /// shape.)
    #[test]
    fn terrain_brush_edit_survives_a_reload() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("assets/scenes")).unwrap();
        std::fs::create_dir_all(root.join("assets/terrain")).unwrap();

        // 4x4 texels, one 10x10-world-unit chunk, flat at raw=20_000 -- the
        // exact fixture shape `bsengine-app::terrain_brush`'s own
        // `held_height_stroke_then_commit_raises_where_a_dropped_body_lands`
        // uses, since that combination is already proven to make a dropped
        // ball's resting height track the brushed texel precisely.
        let width = 4u32;
        let height = 4u32;
        let flat_raw: u16 = 20_000;
        let height_scale = 20.0f32;
        let chunk_size = 10.0f32;
        let original_height = (flat_raw as f32 / u16::MAX as f32) * height_scale;

        // Written directly under `root` (not through any path-resolution
        // helper) since these are the real files the scene's
        // project-relative `Terrain` paths, once resolved against
        // `project_dir` (== `root`), must actually find on disk.
        let heightmap_path = root.join("assets/terrain/heightmap.png");
        let img: image::ImageBuffer<image::Luma<u16>, Vec<u16>> =
            image::ImageBuffer::from_raw(width, height, vec![flat_raw; (width * height) as usize])
                .unwrap();
        image::DynamicImage::ImageLuma16(img)
            .save(&heightmap_path)
            .expect("write the temp project's fixture heightmap");

        image::RgbaImage::from_pixel(2, 2, image::Rgba([80u8, 160, 80, 255]))
            .save(root.join("assets/terrain/grass.png"))
            .expect("write the temp project's fixture layer texture");
        image::RgbaImage::from_pixel(2, 2, image::Rgba([120u8, 120, 120, 255]))
            .save(root.join("assets/terrain/rock.png"))
            .expect("write the temp project's fixture layer texture");
        image::RgbaImage::from_pixel(2, 2, image::Rgba([110u8, 80, 40, 255]))
            .save(root.join("assets/terrain/dirt.png"))
            .expect("write the temp project's fixture layer texture");
        image::RgbaImage::from_pixel(2, 2, image::Rgba([240u8, 240, 250, 255]))
            .save(root.join("assets/terrain/snow.png"))
            .expect("write the temp project's fixture layer texture");

        std::fs::write(
            root.join("project.toml"),
            "[project]\nname = \"Terrain Brush E2E\"\nentry_scene = \"assets/scenes/main.ron\"\n",
        )
        .unwrap();

        std::fs::write(
            root.join("assets/scenes/main.ron"),
            format!(
                r#"SceneDescriptor(entities: [
    EntityDescriptor(
        name: "Camera",
        camera: true,
        transform: Some((position: (5.0, 15.0, 25.0))),
        look_at: Some((5.0, 0.0, 5.0)),
    ),
    EntityDescriptor(
        name: "Ground",
        transform: Some((position: (0.0, 0.0, 0.0))),
        components: [
            ("bsengine_scene::types::Terrain", "(heightmap_path: \"assets/terrain/heightmap.png\", chunk_count: (1, 1), chunk_size: {chunk_size:.1}, height_scale: {height_scale:.1}, layer0_texture_path: \"assets/terrain/grass.png\", layer1_texture_path: \"assets/terrain/rock.png\", layer2_texture_path: \"assets/terrain/dirt.png\", layer3_texture_path: \"assets/terrain/snow.png\", splatmap_path: None)"),
        ],
    ),
])"#
            ),
        )
        .unwrap();

        let project_dir = root.to_str().unwrap().to_string();

        // --- Phase 1: edit, through the real app stack ---
        let mut app = build_test_app(&project_dir, None, false);

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
            "the temp project's Ground entity never finished generating chunks within \
             200 frames"
        );

        let terrain_entity = {
            let mut q = app.world_mut().query::<(Entity, &bsengine_scene::Name)>();
            q.iter(app.world())
                .find(|(_, n)| n.0 == "Ground")
                .map(|(e, _)| e)
                .expect("Ground should have spawned from the scene file")
        };

        // Hold a raising height stroke centered on the chunk for a few
        // frames (mirrors `held_height_stroke_then_commit_raises_where_a_
        // dropped_body_lands`'s exact pattern), then release it -- the
        // Some -> None transition on the next `app.update()` is what
        // `commit_terrain_brush_stroke` watches for.
        let drop_xz = chunk_size / 2.0;
        {
            let mut insp = app.world_mut().resource_mut::<InspectorState>();
            insp.terrain_brush_settings.kind =
                bsengine_core::TerrainBrushKind::Height { raise: true };
            insp.terrain_brush_settings.radius = 20.0;
            insp.terrain_brush_settings.strength = 1.0;
            insp.terrain_brush_stroke = Some(bsengine_core::TerrainBrushStroke {
                terrain_entity_id: terrain_entity.index() as u64,
                world_pos: [drop_xz, 0.0, drop_xz],
            });
        }
        for _ in 0..5 {
            app.update();
        }
        {
            let mut insp = app.world_mut().resource_mut::<InspectorState>();
            insp.terrain_brush_stroke = None;
        }
        app.update(); // the Some -> None transition frame: commits to disk

        // Confirm the heightmap PNG on disk actually changed -- re-decoded
        // independently, not read back through any in-memory cache.
        let edited_bytes =
            std::fs::read(&heightmap_path).expect("heightmap PNG should still exist after commit");
        let edited = bsengine_asset::heightmap_loader::decode_heightmap_png(&edited_bytes)
            .expect("decode the committed heightmap");
        assert_eq!(edited.width, width);
        assert_eq!(edited.height, height);
        assert_ne!(
            edited.data,
            vec![flat_raw; (width * height) as usize],
            "committing a height stroke must actually change the on-disk heightmap"
        );

        // `terrain_chunking::world_to_texel` (the real conversion
        // `preview_terrain_brush_stroke`/`commit_terrain_brush_stroke` use)
        // is `pub(crate)` to `bsengine-app`, unreachable from this crate --
        // but with a single 1x1 chunk its formula collapses to
        // `world / (chunk_size / texel_count)`, reproduced here directly.
        let step = chunk_size / width as f32;
        let tx = (drop_xz / step).round().clamp(0.0, (width - 1) as f32) as u32;
        let tz = (drop_xz / step).round().clamp(0.0, (height - 1) as f32) as u32;
        let raw = edited.data[(tz * edited.width + tx) as usize];
        let new_height = (raw as f32 / u16::MAX as f32) * height_scale;
        assert!(
            new_height > original_height + 0.5,
            "the committed heightmap should read measurably higher at the brushed texel: \
             new={new_height}, original={original_height}"
        );

        // Release phase 1's app (and its GPU device/window-less surface)
        // before building a second one below. This is deliberately two
        // separate `App`s, not one continuing app: the property this test
        // exists to prove is specifically that the edit survives a
        // *reload* (a second app reading the same project fresh), not just
        // that it survives while the first app stays resident in memory.
        drop(app);

        // --- Phase 2: reload, through a brand-new app pointed at the same
        // project directory ---
        let mut app = build_test_app(&project_dir, None, false);

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
            "the reloaded temp project's Ground entity never finished generating chunks \
             within 200 frames"
        );

        let radius = 0.5f32;
        let start = glam::Vec3::new(drop_xz, new_height + 10.0, drop_xz);
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
        let expected = new_height + radius;
        assert!(
            (y - expected).abs() < 0.3,
            "expected the ball to rest at the BRUSHED height y~={expected} (reloaded \
             terrain height {new_height} at the drop point, original was \
             {original_height}), but it settled at y={y} -- the reloaded terrain does not \
             reflect the committed edit"
        );
        assert!(
            (y - (original_height + radius)).abs() > 0.5,
            "the ball must land at the NEW height after reload, not the original -- got \
             y={y}, original resting height would have been {}",
            original_height + radius
        );
    }

    /// Builds a minimal, valid binary glTF (`.glb`) file containing exactly
    /// one triangle: a 12-byte header, a `JSON` chunk (scene/node/mesh/
    /// accessor structure, no external files -- the vertex/index data lives
    /// in the `BIN` chunk right below it), and a `BIN` chunk (3 `f32x3`
    /// positions + 3 `u16` indices). Every length/offset here follows the
    /// glTF 2.0 binary container spec directly (4-byte-aligned chunks, JSON
    /// padded with ASCII spaces, BIN padded with zero bytes).
    ///
    /// Exists only for
    /// `lod_reduces_triangle_count_when_far_from_camera` below, which needs
    /// two real, independently loadable glTF fixtures with a genuinely
    /// different triangle count -- searched this repository first (outside
    /// `target/`, the only committed `.glb`/`.gltf` fixture anywhere is
    /// `games/mini-arena/assets/models/fox.glb`, reused below as the
    /// high-poly LOD 0 mesh) and found no second fixture and no existing
    /// glTF-*writing* helper to reuse, so this constructs the low-poly LOD
    /// 1 fixture by hand instead. Verified during this test's development
    /// (not merely assumed) to actually round-trip through
    /// `GltfLoader::load_full` -- the exact function `GltfSourceLoader`
    /// (and therefore `load_lod_assets`) calls -- reporting `indices.len()
    /// == 3`, i.e. exactly one triangle.
    fn build_single_triangle_glb() -> Vec<u8> {
        let positions: [[f32; 3]; 3] = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
        let mut bin: Vec<u8> = Vec::new();
        for v in positions {
            for c in v {
                bin.extend_from_slice(&c.to_le_bytes());
            }
        }
        let pos_bytes = bin.len() as u32; // 36: 3 vertices * 3 floats * 4 bytes
        for idx in [0u16, 1, 2] {
            bin.extend_from_slice(&idx.to_le_bytes());
        }
        let idx_bytes = bin.len() as u32 - pos_bytes; // 6: 3 indices * 2 bytes
        let total_bin_len = bin.len() as u32; // 42, before BIN-chunk padding
        while !bin.len().is_multiple_of(4) {
            bin.push(0); // BIN chunk padding must be zero bytes, per spec
        }

        let json = format!(
            r#"{{"asset":{{"version":"2.0"}},"scene":0,"scenes":[{{"nodes":[0]}}],"nodes":[{{"mesh":0}}],"meshes":[{{"primitives":[{{"attributes":{{"POSITION":0}},"indices":1,"mode":4}}]}}],"buffers":[{{"byteLength":{total_bin_len}}}],"bufferViews":[{{"buffer":0,"byteOffset":0,"byteLength":{pos_bytes},"target":34962}},{{"buffer":0,"byteOffset":{pos_bytes},"byteLength":{idx_bytes},"target":34963}}],"accessors":[{{"bufferView":0,"componentType":5126,"count":3,"type":"VEC3","min":[0.0,0.0,0.0],"max":[1.0,1.0,0.0]}},{{"bufferView":1,"componentType":5123,"count":3,"type":"SCALAR"}}]}}"#
        );
        let mut json_bytes = json.into_bytes();
        while !json_bytes.len().is_multiple_of(4) {
            json_bytes.push(0x20); // JSON chunk padding must be ASCII spaces, per spec
        }

        let mut glb = Vec::new();
        glb.extend_from_slice(b"glTF");
        glb.extend_from_slice(&2u32.to_le_bytes());
        let total_len_pos = glb.len();
        glb.extend_from_slice(&0u32.to_le_bytes()); // total length, patched below

        glb.extend_from_slice(&(json_bytes.len() as u32).to_le_bytes());
        glb.extend_from_slice(b"JSON");
        glb.extend_from_slice(&json_bytes);

        glb.extend_from_slice(&(bin.len() as u32).to_le_bytes());
        glb.extend_from_slice(b"BIN\0");
        glb.extend_from_slice(&bin);

        let total_len = glb.len() as u32;
        glb[total_len_pos..total_len_pos + 4].copy_from_slice(&total_len.to_le_bytes());
        glb
    }

    /// Task 8, and the roadmap item 43 condition it exists to satisfy
    /// directly ("프로파일러로 LOD 켬/끔 시 프레임 비용 차이를 실측해 효과
    /// 입증" -- prove the effect via the profiler with a measured before/
    /// after, not just "LOD doesn't crash"). `bsengine-render`'s own
    /// `lod_current_index_updates_based_on_camera_distance` already proves
    /// `current_index` responds to distance, and `bsengine-gltf`'s
    /// `lod_request_becomes_lod_levels_once_every_file_loads` already
    /// proves a `LodRequest` resolves into distinct registered mesh ids --
    /// neither proves the thing actually rendered fewer triangles. This
    /// test drives the real headless app (`build_test_app`, the same stack
    /// `--test`/E2E replays and the windowed runtime use) end to end and
    /// reads `get_frame_stats` (the real frame profiler, item 43/PR #1805 --
    /// `crate::test_query::get_frame_stats`, dispatched by exactly the
    /// string `"get_frame_stats"` with empty `{}` args, confirmed by
    /// reading `test_query.rs` and its own
    /// `run_query_dispatches_get_frame_stats` test before writing this one)
    /// with the far LOD active, then again with LOD 0 active, and asserts
    /// the near frame's triangle count is strictly greater.
    ///
    /// Fixtures: LOD 0 is `games/mini-arena`'s real `fox.glb` (576
    /// triangles), copied into this test's own temp project so the scene's
    /// `gltf:`/`lod:` fields can stay project-relative -- exactly the
    /// pattern `terrain_brush_edit_survives_a_reload` above already
    /// established for copying fixtures into a throwaway `tempfile::
    /// tempdir()` project rather than mutating or pointing absolutely at a
    /// shared `games/*` fixture. LOD 1 is `build_single_triangle_glb`'s
    /// hand-built one-triangle `.glb` above -- see that function's own doc
    /// comment for why a hand-built fixture was necessary here.
    ///
    /// The `Tree` entity sits on the camera's forward axis (camera at the
    /// origin with the default identity rotation looks down -Z, the same
    /// convention `moving_a_grandparent_moves_the_grandchild_on_screen` and
    /// `prefab_referenced_from_a_scene_file_renders_at_the_instantiation_
    /// points_position` above both rely on), so its exact camera distance
    /// is just the absolute value of its own z position -- 300 units away
    /// (well past `switch_distances: [50.0]` plus the 2.0 hysteresis band's
    /// half-width) for the far phase, then moved to 5 units away (well
    /// inside the same band's floor) for the near phase.
    #[test]
    fn lod_reduces_triangle_count_when_far_from_camera() {
        use bsengine_render::components::LodLevels;

        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("assets/scenes")).unwrap();
        std::fs::create_dir_all(root.join("assets/models")).unwrap();

        let fox_bytes = std::fs::read(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../games/mini-arena/assets/models/fox.glb"),
        )
        .expect("games/mini-arena's fox.glb fixture should exist");
        std::fs::write(root.join("assets/models/fox.glb"), &fox_bytes)
            .expect("write the temp project's copy of fox.glb");
        std::fs::write(
            root.join("assets/models/lowpoly.glb"),
            build_single_triangle_glb(),
        )
        .expect("write the temp project's hand-built single-triangle LOD fixture");

        std::fs::write(
            root.join("project.toml"),
            "[project]\nname = \"LOD E2E\"\nentry_scene = \"assets/scenes/main.ron\"\n",
        )
        .unwrap();
        std::fs::write(
            root.join("assets/scenes/main.ron"),
            r#"SceneDescriptor(entities: [
    EntityDescriptor(
        name: "Camera",
        camera: true,
        transform: Some((position: (0.0, 0.0, 0.0))),
    ),
    EntityDescriptor(
        name: "Tree",
        gltf: Some("assets/models/fox.glb"),
        lod: Some((
            levels: ["assets/models/lowpoly.glb"],
            distances: [50.0],
            hysteresis_band: 2.0,
        )),
        transform: Some((position: (0.0, 0.0, -300.0))),
    ),
])"#,
        )
        .unwrap();

        let project_dir = root.to_str().unwrap().to_string();
        let mut app = build_test_app(&project_dir, None, false);

        // --- Phase 1: far -- the low-poly LOD level should be selected ---
        let mut far_selected = false;
        for _ in 0..300 {
            app.update();
            let mut q = app
                .world_mut()
                .query::<(&bsengine_scene::Name, &LodLevels)>();
            if let Some((_, lod)) = q.iter(app.world()).find(|(n, _)| n.0 == "Tree") {
                if lod.current_index.is_some() {
                    far_selected = true;
                    break;
                }
            }
        }
        assert!(
            far_selected,
            "Tree's LodLevels never selected a level beyond LOD0 within 300 frames -- \
             either the base mesh or the LOD level failed to load (check the fixtures \
             above), or something is wrong with LOD selection itself: at 300 units \
             away with switch_distances=[50.0], it must have selected the far level"
        );

        let far_stats =
            crate::test_query::run_query(app.world_mut(), "get_frame_stats", &json!({}))
                .expect("get_frame_stats should succeed once RenderPlugin has drawn a frame");
        let far_triangles = far_stats["triangles"]
            .as_u64()
            .expect("get_frame_stats should report triangles as a number");

        // --- Phase 2: near -- move Tree close, LOD0 should be reselected ---
        let tree = {
            let mut q = app.world_mut().query::<(Entity, &bsengine_scene::Name)>();
            q.iter(app.world())
                .find(|(_, n)| n.0 == "Tree")
                .map(|(e, _)| e)
                .expect("Tree should have spawned from the scene file")
        };
        app.world_mut()
            .get_mut::<bsengine_core::Transform>(tree)
            .unwrap()
            .position = glam::Vec3::new(0.0, 0.0, -5.0).into();

        let mut near_selected = false;
        for _ in 0..20 {
            app.update();
            let lod = app
                .world()
                .get::<LodLevels>(tree)
                .expect("LodLevels should still be attached once selected");
            if lod.current_index.is_none() {
                near_selected = true;
                break;
            }
        }
        assert!(
            near_selected,
            "Tree's LodLevels never dropped back to LOD0 (current_index=None) within 20 \
             frames after moving to 5 units from the camera -- well inside the \
             49.0-unit hysteresis floor (switch_distances=[50.0], hysteresis_band=2.0)"
        );

        let near_stats =
            crate::test_query::run_query(app.world_mut(), "get_frame_stats", &json!({}))
                .expect("get_frame_stats should succeed once RenderPlugin has drawn a frame");
        let near_triangles = near_stats["triangles"]
            .as_u64()
            .expect("get_frame_stats should report triangles as a number");

        // The real proof: not "both ran without error", but a measured
        // triangle-count drop. `render_frame` builds one draw-call list from
        // `current_index`'s selected mesh id and every consumer of it (main
        // opaque pass, shadow pass -- both unconditional here since this
        // test does not request `fast_render`) draws that same id, so the
        // near frame (LOD0 = fox.glb, 576 triangles) should report on the
        // order of 2*(576-1) = 1150 more triangles than the far frame
        // (LOD1 = the 1-triangle fixture) -- any constant per-frame
        // contribution from post-processing is identical between the two
        // frames and cancels out of this comparison.
        assert!(
            near_triangles > far_triangles,
            "LOD must measurably reduce triangle count: near (LOD0, fox.glb, 576 \
             triangles) frame reported {near_triangles} triangles, far (LOD1, the \
             1-triangle fixture) frame reported {far_triangles} triangles -- equal or \
             reversed counts mean either the LOD fixtures don't actually differ or LOD \
             selection isn't taking effect (current_index was already checked above)"
        );
        assert!(
            near_triangles > far_triangles + 500,
            "the near/far triangle-count gap ({near_triangles} vs {far_triangles}) is \
             far smaller than the ~1150 this specific fixture pair (576 vs 1 triangles, \
             drawn in both the main and shadow passes) should produce -- a small but \
             nonzero gap would suggest the wrong mesh is being measured somewhere, not \
             a genuinely working LOD reduction"
        );
    }

    /// World-space (x, y) of the 30 small cubes parked 40 units behind the
    /// wall, all of them well inside its screen-space silhouette.
    ///
    /// Two blocks (upper-left and lower-right) rather than one grid
    /// straddling the view axis, and that is load-bearing rather than
    /// decorative. `rasterize_occluder_box` splits each box face into two
    /// triangles and is inner-conservative, so it leaves the seam along
    /// their shared diagonal unwritten -- for a wall centred on the camera
    /// axis that diagonal runs corner-to-corner through the middle of the
    /// silhouette, exactly where `ndc.x == ndc.y`. Anything sitting on it
    /// finds an uncovered pixel and correctly reports itself un-occluded
    /// (a hole under-culls, which is the safe direction, and
    /// `an_entity_beside_an_occluder_is_not_culled_while_one_behind_it_is`
    /// in `bsengine-render` documents the same thing). Keeping every
    /// candidate in a quadrant where `x` and `y` have opposite signs puts
    /// `ndc.x` and `ndc.y` on opposite sides of zero, so none of them can
    /// land on that seam no matter what the exact projection works out to.
    ///
    /// The bounds themselves: at z = -60 with a 60-degree vertical FOV and
    /// a 16:9 aspect the visible half-extents are 61.6 x 34.6 world units,
    /// and the wall's occluder box covers |ndc.x| < 0.59, |ndc.y| < 0.70 --
    /// i.e. |x| < 36 and |y| < 24 at this depth. Every entry below is
    /// inside that with at least five buffer pixels to spare, which also
    /// keeps them far from the 128x128 buffer's own edge (`box_occluded`
    /// refuses to cull anything whose screen rect leaves the buffer).
    const OCCLUSION_HIDDEN_XY: [(f32, f32); 30] = [
        (-30.0, 5.0),
        (-30.0, 12.0),
        (-30.0, 19.0),
        (-24.0, 5.0),
        (-24.0, 12.0),
        (-24.0, 19.0),
        (-18.0, 5.0),
        (-18.0, 12.0),
        (-18.0, 19.0),
        (-12.0, 5.0),
        (-12.0, 12.0),
        (-12.0, 19.0),
        (-6.0, 5.0),
        (-6.0, 12.0),
        (-6.0, 19.0),
        (6.0, -5.0),
        (6.0, -12.0),
        (6.0, -19.0),
        (12.0, -5.0),
        (12.0, -12.0),
        (12.0, -19.0),
        (18.0, -5.0),
        (18.0, -12.0),
        (18.0, -19.0),
        (24.0, -5.0),
        (24.0, -12.0),
        (24.0, -19.0),
        (30.0, -5.0),
        (30.0, -12.0),
        (30.0, -19.0),
    ];

    /// World-space (x, y) of the three cubes at the same depth as the
    /// hidden ones but off to the right of the wall entirely: at x = 48 the
    /// camera ray to them misses the occluder (its silhouette ends at
    /// x = 36 at this depth) while staying inside both the view frustum and
    /// the occlusion buffer, so they are genuinely *tested* against the
    /// buffer and genuinely found visible -- not skipped for being off
    /// screen, which would prove nothing.
    const OCCLUSION_BESIDE_XY: [(f32, f32); 3] = [(48.0, 0.0), (48.0, 10.0), (48.0, -10.0)];

    /// Writes the temp project this test drives, with `occlusion_culling`
    /// set either way. Called twice against the same directory: the second
    /// call rewrites only `project.toml`, so the scene the two runs render
    /// is byte-identical and the toggle is the single variable.
    fn write_occlusion_project(root: &std::path::Path, occlusion_culling: bool) {
        use std::fmt::Write as _;

        std::fs::create_dir_all(root.join("assets/scenes")).unwrap();
        std::fs::write(
            root.join("project.toml"),
            format!(
                "[project]\nname = \"Occlusion E2E\"\n\
                 entry_scene = \"assets/scenes/main.ron\"\n\
                 [render]\nocclusion_culling = {occlusion_culling}\n"
            ),
        )
        .unwrap();

        // The camera sits at the origin unrotated, which looks down -Z --
        // the same convention `lod_reduces_triangle_count_when_far_from_
        // camera` above and the prefab/hierarchy tests before it rely on.
        //
        // The wall is a unit `Cube` primitive scaled to 24 x 16 x 1, so its
        // real geometry spans +-12 x +-8 x +-0.5 world units at z = -20.
        // Its `Occluder` box is authored in LOCAL space (the model matrix
        // supplies the scale) at half-extent 0.49 rather than the mesh's
        // own 0.5: an occluder must fit *inside* the geometry it stands in
        // for, because rasterizing a blocker larger than the real thing is
        // the one way this feature can hide something that was visible.
        //
        // `Occluder` has no typed `EntityDescriptor` field (unlike
        // `gltf:`/`lod:`), so it is authored through the generic reflected
        // `components:` list as a (type path, RON value) pair -- the same
        // mechanism `games/terrain-demo`'s `Terrain` and `games/mini-arena`'s
        // `Bloom`/`ToneMap`/`AudioListener` entries use. Both `center` and
        // `half_extents` are `ReflectVec3`, which serialises as a plain
        // three-float sequence, hence the `(x, y, z)` tuple syntax.
        let mut scene = String::from(
            r#"SceneDescriptor(entities: [
    EntityDescriptor(
        name: "Camera",
        camera: true,
        transform: Some((position: (0.0, 0.0, 0.0))),
    ),
    EntityDescriptor(
        name: "Wall",
        primitive: Some(Cube),
        transform: Some((position: (0.0, 0.0, -20.0), scale: (24.0, 16.0, 1.0))),
        components: [
            ("bsengine_render::components::Occluder", "(center: (0.0, 0.0, 0.0), half_extents: (0.49, 0.49, 0.49))"),
        ],
    ),
"#,
        );
        for (i, (x, y)) in OCCLUSION_HIDDEN_XY.iter().enumerate() {
            writeln!(
                scene,
                "    EntityDescriptor(name: \"Hidden{i}\", primitive: Some(Cube), \
                 transform: Some((position: ({x:.1}, {y:.1}, -60.0)))),"
            )
            .unwrap();
        }
        for (i, (x, y)) in OCCLUSION_BESIDE_XY.iter().enumerate() {
            writeln!(
                scene,
                "    EntityDescriptor(name: \"Beside{i}\", primitive: Some(Cube), \
                 transform: Some((position: ({x:.1}, {y:.1}, -60.0)))),"
            )
            .unwrap();
        }
        scene.push_str("])\n");
        std::fs::write(root.join("assets/scenes/main.ron"), scene).unwrap();
    }

    /// Steps `app` until every one of its `expected` primitive entities has
    /// had its `PrimitiveMesh` resolved into a real `MeshRenderer` (that is
    /// what makes an entity a culling candidate at all -- `render_frame`
    /// only tests entities whose mesh id has registered bounds), then a few
    /// frames more so the stats read back belong to a fully populated
    /// scene rather than a half-loaded one.
    fn step_until_meshes_ready(app: &mut App, expected: usize) {
        let mut ready = false;
        for _ in 0..300 {
            app.update();
            let mut q = app
                .world_mut()
                .query::<&bsengine_render::components::MeshRenderer>();
            if q.iter(app.world()).count() >= expected {
                ready = true;
                break;
            }
        }
        assert!(
            ready,
            "the occlusion temp project never resolved all {expected} primitive meshes \
             within 300 frames -- without a MeshRenderer (and the registered bounds that \
             come with it) an entity is never a culling candidate, so nothing below would \
             be measuring occlusion"
        );
        for _ in 0..3 {
            app.update();
        }
    }

    /// Task 9, and the measured proof the whole occlusion feature exists to
    /// produce: the profiler must show draw calls actually dropping, and it
    /// must show the objects that were never hidden still being drawn.
    ///
    /// `bsengine-render`'s own unit tests already prove the pure functions
    /// (`box_occluded` says yes behind a wall and no beside it) and its
    /// `an_entity_beside_an_occluder_is_not_culled_while_one_behind_it_is`
    /// already proves `render_frame` wires them up. Neither proves the
    /// feature reaches the real runtime: the scene format has to be able to
    /// author an `Occluder`, `project.toml`'s `[render] occlusion_culling`
    /// has to reach the render path, and the saving has to be visible in
    /// `FrameStats`. That is what this test drives, through the same
    /// headless app (`build_test_app`) the E2E replays use, reading the
    /// real frame profiler via `get_frame_stats`.
    ///
    /// **The assertion that matters is the third one.** `occluded_count > 0`
    /// on its own would pass just as happily if every entity in the scene
    /// were wrongly culled -- which is precisely the failure mode occlusion
    /// culling must never have, because an over-cull is a visible rendering
    /// bug while an under-cull only costs a draw call. So the beside-wall
    /// entities are checked to still be drawn, by a floor on `draw_calls`.
    ///
    /// **Reading `draw_calls`.** It is a GPU-side count, not an entity
    /// count: with `fast_render` off, each surviving mesh entity is drawn
    /// once into the directional shadow map and once in the opaque pass, so
    /// the figure is roughly `2 * survivors` plus a fixed post-processing
    /// contribution that is identical between the two runs and cancels out
    /// of any comparison between them. The assertions below are written in
    /// those terms rather than against a hardcoded total.
    ///
    /// Measured when this test was written: culling on reports
    /// `occluded_count = 30`, `draw_calls = 11` (the wall plus the three
    /// beside entities drawn twice each, plus 3 post-processing draws);
    /// culling off reports `occluded_count = 0`, `draw_calls = 71`
    /// (34 x 2 + the same 3). Each of the three properties below was
    /// mutation-verified rather than assumed: making `box_occluded` return
    /// `true` unconditionally trips both the `<= hidden_count` bound and
    /// the `visible_floor` over-cull assertion (draw_calls collapses to 3,
    /// the post-processing floor), and ignoring `OcclusionCullingEnabled`
    /// in `render_frame` trips the phase-2 `occluded_count == 0` check.
    #[test]
    fn occlusion_culling_reduces_draw_calls_without_hiding_visible_objects() {
        let hidden_count = OCCLUSION_HIDDEN_XY.len();
        let beside_count = OCCLUSION_BESIDE_XY.len();
        // Wall + hidden + beside; the camera carries no mesh.
        let mesh_entity_count = 1 + hidden_count + beside_count;

        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write_occlusion_project(root, true);
        let project_dir = root.to_str().unwrap().to_string();

        // --- Phase 1: occlusion_culling = true ---
        let mut app = build_test_app(&project_dir, None, false);
        step_until_meshes_ready(&mut app, mesh_entity_count);

        // A reflected component whose RON does not match its type's shape
        // is skipped with a `tracing::warn!`, not a load failure -- so a
        // typo in the `components:` entry above would leave the wall with
        // no `Occluder` at all and this test would then be measuring an
        // occluder-free scene. Check it directly rather than inferring it
        // from the numbers.
        {
            let mut q = app.world_mut().query::<(
                &bsengine_scene::Name,
                &bsengine_render::components::Occluder,
            )>();
            let wall_occluder = q
                .iter(app.world())
                .find(|(n, _)| n.0 == "Wall")
                .map(|(_, o)| *o);
            let occ = wall_occluder.expect(
                "the Wall entity must carry the Occluder authored in its scene `components:` \
                 list -- an unparseable reflected component is only warned about, so its \
                 absence here means the scene's type path or RON value is wrong",
            );
            assert_eq!(*occ.half_extents, glam::Vec3::splat(0.49));
        }

        let on_stats = crate::test_query::run_query(app.world_mut(), "get_frame_stats", &json!({}))
            .expect("get_frame_stats should succeed once RenderPlugin has drawn a frame");
        let on_occluded = on_stats["occluded_count"]
            .as_u64()
            .expect("get_frame_stats should report occluded_count as a number");
        let on_draws = on_stats["draw_calls"]
            .as_u64()
            .expect("get_frame_stats should report draw_calls as a number");
        println!("occlusion ON : occluded_count={on_occluded} draw_calls={on_draws}");

        assert!(
            on_occluded > 0,
            "with a 24x16 wall standing between the camera and {hidden_count} cubes, the \
             frame profiler reported occluded_count=0 -- nothing was culled at all"
        );
        assert!(
            on_occluded <= hidden_count as u64,
            "occluded_count={on_occluded} exceeds the {hidden_count} entities that are \
             actually hidden, so something visible was culled"
        );
        assert!(
            on_draws < mesh_entity_count as u64,
            "occlusion culling must measurably cut the frame's work: draw_calls={on_draws} \
             is not even below the {mesh_entity_count} mesh entities in the scene, and each \
             drawn entity costs two draw calls (shadow + opaque) before post-processing is \
             counted at all"
        );
        // THE regression assertion. Each surviving entity contributes two
        // draw calls, so the {beside} entities beside the wall plus the
        // wall itself put a floor under `draw_calls` that an
        // over-culling implementation would fall straight through.
        let visible_floor = 2 * (beside_count as u64 + 1);
        assert!(
            on_draws >= visible_floor,
            "over-cull: draw_calls={on_draws} is below the {visible_floor} that the wall \
             and the {beside_count} entities standing clear of it must produce on their \
             own (two draw calls each: shadow pass + opaque pass). Something that was \
             plainly visible got culled, which is a rendering bug, not an optimization"
        );

        // Release phase 1's app and its GPU device before building the
        // second one, the same way `terrain_brush_edit_survives_a_reload`
        // above does.
        drop(app);

        // --- Phase 2: the same scene with occlusion_culling = false ---
        write_occlusion_project(root, false);
        let mut app = build_test_app(&project_dir, None, false);
        step_until_meshes_ready(&mut app, mesh_entity_count);

        let off_stats =
            crate::test_query::run_query(app.world_mut(), "get_frame_stats", &json!({}))
                .expect("get_frame_stats should succeed once RenderPlugin has drawn a frame");
        let off_occluded = off_stats["occluded_count"]
            .as_u64()
            .expect("get_frame_stats should report occluded_count as a number");
        let off_draws = off_stats["draw_calls"]
            .as_u64()
            .expect("get_frame_stats should report draw_calls as a number");
        println!("occlusion OFF: occluded_count={off_occluded} draw_calls={off_draws}");

        assert_eq!(
            off_occluded, 0,
            "`[render] occlusion_culling = false` in project.toml must reach the render \
             path: with the identical scene it still reported occluded_count={off_occluded}"
        );
        assert!(
            off_draws >= 2 * mesh_entity_count as u64,
            "with culling off every one of the {mesh_entity_count} mesh entities must be \
             drawn twice (shadow + opaque), i.e. at least {} draw calls, but the profiler \
             reported {off_draws} -- if the full count is not restored then phase 1's drop \
             was not attributable to occlusion",
            2 * mesh_entity_count
        );
        assert_eq!(
            off_draws - on_draws,
            2 * on_occluded,
            "the whole difference between the two runs should be exactly the culled \
             entities' own draw calls: {on_occluded} culled x 2 passes each. Got \
             off={off_draws}, on={on_draws}. Anything else means the toggle changed \
             something beyond occlusion, or the two runs did not render the same scene"
        );
    }

    /// The environment colour the IBL demo scene's skybox is painted, as
    /// authored in the image file.
    const IBL_SKY_RGB: [u8; 3] = [0, 255, 0];

    /// Writes a throwaway project holding one metallic sphere, optionally under
    /// a strongly coloured skybox.
    ///
    /// `[window]` is pinned small on purpose: the offscreen surface takes its
    /// size from the manifest, and this test only reads a patch in the middle.
    fn write_ibl_project(root: &std::path::Path, with_skybox: bool) {
        std::fs::create_dir_all(root.join("assets/scenes")).unwrap();
        std::fs::write(
            root.join("project.toml"),
            "[project]\nname = \"IBL E2E\"\n\
             entry_scene = \"assets/scenes/main.ron\"\n\
             [window]\nwidth = 200\nheight = 150\n",
        )
        .unwrap();

        // A flat, saturated green sky. Flat because this test asks whether the
        // environment reaches the surface at all, not how it is filtered --
        // `bsengine-rhi-wgpu`'s `pixels_ibl.rs` covers the structure of the
        // reflection with a sky that has something in it to blur. Green
        // because the clear colour and the ambient term are both grey, so
        // green cannot arrive here by accident.
        let [r, g, b] = IBL_SKY_RGB;
        let sky = image::RgbaImage::from_pixel(64, 32, image::Rgba([r, g, b, 255]));
        sky.save(root.join("assets/scenes/sky.png"))
            .expect("write the temp project's skybox image");

        // `Material` has no typed `EntityDescriptor` field -- `color:` writes
        // the albedo and nothing else -- so metallic and roughness are
        // authored through the generic reflected `components:` list, the same
        // way the occlusion test above authors `Occluder`. Every field is
        // spelled out, and `run_and_screenshot` checks the component actually
        // landed rather than inferring it: a reflected value that does not
        // match its type's shape is skipped with a warning, which would leave
        // the sphere a rough dielectric reflecting almost nothing, and this
        // test would then report that IBL does not work.
        //
        // The sun is switched off (`color: (0.0, 0.0, 0.0)`) so every photon
        // reaching the sphere came from the environment. With a white sun the
        // direct specular highlight alone would move these pixels further than
        // IBL does and the measurement would mean nothing.
        let skybox_line = if with_skybox {
            "\n    skybox: Some(\"sky.png\"),"
        } else {
            ""
        };
        std::fs::write(
            root.join("assets/scenes/main.ron"),
            format!(
                r#"SceneDescriptor(entities: [
    EntityDescriptor(
        name: "Camera",
        camera: true,
        transform: Some((position: (0.0, 0.0, 5.0))),
        look_at: Some((0.0, 0.0, 0.0)),
    ),
    EntityDescriptor(
        name: "Sun",
        directional_light: Some((
            direction: (0.0, -1.0, 0.0),
            color: (0.0, 0.0, 0.0),
            ambient: (0.2, 0.2, 0.2),
        )),
    ),
    EntityDescriptor(
        name: "Ball",
        primitive: Some(Sphere),
        transform: Some((position: (0.0, 0.0, 0.0), scale: (3.0, 3.0, 3.0))),
        components: [
            ("bsengine_core::material::Material", "(texture_id: None, metallic: 1.0, roughness: 0.0, emissive: (0.0, 0.0, 0.0), base_color: (1.0, 1.0, 1.0), opacity: 1.0)"),
        ],
    ),
],{skybox_line}
)"#
            ),
        )
        .unwrap();
    }

    /// Runs the temp project until it has drawn a settled frame, then returns
    /// that frame decoded from the `screenshot` query's base64 PNG.
    fn run_and_screenshot(project_dir: &str, expect_ibl: bool) -> image::RgbaImage {
        let mut app = build_test_app(project_dir, None, false);

        // The skybox loads asynchronously and the IBL maps are built when it
        // arrives, so "has the environment reached the renderer" has to be
        // waited on rather than assumed. Waiting on `has_ibl` rather than a
        // fixed frame count also gives the skyboxless run a real assertion:
        // the maps are still absent after as many frames as were enough to
        // build them.
        let mut ready = false;
        for _ in 0..300 {
            app.update();
            let has_ibl = app
                .world()
                .get_resource::<bsengine_rhi_wgpu::WgpuSurfaceResource>()
                .expect("WgpuRHIPlugin::offscreen should have inserted the surface")
                .0
                .has_ibl();
            let meshes = app
                .world_mut()
                .query::<&bsengine_render::components::MeshRenderer>()
                .iter(app.world())
                .count();
            if meshes >= 1 && has_ibl == expect_ibl {
                ready = true;
                break;
            }
        }
        assert!(
            ready,
            "within 300 frames the temp project never reached one MeshRenderer with \
             has_ibl() == {expect_ibl}: either the sphere primitive never resolved to a \
             mesh, or the skybox never loaded and no IBL maps were generated. Either way \
             nothing below would be measuring image-based lighting"
        );
        // A few more frames, so the sphere is drawn with the maps in place
        // rather than in the frame that built them.
        for _ in 0..3 {
            app.update();
        }

        {
            let mut q = app
                .world_mut()
                .query::<(&bsengine_scene::Name, &bsengine_core::Material)>();
            let material = q
                .iter(app.world())
                .find(|(n, _)| n.0 == "Ball")
                .map(|(_, m)| m.clone())
                .expect(
                    "Ball must carry the Material authored in its scene `components:` list -- \
                     an unparseable reflected component is only warned about, so its absence \
                     here means the type path or the RON value is wrong",
                );
            assert_eq!(material.metallic, 1.0, "Ball should be fully metallic");
            assert_eq!(material.roughness, 0.0, "Ball should be mirror-smooth");
        }

        let stats = crate::test_query::run_query(app.world_mut(), "get_frame_stats", &json!({}))
            .expect("get_frame_stats should succeed once RenderPlugin has drawn a frame");
        assert!(
            stats["triangles"].as_u64().unwrap_or(0) > 0,
            "no geometry was drawn at all, so the pixels below are not a sphere: {stats}"
        );

        let shot = crate::test_query::run_query(app.world_mut(), "screenshot", &json!({}))
            .expect("screenshot should succeed once RenderPlugin has drawn a frame");
        use base64::Engine;
        let png_bytes = base64::engine::general_purpose::STANDARD
            .decode(
                shot["data_base64"]
                    .as_str()
                    .expect("screenshot should return a data_base64 string"),
            )
            .expect("data_base64 should be valid base64");
        image::load_from_memory(&png_bytes)
            .expect("the screenshot PNG should decode")
            .to_rgba8()
    }

    /// Mean colour of the disc of `radius` pixels around the frame's centre.
    ///
    /// A patch rather than the single centre pixel, for two reasons found by
    /// looking at the frames rather than guessing at them. The sphere's exact
    /// centre carries a small dark shading artifact -- present in the
    /// skyboxless run too, where the shader takes the pre-IBL path, so it
    /// predates this work and is not IBL's doing -- and the outermost ring of
    /// the silhouette sits against the background. Radius 22 of a silhouette
    /// that reaches past 30 is all surface, and averaging over it makes the
    /// measurement independent of exactly where the sphere lands.
    fn disc_mean(image: &image::RgbaImage, radius: f32) -> [f32; 3] {
        let (cx, cy) = (image.width() as f32 / 2.0, image.height() as f32 / 2.0);
        let mut sum = [0.0f32; 3];
        let mut count = 0.0f32;
        for y in 0..image.height() {
            for x in 0..image.width() {
                let dx = x as f32 - cx;
                let dy = y as f32 - cy;
                if dx * dx + dy * dy <= radius * radius {
                    let px = image.get_pixel(x, y).0;
                    for c in 0..3 {
                        sum[c] += px[c] as f32;
                    }
                    count += 1.0;
                }
            }
        }
        [sum[0] / count, sum[1] / count, sum[2] / count]
    }

    /// Sum of the per-channel gaps between a measured colour and a target.
    ///
    /// Per-channel rather than luma: a frame that merely got brighter closes
    /// the luma gap to a bright environment while taking on none of its hue.
    fn gap_to(colour: [f32; 3], target: [f32; 3]) -> f32 {
        (0..3).map(|i| (colour[i] - target[i]).abs()).sum()
    }

    /// Completion condition 4: the same demo scene, rendered with and without
    /// a skybox and compared as screenshots, must show image-based lighting on
    /// a metallic surface.
    ///
    /// Both runs are captured through the `screenshot` query and its base64
    /// PNG is decoded here -- the round trip
    /// `screenshot_returns_a_decodable_png` above establishes -- so this is
    /// literally the on/off screenshot comparison rather than a proxy for one.
    ///
    /// The assertions are written so that a change alone cannot pass them. An
    /// IBL that simply brightened every frame would raise all three channels;
    /// what is required here is that red and blue *fall* to nothing while
    /// green climbs, which only reflecting a green environment can do.
    #[test]
    fn ibl_visibly_changes_a_metallic_surface_under_a_skybox() {
        const SPHERE_RADIUS_PX: f32 = 22.0;

        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let project_dir = root.to_str().unwrap().to_string();

        write_ibl_project(root, true);
        let with_sky = run_and_screenshot(&project_dir, true);

        // The second app builds its own GPU device, so the first one has to be
        // gone by now: `run_and_screenshot` drops its `App` on return, the
        // same ordering `occlusion_culling_reduces_draw_calls_without_hiding_
        // visible_objects` above keeps between its two phases.
        write_ibl_project(root, false);
        let without_sky = run_and_screenshot(&project_dir, false);

        let lit = disc_mean(&with_sky, SPHERE_RADIUS_PX);
        let unlit = disc_mean(&without_sky, SPHERE_RADIUS_PX);
        // The environment as the renderer actually displays it, read out of
        // the skybox run's own corner rather than assumed from the image file:
        // the sky goes through the same HDR buffer and output encoding the
        // sphere does, and a mirror can only ever match what that produces.
        let env = {
            let px = with_sky.get_pixel(2, 2).0;
            [px[0] as f32, px[1] as f32, px[2] as f32]
        };
        let background = {
            let px = without_sky.get_pixel(2, 2).0;
            [px[0] as f32, px[1] as f32, px[2] as f32]
        };
        println!(
            "IBL on: sphere {lit:?} sky {env:?} | IBL off: sphere {unlit:?} \
             background {background:?}"
        );

        // Before anything is claimed about the sphere's colour, establish that
        // these pixels are the sphere. Were they not, the skybox run would be
        // measuring the green sky itself and would pass whatever the shader
        // did. The scene, camera and geometry are identical between the runs,
        // so proving it in the skyboxless one -- where the background is the
        // grey clear colour rather than the sky -- proves it in both.
        assert!(
            gap_to(unlit, background) > 30.0,
            "the measured patch is barely distinguishable from the frame's corner \
             ({unlit:?} against {background:?}), so the sphere is not covering the middle \
             of the frame and neither measurement below is of a surface at all"
        );

        // Each channel individually, so a uniform brightening cannot pass it:
        // that would raise red and blue as well as green.
        assert!(
            lit[0] <= unlit[0] && lit[1] >= unlit[1] && lit[2] <= unlit[2],
            "every channel of the metallic sphere should move towards the green \
             environment: {unlit:?} without the skybox, {lit:?} with it"
        );

        let near = gap_to(lit, env);
        let far = gap_to(unlit, env);
        assert!(
            near * 2.0 < far,
            "the sphere should end up far closer to the environment's colour under the \
             skybox: per-channel gap {far} without it ({unlit:?}), {near} with it \
             ({lit:?}), against an environment of {env:?}"
        );

        // The strongest form of the claim, and the one a brightening cannot
        // imitate: the off-environment channels collapse rather than merely
        // holding still, while the environment's own channel climbs.
        assert!(
            lit[0] < 20.0 && lit[2] < 20.0,
            "a mirror under a pure green sky should reflect no red and no blue: {lit:?}"
        );
        assert!(
            lit[1] > unlit[1] + 30.0,
            "the green the sphere reflects should be well above the grey it shows with no \
             environment: {} against {}",
            lit[1],
            unlit[1]
        );
    }

    /// Roadmap item 51's demo, and the completion condition it exists to
    /// satisfy: `games/mini-arena/assets/scenes/joint-chain.ron` -- five
    /// bodies, the top one fixed, linked by four **spherical** joints --
    /// swings under gravity without coming apart, driven through the real
    /// runtime stack (`build_test_app`, the same plugin list `--test`/E2E
    /// replays and `main.rs`'s windowed runtime use).
    ///
    /// A chain rather than a door hinge, deliberately. `bsengine-physics`'
    /// own `a_spherical_joint_holds_the_anchor_distance_while_rotation_stays_
    /// free` already proves *one* spherical joint in a hand-built world; what
    /// no unit test here covers is several of them **in series**, where each
    /// joint's correction perturbs its neighbour and the solver has to keep
    /// all four satisfied at once. That is precisely the behaviour item 52's
    /// ragdoll is assembled from, so it is the thing worth proving now.
    ///
    /// **Both halves of the assertion matter, and each covers the other's
    /// blind spot.**
    /// - Every consecutive pair's two joint anchors stay coincident -> the
    ///   chain held. On its own this passes on a chain that never moved at
    ///   all -- a frozen assembly satisfies every constraint perfectly.
    /// - The free end actually travelled -> it swung. On its own this passes
    ///   on a chain that flew apart, since five bodies in free fall move a
    ///   very long way.
    ///
    /// The measured quantity for "the chain held" is the **anchor
    /// separation**, `|(p_lower + R_lower*anchor_a) - (p_upper +
    /// R_upper*anchor_b)|`, which is the constraint a spherical joint
    /// actually enforces and which the solver drives to zero. Body-centre
    /// distance would have been the weaker choice: a spherical joint only
    /// bounds it *above* (by `|anchor_a| + |anchor_b|` = 1.0, since either
    /// body may rotate about the shared anchor), so "centres within 1.0"
    /// would also be satisfied by a fully folded chain and says nothing about
    /// how well the constraint is being met.
    ///
    /// Both quantities are tracked across every frame rather than sampled
    /// once at the end. For the separations that is strictly stronger -- a
    /// mid-run blow-up that the solver later pulls back would be invisible in
    /// a final-frame reading. For the travel it is also *necessary*: a
    /// pendulum returns to where it started, so a final-frame displacement
    /// could legitimately be near zero on a chain that swung through a wide
    /// arc, and the maximum displacement is the honest measure of "it moved".
    ///
    /// A **third** check -- that `ChainAnchor` never moves -- turned out to be
    /// load-bearing rather than decorative, and mutation testing is what
    /// showed it. Flipping the anchor to `Dynamic` makes both halves above
    /// pass *more* convincingly than the real scene does: every separation
    /// reads exactly 0.0 (five bodies falling together satisfy every
    /// constraint perfectly) and the free end "travels" 219 units. Only the
    /// anchor's own immobility distinguishes a chain that swung from one that
    /// simply fell. The other two mutations behave as the design intends:
    /// dropping `sync_joints` from `PhysicsPlugin`'s schedule fails the
    /// `has_joint` check with a 219-unit separation, and swizzling one
    /// scene anchor's sign fails the separation check at 1.03 units -- which
    /// the constants above are restated for, since reading them back out of
    /// the loaded components would have made the check follow the mutation.
    #[test]
    fn a_spherical_joint_chain_swings_without_coming_apart() {
        use bsengine_physics::PhysicsWorld;
        use glam::{Quat, Vec3};

        // Top-down order: `ChainAnchor` is the fixed body, `ChainLink4` the
        // free end. Consecutive entries are exactly the jointed pairs.
        const CHAIN: [&str; 5] = [
            "ChainAnchor",
            "ChainLink1",
            "ChainLink2",
            "ChainLink3",
            "ChainLink4",
        ];
        // The scene's own anchor offsets. Restated here rather than read back
        // out of the loaded `Joint` components on purpose: taking them from
        // the thing under test would make this check agree with whatever the
        // loader produced -- including with anchors it never applied at all.
        const ANCHOR_A: Vec3 = Vec3::new(-0.5, 0.0, 0.0); // on the lower link
        const ANCHOR_B: Vec3 = Vec3::new(0.5, 0.0, 0.0); // on the body above it
        const FRAMES: usize = 400; // ~6.7s at the fixed 1/60 physics step

        let project_dir = format!("{}/../../games/mini-arena", env!("CARGO_MANIFEST_DIR"));
        let mut app = build_test_app(&project_dir, Some("assets/scenes/joint-chain.ron"), false);

        // Spawns the scene's entities. The joints do not exist yet after this
        // one frame -- `sync_joints` can only create a joint once
        // `spawn_bodies` has registered *both* ends with Rapier, which is why
        // it retries every frame; the `has_joint` assertions below are what
        // confirm it got there.
        app.update();

        let ids: Vec<Entity> = CHAIN
            .iter()
            .map(|name| {
                let mut q = app.world_mut().query::<(Entity, &bsengine_scene::Name)>();
                q.iter(app.world())
                    .find(|(_, n)| n.0 == *name)
                    .map(|(e, _)| e)
                    .unwrap_or_else(|| panic!("{name} should have spawned from joint-chain.ron"))
            })
            .collect();

        let pose = |app: &App, entity: Entity| -> (Vec3, Quat) {
            let t = app
                .world()
                .get::<bsengine_core::Transform>(entity)
                .expect("every chain body carries a Transform");
            (t.position.0, t.rotation.0)
        };
        // The world-space point each joint pins together, from the two bodies'
        // own transforms.
        let separation = |app: &App, upper: Entity, lower: Entity| -> f32 {
            let (p_upper, r_upper) = pose(app, upper);
            let (p_lower, r_lower) = pose(app, lower);
            ((p_lower + r_lower * ANCHOR_A) - (p_upper + r_upper * ANCHOR_B)).length()
        };

        let free_end = *ids.last().expect("CHAIN is not empty");
        let start_free_end = pose(&app, free_end).0;
        let start_anchor = pose(&app, ids[0]).0;

        let mut worst_separation = vec![0.0f32; CHAIN.len() - 1];
        let mut max_travel = 0.0f32;
        for _ in 0..FRAMES {
            app.update();
            for (i, pair) in ids.windows(2).enumerate() {
                worst_separation[i] = worst_separation[i].max(separation(&app, pair[0], pair[1]));
            }
            max_travel = max_travel.max((pose(&app, free_end).0 - start_free_end).length());
        }

        let final_separation: Vec<f32> = ids
            .windows(2)
            .map(|pair| separation(&app, pair[0], pair[1]))
            .collect();
        let end_free_end = pose(&app, free_end).0;
        let final_travel = (end_free_end - start_free_end).length();
        let anchor_drift = (pose(&app, ids[0]).0 - start_anchor).length();

        // Printed, not merely asserted on: the point of this test is the
        // measured behaviour of four joints in series, and a pass that only
        // says "under the tolerance" hides how much room there actually was.
        println!(
            "spherical-joint chain, {FRAMES} frames:\n  \
             final anchor separations (0 = constraint exactly satisfied): {final_separation:?}\n  \
             worst separation seen on any frame:                          {worst_separation:?}\n  \
             free end {start_free_end} -> {end_free_end}: travelled {final_travel} by the last \
             frame, {max_travel} at its furthest\n  \
             fixed anchor drift: {anchor_drift}"
        );

        // Sanity first, and the thing to check before ever touching a
        // tolerance below: all four joints reached the simulation at all, and
        // the top body really is immovable. A chain that came apart is a real
        // failure, and these two say whether to look for the cause here.
        let world = app.world().resource::<PhysicsWorld>();
        for (i, pair) in ids.windows(2).enumerate() {
            assert!(
                world.has_joint(pair[0], pair[1]),
                "joint {i} ({} -> {}) never reached the simulation -- the scene's `joint:` \
                 field did not resolve, or `sync_joints` never saw both bodies registered",
                CHAIN[i + 1],
                CHAIN[i],
            );
        }
        assert!(
            anchor_drift < 1e-4,
            "ChainAnchor is `rigidbody: Some(Static)` and must not move at all, but it \
             drifted {anchor_drift} units -- with the top of the chain falling, everything \
             below it is in free fall together and no separation reading below means \
             anything"
        );

        // Half one: the chain held. 0.05 is 5% of the 1.0 link spacing -- a
        // residual the solver leaves behind, not a chain that has pulled
        // apart, which would grow without bound as the links fell away from
        // each other. Measured on this scene: the worst frame of the four
        // joints reads 0.030/0.019/0.020/0.003, all of them on the opening
        // frames where a chain released horizontally is under its heaviest
        // load, settling to ~0.002 or below by the last frame.
        for (i, worst) in worst_separation.iter().enumerate() {
            assert!(
                *worst < 0.05,
                "joint {i} ({} -> {}) let its anchors drift {worst} units apart at their \
                 worst (final separations {final_separation:?}) -- a spherical joint pins \
                 those two points together, so anything approaching the 1.0 link spacing \
                 means the chain came apart. Do not widen this tolerance: check that the \
                 joints exist (asserted above), that the scene's anchors still sum to the \
                 1.0 spacing between consecutive links, and that ChainAnchor is still Static",
                CHAIN[i + 1],
                CHAIN[i],
            );
        }

        // Half two: it swung. Without this, a chain frozen solid at its
        // starting pose -- or one whose bodies never got a Rapier body at all
        // and so never moved -- passes every separation check above perfectly.
        // The chain starts horizontal, so a link that swings to hanging moves
        // on the order of its own distance from the anchor; 1.0 is well under
        // that and well over any settling jitter.
        assert!(
            max_travel > 1.0,
            "the free end (ChainLink4) never got further than {max_travel} units from where \
             it started ({start_free_end}) in {FRAMES} frames -- the chain starts horizontal \
             along +X with nothing under it, so under gravity it must swing. A stationary \
             chain satisfies every joint perfectly and proves nothing"
        );
    }

    /// Roadmap item 52 sub-step 2/2 demo: proves that firing the "die" trigger
    /// on mini-arena's Player activates its `Ragdoll` component, handing the
    /// skeleton to physics rather than the animation clip.
    ///
    /// Two assertions, one in each direction, so neither a "ragdoll everything
    /// at startup" implementation nor a "never activate ragdoll" implementation
    /// can pass both:
    ///
    /// * Before the trigger: `Ragdoll.active` is **false** and the ASM is in
    ///   `"locomotion"`. A broken implementation that activates every entity's
    ///   ragdoll unconditionally fails here.
    /// * After the trigger: `Ragdoll.active` is **true** and the ASM is in
    ///   `"death"`. A no-op implementation where the ASM→Ragdoll connection is
    ///   never wired fails here.
    ///
    /// The values are printed regardless so a failure message names what was
    /// actually observed — not just "expected true, got false".
    #[test]
    fn a_character_collapses_when_its_death_trigger_fires() {
        let project_dir = format!("{}/../../games/mini-arena", env!("CARGO_MANIFEST_DIR"));
        let mut app = build_test_app(&project_dir, None, false);

        // The ASM system queries (AnimationStateMachine, AnimationPlayer) —
        // AnimationPlayer is spawned by GltfPlugin once fox.glb finishes
        // loading. We must wait for that before firing the trigger, or the
        // system skips the entity entirely (the query simply excludes it).
        // The same approach as `mini_arenas_fox_mesh_loads_now_that_rendering_is_on`.
        let mut fox_loaded = false;
        for _ in 0..200 {
            app.update();
            let status =
                crate::test_query::get_asset_status(app.world_mut(), "assets/models/fox.glb");
            if status == json!("loaded") {
                fox_loaded = true;
                break;
            }
        }
        assert!(
            fox_loaded,
            "fox.glb must load before the ragdoll test can run — without \
             AnimationPlayer (spawned by GltfPlugin on load) the ASM system \
             skips the entity and the trigger never fires"
        );

        // One more frame so GltfPlugin's just-spawned AnimationPlayer is
        // visible to the ASM system in the next update.
        app.update();

        // Locate the Player entity by name.
        let player = {
            let mut q = app.world_mut().query::<(&bsengine_scene::Name, Entity)>();
            q.iter(app.world())
                .find(|(n, _)| n.0 == "Player")
                .map(|(_, e)| e)
                .expect("Player must exist after scene load")
        };

        // Diagnostic: verify AnimationPlayer exists on the entity (required by
        // the ASM system) and transitions deserialized correctly.
        let has_anim_player = app
            .world()
            .get::<bsengine_core::AnimationPlayer>(player)
            .is_some();
        let transitions_len = app
            .world()
            .get::<bsengine_core::AnimationStateMachine>(player)
            .map(|a| a.transitions.len())
            .unwrap_or(0);
        println!(
            "diagnostic: has_AnimationPlayer={has_anim_player}, \
             transitions.len={transitions_len}"
        );
        assert!(
            has_anim_player,
            "Player must have an AnimationPlayer for the ASM system to run — \
             fox.glb loaded but GltfPlugin may not have finished spawning it"
        );
        assert!(
            transitions_len > 0,
            "Player's ASM must have at least one transition after scene load — \
             transitions.len={transitions_len} means AsmTransition/TransitionCondition \
             are still not registered in register_gameplay_reflect_types"
        );

        // --- Control: before the trigger the character is animating normally ---
        let active_before = app
            .world()
            .get::<bsengine_physics::Ragdoll>(player)
            .expect(
                "Player must have a Ragdoll component — was it added to \
                 games/mini-arena/assets/scenes/main.ron?",
            )
            .active;
        let state_before = app
            .world()
            .get::<bsengine_core::AnimationStateMachine>(player)
            .expect("Player must have an AnimationStateMachine")
            .current_state
            .clone();

        println!(
            "before 'die' trigger: Ragdoll.active={active_before}, \
             asm.current_state={state_before:?}"
        );

        assert!(
            !active_before,
            "before firing 'die': Ragdoll.active must be false — a broken \
             implementation that activates every ragdoll at startup would fail \
             here; asm.current_state={state_before:?}"
        );
        assert_eq!(
            state_before, "locomotion",
            "before firing 'die': ASM must be in 'locomotion'; \
             Ragdoll.active={active_before}"
        );

        // --- Fire the death trigger directly against the ECS ---
        app.world_mut()
            .get_mut::<bsengine_core::AnimationStateMachine>(player)
            .expect("Player must have an AnimationStateMachine")
            .set_trigger("die");

        // One frame for the ASM system to process the transition and the
        // ragdoll-activation system to run.
        app.update();

        let active_after = app
            .world()
            .get::<bsengine_physics::Ragdoll>(player)
            .map(|r| r.active)
            .unwrap_or(false);
        let state_after = app
            .world()
            .get::<bsengine_core::AnimationStateMachine>(player)
            .map(|a| a.current_state.clone())
            .unwrap_or_default();

        println!(
            "after 'die' trigger: Ragdoll.active={active_after}, \
             asm.current_state={state_after:?}"
        );

        assert!(
            active_after,
            "after 'die' trigger: Ragdoll.active must be true — the ASM→Ragdoll \
             integration in animation_state_machine.rs must set active=true when \
             entering a state with ragdoll:true; a no-op integration fails here; \
             asm.current_state={state_after:?}"
        );
        assert_eq!(
            state_after, "death",
            "after 'die' trigger: ASM must be in 'death' state; \
             Ragdoll.active={active_after}"
        );

        // The flag being set is not the demo. The demo is the character
        // actually collapsing, and this feature family's characteristic
        // failure is that everything upstream looks right -- flag set, bodies
        // built and falling -- while the skinning still reads the clip, so the
        // character animates on as if nothing happened. `pose_override` is the
        // channel physics hands the skeleton over through, so its going from
        // empty to non-empty is the collapse becoming visible.
        let override_before = app
            .world()
            .get::<bsengine_gltf::SkinnedMesh>(player)
            .map(|s| s.pose_override.len())
            .unwrap_or(0);
        for _ in 0..10 {
            app.update();
        }
        let override_after = app
            .world()
            .get::<bsengine_gltf::SkinnedMesh>(player)
            .map(|s| s.pose_override.len())
            .unwrap_or(0);

        println!(
            "pose_override length: {override_before} before the trigger, \
             {override_after} ten frames after"
        );
        assert_eq!(
            override_before, 0,
            "while alive the clip is the sole source of the pose"
        );
        assert!(
            override_after > 0,
            "ten frames after the trigger physics must be driving the \
             skeleton, but pose_override is still empty -- the ragdoll is \
             switched on and simulating underneath a character that is still \
             playing its animation"
        );
    }

    #[test]
    fn the_demo_car_drives_when_its_throttle_key_is_held() {
        // Drives the whole real path end-to-end: a held key reaches
        // `drive.js`, which calls `Bsengine.vehicle.setThrottle`, which queues
        // a `SetVehicleInput`, which writes the `Vehicle` component, which
        // `sync_vehicles` feeds to the Rapier controller, whose wheel rays have
        // to reach the ground. No crate-level test covers that chain.
        //
        // Pressing the key rather than writing `Vehicle.throttle` directly is
        // load-bearing, not stylistic. The first version of this test set the
        // field and measured 0.23 m: `drive.js` runs every frame and set the
        // throttle straight back to 0.0, because no key was held. Writing the
        // component fights the script that a real player drives through.
        let run = |hold_throttle: bool| -> f32 {
            let project_dir = format!("{}/../../games/vehicle-demo", env!("CARGO_MANIFEST_DIR"));
            let mut app = build_test_app(&project_dir, None, false);
            let mut frame: u64 = 0;

            let car = |app: &mut App| {
                let mut q = app
                    .world_mut()
                    .query::<(&bsengine_scene::Name, bevy_ecs::prelude::Entity)>();
                q.iter(app.world())
                    .find(|(n, _)| n.0 == "Car")
                    .map(|(_, e)| e)
                    .expect("the demo scene must contain a Car")
            };

            // Settle onto the suspension first, so what is measured below is
            // driving rather than the initial drop.
            execute_command(&mut app, &mut frame, Command::Step { frames: 30 });

            let e = car(&mut app);
            // An unregistered element type yields an empty `Vec` rather than an
            // error, and a car with no wheels reads as "the throttle does not
            // work" instead of as a load failure. Worth separating.
            let wheels = app
                .world()
                .get::<bsengine_physics::Vehicle>(e)
                .expect("the Car must have a Vehicle component")
                .wheels
                .len();
            assert_eq!(wheels, 4, "all four authored wheels must survive the load");

            if hold_throttle {
                let (resp, _) = execute_command(
                    &mut app,
                    &mut frame,
                    Command::PressKey {
                        key: "W".to_string(),
                    },
                );
                assert!(resp.ok, "PressKey should succeed: {:?}", resp.error);
            }

            let start = app
                .world()
                .get::<bsengine_physics::PhysicsTransform>(e)
                .unwrap()
                .position
                .0;
            execute_command(&mut app, &mut frame, Command::Step { frames: 120 });
            let end = app
                .world()
                .get::<bsengine_physics::PhysicsTransform>(e)
                .unwrap()
                .position
                .0;
            (end - start).length()
        };

        let driven = run(true);
        let coasted = run(false);
        println!("demo car: {driven} m holding W, {coasted} m without");
        assert!(
            driven > 1.0,
            "the demo car must cover real ground with the throttle key held; \
             it moved {driven} m, which is settling, not driving"
        );
        assert!(
            coasted < driven * 0.25,
            "the distance must come from the throttle: with no key held the \
             car moved {coasted} m against {driven} m driven"
        );
    }

    #[test]
    fn the_headless_host_resolves_a_cylinder_to_a_real_mesh() {
        // THE test for the split-dispatch hazard.
        //
        // `Primitive` is mapped to a mesh in THREE separate places -- the
        // windowed game (`bsengine-app/src/main.rs`), the editor app
        // (`apps/bsengine-editor-app/src/main.rs`), and the headless test host
        // (`bsengine-runtime/src/scene_systems.rs`). They are independent
        // implementations of the same mapping. Adding a variant to some but not
        // all means the primitive renders in one host and silently gets no mesh
        // in another -- and this host is the one every E2E test runs in, so a
        // gap here makes the wheels invisible to exactly the tests meant to
        // prove they work.
        //
        // The compiler catches a missing match arm, but only for hosts that are
        // actually built; this asserts the behaviour rather than trusting that.
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("assets/scenes")).unwrap();
        std::fs::write(
            root.join("project.toml"),
            "[project]\nname = \"Cylinder\"\nentry_scene = \"assets/scenes/main.ron\"\n",
        )
        .unwrap();
        std::fs::write(
            root.join("assets/scenes/main.ron"),
            r#"SceneDescriptor(entities: [
                EntityDescriptor(name: "Camera", camera: true, transform: Some((position: (0.0, 0.0, 5.0)))),
                EntityDescriptor(name: "Wheel", primitive: Some(Cylinder), transform: Some((position: (0.0, 0.0, 0.0)))),
            ])"#,
        )
        .unwrap();

        let mut app = build_test_app(root.to_str().unwrap(), None, false);
        app.update();

        let wheel = {
            let mut q = app
                .world_mut()
                .query::<(&bsengine_scene::Name, bevy_ecs::prelude::Entity)>();
            q.iter(app.world())
                .find(|(n, _)| n.0 == "Wheel")
                .map(|(_, e)| e)
                .expect("the Cylinder entity must spawn")
        };
        let mesh = app.world().get::<bsengine_render::MeshRenderer>(wheel);
        assert!(
            mesh.is_some(),
            "a Cylinder must resolve to a mesh in the HEADLESS dispatch. If this \
             fails while the game renders cylinders fine, `Primitive::Cylinder` \
             was added to one host's primitive->mesh match and not this one."
        );
        assert!(
            mesh.unwrap().mesh_id != 0,
            "the mesh id must be a real registered id, not the zero placeholder"
        );
    }

    #[test]
    fn the_demo_cars_wheels_follow_its_suspension_over_terrain() {
        // The visible half of the feature, end to end, and the reason the demo
        // moved off flat ground: on a heightfield the four wheels sit at
        // genuinely different heights, so the suspension visibly does its job.
        // On a flat floor all four settle identically and this assertion could
        // not exist.
        //
        // It also exercises the whole chain the crate-level tests cannot span
        // together: the scene's `Cylinder` primitives resolve in the headless
        // dispatch, `WheelIndex` survives reflection, the wheel raycasts reach
        // a Rapier heightfield rather than a box, and `sync_wheel_transforms`
        // poses the children.
        let project_dir = format!("{}/../../games/vehicle-demo", env!("CARGO_MANIFEST_DIR"));
        let mut app = build_test_app(&project_dir, None, false);
        let mut frame: u64 = 0;

        // Drop onto the terrain and drive a little, so the car is somewhere
        // sloped rather than wherever it spawned.
        execute_command(&mut app, &mut frame, Command::Step { frames: 60 });
        let (resp, _) = execute_command(
            &mut app,
            &mut frame,
            Command::PressKey {
                key: "W".to_string(),
            },
        );
        assert!(resp.ok, "PressKey should succeed: {:?}", resp.error);
        execute_command(&mut app, &mut frame, Command::Step { frames: 120 });

        let names = ["WheelFL", "WheelFR", "WheelRL", "WheelRR"];
        let mut heights = Vec::new();
        let mut meshed = 0;
        for name in names {
            let e = {
                let mut q = app
                    .world_mut()
                    .query::<(&bsengine_scene::Name, bevy_ecs::prelude::Entity)>();
                q.iter(app.world())
                    .find(|(n, _)| n.0 == name)
                    .map(|(_, e)| e)
                    .unwrap_or_else(|| panic!("the demo scene must contain {name}"))
            };
            if app
                .world()
                .get::<bsengine_render::MeshRenderer>(e)
                .is_some()
            {
                meshed += 1;
            }
            heights.push(
                app.world()
                    .get::<bsengine_core::Transform>(e)
                    .expect("a wheel visual keeps its transform")
                    .position
                    .0
                    .y,
            );
        }

        println!("wheel local heights on terrain: {heights:?}");
        assert_eq!(
            meshed, 4,
            "all four wheels must have resolved a Cylinder mesh; a count below \
             four means the headless primitive dispatch does not know Cylinder"
        );

        let max = heights.iter().cloned().fold(f32::MIN, f32::max);
        let min = heights.iter().cloned().fold(f32::MAX, f32::min);
        let spread = max - min;
        println!("wheel height spread {spread} m");
        assert!(
            spread > 0.001,
            "on terrain the wheels must sit at different heights -- that is the \
             suspension working. All four at {min} means they are pinned to \
             their mounts and nothing is driving them"
        );

        // Paired: they must still hang BELOW their mounts, not float. Every
        // mount is authored at local y = -0.2, and the suspension can only push
        // the wheel further down from there.
        assert!(
            max < -0.2,
            "a wheel must hang below its mount at y = -0.2; the highest is at \
             {max}, which is above the chassis mount point"
        );
    }

    #[test]
    fn the_demo_foxs_feet_are_planted_on_the_surface_below_them() {
        // The whole feature end to end, through the real headless app: the
        // scene's `IkChains` must survive reflection, skinning must publish the
        // tip positions, the physics probe must find ground under each foot,
        // and the solver must put the feet there.
        //
        // No crate-level test spans that chain -- each one covers a link.
        let project_dir = format!("{}/../../games/ik-demo", env!("CARGO_MANIFEST_DIR"));
        let mut app = build_test_app(&project_dir, None, false);
        let mut frame: u64 = 0;

        // Let the glTF load and the probe run for a few frames. The probe reads
        // the tip positions skinning published the frame before, so it needs
        // more than one.
        for _ in 0..90 {
            execute_command(&mut app, &mut frame, Command::Step { frames: 1 });
        }

        let fox = {
            let mut q = app
                .world_mut()
                .query::<(&bsengine_scene::Name, bevy_ecs::prelude::Entity)>();
            q.iter(app.world())
                .find(|(n, _)| n.0 == "Fox")
                .map(|(_, e)| e)
                .expect("the demo scene must contain a Fox")
        };

        // The chains have to have survived scene deserialization at all. An
        // unregistered component is silently ABSENT rather than an error, and a
        // fox with no chains simply never reaches for the ground -- which reads
        // as a broken solver rather than a missing registration.
        let chains = app
            .world()
            .get::<bsengine_gltf::IkChains>(fox)
            .expect(
                "the Fox must have IkChains -- if this is absent, IkChains is \
                 not registered for reflection",
            )
            .chains
            .clone();
        assert_eq!(chains.len(), 4, "all four authored chains must survive");

        // Skinning must have published a tip position per chain, or the probe
        // had nothing to cast from.
        let tips = app
            .world()
            .get::<bsengine_gltf::SkinnedMesh>(fox)
            .expect("the Fox must have a SkinnedMesh once fox.glb loads")
            .ik_tip_positions
            .clone();
        assert_eq!(
            tips.len(),
            4,
            "skinning must publish one tip position per chain; got {}",
            tips.len()
        );

        // And the probe must have written real targets. They start at the
        // origin, so a target still there means the probe never found ground.
        let targets: Vec<glam::Vec3> = chains.iter().map(|c| c.target.0).collect();
        println!("foot targets: {targets:?}");
        println!("foot tips:    {tips:?}");
        assert!(
            targets.iter().all(|t| *t != glam::Vec3::ZERO),
            "every chain's target must have been written by the ground probe; \
             a target still at the origin means no ground was found beneath \
             that foot. Targets: {targets:?}"
        );

        // Each foot must be near the surface the probe found for it. This is
        // the assertion that fails if the solver runs but does not reach.
        for (i, (tip, target)) in tips.iter().zip(&targets).enumerate() {
            let err = (*tip - *target).length();
            assert!(
                err < 0.25,
                "foot {i} should be planted on its target: tip {tip:?} is \
                 {err} m from target {target:?}"
            );
        }
    }

    #[test]
    fn the_demo_foxs_feet_sit_at_different_heights_across_the_steps() {
        // The discontinuity case, and the reason the demo has stairs as well as
        // a slope. The fox straddles the seam between two treads 0.25 m apart,
        // so its feet must resolve to two distinct heights -- not a smoothed
        // fraction of the step, and not one height for the whole character.
        //
        // A slope alone cannot make this assertion sharp: a correction that
        // lags or averages still lands somewhere plausible on a continuous
        // surface.
        let project_dir = format!("{}/../../games/ik-demo", env!("CARGO_MANIFEST_DIR"));
        let mut app = build_test_app(&project_dir, None, false);
        let mut frame: u64 = 0;
        for _ in 0..90 {
            execute_command(&mut app, &mut frame, Command::Step { frames: 1 });
        }

        let fox = {
            let mut q = app
                .world_mut()
                .query::<(&bsengine_scene::Name, bevy_ecs::prelude::Entity)>();
            q.iter(app.world())
                .find(|(n, _)| n.0 == "Fox")
                .map(|(_, e)| e)
                .expect("the demo scene must contain a Fox")
        };
        let heights: Vec<f32> = app
            .world()
            .get::<bsengine_gltf::IkChains>(fox)
            .expect("the Fox must have IkChains")
            .chains
            .iter()
            .map(|c| c.target.0.y)
            .collect();

        // Every target has to have been written first. The first version of
        // this test asserted only on the spread and PASSED at 4.04 m -- with
        // three targets still at the origin and one written. A spread computed
        // over unwritten zeros measures the bug, not the feature.
        let targets: Vec<glam::Vec3> = app
            .world()
            .get::<bsengine_gltf::IkChains>(fox)
            .expect("the Fox must have IkChains")
            .chains
            .iter()
            .map(|c| c.target.0)
            .collect();
        assert!(
            targets.iter().all(|t| *t != glam::Vec3::ZERO),
            "every foot must have a probed target before their heights mean \
             anything: {targets:?}"
        );

        let max = heights.iter().cloned().fold(f32::MIN, f32::max);
        let min = heights.iter().cloned().fold(f32::MAX, f32::min);
        let spread = max - min;
        println!("foot target heights {heights:?}, spread {spread} m");
        assert!(
            spread > 0.1,
            "straddling a 0.25 m step, the feet must resolve to different \
             heights; they span only {spread} m ({heights:?}). One height for \
             every foot means a single offset is being applied to the whole \
             character rather than a target per foot."
        );
    }

    #[test]
    fn the_demo_target_rig_follows_the_fox() {
        // End to end through the real headless app: the mapping survives
        // reflection, `bsengine-scene` resolves the source name to an entity,
        // skinning publishes the source's locals, and the target's MAPPED bones
        // move with the source while its UNMAPPED bones do not.
        //
        // Both halves are asserted. "The target moved" alone is satisfied by a
        // character simply playing its own clip, which this one also is.
        let project_dir = format!("{}/../../games/ik-demo", env!("CARGO_MANIFEST_DIR"));
        let mut app = build_test_app(&project_dir, None, false);
        let mut frame: u64 = 0;
        for _ in 0..90 {
            execute_command(&mut app, &mut frame, Command::Step { frames: 1 });
        }

        let find = |app: &mut App, want: &str| {
            let mut q = app
                .world_mut()
                .query::<(&bsengine_scene::Name, bevy_ecs::prelude::Entity)>();
            q.iter(app.world())
                .find(|(n, _)| n.0 == want)
                .map(|(_, e)| e)
                .unwrap_or_else(|| panic!("the demo scene must contain {want}"))
        };
        let fox = find(&mut app, "Fox");
        let shadow = find(&mut app, "Shadow");

        // Put the Shadow on a DIFFERENT clip, and do it HERE rather than in the
        // scene: `GltfPlugin` inserts `AnimationPlayer::new(first_clip_name)`
        // unconditionally when the glTF finishes loading, so anything the scene
        // authored is overwritten at a moment that depends on load timing.
        //
        // The different clip is load-bearing. Both characters are the same rig,
        // so on the same animation every bone agrees by coincidence and the
        // unmapped-bone assertion below cannot tell "kept its own pose" from
        // "was copied wholesale".
        {
            let mut player = app
                .world_mut()
                .get_mut::<bsengine_core::AnimationPlayer>(shadow)
                .expect("the Shadow gets an AnimationPlayer once fox.glb loads");
            player.clip = "Run".to_string();
            player.time = 0.0;
        }
        for _ in 0..30 {
            execute_command(&mut app, &mut frame, Command::Step { frames: 1 });
        }

        // The name must have resolved, or nothing downstream ran at all.
        let retarget = app
            .world()
            .get::<bsengine_gltf::RetargetSource>(shadow)
            .expect(
                "Shadow must have a RetargetSource -- if absent, the component \
                 is not registered for reflection",
            );
        assert!(
            retarget.resolved == Some(fox),
            "the source name must resolve to the Fox entity, got {:?}",
            retarget.resolved
        );
        assert_eq!(retarget.pairs.len(), 4, "all four authored pairs survive");

        // Compared on the published LOCALS, not on `joint_matrices`. A joint
        // matrix folds in the whole parent chain, so it answers "where did this
        // bone end up", while the claim under test is "did this bone receive
        // the source's rotation" -- a bone's own local is that claim directly.
        let joints = |app: &App, e: bevy_ecs::prelude::Entity| {
            app.world()
                .get::<bsengine_gltf::SkinnedMesh>(e)
                .expect("a demo character keeps its skinned mesh")
                .animated_locals
                .clone()
        };
        let nodes_of = |app: &App, e: bevy_ecs::prelude::Entity| {
            app.world()
                .get::<bsengine_gltf::SkinnedMesh>(e)
                .expect("a demo character keeps its skinned mesh")
                .nodes
                .clone()
        };

        let fox_joints = joints(&app, fox);
        let shadow_joints = joints(&app, shadow);
        let nodes = nodes_of(&app, shadow);
        assert!(
            !fox_joints.is_empty() && !shadow_joints.is_empty(),
            "both characters must publish their animated locals"
        );

        // Compare bones by the direction their local rotation sends +Y.
        let dir = |mats: &[glam::Mat4], i: usize| {
            let (_, rot, _) = mats[i].to_scale_rotation_translation();
            rot * glam::Vec3::Y
        };
        let index = |name: &str| {
            nodes
                .iter()
                .position(|n| n.name == name)
                .unwrap_or_else(|| panic!("the rig must have a bone named {name}"))
        };

        // A MAPPED bone must match the fox's.
        let mapped = index("b_LeftLeg01_015");
        let fox_mapped = dir(&fox_joints, mapped);
        let shadow_mapped = dir(&shadow_joints, mapped);
        println!("mapped bone: fox {fox_mapped:?}, shadow {shadow_mapped:?}");
        assert!(
            (fox_mapped - shadow_mapped).length() < 0.05,
            "a mapped bone must follow the source: fox {fox_mapped:?} vs \
             shadow {shadow_mapped:?}"
        );

        // An UNMAPPED bone must NOT have been touched by retargeting. The fox
        // is running foot IK on its legs and the shadow is not, so an unmapped
        // bone that still matched would mean retargeting copied the whole
        // skeleton rather than the four pairs it was given.
        let unmapped = index("b_LeftFoot01_017");
        let fox_unmapped = dir(&fox_joints, unmapped);
        let shadow_unmapped = dir(&shadow_joints, unmapped);
        println!("unmapped bone: fox {fox_unmapped:?}, shadow {shadow_unmapped:?}");
        assert!(
            (fox_unmapped - shadow_unmapped).length() > 0.01,
            "an unmapped bone must keep its own pose, but the shadow's \
             {shadow_unmapped:?} matches the fox's {fox_unmapped:?} -- \
             retargeting copied bones it was not given"
        );
    }
}
