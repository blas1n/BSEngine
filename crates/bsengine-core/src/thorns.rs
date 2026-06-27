use bevy_ecs::prelude::{Component, Entity};

/// Reflects a portion of received melee/direct damage back to the attacker.
///
/// When the damage system calls `reflect(damage, source)`, Thorns calculates
/// how much should be sent back to `source` (via the damage system's counter-
/// damage queue) and returns that value. One-frame flags `just_reflected` and
/// `last_reflected_amount` let VFX/sound systems react.
///
/// `reflect_fraction` and `reflect_flat` combine additively:
///   reflected = clamp(damage * fraction + flat, 0, damage)
/// Setting `min_damage_to_trigger` prevents micro-hits from activating thorns.
///
/// `tick(dt)` clears the single-frame flags.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Thorns {
    /// Fraction [0.0, 1.0] of received damage reflected.
    pub reflect_fraction: f32,
    /// Flat reflected damage added on top of the fraction.
    pub reflect_flat: f32,
    /// Minimum incoming damage to trigger a reflection.
    pub min_damage_to_trigger: f32,
    /// Reflected damage is capped at this value (0.0 = no cap).
    pub max_reflect: f32,
    /// Source entity of the last reflected hit.
    pub last_source: Option<Entity>,
    /// Amount reflected on the last triggering hit.
    pub last_reflected_amount: f32,
    pub just_reflected: bool,
    pub enabled: bool,
}

impl Thorns {
    pub fn new(reflect_fraction: f32) -> Self {
        Self {
            reflect_fraction: reflect_fraction.clamp(0.0, 1.0),
            reflect_flat: 0.0,
            min_damage_to_trigger: 0.0,
            max_reflect: 0.0,
            last_source: None,
            last_reflected_amount: 0.0,
            just_reflected: false,
            enabled: true,
        }
    }

    pub fn with_flat(mut self, flat: f32) -> Self {
        self.reflect_flat = flat.max(0.0);
        self
    }

    pub fn with_min_trigger(mut self, min: f32) -> Self {
        self.min_damage_to_trigger = min.max(0.0);
        self
    }

    pub fn with_max_reflect(mut self, max: f32) -> Self {
        self.max_reflect = max.max(0.0);
        self
    }

    /// Calculate and record the reflected amount for the given incoming `damage`
    /// from `source`. Returns the damage to send back to the source entity.
    ///
    /// Returns 0.0 if disabled, below the trigger threshold, or source is None.
    pub fn reflect(&mut self, damage: f32, source: Option<Entity>) -> f32 {
        if !self.enabled || source.is_none() {
            return 0.0;
        }

        if damage < self.min_damage_to_trigger {
            return 0.0;
        }

        let mut reflected = damage * self.reflect_fraction + self.reflect_flat;
        reflected = reflected.min(damage); // can't reflect more than was received
        if self.max_reflect > 0.0 {
            reflected = reflected.min(self.max_reflect);
        }
        reflected = reflected.max(0.0);

        if reflected > 0.0 {
            self.last_source = source;
            self.last_reflected_amount = reflected;
            self.just_reflected = true;
        }

        reflected
    }

    /// Clear single-frame flags; call once per frame.
    pub fn tick(&mut self, _dt: f32) {
        self.just_reflected = false;
        self.last_reflected_amount = 0.0;
        self.last_source = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::entity::Entity;

    fn src() -> Entity {
        Entity::from_raw(1)
    }

    #[test]
    fn reflects_fraction() {
        let mut t = Thorns::new(0.3);
        let r = t.reflect(100.0, Some(src()));
        assert!((r - 30.0).abs() < 1e-4);
        assert!(t.just_reflected);
        assert!((t.last_reflected_amount - 30.0).abs() < 1e-4);
    }

    #[test]
    fn flat_adds_to_reflection() {
        let mut t = Thorns::new(0.1).with_flat(5.0);
        let r = t.reflect(100.0, Some(src()));
        assert!((r - 15.0).abs() < 1e-4);
    }

    #[test]
    fn cannot_reflect_more_than_damage() {
        let mut t = Thorns::new(1.0).with_flat(100.0);
        let r = t.reflect(20.0, Some(src()));
        assert!((r - 20.0).abs() < 1e-4);
    }

    #[test]
    fn max_reflect_caps_output() {
        let mut t = Thorns::new(0.5).with_max_reflect(10.0);
        let r = t.reflect(100.0, Some(src()));
        assert!((r - 10.0).abs() < 1e-4);
    }

    #[test]
    fn min_trigger_blocks_small_hits() {
        let mut t = Thorns::new(0.5).with_min_trigger(50.0);
        let r = t.reflect(30.0, Some(src()));
        assert!((r).abs() < 1e-4);
        assert!(!t.just_reflected);
    }

    #[test]
    fn no_source_returns_zero() {
        let mut t = Thorns::new(0.5);
        let r = t.reflect(100.0, None);
        assert!((r).abs() < 1e-4);
    }

    #[test]
    fn disabled_returns_zero() {
        let mut t = Thorns::new(0.5);
        t.enabled = false;
        let r = t.reflect(100.0, Some(src()));
        assert!((r).abs() < 1e-4);
    }

    #[test]
    fn tick_clears_flags() {
        let mut t = Thorns::new(0.5);
        t.reflect(100.0, Some(src()));
        t.tick(0.016);
        assert!(!t.just_reflected);
        assert!((t.last_reflected_amount).abs() < 1e-5);
    }
}
