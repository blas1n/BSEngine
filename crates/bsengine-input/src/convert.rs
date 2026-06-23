use crate::types::{ElementState, KeyCode};
use winit::event::ElementState as WinitState;
use winit::keyboard::{KeyCode as WinitKeyCode, PhysicalKey};

pub fn convert_key_code(key: PhysicalKey) -> KeyCode {
    match key {
        PhysicalKey::Code(code) => match code {
            WinitKeyCode::KeyA => KeyCode::A,
            WinitKeyCode::KeyB => KeyCode::B,
            WinitKeyCode::KeyC => KeyCode::C,
            WinitKeyCode::KeyD => KeyCode::D,
            WinitKeyCode::KeyE => KeyCode::E,
            WinitKeyCode::KeyF => KeyCode::F,
            WinitKeyCode::KeyG => KeyCode::G,
            WinitKeyCode::KeyH => KeyCode::H,
            WinitKeyCode::KeyI => KeyCode::I,
            WinitKeyCode::KeyJ => KeyCode::J,
            WinitKeyCode::KeyK => KeyCode::K,
            WinitKeyCode::KeyL => KeyCode::L,
            WinitKeyCode::KeyM => KeyCode::M,
            WinitKeyCode::KeyN => KeyCode::N,
            WinitKeyCode::KeyO => KeyCode::O,
            WinitKeyCode::KeyP => KeyCode::P,
            WinitKeyCode::KeyQ => KeyCode::Q,
            WinitKeyCode::KeyR => KeyCode::R,
            WinitKeyCode::KeyS => KeyCode::S,
            WinitKeyCode::KeyT => KeyCode::T,
            WinitKeyCode::KeyU => KeyCode::U,
            WinitKeyCode::KeyV => KeyCode::V,
            WinitKeyCode::KeyW => KeyCode::W,
            WinitKeyCode::KeyX => KeyCode::X,
            WinitKeyCode::KeyY => KeyCode::Y,
            WinitKeyCode::KeyZ => KeyCode::Z,
            WinitKeyCode::Space => KeyCode::Space,
            WinitKeyCode::Enter => KeyCode::Enter,
            WinitKeyCode::Escape => KeyCode::Escape,
            WinitKeyCode::Backspace => KeyCode::Backspace,
            WinitKeyCode::Tab => KeyCode::Tab,
            WinitKeyCode::ArrowLeft => KeyCode::Left,
            WinitKeyCode::ArrowRight => KeyCode::Right,
            WinitKeyCode::ArrowUp => KeyCode::Up,
            WinitKeyCode::ArrowDown => KeyCode::Down,
            WinitKeyCode::Digit0 => KeyCode::Key0,
            WinitKeyCode::Digit1 => KeyCode::Key1,
            WinitKeyCode::Digit2 => KeyCode::Key2,
            WinitKeyCode::Digit3 => KeyCode::Key3,
            WinitKeyCode::Digit4 => KeyCode::Key4,
            WinitKeyCode::Digit5 => KeyCode::Key5,
            WinitKeyCode::Digit6 => KeyCode::Key6,
            WinitKeyCode::Digit7 => KeyCode::Key7,
            WinitKeyCode::Digit8 => KeyCode::Key8,
            WinitKeyCode::Digit9 => KeyCode::Key9,
            _ => KeyCode::Unknown,
        },
        _ => KeyCode::Unknown,
    }
}

pub fn convert_element_state(state: WinitState) -> ElementState {
    match state {
        WinitState::Pressed => ElementState::Pressed,
        WinitState::Released => ElementState::Released,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{ElementState, KeyCode};

    #[test]
    fn convert_w_key() {
        use winit::keyboard::{KeyCode as WinitKeyCode, PhysicalKey};
        let result = convert_key_code(PhysicalKey::Code(WinitKeyCode::KeyW));
        assert_eq!(result, KeyCode::W);
    }

    #[test]
    fn convert_unknown_key() {
        use winit::keyboard::PhysicalKey;
        let result = convert_key_code(PhysicalKey::Unidentified(
            winit::keyboard::NativeKeyCode::Unidentified,
        ));
        assert_eq!(result, KeyCode::Unknown);
    }

    #[test]
    fn convert_pressed_state() {
        use winit::event::ElementState as WinitState;
        assert_eq!(
            convert_element_state(WinitState::Pressed),
            ElementState::Pressed
        );
        assert_eq!(
            convert_element_state(WinitState::Released),
            ElementState::Released
        );
    }
}
