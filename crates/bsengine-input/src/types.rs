use bevy_ecs::prelude::Resource;
use bsengine_ecs::Event;

/// Platform-independent physical keyboard key, translated from winit key codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyCode {
    /// The "A" key.
    A,
    /// The "B" key.
    B,
    /// The "C" key.
    C,
    /// The "D" key.
    D,
    /// The "E" key.
    E,
    /// The "F" key.
    F,
    /// The "G" key.
    G,
    /// The "H" key.
    H,
    /// The "I" key.
    I,
    /// The "J" key.
    J,
    /// The "K" key.
    K,
    /// The "L" key.
    L,
    /// The "M" key.
    M,
    /// The "N" key.
    N,
    /// The "O" key.
    O,
    /// The "P" key.
    P,
    /// The "Q" key.
    Q,
    /// The "R" key.
    R,
    /// The "S" key.
    S,
    /// The "T" key.
    T,
    /// The "U" key.
    U,
    /// The "V" key.
    V,
    /// The "W" key.
    W,
    /// The "X" key.
    X,
    /// The "Y" key.
    Y,
    /// The "Z" key.
    Z,
    /// The spacebar.
    Space,
    /// The Enter/Return key.
    Enter,
    /// The Escape key.
    Escape,
    /// The Backspace key.
    Backspace,
    /// The Tab key.
    Tab,
    /// The left arrow key.
    Left,
    /// The right arrow key.
    Right,
    /// The up arrow key.
    Up,
    /// The down arrow key.
    Down,
    /// The "0" digit key.
    Key0,
    /// The "1" digit key.
    Key1,
    /// The "2" digit key.
    Key2,
    /// The "3" digit key.
    Key3,
    /// The "4" digit key.
    Key4,
    /// The "5" digit key.
    Key5,
    /// The "6" digit key.
    Key6,
    /// The "7" digit key.
    Key7,
    /// The "8" digit key.
    Key8,
    /// The "9" digit key.
    Key9,
    /// The Delete key.
    Delete,
    /// The "-" (minus/hyphen) key.
    Minus,
    /// The "=" (equals) key.
    Equals,
    /// The "." (period) key.
    Period,
    /// The "," (comma) key.
    Comma,
    /// The Home key.
    Home,
    /// The End key.
    End,
    /// The left Control key.
    ControlLeft,
    /// The right Control key.
    ControlRight,
    /// The left Shift key.
    ShiftLeft,
    /// The right Shift key.
    ShiftRight,
    /// The left Alt key.
    AltLeft,
    /// The right Alt key.
    AltRight,
    /// A key that could not be mapped to a known `KeyCode` variant.
    Unknown,
}

/// Press/release state shared by keyboard and mouse button events.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElementState {
    /// The key or button was pressed down.
    Pressed,
    /// The key or button was released.
    Released,
}

/// Event fired when a keyboard key changes state (pressed or released).
#[derive(Event, Debug, Clone)]
pub struct KeyInput {
    /// The physical key that changed state.
    pub key_code: KeyCode,
    /// Whether the key was pressed or released.
    pub state: ElementState,
    /// Text produced by this key press, accounting for shift/keyboard layout
    /// (e.g. "1", ".", "-"), as reported by the OS/windowing layer. `None`
    /// for releases and for keys that don't produce text (arrows, Ctrl, etc.
    /// held combos where the OS suppresses text).
    pub text: Option<String>,
}

/// A mouse button, as reported by the platform windowing layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MouseButton {
    /// The primary (left) mouse button.
    Left,
    /// The secondary (right) mouse button.
    Right,
    /// The middle mouse button (often the scroll wheel click).
    Middle,
    /// Any other mouse button, identified by its platform-reported index.
    Other(u16),
}

/// Event fired when a mouse button changes state (pressed or released).
#[derive(Event, Debug, Clone)]
pub struct MouseInput {
    /// The mouse button that changed state.
    pub button: MouseButton,
    /// Whether the button was pressed or released.
    pub state: ElementState,
}

/// Event fired with the cursor's absolute position whenever it moves within the window.
#[derive(Event, Debug, Clone)]
pub struct CursorMoved {
    /// Cursor x position in window pixels, measured from the left edge.
    pub x: f64,
    /// Cursor y position in window pixels, measured from the top edge.
    pub y: f64,
}

/// Raw mouse movement delta from the OS device event stream.
/// Unlike CursorMoved, this is not affected by cursor acceleration or screen bounds.
/// Use this for FPS-style camera rotation.
#[derive(Event, Debug, Clone)]
pub struct MouseMotion {
    /// Raw horizontal movement delta since the last frame, in device units.
    pub dx: f64,
    /// Raw vertical movement delta since the last frame, in device units.
    pub dy: f64,
}

/// Scroll wheel delta for the current frame.
/// Positive = scroll up / zoom in; negative = scroll down / zoom out.
/// Line deltas are passed as-is; pixel deltas are normalised by 40 px/line.
#[derive(Event, Debug, Clone)]
pub struct MouseWheel {
    /// Scroll amount for this event; positive scrolls up/zooms in, negative scrolls down/zooms out.
    pub delta: f64,
}

/// Gamepad face and shoulder buttons. Bit indices match the scripting API (0=South..15=DPadRight).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GamepadButton {
    /// Bit 0: bottom face button (A / Cross).
    South, // 0: A / Cross
    /// Bit 1: right face button (B / Circle).
    East, // 1: B / Circle
    /// Bit 2: left face button (X / Square).
    West, // 2: X / Square
    /// Bit 3: top face button (Y / Triangle).
    North, // 3: Y / Triangle
    /// Bit 4: left shoulder bumper (L1 / LeftBumper).
    LB, // 4: L1 / LeftBumper
    /// Bit 5: right shoulder bumper (R1 / RightBumper).
    RB, // 5: R1 / RightBumper
    /// Bit 6: left shoulder trigger, digital press (L2 / LeftTrigger).
    LT, // 6: L2 / LeftTrigger (digital)
    /// Bit 7: right shoulder trigger, digital press (R2 / RightTrigger).
    RT, // 7: R2 / RightTrigger (digital)
    /// Bit 8: select/back button.
    Select, // 8
    /// Bit 9: start/menu button.
    Start, // 9
    /// Bit 10: left stick click (L3).
    LeftStick, // 10: L3
    /// Bit 11: right stick click (R3).
    RightStick, // 11: R3
    /// Bit 12: D-pad up.
    DPadUp, // 12
    /// Bit 13: D-pad down.
    DPadDown, // 13
    /// Bit 14: D-pad left.
    DPadLeft, // 14
    /// Bit 15: D-pad right.
    DPadRight, // 15
}

/// Analog stick and trigger values from the first connected gamepad.
#[derive(Resource, Default, Clone)]
pub struct GamepadSticks {
    /// Left analog stick axes as `(x, y)`, each in the range -1.0..=1.0.
    pub left: (f32, f32),
    /// Right analog stick axes as `(x, y)`, each in the range -1.0..=1.0.
    pub right: (f32, f32),
    /// Left analog trigger pressure, in the range 0.0..=1.0.
    pub left_trigger: f32,
    /// Right analog trigger pressure, in the range 0.0..=1.0.
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
            text: None,
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
