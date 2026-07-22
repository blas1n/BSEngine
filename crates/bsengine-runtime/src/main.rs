use std::env;

use bsengine_app::new_app;
use bsengine_audio::AudioPlugin;
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
        test_mode::run_test_mode(&project_dir);
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
    app.run();
}
