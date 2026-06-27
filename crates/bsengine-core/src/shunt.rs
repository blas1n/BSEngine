use bevy_ecs::prelude::Component;

/// Forced positional displacement — tracks how shuntable an entity is and
/// gates shunts with a per-entity cooldown. When `try_shunt(raw_magnitude)`
/// succeeds, the caller is expected to translate the entity by
/// `effective_magnitude(raw_magnitude)` in whatever direction makes sense for
/// the game; this component records the event and starts the cooldown.
///
/// `try_shunt(raw_magnitude)` succeeds when the component is enabled and not
/// on cooldown: increments `shunts_received`, stores
/// `last_shunt_magnitude = raw_magnitude * (1 - shunt_resistance)`, resets
/// `cooldown_timer` to `cooldown`, and fires `just_shunted`. Returns `true`
/// on success, `false` when disabled, on cooldown, or `raw_magnitude ≤ 0`.
///
/// `tick(dt)` clears `just_shunted` at the start; counts down `cooldown_timer`
/// (floored at 0.0). No-op when disabled.
///
/// `effective_magnitude(raw)` returns `raw * (1.0 - shunt_resistance)` —
/// the fraction of the raw displacement that actually affects the entity.
/// Independent of enabled state (pure query).
///
/// `can_be_shunted()` returns `enabled && !is_on_cooldown()`.
///
/// `is_on_cooldown()` returns `cooldown_timer > 0.0`.
///
/// Distinct from `Knockback` (physics impulse that applies force through the
/// physics engine), `Repel` (continuous radial force field), and
/// `Root` (prevents voluntary movement): Shunt is a **discrete positional
/// override** — an instantaneous forced relocation (teleport-style) with
/// per-entity resistance and a cooldown that prevents spam.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Shunt {
    /// Fraction of incoming displacement resisted. Clamped [0.0, 1.0].
    /// 0.0 = freely shunted; 1.0 = immune to shunting.
    pub shunt_resistance: f32,
    /// Effective magnitude of the most recent successful shunt.
    pub last_shunt_magnitude: f32,
    /// Total number of shunts that have succeeded.
    pub shunts_received: u32,
    /// Remaining cooldown in seconds before the entity can be shunted again.
    pub cooldown_timer: f32,
    /// Cooldown applied after each shunt. Clamped >= 0.0.
    pub cooldown: f32,
    pub just_shunted: bool,
    pub enabled: bool,
}

impl Shunt {
    pub fn new(shunt_resistance: f32, cooldown: f32) -> Self {
        Self {
            shunt_resistance: shunt_resistance.clamp(0.0, 1.0),
            last_shunt_magnitude: 0.0,
            shunts_received: 0,
            cooldown_timer: 0.0,
            cooldown: cooldown.max(0.0),
            just_shunted: false,
            enabled: true,
        }
    }

    /// Attempt to shunt the entity by `raw_magnitude` units. Succeeds when
    /// enabled, not on cooldown, and `raw_magnitude > 0`. On success, records
    /// the event, resets the cooldown timer, and fires `just_shunted`.
    /// Returns `true` on success, `false` otherwise.
    pub fn try_shunt(&mut self, raw_magnitude: f32) -> bool {
        if !self.enabled || self.is_on_cooldown() || raw_magnitude <= 0.0 {
            return false;
        }
        self.shunts_received += 1;
        self.last_shunt_magnitude = self.effective_magnitude(raw_magnitude);
        self.cooldown_timer = self.cooldown;
        self.just_shunted = true;
        true
    }

    /// Advance the cooldown timer. Clears `just_shunted` at start; counts
    /// down `cooldown_timer`. No-op when disabled.
    pub fn tick(&mut self, dt: f32) {
        self.just_shunted = false;

        if !self.enabled {
            return;
        }

        if self.cooldown_timer > 0.0 {
            self.cooldown_timer = (self.cooldown_timer - dt).max(0.0);
        }
    }

    /// `true` when the entity is enabled and not on cooldown.
    pub fn can_be_shunted(&self) -> bool {
        self.enabled && !self.is_on_cooldown()
    }

    /// `true` when the cooldown timer is still running.
    pub fn is_on_cooldown(&self) -> bool {
        self.cooldown_timer > 0.0
    }

    /// Returns the portion of `raw` that bypasses resistance.
    /// `raw * (1.0 - shunt_resistance)`, independent of enabled state.
    pub fn effective_magnitude(&self, raw: f32) -> f32 {
        raw * (1.0 - self.shunt_resistance)
    }
}

