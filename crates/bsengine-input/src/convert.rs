use crate::touch::{TouchEvent, TouchInput};
use crate::types::{ElementState, KeyCode};
use winit::event::ElementState as WinitState;
use winit::keyboard::{Key, KeyCode as WinitKeyCode, NamedKey, PhysicalKey};

/// Maps a winit physical key code to BSEngine's `KeyCode`, returning `KeyCode::Unknown`
/// for unmapped or unidentified keys.
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
            WinitKeyCode::Delete => KeyCode::Delete,
            WinitKeyCode::Minus => KeyCode::Minus,
            WinitKeyCode::Equal => KeyCode::Equals,
            WinitKeyCode::Period => KeyCode::Period,
            WinitKeyCode::Comma => KeyCode::Comma,
            WinitKeyCode::Home => KeyCode::Home,
            WinitKeyCode::End => KeyCode::End,
            WinitKeyCode::ControlLeft => KeyCode::ControlLeft,
            WinitKeyCode::ControlRight => KeyCode::ControlRight,
            WinitKeyCode::ShiftLeft => KeyCode::ShiftLeft,
            WinitKeyCode::ShiftRight => KeyCode::ShiftRight,
            WinitKeyCode::AltLeft => KeyCode::AltLeft,
            WinitKeyCode::AltRight => KeyCode::AltRight,
            _ => KeyCode::Unknown,
        },
        _ => KeyCode::Unknown,
    }
}

/// Like [`convert_key_code`], for a platform whose Back button arrives as a
/// key: Android's does, as winit's logical `BrowserBack` with *no* physical
/// key (`to_physical_key` has no row for it), so [`convert_key_code`] alone
/// reads it as `Unknown` and the press is dropped -- while winit has already
/// told the system the key was handled, so the activity does not finish
/// either, and Back did nothing at all.
///
/// Back becomes [`KeyCode::Escape`], as Unity reports it (`KeyCode.Escape`)
/// and as Godot's `ui_cancel` is its go-back: the "cancel / open the menu"
/// key, which a game with a pause menu already handles. The window layer calls
/// this only on Android; on a desktop keyboard a "Browser Back" media key is
/// not a request to pause the game.
pub fn convert_key_code_with_back(physical: PhysicalKey, logical: &Key) -> KeyCode {
    match logical {
        Key::Named(NamedKey::BrowserBack) => KeyCode::Escape,
        _ => convert_key_code(physical),
    }
}

/// Maps a winit `ElementState` (pressed/released) to BSEngine's own `ElementState`.
pub fn convert_element_state(state: WinitState) -> ElementState {
    match state {
        WinitState::Pressed => ElementState::Pressed,
        WinitState::Released => ElementState::Released,
    }
}

/// Maps a winit touch (`WindowEvent::Touch`) to BSEngine's `TouchInput`.
pub fn convert_touch(touch: &winit::event::Touch) -> TouchInput {
    use winit::event::TouchPhase as WinitPhase;
    TouchInput {
        id: touch.id,
        event: match touch.phase {
            WinitPhase::Started => TouchEvent::Started,
            WinitPhase::Moved => TouchEvent::Moved,
            WinitPhase::Ended => TouchEvent::Ended,
            WinitPhase::Cancelled => TouchEvent::Cancelled,
        },
        x: touch.location.x,
        y: touch.location.y,
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

    /// Back is Escape, and nothing else is changed by asking for it: the
    /// physical key still decides every other key, and a key that is not Back
    /// with no physical key stays `Unknown`. The premise: without the logical
    /// key, the very same physical key (`Unidentified`, what Android sends for
    /// Back) is `Unknown`.
    #[test]
    fn back_is_escape_and_only_back() {
        use winit::keyboard::{NativeKeyCode, SmolStr};
        let none = PhysicalKey::Unidentified(NativeKeyCode::Unidentified);
        let back = Key::Named(NamedKey::BrowserBack);
        assert_eq!(convert_key_code(none), KeyCode::Unknown);
        assert_eq!(convert_key_code_with_back(none, &back), KeyCode::Escape);

        let w = PhysicalKey::Code(WinitKeyCode::KeyW);
        assert_eq!(
            convert_key_code_with_back(w, &Key::Character(SmolStr::new("w"))),
            KeyCode::W
        );
        assert_eq!(
            convert_key_code_with_back(none, &Key::Named(NamedKey::BrowserForward)),
            KeyCode::Unknown
        );
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
    fn convert_touch_keeps_id_position_and_phase() {
        use winit::event::{Touch, TouchPhase as WinitPhase};
        for (phase, event) in [
            (WinitPhase::Started, TouchEvent::Started),
            (WinitPhase::Moved, TouchEvent::Moved),
            (WinitPhase::Ended, TouchEvent::Ended),
            (WinitPhase::Cancelled, TouchEvent::Cancelled),
        ] {
            let touch = Touch {
                device_id: winit::event::DeviceId::dummy(),
                phase,
                location: winit::dpi::PhysicalPosition::new(12.5, 34.0),
                force: None,
                id: 9,
            };
            let converted = convert_touch(&touch);
            assert_eq!(converted.event, event);
            assert_eq!((converted.id, converted.x, converted.y), (9, 12.5, 34.0));
        }
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
