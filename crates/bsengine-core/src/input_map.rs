use bevy_ecs::prelude::Component;
use std::collections::HashMap;

/// A single raw input code — keyboard key, mouse button, or gamepad button.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InputCode {
    Key(u32),
    MouseButton(u8),
    GamepadButton { pad: u8, button: u8 },
}

/// Binding state for one action: primary key + optional alternate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Binding {
    pub primary: InputCode,
    pub alternate: Option<InputCode>,
}

impl Binding {
    pub fn key(key_code: u32) -> Self {
        Self {
            primary: InputCode::Key(key_code),
            alternate: None,
        }
    }

    pub fn mouse(button: u8) -> Self {
        Self {
            primary: InputCode::MouseButton(button),
            alternate: None,
        }
    }

    pub fn with_alternate(mut self, code: InputCode) -> Self {
        self.alternate = Some(code);
        self
    }

    /// Returns `true` if `code` matches primary or alternate.
    pub fn matches(&self, code: InputCode) -> bool {
        self.primary == code || self.alternate == Some(code)
    }
}

/// Maps named game actions (e.g. `"jump"`, `"fire"`) to input bindings.
/// Systems query this component to translate raw input codes into semantic actions.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct InputMap {
    bindings: HashMap<String, Binding>,
    pub enabled: bool,
}

impl InputMap {
    pub fn new() -> Self {
        Self {
            bindings: HashMap::new(),
            enabled: true,
        }
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    pub fn bind(mut self, action: impl Into<String>, binding: Binding) -> Self {
        self.bindings.insert(action.into(), binding);
        self
    }

    /// Add or overwrite a binding at runtime.
    pub fn set(&mut self, action: impl Into<String>, binding: Binding) {
        self.bindings.insert(action.into(), binding);
    }

    /// Remove a binding, returning it if it existed.
    pub fn unset(&mut self, action: &str) -> Option<Binding> {
        self.bindings.remove(action)
    }

    /// Retrieve the binding for an action.
    pub fn get(&self, action: &str) -> Option<&Binding> {
        self.bindings.get(action)
    }

    /// Returns `true` if the given raw code triggers `action`.
    pub fn is_action(&self, action: &str, code: InputCode) -> bool {
        if !self.enabled {
            return false;
        }
        self.bindings.get(action).map_or(false, |b| b.matches(code))
    }

    pub fn action_count(&self) -> usize {
        self.bindings.len()
    }
}

impl Default for InputMap {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn input_map_bind_and_get() {
        let map = InputMap::new().bind("jump", Binding::key(32));
        assert!(map.get("jump").is_some());
        assert_eq!(map.action_count(), 1);
    }

    #[test]
    fn is_action_primary_match() {
        let map = InputMap::new().bind("fire", Binding::mouse(0));
        assert!(map.is_action("fire", InputCode::MouseButton(0)));
    }

    #[test]
    fn is_action_alternate_match() {
        let map = InputMap::new().bind(
            "dodge",
            Binding::key(16).with_alternate(InputCode::GamepadButton { pad: 0, button: 1 }),
        );
        assert!(map.is_action("dodge", InputCode::GamepadButton { pad: 0, button: 1 }));
    }

    #[test]
    fn set_and_unset() {
        let mut map = InputMap::new().bind("use", Binding::key(69));
        map.set("crouch", Binding::key(17));
        assert_eq!(map.action_count(), 2);
        map.unset("use");
        assert_eq!(map.action_count(), 1);
    }

    #[test]
    fn disabled_map_never_matches() {
        let map = InputMap::new().bind("jump", Binding::key(32)).disabled();
        assert!(!map.is_action("jump", InputCode::Key(32)));
    }
}
