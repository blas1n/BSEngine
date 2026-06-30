use bevy_app::{App, Plugin, PreUpdate};
use bevy_ecs::prelude::{EventReader, NonSendMut, ResMut, Resource};
use bsengine_ecs::IntoSystemConfigs;

use crate::{
    state::Input,
    types::{
        CursorMoved, ElementState, GamepadButton, GamepadSticks, KeyCode, KeyInput, MouseButton,
        MouseInput, MouseMotion,
    },
};

pub struct GilrsResource(gilrs::Gilrs);

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
            .insert_resource(Input::<GamepadButton>::default())
            .insert_resource(MouseState::default())
            .insert_resource(GamepadSticks::default())
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

        match gilrs::Gilrs::new() {
            Ok(g) => {
                app.insert_non_send_resource(GilrsResource(g));
                app.add_systems(PreUpdate, poll_gamepad_events.after(clear_input_state));
            }
            Err(e) => eprintln!("[input] gamepad not available: {e}"),
        }
    }
}

fn clear_input_state(
    mut keys: ResMut<Input<KeyCode>>,
    mut buttons: ResMut<Input<MouseButton>>,
    mut gamepad: ResMut<Input<GamepadButton>>,
) {
    keys.clear_transient();
    buttons.clear_transient();
    gamepad.clear_transient();
}

fn poll_gamepad_events(
    gilrs: Option<NonSendMut<GilrsResource>>,
    mut buttons: ResMut<Input<GamepadButton>>,
    mut sticks: ResMut<GamepadSticks>,
) {
    let Some(mut gilrs) = gilrs else { return };
    while let Some(event) = gilrs.0.next_event() {
        match event.event {
            gilrs::EventType::ButtonPressed(btn, _) => {
                if let Some(b) = map_gilrs_button(btn) {
                    buttons.press(b);
                }
            }
            gilrs::EventType::ButtonReleased(btn, _) => {
                if let Some(b) = map_gilrs_button(btn) {
                    buttons.release(b);
                }
            }
            gilrs::EventType::AxisChanged(axis, value, _) => match axis {
                gilrs::Axis::LeftStickX => sticks.left.0 = value,
                gilrs::Axis::LeftStickY => sticks.left.1 = value,
                gilrs::Axis::RightStickX => sticks.right.0 = value,
                gilrs::Axis::RightStickY => sticks.right.1 = value,
                gilrs::Axis::LeftZ => sticks.left_trigger = value,
                gilrs::Axis::RightZ => sticks.right_trigger = value,
                _ => {}
            },
            _ => {}
        }
    }
}

fn map_gilrs_button(btn: gilrs::Button) -> Option<GamepadButton> {
    match btn {
        gilrs::Button::South => Some(GamepadButton::South),
        gilrs::Button::East => Some(GamepadButton::East),
        gilrs::Button::West => Some(GamepadButton::West),
        gilrs::Button::North => Some(GamepadButton::North),
        gilrs::Button::LeftTrigger => Some(GamepadButton::LB),
        gilrs::Button::RightTrigger => Some(GamepadButton::RB),
        gilrs::Button::LeftTrigger2 => Some(GamepadButton::LT),
        gilrs::Button::RightTrigger2 => Some(GamepadButton::RT),
        gilrs::Button::Select => Some(GamepadButton::Select),
        gilrs::Button::Start => Some(GamepadButton::Start),
        gilrs::Button::LeftThumb => Some(GamepadButton::LeftStick),
        gilrs::Button::RightThumb => Some(GamepadButton::RightStick),
        gilrs::Button::DPadUp => Some(GamepadButton::DPadUp),
        gilrs::Button::DPadDown => Some(GamepadButton::DPadDown),
        gilrs::Button::DPadLeft => Some(GamepadButton::DPadLeft),
        gilrs::Button::DPadRight => Some(GamepadButton::DPadRight),
        _ => None,
    }
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
        state::Input, CursorMoved, ElementState, GamepadButton, GamepadSticks, InputPlugin,
        KeyCode, KeyInput, MouseButton, MouseInput, MouseMotion, MouseState,
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

    #[test]
    fn input_plugin_registers_gamepad_button_resource() {
        let mut app = new_app();
        app.add_plugins(InputPlugin);
        assert!(app.world().get_resource::<Input<GamepadButton>>().is_some());
    }

    #[test]
    fn input_plugin_registers_gamepad_sticks_resource() {
        let mut app = new_app();
        app.add_plugins(InputPlugin);
        assert!(app.world().get_resource::<GamepadSticks>().is_some());
    }
}