impl Default for Shunt {
    fn default() -> Self {
        Self::new(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_starts_ready() {
        let s = Shunt::new(0.0, 1.0);
        assert!(s.can_be_shunted());
        assert!(!s.is_on_cooldown());
        assert_eq!(s.shunts_received, 0);
    }

    #[test]
    fn try_shunt_returns_true_on_success() {
        let mut s = Shunt::new(0.0, 1.0);
        assert!(s.try_shunt(5.0));
    }

    #[test]
    fn try_shunt_increments_counter() {
        let mut s = Shunt::new(0.0, 1.0);
        s.try_shunt(5.0);
        assert_eq!(s.shunts_received, 1);
        s.tick(2.0); // cooldown expires
        s.try_shunt(5.0);
        assert_eq!(s.shunts_received, 2);
    }

    #[test]
    fn try_shunt_records_effective_magnitude() {
        let mut s = Shunt::new(0.4, 1.0);
        s.try_shunt(10.0);
        // 10.0 * (1 - 0.4) = 6.0
        assert!((s.last_shunt_magnitude - 6.0).abs() < 1e-5);
    }

    #[test]
    fn try_shunt_fires_just_shunted() {
        let mut s = Shunt::new(0.0, 1.0);
        s.try_shunt(5.0);
        assert!(s.just_shunted);
    }

    #[test]
    fn try_shunt_starts_cooldown() {
        let mut s = Shunt::new(0.0, 2.0);
        s.try_shunt(5.0);
        assert!(s.is_on_cooldown());
        assert!((s.cooldown_timer - 2.0).abs() < 1e-5);
    }

    #[test]
    fn try_shunt_fails_when_on_cooldown() {
        let mut s = Shunt::new(0.0, 5.0);
        s.try_shunt(5.0);
        assert!(!s.try_shunt(5.0)); // still on cooldown
        assert_eq!(s.shunts_received, 1);
    }

    #[test]
    fn try_shunt_fails_when_disabled() {
        let mut s = Shunt::new(0.0, 1.0);
        s.enabled = false;
        assert!(!s.try_shunt(5.0));
        assert_eq!(s.shunts_received, 0);
    }

    #[test]
    fn try_shunt_fails_when_magnitude_zero() {
        let mut s = Shunt::new(0.0, 1.0);
        assert!(!s.try_shunt(0.0));
    }

    #[test]
    fn try_shunt_fails_when_magnitude_negative() {
        let mut s = Shunt::new(0.0, 1.0);
        assert!(!s.try_shunt(-1.0));
    }

    #[test]
    fn tick_clears_just_shunted() {
        let mut s = Shunt::new(0.0, 2.0);
        s.try_shunt(5.0);
        s.tick(0.016);
        assert!(!s.just_shunted);
    }

    #[test]
    fn tick_counts_down_cooldown() {
        let mut s = Shunt::new(0.0, 2.0);
        s.try_shunt(5.0);
        s.tick(1.0);
        assert!((s.cooldown_timer - 1.0).abs() < 1e-5);
        assert!(s.is_on_cooldown());
    }

    #[test]
    fn tick_clears_cooldown_on_expiry() {
        let mut s = Shunt::new(0.0, 1.0);
        s.try_shunt(5.0);
        s.tick(1.0);
        assert!(!s.is_on_cooldown());
        assert!(s.can_be_shunted());
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut s = Shunt::new(0.0, 2.0);
        s.try_shunt(5.0);
        s.enabled = false;
        s.tick(10.0); // should not clear cooldown
        assert!((s.cooldown_timer - 2.0).abs() < 1e-5);
    }

    #[test]
    fn tick_no_op_when_no_cooldown() {
        let mut s = Shunt::new(0.0, 0.0);
        s.tick(1.0); // no panic
        assert_eq!(s.cooldown_timer, 0.0);
    }

    #[test]
    fn can_be_shunted_false_on_cooldown() {
        let mut s = Shunt::new(0.0, 5.0);
        s.try_shunt(5.0);
        assert!(!s.can_be_shunted());
    }

    #[test]
    fn can_be_shunted_false_when_disabled() {
        let mut s = Shunt::new(0.0, 1.0);
        s.enabled = false;
        assert!(!s.can_be_shunted());
    }

    #[test]
    fn can_be_shunted_true_after_cooldown_expires() {
        let mut s = Shunt::new(0.0, 1.0);
        s.try_shunt(5.0);
        s.tick(1.0);
        assert!(s.can_be_shunted());
    }

    #[test]
    fn effective_magnitude_no_resistance() {
        let s = Shunt::new(0.0, 1.0);
        assert!((s.effective_magnitude(10.0) - 10.0).abs() < 1e-5);
    }

    #[test]
    fn effective_magnitude_half_resistance() {
        let s = Shunt::new(0.5, 1.0);
        // 10 * 0.5 = 5
        assert!((s.effective_magnitude(10.0) - 5.0).abs() < 1e-5);
    }

    #[test]
    fn effective_magnitude_full_resistance_immune() {
        let s = Shunt::new(1.0, 1.0);
        assert!((s.effective_magnitude(10.0)).abs() < 1e-5);
    }

    #[test]
    fn effective_magnitude_independent_of_enabled() {
        let mut s = Shunt::new(0.5, 1.0);
        s.enabled = false;
        assert!((s.effective_magnitude(10.0) - 5.0).abs() < 1e-5);
    }

    #[test]
    fn shunt_resistance_clamped_to_one() {
        let s = Shunt::new(2.0, 1.0);
        assert!((s.shunt_resistance - 1.0).abs() < 1e-5);
    }

    #[test]
    fn shunt_resistance_clamped_to_zero() {
        let s = Shunt::new(-0.5, 1.0);
        assert_eq!(s.shunt_resistance, 0.0);
    }

    #[test]
    fn cooldown_clamped_to_zero() {
        let s = Shunt::new(0.0, -1.0);
        assert_eq!(s.cooldown, 0.0);
    }

    #[test]
    fn zero_cooldown_allows_immediate_reshunt() {
        let mut s = Shunt::new(0.0, 0.0);
        s.try_shunt(5.0);
        // cooldown_timer = 0 → not on cooldown
        assert!(!s.is_on_cooldown());
        assert!(s.try_shunt(5.0));
        assert_eq!(s.shunts_received, 2);
    }

    #[test]
    fn full_resistance_records_zero_last_shunt() {
        let mut s = Shunt::new(1.0, 0.0);
        s.try_shunt(20.0);
        assert!((s.last_shunt_magnitude).abs() < 1e-5);
    }

    #[test]
    fn can_reshunt_after_cooldown() {
        let mut s = Shunt::new(0.0, 1.0);
        s.try_shunt(5.0);
        s.tick(1.0); // cooldown expires
        assert!(s.try_shunt(8.0));
        assert_eq!(s.shunts_received, 2);
        assert!((s.last_shunt_magnitude - 8.0).abs() < 1e-5);
    }
}
