use bevy_ecs::prelude::{Component, Entity};

/// Links this entity to a destination portal entity.
/// The physics/trigger system teleports any entity that enters this portal's volume
/// to the position and orientation of the linked `destination` entity.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Portal {
    /// The entity whose transform defines where travelers exit.
    /// `None` disables teleportation until a destination is assigned.
    pub destination: Option<Entity>,
    /// When true, preserve the traveler's velocity relative to this portal's frame,
    /// re-emitting it from the destination's frame. When false, velocity is cleared.
    pub preserve_velocity: bool,
    /// Minimum seconds between two teleportations of the same entity.
    /// Prevents immediately teleporting back through the exit portal.
    pub cooldown: f32,
    pub enabled: bool,
}

impl Portal {
    /// Portal with a known destination.
    pub fn to(destination: Entity) -> Self {
        Self {
            destination: Some(destination),
            ..Self::default()
        }
    }

    pub fn with_cooldown(mut self, seconds: f32) -> Self {
        self.cooldown = seconds.max(0.0);
        self
    }

    pub fn without_velocity(mut self) -> Self {
        self.preserve_velocity = false;
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Returns `true` if this portal can teleport entities right now.
    pub fn is_active(&self) -> bool {
        self.enabled && self.destination.is_some()
    }
}

impl Default for Portal {
    fn default() -> Self {
        Self {
            destination: None,
            preserve_velocity: true,
            cooldown: 0.5,
            enabled: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::world::World;

    fn make_entity() -> Entity {
        let mut world = World::new();
        world.spawn_empty().id()
    }

    #[test]
    fn portal_defaults() {
        let p = Portal::default();
        assert!(p.destination.is_none());
        assert!(p.preserve_velocity);
        assert!((p.cooldown - 0.5).abs() < 0.001);
        assert!(p.enabled);
    }

    #[test]
    fn portal_to_destination() {
        let dest = make_entity();
        let p = Portal::to(dest);
        assert_eq!(p.destination, Some(dest));
        assert!(p.is_active());
    }

    #[test]
    fn no_destination_not_active() {
        let p = Portal::default();
        assert!(!p.is_active());
    }

    #[test]
    fn cooldown_clamped() {
        let dest = make_entity();
        let p = Portal::to(dest).with_cooldown(-1.0);
        assert_eq!(p.cooldown, 0.0);
    }

    #[test]
    fn disabled_not_active() {
        let dest = make_entity();
        let p = Portal::to(dest).disabled();
        assert!(!p.is_active());
    }
}
