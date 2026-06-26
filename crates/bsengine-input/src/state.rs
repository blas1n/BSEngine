use std::collections::HashSet;
use std::hash::Hash;

use bevy_ecs::prelude::Resource;

/// Persistent per-frame input state for a button-like type (key or mouse button).
///
/// Updated once per frame from the corresponding event stream. Use this for
/// continuous checks (hold-to-move) and for detecting frame-boundary transitions.
#[derive(Resource)]
pub struct Input<T: Eq + Hash> {
    pressed: HashSet<T>,
    just_pressed: HashSet<T>,
    just_released: HashSet<T>,
}

impl<T: Eq + Hash> Default for Input<T> {
    fn default() -> Self {
        Self {
            pressed: HashSet::new(),
            just_pressed: HashSet::new(),
            just_released: HashSet::new(),
        }
    }
}

impl<T: Eq + Hash + Clone> Input<T> {
    pub fn is_pressed(&self, key: &T) -> bool {
        self.pressed.contains(key)
    }

    pub fn just_pressed(&self, key: &T) -> bool {
        self.just_pressed.contains(key)
    }

    pub fn just_released(&self, key: &T) -> bool {
        self.just_released.contains(key)
    }

    pub(crate) fn press(&mut self, key: T) {
        if !self.pressed.contains(&key) {
            self.just_pressed.insert(key.clone());
        }
        self.pressed.insert(key);
    }

    pub(crate) fn release(&mut self, key: T) {
        self.pressed.remove(&key);
        self.just_released.insert(key);
    }

    pub(crate) fn clear_transient(&mut self) {
        self.just_pressed.clear();
        self.just_released.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    enum TestKey {
        A,
        B,
    }

    #[test]
    fn press_sets_is_pressed_and_just_pressed() {
        let mut input = Input::<TestKey>::default();
        input.press(TestKey::A);
        assert!(input.is_pressed(&TestKey::A));
        assert!(input.just_pressed(&TestKey::A));
        assert!(!input.just_released(&TestKey::A));
    }

    #[test]
    fn clear_removes_just_pressed_but_keeps_pressed() {
        let mut input = Input::<TestKey>::default();
        input.press(TestKey::A);
        input.clear_transient();
        assert!(input.is_pressed(&TestKey::A));
        assert!(!input.just_pressed(&TestKey::A));
    }

    #[test]
    fn release_removes_pressed_and_sets_just_released() {
        let mut input = Input::<TestKey>::default();
        input.press(TestKey::A);
        input.clear_transient();
        input.release(TestKey::A);
        assert!(!input.is_pressed(&TestKey::A));
        assert!(input.just_released(&TestKey::A));
    }

    #[test]
    fn double_press_does_not_add_second_just_pressed() {
        let mut input = Input::<TestKey>::default();
        input.press(TestKey::A);
        input.press(TestKey::A);
        assert!(input.just_pressed(&TestKey::A));
        assert_eq!(input.just_pressed.len(), 1);
    }
}
