use bevy_app::Update;
use bevy_ecs::message::{MessageReader, Messages};
use bsengine_app::new_app;
use bsengine_ecs::{ResMut, Resource};
use bsengine_input::{ElementState, InputPlugin, KeyCode, KeyInput, MouseButton, MouseInput};

#[derive(Resource, Default)]
struct InputLog {
    w_presses: u32,
    left_clicks: u32,
}

fn track_input(
    mut events_key: MessageReader<KeyInput>,
    mut events_mouse: MessageReader<MouseInput>,
    mut log: ResMut<InputLog>,
) {
    for ev in events_key.read() {
        if ev.key_code == KeyCode::W && ev.state == ElementState::Pressed {
            log.w_presses += 1;
        }
    }
    for ev in events_mouse.read() {
        if ev.button == MouseButton::Left && ev.state == ElementState::Pressed {
            log.left_clicks += 1;
        }
    }
}

#[test]
fn key_events_received_by_system() {
    let mut app = new_app();
    app.add_plugins(InputPlugin)
        .init_resource::<InputLog>()
        .add_systems(Update, track_input);

    // Manually send events (simulating winit input)
    {
        let mut events = app.world_mut().resource_mut::<Messages<KeyInput>>();
        events.write(KeyInput {
            key_code: KeyCode::W,
            state: ElementState::Pressed,
            text: None,
        });
        events.write(KeyInput {
            key_code: KeyCode::W,
            state: ElementState::Pressed,
            text: None,
        });
        events.write(KeyInput {
            key_code: KeyCode::S,
            state: ElementState::Pressed,
            text: None,
        }); // ignored
    }

    app.update();

    assert_eq!(app.world().resource::<InputLog>().w_presses, 2);
}

#[test]
fn mouse_events_received_by_system() {
    let mut app = new_app();
    app.add_plugins(InputPlugin)
        .init_resource::<InputLog>()
        .add_systems(Update, track_input);

    {
        let mut events = app.world_mut().resource_mut::<Messages<MouseInput>>();
        events.write(MouseInput {
            button: MouseButton::Left,
            state: ElementState::Pressed,
        });
        events.write(MouseInput {
            button: MouseButton::Right,
            state: ElementState::Pressed,
        }); // ignored
    }

    app.update();

    assert_eq!(app.world().resource::<InputLog>().left_clicks, 1);
}

#[test]
fn events_cleared_between_frames() {
    let mut app = new_app();
    app.add_plugins(InputPlugin)
        .init_resource::<InputLog>()
        .add_systems(Update, track_input);

    // Frame 1: send W
    {
        let mut events = app.world_mut().resource_mut::<Messages<KeyInput>>();
        events.write(KeyInput {
            key_code: KeyCode::W,
            state: ElementState::Pressed,
            text: None,
        });
    }
    app.update();
    assert_eq!(app.world().resource::<InputLog>().w_presses, 1);

    // Frame 2: no new events — bevy clears double-buffered events after 2 updates
    app.update();
    // Count should still be 1 (no new W presses in frame 2)
    assert_eq!(app.world().resource::<InputLog>().w_presses, 1);
}
