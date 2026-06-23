use bsengine_app::new_app;
use bsengine_input::InputPlugin;
use bsengine_window::WindowPlugin;

fn main() {
    new_app()
        .add_plugins(WindowPlugin::default())
        .add_plugins(InputPlugin)
        .run();
}
