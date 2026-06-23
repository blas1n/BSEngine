use bsengine_ecs::Event;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyCode {
    A, B, C, D, E, F, G, H, I, J, K, L, M,
    N, O, P, Q, R, S, T, U, V, W, X, Y, Z,
    Space, Enter, Escape, Backspace, Tab,
    Left, Right, Up, Down,
    Key0, Key1, Key2, Key3, Key4,
    Key5, Key6, Key7, Key8, Key9,
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
