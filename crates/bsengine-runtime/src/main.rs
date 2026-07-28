use std::env;

use bsengine_app::new_app;
use bsengine_audio::AudioPlugin;
use bsengine_core::{EditorPlayState, InspectorState};
use bsengine_editor::EditorPlugin;
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
    app.add_plugins(WgpuRHIPlugin)
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
