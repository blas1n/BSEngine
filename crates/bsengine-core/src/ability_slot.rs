use bevy_ecs::prelude::Component;

/// One slot in an ability bar — maps a hotkey index to an optional ability name.
#[derive(Debug, Clone, PartialEq)]
pub struct Slot {
    /// Hotkey index (0 = first slot, 1 = second, …).
    pub index: u8,
    /// The ability bound to this slot, identified by name.
    pub ability_name: Option<String>,
    /// Whether this slot is currently locked (cannot be rebound or activated).
    pub locked: bool,
}

impl Slot {
    pub fn empty(index: u8) -> Self {
        Self {
            index,
            ability_name: None,
            locked: false,
        }
    }

    pub fn bound(index: u8, ability: impl Into<String>) -> Self {
        Self {
            index,
            ability_name: Some(ability.into()),
            locked: false,
        }
    }

    pub fn locked(mut self) -> Self {
        self.locked = true;
        self
    }

    pub fn is_empty(&self) -> bool {
        self.ability_name.is_none()
    }
}

/// Hotbar / action bar for a character entity.
/// The UI and input system use this to look up which ability to activate
/// when the player presses a hotkey.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct AbilitySlot {
    pub slots: Vec<Slot>,
    pub enabled: bool,
}

impl AbilitySlot {
    /// Create a bar with `count` empty slots.
    pub fn new(count: u8) -> Self {
        Self {
            slots: (0..count).map(Slot::empty).collect(),
            enabled: true,
        }
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Bind `ability_name` to slot `index`. Returns `false` if slot is locked or out of range.
    pub fn bind(&mut self, index: u8, ability_name: impl Into<String>) -> bool {
        if let Some(slot) = self.slots.iter_mut().find(|s| s.index == index) {
            if slot.locked {
                return false;
            }
            slot.ability_name = Some(ability_name.into());
            return true;
        }
        false
    }

    /// Clear the ability from slot `index`. Returns `false` if locked or out of range.
    pub fn unbind(&mut self, index: u8) -> bool {
        if let Some(slot) = self.slots.iter_mut().find(|s| s.index == index) {
            if slot.locked {
                return false;
            }
            slot.ability_name = None;
            return true;
        }
        false
    }

    /// Look up the ability bound to `index`.
    pub fn get(&self, index: u8) -> Option<&str> {
        self.slots
            .iter()
            .find(|s| s.index == index)
            .and_then(|s| s.ability_name.as_deref())
    }

    pub fn slot_count(&self) -> usize {
        self.slots.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ability_slot_bind_and_get() {
        let mut bar = AbilitySlot::new(4);
        assert!(bar.bind(0, "fireball"));
        assert_eq!(bar.get(0), Some("fireball"));
    }

    #[test]
    fn ability_slot_unbind() {
        let mut bar = AbilitySlot::new(4);
        bar.bind(1, "dash");
        assert!(bar.unbind(1));
        assert_eq!(bar.get(1), None);
    }

    #[test]
    fn ability_slot_locked_rejects_bind() {
        let mut bar = AbilitySlot::new(2);
        bar.slots[0].locked = true;
        assert!(!bar.bind(0, "heal"));
    }

    #[test]
    fn ability_slot_out_of_range() {
        let mut bar = AbilitySlot::new(2);
        assert!(!bar.bind(5, "some_ability"));
        assert_eq!(bar.get(5), None);
    }

    #[test]
    fn ability_slot_count() {
        let bar = AbilitySlot::new(6);
        assert_eq!(bar.slot_count(), 6);
    }
}
