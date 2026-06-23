use bevy_app::{App, Plugin};
use crate::types::{CursorMoved, KeyInput, MouseInput};

pub struct InputPlugin;

impl Plugin for InputPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<KeyInput>()
            .add_event::<MouseInput>()
            .add_event::<CursorMoved>();
    }
}

#[cfg(test)]
mod tests {
    use bsengine_app::new_app;
    use bevy_ecs::event::Events;
    use crate::{InputPlugin, KeyInput, MouseInput, CursorMoved};

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
}
