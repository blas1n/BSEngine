use bevy_ecs::prelude::Component;

/// Stationary ambush posture: entity hides in place with reduced detection
/// range and deals bonus damage on the first strike it lands from concealment.
///
/// Call `enter()` to adopt the lurk posture (`just_lurked` fires once).
/// While `is_lurking()`, detection systems should apply
/// `effective_detection_range(base)` to compute how far enemies can spot this
/// entity. When the entity attacks, call `strike()` — it returns the ambush
/// multiplier, sets `just_struck`, and automatically exits the lurk posture.
///
/// `tick()` clears the one-frame flags `just_lurked` and `just_struck`.
/// `exit()` ends lurking without consuming the ambush bonus.
///
/// Distinct from `Stealth` (active full invisibility, may move freely),
/// `Ghost` (pass-through objects), and `Shroud` (area-denial fog): Lurk is
/// specifically a **stationary ambush posture** — the entity waits motionless
/// and is rewarded with a burst multiplier on the first strike that breaks
/// concealment.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Lurk {
    /// Fraction by which detection range is reduced while lurking.
    /// Clamped to [0.0, 1.0]. e.g. 0.7 = enemies detect this entity at 30%
    /// of their normal range.
    pub detection_range_fraction: f32,
    /// Damage multiplier applied to the first strike from lurk.
    /// Clamped ≥ 1.0.
    pub ambush_multiplier: f32,
    pub lurking: bool,
    pub just_lurked: bool,
    pub just_struck: bool,
    pub enabled: bool,
}

impl Lurk {
    pub fn new(detection_range_fraction: f32, ambush_multiplier: f32) -> Self {
        Self {
            detection_range_fraction: detection_range_fraction.clamp(0.0, 1.0),
            ambush_multiplier: ambush_multiplier.max(1.0),
            lurking: false,
            just_lurked: false,
            just_struck: false,
            enabled: true,
        }
    }

    /// Adopt the lurk posture. No-op when already lurking or disabled.
    pub fn enter(&mut self) {
        if !self.enabled || self.lurking {
            return;
        }
        self.lurking = true;
        self.just_lurked = true;
    }

    /// Leave the lurk posture without consuming the ambush bonus (e.g., the
    /// entity is forced out of hiding by damage or movement).
    pub fn exit(&mut self) {
        self.lurking = false;
    }

    /// Consume the ambush bonus: exits lurk, sets `just_struck`, and returns
    /// `ambush_multiplier`. No-op when not lurking or disabled — returns
    /// `1.0` in that case.
    pub fn strike(&mut self) -> f32 {
        if !self.lurking || !self.enabled {
            return 1.0;
        }
        self.lurking = false;
        self.just_struck = true;
        self.ambush_multiplier
    }

    /// Clear one-frame flags. Call once per game tick.
    pub fn tick(&mut self) {
        self.just_lurked = false;
        self.just_struck = false;
    }

    pub fn is_lurking(&self) -> bool {
        self.lurking
    }

    /// Detection range an enemy can spot this entity from while lurking.
    /// Returns `base * (1.0 - detection_range_fraction)` when lurking and
    /// enabled, `base` otherwise.
    pub fn effective_detection_range(&self, base: f32) -> f32 {
        if self.lurking && self.enabled {
            base * (1.0 - self.detection_range_fraction)
        } else {
            base
        }
    }

    /// Damage that would be dealt on an ambush strike at `base_damage`.
    /// Returns `base_damage * ambush_multiplier` when lurking and enabled,
    /// `base_damage` otherwise. Does NOT consume the bonus — call `strike()`
    /// to both query and consume.
    pub fn ambush_damage(&self, base_damage: f32) -> f32 {
        if self.lurking && self.enabled {
            base_damage * self.ambush_multiplier
        } else {
            base_damage
        }
    }
}

