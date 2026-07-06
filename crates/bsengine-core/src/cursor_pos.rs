use bevy_ecs::prelude::Resource;

/// Current screen-space cursor position in logical pixels.
/// Updated every frame by the window event loop.
#[derive(Resource, Default, Clone, Copy, Debug)]
pub struct CursorPos {
    pub x: f32,
    pub y: f32,
}
