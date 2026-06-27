use bevy_ecs::prelude::Component;

/// Character movement-speed stats with layered modifiers.
///
/// Keeps the base speed and two modifier layers separate so systems can
/// reason about each layer independently:
///
/// - **additive** — flat bonuses/penalties (boots, debuffs).
/// - **multiplier** — fractional scale applied last (haste, slow, stun).
///
/// `effective()` = `(base + additive).max(0.0) * multiplier.max(0.0)`
///
/// Distinct from `Velocity` (current frame velocity) and `Haste` (stack-
/// based multiplicative ramp). `MoveSpeed` is the *target* speed ceiling
/// that locomotion systems read when computing desired velocity.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct MoveSpeed {
    /// Base speed in world-units per second.
    pub base: f32,
    /// Summed flat additions (may be negative).
    pub additive: f32,
    /// Multiplicative scale; 1.0 = no change, 0.5 = half speed.
    pub multiplier: f32,
    pub enabled: bool,
}

impl MoveSpeed {
    pub fn new(base: f32) -> Self {
        Self {
            base: base.max(0.0),
            additive: 0.0,
            multiplier: 1.0,
            enabled: true,
        }
    }

    pub fn with_additive(mut self, value: f32) -> Self {
        self.additive += value;
        self
    }

    pub fn with_multiplier(mut self, factor: f32) -> Self {
        self.multiplier *= factor.max(0.0);
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Add a flat value to the additive layer (may be negative).
    pub fn add_flat(&mut self, value: f32) {
        self.additive += value;
    }

    /// Scale the multiplier by `factor` (stacks multiplicatively).
    pub fn scale(&mut self, factor: f32) {
        self.multiplier *= factor.max(0.0);
    }

    /// Reset the additive layer to zero.
    pub fn clear_additive(&mut self) {
        self.additive = 0.0;
    }

    /// Reset the multiplier back to 1.0.
    pub fn reset_multiplier(&mut self) {
        self.multiplier = 1.0;
    }

    /// Compute the effective movement speed.
    pub fn effective(&self) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        (self.base + self.additive).max(0.0) * self.multiplier.max(0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_speed() {
        let ms = MoveSpeed::new(6.0);
        assert!((ms.effective() - 6.0).abs() < 1e-5);
    }

    #[test]
    fn flat_bonus() {
        let ms = MoveSpeed::new(6.0).with_additive(2.0);
        assert!((ms.effective() - 8.0).abs() < 1e-5);
    }

    #[test]
    fn flat_penalty_clamps_at_zero() {
        let ms = MoveSpeed::new(4.0).with_additive(-10.0);
        assert_eq!(ms.effective(), 0.0);
    }

    #[test]
    fn multiplier_scales() {
        let ms = MoveSpeed::new(10.0).with_multiplier(0.5);
        assert!((ms.effective() - 5.0).abs() < 1e-5);
    }

    #[test]
    fn stacked_multipliers() {
        // 10 * 0.8 * 0.5 = 4.0
        let ms = MoveSpeed::new(10.0)
            .with_multiplier(0.8)
            .with_multiplier(0.5);
        assert!((ms.effective() - 4.0).abs() < 1e-4);
    }

    #[test]
    fn reset_multiplier() {
        let mut ms = MoveSpeed::new(10.0).with_multiplier(0.5);
        ms.reset_multiplier();
        assert!((ms.effective() - 10.0).abs() < 1e-5);
    }

    #[test]
    fn disabled_returns_zero() {
        let ms = MoveSpeed::new(10.0).disabled();
        assert_eq!(ms.effective(), 0.0);
    }
}