impl Default for Lurk {
    fn default() -> Self {
        Self::new(0.7, 2.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enter_starts_lurking() {
        let mut l = Lurk::new(0.7, 2.5);
        l.enter();
        assert!(l.is_lurking());
        assert!(l.just_lurked);
    }

    #[test]
    fn enter_no_op_when_already_lurking() {
        let mut l = Lurk::new(0.7, 2.5);
        l.enter();
        l.tick();
        l.enter(); // already lurking — no-op
        assert!(!l.just_lurked);
    }

    #[test]
    fn exit_ends_lurking() {
        let mut l = Lurk::new(0.7, 2.5);
        l.enter();
        l.exit();
        assert!(!l.is_lurking());
    }

    #[test]
    fn strike_consumes_bonus_and_exits() {
        let mut l = Lurk::new(0.7, 2.5);
        l.enter();
        let mult = l.strike();
        assert!((mult - 2.5).abs() < 1e-5);
        assert!(!l.is_lurking());
        assert!(l.just_struck);
    }

    #[test]
    fn strike_returns_one_when_not_lurking() {
        let mut l = Lurk::new(0.7, 2.5);
        let mult = l.strike();
        assert!((mult - 1.0).abs() < 1e-5);
        assert!(!l.just_struck);
    }

    #[test]
    fn tick_clears_just_lurked() {
        let mut l = Lurk::new(0.7, 2.5);
        l.enter();
        l.tick();
        assert!(!l.just_lurked);
    }

    #[test]
    fn tick_clears_just_struck() {
        let mut l = Lurk::new(0.7, 2.5);
        l.enter();
        l.strike();
        l.tick();
        assert!(!l.just_struck);
    }

    #[test]
    fn detection_range_reduced_while_lurking() {
        let mut l = Lurk::new(0.7, 2.5);
        l.enter();
        // 20.0 * (1.0 - 0.7) = 6.0
        assert!((l.effective_detection_range(20.0) - 6.0).abs() < 1e-4);
    }

    #[test]
    fn detection_range_full_when_not_lurking() {
        let l = Lurk::new(0.7, 2.5);
        assert!((l.effective_detection_range(20.0) - 20.0).abs() < 1e-5);
    }

    #[test]
    fn ambush_damage_scaled_while_lurking() {
        let mut l = Lurk::new(0.7, 2.5);
        l.enter();
        assert!((l.ambush_damage(40.0) - 100.0).abs() < 1e-3); // 40 * 2.5
    }

    #[test]
    fn ambush_damage_base_when_not_lurking() {
        let l = Lurk::new(0.7, 2.5);
        assert!((l.ambush_damage(40.0) - 40.0).abs() < 1e-5);
    }

    #[test]
    fn ambush_damage_does_not_consume_bonus() {
        let mut l = Lurk::new(0.7, 2.5);
        l.enter();
        l.ambush_damage(40.0);
        assert!(l.is_lurking()); // still lurking
    }

    #[test]
    fn detection_range_fraction_clamped() {
        let l = Lurk::new(1.5, 2.5); // > 1.0 → clamped to 1.0
        assert!((l.detection_range_fraction - 1.0).abs() < 1e-5);
        let l2 = Lurk::new(-0.5, 2.5); // < 0.0 → clamped to 0.0
        assert!((l2.detection_range_fraction - 0.0).abs() < 1e-5);
    }

    #[test]
    fn ambush_multiplier_clamped() {
        let l = Lurk::new(0.7, 0.5); // < 1.0 → clamped to 1.0
        assert!((l.ambush_multiplier - 1.0).abs() < 1e-5);
    }

    #[test]
    fn disabled_enter_no_op() {
        let mut l = Lurk::new(0.7, 2.5);
        l.enabled = false;
        l.enter();
        assert!(!l.is_lurking());
    }

    #[test]
    fn disabled_strike_returns_one() {
        let mut l = Lurk::new(0.7, 2.5);
        l.enter();
        l.enabled = false;
        let mult = l.strike();
        assert!((mult - 1.0).abs() < 1e-5);
        assert!(l.is_lurking()); // still lurking — disabled strike is no-op
    }

    #[test]
    fn disabled_detection_range_unaffected() {
        let mut l = Lurk::new(0.7, 2.5);
        l.enter();
        l.enabled = false;
        assert!((l.effective_detection_range(20.0) - 20.0).abs() < 1e-5);
    }
}
