use bsengine_core::init_logging;

pub use bevy_app::App;
pub use bevy_app::Plugin as BsPlugin;
pub use bevy_app::{Last, PostUpdate, PreUpdate, Startup, Update};

/// Initializes engine logging and constructs a fresh, empty `bevy_app::App`.
pub fn new_app() -> App {
    init_logging();
    App::new()
}
