use bevy_ecs::prelude::Resource;

/// Resource controlling the OS cursor's visibility and whether it is locked
/// (confined/grabbed) to the window, e.g. for mouse-look camera controls.
#[derive(Resource, Clone, Copy, Debug)]
pub struct CursorConfig {
    /// Whether the OS cursor is shown.
    pub visible: bool,
    /// Whether the cursor is confined/grabbed to the window.
    pub locked: bool,
}

impl Default for CursorConfig {
    fn default() -> Self {
        Self {
            visible: true,
            locked: false,
        }
    }
}
