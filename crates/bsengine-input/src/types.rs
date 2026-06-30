use bevy_ecs::prelude::Resource;
use bsengine_ecs::Event;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyCode {
    A,
    B,
    C,
    D,
    E,
    F,
    G,
    H,
    I,
    J,
    K,
    L,
    M,
    N,
    O,
    P,
    Q,
    R,
    S,
    T,
    U,
    V,
    W,
    X,
    Y,
    Z,
    Space,
    Enter,
    Escape,
    Backspace,
    Tab,
    Left,
    Right,
    Up,
    Down,
    Key0,
    Key1,
    Key2,
    Key3,
    Key4,
    Key5,
    Key6,
    Key7,
    Key8,
    Key9,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElementState {
    Pressed,
    Released,
}

#[derive(Event, Debug, Clone)]
pub struct KeyInput {
    pub key_code: KeyCode,
    pub state: ElementState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    Other(u16),
}

#[derive(Event, Debug, Clone)]
pub struct MouseInput {
    pub button: MouseButton,
    pub state: ElementState,
}

#[derive(Event, Debug, Clone)]
pub struct CursorMoved {
    pub x: f64,
    pub y: f64,
}

/// Raw mouse movement delta from the OS device event stream.
/// Unlike CursorMoved, this is not affected by cursor acceleration or screen bounds.
/// Use this for FPS-style camera rotation.
#[derive(Event, Debug, Clone)]
pub struct MouseMotion {
    pub dx: f64,
    pub dy: f64,
}

/// Gamepad face and shoulder buttons. Bit indices match the scripting API (0=South..15=DPadRight).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GamepadButton {
    South,      // 0: A / Cross
    East,       // 1: B / Circle
    West,       // 2: X / Square
    North,      // 3: Y / Triangle
    LB,         // 4: L1 / LeftBumper
    RB,         // 5: R1 / RightBumper
    LT,         // 6: L2 / LeftTrigger (digital)
    RT,         // 7: R2 / RightTrigger (digital)
    Select,     // 8
    Start,      // 9
    LeftStick,  // 10: L3
    RightStick, // 11: R3
    DPadUp,     // 12
    DPadDown,   // 13
    DPadLeft,   // 14
    DPadRight,  // 15
}

/// Analog stick and trigger values from the first connected gamepad.
#[derive(Resource, Default, Clone)]
pub struct GamepadSticks {
    pub left: (f32, f32),
    pub right: (f32, f32),
    pub left_trigger: f32,
    pub right_trigger: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_input_event_fields() {
        let event = KeyInput {
            key_code: KeyCode::W,
            state: ElementState::Pressed,
        };
        assert_eq!(event.key_code, KeyCode::W);
        assert_eq!(event.state, ElementState::Pressed);
    }

    #[test]
    fn mouse_button_other_variant() {
        let btn = MouseButton::Other(5);
        assert_eq!(btn, MouseButton::Other(5));
    }
}
