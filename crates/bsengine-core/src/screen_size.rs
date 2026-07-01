use bevy_ecs::prelude::Resource;

#[derive(Resource, Clone, Copy, Debug)]
pub struct ScreenSize {
    pub width: u32,
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
