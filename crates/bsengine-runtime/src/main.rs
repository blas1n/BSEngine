use std::env;

use bsengine_app::{
    new_app, AnimationPlugin, AnimationStateMachinePlugin, NavMeshPlugin, TimePlugin,
};
use bsengine_asset::{AssetPlugin, AssetStatusPlugin, AssetWatcherPlugin};
use bsengine_audio::AudioPlugin;
use bsengine_core::{EditorPlayState, InspectorState};
use bsengine_editor::EditorPlugin;
use bsengine_gltf::{GltfPlugin, SkinnedMeshPlugin};
use bsengine_input::InputPlugin;
use bsengine_network::NetworkPlugin;
use bsengine_physics::PhysicsPlugin;
use bsengine_render::RenderPlugin;
use bsengine_rhi_wgpu::WgpuRHIPlugin;
use bsengine_scene::ScenePlugin;
use bsengine_scripting::ScriptingPlugin;
use bsengine_window::{WindowDescriptor, WindowPlugin};

mod scene_systems;
mod test_mode;
mod test_protocol;
mod test_query;

use scene_systems::{register_scene_systems, ProjectManifest};

fn main() {
    let mut args = env::args().skip(1);
    let first_arg = args.next().unwrap_or_else(|| ".".to_string());

    if first_arg == "--test" {
        let project_dir = args.next().unwrap_or_else(|| ".".to_string());
        match args.next().as_deref() {
            Some("--replay") => {
                let log_path = args
                    .next()
                    .unwrap_or_else(|| panic!("--replay requires a log file path"));
                let passed = test_mode::run_replay_mode(&project_dir, &log_path);
                std::process::exit(if passed { 0 } else { 1 });
            }
            Some(other) => panic!("unknown argument after project dir: {other}"),
            None => test_mode::run_test_mode(&project_dir),
        }
        return;
    }

    run_windowed(&first_arg);
}

fn run_windowed(project_dir: &str) {
    let manifest_path = format!("{project_dir}/project.toml");

    let manifest_str = std::fs::read_to_string(&manifest_path)
        .unwrap_or_else(|e| panic!("Cannot read {manifest_path}: {e}"));
    let manifest: ProjectManifest = toml::from_str(&manifest_str)
        .unwrap_or_else(|e| panic!("Cannot parse {manifest_path}: {e}"));

    let scene_path = format!("{}/{}", project_dir, manifest.project.entry_scene);
    let title = manifest
        .window
        .title
        .clone()
        .unwrap_or_else(|| manifest.project.name.clone());

    let mut app = new_app();
    app.add_plugins(TimePlugin)
        .add_plugins(AssetPlugin)
        // Windowed only, deliberately. `--test` builds its own app
        // (test_mode::build_test_app) with its own plugin list, so leaving
        // this out of that list is the whole of the decision: a replay has
        // nobody editing files, so a watcher there would buy nothing and
        // cost a background thread plus a source of frame-to-frame variation
        // in the one mode that pins its clocks precisely to stay
        // reproducible. Needs AssetPlugin (for AssetServer) and a ProjectDir,
        // which ScriptingPlugin inserts below at build time — i.e. before any
        // Startup system, including this plugin's, ever runs.
        .add_plugins(AssetWatcherPlugin)
        // Unlike the watcher above, this one is in `--test`'s plugin list
        // too (test_mode::build_test_app) — see there for why. Without it
        // registered *somewhere* the whole status API is inert: the resource
        // never exists, so `AssetStatuses::get` and `Bsengine.getAssetStatus`
        // answer `unknown` for every path forever, including the ones that
        // just failed to load.
        .add_plugins(AssetStatusPlugin)
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
        .add_plugins(AudioPlugin)
        .add_plugins(PhysicsPlugin)
        .add_plugins(NetworkPlugin)
        .add_plugins(EditorPlugin)
        .add_plugins(RenderPlugin)
        .add_plugins(GltfPlugin)
        .add_plugins(SkinnedMeshPlugin)
        .add_plugins(AnimationPlugin)
        .add_plugins(AnimationStateMachinePlugin)
        .add_plugins(NavMeshPlugin)
        .add_plugins(ScenePlugin::from_file(&scene_path))
        .add_plugins(ScriptingPlugin {
            project_dir: project_dir.to_string(),
        });
    register_scene_systems(&mut app);

    // bsengine-runtime's job is to run a game, not edit one — EditorPlugin
    // is still included (for now, this is the only windowed entry point,
    // and its inspector/hierarchy tooling is useful during development),
    // but it defaults to InspectorState::editor()'s Stopped play state,
    // which silently gates scripts (WASD, onUpdate, ...) off until the
    // user finds and clicks the toolbar's Play button. Force Playing here
    // so `cargo run -p bsengine-runtime -- <game>` actually plays the game
    // immediately, matching what running a game is supposed to do.
    {
        let mut inspector = app.world_mut().resource_mut::<InspectorState>();
        inspector.play_state = EditorPlayState::Playing;
        // Populated on manual Ctrl+S saves otherwise; without this, a
        // freshly-launched game (never saved) has no path for the Play
        // button's "reload the scene" behavior to reload from.
        inspector.current_scene_path = Some(scene_path.clone());
    }

    app.run();
}
