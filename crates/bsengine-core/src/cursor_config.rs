use bevy_ecs::prelude::Resource;

#[derive(Resource, Clone, Copy, Debug)]
pub struct CursorConfig {
    pub visible: bool,
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
