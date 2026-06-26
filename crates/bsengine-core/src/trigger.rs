use bevy_ecs::prelude::{Component, Entity};

/// Events emitted by the physics system when bodies interact with a Trigger.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriggerEvent {
    /// A body entered the trigger volume this frame.
    Enter(Entity),
    /// A body stayed inside the trigger volume (reported every frame while overlapping).
    Stay(Entity),
    /// A body exited the trigger volume this frame.
    Exit(Entity),
}

/// A physics sensor (non-solid collider) that detects overlapping bodies.
/// The physics system fires `TriggerEvent`s for all bodies on matching `layer_mask` layers.
/// Does not block movement — overlapping entities pass through freely.
#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct Trigger {
    /// Bitmask of collision layers to detect (0 = detect nothing).
    pub layer_mask: u32,
    pub enabled: bool,
}

impl Trigger {
    /// Detects all layers (mask = all bits set).
    pub fn all_layers() -> Self {
        Self {
            layer_mask: u32::MAX,
            enabled: true,
        }
    }

    pub fn with_layer_mask(mut self, mask: u32) -> Self {
        self.layer_mask = mask;
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Returns `true` if the given layer index (0–31) is included in the mask.
    pub fn detects_layer(&self, layer: u32) -> bool {
        layer < 32 && (self.layer_mask & (1 << layer)) != 0
    }
}

impl Default for Trigger {
    fn default() -> Self {
        Self::all_layers()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_layers_default() {
        let t = Trigger::default();
        assert_eq!(t.layer_mask, u32::MAX);
        assert!(t.enabled);
    }

    #[test]
    fn custom_layer_mask() {
        let t = Trigger::all_layers().with_layer_mask(0b0111);
        assert!(t.detects_layer(0));
        assert!(t.detects_layer(1));
        assert!(t.detects_layer(2));
        assert!(!t.detects_layer(3));
    }

    #[test]
    fn layer_out_of_range_not_detected() {
        let t = Trigger::all_layers();
        assert!(!t.detects_layer(32));
    }

    #[test]
    fn disabled_flag() {
        let t = Trigger::all_layers().disabled();
        assert!(!t.enabled);
    }

    #[test]
    fn trigger_event_enter_exit() {
        use bevy_ecs::world::World;
        let mut world = World::new();
        let e = world.spawn_empty().id();
        let enter = TriggerEvent::Enter(e);
        let exit = TriggerEvent::Exit(e);
        assert_ne!(enter, exit);
    }
}
