use bevy_app::{App, Plugin, PreUpdate};
use bevy_ecs::prelude::{EventReader, Res, ResMut, Resource};
use bsengine_ecs::IntoSystemConfigs;

use crate::{
    state::Input,
    types::{CursorMoved, ElementState, KeyCode, KeyInput, MouseButton, MouseInput, MouseMotion},
};

/// Per-frame mouse position and raw movement delta.
/// `position` tracks the last cursor position (pixels, top-left origin).
/// `delta` accumulates raw mouse motion for the current frame and resets each frame.
#[derive(Resource, Default, Clone)]
pub struct MouseState {
    pub position: (f64, f64),
    pub delta: (f64, f64),
}

pub struct InputPlugin;

impl Plugin for InputPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<KeyInput>()
            .add_event::<MouseInput>()
            .add_event::<CursorMoved>()
            .add_event::<MouseMotion>()
            .insert_resource(Input::<KeyCode>::default())
            .insert_resource(Input::<MouseButton>::default())
            .insert_resource(MouseState::default())
            .add_systems(
                PreUpdate,
                (
                    clear_input_state,
                    update_keyboard_state,
                    update_mouse_button_state,
                    update_mouse_position_state,
                )
                    .chain(),
            );
    }
}

fn clear_input_state(mut keys: ResMut<Input<KeyCode>>, mut buttons: ResMut<Input<MouseButton>>) {
    keys.clear_transient();
    buttons.clear_transient();
}

fn update_keyboard_state(mut keys: ResMut<Input<KeyCode>>, mut events: EventReader<KeyInput>) {
    for ev in events.read() {
        match ev.state {
            ElementState::Pressed => keys.press(ev.key_code),
            ElementState::Released => keys.release(ev.key_code),
        }
    }
}

fn update_mouse_button_state(
    mut buttons: ResMut<Input<MouseButton>>,
    mut events: EventReader<MouseInput>,
) {
    for ev in events.read() {
        match ev.state {
            ElementState::Pressed => buttons.press(ev.button),
            ElementState::Released => buttons.release(ev.button),
        }
    }
}

fn update_mouse_position_state(
    mut mouse_state: ResMut<MouseState>,
    mut cursor_events: EventReader<CursorMoved>,
    mut motion_events: EventReader<MouseMotion>,
) {
    mouse_state.delta = (0.0, 0.0);
    for ev in cursor_events.read() {
        mouse_state.position = (ev.x, ev.y);
    }
    for ev in motion_events.read() {
        mouse_state.delta.0 += ev.dx;
        mouse_state.delta.1 += ev.dy;
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        state::Input, CursorMoved, ElementState, InputPlugin, KeyCode, KeyInput, MouseButton,
        MouseInput, MouseMotion, MouseState,
    };
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

    #[test]
    fn key_input_state_is_pressed_after_event() {
        let mut app = new_app();
        app.add_plugins(InputPlugin);

        app.world_mut()
            .resource_mut::<Events<KeyInput>>()
            .send(KeyInput {
                key_code: KeyCode::W,
                state: ElementState::Pressed,
            });

        app.update();

        let keys = app.world().resource::<Input<KeyCode>>();
        assert!(keys.is_pressed(&KeyCode::W));
        assert!(keys.just_pressed(&KeyCode::W));
    }

    #[test]
    fn just_pressed_cleared_next_frame() {
        let mut app = new_app();
        app.add_plugins(InputPlugin);

        app.world_mut()
            .resource_mut::<Events<KeyInput>>()
            .send(KeyInput {
                key_code: KeyCode::Space,
                state: ElementState::Pressed,
            });

        app.update();
        app.update();

        let keys = app.world().resource::<Input<KeyCode>>();
        assert!(keys.is_pressed(&KeyCode::Space));
        assert!(!keys.just_pressed(&KeyCode::Space));
    }

    #[test]
    fn key_released_clears_pressed() {
        let mut app = new_app();
        app.add_plugins(InputPlugin);

        app.world_mut()
            .resource_mut::<Events<KeyInput>>()
            .send(KeyInput {
                key_code: KeyCode::A,
                state: ElementState::Pressed,
            });
        app.update();

        app.world_mut()
            .resource_mut::<Events<KeyInput>>()
            .send(KeyInput {
                key_code: KeyCode::A,
                state: ElementState::Released,
            });
        app.update();

        let keys = app.world().resource::<Input<KeyCode>>();
        assert!(!keys.is_pressed(&KeyCode::A));
        assert!(keys.just_released(&KeyCode::A));
    }

    #[test]
    fn mouse_button_state_tracked() {
        let mut app = new_app();
        app.add_plugins(InputPlugin);

        app.world_mut()
            .resource_mut::<Events<MouseInput>>()
            .send(MouseInput {
                button: MouseButton::Left,
                state: ElementState::Pressed,
            });

        app.update();

        let buttons = app.world().resource::<Input<MouseButton>>();
        assert!(buttons.is_pressed(&MouseButton::Left));
    }

    #[test]
    fn mouse_state_delta_accumulates_from_motion_events() {
        let mut app = new_app();
        app.add_plugins(InputPlugin);

        app.world_mut()
            .resource_mut::<Events<MouseMotion>>()
            .send(MouseMotion { dx: 3.0, dy: -2.0 });
        app.world_mut()
            .resource_mut::<Events<MouseMotion>>()
            .send(MouseMotion { dx: 1.0, dy: 1.0 });

        app.update();

        let ms = app.world().resource::<MouseState>();
        assert!((ms.delta.0 - 4.0).abs() < 1e-9, "delta.x should be 4.0");
        assert!((ms.delta.1 + 1.0).abs() < 1e-9, "delta.y should be -1.0");
    }

    #[test]
    fn mouse_state_delta_resets_each_frame() {
        let mut app = new_app();
        app.add_plugins(InputPlugin);

        app.world_mut()
            .resource_mut::<Events<MouseMotion>>()
            .send(MouseMotion { dx: 5.0, dy: 5.0 });
        app.update();
        app.update(); // no new events

        let ms = app.world().resource::<MouseState>();
        assert!((ms.delta.0).abs() < 1e-9, "delta should reset");
    }

    #[test]
    fn cursor_moved_updates_position() {
        let mut app = new_app();
        app.add_plugins(InputPlugin);

        app.world_mut()
            .resource_mut::<Events<CursorMoved>>()
            .send(CursorMoved { x: 100.0, y: 200.0 });

        app.update();

        let ms = app.world().resource::<MouseState>();
        assert!((ms.position.0 - 100.0).abs() < 1e-9);
        assert!((ms.position.1 - 200.0).abs() < 1e-9);
    }

    #[test]
    fn input_plugin_registers_mouse_state_resource() {
        let mut app = new_app();
        app.add_plugins(InputPlugin);
        assert!(app.world().get_resource::<MouseState>().is_some());
    }
}
