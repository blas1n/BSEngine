use bevy_ecs::prelude::Resource;

/// Current screen-space cursor position in logical pixels.
/// Updated every frame by the window event loop.
#[derive(Resource, Default, Clone, Copy, Debug)]
pub struct CursorPos {
    /// Horizontal cursor position, in logical pixels from the window's left edge.
    pub x: f32,
    /// Vertical cursor position, in logical pixels from the window's top edge.
    pub y: f32,
}
