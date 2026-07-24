use bevy_ecs::prelude::Resource;

/// Current window/render-target size, in pixels, kept in sync with the OS window.
#[derive(Resource, Clone, Copy, Debug)]
pub struct ScreenSize {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
}

impl Default for ScreenSize {
    fn default() -> Self {
        Self {
            width: 1280,
            height: 720,
        }
    }
}
