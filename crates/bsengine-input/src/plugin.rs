use crate::types::{CursorMoved, KeyInput, MouseInput, MouseMotion};
use bevy_app::{App, Plugin};

pub struct InputPlugin;

impl Plugin for InputPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<KeyInput>()
            .add_event::<MouseInput>()
            .add_event::<CursorMoved>()
            .add_event::<MouseMotion>();
    }
}

#[cfg(test)]
mod tests {
    use crate::{CursorMoved, InputPlugin, KeyInput, MouseInput, MouseMotion};
    use bevy_ecs::event::Events;
    use bsengine_app::new_app;

    #[test]
    fn input_plugin_registers_key_events() {
        let mut app = new_app();
        app.add_plugins(InputPlugin);
        assert!(app.world().get_resource::<Events<KeyInput>>().is_some());
    }

    #[test]
    fn input_plugin_registers_mouse_events() {
        let mut app = new_app();
        app.add_plugins(InputPlugin);
        assert!(app.world().get_resource::<Events<MouseInput>>().is_some());
    }

    #[test]
    fn input_plugin_registers_cursor_events() {
        let mut app = new_app();
        app.add_plugins(InputPlugin);
        assert!(app.world().get_resource::<Events<CursorMoved>>().is_some());
    }

    #[test]
    fn input_plugin_registers_mouse_motion_events() {
        let mut app = new_app();
        app.add_plugins(InputPlugin);
        assert!(app.world().get_resource::<Events<MouseMotion>>().is_some());
    }
}
